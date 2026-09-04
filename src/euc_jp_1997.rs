//! EUC-JP as standardised: ASCII in GL, the C1 controls in CR, JIS X 0201's
//! katakana behind SS2, JIS X 0208 in GR and JIS X 0212 behind SS3.
//!
//! The WHATWG Encoding Standard's encoding of the same name folds the NEC and
//! IBM extension rows into the JIS X 0208 plane and gives six of that charset's
//! own pointers a different character, so `euc-jp` there does not name EUC-JP.
//! That one is [`X_WHATWG_EUC_JP`]; it also refuses to encode into the
//! JIS X 0212 plane, which this one uses.
//!
//! This is byte for byte what glibc's `iconv -f EUC-JP` does.
//!
//! [`X_WHATWG_EUC_JP`]: crate::X_WHATWG_EUC_JP

use crate::ascii::ascii_prefix_len_capped;
use crate::index;
use crate::jis0208_1997::{code_point, pointer};
use crate::result::{DecoderResult, EncoderResult};
use crate::sink::{ByteSink, DECODER_HEADROOM, ENCODER_HEADROOM};
use crate::tables::jis::{
    JIS0212_DECODE, JIS0212_ENCODE_BUCKETS, JIS0212_ENCODE_CODE_POINTS, JIS0212_ENCODE_POINTERS,
};

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct EucJp1997Decoder {
    jis0212: bool,
    leading: u8,
}

impl EucJp1997Decoder {
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
                let run = ascii_prefix_len_capped(&src[read..], sink.room());
                if run > 0 {
                    sink.write_slice(&src[read..read + run]);
                    read += run;
                    continue;
                }
            }

            // Bytes already folded into the decoder state, which an error discards.
            let consumed = 1 + u8::from(self.jis0212);

            let Some(byte) = src.get(read).copied() else {
                if !last || self.leading == 0 {
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
                let decoded = if (0xA1..=0xFE).contains(&leading) && (0xA1..=0xFE).contains(&byte) {
                    let pointer = (usize::from(leading) - 0xA1) * 94 + (usize::from(byte) - 0xA1);
                    if jis0212 {
                        index::code_point(&JIS0212_DECODE, pointer)
                    } else {
                        code_point(pointer)
                    }
                } else {
                    None
                };
                if let Some(code_point) = decoded {
                    sink.write_code_point(code_point);
                    continue;
                }
                // An ASCII byte is text that follows the broken sequence.
                if byte.is_ascii() {
                    read -= 1;
                    return (DecoderResult::Malformed(consumed), read);
                }
                return (DecoderResult::Malformed(consumed + 1), read);
            }

            match byte {
                _ if byte.is_ascii() => sink.write_byte(byte),
                0x8E | 0x8F => self.leading = byte,
                0xA1..=0xFE => self.leading = byte,
                // EUC's CR region is the C1 controls, less the two single
                // shifts that sit in it.
                0x80..=0x8D | 0x90..=0x9F => sink.write_code_point(u32::from(byte)),
                _ => return (DecoderResult::Malformed(1), read),
            }
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct EucJp1997Encoder;

impl EucJp1997Encoder {
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

            let scalar = match u32::from(c) {
                // G0 is ASCII, so these two have no cell of their own; every
                // implementation writes the byte the JIS X 0201 Roman set gives
                // them rather than refusing.
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
                // The CR region, less the cells the single shifts occupy.
                control @ (0x80..=0x8D | 0x90..=0x9F) => {
                    sink.write_byte(control as u8);
                    continue;
                }
                0x008E | 0x008F => return (EncoderResult::Unmappable(c), read),
                other => other,
            };

            if let Some(pointer) = pointer(scalar) {
                let pointer = u32::from(pointer);
                sink.write_slice(&[(pointer / 94 + 0xA1) as u8, (pointer % 94 + 0xA1) as u8]);
                continue;
            }
            // Then the supplementary plane, behind the single shift.
            let Some(pointer) = index::pointer(
                &JIS0212_ENCODE_CODE_POINTS,
                &JIS0212_ENCODE_POINTERS,
                &JIS0212_ENCODE_BUCKETS,
                scalar,
            ) else {
                return (EncoderResult::Unmappable(c), read);
            };
            let pointer = u32::from(pointer);
            sink.write_slice(&[
                0x8F,
                (pointer / 94 + 0xA1) as u8,
                (pointer % 94 + 0xA1) as u8,
            ]);
        }
    }
}
