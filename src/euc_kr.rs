//! EUC-KR, which in practice is the Unified Hangul Code (Windows codepage 949).

use crate::ascii::ascii_prefix_len_capped;
use crate::index;
use crate::result::{DecoderResult, EncoderResult};
use crate::sink::{ByteSink, DECODER_HEADROOM, ENCODER_HEADROOM};
use crate::tables::euc_kr::{
    EUC_KR_DECODE, EUC_KR_ENCODE_BUCKETS, EUC_KR_ENCODE_CODE_POINTS, EUC_KR_ENCODE_POINTERS,
};

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct EucKrDecoder {
    leading: u8,
}

impl EucKrDecoder {
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
                let code_point = if (0x41..=0xFE).contains(&byte) {
                    let pointer = (usize::from(leading) - 0x81) * 190 + (usize::from(byte) - 0x41);
                    index::code_point(&EUC_KR_DECODE, pointer)
                } else {
                    None
                };
                if let Some(code_point) = code_point {
                    sink.write_code_point(code_point);
                    continue;
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
pub(crate) struct EucKrEncoder;

impl EucKrEncoder {
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

            let Some(pointer) = index::pointer(
                &EUC_KR_ENCODE_CODE_POINTS,
                &EUC_KR_ENCODE_POINTERS,
                &EUC_KR_ENCODE_BUCKETS,
                u32::from(c),
            ) else {
                return (EncoderResult::Unmappable(c), read);
            };
            let pointer = u32::from(pointer);
            sink.write_slice(&[(pointer / 190 + 0x81) as u8, (pointer % 190 + 0x41) as u8]);
        }
    }
}
