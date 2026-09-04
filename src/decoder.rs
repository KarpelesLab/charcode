//! The streaming decoder.

#[cfg(feature = "alloc")]
use alloc::string::String;

use crate::Encoding;
use crate::encodings::{UTF_8, UTF_16BE, UTF_16LE};
use crate::options::{Bom, DecodeOptions, Malformed, Tally};
use crate::result::DecoderResult;
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
/// # #[cfg(all(feature = "alloc", feature = "whatwg"))]
/// # fn main() {
/// use charcode::SHIFT_JIS;
///
/// let mut decoder = SHIFT_JIS.new_decoder();
/// let mut text = String::new();
/// // The two bytes of this character arrive in separate chunks.
/// decoder.decode_to_string(&[0x93], &mut text, false);
/// decoder.decode_to_string(&[0xFA], &mut text, true);
/// assert_eq!(text, "\u{65E5}");
/// # }
/// # #[cfg(not(all(feature = "alloc", feature = "whatwg")))]
/// # fn main() {}
/// ```
#[derive(Debug, Clone)]
pub struct Decoder {
    encoding: &'static Encoding,
    variant: VariantDecoder,
    options: DecodeOptions,
    /// Cleared once the byte order mark question is settled.
    sniffing: bool,
    /// Bytes held back while deciding whether they start a BOM.
    bom_buf: [u8; 3],
    bom_len: u8,
    /// Bytes that turned out not to be a BOM and still have to be decoded.
    prefix: [u8; 3],
    prefix_pos: u8,
    prefix_len: u8,
    /// A replacement character not yet written in full.
    pending: [u8; 4],
    pending_pos: u8,
    pending_len: u8,
    tally: Tally,
}

impl Decoder {
    pub(crate) fn new(encoding: &'static Encoding, options: DecodeOptions) -> Decoder {
        let sniffing = match options.bom {
            Bom::Sniff => true,
            Bom::Remove => encoding.bom().is_some(),
            Bom::Ignore => false,
        };
        Decoder {
            encoding,
            variant: encoding.variant().new_decoder(),
            options,
            sniffing,
            bom_buf: [0; 3],
            bom_len: 0,
            prefix: [0; 3],
            prefix_pos: 0,
            prefix_len: 0,
            pending: [0; 4],
            pending_pos: 0,
            pending_len: 0,
            tally: Tally::default(),
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

    /// Decodes into `dst`.
    ///
    /// Returns why it stopped, how many bytes of `src` were consumed and how
    /// many bytes were written.  The written bytes are always valid UTF-8.
    /// `dst` must be at least [`DECODER_MIN_BUFFER`] bytes long or no progress
    /// is possible.
    ///
    /// `Malformed` comes back only under [`Malformed::Fail`]; every other
    /// policy handles the sequence and keeps going.
    ///
    /// Pass `last = true` only with the final buffer of the stream.
    pub fn decode_to_utf8(
        &mut self,
        src: &[u8],
        dst: &mut [u8],
        last: bool,
    ) -> (DecoderResult, usize, usize) {
        let mut total_read = 0usize;
        let mut total_written = 0usize;
        loop {
            {
                let mut sink = ByteSink::new(&mut dst[total_written..]);
                if !self.drain_pending(&mut sink) {
                    return (
                        DecoderResult::OutputFull,
                        total_read,
                        total_written + sink.written(),
                    );
                }
                total_written += sink.written();
            }
            let (result, read, written) =
                self.decode_step(&src[total_read..], &mut dst[total_written..], last);
            total_read += read;
            total_written += written;
            match result {
                DecoderResult::Malformed(len) => match self.options.malformed {
                    Malformed::Fail => {
                        return (DecoderResult::Malformed(len), total_read, total_written);
                    }
                    Malformed::Omit => {
                        self.tally.errors += 1;
                    }
                    Malformed::Replace(c) => {
                        self.tally.errors += 1;
                        let mut buf = [0u8; 4];
                        let encoded = c.encode_utf8(&mut buf).as_bytes();
                        self.pending[..encoded.len()].copy_from_slice(encoded);
                        self.pending_pos = 0;
                        self.pending_len = encoded.len() as u8;
                    }
                },
                other => return (other, total_read, total_written),
            }
        }
    }

    /// How much has been substituted or dropped so far.
    pub fn tally(&self) -> Tally {
        self.tally
    }

    /// Writes as much of a carried-over replacement character as fits.
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

    /// Decodes all of `src`, appending to `dst`.
    #[cfg(feature = "alloc")]
    pub fn decode_to_string(
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

    /// One pass over the variant decoder, after byte order mark handling.
    fn decode_step(
        &mut self,
        src: &[u8],
        dst: &mut [u8],
        last: bool,
    ) -> (DecoderResult, usize, usize) {
        let mut sink = ByteSink::new(dst);
        let mut read = 0usize;

        if self.sniffing {
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
                    self.sniffing = false;
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
        match self.options.bom {
            Bom::Sniff => prefix_decision(&BOMS, seen),
            Bom::Remove => match self.encoding.bom() {
                Some(bom) => prefix_decision(&[(bom, self.encoding)], seen),
                None => Decision::NoBom,
            },
            Bom::Ignore => Decision::NoBom,
        }
    }

    fn buffered_bytes_are_content(&mut self) {
        self.prefix = self.bom_buf;
        self.prefix_pos = 0;
        self.prefix_len = self.bom_len;
        self.bom_len = 0;
        self.sniffing = false;
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
