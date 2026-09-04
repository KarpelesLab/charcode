//! Word-at-a-time ASCII scanning, used to take shortcuts in the ASCII-compatible
//! encodings.  Nothing here uses `unsafe`; the chunked loop is enough to keep the
//! common all-ASCII case from costing a branch per byte.

const CHUNK: usize = 8;
const HIGH_BITS: u64 = 0x8080_8080_8080_8080;

/// Returns the length of the leading run of ASCII bytes in `bytes`.
pub(crate) fn ascii_prefix_len(bytes: &[u8]) -> usize {
    let mut offset = 0;
    let (chunks, _) = bytes.as_chunks::<CHUNK>();
    for chunk in chunks {
        if u64::from_ne_bytes(*chunk) & HIGH_BITS != 0 {
            break;
        }
        offset += CHUNK;
    }
    while offset < bytes.len() && bytes[offset] < 0x80 {
        offset += 1;
    }
    offset
}

/// The length of the leading ASCII run, looking no further than `limit` bytes.
///
/// The cap is not an optimization, it is the difference between linear and
/// quadratic: a converter that can only take `limit` bytes of output would
/// otherwise rescan the whole remaining input on every call and throw all but
/// `limit` of the answer away.
///
/// On a `&str` the result is always a character boundary, since a non-ASCII
/// scalar value never begins with an ASCII byte in UTF-8.
pub(crate) fn ascii_prefix_len_capped(bytes: &[u8], limit: usize) -> usize {
    ascii_prefix_len(&bytes[..core::cmp::min(bytes.len(), limit)])
}

/// Returns true if every byte is ASCII.
///
/// Only the borrowing `Cow` fast paths ask, and those need an allocator.
#[cfg(any(feature = "alloc", test))]
pub(crate) fn is_ascii(bytes: &[u8]) -> bool {
    ascii_prefix_len(bytes) == bytes.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefix_len() {
        assert_eq!(ascii_prefix_len(b""), 0);
        assert_eq!(ascii_prefix_len(b"hello"), 5);
        assert_eq!(ascii_prefix_len(b"\x80"), 0);
        assert_eq!(ascii_prefix_len(b"0123456789abcdef\x80"), 16);
        assert_eq!(ascii_prefix_len(b"0123456\x80abcdef"), 7);
        // The tail loop has to pick up where the chunked loop stopped.
        assert_eq!(ascii_prefix_len(b"0123456789\xC3\xA9"), 10);
    }

    #[test]
    fn the_cap_bounds_the_scan() {
        let bytes = b"0123456789abcdef";
        assert_eq!(ascii_prefix_len_capped(bytes, 4), 4);
        assert_eq!(ascii_prefix_len_capped(bytes, 100), 16);
        assert_eq!(ascii_prefix_len_capped(b"ab\x80cd", 100), 2);
        assert_eq!(ascii_prefix_len_capped(b"", 8), 0);
    }

    #[test]
    fn all_ascii() {
        assert!(is_ascii(b"plain ascii text, all of it"));
        assert!(!is_ascii("caf\u{E9}".as_bytes()));
    }
}
