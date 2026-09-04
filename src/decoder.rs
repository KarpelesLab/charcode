//! The streaming decoder.

use alloc::string::String;

use crate::Encoding;
use crate::encodings::{UTF_8, UTF_16BE, UTF_16LE};
use crate::result::{CoderResult, DecoderResult};
use crate::sink::ByteSink;
use crate::variant::VariantDecoder;

/// The smallest output buffer [`Decoder::decode_to_utf8`] can make progress with.
///
/// It is the longest sequence a decoder writes for one input byte: four bytes for
/// a supplementary scalar value, or for the two scalar values some Big5 pointers
/// stand for.
pub const DECODER_MIN_BUFFER: usize = 4;

/// The three byte order marks the standard recognizes, in sniffing order.
const BOMS: [(&[u8], &Encoding); 3] = [
    (&[0xEF, 0xBB, 0xBF], UTF_8),
    (&[0xFE, 0xFF], UTF_16BE),
    (&[0xFF, 0xFE], UTF_16LE),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BomHandling {
    /// Look for any BOM and switch to the encoding it names.
    Sniff,
    /// Strip only this decoder's own BOM.
    Remove,
    /// Treat a BOM as ordinary content.
    Off,
}

enum Decision {
    /// More bytes are needed before a BOM can be ruled in or out.
    NeedMore,
    Bom(&'static Encoding),
    NoBom,
}

/// A streaming decoder for one byte stream.
///
/// A decoder is created from an [`Encoding`] and holds the state that a partial
/// sequence at the end of one input buffer needs in order to be finished by the
/// next.  Feed it buffers in order and pass `last = true` with the final one so
/// that a truncated sequence is reported instead of silently dropped.
///
/// # Examples
///
/// ```
/// use charcode::{CoderResult, SHIFT_JIS};
///
/// let mut decoder = SHIFT_JIS.new_decoder();
/// let mut text = String::new();
/// // The two bytes of this character arrive in separate chunks.
/// decoder.decode_to_string(&[0x93], &mut text, false);
/// decoder.decode_to_string(&[0xFA], &mut text, true);
/// assert_eq!(text, "\u{65E5}");
/// # let _ = CoderResult::InputEmpty;
/// ```
#[derive(Debug, Clone)]
pub struct Decoder {
    encoding: &'static Encoding,
    variant: VariantDecoder,
    bom: BomHandling,
    /// Bytes held back while deciding whether they start a BOM.
    bom_buf: [u8; 3],
    bom_len: u8,
    /// Bytes that turned out not to be a BOM and still have to be decoded.
    prefix: [u8; 3],
    prefix_pos: u8,
    prefix_len: u8,
}

impl Decoder {
    pub(crate) fn new(encoding: &'static Encoding, sniff: bool, remove: bool) -> Decoder {
        let bom = if sniff {
            BomHandling::Sniff
        } else if remove && encoding.bom().is_some() {
            BomHandling::Remove
        } else {
            BomHandling::Off
        };
        Decoder {
            encoding,
            variant: encoding.variant().new_decoder(),
            bom,
            bom_buf: [0; 3],
            bom_len: 0,
            prefix: [0; 3],
            prefix_pos: 0,
            prefix_len: 0,
        }
    }

    /// The encoding being decoded from.
    ///
    /// For a BOM-sniffing decoder this changes, once, if the stream turns out to
    /// start with a byte order mark that names a different encoding.
    pub fn encoding(&self) -> &'static Encoding {
        self.encoding
    }

    /// An upper bound on the UTF-8 bytes that decoding `byte_length` more input
    /// bytes can produce.  `None` if the count would overflow `usize`.
    pub fn max_utf8_buffer_length(&self, byte_length: usize) -> Option<usize> {
        self.variant.max_utf8_buffer_length(byte_length)
    }

    /// Decodes into `dst`, reporting malformed sequences instead of substituting
    /// them.
    ///
    /// Returns why it stopped, how many bytes of `src` were consumed and how many
    /// bytes were written.  The written bytes are always valid UTF-8.  `dst` must
    /// be at least [`DECODER_MIN_BUFFER`] bytes long or no progress is possible.
    ///
    /// Pass `last = true` only with the final buffer of the stream.
    pub fn decode_to_utf8(
        &mut self,
        src: &[u8],
        dst: &mut [u8],
        last: bool,
    ) -> (DecoderResult, usize, usize) {
        let mut sink = ByteSink::new(dst);
        let mut read = 0usize;

        if self.bom != BomHandling::Off {
            match self.consume_bom(src, last) {
                Some(consumed) => read = consumed,
                None => return (DecoderResult::InputEmpty, src.len(), 0),
            }
        }

        // Replay bytes that were buffered for BOM sniffing but turned out to be
        // ordinary content.  They belong to the stream before anything in `src`.
        while self.prefix_pos < self.prefix_len {
            let prefix = self.prefix;
            let (start, end) = (usize::from(self.prefix_pos), usize::from(self.prefix_len));
            let prefix_is_last = last && read == src.len();
            let (result, n) = self
                .variant
                .decode(&prefix[start..end], &mut sink, prefix_is_last);
            self.prefix_pos += n as u8;
            if result != DecoderResult::InputEmpty {
                return (result, read, sink.written());
            }
        }

        let (result, n) = self.variant.decode(&src[read..], &mut sink, last);
        (result, read + n, sink.written())
    }

    /// Decodes into `dst`, writing U+FFFD for each malformed sequence.
    ///
    /// The final `bool` is true if at least one substitution was made.
    pub fn decode_to_utf8_with_replacement(
        &mut self,
        src: &[u8],
        dst: &mut [u8],
        last: bool,
    ) -> (CoderResult, usize, usize, bool) {
        let mut total_read = 0usize;
        let mut total_written = 0usize;
        let mut had_errors = false;
        loop {
            let (result, read, written) =
                self.decode_to_utf8(&src[total_read..], &mut dst[total_written..], last);
            total_read += read;
            total_written += written;
            match result {
                DecoderResult::Malformed(_) => {
                    had_errors = true;
                    let replacement = "\u{FFFD}".as_bytes();
                    if dst.len() - total_written < replacement.len() {
                        // Decoders reserve room for a substitution before reporting
                        // an error, so this is unreachable in practice.
                        return (
                            CoderResult::OutputFull,
                            total_read,
                            total_written,
                            had_errors,
                        );
                    }
                    dst[total_written..total_written + replacement.len()]
                        .copy_from_slice(replacement);
                    total_written += replacement.len();
                }
                other => {
                    return (
                        other.as_coder_result(),
                        total_read,
                        total_written,
                        had_errors,
                    );
                }
            }
        }
    }

    /// Decodes all of `src`, appending to `dst` and writing U+FFFD for each
    /// malformed sequence.  Returns true if at least one substitution was made.
    pub fn decode_to_string(&mut self, src: &[u8], dst: &mut String, last: bool) -> bool {
        let mut buffer = [0u8; 1024];
        let mut read = 0usize;
        let mut had_errors = false;
        loop {
            let (result, n, written, errors) =
                self.decode_to_utf8_with_replacement(&src[read..], &mut buffer, last);
            read += n;
            had_errors |= errors;
            dst.push_str(
                core::str::from_utf8(&buffer[..written]).expect("decoders emit valid UTF-8"),
            );
            if result == CoderResult::InputEmpty {
                return had_errors;
            }
        }
    }

    /// Decodes all of `src`, appending to `dst` and stopping at the first
    /// malformed sequence, which is reported as `Err`.
    pub fn decode_to_string_without_replacement(
        &mut self,
        src: &[u8],
        dst: &mut String,
        last: bool,
    ) -> Result<(), MalformedError> {
        let mut buffer = [0u8; 1024];
        let mut read = 0usize;
        loop {
            let (result, n, written) = self.decode_to_utf8(&src[read..], &mut buffer, last);
            read += n;
            dst.push_str(
                core::str::from_utf8(&buffer[..written]).expect("decoders emit valid UTF-8"),
            );
            match result {
                DecoderResult::InputEmpty => return Ok(()),
                DecoderResult::OutputFull => {}
                DecoderResult::Malformed(len) => {
                    return Err(MalformedError { offset: read, len });
                }
            }
        }
    }

    /// Feeds `src` to the BOM state machine.
    ///
    /// Returns how many bytes of `src` the byte order mark consumed, or `None` if
    /// the whole of `src` was buffered and a decision still needs more input.
    fn consume_bom(&mut self, src: &[u8], last: bool) -> Option<usize> {
        let mut consumed = 0usize;
        loop {
            match self.bom_decision() {
                Decision::NeedMore => {
                    if consumed == src.len() {
                        if !last {
                            return None;
                        }
                        self.buffered_bytes_are_content();
                        return Some(consumed);
                    }
                    self.bom_buf[usize::from(self.bom_len)] = src[consumed];
                    self.bom_len += 1;
                    consumed += 1;
                }
                Decision::Bom(encoding) => {
                    self.bom_len = 0;
                    self.bom = BomHandling::Off;
                    if encoding != self.encoding {
                        self.encoding = encoding;
                        self.variant = encoding.variant().new_decoder();
                    }
                    return Some(consumed);
                }
                Decision::NoBom => {
                    self.buffered_bytes_are_content();
                    return Some(consumed);
                }
            }
        }
    }

    fn bom_decision(&self) -> Decision {
        let seen = &self.bom_buf[..usize::from(self.bom_len)];
        match self.bom {
            BomHandling::Sniff => prefix_decision(&BOMS, seen),
            BomHandling::Remove => match self.encoding.bom() {
                Some(bom) => prefix_decision(&[(bom, self.encoding)], seen),
                None => Decision::NoBom,
            },
            BomHandling::Off => Decision::NoBom,
        }
    }

    fn buffered_bytes_are_content(&mut self) {
        self.prefix = self.bom_buf;
        self.prefix_pos = 0;
        self.prefix_len = self.bom_len;
        self.bom_len = 0;
        self.bom = BomHandling::Off;
    }
}

fn prefix_decision(candidates: &[(&[u8], &'static Encoding)], seen: &[u8]) -> Decision {
    let mut partial = false;
    for &(bom, encoding) in candidates {
        if seen.len() >= bom.len() {
            if &seen[..bom.len()] == bom {
                return Decision::Bom(encoding);
            }
        } else if bom.starts_with(seen) {
            partial = true;
        }
    }
    if partial {
        Decision::NeedMore
    } else {
        Decision::NoBom
    }
}

/// A malformed byte sequence found by a decode that does not substitute errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MalformedError {
    /// How far into the buffer passed to this call decoding got.
    pub offset: usize,
    /// How many bytes the malformed sequence spans.  A sequence that began in an
    /// earlier call can be longer than `offset`.
    pub len: u8,
}

impl core::fmt::Display for MalformedError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "malformed byte sequence of {} byte(s) ending at offset {}",
            self.len, self.offset
        )
    }
}

#[cfg(feature = "std")]
impl std::error::Error for MalformedError {}
