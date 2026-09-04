//! Character encoding conversion, implementing the [WHATWG Encoding Standard].
//!
//! `charcode` converts between UTF-8 and the encodings the web actually uses:
//! UTF-16, the 28 legacy single-byte encodings, and the legacy Chinese, Japanese
//! and Korean multi-byte encodings.  It has no dependencies outside the standard
//! library, contains no `unsafe` code, and works on `no_std` targets that have an
//! allocator.
//!
//! # Converting a whole buffer
//!
//! [`Encoding::decode`] is the usual entry point.  It sniffs for a byte order
//! mark, falls back to the encoding it was called on, and substitutes U+FFFD for
//! malformed sequences the way a browser does:
//!
//! ```
//! # #[cfg(feature = "alloc")]
//! # fn main() {
//! use charcode::{Encoding, WINDOWS_1252};
//!
//! let (text, encoding, had_errors) = WINDOWS_1252.decode(b"caf\xE9");
//! assert_eq!(text, "caf\u{E9}");
//! assert_eq!(encoding, WINDOWS_1252);
//! assert!(!had_errors);
//!
//! // A byte order mark wins over the encoding you name.
//! let (text, encoding, _) = WINDOWS_1252.decode(b"\xEF\xBB\xBFcaf\xC3\xA9");
//! assert_eq!(text, "caf\u{E9}");
//! assert_eq!(encoding.name(), "UTF-8");
//! # }
//! # #[cfg(not(feature = "alloc"))]
//! # fn main() {}
//! ```
//!
//! Encodings are looked up by any of the labels the standard defines, which is
//! what a `Content-Type` header or an HTML `<meta charset>` carries:
//!
//! ```
//! use charcode::Encoding;
//!
//! assert_eq!(Encoding::for_label(b"latin1").unwrap().name(), "windows-1252");
//! assert_eq!(Encoding::for_label(b"  Shift-JIS ").unwrap().name(), "Shift_JIS");
//! assert!(Encoding::for_label(b"not-an-encoding").is_none());
//! ```
//!
//! Encoding goes the other way.  Characters the target cannot represent become
//! HTML numeric character references, matching form submission:
//!
//! ```
//! # #[cfg(feature = "alloc")]
//! # fn main() {
//! use charcode::EUC_KR;
//!
//! let (bytes, encoding, had_unmappable) = EUC_KR.encode("\u{D55C}\u{1F600}");
//! assert_eq!(&bytes[..], b"\xC7\xD1&#128512;");
//! assert_eq!(encoding, EUC_KR);
//! assert!(had_unmappable);
//! # }
//! # #[cfg(not(feature = "alloc"))]
//! # fn main() {}
//! ```
//!
//! Both return a [`Cow`], borrowed when the input is already the answer, so
//! passing ASCII through an ASCII-compatible encoding costs nothing but the scan.
//!
//! # Converting a stream
//!
//! For input that arrives in pieces, [`Encoding::new_decoder`] and
//! [`Encoding::new_encoder`] give state machines that carry a partial sequence
//! from one buffer to the next.  Pass `last = true` with the final buffer so that
//! a truncated sequence is reported rather than dropped:
//!
//! ```
//! # #[cfg(feature = "alloc")]
//! # fn main() {
//! use charcode::BIG5;
//!
//! let mut decoder = BIG5.new_decoder();
//! let mut text = String::new();
//! decoder.decode_to_string(&[0xA4], &mut text, false);
//! decoder.decode_to_string(&[0x40], &mut text, true);
//! assert_eq!(text, "\u{4E00}");
//! # }
//! # #[cfg(not(feature = "alloc"))]
//! # fn main() {}
//! ```
//!
//! [`Decoder::decode_to_utf8`] and [`Encoder::encode_from_utf8`] are the
//! allocation-free forms, writing into a caller-provided `&mut [u8]`.
//!
//! # Errors
//!
//! Every conversion comes in two flavours.  The default substitutes errors, as
//! the standard requires of web content: U+FFFD when decoding, a numeric
//! character reference when encoding.  The `without_replacement` forms instead
//! stop and report, for callers that need to reject bad input.
//!
//! # Features
//!
//! - `std` (default): implements [`std::error::Error`] for the error types.
//!   Implies `alloc`.
//! - `alloc` (default, through `std`): the conveniences that hand back an owned
//!   `String`, `Vec` or `Cow` — [`Encoding::decode`] and [`Encoding::encode`],
//!   [`Decoder::decode_to_string`], [`Encoder::encode_from_str`] and their
//!   variants.  Without it the crate never allocates: what remains is encoding
//!   lookup plus the streaming API, which converts into buffers the caller
//!   provides.
//! - `serde`: serializes an encoding as its name.  Needs neither `std` nor
//!   `alloc`.
//! - `cli`: builds the `charcode` command-line tool.  Needs `std`.
//!
//! [WHATWG Encoding Standard]: https://encoding.spec.whatwg.org/

#![no_std]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

#[cfg(feature = "alloc")]
extern crate alloc;
#[cfg(feature = "std")]
extern crate std;

/// The examples in the README are compiled and run as part of the test suite.
///
/// They use the owned-output API, so they are only checked when `alloc` is on;
/// hidden `cfg` lines are not an option here because GitHub renders them.
#[cfg(all(doctest, feature = "alloc"))]
#[doc = include_str!("../README.md")]
struct Readme;

mod ascii;
mod big5;
mod code_page;
mod decoder;
mod encoder;
mod euc_jp;
mod euc_kr;
mod gb18030;
mod index;
mod iso_2022_jp;
mod replacement;
mod result;
mod shift_jis;
mod single_byte;
mod sink;
mod tables;
#[cfg(test)]
mod tests;
mod utf_16;
mod utf_8;
mod variant;
mod x_user_defined;

mod encodings;

#[cfg(feature = "serde")]
mod serde_impl;

#[cfg(feature = "alloc")]
use alloc::{borrow::Cow, string::String, vec::Vec};

pub use crate::decoder::{DECODER_MIN_BUFFER, Decoder, MalformedError};
pub use crate::encoder::{ENCODER_MIN_BUFFER, Encoder, UnmappableError};
pub use crate::encodings::*;
pub use crate::result::{CoderResult, DecoderResult, EncoderResult};

use crate::code_page::CODE_PAGES;
use crate::tables::labels::{ALL_ENCODINGS, LABELS};
use crate::variant::VariantEncoding;

/// The longest label in the standard is 19 bytes.
const MAX_LABEL_LEN: usize = 24;

/// A character encoding.
///
/// Encodings are static and unique: there is exactly one instance per encoding in
/// the standard, reachable as a constant such as [`UTF_8`] or by label through
/// [`Encoding::for_label`].  Comparing two `&'static Encoding` values compares
/// which encoding they are.
pub struct Encoding {
    name: &'static str,
    variant: VariantEncoding,
}

impl Encoding {
    pub(crate) const fn new(name: &'static str, variant: VariantEncoding) -> Encoding {
        Encoding { name, variant }
    }

    pub(crate) fn variant(&self) -> VariantEncoding {
        self.variant
    }

    /// The byte order mark that names this encoding, if it has one.
    pub(crate) fn bom(&self) -> Option<&'static [u8]> {
        match self.variant {
            VariantEncoding::Utf8 => Some(&[0xEF, 0xBB, 0xBF]),
            VariantEncoding::Utf16Be => Some(&[0xFE, 0xFF]),
            VariantEncoding::Utf16Le => Some(&[0xFF, 0xFE]),
            _ => None,
        }
    }

    /// The encoding's name, in the exact capitalization the standard uses.
    ///
    /// Names are stable identifiers: `"UTF-8"`, `"windows-1252"`, `"Shift_JIS"`.
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// Every encoding in the standard, in specification order.
    pub fn all() -> &'static [&'static Encoding] {
        &ALL_ENCODINGS
    }

    /// Every label the standard defines for this encoding, in sorted order.
    ///
    /// The name itself is always among them, ASCII-lowercased.
    ///
    /// ```
    /// # use charcode::IBM866;
    /// let labels: Vec<_> = IBM866.labels().collect();
    /// assert_eq!(labels, ["866", "cp866", "csibm866", "ibm866"]);
    /// ```
    pub fn labels(&'static self) -> impl Iterator<Item = &'static str> {
        LABELS
            .iter()
            .filter(move |(_, encoding)| *encoding == self)
            .map(|(label, _)| *label)
    }

    /// Looks up an encoding by label, implementing `get an encoding`.
    ///
    /// Leading and trailing ASCII whitespace is ignored and the comparison is
    /// ASCII case-insensitive, so a `Content-Type` charset parameter or a
    /// `<meta charset>` value can be passed through unchanged.
    ///
    /// ```
    /// # use charcode::{Encoding, UTF_8};
    /// assert_eq!(Encoding::for_label(b"utf8"), Some(UTF_8));
    /// assert_eq!(Encoding::for_label(b"\tUTF-8\n"), Some(UTF_8));
    /// ```
    pub fn for_label(label: &[u8]) -> Option<&'static Encoding> {
        let trimmed = trim_ascii_whitespace(label);
        if trimmed.is_empty() || trimmed.len() > MAX_LABEL_LEN {
            return None;
        }
        let mut lowercased = [0u8; MAX_LABEL_LEN];
        for (slot, byte) in lowercased.iter_mut().zip(trimmed) {
            *slot = byte.to_ascii_lowercase();
        }
        let needle = &lowercased[..trimmed.len()];
        LABELS
            .binary_search_by(|(label, _)| label.as_bytes().cmp(needle))
            .ok()
            .map(|i| LABELS[i].1)
    }

    /// Like [`Encoding::for_label`], but treats the labels that map to
    /// [`REPLACEMENT`] as unknown.
    ///
    /// This is what a caller that wants to *use* the encoding rather than defend
    /// against it should call: `replacement` exists only to neutralize labels for
    /// encodings that are unsafe to support.
    pub fn for_label_no_replacement(label: &[u8]) -> Option<&'static Encoding> {
        match Encoding::for_label(label) {
            Some(encoding) if encoding.variant == VariantEncoding::Replacement => None,
            other => other,
        }
    }

    /// Looks up an encoding by Windows code page identifier.
    ///
    /// Code page numbers come from Microsoft's registry rather than from the
    /// Encoding Standard, and are what `GetACP`, a .NET `Encoding.CodePage` or
    /// an old database column reports.
    ///
    /// A number for an encoding the standard folds into a superset resolves to
    /// that superset, exactly as the equivalent label does: 28591 (ISO-8859-1)
    /// gives [`WINDOWS_1252`], and 20127 (US-ASCII) does too.
    ///
    /// ```
    /// # use charcode::{Encoding, BIG5, SHIFT_JIS, WINDOWS_1252};
    /// assert_eq!(Encoding::for_windows_code_page(1252), Some(WINDOWS_1252));
    /// assert_eq!(Encoding::for_windows_code_page(932), Some(SHIFT_JIS));
    /// assert_eq!(Encoding::for_windows_code_page(950), Some(BIG5));
    /// // Not an encoding this crate has.
    /// assert_eq!(Encoding::for_windows_code_page(437), None);
    /// ```
    pub fn for_windows_code_page(code_page: u32) -> Option<&'static Encoding> {
        CODE_PAGES
            .binary_search_by_key(&code_page, |entry| entry.number)
            .ok()
            .map(|i| CODE_PAGES[i].encoding)
    }

    /// Looks up an encoding by code page number written the `cpNNN` way.
    ///
    /// `cp932` and `windows-932` name the same entry in the same registry, so
    /// this reads the same table as [`Encoding::for_windows_code_page`]; it
    /// exists so that a call site spelled after `cp1252` or `cp866` does not
    /// have to say "windows" to mean IBM's or DOS's numbering.
    ///
    /// ```
    /// # use charcode::{Encoding, IBM866, SHIFT_JIS};
    /// assert_eq!(Encoding::for_cp(932), Some(SHIFT_JIS));
    /// assert_eq!(Encoding::for_cp(866), Some(IBM866));
    /// assert_eq!(Encoding::for_cp(437), None);
    /// ```
    pub fn for_cp(code_page: u32) -> Option<&'static Encoding> {
        Encoding::for_windows_code_page(code_page)
    }

    /// Like [`Encoding::for_windows_code_page`] and [`Encoding::for_cp`], but
    /// treats the numbers that map to [`REPLACEMENT`] — 50225, 50227, 50229
    /// and 52936 — as unknown.
    pub fn for_windows_code_page_no_replacement(code_page: u32) -> Option<&'static Encoding> {
        match Encoding::for_windows_code_page(code_page) {
            Some(encoding) if encoding.variant == VariantEncoding::Replacement => None,
            other => other,
        }
    }

    /// The Windows code page identifier for this encoding, if it has one.
    ///
    /// An encoding reachable through several numbers reports the one Microsoft
    /// treats as its own, so this is the inverse of
    /// [`Encoding::for_windows_code_page`] only up to those aliases.
    ///
    /// ```
    /// # use charcode::{Encoding, SHIFT_JIS, WINDOWS_1252, X_USER_DEFINED};
    /// assert_eq!(WINDOWS_1252.windows_code_page(), Some(1252));
    /// assert_eq!(SHIFT_JIS.windows_code_page(), Some(932));
    /// // 28591 also resolves to windows-1252, but 1252 is its own number.
    /// assert_eq!(Encoding::for_windows_code_page(28591), Some(WINDOWS_1252));
    /// // Microsoft has no number for this one.
    /// assert_eq!(X_USER_DEFINED.windows_code_page(), None);
    /// ```
    pub fn windows_code_page(&self) -> Option<u32> {
        CODE_PAGES
            .iter()
            .find(|entry| entry.canonical && entry.encoding == self)
            .map(|entry| entry.number)
    }

    /// Implements `BOM sniff`: if `buffer` starts with a byte order mark, returns
    /// the encoding it names and the mark's length in bytes.
    ///
    /// ```
    /// # use charcode::{Encoding, UTF_16LE};
    /// assert_eq!(Encoding::for_bom(b"\xFF\xFEa\0"), Some((UTF_16LE, 2)));
    /// assert_eq!(Encoding::for_bom(b"plain"), None);
    /// ```
    pub fn for_bom(buffer: &[u8]) -> Option<(&'static Encoding, usize)> {
        if buffer.starts_with(&[0xEF, 0xBB, 0xBF]) {
            Some((UTF_8, 3))
        } else if buffer.starts_with(&[0xFE, 0xFF]) {
            Some((UTF_16BE, 2))
        } else if buffer.starts_with(&[0xFF, 0xFE]) {
            Some((UTF_16LE, 2))
        } else {
            None
        }
    }

    /// Implements `get an output encoding`: the encoding to use when encoding
    /// *to* this one.
    ///
    /// `replacement`, UTF-16BE and UTF-16LE have no encoder, and map to UTF-8.
    /// Every other encoding maps to itself.
    pub fn output_encoding(&'static self) -> &'static Encoding {
        match self.variant {
            VariantEncoding::Replacement | VariantEncoding::Utf16Be | VariantEncoding::Utf16Le => {
                UTF_8
            }
            _ => self,
        }
    }

    /// Whether this encoding maps every byte to at most one character.
    pub fn is_single_byte(&self) -> bool {
        self.variant.is_single_byte()
    }

    /// Whether bytes below 0x80 always stand for the corresponding ASCII
    /// characters.
    ///
    /// False for UTF-16BE/LE, for `replacement`, and for ISO-2022-JP, whose
    /// escape sequences change what an ASCII byte means.
    pub fn is_ascii_compatible(&self) -> bool {
        self.variant.is_ascii_compatible()
    }

    /// A decoder that sniffs for a byte order mark.
    ///
    /// If the stream starts with one, the decoder switches to the encoding the
    /// mark names and [`Decoder::encoding`] reports the change.
    pub fn new_decoder(&'static self) -> Decoder {
        Decoder::new(self, true, false)
    }

    /// A decoder that strips this encoding's own byte order mark, if present, and
    /// ignores any other.
    pub fn new_decoder_with_bom_removal(&'static self) -> Decoder {
        Decoder::new(self, false, true)
    }

    /// A decoder that treats a byte order mark as ordinary content.
    pub fn new_decoder_without_bom_handling(&'static self) -> Decoder {
        Decoder::new(self, false, false)
    }

    /// An encoder for this encoding's [output encoding](Encoding::output_encoding).
    pub fn new_encoder(&'static self) -> Encoder {
        Encoder::new(self.output_encoding())
    }

    #[cfg(feature = "alloc")]
    /// Decodes `bytes`, honouring a leading byte order mark.
    ///
    /// Implements the standard's `decode` hook: a byte order mark takes priority
    /// over `self`, malformed sequences become U+FFFD, and the returned flag says
    /// whether any did.  The second element is the encoding actually used.
    pub fn decode<'a>(&'static self, bytes: &'a [u8]) -> (Cow<'a, str>, &'static Encoding, bool) {
        let (encoding, bom_len) = Encoding::for_bom(bytes).unwrap_or((self, 0));
        let (text, had_errors) = encoding.decode_without_bom_handling(&bytes[bom_len..]);
        (text, encoding, had_errors)
    }

    #[cfg(feature = "alloc")]
    /// Decodes `bytes`, first removing this encoding's own byte order mark if it
    /// is there.  A mark belonging to another encoding is decoded as content.
    pub fn decode_with_bom_removal<'a>(&'static self, bytes: &'a [u8]) -> (Cow<'a, str>, bool) {
        let bom_len = match self.bom() {
            Some(bom) if bytes.starts_with(bom) => bom.len(),
            _ => 0,
        };
        self.decode_without_bom_handling(&bytes[bom_len..])
    }

    #[cfg(feature = "alloc")]
    /// Decodes `bytes`, treating a byte order mark as ordinary content.
    pub fn decode_without_bom_handling<'a>(&'static self, bytes: &'a [u8]) -> (Cow<'a, str>, bool) {
        if let Some(borrowed) = self.borrow_as_str(bytes) {
            return (Cow::Borrowed(borrowed), false);
        }
        let mut text = String::with_capacity(bytes.len());
        let had_errors = self
            .new_decoder_without_bom_handling()
            .decode_to_string(bytes, &mut text, true);
        (Cow::Owned(text), had_errors)
    }

    #[cfg(feature = "alloc")]
    /// Decodes `bytes` and fails, returning `None`, on the first malformed
    /// sequence.  A byte order mark is treated as ordinary content.
    pub fn decode_without_bom_handling_and_without_replacement<'a>(
        &'static self,
        bytes: &'a [u8],
    ) -> Option<Cow<'a, str>> {
        if let Some(borrowed) = self.borrow_as_str(bytes) {
            return Some(Cow::Borrowed(borrowed));
        }
        let mut text = String::with_capacity(bytes.len());
        self.new_decoder_without_bom_handling()
            .decode_to_string_without_replacement(bytes, &mut text, true)
            .ok()?;
        Some(Cow::Owned(text))
    }

    #[cfg(feature = "alloc")]
    /// Encodes `string` into this encoding's
    /// [output encoding](Encoding::output_encoding).
    ///
    /// Characters the encoding cannot represent become decimal numeric character
    /// references, as in HTML form submission, and the returned flag says whether
    /// any did.
    pub fn encode<'a>(&'static self, string: &'a str) -> (Cow<'a, [u8]>, &'static Encoding, bool) {
        let output = self.output_encoding();
        if output.borrow_as_str(string.as_bytes()).is_some() {
            return (Cow::Borrowed(string.as_bytes()), output, false);
        }
        let mut bytes = Vec::with_capacity(string.len());
        let had_unmappable = output
            .new_encoder()
            .encode_from_str(string, &mut bytes, true);
        (Cow::Owned(bytes), output, had_unmappable)
    }

    #[cfg(feature = "alloc")]
    /// Returns `bytes` as a `&str` when this encoding decodes them to exactly
    /// themselves, which is what makes the borrowing `Cow` cases possible.
    fn borrow_as_str<'a>(&self, bytes: &'a [u8]) -> Option<&'a str> {
        let decodes_verbatim = match self.variant {
            VariantEncoding::Utf8 => true,
            _ if self.variant.is_ascii_compatible() => ascii::is_ascii(bytes),
            _ => false,
        };
        if decodes_verbatim {
            core::str::from_utf8(bytes).ok()
        } else {
            None
        }
    }
}

/// Removes leading and trailing ASCII whitespace, as the standard defines it.
fn trim_ascii_whitespace(label: &[u8]) -> &[u8] {
    let is_space = |b: u8| matches!(b, 0x09 | 0x0A | 0x0C | 0x0D | 0x20);
    let start = label.iter().position(|&b| !is_space(b));
    match start {
        None => &[],
        Some(start) => {
            let end = label.iter().rposition(|&b| !is_space(b)).unwrap_or(start);
            &label[start..=end]
        }
    }
}

impl PartialEq for Encoding {
    fn eq(&self, other: &Encoding) -> bool {
        // Names are unique across the standard, and every encoding exists once.
        core::ptr::eq(self, other) || self.name == other.name
    }
}

impl Eq for Encoding {}

impl core::hash::Hash for Encoding {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}

impl core::fmt::Debug for Encoding {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Encoding({})", self.name)
    }
}

impl core::fmt::Display for Encoding {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.name)
    }
}
