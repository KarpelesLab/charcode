//! The charsets outside the WHATWG Encoding Standard.
//!
//! As with the standard's indexes, each test reconstructs what a byte should
//! decode to from the generated table, independently of the decoder, and then
//! round-trips every code point the encoder can produce.

use alloc::string::String;
use alloc::vec::Vec;

use crate::Encoding;

/// Checks a 128-entry table covering bytes 0x80 to 0xFF, with 0 for unmapped.
#[cfg(any(feature = "dos", feature = "mac", feature = "misc"))]
fn check_half(encoding: &'static Encoding, table: &[u16; 128]) {
    for byte in 0..=0x7Fu8 {
        assert_eq!(
            decode(encoding, &[byte]).as_deref(),
            Some(&*String::from(byte as char)),
            "{} byte {byte:02X} should be ASCII",
            encoding.name()
        );
    }
    for (i, &code_point) in table.iter().enumerate() {
        let byte = (i + 0x80) as u8;
        check_byte(
            encoding,
            byte,
            if code_point == 0 {
                None
            } else {
                Some(u32::from(code_point))
            },
        );
    }
}

/// Checks a 256-entry table, with 0xFFFF for unmapped.
#[cfg(any(feature = "dos", feature = "ebcdic"))]
fn check_full(encoding: &'static Encoding, table: &[u16; 256]) {
    for (byte, &code_point) in table.iter().enumerate() {
        let expected = if code_point == crate::full_byte::UNMAPPED {
            None
        } else {
            Some(u32::from(code_point))
        };
        check_byte(encoding, byte as u8, expected);
    }
}

#[cfg(any(feature = "dos", feature = "ebcdic", feature = "mac", feature = "misc"))]
fn check_byte(encoding: &'static Encoding, byte: u8, expected: Option<u32>) {
    let decoded = decode(encoding, &[byte]);
    match expected {
        Some(code_point) => {
            let c = char::from_u32(code_point).expect("tables hold scalar values");
            assert_eq!(
                decoded.as_deref(),
                Some(&*String::from(c)),
                "{} byte {byte:02X} should be U+{code_point:04X}",
                encoding.name()
            );
            // And it must come back as the same byte, unless the table maps two
            // bytes to it and this is the later one.
            let mut out = Vec::new();
            let mut string = String::new();
            string.push(c);
            encoding
                .new_encoder()
                .encode_from_str(&string, &mut out, true)
                .unwrap_or_else(|e| panic!("{} cannot encode {e}", encoding.name()));
            assert_eq!(out.len(), 1, "{} U+{code_point:04X}", encoding.name());
            assert_eq!(
                decode(encoding, &out).as_deref(),
                Some(&*String::from(c)),
                "{} U+{code_point:04X} does not round-trip",
                encoding.name()
            );
        }
        None => assert_eq!(
            decoded,
            None,
            "{} byte {byte:02X} should be unmapped",
            encoding.name()
        ),
    }
}

fn decode(encoding: &'static Encoding, bytes: &[u8]) -> Option<String> {
    encoding
        .try_decode(
            bytes,
            crate::DecodeOptions::new()
                .bom(crate::Bom::Ignore)
                .malformed(crate::Malformed::Fail),
        )
        .ok()
        .map(|(text, _, _)| text.into_owned())
}

#[cfg(feature = "dos")]
#[test]
fn dos_code_pages() {
    use crate::extra_encodings::*;
    use crate::tables::extra as t;
    for (encoding, table) in [
        (IBM437, &t::IBM437_DECODE),
        (IBM737, &t::IBM737_DECODE),
        (IBM775, &t::IBM775_DECODE),
        (IBM850, &t::IBM850_DECODE),
        (IBM852, &t::IBM852_DECODE),
        (IBM855, &t::IBM855_DECODE),
        (IBM856, &t::IBM856_DECODE),
        (IBM857, &t::IBM857_DECODE),
        (IBM860, &t::IBM860_DECODE),
        (IBM861, &t::IBM861_DECODE),
        (IBM862, &t::IBM862_DECODE),
        (IBM863, &t::IBM863_DECODE),
        (IBM865, &t::IBM865_DECODE),
        (IBM869, &t::IBM869_DECODE),
        (IBM1006, &t::IBM1006_DECODE),
    ] {
        check_half(encoding, table);
    }
    // CP864 reassigns an ASCII byte, so it carries a full table.
    check_full(IBM864, &t::IBM864_DECODE);
    // A spot check against the code page's best-known character.
    assert_eq!(decode(IBM437, b"\xDB").as_deref(), Some("\u{2588}"));
    assert_eq!(decode(IBM850, b"\xB0").as_deref(), Some("\u{2591}"));
}

#[cfg(feature = "mac")]
#[test]
fn mac_regional_variants() {
    use crate::extra_encodings::*;
    use crate::tables::extra as t;
    for (encoding, table) in [
        (X_MAC_ARABIC, &t::X_MAC_ARABIC_DECODE),
        (X_MAC_CELTIC, &t::X_MAC_CELTIC_DECODE),
        (X_MAC_CENTRALEURROMAN, &t::X_MAC_CENTRALEURROMAN_DECODE),
        (X_MAC_CROATIAN, &t::X_MAC_CROATIAN_DECODE),
        (X_MAC_FARSI, &t::X_MAC_FARSI_DECODE),
        (X_MAC_GAELIC, &t::X_MAC_GAELIC_DECODE),
        (X_MAC_GREEK, &t::X_MAC_GREEK_DECODE),
        (X_MAC_ICELANDIC, &t::X_MAC_ICELANDIC_DECODE),
        (X_MAC_ROMANIAN, &t::X_MAC_ROMANIAN_DECODE),
        (X_MAC_TURKISH, &t::X_MAC_TURKISH_DECODE),
    ] {
        check_half(encoding, table);
    }
    // Mac OS Roman keeps the Apple logo at 0xF0; the regional variants reuse
    // that byte, which is exactly the kind of thing this crate has to get right.
    #[cfg(feature = "single-byte")]
    assert_eq!(
        decode(crate::MACINTOSH, b"\xF0").as_deref(),
        Some("\u{F8FF}")
    );
    assert_eq!(decode(X_MAC_CELTIC, b"\xF0").as_deref(), Some("\u{2663}"));
    assert_eq!(decode(X_MAC_GREEK, b"\xF0").as_deref(), Some("\u{03C0}"));
}

#[cfg(feature = "ebcdic")]
#[test]
fn ebcdic_code_pages() {
    use crate::extra_encodings::*;
    use crate::tables::extra as t;
    for (encoding, table) in [
        (IBM037, &t::IBM037_DECODE),
        (IBM424, &t::IBM424_DECODE),
        (IBM500, &t::IBM500_DECODE),
        (IBM875, &t::IBM875_DECODE),
        (IBM1026, &t::IBM1026_DECODE),
    ] {
        check_full(encoding, table);
        // EBCDIC is the one family where a byte below 0x80 is not its own
        // character, which the metadata has to admit.
        assert!(!encoding.is_ascii_compatible(), "{}", encoding.name());
        assert!(encoding.is_single_byte(), "{}", encoding.name());
    }
    // The classic giveaway: 'A' is 0xC1, and 0x40 is the space.
    assert_eq!(decode(IBM037, b"\xC1").as_deref(), Some("A"));
    assert_eq!(decode(IBM037, b"\x40").as_deref(), Some(" "));
    assert_eq!(decode(IBM037, b"\x00").as_deref(), Some("\u{0}"));
}

#[cfg(feature = "misc")]
#[test]
fn misc_charsets() {
    use crate::extra_encodings::*;
    use crate::tables::extra as t;
    check_half(ATARI_ST, &t::ATARI_ST_DECODE);
    check_half(KZ_1048, &t::KZ_1048_DECODE);
}

/// Whatever is compiled in has to be reachable, and cannot claim a label the
/// standard already uses.
#[test]
fn extras_are_reachable_and_do_not_collide() {
    use crate::tables::labels::LABELS;
    assert!(
        LABELS.windows(2).all(|w| w[0].text < w[1].text),
        "labels are sorted"
    );
    for entry in LABELS {
        assert_eq!(
            Encoding::for_label(entry.text.as_bytes()),
            Some(entry.encoding),
            "{}",
            entry.text
        );
        // A label from outside the standard must never reach one of its
        // encodings through the standard's own lookup.  It may still name
        // something there — `iso-8859-1` is ours and the standard's, resolving
        // differently in each — but never to the same place.
        #[cfg(feature = "whatwg-aliases")]
        if !entry.whatwg
            && let Some(found) = Encoding::for_whatwg_label(entry.text.as_bytes())
        {
            assert_ne!(found, entry.encoding, "{} resolves alike", entry.text);
            assert!(found.is_whatwg(), "{}", entry.text);
        }
        assert_eq!(entry.encoding.is_whatwg(), entry.whatwg, "{}", entry.text);
    }
    // Every charset outside the standard brings its own labels, so each is
    // reachable by name whatever else is compiled in.
    for &encoding in Encoding::all() {
        if crate::tables::labels::ALL_ENCODINGS.contains(&encoding)
            && encoding.labels().next().is_some()
        {
            assert_eq!(
                Encoding::for_label(encoding.name().as_bytes()),
                Some(encoding),
                "{}",
                encoding.name()
            );
        }
    }
}

#[cfg(feature = "unicode-extras")]
#[test]
fn utf_32_both_orders() {
    use crate::{UTF_32BE, UTF_32LE};

    assert_eq!(decode(UTF_32LE, b"A\0\0\0").as_deref(), Some("A"));
    assert_eq!(decode(UTF_32BE, b"\0\0\0A").as_deref(), Some("A"));
    assert_eq!(
        decode(UTF_32LE, b"\x00\xF6\x01\x00").as_deref(),
        Some("\u{1F600}")
    );
    // Out of range, and a lone surrogate: neither is a scalar value.
    assert_eq!(decode(UTF_32LE, b"\x00\x00\x11\x00"), None);
    assert_eq!(decode(UTF_32LE, b"\x00\xD8\x00\x00"), None);
    // A truncated code unit at the end of the stream.
    assert_eq!(decode(UTF_32LE, b"A\0\0"), None);

    let text = "a\u{E9}\u{65E5}\u{1F600}";
    for encoding in [UTF_32LE, UTF_32BE] {
        let mut bytes = Vec::new();
        encoding
            .new_encoder()
            .encode_from_str(text, &mut bytes, true)
            .expect("UTF-32 can encode anything");
        assert_eq!(bytes.len(), text.chars().count() * 4);
        assert_eq!(decode(encoding, &bytes).as_deref(), Some(text));
    }
}

#[cfg(feature = "unicode-extras")]
#[test]
fn utf_7_matches_rfc_2152() {
    use crate::UTF_7;

    // The RFC's own examples.
    assert_eq!(
        decode(UTF_7, b"A+ImIDkQ.").as_deref(),
        Some("A\u{2262}\u{391}.")
    );
    assert_eq!(
        decode(UTF_7, b"Hi Mom -+Jjo--!").as_deref(),
        Some("Hi Mom -\u{263A}-!")
    );
    assert_eq!(
        decode(UTF_7, b"+ZeVnLIqe-").as_deref(),
        Some("\u{65E5}\u{672C}\u{8A9E}")
    );
    assert_eq!(
        decode(UTF_7, b"Item 3 is +AKM-1.").as_deref(),
        Some("Item 3 is \u{A3}1.")
    );
    // `+-` is a literal plus sign.
    assert_eq!(decode(UTF_7, b"1 +- 1 = 2").as_deref(), Some("1 + 1 = 2"));
    // A run may be ended by any byte that is not base64, which is then text.
    assert_eq!(decode(UTF_7, b"+AKM.").as_deref(), Some("\u{A3}."));
    // A supplementary character is a surrogate pair in the base64 run.
    assert_eq!(decode(UTF_7, b"+2D3eAA-").as_deref(), Some("\u{1F600}"));

    // Malformed: a byte above 0x7F, a lone surrogate, non-zero padding bits.
    assert_eq!(decode(UTF_7, b"\xE9"), None);
    assert_eq!(decode(UTF_7, b"+2D0-"), None);
    assert_eq!(decode(UTF_7, b"+AKMB-"), None);

    // The encoder writes Set D and whitespace literally, and encodes the rest.
    let encode = |text: &str| {
        let mut bytes = Vec::new();
        UTF_7
            .new_encoder()
            .encode_from_str(text, &mut bytes, true)
            .expect("UTF-7 can encode anything");
        bytes
    };
    assert_eq!(encode("Hi Mom -\u{263A}-!"), b"Hi Mom -+Jjo--+ACE-");
    assert_eq!(encode("1 + 1 = 2"), b"1 +- 1 +AD0- 2");
    assert_eq!(encode("plain"), b"plain");

    // Whatever it writes has to read back as what went in.
    for text in [
        "a\u{E9}\u{65E5}\u{1F600}",
        "\u{263A}\u{263A}\u{263A}",
        "+++",
        "mixed \u{20AC} and \u{1F600} and plain",
        "",
    ] {
        let bytes = encode(text);
        assert_eq!(decode(UTF_7, &bytes).as_deref(), Some(text), "{text:?}");
    }
}

/// The charset the boundary exists for: reachable directly, never through the
/// standard's lookup.
#[cfg(all(feature = "unicode-extras", feature = "whatwg-aliases"))]
#[test]
fn utf_7_is_not_reachable_through_the_standards_lookup() {
    assert_eq!(Encoding::for_label(b"utf-7"), Some(crate::UTF_7));
    assert_eq!(Encoding::for_whatwg_label(b"utf-7"), None);
    assert_eq!(Encoding::for_whatwg_label(b"utf-32"), None);
    assert!(!crate::UTF_7.is_whatwg());
    assert!(!crate::UTF_7.is_ascii_compatible());
}
