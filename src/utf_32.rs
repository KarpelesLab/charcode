//! UTF-32BE and UTF-32LE.
//!
//! Not part of the Encoding Standard, which has no use for them on the web.

use crate::result::{DecoderResult, EncoderResult};
use crate::sink::{ByteSink, DECODER_HEADROOM, ENCODER_HEADROOM};

#[derive(Debug, Clone, Copy)]
pub(crate) struct Utf32Decoder {
    big_endian: bool,
    /// Bytes of a code unit that the previous call ended in the middle of.
    buf: [u8; 4],
    len: u8,
}

impl Utf32Decoder {
    pub(crate) fn new(big_endian: bool) -> Self {
        Utf32Decoder {
            big_endian,
            buf: [0; 4],
            len: 0,
        }
    }

    pub(crate) fn decode(
        &mut self,
        src: &[u8],
        sink: &mut ByteSink,
        last: bool,
    ) -> (DecoderResult, usize) {
        let mut read = 0usize;
        loop {
            if !sink.has_room(DECODER_HEADROOM) {
                return (DecoderResult::OutputFull, read);
            }
            let Some(byte) = src.get(read).copied() else {
                if !last || self.len == 0 {
                    return (DecoderResult::InputEmpty, read);
                }
                let bad = self.len;
                self.len = 0;
                return (DecoderResult::Malformed(bad), read);
            };
            read += 1;
            self.buf[usize::from(self.len)] = byte;
            self.len += 1;
            if self.len < 4 {
                continue;
            }
            self.len = 0;
            let scalar = if self.big_endian {
                u32::from_be_bytes(self.buf)
            } else {
                u32::from_le_bytes(self.buf)
            };
            match char::from_u32(scalar) {
                Some(c) => sink.write_char(c),
                // Out of range, or a surrogate, which is not a scalar value.
                None => return (DecoderResult::Malformed(4), read),
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Utf32Encoder {
    big_endian: bool,
}

impl Utf32Encoder {
    pub(crate) fn new(big_endian: bool) -> Self {
        Utf32Encoder { big_endian }
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
            sink.write_slice(&if self.big_endian {
                scalar.to_be_bytes()
            } else {
                scalar.to_le_bytes()
            });
        }
    }
}
