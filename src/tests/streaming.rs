//! The streaming API has to agree with the one-shot API no matter how the input
//! is split or how small the output buffer is.  These tests drive every decoder
//! and encoder over pseudo-random input at every chunk size that matters.

use alloc::string::String;
use alloc::vec::Vec;

use crate::{CoderResult, DECODER_MIN_BUFFER, ENCODER_MIN_BUFFER, Encoding};

/// A xorshift generator, so the corpus is reproducible without a dependency.
struct Rng(u64);

impl Rng {
    fn next_u64(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn byte(&mut self) -> u8 {
        (self.next_u64() >> 32) as u8
    }
}

/// Byte strings that exercise lead bytes, escapes and truncated sequences.
fn byte_corpus() -> Vec<Vec<u8>> {
    let mut corpus: Vec<Vec<u8>> = alloc::vec![
        Vec::new(),
        b"plain ascii".to_vec(),
        b"\x1B$B$O$m$&\x1B(Bmixed".to_vec(),
        b"\x1B".to_vec(),
        b"\x1B$".to_vec(),
        b"\x1B$B".to_vec(),
        b"\x1B(I\x21\x5F\x1B(B".to_vec(),
        b"\x81".to_vec(),
        b"\x81\x30\x81".to_vec(),
        b"\x81\x30\x81\x30".to_vec(),
        b"\x81\x30\x81\x41".to_vec(),
        b"\x8F\xA1\xA1".to_vec(),
        b"\x8E\xA1".to_vec(),
        b"\xEF\xBB\xBFwith bom".to_vec(),
        b"\xFF\xFEa\x00".to_vec(),
        b"\xC3".to_vec(),
        b"\xE2\x82".to_vec(),
        b"\xF0\x9F\x98\x80".to_vec(),
        b"\xED\xA0\x80".to_vec(),
        b"\xA4\x40\xA1\x40".to_vec(),
    ];
    let mut rng = Rng(0x2545_F491_4F6C_DD1D);
    for len in [1usize, 2, 3, 5, 8, 17, 64] {
        for _ in 0..24 {
            corpus.push((0..len).map(|_| rng.byte()).collect());
        }
    }
    corpus
}

fn text_corpus() -> Vec<String> {
    alloc::vec![
        String::new(),
        String::from("plain ascii"),
        String::from("caf\u{E9} na\u{EF}ve"),
        String::from("\u{65E5}\u{672C}\u{8A9E}\u{FF61}\u{FF9F}"),
        String::from("\u{D55C}\u{AD6D}\u{C5B4}"),
        String::from("\u{4E00}\u{2550}\u{5341}"),
        String::from("\u{00A5}\u{203E}\u{2212}"),
        String::from("mixed \u{20AC} and \u{1F600} and \u{E5E5}"),
        String::from("\u{000E}\u{000F}\u{001B}"),
        String::from("a\u{10FFFF}b"),
    ]
}

/// Decodes the whole input in one call, as the reference result.
fn decode_all(encoding: &'static Encoding, bytes: &[u8]) -> (String, bool) {
    let mut text = String::new();
    let errors = encoding
        .new_decoder_without_bom_handling()
        .decode_to_string(bytes, &mut text, true);
    (text, errors)
}

/// Decodes feeding `chunk` bytes at a time into a `dst`-byte output buffer.
fn decode_chunked(
    encoding: &'static Encoding,
    bytes: &[u8],
    chunk: usize,
    dst_len: usize,
) -> (String, bool) {
    let mut decoder = encoding.new_decoder_without_bom_handling();
    let mut buffer = alloc::vec![0u8; dst_len];
    let mut text = String::new();
    let mut had_errors = false;
    let mut offset = 0;
    loop {
        let end = core::cmp::min(offset + chunk, bytes.len());
        let last = end == bytes.len();
        let mut read = 0;
        loop {
            let (result, n, written, errors) = decoder.decode_to_utf8_with_replacement(
                &bytes[offset + read..end],
                &mut buffer,
                last,
            );
            read += n;
            had_errors |= errors;
            text.push_str(core::str::from_utf8(&buffer[..written]).expect("valid UTF-8"));
            if result == CoderResult::InputEmpty {
                break;
            }
        }
        offset = end;
        if last {
            return (text, had_errors);
        }
    }
}

#[test]
fn decoding_is_independent_of_chunking() {
    for &encoding in Encoding::all() {
        for bytes in byte_corpus() {
            let reference = decode_all(encoding, &bytes);
            for chunk in [1usize, 2, 3, 5, 64] {
                for dst_len in [DECODER_MIN_BUFFER, 5, 7, 64] {
                    let got = decode_chunked(encoding, &bytes, chunk, dst_len);
                    assert_eq!(
                        got,
                        reference,
                        "{} chunk={chunk} dst={dst_len} input={bytes:02X?}",
                        encoding.name()
                    );
                }
            }
        }
    }
}

fn encode_all(encoding: &'static Encoding, text: &str) -> (Vec<u8>, bool) {
    let mut bytes = Vec::new();
    let unmappable = encoding
        .new_encoder()
        .encode_from_str(text, &mut bytes, true);
    (bytes, unmappable)
}

fn encode_chunked(
    encoding: &'static Encoding,
    text: &str,
    chars_per_chunk: usize,
    dst_len: usize,
) -> (Vec<u8>, bool) {
    let mut encoder = encoding.new_encoder();
    let mut buffer = alloc::vec![0u8; dst_len];
    let mut out = Vec::new();
    let mut had_unmappable = false;
    let boundaries: Vec<usize> = text
        .char_indices()
        .map(|(i, _)| i)
        .chain(core::iter::once(text.len()))
        .collect();
    let mut index = 0;
    loop {
        let start = boundaries[index];
        index = core::cmp::min(index + chars_per_chunk, boundaries.len() - 1);
        let end = boundaries[index];
        let last = end == text.len();
        let mut read = 0;
        loop {
            let (result, n, written, unmappable) = encoder.encode_from_utf8_with_replacement(
                &text[start + read..end],
                &mut buffer,
                last,
            );
            read += n;
            had_unmappable |= unmappable;
            out.extend_from_slice(&buffer[..written]);
            if result == CoderResult::InputEmpty {
                break;
            }
        }
        if last {
            return (out, had_unmappable);
        }
    }
}

#[test]
fn encoding_is_independent_of_chunking() {
    for &encoding in Encoding::all() {
        for text in text_corpus() {
            let reference = encode_all(encoding, &text);
            for chars_per_chunk in [1usize, 2, 3, 64] {
                for dst_len in [ENCODER_MIN_BUFFER, 5, 11, 64] {
                    let got = encode_chunked(encoding, &text, chars_per_chunk, dst_len);
                    assert_eq!(
                        got,
                        reference,
                        "{} chars={chars_per_chunk} dst={dst_len} input={text:?}",
                        encoding.name()
                    );
                }
            }
        }
    }
}

/// Whatever a decoder produces must be valid UTF-8 and must never panic, for any
/// byte sequence at all.
#[test]
fn decoders_accept_arbitrary_bytes() {
    for &encoding in Encoding::all() {
        for lead in 0..=0xFFu8 {
            let mut bytes = Vec::with_capacity(512);
            for trail in 0..=0xFFu8 {
                bytes.push(lead);
                bytes.push(trail);
            }
            let (text, _) = decode_all(encoding, &bytes);
            assert!(text.is_char_boundary(0));
        }
    }
}
