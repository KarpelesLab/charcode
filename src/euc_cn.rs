//! GB 2312-80, in its EUC-CN form.
//!
//! The charset the label `gb2312` actually names.  The WHATWG Encoding
//! Standard resolves that label to GBK, which is a superset — but not a
//! faithful one: it gives two of GB 2312's code points a different character.
//! So this is not GBK narrowed, it is GBK narrowed *and corrected*, which is
//! what [`GB2312_DECODE_DELTA`] holds.
//!
//! It needs no table of its own: the other 7443 code points are in index
//! gb18030 unchanged.

use crate::ascii::ascii_prefix_len_capped;
use crate::index;
use crate::result::{DecoderResult, EncoderResult};
use crate::sink::{ByteSink, DECODER_HEADROOM, ENCODER_HEADROOM};
use crate::tables::gb2312::{GB2312_DECODE_DELTA, GB2312_ENCODE_DELTA};
use crate::tables::gb18030::{
    GB18030_DECODE, GB18030_ENCODE_BUCKETS, GB18030_ENCODE_CODE_POINTS, GB18030_ENCODE_POINTERS,
};

/// The bytes GB 2312 admits, as G1 of EUC-CN.
const LEAD: core::ops::RangeInclusive<u8> = 0xA1..=0xF7;
const TRAIL: core::ops::RangeInclusive<u8> = 0xA1..=0xFE;

/// The pointer index gb18030 uses for a byte pair.  GB 2312's trail bytes are
/// always at or above 0xA1, so the offset is always 0x41.
#[inline]
fn pointer_of(lead: u8, trail: u8) -> usize {
    (usize::from(lead) - 0x81) * 190 + (usize::from(trail) - 0x41)
}

/// The pointers GB 2312 leaves unassigned carry this in the delta table.
const UNASSIGNED: u16 = 0xFFFF;

/// How GB 2312 differs from index gb18030 at a pointer, if it does.
///
/// `Some(None)` means GB 2312 leaves it unassigned where GBK fills it in;
/// `Some(Some(c))` that the two give different characters.
#[inline]
fn decode_delta(pointer: usize) -> Option<Option<u32>> {
    if pointer > u16::MAX as usize {
        return None;
    }
    GB2312_DECODE_DELTA
        .binary_search_by_key(&(pointer as u16), |&(p, _)| p)
        .ok()
        .map(|i| match GB2312_DECODE_DELTA[i].1 {
            UNASSIGNED => None,
            code_point => Some(u32::from(code_point)),
        })
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Gb2312Decoder {
    leading: u8,
}

impl Gb2312Decoder {
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
                if !TRAIL.contains(&byte) {
                    // An ASCII byte is text that follows the broken sequence.
                    if byte.is_ascii() {
                        read -= 1;
                        return (DecoderResult::Malformed(1), read);
                    }
                    return (DecoderResult::Malformed(2), read);
                }
                let pointer = pointer_of(lead, byte);
                let code_point = match decode_delta(pointer) {
                    Some(overridden) => overridden,
                    None => index::code_point(&GB18030_DECODE, pointer),
                };
                match code_point {
                    Some(code_point) => sink.write_code_point(code_point),
                    None => return (DecoderResult::Malformed(2), read),
                }
                continue;
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
pub(crate) struct Gb2312Encoder;

impl Gb2312Encoder {
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

            // The corrected pointers first: index gb18030 has these code points
            // elsewhere, or not at all.
            let pointer = if scalar <= u32::from(u16::MAX)
                && let Ok(i) =
                    GB2312_ENCODE_DELTA.binary_search_by_key(&(scalar as u16), |&(cp, _)| cp)
            {
                usize::from(GB2312_ENCODE_DELTA[i].1)
            } else {
                let Some(pointer) = index::pointer(
                    &GB18030_ENCODE_CODE_POINTS,
                    &GB18030_ENCODE_POINTERS,
                    &GB18030_ENCODE_BUCKETS,
                    scalar,
                ) else {
                    return (EncoderResult::Unmappable(c), read);
                };
                let pointer = usize::from(pointer);
                // A pointer GB 2312 reads differently cannot be used for the
                // code point index gb18030 puts there.
                if decode_delta(pointer).is_some() {
                    return (EncoderResult::Unmappable(c), read);
                }
                pointer
            };

            let (lead, trail) = ((pointer / 190 + 0x81) as u8, (pointer % 190 + 0x41) as u8);
            if !LEAD.contains(&lead) || !TRAIL.contains(&trail) {
                return (EncoderResult::Unmappable(c), read);
            }
            sink.write_slice(&[lead, trail]);
        }
    }
}
