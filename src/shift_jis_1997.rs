//! Shift_JIS as standardised, in JIS X 0208:1997 Annex 1.
//!
//! JIS X 0201's Roman set in the single-byte range and JIS X 0208 in the
//! double-byte one — nothing else.  The WHATWG Encoding Standard's encoding of
//! the same name is Windows codepage 932: ASCII rather than JIS X 0201 below
//! 0x80, the NEC and IBM extension rows on top of JIS X 0208, the end-user
//! defined area behind lead bytes 0xF0 to 0xF9, and six of JIS X 0208's own
//! pointers given a different character.  That one is [`WINDOWS_31J`].
//!
//! The visible difference is 0x5C: the yen sign here, the backslash there.
//! Both readings are in wide use — glibc's `SHIFT_JIS` gives the yen sign,
//! Python's `shift_jis` the backslash — but only one of them is Shift_JIS, and
//! text that means the backslash is codepage 932.
//!
//! This is byte for byte what glibc's `iconv -f SHIFT_JIS` does, over the whole
//! one- and two-byte space.
//!
//! [`WINDOWS_31J`]: crate::WINDOWS_31J

use crate::ascii::jis_roman_prefix_len;
use crate::index;
use crate::result::{DecoderResult, EncoderResult};
use crate::sink::{ByteSink, DECODER_HEADROOM, ENCODER_HEADROOM};
use crate::tables::jis::JIS0208_DECODE;
use crate::tables::jis0208_1997::{
    JIS0208_1997_DECODE_DELTA, JIS0208_1997_ENCODE_BUCKETS, JIS0208_1997_ENCODE_CODE_POINTS,
    JIS0208_1997_ENCODE_POINTERS,
};

/// The pointers JIS X 0208 leaves unassigned carry this in the delta table.
const UNASSIGNED: u32 = 0xFFFF;

/// How JIS X 0208 differs from index jis0208 at a pointer, if it does.
///
/// `Some(None)` means the pointer is one of the NEC or IBM extension rows the
/// index folds in; `Some(Some(c))` that the two give different characters —
/// the wave dash, double vertical line, minus sign, cent, pound and not signs.
#[inline]
pub(crate) fn decode_delta(pointer: usize) -> Option<Option<u32>> {
    if pointer > u16::MAX as usize {
        return None;
    }
    JIS0208_1997_DECODE_DELTA
        .binary_search_by_key(&(pointer as u16), |&(p, _)| p)
        .ok()
        .map(|i| match JIS0208_1997_DECODE_DELTA[i].1 {
            UNASSIGNED => None,
            code_point => Some(code_point),
        })
}

/// `the index jis0208 code point for pointer`, corrected to JIS X 0208.
#[inline]
pub(crate) fn code_point(pointer: usize) -> Option<u32> {
    match decode_delta(pointer) {
        Some(overridden) => overridden,
        None => index::code_point(&JIS0208_DECODE, pointer),
    }
}

/// `the index jis0208 pointer for code point`, over JIS X 0208 alone.
#[inline]
pub(crate) fn pointer(scalar: u32) -> Option<u16> {
    index::pointer(
        &JIS0208_1997_ENCODE_CODE_POINTS,
        &JIS0208_1997_ENCODE_POINTERS,
        &JIS0208_1997_ENCODE_BUCKETS,
        scalar,
    )
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct ShiftJis1997Decoder {
    leading: u8,
}

impl ShiftJis1997Decoder {
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
                let run = jis_roman_prefix_len(&src[read..], sink.room());
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
                let leading = self.leading;
                self.leading = 0;
                let pointer = if (0x40..=0x7E).contains(&byte) || (0x80..=0xFC).contains(&byte) {
                    let offset = if byte < 0x7F { 0x40 } else { 0x41 };
                    let leading_offset = if leading < 0xA0 { 0x81 } else { 0xC1 };
                    Some(
                        (usize::from(leading) - leading_offset) * 188
                            + (usize::from(byte) - offset),
                    )
                } else {
                    None
                };
                if let Some(code_point) = pointer.and_then(code_point) {
                    sink.write_code_point(code_point);
                    continue;
                }
                // An ASCII byte is text that follows the broken sequence.
                if byte.is_ascii() {
                    read -= 1;
                    return (DecoderResult::Malformed(1), read);
                }
                return (DecoderResult::Malformed(2), read);
            }

            match byte {
                // JIS X 0201's Roman set is ASCII but for these two.
                0x5C => sink.write_code_point(0x00A5),
                0x7E => sink.write_code_point(0x203E),
                _ if byte.is_ascii() => sink.write_byte(byte),
                0xA1..=0xDF => sink.write_code_point(0xFF61 - 0xA1 + u32::from(byte)),
                // Lead 0xF0 and up is the Windows end-user defined area, which
                // is not part of the charset; 0x81 to 0xEF is exactly the 94
                // rows JIS X 0208 defines.
                0x81..=0x9F | 0xE0..=0xEF => self.leading = byte,
                _ => return (DecoderResult::Malformed(1), read),
            }
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct ShiftJis1997Encoder;

impl ShiftJis1997Encoder {
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
            let run = jis_roman_prefix_len(rest.as_bytes(), sink.room());
            if run > 0 {
                sink.write_slice(&rest.as_bytes()[..run]);
                read += run;
                continue;
            }
            read += c.len_utf8();

            match u32::from(c) {
                // 0x5C and 0x7E are the yen sign and the overline, so the
                // charset has no backslash or tilde left to give back.  The
                // fullwidth forms are U+FF3C and U+FF5E, and those it has.
                0x005C | 0x007E => return (EncoderResult::Unmappable(c), read),
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
                scalar => {
                    let Some(pointer) = pointer(scalar) else {
                        return (EncoderResult::Unmappable(c), read);
                    };
                    let pointer = u32::from(pointer);
                    let leading = pointer / 188;
                    let leading_offset = if leading < 0x1F { 0x81 } else { 0xC1 };
                    let trailing = pointer % 188;
                    let offset = if trailing < 0x3F { 0x40 } else { 0x41 };
                    sink.write_slice(&[
                        (leading + leading_offset) as u8,
                        (trailing + offset) as u8,
                    ]);
                }
            }
        }
    }
}
