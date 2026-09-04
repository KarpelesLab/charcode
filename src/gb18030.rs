//! gb18030, and GBK which shares its decoder and restricts its encoder.

use crate::ascii::ascii_prefix_len_capped;
use crate::index;
use crate::result::{DecoderResult, EncoderResult};
use crate::sink::{ByteSink, DECODER_HEADROOM, ENCODER_HEADROOM, Pushback};
use crate::tables::gb18030::{
    GB18030_DECODE, GB18030_ENCODE_BUCKETS, GB18030_ENCODE_CODE_POINTS, GB18030_ENCODE_POINTERS,
};

/// Code points the encoder maps asymmetrically, to stay compatible with how
/// GB18030-2005 encoded them before the 2022 revision moved them into ranges.
const ENCODER_SIDE_TABLE: [(u32, u8, u8); 18] = [
    (0xE78D, 0xA6, 0xD9),
    (0xE78E, 0xA6, 0xDA),
    (0xE78F, 0xA6, 0xDB),
    (0xE790, 0xA6, 0xDC),
    (0xE791, 0xA6, 0xDD),
    (0xE792, 0xA6, 0xDE),
    (0xE793, 0xA6, 0xDF),
    (0xE794, 0xA6, 0xEC),
    (0xE795, 0xA6, 0xED),
    (0xE796, 0xA6, 0xF3),
    (0xE81E, 0xFE, 0x59),
    (0xE826, 0xFE, 0x61),
    (0xE82B, 0xFE, 0x66),
    (0xE82C, 0xFE, 0x67),
    (0xE832, 0xFE, 0x6D),
    (0xE843, 0xFE, 0x7E),
    (0xE854, 0xFE, 0x90),
    (0xE864, 0xFE, 0xA0),
];

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Gb18030Decoder {
    first: u8,
    second: u8,
    third: u8,
    pending: Pushback,
}

impl Gb18030Decoder {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    fn pending_bytes(&self) -> u8 {
        u8::from(self.first != 0) + u8::from(self.second != 0) + u8::from(self.third != 0)
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

            if self.first == 0 && self.pending.is_empty() {
                let run = ascii_prefix_len_capped(&src[read..], sink.room());
                if run > 0 {
                    sink.write_slice(&src[read..read + run]);
                    read += run;
                    continue;
                }
            }

            let (byte, from_pending) = match self.pending.next() {
                Some(byte) => (byte, true),
                None => match src.get(read).copied() {
                    Some(byte) => {
                        read += 1;
                        (byte, false)
                    }
                    None => {
                        if !last {
                            return (DecoderResult::InputEmpty, read);
                        }
                        let bad = self.pending_bytes();
                        if bad == 0 {
                            return (DecoderResult::InputEmpty, read);
                        }
                        self.first = 0;
                        self.second = 0;
                        self.third = 0;
                        return (DecoderResult::Malformed(bad), read);
                    }
                },
            };

            if self.third != 0 {
                debug_assert!(!from_pending);
                if !(0x30..=0x39).contains(&byte) {
                    // Restore the second and third bytes along with this one; only
                    // the first byte is consumed by the error.
                    self.pending.set(&[self.second, self.third]);
                    self.first = 0;
                    self.second = 0;
                    self.third = 0;
                    read -= 1;
                    return (DecoderResult::Malformed(1), read);
                }
                let pointer = (u32::from(self.first) - 0x81) * (10 * 126 * 10)
                    + (u32::from(self.second) - 0x30) * (10 * 126)
                    + (u32::from(self.third) - 0x81) * 10
                    + (u32::from(byte) - 0x30);
                self.first = 0;
                self.second = 0;
                self.third = 0;
                match index::gb18030_ranges_code_point(pointer) {
                    Some(code_point) => sink.write_code_point(code_point),
                    None => return (DecoderResult::Malformed(4), read),
                }
                continue;
            }

            if self.second != 0 {
                debug_assert!(!from_pending);
                if (0x81..=0xFE).contains(&byte) {
                    self.third = byte;
                    continue;
                }
                self.pending.set(&[self.second]);
                self.first = 0;
                self.second = 0;
                read -= 1;
                return (DecoderResult::Malformed(1), read);
            }

            if self.first != 0 {
                if (0x30..=0x39).contains(&byte) {
                    self.second = byte;
                    continue;
                }
                let leading = self.first;
                self.first = 0;
                let offset = if byte < 0x7F { 0x40 } else { 0x41 };
                let pointer = if (0x40..=0x7E).contains(&byte) || (0x80..=0xFE).contains(&byte) {
                    Some((usize::from(leading) - 0x81) * 190 + (usize::from(byte) - offset))
                } else {
                    None
                };
                if let Some(code_point) =
                    pointer.and_then(|p| index::code_point(&GB18030_DECODE, p))
                {
                    sink.write_code_point(code_point);
                    continue;
                }
                if byte.is_ascii() {
                    if from_pending {
                        self.pending.unread();
                    } else {
                        read -= 1;
                    }
                    return (DecoderResult::Malformed(1), read);
                }
                return (DecoderResult::Malformed(2), read);
            }

            if byte.is_ascii() {
                sink.write_byte(byte);
            } else if byte == 0x80 {
                sink.write_code_point(0x20AC);
            } else if byte <= 0xFE {
                self.first = byte;
            } else {
                return (DecoderResult::Malformed(1), read);
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Gb18030Encoder {
    is_gbk: bool,
}

impl Gb18030Encoder {
    pub(crate) fn new(is_gbk: bool) -> Self {
        Gb18030Encoder { is_gbk }
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

            if scalar == 0xE5E5 {
                return (EncoderResult::Unmappable(c), read);
            }
            if self.is_gbk && scalar == 0x20AC {
                sink.write_byte(0x80);
                continue;
            }
            if let Some(&(_, first, second)) =
                ENCODER_SIDE_TABLE.iter().find(|&&(cp, _, _)| cp == scalar)
            {
                sink.write_slice(&[first, second]);
                continue;
            }
            if let Some(pointer) = index::pointer(
                &GB18030_ENCODE_CODE_POINTS,
                &GB18030_ENCODE_POINTERS,
                &GB18030_ENCODE_BUCKETS,
                scalar,
            ) {
                let pointer = u32::from(pointer);
                let leading = pointer / 190 + 0x81;
                let trailing = pointer % 190;
                let offset = if trailing < 0x3F { 0x40 } else { 0x41 };
                sink.write_slice(&[leading as u8, (trailing + offset) as u8]);
                continue;
            }
            if self.is_gbk {
                return (EncoderResult::Unmappable(c), read);
            }
            let Some(mut pointer) = index::gb18030_ranges_pointer(scalar) else {
                return (EncoderResult::Unmappable(c), read);
            };
            let byte1 = pointer / (10 * 126 * 10);
            pointer %= 10 * 126 * 10;
            let byte2 = pointer / (10 * 126);
            pointer %= 10 * 126;
            let byte3 = pointer / 10;
            let byte4 = pointer % 10;
            sink.write_slice(&[
                (byte1 + 0x81) as u8,
                (byte2 + 0x30) as u8,
                (byte3 + 0x81) as u8,
                (byte4 + 0x30) as u8,
            ]);
        }
    }
}
