//! The legacy single-byte encodings, which differ only in their 128-entry index.

use crate::ascii::ascii_prefix_len_capped;
use crate::result::{DecoderResult, EncoderResult};
use crate::sink::{ByteSink, DECODER_HEADROOM, ENCODER_HEADROOM};

/// The decode half of `index single-byte`: entry `n` is the code point for byte
/// `n + 0x80`, or 0 where the byte is unmapped.
#[derive(Debug, Clone, Copy)]
pub(crate) struct SingleByteDecoder {
    table: &'static [u16; 128],
}

impl SingleByteDecoder {
    pub(crate) fn new(table: &'static [u16; 128]) -> Self {
        SingleByteDecoder { table }
    }

    pub(crate) fn decode(&mut self, src: &[u8], sink: &mut ByteSink) -> (DecoderResult, usize) {
        let mut read = 0usize;
        loop {
            if !sink.has_room(DECODER_HEADROOM) {
                return (DecoderResult::OutputFull, read);
            }
            let rest = &src[read..];
            if rest.is_empty() {
                return (DecoderResult::InputEmpty, read);
            }

            let run = ascii_prefix_len_capped(rest, sink.room());
            if run > 0 {
                sink.write_slice(&rest[..run]);
                read += run;
                continue;
            }

            let byte = rest[0];
            read += 1;
            let code_point = self.table[usize::from(byte) - 0x80];
            if code_point == 0 {
                return (DecoderResult::Malformed(1), read);
            }
            sink.write_code_point(u32::from(code_point));
        }
    }
}

/// The encode half: code points sorted for binary search, and the byte each maps to.
#[derive(Debug, Clone, Copy)]
pub(crate) struct SingleByteEncoder {
    code_points: &'static [u16],
    bytes: &'static [u8],
}

impl SingleByteEncoder {
    pub(crate) fn new(code_points: &'static [u16], bytes: &'static [u8]) -> Self {
        debug_assert_eq!(code_points.len(), bytes.len());
        SingleByteEncoder { code_points, bytes }
    }

    pub(crate) fn encode(&mut self, src: &str, sink: &mut ByteSink) -> (EncoderResult, usize) {
        let mut read = 0usize;
        loop {
            if !sink.has_room(ENCODER_HEADROOM) {
                return (EncoderResult::OutputFull, read);
            }
            let rest = &src[read..];
            let Some(c) = rest.chars().next() else {
                return (EncoderResult::InputEmpty, read);
            };

            let run = ascii_prefix_len_capped(rest.as_bytes(), sink.room());
            if run > 0 {
                sink.write_slice(&rest.as_bytes()[..run]);
                read += run;
                continue;
            }

            read += c.len_utf8();
            let scalar = u32::from(c);
            if scalar > 0xFFFF {
                return (EncoderResult::Unmappable(c), read);
            }
            match self.code_points.binary_search(&(scalar as u16)) {
                Ok(i) => sink.write_byte(self.bytes[i]),
                Err(_) => return (EncoderResult::Unmappable(c), read),
            }
        }
    }
}
