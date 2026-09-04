//! Best-effort ASCII transliteration, `iconv`'s `//TRANSLIT`.
//!
//! Most of the table comes from Unicode's own decompositions: expand a
//! character, drop the combining marks, and keep the result if it is ASCII.
//! That covers accented Latin, ligatures, fullwidth forms, fractions and the
//! letterlike symbols.  What does not decompose — `æ`, `ø`, `—`, `€` — is
//! listed in the generator.
//!
//! It is approximate by design and has no idea about language: `ä` folds to
//! `a`, not the `ae` a German reader would expect, and nothing outside the
//! basic multilingual plane is covered at all.  A character with no sensible
//! ASCII form, which is most of CJK, has no entry, and the encoder falls back
//! to whatever [`Unmappable`](crate::Unmappable) policy is set.

use crate::tables::translit::{DATA, KEYS, SPANS};

/// The ASCII form of `c`, if there is a sensible one.
///
/// ```ignore
/// assert_eq!(ascii_fold('\u{E9}'), Some("e"));
/// assert_eq!(ascii_fold('\u{153}'), Some("oe"));
/// assert_eq!(ascii_fold('\u{4E00}'), None);
/// ```
pub(crate) fn ascii_fold(c: char) -> Option<&'static str> {
    let scalar = u32::from(c);
    if scalar > 0xFFFF {
        return None;
    }
    let index = KEYS.binary_search(&(scalar as u16)).ok()?;
    let (offset, len) = SPANS[index];
    let (offset, len) = (usize::from(offset), usize::from(len));
    DATA.get(offset..offset + len)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folds_what_unicode_decomposes() {
        assert_eq!(ascii_fold('\u{E9}'), Some("e")); // é
        assert_eq!(ascii_fold('\u{C5}'), Some("A")); // Å
        assert_eq!(ascii_fold('\u{FB01}'), Some("fi")); // ﬁ
        assert_eq!(ascii_fold('\u{FF21}'), Some("A")); // Ａ
        assert_eq!(ascii_fold('\u{BD}'), Some("1/2")); // ½
        assert_eq!(ascii_fold('\u{2075}'), Some("5")); // ⁵
    }

    #[test]
    fn folds_what_it_does_not() {
        assert_eq!(ascii_fold('\u{E6}'), Some("ae")); // æ
        assert_eq!(ascii_fold('\u{153}'), Some("oe")); // œ
        assert_eq!(ascii_fold('\u{DF}'), Some("ss")); // ß
        assert_eq!(ascii_fold('\u{F8}'), Some("o")); // ø
        assert_eq!(ascii_fold('\u{2014}'), Some("-")); // —
        assert_eq!(ascii_fold('\u{201C}'), Some("\"")); // “
        assert_eq!(ascii_fold('\u{2026}'), Some("...")); // …
        assert_eq!(ascii_fold('\u{20AC}'), Some("EUR")); // €
        assert_eq!(ascii_fold('\u{A9}'), Some("(C)")); // ©
    }

    #[test]
    fn has_nothing_to_say_about_cjk_or_ascii() {
        assert_eq!(ascii_fold('\u{4E00}'), None);
        assert_eq!(ascii_fold('\u{3042}'), None);
        assert_eq!(ascii_fold('\u{1F600}'), None);
        // ASCII is already ASCII; the encoder never asks.
        assert_eq!(ascii_fold('a'), None);
    }

    #[test]
    fn every_fold_is_ascii_and_the_keys_are_sorted() {
        assert!(KEYS.windows(2).all(|w| w[0] < w[1]));
        assert_eq!(KEYS.len(), SPANS.len());
        for (i, &key) in KEYS.iter().enumerate() {
            let (offset, len) = SPANS[i];
            let text = DATA
                .get(usize::from(offset)..usize::from(offset) + usize::from(len))
                .unwrap_or_else(|| panic!("U+{key:04X} span is not a character boundary"));
            assert!(!text.is_empty());
            assert!(text.is_ascii(), "U+{key:04X} folds to {text:?}");
        }
    }
}
