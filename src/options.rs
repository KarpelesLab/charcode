//! How a conversion should treat a byte order mark, malformed input and
//! characters the target encoding cannot represent.

/// What to do with a byte order mark at the start of the input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Bom {
    /// Look for any byte order mark and switch to the encoding it names, which
    /// is what the standard's `decode` hook does and what browsers do.
    #[default]
    Sniff,
    /// Strip only this encoding's own byte order mark, and treat any other as
    /// content.
    Remove,
    /// Treat a byte order mark as ordinary content, decoding it to U+FEFF.
    Ignore,
}

/// What to do with a byte sequence that does not decode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Malformed {
    /// Write this character in its place.  The default is U+FFFD, which is what
    /// the standard requires and what browsers show.
    Replace(char),
    /// Drop it, as `iconv -c` does.
    Omit,
    /// Stop, and report where.
    Fail,
}

impl Default for Malformed {
    fn default() -> Self {
        Malformed::Replace(char::REPLACEMENT_CHARACTER)
    }
}

/// What to do with a character the target encoding cannot represent.
///
/// Every variant but [`Unmappable::Fail`] changes the text, so none of them is
/// the default: an escape is only correct when you know how the consumer will
/// read it back.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Unmappable {
    /// Stop, and report which character.
    #[default]
    Fail,
    /// Drop it, as `iconv -c` does.
    Omit,
    /// Write this character in its place — conventionally `'?'`, the "unknown
    /// character" of many older converters.
    ///
    /// It must itself be representable in the target encoding, or the
    /// conversion fails naming the replacement.  An ASCII character is always
    /// safe, including in the EBCDIC pages, where the encoder maps it for you.
    Replace(char),
    /// Write a decimal numeric character reference: `&#19968;`.
    ///
    /// An escape is only unambiguous if its introducer cannot appear by
    /// accident, so this mode **also rewrites `&` as `&amp;`** wherever it
    /// occurs in text the encoding *can* represent.  Without that, a literal
    /// `&#65;` in the input would read back as `A`.  Rewriting `&` is not
    /// counted in the [`Tally`]: nothing was lost.
    ///
    /// For the standard's own `encode` hook — which emits references but does
    /// not escape `&`, an ambiguity HTML form submission has always had — use
    /// [`Encoding::encode_html_form`](crate::Encoding::encode_html_form).
    Html,
    /// Write a `\uXXXX` escape, using a surrogate pair above the basic
    /// multilingual plane: `\u4e00`, `\ud83d\ude00`.
    ///
    /// The form JSON uses, and JavaScript, Java and C# with it.  As with
    /// [`Unmappable::Html`] the introducer has to stay unambiguous, so this
    /// mode **also rewrites `\` as `\\`** in text the encoding can represent.
    JsonEscape,
}

/// How to decode.
///
/// ```
/// # #[cfg(all(feature = "alloc", feature = "single-byte"))]
/// # fn main() {
/// use charcode::{Bom, DecodeOptions, Malformed, WINDOWS_1252};
///
/// let options = DecodeOptions::new()
///     .bom(Bom::Ignore)
///     .malformed(Malformed::Omit);
/// let (text, _, tally) = WINDOWS_1252.decode_with(b"caf\xE9", options);
/// assert_eq!(text, "caf\u{E9}");
/// assert!(tally.is_lossless());
/// # }
/// # #[cfg(not(all(feature = "alloc", feature = "single-byte")))]
/// # fn main() {}
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DecodeOptions {
    pub(crate) bom: Bom,
    pub(crate) malformed: Malformed,
}

impl DecodeOptions {
    /// Byte order mark sniffing, and U+FFFD for malformed input.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets how a leading byte order mark is treated.
    #[must_use]
    pub fn bom(mut self, bom: Bom) -> Self {
        self.bom = bom;
        self
    }

    /// Sets what happens to a byte sequence that does not decode.
    #[must_use]
    pub fn malformed(mut self, malformed: Malformed) -> Self {
        self.malformed = malformed;
        self
    }

    /// The configured policy for malformed input.
    pub fn malformed_policy(&self) -> Malformed {
        self.malformed
    }

    /// The configured byte order mark handling.
    pub fn bom_handling(&self) -> Bom {
        self.bom
    }
}

/// How to encode.
///
/// ```
/// # #[cfg(all(feature = "alloc", feature = "single-byte"))]
/// # fn main() {
/// use charcode::{EncodeOptions, Unmappable, WINDOWS_1252};
///
/// // The default stops at a character the encoding cannot represent.
/// assert!(WINDOWS_1252.encode("a\u{4E00}").is_err());
///
/// let options = EncodeOptions::new().unmappable(Unmappable::Replace('?'));
/// let (bytes, _, tally) = WINDOWS_1252.encode_with("a\u{4E00}", options).unwrap();
/// assert_eq!(&bytes[..], b"a?");
/// assert_eq!(tally.errors, 1);
///
/// let options = EncodeOptions::new().unmappable(Unmappable::JsonEscape);
/// let (bytes, _, _) = WINDOWS_1252.encode_with("a\u{4E00}\\", options).unwrap();
/// assert_eq!(&bytes[..], b"a\\u4e00\\\\");
/// # }
/// # #[cfg(not(all(feature = "alloc", feature = "single-byte")))]
/// # fn main() {}
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EncodeOptions {
    pub(crate) unmappable: Unmappable,
    pub(crate) transliterate: bool,
}

impl EncodeOptions {
    /// Stops at the first character the encoding cannot represent.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets what happens to a character the encoding cannot represent.
    #[must_use]
    pub fn unmappable(mut self, unmappable: Unmappable) -> Self {
        self.unmappable = unmappable;
        self
    }

    /// Tries to write a close ASCII equivalent before giving up on a character:
    /// `é` as `e`, `œ` as `oe`, `—` as `-`, `€` as `EUR`.
    ///
    /// This is `iconv`'s `//TRANSLIT`.  It is approximate by nature and covers
    /// Latin text, punctuation and common symbols; a character with no sensible
    /// equivalent — most of CJK — falls through to
    /// [`unmappable`](EncodeOptions::unmappable), which is why that setting
    /// still matters when this is on.
    #[cfg(feature = "translit")]
    #[must_use]
    pub fn transliterate(mut self, transliterate: bool) -> Self {
        self.transliterate = transliterate;
        self
    }

    /// The configured policy for unmappable characters.
    pub fn unmappable_policy(&self) -> Unmappable {
        self.unmappable
    }

    /// Whether transliteration is tried first.
    pub fn transliterates(&self) -> bool {
        self.transliterate
    }
}

/// How much a conversion had to substitute or drop.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Tally {
    /// Malformed byte sequences when decoding, or characters the encoding could
    /// not represent when encoding.
    pub errors: u64,
}

impl Tally {
    /// True if nothing was substituted or dropped, so the conversion is exact.
    pub fn is_lossless(self) -> bool {
        self.errors == 0
    }
}
