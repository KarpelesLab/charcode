//! The streaming API driven entirely through fixed-size buffers.
//!
//! Nothing here allocates, so these run in the `no_std`, no-allocator build as
//! well — which is the configuration in which they are the only tests there are.

use crate::CoderResult;
use crate::encodings::*;
#[allow(unused_imports)]
use crate::{DECODER_MIN_BUFFER, DecoderResult, ENCODER_MIN_BUFFER, EncoderResult};

/// Decodes all of `bytes` into `out`, substituting errors, and returns the text
/// and whether anything was substituted.
fn decode<'a>(
    encoding: &'static crate::Encoding,
    bytes: &[u8],
    out: &'a mut [u8],
) -> (&'a str, bool) {
    let mut decoder = encoding.new_decoder_without_bom_handling();
    let (result, read, written, had_errors) =
        decoder.decode_to_utf8_with_replacement(bytes, out, true);
    assert_eq!(result, CoderResult::InputEmpty, "the buffer was big enough");
    assert_eq!(read, bytes.len());
    let text = core::str::from_utf8(&out[..written]).expect("decoders emit valid UTF-8");
    (text, had_errors)
}

fn encode<'a>(
    encoding: &'static crate::Encoding,
    text: &str,
    out: &'a mut [u8],
) -> (&'a [u8], bool) {
    let mut encoder = encoding.new_encoder();
    let (result, read, written, unmappable) =
        encoder.encode_from_utf8_with_replacement(text, out, true);
    assert_eq!(result, CoderResult::InputEmpty, "the buffer was big enough");
    assert_eq!(read, text.len());
    (&out[..written], unmappable)
}

/// The always-present encodings: these need no table group at all.
#[test]
fn round_trips_through_stack_buffers() {
    let mut buffer = [0u8; 64];
    assert_eq!(
        decode(UTF_8, "caf\u{E9}".as_bytes(), &mut buffer).0,
        "caf\u{E9}"
    );
    assert_eq!(decode(UTF_16LE, b"a\x00\xE9\x00", &mut buffer).0, "a\u{E9}");
    assert_eq!(decode(UTF_16BE, b"\x00a\x00\xE9", &mut buffer).0, "a\u{E9}");
    assert_eq!(
        decode(X_USER_DEFINED, b"a\x80\xFF", &mut buffer).0,
        "a\u{F780}\u{F7FF}"
    );
    assert_eq!(decode(REPLACEMENT, b"anything", &mut buffer).0, "\u{FFFD}");
    assert_eq!(
        encode(UTF_8, "caf\u{E9}", &mut buffer).0,
        "caf\u{E9}".as_bytes()
    );
    assert_eq!(encode(X_USER_DEFINED, "a\u{F780}", &mut buffer).0, b"a\x80");
    // UTF-16 has no encoder; the standard says to encode as UTF-8.
    assert_eq!(encode(UTF_16LE, "hi", &mut buffer).0, b"hi");
}

#[cfg(all(
    feature = "shift-jis",
    feature = "iso-2022-jp",
    feature = "single-byte"
))]
#[test]
fn round_trips_through_stack_buffers_with_tables() {
    let mut buffer = [0u8; 64];
    assert_eq!(decode(WINDOWS_1252, b"caf\xE9", &mut buffer).0, "caf\u{E9}");
    assert_eq!(
        decode(SHIFT_JIS, b"\x93\xFA\x96\x7B", &mut buffer).0,
        "\u{65E5}\u{672C}"
    );
    assert_eq!(encode(WINDOWS_1252, "caf\u{E9}", &mut buffer).0, b"caf\xE9");
    assert_eq!(
        encode(ISO_2022_JP, "\u{65E5}", &mut buffer).0,
        b"\x1B$B\x46\x7C\x1B(B"
    );
}

#[cfg(feature = "single-byte")]
#[test]
fn errors_are_substituted_without_allocating() {
    let mut buffer = [0u8; 64];
    let (text, had_errors) = decode(UTF_8, b"a\xFFb", &mut buffer);
    assert_eq!(text, "a\u{FFFD}b");
    assert!(had_errors);

    let (bytes, unmappable) = encode(WINDOWS_1252, "a\u{4E00}b", &mut buffer);
    assert_eq!(bytes, b"a&#19968;b");
    assert!(unmappable);
}

#[cfg(feature = "single-byte")]
#[test]
fn errors_can_be_reported_instead() {
    let mut buffer = [0u8; 64];
    let mut decoder = UTF_8.new_decoder_without_bom_handling();
    let (result, read, written) = decoder.decode_to_utf8(b"ab\xFFc", &mut buffer, true);
    assert_eq!(result, DecoderResult::Malformed(1));
    assert_eq!((read, written), (3, 2));

    let mut encoder = WINDOWS_1252.new_encoder();
    let (result, read, written) = encoder.encode_from_utf8("ab\u{4E00}c", &mut buffer, true);
    assert_eq!(result, EncoderResult::Unmappable('\u{4E00}'));
    assert_eq!((read, written), (5, 2));
}

/// The whole point of the buffer API is that the caller chooses the buffer, so
/// the smallest documented size has to keep making progress.
#[cfg(all(feature = "big5", feature = "gb18030"))]
#[test]
fn the_minimum_buffer_size_makes_progress() {
    let mut decoder = BIG5.new_decoder_without_bom_handling();
    let mut buffer = [0u8; DECODER_MIN_BUFFER];
    let input = b"\xA4\x40\xA4\x40";
    let mut read = 0;
    let mut chars = 0;
    loop {
        let (result, n, written, _) =
            decoder.decode_to_utf8_with_replacement(&input[read..], &mut buffer, true);
        read += n;
        chars += core::str::from_utf8(&buffer[..written])
            .expect("valid UTF-8")
            .chars()
            .count();
        if result == CoderResult::InputEmpty {
            break;
        }
    }
    assert_eq!((read, chars), (4, 2));

    let mut encoder = GB18030.new_encoder();
    let mut buffer = [0u8; ENCODER_MIN_BUFFER];
    let text = "\u{10000}\u{10001}";
    let (mut read, mut written_total) = (0, 0);
    loop {
        let (result, n, written, _) =
            encoder.encode_from_utf8_with_replacement(&text[read..], &mut buffer, true);
        read += n;
        written_total += written;
        if result == CoderResult::InputEmpty {
            break;
        }
    }
    assert_eq!((read, written_total), (text.len(), 8));
}

#[cfg(feature = "single-byte")]
#[test]
fn a_byte_order_mark_still_switches_encoding() {
    let mut buffer = [0u8; 64];
    let mut decoder = WINDOWS_1252.new_decoder();
    let (result, _, written, _) =
        decoder.decode_to_utf8_with_replacement(b"\xEF\xBB\xBFcaf\xC3\xA9", &mut buffer, true);
    assert_eq!(result, CoderResult::InputEmpty);
    assert_eq!(
        core::str::from_utf8(&buffer[..written]).expect("valid UTF-8"),
        "caf\u{E9}"
    );
    assert_eq!(decoder.encoding(), UTF_8);
}

#[cfg(feature = "whatwg")]
#[test]
fn lookup_and_metadata_need_no_allocator() {
    use crate::Encoding;

    assert_eq!(Encoding::for_label(b"  LATIN1 "), Some(WINDOWS_1252));
    assert_eq!(Encoding::for_label_no_replacement(b"iso-2022-kr"), None);
    assert_eq!(Encoding::for_bom(b"\xFF\xFEa\0"), Some((UTF_16LE, 2)));
    assert_eq!(UTF_16BE.output_encoding(), UTF_8);
    // The extras add to this, so the exact count only holds without them.
    #[cfg(not(any(feature = "dos", feature = "ebcdic", feature = "mac", feature = "misc")))]
    assert_eq!(Encoding::all().len(), 40);
    assert!(Encoding::all().len() >= 40);
    assert!(IBM866.labels().any(|label| label == "cp866"));
    assert!(WINDOWS_1252.is_single_byte());
    assert!(!ISO_2022_JP.is_ascii_compatible());
}

#[cfg(feature = "whatwg")]
#[test]
fn code_pages_resolve_and_round_trip() {
    use crate::Encoding;

    // Sorted, so the binary search is correct.
    assert!(
        crate::code_page::CODE_PAGES
            .windows(2)
            .all(|w| w[0].number < w[1].number)
    );

    for (number, expected) in [
        (866u32, IBM866),
        (874, WINDOWS_874),
        (932, SHIFT_JIS),
        (936, GBK),
        (949, EUC_KR),
        (950, BIG5),
        (1200, UTF_16LE),
        (1201, UTF_16BE),
        (1252, WINDOWS_1252),
        (10000, MACINTOSH),
        (10007, X_MAC_CYRILLIC),
        (20866, KOI8_R),
        (21866, KOI8_U),
        (28592, ISO_8859_2),
        (38598, ISO_8859_8_I),
        (50220, ISO_2022_JP),
        (51932, EUC_JP),
        (54936, GB18030),
        (65001, UTF_8),
    ] {
        assert_eq!(
            Encoding::for_windows_code_page(number),
            Some(expected),
            "code page {number}"
        );
    }

    // Every canonical entry is the one the reverse lookup reports.
    for entry in crate::code_page::CODE_PAGES {
        if entry.canonical {
            assert_eq!(
                entry.encoding.windows_code_page(),
                Some(entry.number),
                "{}",
                entry.encoding.name()
            );
        }
        // And every number resolves to an encoding that agrees.
        assert_eq!(
            Encoding::for_windows_code_page(entry.number),
            Some(entry.encoding)
        );
    }
    // Exactly one canonical number per encoding that has any.
    for encoding in Encoding::all() {
        let canonical = crate::code_page::CODE_PAGES
            .iter()
            .filter(|entry| entry.canonical && entry.encoding == *encoding)
            .count();
        assert!(canonical <= 1, "{} has {canonical}", encoding.name());
    }

    // The numbers the standard folds into a superset behave like their labels.
    assert_eq!(Encoding::for_windows_code_page(28591), Some(WINDOWS_1252));
    assert_eq!(Encoding::for_windows_code_page(20127), Some(WINDOWS_1252));
    assert_eq!(Encoding::for_windows_code_page(28599), Some(WINDOWS_1254));
    assert_eq!(Encoding::for_windows_code_page(10017), Some(X_MAC_CYRILLIC));

    // The neutralized ones, and the filtered variant.
    for number in [50225u32, 50227, 50229, 52936] {
        assert_eq!(Encoding::for_windows_code_page(number), Some(REPLACEMENT));
        assert_eq!(
            Encoding::for_windows_code_page_no_replacement(number),
            None,
            "code page {number}"
        );
    }

    // Code pages this crate has no encoding for.
    for number in [0u32, 1361, 65000, 65005, 999_999] {
        assert_eq!(Encoding::for_windows_code_page(number), None, "{number}");
    }
    // These belong to the `dos` group rather than to the standard.
    #[cfg(not(feature = "dos"))]
    for number in [437u32, 850] {
        assert_eq!(Encoding::for_windows_code_page(number), None, "{number}");
    }
    assert_eq!(X_USER_DEFINED.windows_code_page(), None);
    assert_eq!(REPLACEMENT.windows_code_page(), Some(50225));
}
