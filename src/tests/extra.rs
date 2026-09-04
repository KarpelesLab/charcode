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

#[cfg(feature = "iso-2022-kr")]
#[test]
fn iso_2022_kr_matches_rfc_1557() {
    use crate::ISO_2022_KR;

    let encode = |text: &str| {
        let mut out = Vec::new();
        ISO_2022_KR
            .new_encoder()
            .encode_from_str(text, &mut out, true)
            .unwrap_or_else(|e| panic!("ISO-2022-KR cannot encode {e}"));
        out
    };

    // Vectors both glibc and Python produce.
    assert_eq!(
        decode(ISO_2022_KR, b"\x1B$)C\x0E0!\x0F").as_deref(),
        Some("\u{AC00}")
    );
    assert_eq!(
        decode(ISO_2022_KR, b"\x1B$)C\x0Elm\\be^\x0Fzz\x0E,(\x0F").as_deref(),
        Some("\u{65E5}\u{672C}\u{8A9E}zz\u{416}")
    );
    assert_eq!(encode("\u{AC00}"), b"\x1B$)C\x0E0!\x0F");
    // The designator is written once, lazily, and the stream ends in ASCII.
    assert_eq!(encode("abc\u{AC00}def"), b"abc\x1B$)C\x0E0!\x0Fdef");
    assert_eq!(encode("plain"), b"plain");

    // The designator is a no-op, wherever it appears and however often.
    assert_eq!(
        decode(ISO_2022_KR, b"\x1B$)C\x1B$)Cab").as_deref(),
        Some("ab")
    );
    assert_eq!(decode(ISO_2022_KR, b"ab\x1B$)C").as_deref(), Some("ab"));

    // A line ends in ASCII, so a run left open does not swallow the next line.
    assert_eq!(
        decode(ISO_2022_KR, b"\x1B$)C\x0E0!\n0!").as_deref(),
        Some("\u{AC00}\n0!")
    );

    // Errors: a high byte, an unassigned cell, a truncated pair, and an escape
    // that is not the designator — which is refused rather than passed through,
    // since passing it through is what lets markup hide.
    assert_eq!(decode(ISO_2022_KR, b"\x80"), None);
    assert_eq!(decode(ISO_2022_KR, b"\x1B$)C\x0E-!\x0F"), None);
    assert_eq!(decode(ISO_2022_KR, b"\x1B$)C\x0E0"), None);
    assert_eq!(decode(ISO_2022_KR, b"a\x1B$)Ab"), None);
    // ...and the bytes after a bad escape are still read, not dropped.
    let (text, _, _) = ISO_2022_KR.decode_with(
        b"a\x1B$)Ab",
        crate::DecodeOptions::new().bom(crate::Bom::Ignore),
    );
    assert_eq!(text, "a\u{FFFD}$)Ab");

    // The encoder refuses to write the codes its own structure is made of.
    let mut out = Vec::new();
    assert!(
        ISO_2022_KR
            .new_encoder()
            .encode_from_str("a\u{0E}b", &mut out, true)
            .is_err()
    );
}

/// The standard refuses this label; compiling the encoding in must not change
/// that, or a build that wants it for a mail archive would widen what a label
/// off the network can select.
#[cfg(all(feature = "iso-2022-kr", feature = "whatwg-aliases"))]
#[test]
fn iso_2022_kr_is_not_reachable_through_the_standards_lookup() {
    assert_eq!(
        Encoding::for_label(b"iso-2022-kr"),
        Some(crate::ISO_2022_KR)
    );
    assert_eq!(
        Encoding::for_whatwg_label(b"iso-2022-kr"),
        Some(crate::REPLACEMENT)
    );
    assert_eq!(
        Encoding::for_whatwg_label(b"csiso2022kr"),
        Some(crate::REPLACEMENT)
    );
    assert!(!crate::ISO_2022_KR.is_whatwg());
}

/// Big5 reconstructed from the delta, checked against the decoder over the
/// whole two-byte space, and round-tripped.
#[cfg(feature = "big5")]
#[test]
fn big5_is_index_big5_narrowed_and_corrected() {
    use crate::tables::big5::BIG5_DECODE;
    use crate::tables::big5_1984::{BIG5_1984_DECODE_DELTA, BIG5_1984_ENCODE_DELTA};

    let mut mapped = 0usize;
    for lead in 0x81..=0xFEu8 {
        for trail in (0x40..=0x7Eu8).chain(0xA1..=0xFEu8) {
            let offset = if trail < 0x7F { 0x40 } else { 0x62 };
            let pointer = (usize::from(lead) - 0x81) * 157 + (usize::from(trail) - offset);
            // What Big5 itself says, working only from the tables.
            let expected = if !(0xA1..=0xF9).contains(&lead) {
                None
            } else {
                match BIG5_1984_DECODE_DELTA.binary_search_by_key(&(pointer as u16), |&(p, _)| p) {
                    Ok(i) => match BIG5_1984_DECODE_DELTA[i].1 {
                        0xFFFF => None,
                        code_point => Some(code_point),
                    },
                    Err(_) => BIG5_DECODE.get(pointer).copied().filter(|&c| c != 0),
                }
            };
            let bytes = [lead, trail];
            match expected {
                Some(code_point) => {
                    mapped += 1;
                    let expected = char::from_u32(code_point).unwrap();
                    assert_eq!(
                        decode(crate::BIG5, &bytes).as_deref(),
                        Some(&*String::from(expected)),
                        "{bytes:02X?} should be U+{code_point:04X}"
                    );
                }
                None => assert_eq!(decode(crate::BIG5, &bytes), None, "pointer {pointer}"),
            }
        }
    }
    // Every cell `BIG5.TXT` maps inside leads 0xA1 to 0xF9.  Seven more it
    // lists as U+FFFD, for cells whose Unicode mapping was never settled.
    assert_eq!(mapped, 13_703);

    // Every code point the encoder can reach comes back as itself.
    for &(code_point, _) in BIG5_1984_ENCODE_DELTA.iter() {
        let c = char::from_u32(u32::from(code_point)).unwrap();
        let mut out = Vec::new();
        crate::BIG5
            .new_encoder()
            .encode_from_str(&String::from(c), &mut out, true)
            .unwrap();
        assert_eq!(
            decode(crate::BIG5, &out).as_deref(),
            Some(&*String::from(c))
        );
    }
}

/// The two Big5s are different charsets, and the honest lookup must give the
/// one the label actually names.
#[cfg(all(feature = "big5", feature = "whatwg-aliases"))]
#[test]
fn big5_labels_do_not_resolve_to_the_hong_kong_superset() {
    for label in [b"big5".as_slice(), b"csbig5", b"cn-big5", b"x-x-big5"] {
        assert_eq!(Encoding::for_label(label), Some(crate::BIG5));
        assert_eq!(Encoding::for_whatwg_label(label), Some(crate::BIG5_HKSCS));
    }
    // The superset keeps the label that names it, in both lookups.
    assert_eq!(Encoding::for_label(b"big5-hkscs"), Some(crate::BIG5_HKSCS));
    assert_eq!(
        Encoding::for_whatwg_label(b"big5-hkscs"),
        Some(crate::BIG5_HKSCS)
    );
    assert!(!crate::BIG5.is_whatwg());
    assert!(crate::BIG5_HKSCS.is_whatwg());

    // 0xA1E3 is the cell the two disagree on most visibly, and 0x8862 is
    // HKSCS' own, below Big5's lead range entirely.
    assert_eq!(
        decode(crate::BIG5, b"\xA1\xE3").as_deref(),
        Some("\u{223C}")
    );
    assert_eq!(
        decode(crate::BIG5_HKSCS, b"\xA1\xE3").as_deref(),
        Some("\u{FF5E}")
    );
    assert_eq!(decode(crate::BIG5, b"\x88\x62"), None);
}

/// Shift_JIS reconstructed from the delta, checked against the decoder over the
/// whole space it admits, and round-tripped.
#[cfg(feature = "shift-jis")]
#[test]
fn shift_jis_is_jis_x_0208_and_jis_x_0201() {
    use crate::tables::jis::JIS0208_DECODE;
    use crate::tables::jis0208_1997::{JIS0208_1997_DECODE_DELTA, JIS0208_1997_ENCODE_CODE_POINTS};

    // JIS X 0201's Roman set: ASCII but for the yen sign and the overline.
    for byte in 0..=0x7Fu8 {
        let expected = match byte {
            0x5C => 0x00A5,
            0x7E => 0x203E,
            _ => u32::from(byte),
        };
        let c = char::from_u32(expected).unwrap();
        assert_eq!(
            decode(crate::SHIFT_JIS, &[byte]).as_deref(),
            Some(&*String::from(c)),
            "byte {byte:02X}"
        );
    }
    // The half-width katakana, and nothing else in the single-byte range.
    for byte in 0x80..=0xFFu8 {
        let expected = match byte {
            0xA1..=0xDF => Some(0xFF61 - 0xA1 + u32::from(byte)),
            _ => None,
        };
        let decoded = decode(crate::SHIFT_JIS, &[byte]);
        match expected {
            Some(code_point) => {
                let c = char::from_u32(code_point).unwrap();
                assert_eq!(decoded.as_deref(), Some(&*String::from(c)), "{byte:02X}");
            }
            None => assert_eq!(decoded, None, "byte {byte:02X}"),
        }
    }

    let mut mapped = 0usize;
    for lead in (0x81..=0x9Fu8).chain(0xE0..=0xEF) {
        for trail in 0x40..=0xFCu8 {
            // What JIS X 0208 says, working only from the tables.
            let expected = if trail == 0x7F {
                None
            } else {
                let offset = if trail < 0x7F { 0x40 } else { 0x41 };
                let leading_offset = if lead < 0xA0 { 0x81 } else { 0xC1 };
                let pointer =
                    (usize::from(lead) - leading_offset) * 188 + (usize::from(trail) - offset);
                match JIS0208_1997_DECODE_DELTA.binary_search_by_key(&(pointer as u16), |&(p, _)| p)
                {
                    Ok(i) => match JIS0208_1997_DECODE_DELTA[i].1 {
                        0xFFFF => None,
                        code_point => Some(code_point),
                    },
                    Err(_) => JIS0208_DECODE
                        .get(pointer)
                        .copied()
                        .filter(|&c| c != 0)
                        .map(u32::from),
                }
            };
            let bytes = [lead, trail];
            match expected {
                Some(code_point) => {
                    mapped += 1;
                    let c = char::from_u32(code_point).unwrap();
                    assert_eq!(
                        decode(crate::SHIFT_JIS, &bytes).as_deref(),
                        Some(&*String::from(c)),
                        "{bytes:02X?} should be U+{code_point:04X}"
                    );
                }
                None => assert_eq!(decode(crate::SHIFT_JIS, &bytes), None, "{bytes:02X?}"),
            }
        }
    }
    // The 6879 code points JIS X 0208 defines, at one pointer each.
    assert_eq!(mapped, 6879);

    // Everything the encoder can reach comes back as itself, including the two
    // single-byte cells JIS X 0201 does not share with ASCII.
    for &code_point in JIS0208_1997_ENCODE_CODE_POINTS.iter() {
        let c = char::from_u32(u32::from(code_point)).unwrap();
        let mut out = Vec::new();
        crate::SHIFT_JIS
            .new_encoder()
            .encode_from_str(&String::from(c), &mut out, true)
            .unwrap();
        assert_eq!(
            decode(crate::SHIFT_JIS, &out).as_deref(),
            Some(&*String::from(c))
        );
    }
    for (c, byte) in [('\u{A5}', 0x5Cu8), ('\u{203E}', 0x7E)] {
        let mut out = Vec::new();
        crate::SHIFT_JIS
            .new_encoder()
            .encode_from_str(&String::from(c), &mut out, true)
            .unwrap();
        assert_eq!(out, [byte]);
    }
    // The backslash and the tilde are not in the charset at all.
    for c in ['\\', '~'] {
        let mut out = Vec::new();
        assert!(
            crate::SHIFT_JIS
                .new_encoder()
                .encode_from_str(&String::from(c), &mut out, true)
                .is_err()
        );
    }
}

/// The two Shift_JISes are different charsets, and the honest lookup must give
/// the one the label actually names.
#[cfg(all(feature = "shift-jis", feature = "whatwg-aliases"))]
#[test]
fn shift_jis_labels_do_not_resolve_to_codepage_932() {
    for label in [
        b"shift_jis".as_slice(),
        b"shift-jis",
        b"sjis",
        b"csshiftjis",
    ] {
        assert_eq!(Encoding::for_label(label), Some(crate::SHIFT_JIS));
        assert_eq!(Encoding::for_whatwg_label(label), Some(crate::WINDOWS_31J));
    }
    // The labels that name the codepage keep it, in both lookups.
    for label in [b"windows-31j".as_slice(), b"ms932"] {
        assert_eq!(Encoding::for_label(label), Some(crate::WINDOWS_31J));
        assert_eq!(Encoding::for_whatwg_label(label), Some(crate::WINDOWS_31J));
    }
    assert!(!crate::SHIFT_JIS.is_whatwg());
    assert!(crate::WINDOWS_31J.is_whatwg());

    // 0x5C, and one of the six pointers the standard's index remaps.
    assert_eq!(decode(crate::SHIFT_JIS, b"\x5C").as_deref(), Some("\u{A5}"));
    assert_eq!(decode(crate::WINDOWS_31J, b"\x5C").as_deref(), Some("\\"));
    assert_eq!(
        decode(crate::SHIFT_JIS, b"\x81\x60").as_deref(),
        Some("\u{301C}")
    );
    assert_eq!(
        decode(crate::WINDOWS_31J, b"\x81\x60").as_deref(),
        Some("\u{FF5E}")
    );
    // The NEC row and the end-user defined area are the codepage's alone.
    assert_eq!(decode(crate::SHIFT_JIS, b"\x87\x40"), None);
    assert_eq!(decode(crate::SHIFT_JIS, b"\xF0\x40"), None);
}

/// EUC-JP reconstructed from the tables, checked against the decoder over every
/// sequence it admits, and round-tripped.
#[cfg(feature = "euc-jp")]
#[test]
fn euc_jp_is_jis_x_0208_and_jis_x_0212() {
    use crate::tables::jis::{JIS0212_DECODE, JIS0212_ENCODE_CODE_POINTS};
    use crate::tables::jis0208_1997::JIS0208_1997_ENCODE_CODE_POINTS;

    // GL is ASCII; CR is the C1 controls, less the two single shifts.
    for byte in 0..=0xFFu8 {
        let expected = match byte {
            0x00..=0x7F => Some(u32::from(byte)),
            0x80..=0x8D | 0x90..=0x9F => Some(u32::from(byte)),
            _ => None,
        };
        let decoded = decode(crate::EUC_JP, &[byte]);
        match expected {
            Some(code_point) => {
                let c = char::from_u32(code_point).unwrap();
                assert_eq!(
                    decoded.as_deref(),
                    Some(&*String::from(c)),
                    "byte {byte:02X}"
                );
            }
            None => assert_eq!(decoded, None, "byte {byte:02X}"),
        }
    }
    // SS2 selects JIS X 0201's katakana.
    for byte in 0xA1..=0xDFu8 {
        let c = char::from_u32(0xFF61 - 0xA1 + u32::from(byte)).unwrap();
        assert_eq!(
            decode(crate::EUC_JP, &[0x8E, byte]).as_deref(),
            Some(&*String::from(c))
        );
    }

    let (mut plane0208, mut plane0212) = (0usize, 0usize);
    for lead in 0xA1..=0xFEu8 {
        for trail in 0xA1..=0xFEu8 {
            let pointer = (usize::from(lead) - 0xA1) * 94 + (usize::from(trail) - 0xA1);
            // JIS X 0208, in GR.
            match crate::jis0208_1997::code_point(pointer) {
                Some(code_point) => {
                    plane0208 += 1;
                    let c = char::from_u32(code_point).unwrap();
                    assert_eq!(
                        decode(crate::EUC_JP, &[lead, trail]).as_deref(),
                        Some(&*String::from(c))
                    );
                }
                None => assert_eq!(decode(crate::EUC_JP, &[lead, trail]), None, "{pointer}"),
            }
            // JIS X 0212, behind SS3.
            match JIS0212_DECODE.get(pointer).copied().filter(|&c| c != 0) {
                Some(code_point) => {
                    plane0212 += 1;
                    let c = char::from_u32(u32::from(code_point)).unwrap();
                    assert_eq!(
                        decode(crate::EUC_JP, &[0x8F, lead, trail]).as_deref(),
                        Some(&*String::from(c))
                    );
                }
                None => assert_eq!(
                    decode(crate::EUC_JP, &[0x8F, lead, trail]),
                    None,
                    "{pointer}"
                ),
            }
        }
    }
    assert_eq!(plane0208, 6879);
    assert_eq!(plane0212, 6067);

    // Both planes round-trip, the supplementary one behind the single shift.
    for table in [
        &JIS0208_1997_ENCODE_CODE_POINTS[..],
        &JIS0212_ENCODE_CODE_POINTS[..],
    ] {
        for &code_point in table {
            let c = char::from_u32(u32::from(code_point)).unwrap();
            let mut out = Vec::new();
            crate::EUC_JP
                .new_encoder()
                .encode_from_str(&String::from(c), &mut out, true)
                .unwrap();
            assert_eq!(
                decode(crate::EUC_JP, &out).as_deref(),
                Some(&*String::from(c))
            );
        }
    }
}

/// ISO-2022-JP has only the three sets RFC 1468 gives it.
#[cfg(feature = "iso-2022-jp")]
#[test]
fn iso_2022_jp_has_no_katakana_mode() {
    // `ESC ( I` is the standard's addition, and not an escape RFC 1468 knows.
    assert_eq!(decode(crate::ISO_2022_JP, b"\x1B(I1\x1B(B"), None);
    assert_eq!(
        decode(crate::X_WHATWG_ISO_2022_JP, b"\x1B(I1\x1B(B").as_deref(),
        Some("\u{FF71}")
    );
    // Both JIS X 0208 designators work, and give JIS X 0208.
    for escape in [b"\x1B$@".as_slice(), b"\x1B$B"] {
        let mut bytes = escape.to_vec();
        bytes.extend_from_slice(b"\x21\x41\x1B(B");
        assert_eq!(
            decode(crate::ISO_2022_JP, &bytes).as_deref(),
            Some("\u{301C}"),
            "{escape:02X?}"
        );
    }
    // JIS X 0201's Roman set, and the NEC row the standard's index folds in.
    assert_eq!(
        decode(crate::ISO_2022_JP, b"\x1B(J\x5C\x7E\x1B(B").as_deref(),
        Some("\u{A5}\u{203E}")
    );
    assert_eq!(decode(crate::ISO_2022_JP, b"\x1B$B\x2D\x21\x1B(B"), None);

    // The encoder refuses the half-width katakana rather than reaching for the
    // fullwidth forms, and writes U+2212 where JIS X 0208 puts it.
    let mut out = Vec::new();
    assert!(
        crate::ISO_2022_JP
            .new_encoder()
            .encode_from_str("\u{FF71}", &mut out, true)
            .is_err()
    );
    out.clear();
    crate::ISO_2022_JP
        .new_encoder()
        .encode_from_str("\u{2212}", &mut out, true)
        .unwrap();
    assert_eq!(out, b"\x1B$B\x21\x5D\x1B(B");
}

/// Neither Japanese label may resolve to the standard's altered encoding.
#[cfg(all(
    feature = "euc-jp",
    feature = "iso-2022-jp",
    feature = "whatwg-aliases"
))]
#[test]
fn the_japanese_labels_name_the_charsets_they_say() {
    for (label, ours, theirs) in [
        (b"euc-jp".as_slice(), crate::EUC_JP, crate::X_WHATWG_EUC_JP),
        (b"x-euc-jp", crate::EUC_JP, crate::X_WHATWG_EUC_JP),
        (
            b"iso-2022-jp",
            crate::ISO_2022_JP,
            crate::X_WHATWG_ISO_2022_JP,
        ),
        (
            b"csiso2022jp",
            crate::ISO_2022_JP,
            crate::X_WHATWG_ISO_2022_JP,
        ),
    ] {
        assert_eq!(Encoding::for_label(label), Some(ours), "{label:?}");
        assert_eq!(Encoding::for_whatwg_label(label), Some(theirs), "{label:?}");
        assert!(!ours.is_whatwg());
        assert!(theirs.is_whatwg());
    }
    // The coined names reach the standard's own, in both lookups.
    for (label, expected) in [
        (b"x-whatwg-euc-jp".as_slice(), crate::X_WHATWG_EUC_JP),
        (b"x-whatwg-iso-2022-jp", crate::X_WHATWG_ISO_2022_JP),
    ] {
        assert_eq!(Encoding::for_label(label), Some(expected));
        assert_eq!(Encoding::for_whatwg_label(label), Some(expected));
    }
}

/// ISO-2022-CN's three sets, reconstructed from the tables and checked against
/// the decoder over every sequence they admit.
#[cfg(feature = "iso-2022-cn")]
#[test]
fn iso_2022_cn_designates_gb2312_and_two_cns_planes() {
    use crate::tables::cns11643::{CNS_PLANE1_DECODE, CNS_PLANE2_DECODE};

    let (mut gb, mut cns1, mut cns2) = (0usize, 0usize, 0usize);
    for lead in 0x21..=0x7Eu8 {
        for trail in 0x21..=0x7Eu8 {
            let pointer = (usize::from(lead) - 0x21) * 94 + (usize::from(trail) - 0x21);

            // GB 2312 in G1, whose bytes are EUC-CN's with the high bit off.
            let mut bytes = alloc::vec![0x1B, 0x24, 0x29, 0x41, 0x0E, lead, trail, 0x0F];
            match crate::euc_cn::code_point(lead | 0x80, trail | 0x80) {
                Some(code_point) => {
                    gb += 1;
                    let c = char::from_u32(code_point).unwrap();
                    assert_eq!(
                        decode(crate::ISO_2022_CN, &bytes).as_deref(),
                        Some(&*String::from(c))
                    );
                }
                None => assert_eq!(decode(crate::ISO_2022_CN, &bytes), None, "gb {pointer}"),
            }

            // CNS 11643 plane 1, also in G1.
            bytes[3] = 0x47;
            match CNS_PLANE1_DECODE.get(pointer).copied().filter(|&c| c != 0) {
                Some(code_point) => {
                    cns1 += 1;
                    let c = char::from_u32(u32::from(code_point)).unwrap();
                    assert_eq!(
                        decode(crate::ISO_2022_CN, &bytes).as_deref(),
                        Some(&*String::from(c))
                    );
                }
                None => assert_eq!(decode(crate::ISO_2022_CN, &bytes), None, "cns1 {pointer}"),
            }

            // CNS 11643 plane 2, in G2, one character per single shift.
            let bytes = [0x1B, 0x24, 0x2A, 0x48, 0x1B, 0x4E, lead, trail];
            match CNS_PLANE2_DECODE.get(pointer).copied().filter(|&c| c != 0) {
                Some(code_point) => {
                    cns2 += 1;
                    let c = char::from_u32(u32::from(code_point)).unwrap();
                    assert_eq!(
                        decode(crate::ISO_2022_CN, &bytes).as_deref(),
                        Some(&*String::from(c))
                    );
                }
                None => assert_eq!(decode(crate::ISO_2022_CN, &bytes), None, "cns2 {pointer}"),
            }
        }
    }
    assert_eq!((gb, cns1, cns2), (7445, 5867, 7650));
}

/// The structural rules: designations expire with the line, a single shift is
/// single, and nothing may be used before it is designated.
#[cfg(feature = "iso-2022-cn")]
#[test]
fn iso_2022_cn_scopes_its_designations_to_the_line() {
    let cn = crate::ISO_2022_CN;
    // GB 2312 for U+4E2D, once designated.
    assert_eq!(
        decode(cn, b"\x1B$)A\x0EVP\x0F").as_deref(),
        Some("\u{4E2D}")
    );
    // Shifting out with nothing in G1 is an error, and so is the same text on
    // a second line after the first has ended.
    assert_eq!(decode(cn, b"\x0EVP\x0F"), None);
    assert_eq!(decode(cn, b"\x1B$)A\x0EVP\x0F\n\x0EVP\x0F"), None);
    // A single shift covers one character; the pair after it is ASCII again.
    assert_eq!(
        decode(cn, b"\x1B$*H\x1BN\x21\x21\x21\x22").as_deref(),
        Some("\u{4E42}!\"")
    );
    // ...and it needs G2 designated first.
    assert_eq!(decode(cn, b"\x1BN\x21\x21"), None);
    // ISO-2022-CN-EXT's designators are not this encoding's.
    for escape in [b"\x1B$)E".as_slice(), b"\x1B$+I", b"\x1B$+M", b"\x1BO"] {
        assert_eq!(decode(cn, escape), None, "{escape:02X?}");
    }

    // The encoder names its sets again on every line, and returns to ASCII.
    let mut out = Vec::new();
    cn.new_encoder()
        .encode_from_str("\u{4E2D}\n\u{4E2D}", &mut out, true)
        .unwrap();
    assert_eq!(out, b"\x1B$)A\x0EVP\x0F\n\x1B$)A\x0EVP\x0F");
    // Everything it writes reads back as itself.
    assert_eq!(decode(cn, &out).as_deref(), Some("\u{4E2D}\n\u{4E2D}"));

    // It refuses the codes its own structure is made of.
    for c in ['\u{0E}', '\u{0F}', '\u{1B}'] {
        let mut out = Vec::new();
        assert!(
            cn.new_encoder()
                .encode_from_str(&String::from(c), &mut out, true)
                .is_err()
        );
    }
}

/// The standard refuses this label; compiling the encoding in must not change
/// that, or a build that wants it for a news archive would widen what a label
/// off the network can select.
#[cfg(all(feature = "iso-2022-cn", feature = "whatwg-aliases"))]
#[test]
fn iso_2022_cn_is_not_reachable_through_the_standards_lookup() {
    assert_eq!(
        Encoding::for_label(b"iso-2022-cn"),
        Some(crate::ISO_2022_CN)
    );
    for label in [b"iso-2022-cn".as_slice(), b"iso-2022-cn-ext"] {
        assert_eq!(
            Encoding::for_whatwg_label(label),
            Some(crate::REPLACEMENT),
            "{label:?}"
        );
    }
    // ISO-2022-CN-EXT is a different encoding, and this crate does not have it.
    assert_eq!(Encoding::for_label(b"iso-2022-cn-ext"), None);
    assert!(!crate::ISO_2022_CN.is_whatwg());
}

/// ISO-2022-JP-2's six sets, each reconstructed from the tables the other
/// encodings own and checked against the decoder over every cell.
#[cfg(feature = "iso-2022-jp-2")]
#[test]
fn iso_2022_jp_2_designates_six_sets() {
    use crate::tables::jis::JIS0212_DECODE;
    use crate::tables::single_byte::ISO_8859_7_DECODE;

    let jp2 = crate::ISO_2022_JP_2;
    let decode_in = |escape: &[u8], bytes: &[u8]| {
        let mut input = escape.to_vec();
        input.extend_from_slice(bytes);
        input.extend_from_slice(b"\x1B(B");
        decode(jp2, &input)
    };

    // ASCII and JIS X 0201's Roman set, which differ at exactly two bytes.
    for byte in 0..=0x7Fu8 {
        let ascii = decode_in(b"\x1B(B", &[byte]);
        let roman = decode_in(b"\x1B(J", &[byte]);
        // The codes the structure is made of are refused in both.
        if matches!(byte, 0x0E | 0x0F | 0x1B) {
            assert_eq!(ascii, None, "{byte:02X}");
            assert_eq!(roman, None, "{byte:02X}");
            continue;
        }
        assert_eq!(ascii.as_deref(), Some(&*String::from(byte as char)));
        let expected = match byte {
            0x5C => '\u{A5}',
            0x7E => '\u{203E}',
            _ => byte as char,
        };
        assert_eq!(
            roman.as_deref(),
            Some(&*String::from(expected)),
            "{byte:02X}"
        );
    }

    let (mut jis0208, mut jis0212, mut gb, mut ks) = (0usize, 0, 0, 0);
    for lead in 0x21..=0x7Eu8 {
        for trail in 0x21..=0x7Eu8 {
            let pointer = (usize::from(lead) - 0x21) * 94 + (usize::from(trail) - 0x21);
            let pair = [lead, trail];
            let cases: [(&[u8], Option<u32>, &mut usize); 4] = [
                (
                    b"\x1B$B",
                    crate::jis0208_1997::code_point(pointer),
                    &mut jis0208,
                ),
                (
                    b"\x1B$(D",
                    JIS0212_DECODE
                        .get(pointer)
                        .copied()
                        .filter(|&c| c != 0)
                        .map(u32::from),
                    &mut jis0212,
                ),
                (
                    b"\x1B$A",
                    crate::euc_cn::code_point(lead | 0x80, trail | 0x80),
                    &mut gb,
                ),
                (
                    b"\x1B$(C",
                    crate::euc_kr::ksx1001_code_point(lead, trail),
                    &mut ks,
                ),
            ];
            for (escape, expected, count) in cases {
                match expected {
                    Some(code_point) => {
                        *count += 1;
                        let c = char::from_u32(code_point).unwrap();
                        assert_eq!(
                            decode_in(escape, &pair).as_deref(),
                            Some(&*String::from(c)),
                            "{escape:02X?} {pair:02X?}"
                        );
                    }
                    None => assert_eq!(decode_in(escape, &pair), None, "{escape:02X?} {pair:02X?}"),
                }
            }
        }
    }
    // JIS X 0208-1978 designates the same set as JIS X 0208-1983.
    assert_eq!(
        decode_in(b"\x1B$@", b"\x46\x7C").as_deref(),
        decode_in(b"\x1B$B", b"\x46\x7C").as_deref()
    );
    // KS X 1001 fills rows 1 to 12, 16 to 40 and 42 to 93; the standard's
    // index EUC-KR maps 8226 of those cells and remaps none of them.
    assert_eq!((jis0208, jis0212, gb, ks), (6879, 6067, 7445, 8226));

    // The two 96-sets in G2, one character per single shift.
    for byte in 0x20..=0x7Fu8 {
        let latin1 = char::from_u32(u32::from(byte) + 0x80).unwrap();
        assert_eq!(
            decode_in(b"\x1B.A", &[0x1B, 0x4E, byte]).as_deref(),
            Some(&*String::from(latin1)),
            "{byte:02X}"
        );
        let greek = decode_in(b"\x1B.F", &[0x1B, 0x4E, byte]);
        match ISO_8859_7_DECODE[usize::from(byte)] {
            0 => assert_eq!(greek, None, "{byte:02X}"),
            code_point => {
                let c = char::from_u32(u32::from(code_point)).unwrap();
                assert_eq!(greek.as_deref(), Some(&*String::from(c)), "{byte:02X}");
            }
        }
    }
    // A single shift with nothing in G2, and one that covers only its own
    // character: the pair after it is read in G0 again.
    assert_eq!(decode(jp2, b"\x1BN\x21"), None);
    assert_eq!(
        decode(jp2, b"\x1B.A\x1BN\x69\x1B$B\x46\x7C").as_deref(),
        Some("\u{E9}\u{65E5}")
    );

    // RFC 1554 has no half-width katakana mode; that is the standard's.
    assert_eq!(decode(jp2, b"\x1B(I1\x1B(B"), None);
    // Space and delete stay themselves inside a double-byte set.
    assert_eq!(
        decode(jp2, b"\x1B$B\x46\x7C \x7F\x1B(B").as_deref(),
        Some("\u{65E5} \u{7F}")
    );
}

/// One message carrying five scripts, and the choices the encoder makes.
#[cfg(feature = "iso-2022-jp-2")]
#[test]
fn iso_2022_jp_2_carries_five_scripts_in_one_message() {
    let jp2 = crate::ISO_2022_JP_2;
    let text = "a\u{65E5}\u{6C49}\u{D55C}\u{E9}\u{3B1}\u{A5}";
    let mut out = Vec::new();
    jp2.new_encoder()
        .encode_from_str(text, &mut out, true)
        .unwrap();
    assert_eq!(
        out,
        // ASCII, JIS X 0208 for the kanji and the Greek alpha, GB 2312 for the
        // simplified form, KS X 1001 for the hangul, JIS X 0212 for the
        // e-acute, JIS X 0201's Roman set for the yen sign, back to ASCII.
        b"a\x1B$B\x46\x7C\x1B$A\x3A\x3A\x1B$(C\x47\x51\x1B$(D\x2B\x31\
          \x1B$B\x26\x41\x1B(J\x5C\x1B(B"
    );
    assert_eq!(decode(jp2, &out).as_deref(), Some(text));

    // A Latin-1 character no Japanese set has goes to G2, not to the Chinese
    // or Korean set that also carries it.
    let mut out = Vec::new();
    jp2.new_encoder()
        .encode_from_str("\u{B7}", &mut out, true)
        .unwrap();
    assert_eq!(out, b"\x1B.A\x1BN\x37");
    assert_eq!(decode(jp2, &out).as_deref(), Some("\u{B7}"));

    // Every line ends in ASCII, and the encoder refuses the codes its own
    // structure is made of.
    let mut out = Vec::new();
    jp2.new_encoder()
        .encode_from_str("\u{65E5}\n\u{65E5}", &mut out, true)
        .unwrap();
    assert_eq!(out, b"\x1B$B\x46\x7C\x1B(B\n\x1B$B\x46\x7C\x1B(B");
    for c in ['\u{0E}', '\u{0F}', '\u{1B}'] {
        let mut out = Vec::new();
        assert!(
            jp2.new_encoder()
                .encode_from_str(&String::from(c), &mut out, true)
                .is_err()
        );
    }
}

/// It is not a label the standard defines, so neither lookup can be surprised.
#[cfg(all(feature = "iso-2022-jp-2", feature = "whatwg-aliases"))]
#[test]
fn iso_2022_jp_2_is_ours_alone() {
    assert_eq!(
        Encoding::for_label(b"iso-2022-jp-2"),
        Some(crate::ISO_2022_JP_2)
    );
    assert_eq!(Encoding::for_whatwg_label(b"iso-2022-jp-2"), None);
    assert!(!crate::ISO_2022_JP_2.is_whatwg());
    // ...and it does not shadow ISO-2022-JP, which is a different encoding.
    assert_eq!(
        Encoding::for_label(b"iso-2022-jp"),
        Some(crate::ISO_2022_JP)
    );
}

/// The four worked examples in UTS #6 section 9, which are the only byte
/// sequences the specification itself pins down.
#[cfg(feature = "scsu")]
#[test]
fn scsu_decodes_the_specifications_own_examples() {
    let cases: [(&[u8], &str); 4] = [
        // 9.1 German, which the default window 0 covers entirely.
        (
            b"\xD6\x6C\x20\x66\x6C\x69\x65\xDF\x74",
            "\u{00D6}l flie\u{00DF}t",
        ),
        // 9.2 Russian, one locking shift to the default window 2.
        (
            b"\x12\x9C\xBE\xC1\xBA\xB2\xB0",
            "\u{041C}\u{043E}\u{0441}\u{043A}\u{0432}\u{0430}",
        ),
        // 9.3 Japanese, 116 characters in 178 bytes: three quarters of what
        // UTF-16 would take, moving between the kana and CJK windows.
        (
            b"\x08\x00\x1B\x4C\xEA\x16\xCA\xD3\x94\x0F\x53\xEF\x61\x1B\xE5\x84\
         \xC4\x0F\x53\xEF\x61\x1B\xE5\x84\xC4\x16\xCA\xD3\x94\x08\x02\x0F\
         \x53\x4A\x4E\x16\x7D\x00\x30\x82\x52\x4D\x30\x6B\x6D\x41\x88\x4C\
         \xE5\x97\x9F\x08\x0C\x16\xCA\xD3\x94\x15\xAE\x0E\x6B\x4C\x08\x0D\
         \x8C\xB4\xA3\x9F\xCA\x99\xCB\x8B\xC2\x97\xCC\xAA\x84\x08\x02\x0E\
         \x7C\x73\xE2\x16\xA3\xB7\xCB\x93\xD3\xB4\xC5\xDC\x9F\x0E\x79\x3E\
         \x06\xAE\xB1\x9D\x93\xD3\x08\x0C\xBE\xA3\x8F\x08\x88\xBE\xA3\x8D\
         \xD3\xA8\xA3\x97\xC5\x17\x89\x08\x0D\x15\xD2\x08\x01\x93\xC8\xAA\
         \x8F\x0E\x61\x1B\x99\xCB\x0E\x4E\xBA\x9F\xA1\xAE\x93\xA8\xA0\x08\
         \x02\x08\x0C\xE2\x16\xA3\xB7\xCB\x0F\x4F\xE1\x80\x05\xEC\x60\x8D\
         \xEA\x06\xD3\xE6\x0F\x8A\x00\x30\x44\x65\xB9\xE4\xFE\xE7\xC2\x06\
         \xCB\x82",
            "\u{3000}\u{266A}\u{30EA}\u{30F3}\u{30B4}\u{53EF}\u{611B}\u{3044}\u{3084}\u{53EF}\
         \u{611B}\u{3044}\u{3084}\u{30EA}\u{30F3}\u{30B4}\u{3002}\u{534A}\u{4E16}\u{7D00}\
         \u{3082}\u{524D}\u{306B}\u{6D41}\u{884C}\u{3057}\u{305F}\u{300C}\u{30EA}\u{30F3}\
         \u{30B4}\u{306E}\u{6B4C}\u{300D}\u{304C}\u{3074}\u{3063}\u{305F}\u{308A}\u{3059}\
         \u{308B}\u{304B}\u{3082}\u{3057}\u{308C}\u{306A}\u{3044}\u{3002}\u{7C73}\u{30A2}\
         \u{30C3}\u{30D7}\u{30EB}\u{30B3}\u{30F3}\u{30D4}\u{30E5}\u{30FC}\u{30BF}\u{793E}\
         \u{306E}\u{30D1}\u{30BD}\u{30B3}\u{30F3}\u{300C}\u{30DE}\u{30C3}\u{30AF}\u{FF08}\
         \u{30DE}\u{30C3}\u{30AD}\u{30F3}\u{30C8}\u{30C3}\u{30B7}\u{30E5}\u{FF09}\u{300D}\
         \u{3092}\u{3001}\u{3053}\u{3088}\u{306A}\u{304F}\u{611B}\u{3059}\u{308B}\u{4EBA}\
         \u{305F}\u{3061}\u{306E}\u{3053}\u{3068}\u{3060}\u{3002}\u{300C}\u{30A2}\u{30C3}\
         \u{30D7}\u{30EB}\u{4FE1}\u{8005}\u{300D}\u{306A}\u{3093}\u{3066}\u{8A00}\u{3044}\
         \u{65B9}\u{307E}\u{3067}\u{3042}\u{308B}\u{3002}",
        ),
        // 9.4 All features: a representative of every tag, including SDX and
        // a supplementary character reached through an extended window.
        (
            b"\x41\xDF\x12\x81\x03\x5F\x10\xDF\x1B\x03\xDF\x1C\x88\x80\x0B\xBF\
         \xFF\xFF\x0D\x0A\x41\x10\xDF\x12\x81\x03\x5F\x10\xDF\x13\xDF\x14\
         \x80\x15\xFF",
            "\u{0041}\u{00DF}\u{0401}\u{015F}\u{00DF}\u{01DF}\u{F000}\u{10FFFF}\u{000D}\u{000A}\
         \u{0041}\u{00DF}\u{0401}\u{015F}\u{00DF}\u{01DF}\u{F000}\u{10FFFF}",
        ),
    ];
    for (bytes, text) in cases {
        assert_eq!(
            decode(crate::SCSU, bytes).as_deref(),
            Some(text),
            "{bytes:02X?}"
        );
    }

    // The first two are forced: there is one sensible encoding of each, and
    // this writes it.
    for (bytes, text) in &cases[..2] {
        let mut out = Vec::new();
        crate::SCSU
            .new_encoder()
            .encode_from_str(text, &mut out, true)
            .unwrap();
        assert_eq!(&out, bytes, "{text:?}");
    }
}

/// Every scalar value survives a round trip, and the windows earn their keep.
#[cfg(feature = "scsu")]
#[test]
fn scsu_round_trips_all_of_unicode() {
    let mut buffer = String::new();
    for scalar in 0..=0x10FFFFu32 {
        let Some(c) = char::from_u32(scalar) else {
            continue;
        };
        buffer.clear();
        buffer.push(c);
        let mut out = Vec::new();
        crate::SCSU
            .new_encoder()
            .encode_from_str(&buffer, &mut out, true)
            .unwrap_or_else(|e| panic!("SCSU cannot encode U+{scalar:04X}: {e}"));
        assert_eq!(
            decode(crate::SCSU, &out).as_deref(),
            Some(&*buffer),
            "U+{scalar:04X} encoded to {out:02X?}"
        );
    }

    // Runs in one alphabet cost about a byte a character, which is the point.
    for (text, most) in [
        ("Hello, world!", 13),
        // Cyrillic: one locking shift, then a byte each.
        ("\u{41C}\u{43E}\u{441}\u{43A}\u{432}\u{430}", 7),
        // Kana and CJK, which UTF-8 spends three bytes a character on.
        (
            "\u{65E5}\u{672C}\u{8A9E}\u{306E}\u{30C6}\u{30AD}\u{30B9}\u{30C8}",
            14,
        ),
        // Even the supplementary planes, through an extended window.
        ("\u{1F600}\u{1F601}\u{1F602}\u{1F603}", 7),
    ] {
        let mut out = Vec::new();
        crate::SCSU
            .new_encoder()
            .encode_from_str(text, &mut out, true)
            .unwrap();
        assert!(out.len() <= most, "{text:?} took {} bytes", out.len());
        assert_eq!(decode(crate::SCSU, &out).as_deref(), Some(text));
    }
}

/// The byte values the scheme reserves, and the sequences it leaves dangling.
#[cfg(feature = "scsu")]
#[test]
fn scsu_refuses_what_the_scheme_reserves() {
    let scsu = crate::SCSU;
    // 0x0C in single-byte mode and 0xF2 in Unicode mode are reserved.
    assert_eq!(decode(scsu, b"A\x0CB"), None);
    assert_eq!(decode(scsu, b"\x0F\xF2\x00"), None);
    // So are the window offset table's own gaps.
    for index in [0x00u8, 0xA8, 0xF8] {
        assert_eq!(decode(scsu, &[0x18, index]), None, "{index:02X}");
    }
    // A tag with its arguments cut off.
    for truncated in [
        b"\x0E".as_slice(), // SQU, no character
        b"\x0E\x30",        // SQU, half a character
        b"\x18",            // SD0, no index
        b"\x0B\x88",        // SDX, one argument
        b"\x01",            // SQ0, nothing quoted
        b"\x0F\x30",        // Unicode mode, half a code unit
    ] {
        assert_eq!(decode(scsu, truncated), None, "{truncated:02X?}");
    }
    // A surrogate that never finds its partner, either way round.
    assert_eq!(decode(scsu, b"\x0E\xD8\x00"), None);
    assert_eq!(decode(scsu, b"\x0E\xDC\x00"), None);
    assert_eq!(decode(scsu, b"\x0E\xD8\x00\x41"), None);
    // ...and one that does, split across the two mechanisms that can carry it.
    assert_eq!(
        decode(scsu, b"\x0E\xD8\x00\x0F\xDC\x00").as_deref(),
        Some("\u{10000}")
    );

    // The encoder never writes a byte the decoder would read as a tag, so the
    // control characters that collide with one are quoted.
    let mut out = Vec::new();
    scsu.new_encoder()
        .encode_from_str("\u{1}\u{B}\u{1B}", &mut out, true)
        .unwrap();
    assert_eq!(out, b"\x01\x01\x01\x0B\x01\x1B");
    assert_eq!(decode(scsu, &out).as_deref(), Some("\u{1}\u{B}\u{1B}"));

    // It is not ASCII-transparent, since 0x01 to 0x08 are tags.
    assert!(!scsu.is_ascii_compatible());
}

/// Not a label the standard defines, so neither lookup can be surprised.
#[cfg(all(feature = "scsu", feature = "whatwg-aliases"))]
#[test]
fn scsu_is_ours_alone() {
    for label in [b"scsu".as_slice(), b"csscsu"] {
        assert_eq!(Encoding::for_label(label), Some(crate::SCSU), "{label:?}");
        assert_eq!(Encoding::for_whatwg_label(label), None, "{label:?}");
    }
    assert!(!crate::SCSU.is_whatwg());
}
