//! Big5, including the Hong Kong Supplementary Character Set extensions.

use crate::ascii::{ascii_prefix_len, ascii_prefix_len_str};
use crate::index;
use crate::result::{DecoderResult, EncoderResult};
use crate::sink::{ByteSink, DECODER_HEADROOM, ENCODER_HEADROOM};
use crate::tables::big5::{BIG5_DECODE, BIG5_ENCODE_CODE_POINTS, BIG5_ENCODE_POINTERS};

/// The four pointers that decode to a base letter plus a combining mark; an index
/// entry can only hold a single code point, so the standard lists them separately.
fn combining_pair(pointer: usize) -> Option<(char, char)> {
    match pointer {
        1133 => Some(('\u{00CA}', '\u{0304}')),
        1135 => Some(('\u{00CA}', '\u{030C}')),
        1164 => Some(('\u{00EA}', '\u{0304}')),
        1166 => Some(('\u{00EA}', '\u{030C}')),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Big5Decoder {
    leading: u8,
}

impl Big5Decoder {
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
                let offset = if byte < 0x7F { 0x40 } else { 0x62 };
                let pointer = if (0x40..=0x7E).contains(&byte) || (0xA1..=0xFE).contains(&byte) {
                    Some((usize::from(leading) - 0x81) * 157 + (usize::from(byte) - offset))
                } else {
                    None
                };
                if let Some(pointer) = pointer {
                    if let Some((first, second)) = combining_pair(pointer) {
                        sink.write_char(first);
                        sink.write_char(second);
                        continue;
                    }
                    if let Some(code_point) = index::code_point_wide(&BIG5_DECODE, pointer) {
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
            } else if (0x81..=0xFE).contains(&byte) {
                self.leading = byte;
            } else {
                return (DecoderResult::Malformed(1), read);
            }
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Big5Encoder;

impl Big5Encoder {
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

            let Some(pointer) = index::pointer_wide(
                &BIG5_ENCODE_CODE_POINTS,
                &BIG5_ENCODE_POINTERS,
                u32::from(c),
            ) else {
                return (EncoderResult::Unmappable(c), read);
            };
            let pointer = u32::from(pointer);
            let leading = pointer / 157 + 0x81;
            let trailing = pointer % 157;
            let offset = if trailing < 0x3F { 0x40 } else { 0x62 };
            sink.write_slice(&[leading as u8, (trailing + offset) as u8]);
        }
    }
}
