//! The streaming encoder.

#[cfg(feature = "alloc")]
use alloc::vec::Vec;
use core::fmt;

use crate::Encoding;
use crate::options::{EncodeOptions, Tally, Unmappable};
use crate::result::EncoderResult;
use crate::sink::ByteSink;
use crate::variant::VariantEncoder;

/// The smallest output buffer [`Encoder::encode_from_utf8`] can make progress with.
pub const ENCODER_MIN_BUFFER: usize = 4;

/// Room for the longest escape any policy writes, after the target encoding has
/// had its way with it: `😀` is twelve characters, and ISO-2022-JP can
/// prefix three more bytes of escape sequence.
const STAGING: usize = 16;

/// A streaming encoder for one text stream.
///
/// Created by [`Encoding::new_encoder`] or [`Encoding::new_encoder_with`].
/// Because the standard has no encoder for `replacement`, UTF-16BE or
/// UTF-16LE, asking those encodings for one yields a UTF-8 encoder;
/// [`Encoder::encoding`] reports what is actually being written.
///
/// # Examples
///
/// ```
/// # #[cfg(all(feature = "alloc", feature = "single-byte"))]
/// # fn main() {
/// use charcode::{EncodeOptions, Unmappable, WINDOWS_1252};
///
/// let mut encoder = WINDOWS_1252
///     .new_encoder_with(EncodeOptions::new().unmappable(Unmappable::Replace('?')));
/// let mut bytes = Vec::new();
/// encoder.encode_from_str(" \u{20AC} \u{4E00}", &mut bytes, true).unwrap();
/// // The euro sign is in windows-1252; the ideograph is not.
/// assert_eq!(bytes, b" \x80 ?");
/// assert_eq!(encoder.tally().errors, 1);
/// # }
/// # #[cfg(not(all(feature = "alloc", feature = "single-byte")))]
/// # fn main() {}
/// ```
#[derive(Debug, Clone)]
pub struct Encoder {
    encoding: &'static Encoding,
    variant: VariantEncoder,
    options: EncodeOptions,
    /// An escape that did not fit in the previous output buffer, carried over
    /// so that any buffer size works.
    pending: [u8; STAGING],
    pending_pos: u8,
    pending_len: u8,
    /// Cleared only for the standard's form-submission hook, which leaves `&`
    /// alone.
    escape_ambiguous: bool,
    tally: Tally,
}

impl Encoder {
    pub(crate) fn new(encoding: &'static Encoding, options: EncodeOptions) -> Encoder {
        Encoder {
            encoding,
            variant: encoding.variant().new_encoder(),
            options,
            pending: [0; STAGING],
            pending_pos: 0,
            pending_len: 0,
            escape_ambiguous: true,
            tally: Tally::default(),
        }
    }

    /// The standard's `encode` hook: numeric character references, and no `&`
    /// escaping.  Private because only [`Encoding::encode_html_form`] should
    /// reach it.
    #[cfg(feature = "alloc")]
    pub(crate) fn new_html_form(encoding: &'static Encoding) -> Encoder {
        let mut encoder = Encoder::new(encoding, EncodeOptions::new().unmappable(Unmappable::Html));
        encoder.escape_ambiguous = false;
        encoder
    }

    /// The encoding being written, which is always an output encoding.
    pub fn encoding(&self) -> &'static Encoding {
        self.encoding
    }

    /// How many characters have been substituted or dropped so far.
    pub fn tally(&self) -> Tally {
        self.tally
    }

    /// An upper bound on the bytes that encoding `byte_length` more UTF-8 bytes
    /// can produce.
    pub fn max_buffer_length_from_utf8(&self, byte_length: usize) -> Option<usize> {
        match self.options.unmappable {
            Unmappable::Fail | Unmappable::Omit | Unmappable::Replace(_) => self
                .variant
                .max_buffer_length_from_utf8_without_replacement(byte_length),
            Unmappable::Html | Unmappable::JsonEscape => {
                self.variant.max_buffer_length_from_utf8(byte_length)
            }
        }
    }

    /// The character whose presence in representable text this policy has to
    /// escape, so that the escapes it writes stay unambiguous.
    fn ambiguous(&self) -> Option<char> {
        if !self.escape_ambiguous {
            return None;
        }
        match self.options.unmappable {
            Unmappable::Html => Some('&'),
            Unmappable::JsonEscape => Some('\\'),
            _ => None,
        }
    }

    /// Encodes into `dst`.
    ///
    /// Returns why it stopped, how many bytes of `src` were consumed (always on
    /// a character boundary) and how many bytes were written.  `dst` must be at
    /// least [`ENCODER_MIN_BUFFER`] bytes long or no progress is possible.
    ///
    /// `Unmappable` comes back only under [`Unmappable::Fail`], or when the
    /// policy's own replacement turns out to be unrepresentable; every other
    /// case is handled and encoding continues.
    pub fn encode_from_utf8(
        &mut self,
        src: &str,
        dst: &mut [u8],
        last: bool,
    ) -> (EncoderResult, usize, usize) {
        let mut sink = ByteSink::new(dst);
        let mut read = 0usize;
        loop {
            if !self.drain_pending(&mut sink) {
                return (EncoderResult::OutputFull, read, sink.written());
            }
            let rest = &src[read..];

            // Hand the variant encoder everything up to the next character this
            // policy has to escape, so that one is dealt with here.
            let chunk = match self.ambiguous() {
                Some(ambiguous) if rest.starts_with(ambiguous) => {
                    let escaped = if ambiguous == '&' { "&amp;" } else { "\\\\" };
                    if let Err(bad) = self.stage(escaped) {
                        return (EncoderResult::Unmappable(bad), read, sink.written());
                    }
                    read += ambiguous.len_utf8();
                    continue;
                }
                Some(ambiguous) => rest.find(ambiguous).unwrap_or(rest.len()),
                None => rest.len(),
            };
            let is_final = chunk == rest.len();

            let (result, n) = self
                .variant
                .encode(&rest[..chunk], &mut sink, last && is_final);
            read += n;
            match result {
                EncoderResult::InputEmpty if is_final => {
                    return (EncoderResult::InputEmpty, read, sink.written());
                }
                EncoderResult::InputEmpty => continue,
                EncoderResult::OutputFull => {
                    return (EncoderResult::OutputFull, read, sink.written());
                }
                EncoderResult::Unmappable(c) => match self.handle(c) {
                    Ok(()) => continue,
                    Err(bad) => return (EncoderResult::Unmappable(bad), read, sink.written()),
                },
            }
        }
    }

    /// Applies the policy to a character the encoding cannot represent.
    ///
    /// `Ok` means it was dealt with — dropped, or staged as an escape — and
    /// encoding should carry on.
    fn handle(&mut self, c: char) -> Result<(), char> {
        if self.options.unmappable == Unmappable::Fail && !self.will_transliterate(c) {
            return Err(c);
        }
        self.tally.errors += 1;

        #[cfg(feature = "translit")]
        if self.options.transliterate {
            if let Some(text) = crate::translit::ascii_fold(c) {
                return self.stage(text);
            }
            if self.options.unmappable == Unmappable::Fail {
                self.tally.errors -= 1;
                return Err(c);
            }
        }

        let mut buf = [0u8; STAGING];
        match self.options.unmappable {
            Unmappable::Fail => Err(c),
            Unmappable::Omit => Ok(()),
            Unmappable::Replace(replacement) => {
                let mut small = [0u8; 4];
                self.stage(replacement.encode_utf8(&mut small))
            }
            Unmappable::Html => self.stage(numeric_reference(c, &mut buf)),
            Unmappable::JsonEscape => self.stage(json_escape(c, &mut buf)),
        }
    }

    /// Whether transliteration will handle `c`, so that `Fail` should not fire.
    fn will_transliterate(&self, c: char) -> bool {
        #[cfg(feature = "translit")]
        {
            self.options.transliterate && crate::translit::ascii_fold(c).is_some()
        }
        #[cfg(not(feature = "translit"))]
        {
            let _ = c;
            false
        }
    }

    /// Encodes a short escape through the target encoding and holds the bytes
    /// until they fit in the caller's buffer.
    fn stage(&mut self, text: &str) -> Result<(), char> {
        debug_assert_eq!(self.pending_len, 0);
        let mut buf = [0u8; STAGING];
        let mut sink = ByteSink::new(&mut buf);
        let (result, _) = self.variant.encode(text, &mut sink, false);
        if let EncoderResult::Unmappable(c) = result {
            return Err(c);
        }
        let written = sink.written();
        self.pending[..written].copy_from_slice(&buf[..written]);
        self.pending_pos = 0;
        self.pending_len = written as u8;
        Ok(())
    }

    /// Writes as much of the carried-over escape as fits.  Returns false if
    /// some of it is still pending.
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

    /// Encodes all of `src`, appending to `dst`.
    #[cfg(feature = "alloc")]
    pub fn encode_from_str(
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
}

/// `&#19968;`
fn numeric_reference(c: char, buf: &mut [u8; STAGING]) -> &str {
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
    buf[0] = b'&';
    buf[1] = b'#';
    let mut len = 2;
    for i in (0..n).rev() {
        buf[len] = digits[i];
        len += 1;
    }
    buf[len] = b';';
    len += 1;
    core::str::from_utf8(&buf[..len]).unwrap_or("&#65533;")
}

/// `一`, or a surrogate pair above the basic multilingual plane.
fn json_escape(c: char, buf: &mut [u8; STAGING]) -> &str {
    fn unit(buf: &mut [u8], at: usize, unit: u16) {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        buf[at] = b'\\';
        buf[at + 1] = b'u';
        for i in 0..4 {
            buf[at + 2 + i] = HEX[((unit >> (12 - 4 * i)) & 0xF) as usize];
        }
    }
    let mut units = [0u16; 2];
    let units = c.encode_utf16(&mut units);
    let mut len = 0;
    for &u in units.iter() {
        unit(buf, len, u);
        len += 6;
    }
    core::str::from_utf8(&buf[..len]).unwrap_or("\\ufffd")
}

/// A character that the target encoding cannot represent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnmappableError {
    /// The character that could not be encoded.
    pub character: char,
    /// How far into the string passed to this call encoding got, counted in
    /// UTF-8 bytes and including the unmappable character.
    pub offset: usize,
}

impl fmt::Display for UnmappableError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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
