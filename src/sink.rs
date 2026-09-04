//! Bounded output buffers shared by every decoder and encoder.

/// A UTF-8 output buffer.
///
/// Decoders check [`ByteSink::has_room`] before consuming an input byte, which is
/// what lets them report `OutputFull` without having half-consumed a multi-byte
/// sequence.
pub(crate) struct ByteSink<'a> {
    dst: &'a mut [u8],
    written: usize,
}

/// The number of output bytes a decoder guarantees is free before it consumes a byte.
///
/// Four covers the longest single scalar value in UTF-8, the two-scalar-value Big5
/// pointers (2 + 2 bytes), and U+FFFD (3 bytes) written by the replacement wrappers.
pub(crate) const DECODER_HEADROOM: usize = 4;

/// The number of output bytes an encoder guarantees is free before it consumes a
/// character: the longest sequence any encoder emits in one step is the four bytes
/// of a gb18030 four-byte sequence.
pub(crate) const ENCODER_HEADROOM: usize = 4;

impl<'a> ByteSink<'a> {
    pub(crate) fn new(dst: &'a mut [u8]) -> Self {
        ByteSink { dst, written: 0 }
    }

    pub(crate) fn written(&self) -> usize {
        self.written
    }

    pub(crate) fn room(&self) -> usize {
        self.dst.len() - self.written
    }

    pub(crate) fn has_room(&self, needed: usize) -> bool {
        self.room() >= needed
    }

    /// Writes a scalar value as UTF-8.  The caller must have checked for room.
    pub(crate) fn write_char(&mut self, c: char) {
        self.written += c.encode_utf8(&mut self.dst[self.written..]).len();
    }

    /// Writes a code point coming out of an index table.
    ///
    /// No index in the standard contains a surrogate or an out-of-range value, so the
    /// fallback is unreachable; it is here so that table lookups stay panic-free.
    pub(crate) fn write_code_point(&mut self, code_point: u32) {
        self.write_char(char::from_u32(code_point).unwrap_or(char::REPLACEMENT_CHARACTER));
    }

    /// Writes a single byte verbatim.  The caller must have checked for room.
    pub(crate) fn write_byte(&mut self, byte: u8) {
        self.dst[self.written] = byte;
        self.written += 1;
    }

    /// Copies `bytes` verbatim.  The caller must have checked for room.
    pub(crate) fn write_slice(&mut self, bytes: &[u8]) {
        self.dst[self.written..self.written + bytes.len()].copy_from_slice(bytes);
        self.written += bytes.len();
    }

    /// Copies as much of `bytes` as fits and returns how much was copied.
    pub(crate) fn write_slice_partial(&mut self, bytes: &[u8]) -> usize {
        let n = core::cmp::min(bytes.len(), self.room());
        self.write_slice(&bytes[..n]);
        n
    }
}

/// A queue of at most two bytes that a decoder has pushed back into its input.
///
/// The standard describes errors in terms of restoring bytes to the input stream.
/// Every such restore involves at most the current byte, which the decoder simply
/// leaves unconsumed, plus at most two bytes it had been holding in its own state.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Pushback {
    buf: [u8; 2],
    pos: u8,
    len: u8,
}

impl Pushback {
    pub(crate) fn is_empty(&self) -> bool {
        self.pos == self.len
    }

    /// Queues `bytes` to be read before any further input.  Only valid when empty.
    pub(crate) fn set(&mut self, bytes: &[u8]) {
        debug_assert!(self.is_empty() && bytes.len() <= self.buf.len());
        self.buf[..bytes.len()].copy_from_slice(bytes);
        self.pos = 0;
        self.len = bytes.len() as u8;
    }

    pub(crate) fn next(&mut self) -> Option<u8> {
        if self.is_empty() {
            return None;
        }
        let byte = self.buf[self.pos as usize];
        self.pos += 1;
        Some(byte)
    }

    /// Un-reads the byte most recently returned by [`Pushback::next`].
    pub(crate) fn unread(&mut self) {
        debug_assert!(self.pos > 0);
        self.pos -= 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sink_writes_utf8() {
        let mut buf = [0u8; 8];
        let mut sink = ByteSink::new(&mut buf);
        sink.write_char('a');
        sink.write_code_point(0x20AC);
        assert_eq!(sink.written(), 4);
        assert_eq!(&buf[..4], b"a\xE2\x82\xAC");
    }

    #[test]
    fn partial_write_reports_what_fit() {
        let mut buf = [0u8; 3];
        let mut sink = ByteSink::new(&mut buf);
        assert_eq!(sink.write_slice_partial(b"abcdef"), 3);
        assert_eq!(sink.room(), 0);
    }

    #[test]
    fn pushback_is_fifo() {
        let mut pb = Pushback::default();
        assert!(pb.is_empty());
        pb.set(&[0x30, 0x81]);
        assert_eq!(pb.next(), Some(0x30));
        assert_eq!(pb.next(), Some(0x81));
        assert_eq!(pb.next(), None);
        assert!(pb.is_empty());
    }
}
