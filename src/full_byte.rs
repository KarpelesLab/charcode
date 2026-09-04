//! Single-byte encodings whose low half is not plain ASCII.
//!
//! The EBCDIC code pages permute the whole byte range, and a few PC code pages
//! (CP864, say) reassign one ASCII byte.  Neither fits
//! [`SingleByteDecoder`](crate::single_byte::SingleByteDecoder), which stores
//! only the 128 bytes above 0x7F and passes the rest through, so those
//! encodings carry a full 256-entry table instead.
//!
//! `0xFFFF` marks an unmapped byte here, rather than the 0 the WHATWG indexes
//! use: EBCDIC maps a byte to U+0000, so zero is a real code point.

use crate::result::{DecoderResult, EncoderResult};
use crate::sink::{ByteSink, DECODER_HEADROOM, ENCODER_HEADROOM};

/// The value marking a byte with no code point.
pub(crate) const UNMAPPED: u16 = 0xFFFF;

#[derive(Debug, Clone, Copy)]
pub(crate) struct FullByteDecoder {
    table: &'static [u16; 256],
}

impl FullByteDecoder {
    pub(crate) fn new(table: &'static [u16; 256]) -> Self {
        FullByteDecoder { table }
    }

    pub(crate) fn decode(&mut self, src: &[u8], sink: &mut ByteSink) -> (DecoderResult, usize) {
        let mut read = 0usize;
        loop {
            if !sink.has_room(DECODER_HEADROOM) {
                return (DecoderResult::OutputFull, read);
            }
            let Some(byte) = src.get(read).copied() else {
                return (DecoderResult::InputEmpty, read);
            };
            read += 1;
            let code_point = self.table[usize::from(byte)];
            if code_point == UNMAPPED {
                return (DecoderResult::Malformed(1), read);
            }
            sink.write_code_point(u32::from(code_point));
        }
    }
}

/// Code points sorted for binary search, and the byte each maps to.
#[derive(Debug, Clone, Copy)]
pub(crate) struct FullByteEncoder {
    code_points: &'static [u16],
    bytes: &'static [u8],
}

impl FullByteEncoder {
    pub(crate) fn new(code_points: &'static [u16], bytes: &'static [u8]) -> Self {
        debug_assert_eq!(code_points.len(), bytes.len());
        FullByteEncoder { code_points, bytes }
    }

    pub(crate) fn encode(&mut self, src: &str, sink: &mut ByteSink) -> (EncoderResult, usize) {
        let mut read = 0usize;
        loop {
            if !sink.has_room(ENCODER_HEADROOM) {
                return (EncoderResult::OutputFull, read);
            }
            let Some(c) = src[read..].chars().next() else {
                return (EncoderResult::InputEmpty, read);
            };
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
