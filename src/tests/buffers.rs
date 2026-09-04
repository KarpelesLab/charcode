//! The streaming API driven entirely through fixed-size buffers.
//!
//! Nothing here allocates, so these run in the `no_std`, no-allocator build as
//! well — which is the configuration in which they are the only tests there are.

use crate::encodings::*;
use crate::{CoderResult, DECODER_MIN_BUFFER, DecoderResult, ENCODER_MIN_BUFFER, EncoderResult};

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

#[test]
fn round_trips_through_stack_buffers() {
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

#[test]
fn lookup_and_metadata_need_no_allocator() {
    use crate::Encoding;

    assert_eq!(Encoding::for_label(b"  LATIN1 "), Some(WINDOWS_1252));
    assert_eq!(Encoding::for_label_no_replacement(b"iso-2022-kr"), None);
    assert_eq!(Encoding::for_bom(b"\xFF\xFEa\0"), Some((UTF_16LE, 2)));
    assert_eq!(UTF_16BE.output_encoding(), UTF_8);
    assert_eq!(Encoding::all().len(), 40);
    assert!(IBM866.labels().any(|label| label == "cp866"));
    assert!(WINDOWS_1252.is_single_byte());
    assert!(!ISO_2022_JP.is_ascii_compatible());
}
