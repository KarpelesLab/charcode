//! Shift_JIS, including the Windows end-user defined character range.

use crate::ascii::{ascii_prefix_len, ascii_prefix_len_str};
use crate::index;
use crate::result::{DecoderResult, EncoderResult};
use crate::sink::{ByteSink, DECODER_HEADROOM, ENCODER_HEADROOM};
use crate::tables::jis::{JIS0208_DECODE, SHIFT_JIS_ENCODE_CODE_POINTS, SHIFT_JIS_ENCODE_POINTERS};

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct ShiftJisDecoder {
    leading: u8,
}

impl ShiftJisDecoder {
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

            let Some(byte) = src.get(read).copied() else {
                if !last {
                    return (DecoderResult::InputEmpty, read);
                }
                if self.leading == 0 {
                    return (DecoderResult::InputEmpty, read);
                }
                self.leading = 0;
                return (DecoderResult::Malformed(1), read);
            };
            read += 1;

            if self.leading != 0 {
                let leading = self.leading;
                self.leading = 0;
                let offset = if byte < 0x7F { 0x40 } else { 0x41 };
                let leading_offset = if leading < 0xA0 { 0x81 } else { 0xC1 };
                let pointer = if (0x40..=0x7E).contains(&byte) || (0x80..=0xFC).contains(&byte) {
                    Some(
                        (usize::from(leading) - leading_offset) * 188
                            + (usize::from(byte) - offset),
                    )
                } else {
                    None
                };
                if let Some(pointer) = pointer {
                    // The Windows EUDC block, which no index covers.
                    if (8836..=10715).contains(&pointer) {
                        sink.write_code_point(0xE000 - 8836 + pointer as u32);
                        continue;
                    }
                    if let Some(code_point) = index::code_point(&JIS0208_DECODE, pointer) {
                        sink.write_code_point(code_point);
                        continue;
                    }
                }
                if byte.is_ascii() {
                    read -= 1;
                    return (DecoderResult::Malformed(1), read);
                }
                return (DecoderResult::Malformed(2), read);
            }

            if byte.is_ascii() {
                sink.write_byte(byte);
            } else if byte == 0x80 {
                // U+0080, which is two bytes in UTF-8 rather than a copy of 0x80.
                sink.write_code_point(0x80);
            } else if (0xA1..=0xDF).contains(&byte) {
                sink.write_code_point(0xFF61 - 0xA1 + u32::from(byte));
            } else if (0x81..=0x9F).contains(&byte) || (0xE0..=0xFC).contains(&byte) {
                self.leading = byte;
            } else {
                return (DecoderResult::Malformed(1), read);
            }
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct ShiftJisEncoder;

impl ShiftJisEncoder {
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
                0x0080 => {
                    sink.write_byte(0x80);
                    continue;
                }
                0x00A5 => {
                    sink.write_byte(0x5C);
                    continue;
                }
                0x203E => {
                    sink.write_byte(0x7E);
                    continue;
                }
                halfwidth @ 0xFF61..=0xFF9F => {
                    sink.write_byte((halfwidth - 0xFF61 + 0xA1) as u8);
                    continue;
                }
                0x2212 => 0xFF0D,
                other => other,
            };

            let Some(pointer) = index::pointer(
                &SHIFT_JIS_ENCODE_CODE_POINTS,
                &SHIFT_JIS_ENCODE_POINTERS,
                scalar,
            ) else {
                return (EncoderResult::Unmappable(c), read);
            };
            let pointer = u32::from(pointer);
            let leading = pointer / 188;
            let leading_offset = if leading < 0x1F { 0x81 } else { 0xC1 };
            let trailing = pointer % 188;
            let offset = if trailing < 0x3F { 0x40 } else { 0x41 };
            sink.write_slice(&[(leading + leading_offset) as u8, (trailing + offset) as u8]);
        }
    }
}
