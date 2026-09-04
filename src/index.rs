//! Lookups over the generated index tables.
//!
//! Decode tables are indexed by pointer and use 0 to mean "no code point at this
//! pointer"; no index in the standard maps a pointer to U+0000, so the sentinel is
//! unambiguous.  Encode tables are two parallel arrays sorted by code point, giving
//! the `index pointer for code point` operation as a binary search.

#[cfg(feature = "gb18030")]
use crate::tables::gb18030::GB18030_RANGES;

/// `the index code point for pointer in index`, for a 16-bit table.
#[cfg(any(
    feature = "euc-jp",
    feature = "euc-kr",
    feature = "gb18030",
    feature = "iso-2022-jp",
    feature = "shift-jis"
))]
#[inline]
pub(crate) fn code_point(table: &[u16], pointer: usize) -> Option<u32> {
    match table.get(pointer).copied() {
        None | Some(0) => None,
        Some(cp) => Some(u32::from(cp)),
    }
}

/// `the index code point for pointer in index`, for a 32-bit table (Big5 only).
#[cfg(feature = "big5")]
#[inline]
pub(crate) fn code_point_wide(table: &[u32], pointer: usize) -> Option<u32> {
    match table.get(pointer).copied() {
        None | Some(0) => None,
        Some(cp) => Some(cp),
    }
}

/// The slice of `code_points` that could hold `scalar`, per the bucket index.
///
/// Searching one bucket rather than the whole table is what keeps an encode
/// lookup to a few probes over a contiguous run instead of fifteen scattered
/// ones.
#[inline]
fn bucket(buckets: &[u16; 258], scalar: u32) -> core::ops::Range<usize> {
    let high = core::cmp::min(scalar >> 8, 0x100) as usize;
    usize::from(buckets[high])..usize::from(buckets[high + 1])
}

/// `the index pointer for code point in index`, for a 16-bit table.
#[cfg(any(
    feature = "euc-jp",
    feature = "euc-kr",
    feature = "gb18030",
    feature = "iso-2022-jp",
    feature = "shift-jis"
))]
#[inline]
pub(crate) fn pointer(
    code_points: &[u16],
    pointers: &[u16],
    buckets: &[u16; 258],
    scalar: u32,
) -> Option<u16> {
    if scalar > 0xFFFF {
        return None;
    }
    let range = bucket(buckets, scalar);
    let found = code_points[range.clone()]
        .binary_search(&(scalar as u16))
        .ok()?;
    pointers.get(range.start + found).copied()
}

/// `the index pointer for code point in index`, for a 32-bit table (Big5 only).
#[cfg(feature = "big5")]
#[inline]
pub(crate) fn pointer_wide(
    code_points: &[u32],
    pointers: &[u16],
    buckets: &[u16; 258],
    scalar: u32,
) -> Option<u16> {
    let range = bucket(buckets, scalar);
    let found = code_points[range.clone()].binary_search(&scalar).ok()?;
    pointers.get(range.start + found).copied()
}

/// `the index gb18030 ranges code point for pointer`.
#[cfg(feature = "gb18030")]
pub(crate) fn gb18030_ranges_code_point(pointer: u32) -> Option<u32> {
    if (pointer > 39419 && pointer < 189_000) || pointer > 1_237_575 {
        return None;
    }
    if pointer == 7457 {
        return Some(0xE7C7);
    }
    let i = GB18030_RANGES
        .partition_point(|&(p, _)| p <= pointer)
        .checked_sub(1)?;
    let (offset, code_point_offset) = GB18030_RANGES[i];
    Some(code_point_offset + (pointer - offset))
}

/// `the index gb18030 ranges pointer for code point`.
#[cfg(feature = "gb18030")]
pub(crate) fn gb18030_ranges_pointer(scalar: u32) -> Option<u32> {
    if scalar == 0xE7C7 {
        return Some(7457);
    }
    let i = GB18030_RANGES
        .partition_point(|&(_, c)| c <= scalar)
        .checked_sub(1)?;
    let (offset, code_point_offset) = GB18030_RANGES[i];
    Some(offset + (scalar - code_point_offset))
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;

    #[cfg(feature = "gb18030")]
    #[test]
    fn ranges_round_trip() {
        for &pointer in &[0u32, 7457, 39419, 189_000, 1_237_575] {
            let cp = gb18030_ranges_code_point(pointer).expect("mapped pointer");
            assert_eq!(
                gb18030_ranges_pointer(cp),
                Some(pointer),
                "pointer {pointer}"
            );
        }
        assert_eq!(gb18030_ranges_code_point(7457), Some(0xE7C7));
        assert_eq!(gb18030_ranges_code_point(39420), None);
        assert_eq!(gb18030_ranges_code_point(1_237_576), None);
        assert_eq!(gb18030_ranges_code_point(189_000), Some(0x10000));
        assert_eq!(gb18030_ranges_pointer(0x10FFFF), Some(1_237_575));
    }

    #[cfg(any(
        feature = "euc-jp",
        feature = "euc-kr",
        feature = "gb18030",
        feature = "iso-2022-jp",
        feature = "shift-jis"
    ))]
    #[test]
    fn sentinel_means_unmapped() {
        assert_eq!(code_point(&[0, 0x41], 0), None);
        assert_eq!(code_point(&[0, 0x41], 1), Some(0x41));
        assert_eq!(code_point(&[0, 0x41], 2), None);
    }
}
