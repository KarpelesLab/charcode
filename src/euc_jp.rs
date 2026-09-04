//! EUC-JP.  The decoder additionally accepts JIS X 0212 via the 0x8F prefix; the
//! encoder never produces it.

use crate::ascii::{ascii_prefix_len, ascii_prefix_len_str};
use crate::index;
use crate::result::{DecoderResult, EncoderResult};
use crate::sink::{ByteSink, DECODER_HEADROOM, ENCODER_HEADROOM};
use crate::tables::jis::{
    JIS0208_DECODE, JIS0208_ENCODE_CODE_POINTS, JIS0208_ENCODE_POINTERS, JIS0212_DECODE,
};

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct EucJpDecoder {
    jis0212: bool,
    leading: u8,
}

impl EucJpDecoder {
    pub(crate) fn new() -> Self {
        Self::default()
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

            if self.leading == 0 {
                let run = core::cmp::min(ascii_prefix_len(&src[read..]), sink.room());
                if run > 0 {
                    sink.write_slice(&src[read..read + run]);
                    read += run;
                    continue;
                }
            }

            // Bytes already folded into the decoder state, which an error discards.
            let consumed = 1 + u8::from(self.jis0212);

            let Some(byte) = src.get(read).copied() else {
                if !last {
                    return (DecoderResult::InputEmpty, read);
                }
                if self.leading == 0 {
                    return (DecoderResult::InputEmpty, read);
                }
                self.leading = 0;
                self.jis0212 = false;
                return (DecoderResult::Malformed(consumed), read);
            };
            read += 1;

            if self.leading == 0x8E && (0xA1..=0xDF).contains(&byte) {
                self.leading = 0;
                sink.write_code_point(0xFF61 - 0xA1 + u32::from(byte));
                continue;
            }
            if self.leading == 0x8F && (0xA1..=0xFE).contains(&byte) {
                self.jis0212 = true;
                self.leading = byte;
                continue;
            }

            if self.leading != 0 {
                let leading = self.leading;
                let jis0212 = self.jis0212;
                self.leading = 0;
                self.jis0212 = false;
                let code_point = if (0xA1..=0xFE).contains(&leading)
                    && (0xA1..=0xFE).contains(&byte)
                {
                    let pointer = (usize::from(leading) - 0xA1) * 94 + (usize::from(byte) - 0xA1);
                    if jis0212 {
                        index::code_point(&JIS0212_DECODE, pointer)
                    } else {
                        index::code_point(&JIS0208_DECODE, pointer)
                    }
                } else {
                    None
                };
                if let Some(code_point) = code_point {
                    sink.write_code_point(code_point);
                    continue;
                }
                if byte.is_ascii() {
                    read -= 1;
                    return (DecoderResult::Malformed(consumed), read);
                }
                return (DecoderResult::Malformed(consumed + 1), read);
            }

            if byte.is_ascii() {
                sink.write_byte(byte);
            } else if byte == 0x8E || byte == 0x8F || (0xA1..=0xFE).contains(&byte) {
                self.leading = byte;
            } else {
                return (DecoderResult::Malformed(1), read);
            }
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct EucJpEncoder;

impl EucJpEncoder {
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
            let run = core::cmp::min(ascii_prefix_len_str(rest), sink.room());
            if run > 0 {
                sink.write_slice(&rest.as_bytes()[..run]);
                read += run;
                continue;
            }
            read += c.len_utf8();

            let scalar = match u32::from(c) {
                0x00A5 => {
                    sink.write_byte(0x5C);
                    continue;
                }
                0x203E => {
                    sink.write_byte(0x7E);
                    continue;
                }
                halfwidth @ 0xFF61..=0xFF9F => {
                    sink.write_slice(&[0x8E, (halfwidth - 0xFF61 + 0xA1) as u8]);
                    continue;
                }
                0x2212 => 0xFF0D,
                other => other,
            };

            let Some(pointer) = index::pointer(
                &JIS0208_ENCODE_CODE_POINTS,
                &JIS0208_ENCODE_POINTERS,
                scalar,
            ) else {
                return (EncoderResult::Unmappable(c), read);
            };
            let pointer = u32::from(pointer);
            sink.write_slice(&[(pointer / 94 + 0xA1) as u8, (pointer % 94 + 0xA1) as u8]);
        }
    }
}
