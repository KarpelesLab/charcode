//! UTF-8.
//!
//! The decoder delegates the actual validation to [`core::str::from_utf8`], whose
//! error reporting already follows the Unicode "Best Practices for Using U+FFFD"
//! substitution that the Encoding Standard mandates.  That keeps the bulk of the
//! work a `memcpy` and leaves only sequences straddling a call boundary to the
//! byte-at-a-time path.

use crate::result::{DecoderResult, EncoderResult};
use crate::sink::{ByteSink, DECODER_HEADROOM, ENCODER_HEADROOM};

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Utf8Decoder {
    /// A sequence that is a valid prefix so far but is not yet complete.
    buf: [u8; 4],
    len: u8,
}

impl Utf8Decoder {
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

            // Finish the sequence carried over from a previous call, one byte at a
            // time.  This runs at most three times per stream position.
            if self.len > 0 {
                if read == src.len() {
                    if !last {
                        return (DecoderResult::InputEmpty, read);
                    }
                    let bad = self.len;
                    self.len = 0;
                    return (DecoderResult::Malformed(bad), read);
                }
                self.buf[self.len as usize] = src[read];
                self.len += 1;
                read += 1;
                match core::str::from_utf8(&self.buf[..self.len as usize]) {
                    Ok(s) => {
                        sink.write_slice(s.as_bytes());
                        self.len = 0;
                    }
                    Err(e) => match e.error_len() {
                        // Still incomplete: keep collecting.
                        None => {}
                        Some(bad) => {
                            // The byte just added terminated the sequence without
                            // completing it, so it belongs to the next one.
                            debug_assert_eq!(bad, self.len as usize - 1);
                            self.len = 0;
                            read -= 1;
                            return (DecoderResult::Malformed(bad as u8), read);
                        }
                    },
                }
                continue;
            }

            let rest = &src[read..];
            if rest.is_empty() {
                return (DecoderResult::InputEmpty, read);
            }

            let (valid, error_len) = match core::str::from_utf8(rest) {
                Ok(s) => (s.len(), None),
                Err(e) => (e.valid_up_to(), Some(e.error_len())),
            };

            if valid > 0 {
                let copied = copy_on_char_boundary(sink, &rest[..valid]);
                read += copied;
                if copied < valid {
                    return (DecoderResult::OutputFull, read);
                }
            }

            match error_len {
                None => return (DecoderResult::InputEmpty, read),
                Some(Some(bad)) => {
                    if !sink.has_room(DECODER_HEADROOM) {
                        // Report the error only once there is room for a
                        // substitution; the next call re-discovers it unchanged.
                        return (DecoderResult::OutputFull, read);
                    }
                    read += bad;
                    return (DecoderResult::Malformed(bad as u8), read);
                }
                Some(None) => {
                    // The input ends in the middle of a sequence.
                    let tail = &rest[valid..];
                    debug_assert!(tail.len() < self.buf.len());
                    self.buf[..tail.len()].copy_from_slice(tail);
                    self.len = tail.len() as u8;
                    read += tail.len();
                    if !last {
                        return (DecoderResult::InputEmpty, read);
                    }
                }
            }
        }
    }
}

/// Copies as much of `bytes` as fits, truncated to a UTF-8 character boundary so
/// that the output stays valid UTF-8 at every step.
fn copy_on_char_boundary(sink: &mut ByteSink, bytes: &[u8]) -> usize {
    let mut n = core::cmp::min(bytes.len(), sink.room());
    if n < bytes.len() {
        // `bytes` is known-valid UTF-8, so backing up finds a boundary within
        // three bytes.
        while n > 0 && bytes[n] & 0xC0 == 0x80 {
            n -= 1;
        }
    }
    sink.write_slice(&bytes[..n]);
    n
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Utf8Encoder;

impl Utf8Encoder {
    pub(crate) fn encode(&mut self, src: &str, sink: &mut ByteSink) -> (EncoderResult, usize) {
        if !sink.has_room(ENCODER_HEADROOM) && !src.is_empty() {
            return (EncoderResult::OutputFull, 0);
        }
        let copied = copy_on_char_boundary(sink, src.as_bytes());
        if copied < src.len() {
            (EncoderResult::OutputFull, copied)
        } else {
            (EncoderResult::InputEmpty, copied)
        }
    }
}
