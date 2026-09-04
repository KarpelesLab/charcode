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
//! # #[cfg(all(feature = "alloc", feature = "whatwg"))]
//! # fn main() {
//! use charcode::{Encoding, WINDOWS_1252};
//!
//! let (text, encoding, tally) = WINDOWS_1252.decode(b"caf\xE9");
//! assert_eq!(text, "caf\u{E9}");
//! assert_eq!(encoding, WINDOWS_1252);
//! assert!(tally.is_lossless());
//!
//! // A byte order mark wins over the encoding you name.
//! let (text, encoding, _) = WINDOWS_1252.decode(b"\xEF\xBB\xBFcaf\xC3\xA9");
//! assert_eq!(text, "caf\u{E9}");
//! assert_eq!(encoding.name(), "UTF-8");
//! # }
//! # #[cfg(not(all(feature = "alloc", feature = "whatwg")))]
//! # fn main() {}
//! ```
//!
//! Encodings are looked up by label.  For anything that came off the network —
//! a `Content-Type` charset parameter, an HTML `<meta charset>` — use
//! [`Encoding::for_whatwg_label`], which implements the standard's
//! `get an encoding` and answers only with encodings the standard sanctions:
//!
//! ```
//! # #[cfg(feature = "whatwg")]
//! # fn main() {
//! use charcode::Encoding;
//!
//! assert_eq!(Encoding::for_whatwg_label(b"latin1").unwrap().name(), "windows-1252");
//! assert_eq!(Encoding::for_whatwg_label(b"  Shift-JIS ").unwrap().name(), "Shift_JIS");
//! assert!(Encoding::for_whatwg_label(b"not-an-encoding").is_none());
//! # }
//! # #[cfg(not(feature = "whatwg"))]
//! # fn main() {}
//! ```
//!
//! Encoding goes the other way, and stops at the first character the target
//! cannot represent — silently mangling text is never the default:
//!
//! ```
//! # #[cfg(all(feature = "alloc", feature = "whatwg"))]
//! # fn main() {
//! use charcode::{EncodeOptions, EUC_KR, Unmappable};
//!
//! let (bytes, encoding, _) = EUC_KR.encode("\u{D55C}").unwrap();
//! assert_eq!(&bytes[..], b"\xC7\xD1");
//! assert_eq!(encoding, EUC_KR);
//!
//! // An emoji is not in EUC-KR, so say what should happen to it.
//! assert!(EUC_KR.encode("\u{D55C}\u{1F600}").is_err());
//! let options = EncodeOptions::new().unmappable(Unmappable::Replace('?'));
//! let (bytes, _, tally) = EUC_KR.encode_with("\u{D55C}\u{1F600}", options).unwrap();
//! assert_eq!(&bytes[..], b"\xC7\xD1?");
//! assert_eq!(tally.errors, 1);
//! # }
//! # #[cfg(not(all(feature = "alloc", feature = "whatwg")))]
//! # fn main() {}
//! ```
//!
//! # Error policies
//!
//! [`DecodeOptions`] and [`EncodeOptions`] say what to do about input that does
//! not decode and characters the target cannot represent.  Decoding substitutes
//! U+FFFD by default, as the standard requires; encoding fails by default,
//! because every alternative changes the text:
//!
//! | | [`Malformed`] | [`Unmappable`] |
//! | --- | --- | --- |
//! | stop and report | `Fail` | `Fail` *(default)* |
//! | drop it | `Omit` | `Omit` |
//! | write a character | `Replace(c)` *(default U+FFFD)* | `Replace(c)` |
//! | `&#19968;` | | `Html` |
//! | `\u4e00` | | `JsonEscape` |
//!
//! The two escaping policies also rewrite their own introducer — `&` becomes
//! `&amp;`, `\` becomes `\\` — so what they write reads back unambiguously.
//! With the `translit` feature, [`EncodeOptions::transliterate`] tries a close
//! ASCII equivalent first and falls through to the policy above.
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
//! # #[cfg(all(feature = "alloc", feature = "whatwg"))]
//! # fn main() {
//! use charcode::BIG5;
//!
//! let mut decoder = BIG5.new_decoder();
//! let mut text = String::new();
//! decoder.decode_to_string(&[0xA4], &mut text, false).unwrap();
//! decoder.decode_to_string(&[0x40], &mut text, true).unwrap();
//! assert_eq!(text, "\u{4E00}");
//! # }
//! # #[cfg(not(all(feature = "alloc", feature = "whatwg")))]
//! # fn main() {}
//! ```
//!
//! [`Decoder::decode_to_utf8`] and [`Encoder::encode_from_utf8`] are the
//! allocation-free forms, writing into a caller-provided `&mut [u8]`.
//!
//! # Errors
//!
//! # Features
//!
//! Capabilities:
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
//! The encodings themselves come in table groups, all of which are independent:
//!
//! - `whatwg` (default): the standard's 40 encodings and `whatwg-aliases`.
//! - `single-byte`: the standard's 28 legacy single-byte encodings.
//! - `big5`, `euc-jp`, `euc-kr`, `gb18030`, `iso-2022-jp`, `shift-jis`: one per
//!   legacy multi-byte encoding, because theirs are the large tables.
//!   `gb18030` also provides [`GBK`].
//! - `extras`: everything below at once.
//! - `dos`: IBM PC / OEM code pages — 437, 737, 775, 850, 852, 855, 856, 857,
//!   860 to 865, 869, 1006.
//! - `ebcdic`: IBM mainframe code pages — 037, 424, 500, 875, 1026.
//! - `mac`: Apple's regional variants of Mac OS Roman.
//! - `misc`: Atari ST and KZ-1048.
//! - `unicode-extras`: UTF-32BE/LE and UTF-7.  No tables; these are algorithmic.
//!
//! UTF-8, UTF-16BE/LE, `replacement` and `x-user-defined` need no tables and
//! are always present; the first three are what byte order mark sniffing
//! resolves to.  A build with no table group at all is about 1 KiB of static
//! data; the whole standard is about 540 KiB, and everything is about 560 KiB.
//!
//! Separately, `whatwg-aliases` adds [`Encoding::for_whatwg_label`], the
//! standard's `get an encoding`.  It is independent of which tables you take,
//! and it never answers with a charset from outside the standard, so adding
//! `dos` or `ebcdic` for local use cannot widen what a label off the network
//! can select:
//!
//! ```toml
//! # Japanese and Unicode, with the standard's naming for them, and the DOS
//! # code pages available locally but not selectable by a remote label.
//! charcode = { version = "0.1", default-features = false, features = [
//!     "std", "whatwg-aliases", "shift-jis", "euc-jp", "iso-2022-jp", "dos",
//! ] }
//! ```
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
/// They use the owned-output API and the standard's encodings, so they are only
/// checked when `alloc` and `whatwg` are on; hidden `cfg` lines are not an
/// option here because GitHub renders them.
#[cfg(all(doctest, feature = "alloc", feature = "whatwg"))]
#[doc = include_str!("../README.md")]
struct Readme;

mod ascii;
#[cfg(feature = "big5")]
mod big5;
mod code_page;
mod decoder;
mod encoder;
#[cfg(feature = "euc-jp")]
mod euc_jp;
#[cfg(feature = "euc-kr")]
mod euc_kr;
#[cfg(feature = "full-byte")]
mod full_byte;
#[cfg(feature = "gb18030")]
mod gb18030;
#[cfg(any(
    feature = "big5",
    feature = "euc-jp",
    feature = "euc-kr",
    feature = "gb18030",
    feature = "iso-2022-jp",
    feature = "shift-jis"
))]
mod index;
#[cfg(feature = "iso-2022-jp")]
mod iso_2022_jp;
mod options;
mod replacement;
mod result;
#[cfg(feature = "shift-jis")]
mod shift_jis;
#[cfg(feature = "half-byte")]
mod single_byte;
mod sink;
#[cfg(feature = "translit")]
mod translit;

mod tables;
#[cfg(test)]
mod tests;
mod utf_16;
#[cfg(feature = "unicode-extras")]
mod utf_32;
#[cfg(feature = "unicode-extras")]
mod utf_7;
mod utf_8;
mod variant;
mod x_user_defined;

mod encodings;
#[cfg(any(
    feature = "dos",
    feature = "ebcdic",
    feature = "mac",
    feature = "misc",
    feature = "unicode-extras"
))]
mod extra_encodings;

#[cfg(feature = "serde")]
mod serde_impl;

#[cfg(feature = "alloc")]
use alloc::{borrow::Cow, string::String, vec::Vec};

pub use crate::decoder::{DECODER_MIN_BUFFER, Decoder, MalformedError};
pub use crate::encoder::{ENCODER_MIN_BUFFER, Encoder, UnmappableError};
pub use crate::encodings::*;
#[cfg(any(
    feature = "dos",
    feature = "ebcdic",
    feature = "mac",
    feature = "misc",
    feature = "unicode-extras"
))]
pub use crate::extra_encodings::*;
pub use crate::options::{Bom, DecodeOptions, EncodeOptions, Malformed, Tally, Unmappable};
pub use crate::result::{DecoderResult, EncoderResult};

use crate::code_page::CODE_PAGES;
use crate::tables::extra_labels::EXTRA_CODE_PAGES;
use crate::tables::labels::{ALL_ENCODINGS, LABELS, Label};
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
        ALL_ENCODINGS
    }

    /// Every label the standard defines for this encoding, in sorted order.
    ///
    /// The name itself is always among them, ASCII-lowercased.
    ///
    /// ```
    /// # #[cfg(feature = "whatwg")]
    /// # fn main() {
    /// # use charcode::IBM866;
    /// let labels: Vec<_> = IBM866.labels().collect();
    /// assert_eq!(labels, ["866", "cp866", "csibm866", "ibm866"]);
    /// # }
    /// # #[cfg(not(feature = "whatwg"))]
    /// # fn main() {}
    /// ```
    pub fn labels(&'static self) -> impl Iterator<Item = &'static str> {
        LABELS
            .iter()
            .filter(move |entry| entry.encoding == self)
            .map(|entry| entry.text)
    }

    /// Looks up an encoding by label, implementing `get an encoding`.
    ///
    /// Leading and trailing ASCII whitespace is ignored and the comparison is
    /// ASCII case-insensitive, so a `Content-Type` charset parameter or a
    /// `<meta charset>` value can be passed through unchanged.
    ///
    /// ```
    /// # #[cfg(feature = "whatwg")]
    /// # fn main() {
    /// # use charcode::{Encoding, UTF_8};
    /// assert_eq!(Encoding::for_label(b"utf8"), Some(UTF_8));
    /// assert_eq!(Encoding::for_label(b"\tUTF-8\n"), Some(UTF_8));
    /// # }
    /// # #[cfg(not(feature = "whatwg"))]
    /// # fn main() {}
    /// ```
    pub fn for_label(label: &[u8]) -> Option<&'static Encoding> {
        Encoding::look_up(label).map(|entry| entry.encoding)
    }

    /// The row in the label table for `label`, after normalization.
    fn look_up(label: &[u8]) -> Option<&'static Label> {
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
            .binary_search_by(|entry| entry.text.as_bytes().cmp(needle))
            .ok()
            .map(|i| &LABELS[i])
    }

    /// Implements the standard's `get an encoding`, and answers only with an
    /// encoding the standard defines.
    ///
    /// This is the lookup to use on anything that came off the network — a
    /// `Content-Type` charset parameter, an HTML `<meta charset>`, an XML
    /// declaration.  It differs from [`Encoding::for_label`] in what it will
    /// *refuse*: a charset outside the standard is never returned, however many
    /// of the extra table groups are compiled in.  That matters because the
    /// standard leaves some charsets out on purpose — UTF-7 and HZ-GB-2312 can
    /// both be used to smuggle markup past a filter that only inspects the
    /// bytes — so a build that adds them for local use must not thereby widen
    /// what a remote label can select.
    ///
    /// ```
    /// # #[cfg(all(feature = "whatwg-aliases", feature = "single-byte"))]
    /// # fn main() {
    /// # use charcode::{Encoding, WINDOWS_1252};
    /// // The standard's own naming, including the labels it folds together.
    /// assert_eq!(Encoding::for_whatwg_label(b"latin1"), Some(WINDOWS_1252));
    /// assert_eq!(Encoding::for_whatwg_label(b"ISO-8859-1"), Some(WINDOWS_1252));
    ///
    /// // `cp437` names a real encoding here when the `dos` group is on, but it
    /// // is not one the standard sanctions, so this lookup will not return it.
    /// assert_eq!(Encoding::for_whatwg_label(b"cp437"), None);
    /// # }
    /// # #[cfg(not(all(feature = "whatwg-aliases", feature = "single-byte")))]
    /// # fn main() {}
    /// ```
    #[cfg(feature = "whatwg-aliases")]
    pub fn for_whatwg_label(label: &[u8]) -> Option<&'static Encoding> {
        Encoding::look_up(label)
            .filter(|entry| entry.whatwg)
            .map(|entry| entry.encoding)
    }

    /// Like [`Encoding::for_whatwg_label`], but treats the labels that map to
    /// [`REPLACEMENT`] as unknown.
    #[cfg(feature = "whatwg-aliases")]
    pub fn for_whatwg_label_no_replacement(label: &[u8]) -> Option<&'static Encoding> {
        match Encoding::for_whatwg_label(label) {
            Some(encoding) if encoding.variant == VariantEncoding::Replacement => None,
            other => other,
        }
    }

    /// Whether this encoding is one the WHATWG Encoding Standard defines.
    ///
    /// False for everything the extra table groups add.
    pub fn is_whatwg(&self) -> bool {
        LABELS
            .iter()
            .any(|entry| entry.whatwg && entry.encoding == self)
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
    /// # #[cfg(feature = "whatwg")]
    /// # fn main() {
    /// # use charcode::{Encoding, BIG5, SHIFT_JIS, WINDOWS_1252};
    /// assert_eq!(Encoding::for_windows_code_page(1252), Some(WINDOWS_1252));
    /// assert_eq!(Encoding::for_windows_code_page(932), Some(SHIFT_JIS));
    /// assert_eq!(Encoding::for_windows_code_page(950), Some(BIG5));
    /// // 437 is IBM437, which the off-by-default `dos` group carries.
    /// # #[cfg(not(feature = "dos"))]
    /// assert_eq!(Encoding::for_windows_code_page(437), None);
    /// # }
    /// # #[cfg(not(feature = "whatwg"))]
    /// # fn main() {}
    /// ```
    pub fn for_windows_code_page(code_page: u32) -> Option<&'static Encoding> {
        for table in [CODE_PAGES, EXTRA_CODE_PAGES] {
            if let Ok(i) = table.binary_search_by_key(&code_page, |entry| entry.number) {
                return Some(table[i].encoding);
            }
        }
        None
    }

    /// Looks up an encoding by code page number written the `cpNNN` way.
    ///
    /// `cp932` and `windows-932` name the same entry in the same registry, so
    /// this reads the same table as [`Encoding::for_windows_code_page`]; it
    /// exists so that a call site spelled after `cp1252` or `cp866` does not
    /// have to say "windows" to mean IBM's or DOS's numbering.
    ///
    /// ```
    /// # #[cfg(feature = "whatwg")]
    /// # fn main() {
    /// # use charcode::{Encoding, IBM866, SHIFT_JIS};
    /// assert_eq!(Encoding::for_cp(932), Some(SHIFT_JIS));
    /// assert_eq!(Encoding::for_cp(866), Some(IBM866));
    /// # #[cfg(not(feature = "dos"))]
    /// assert_eq!(Encoding::for_cp(437), None);
    /// # }
    /// # #[cfg(not(feature = "whatwg"))]
    /// # fn main() {}
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
    /// # #[cfg(feature = "whatwg")]
    /// # fn main() {
    /// # use charcode::{Encoding, SHIFT_JIS, WINDOWS_1252, X_USER_DEFINED};
    /// assert_eq!(WINDOWS_1252.windows_code_page(), Some(1252));
    /// assert_eq!(SHIFT_JIS.windows_code_page(), Some(932));
    /// // 28591 also resolves to windows-1252, but 1252 is its own number.
    /// assert_eq!(Encoding::for_windows_code_page(28591), Some(WINDOWS_1252));
    /// // Microsoft has no number for this one.
    /// assert_eq!(X_USER_DEFINED.windows_code_page(), None);
    /// # }
    /// # #[cfg(not(feature = "whatwg"))]
    /// # fn main() {}
    /// ```
    pub fn windows_code_page(&self) -> Option<u32> {
        CODE_PAGES
            .iter()
            .chain(EXTRA_CODE_PAGES)
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

    /// A decoder with the default options: sniff for a byte order mark, and
    /// substitute U+FFFD for malformed input.
    ///
    /// If the stream starts with a byte order mark, the decoder switches to the
    /// encoding it names and [`Decoder::encoding`] reports the change.
    pub fn new_decoder(&'static self) -> Decoder {
        Decoder::new(self, DecodeOptions::new())
    }

    /// A decoder with the given options.
    pub fn new_decoder_with(&'static self, options: DecodeOptions) -> Decoder {
        Decoder::new(self, options)
    }

    /// An encoder for this encoding's [output encoding](Encoding::output_encoding),
    /// which stops at the first character the encoding cannot represent.
    pub fn new_encoder(&'static self) -> Encoder {
        Encoder::new(self.output_encoding(), EncodeOptions::new())
    }

    /// An encoder with the given options.
    pub fn new_encoder_with(&'static self, options: EncodeOptions) -> Encoder {
        Encoder::new(self.output_encoding(), options)
    }

    /// Decodes `bytes` with the default options: a leading byte order mark wins
    /// over `self`, and malformed sequences become U+FFFD.
    ///
    /// The second element is the encoding actually used, which differs from
    /// `self` when a byte order mark named another one.
    #[cfg(feature = "alloc")]
    pub fn decode<'a>(&'static self, bytes: &'a [u8]) -> (Cow<'a, str>, &'static Encoding, Tally) {
        self.decode_with(bytes, DecodeOptions::new())
    }

    /// Decodes `bytes` with the given options.
    ///
    /// Under [`Malformed::Fail`] this stops at the first bad sequence and the
    /// [`Tally`] says so; use [`Encoding::try_decode`] to get the error itself.
    #[cfg(feature = "alloc")]
    pub fn decode_with<'a>(
        &'static self,
        bytes: &'a [u8],
        options: DecodeOptions,
    ) -> (Cow<'a, str>, &'static Encoding, Tally) {
        match self.try_decode(bytes, options) {
            Ok((text, encoding, tally)) => (text, encoding, tally),
            Err((text, encoding, _)) => (Cow::Owned(text), encoding, Tally { errors: 1 }),
        }
    }

    /// Decodes `bytes`, returning the first malformed sequence as an error.
    ///
    /// The error carries what had been decoded before it, so a caller can
    /// report where the input went wrong.
    #[cfg(feature = "alloc")]
    #[allow(clippy::type_complexity)]
    pub fn try_decode<'a>(
        &'static self,
        bytes: &'a [u8],
        options: DecodeOptions,
    ) -> Result<(Cow<'a, str>, &'static Encoding, Tally), (String, &'static Encoding, MalformedError)>
    {
        let (encoding, skip) = match options.bom {
            Bom::Sniff => Encoding::for_bom(bytes).unwrap_or((self, 0)),
            Bom::Remove => match self.bom() {
                Some(bom) if bytes.starts_with(bom) => (self, bom.len()),
                _ => (self, 0),
            },
            Bom::Ignore => (self, 0),
        };
        let bytes = &bytes[skip..];
        if let Some(borrowed) = encoding.borrow_as_str(bytes) {
            return Ok((Cow::Borrowed(borrowed), encoding, Tally::default()));
        }
        let mut text = String::with_capacity(bytes.len());
        // The mark is already gone, so the decoder must not look for one again.
        let mut decoder = encoding.new_decoder_with(DecodeOptions {
            bom: Bom::Ignore,
            ..options
        });
        match decoder.decode_to_string(bytes, &mut text, true) {
            Ok(()) => Ok((Cow::Owned(text), encoding, decoder.tally())),
            Err(e) => Err((text, encoding, e)),
        }
    }

    /// Encodes `string` into this encoding's
    /// [output encoding](Encoding::output_encoding), failing on the first
    /// character it cannot represent.
    ///
    /// Pass [`EncodeOptions`] to [`Encoding::encode_with`] to substitute, drop
    /// or escape those characters instead.
    #[cfg(feature = "alloc")]
    #[allow(clippy::type_complexity)]
    pub fn encode<'a>(
        &'static self,
        string: &'a str,
    ) -> Result<(Cow<'a, [u8]>, &'static Encoding, Tally), UnmappableError> {
        self.encode_with(string, EncodeOptions::new())
    }

    /// Encodes `string` with the given options.
    #[cfg(feature = "alloc")]
    #[allow(clippy::type_complexity)]
    pub fn encode_with<'a>(
        &'static self,
        string: &'a str,
        options: EncodeOptions,
    ) -> Result<(Cow<'a, [u8]>, &'static Encoding, Tally), UnmappableError> {
        let output = self.output_encoding();
        // Nothing to escape or replace means the bytes may already be the answer.
        if options.unmappable != Unmappable::Html
            && options.unmappable != Unmappable::JsonEscape
            && output.borrow_as_str(string.as_bytes()).is_some()
        {
            return Ok((Cow::Borrowed(string.as_bytes()), output, Tally::default()));
        }
        let mut bytes = Vec::with_capacity(string.len());
        let mut encoder = output.new_encoder_with(options);
        encoder.encode_from_str(string, &mut bytes, true)?;
        Ok((Cow::Owned(bytes), output, encoder.tally()))
    }

    /// The standard's `encode` hook, for HTML form submission.
    ///
    /// Unmappable characters become decimal numeric character references, and
    /// `&` is **not** escaped — an ambiguity form submission has always had.
    /// For general use prefer [`Unmappable::Html`], which does escape it.
    #[cfg(feature = "alloc")]
    pub fn encode_html_form<'a>(
        &'static self,
        string: &'a str,
    ) -> (Cow<'a, [u8]>, &'static Encoding, Tally) {
        let output = self.output_encoding();
        if output.borrow_as_str(string.as_bytes()).is_some() {
            return (Cow::Borrowed(string.as_bytes()), output, Tally::default());
        }
        let mut bytes = Vec::with_capacity(string.len());
        let mut encoder = Encoder::new_html_form(output);
        encoder
            .encode_from_str(string, &mut bytes, true)
            .expect("numeric character references never fail");
        (Cow::Owned(bytes), output, encoder.tally())
    }

    /// Returns `bytes` as a `&str` when this encoding decodes them to exactly
    /// themselves, which is what makes the borrowing `Cow` cases possible.
    #[cfg(feature = "alloc")]
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
