//! JIS X 0208 itself, as a narrowing of the standard's index jis0208.
//!
//! The standard's index is JIS X 0208 with the NEC and IBM extension rows
//! folded in — row 13, rows 89 to 92 and rows 115 to 119 — and it gives six of
//! JIS X 0208's own pointers a different character.  These two accessors put
//! that back, and are shared by Shift_JIS, EUC-JP and ISO-2022-JP.

use crate::index;
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
