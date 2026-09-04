//! The streaming encoder.

use alloc::vec::Vec;

use crate::Encoding;
use crate::result::{CoderResult, EncoderResult};
use crate::sink::ByteSink;
use crate::variant::VariantEncoder;

/// The smallest output buffer [`Encoder::encode_from_utf8`] can make progress with.
pub const ENCODER_MIN_BUFFER: usize = 4;

/// The longest numeric character reference the replacement path can write:
/// `&#1114111;`.
const MAX_NUMERIC_REFERENCE: usize = 10;

/// A streaming encoder for one text stream.
///
/// Created by [`Encoding::new_encoder`].  Because the standard has no encoder for
/// `replacement`, UTF-16BE or UTF-16LE, asking those encodings for one yields a
/// UTF-8 encoder; [`Encoder::encoding`] reports what is actually being written.
///
/// # Examples
///
/// ```
/// use charcode::WINDOWS_1252;
///
/// let mut encoder = WINDOWS_1252.new_encoder();
/// let mut bytes = Vec::new();
/// let had_unmappable = encoder.encode_from_str(" \u{20AC} \u{4E00}", &mut bytes, true);
/// assert!(had_unmappable);
/// // The euro sign is in windows-1252; the ideograph is not, so it becomes a
/// // numeric character reference.
/// assert_eq!(bytes, b" \x80 &#19968;");
/// ```
#[derive(Debug, Clone)]
pub struct Encoder {
    encoding: &'static Encoding,
    variant: VariantEncoder,
    /// A numeric character reference that did not fit in the previous output
    /// buffer, carried over so that any buffer size works.
    pending: [u8; MAX_NUMERIC_REFERENCE],
    pending_pos: u8,
    pending_len: u8,
}

impl Encoder {
    pub(crate) fn new(encoding: &'static Encoding) -> Encoder {
        Encoder {
            encoding,
            variant: encoding.variant().new_encoder(),
            pending: [0; MAX_NUMERIC_REFERENCE],
            pending_pos: 0,
            pending_len: 0,
        }
    }

    /// The encoding being written, which is always an output encoding.
    pub fn encoding(&self) -> &'static Encoding {
        self.encoding
    }

    /// An upper bound on the bytes that encoding `byte_length` more UTF-8 bytes
    /// can produce when unmappable characters are replaced.
    pub fn max_buffer_length_from_utf8(&self, byte_length: usize) -> Option<usize> {
        self.variant.max_buffer_length_from_utf8(byte_length)
    }

    /// An upper bound for the case where every character is representable.
    pub fn max_buffer_length_from_utf8_without_replacement(
        &self,
        byte_length: usize,
    ) -> Option<usize> {
        self.variant
            .max_buffer_length_from_utf8_without_replacement(byte_length)
    }

    /// Encodes into `dst`, reporting unmappable characters instead of replacing
    /// them.
    ///
    /// Returns why it stopped, how many bytes of `src` were consumed (always on a
    /// character boundary) and how many bytes were written.  `dst` must be at
    /// least [`ENCODER_MIN_BUFFER`] bytes long or no progress is possible.
    pub fn encode_from_utf8(
        &mut self,
        src: &str,
        dst: &mut [u8],
        last: bool,
    ) -> (EncoderResult, usize, usize) {
        let mut sink = ByteSink::new(dst);
        if !self.drain_pending(&mut sink) {
            return (EncoderResult::OutputFull, 0, sink.written());
        }
        let (result, read) = self.variant.encode(src, &mut sink, last);
        (result, read, sink.written())
    }

    /// Encodes into `dst`, replacing each unmappable character with a decimal
    /// numeric character reference, as HTML form submission does.
    ///
    /// The final `bool` is true if at least one character was replaced.
    pub fn encode_from_utf8_with_replacement(
        &mut self,
        src: &str,
        dst: &mut [u8],
        last: bool,
    ) -> (CoderResult, usize, usize, bool) {
        let mut total_read = 0usize;
        let mut total_written = 0usize;
        let mut had_unmappable = false;
        loop {
            let (result, read, written) =
                self.encode_from_utf8(&src[total_read..], &mut dst[total_written..], last);
            total_read += read;
            total_written += written;
            match result {
                EncoderResult::Unmappable(c) => {
                    had_unmappable = true;
                    self.queue_numeric_reference(c);
                    let mut sink = ByteSink::new(&mut dst[total_written..]);
                    let drained = self.drain_pending(&mut sink);
                    total_written += sink.written();
                    if !drained {
                        return (
                            CoderResult::OutputFull,
                            total_read,
                            total_written,
                            had_unmappable,
                        );
                    }
                }
                other => {
                    return (
                        other.as_coder_result(),
                        total_read,
                        total_written,
                        had_unmappable,
                    );
                }
            }
        }
    }

    /// Encodes all of `src`, appending to `dst` and replacing each unmappable
    /// character with a numeric character reference.  Returns true if at least one
    /// character was replaced.
    pub fn encode_from_str(&mut self, src: &str, dst: &mut Vec<u8>, last: bool) -> bool {
        let mut buffer = [0u8; 1024];
        let mut read = 0usize;
        let mut had_unmappable = false;
        loop {
            let (result, n, written, unmappable) =
                self.encode_from_utf8_with_replacement(&src[read..], &mut buffer, last);
            read += n;
            had_unmappable |= unmappable;
            dst.extend_from_slice(&buffer[..written]);
            if result == CoderResult::InputEmpty {
                return had_unmappable;
            }
        }
    }

    /// Encodes all of `src`, appending to `dst` and stopping at the first
    /// character the encoding cannot represent, which is returned as `Err`.
    pub fn encode_from_str_without_replacement(
        &mut self,
        src: &str,
        dst: &mut Vec<u8>,
        last: bool,
    ) -> Result<(), UnmappableError> {
        let mut buffer = [0u8; 1024];
        let mut read = 0usize;
        loop {
            let (result, n, written) = self.encode_from_utf8(&src[read..], &mut buffer, last);
            read += n;
            dst.extend_from_slice(&buffer[..written]);
            match result {
                EncoderResult::InputEmpty => return Ok(()),
                EncoderResult::OutputFull => {}
                EncoderResult::Unmappable(c) => {
                    return Err(UnmappableError {
                        character: c,
                        offset: read,
                    });
                }
            }
        }
    }

    fn queue_numeric_reference(&mut self, c: char) {
        debug_assert_eq!(self.pending_len, 0);
        let mut digits = [0u8; 7];
        let mut value = u32::from(c);
        let mut n = 0;
        loop {
            digits[n] = b'0' + (value % 10) as u8;
            value /= 10;
            n += 1;
            if value == 0 {
                break;
            }
        }
        let mut len = 0;
        for (i, byte) in b"&#".iter().enumerate() {
            self.pending[i] = *byte;
            len += 1;
        }
        for i in (0..n).rev() {
            self.pending[len] = digits[i];
            len += 1;
        }
        self.pending[len] = b';';
        len += 1;
        self.pending_pos = 0;
        self.pending_len = len as u8;
    }

    /// Writes as much of the carried-over numeric reference as fits.  Returns
    /// false if some of it is still pending.
    fn drain_pending(&mut self, sink: &mut ByteSink) -> bool {
        if self.pending_pos == self.pending_len {
            return true;
        }
        let (start, end) = (usize::from(self.pending_pos), usize::from(self.pending_len));
        let written = sink.write_slice_partial(&self.pending[start..end]);
        self.pending_pos += written as u8;
        if self.pending_pos == self.pending_len {
            self.pending_pos = 0;
            self.pending_len = 0;
            true
        } else {
            false
        }
    }
}

/// A character that the target encoding cannot represent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnmappableError {
    /// The character that could not be encoded.
    pub character: char,
    /// How far into the string passed to this call encoding got, counted in UTF-8
    /// bytes and including the unmappable character.
    pub offset: usize,
}

impl core::fmt::Display for UnmappableError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "U+{:04X} cannot be represented in this encoding (at offset {})",
            u32::from(self.character),
            self.offset
        )
    }
}

#[cfg(feature = "std")]
impl std::error::Error for UnmappableError {}
