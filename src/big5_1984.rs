//! Big5 as standardised, which is not what the WHATWG Encoding Standard calls
//! "Big5".
//!
//! The standard's index is Big5 plus the Hong Kong Supplementary Character Set
//! and other common extensions, and it is not a faithful superset: within
//! Big5's own lead range it gives 260 pointers a different character — 0xA145
//! is U+2022 bullet here and U+2027 hyphenation point there — and fills 198
//! that Big5 leaves undefined.  So `for_label(b"big5")` answers with this and
//! `for_whatwg_label(b"big5")` with [`BIG5_HKSCS`](crate::BIG5_HKSCS).
//!
//! No table of its own: lead 0xA1 to 0xF9, plus those 458 overrides.

use crate::ascii::ascii_prefix_len_capped;
use crate::index;
use crate::result::{DecoderResult, EncoderResult};
use crate::sink::{ByteSink, DECODER_HEADROOM, ENCODER_HEADROOM};
use crate::tables::big5::{
    BIG5_DECODE, BIG5_ENCODE_BUCKETS, BIG5_ENCODE_CODE_POINTS, BIG5_ENCODE_POINTERS,
};
use crate::tables::big5_1984::{BIG5_1984_DECODE_DELTA, BIG5_1984_ENCODE_DELTA};

/// Marks a pointer Big5 leaves undefined where the standard's index fills it.
const UNASSIGNED: u32 = 0xFFFF;

/// The lead bytes Big5 itself uses; below 0xA1 is the HKSCS extension area.
const LEAD: core::ops::RangeInclusive<u8> = 0xA1..=0xF9;

#[inline]
fn pointer_of(lead: u8, trail: u8) -> usize {
    let offset = if trail < 0x7F { 0x40 } else { 0x62 };
    (usize::from(lead) - 0x81) * 157 + (usize::from(trail) - offset)
}

/// How Big5 differs from the standard's index at a pointer, if it does.
#[inline]
fn decode_delta(pointer: usize) -> Option<Option<u32>> {
    if pointer > u16::MAX as usize {
        return None;
    }
    BIG5_1984_DECODE_DELTA
        .binary_search_by_key(&(pointer as u16), |&(p, _)| p)
        .ok()
        .map(|i| match BIG5_1984_DECODE_DELTA[i].1 {
            UNASSIGNED => None,
            code_point => Some(code_point),
        })
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Big5_1984Decoder {
    leading: u8,
}

impl Big5_1984Decoder {
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
                if !last || self.leading == 0 {
                    return (DecoderResult::InputEmpty, read);
                }
                self.leading = 0;
                return (DecoderResult::Malformed(1), read);
            };
            read += 1;

            if self.leading != 0 {
                let lead = self.leading;
                self.leading = 0;
                let valid = (0x40..=0x7E).contains(&byte) || (0xA1..=0xFE).contains(&byte);
                let code_point = if valid {
                    let pointer = pointer_of(lead, byte);
                    match decode_delta(pointer) {
                        Some(overridden) => overridden,
                        None => index::code_point_wide(&BIG5_DECODE, pointer),
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
                    return (DecoderResult::Malformed(1), read);
                }
                return (DecoderResult::Malformed(2), read);
            }

            if byte.is_ascii() {
                sink.write_byte(byte);
            } else if LEAD.contains(&byte) {
                self.leading = byte;
            } else {
                return (DecoderResult::Malformed(1), read);
            }
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Big5_1984Encoder;

impl Big5_1984Encoder {
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

            let pointer = if scalar <= u32::from(u16::MAX)
                && let Ok(i) =
                    BIG5_1984_ENCODE_DELTA.binary_search_by_key(&(scalar as u16), |&(cp, _)| cp)
            {
                usize::from(BIG5_1984_ENCODE_DELTA[i].1)
            } else {
                let Some(pointer) = index::pointer_wide(
                    &BIG5_ENCODE_CODE_POINTS,
                    &BIG5_ENCODE_POINTERS,
                    &BIG5_ENCODE_BUCKETS,
                    scalar,
                ) else {
                    return (EncoderResult::Unmappable(c), read);
                };
                let pointer = usize::from(pointer);
                // A pointer Big5 reads differently cannot stand for the code
                // point the standard's index puts there.
                if decode_delta(pointer).is_some() {
                    return (EncoderResult::Unmappable(c), read);
                }
                pointer
            };

            let lead = (pointer / 157 + 0x81) as u8;
            let trailing = pointer % 157;
            let trail = (trailing + if trailing < 0x3F { 0x40 } else { 0x62 }) as u8;
            if !LEAD.contains(&lead) {
                return (EncoderResult::Unmappable(c), read);
            }
            sink.write_slice(&[lead, trail]);
        }
    }
}
