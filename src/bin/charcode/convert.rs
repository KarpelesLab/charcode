//! The streaming conversion itself: decode a chunk of input, encode the text it
//! produced, write the bytes out, repeat.
//!
//! What happens to malformed input and to characters the target cannot
//! represent is the library's business now; this only chooses the policy and
//! reports what it did.

use std::fmt;
use std::io::{self, Write};

use charcode::{DecodeOptions, EncodeOptions, Encoder, Encoding, MalformedError, Tally};
use charcode::{Decoder, UnmappableError};

/// Where the bytes being converted came from, for diagnostics.
pub struct Origin<'a> {
    pub name: &'a str,
    /// How many bytes of this input were converted before the current chunk.
    pub base: u64,
}

#[derive(Debug)]
pub enum ConvertError {
    Malformed {
        source: String,
        offset: u64,
        len: u8,
    },
    Unmappable {
        source: String,
        character: char,
        encoding: &'static str,
    },
}

impl fmt::Display for ConvertError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConvertError::Malformed {
                source,
                offset,
                len,
            } => write!(
                f,
                "{source}: malformed byte sequence of {len} byte(s) at offset {offset}\n\
                 Use -c to omit it, or --substitute to replace it with U+FFFD."
            ),
            ConvertError::Unmappable {
                source,
                character,
                encoding,
            } => write!(
                f,
                "{source}: U+{:04X} ({character:?}) cannot be represented in {encoding}\n\
                 Use -c to omit it, --substitute to write '?', --translit for a close\n\
                 ASCII equivalent, or --html / --json for an escape.",
                u32::from(*character)
            ),
        }
    }
}

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    Convert(ConvertError),
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::Io(e)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e) => e.fmt(f),
            Error::Convert(e) => e.fmt(f),
        }
    }
}

pub struct Converter {
    decoder: Decoder,
    encoder: Encoder,
    to: &'static Encoding,
    text: String,
    bytes: Vec<u8>,
    pub bytes_in: u64,
    pub bytes_out: u64,
}

impl Converter {
    pub fn new(
        from: &'static Encoding,
        to: &'static Encoding,
        decode: DecodeOptions,
        encode: EncodeOptions,
    ) -> Converter {
        Converter {
            decoder: from.new_decoder_with(decode),
            encoder: to.new_encoder_with(encode),
            to: to.output_encoding(),
            text: String::new(),
            bytes: Vec::new(),
            bytes_in: 0,
            bytes_out: 0,
        }
    }

    /// What the two halves had to substitute or drop.
    pub fn tally(&self) -> (Tally, Tally) {
        (self.decoder.tally(), self.encoder.tally())
    }

    /// Converts one chunk.  `last` must be true for the final chunk of the last
    /// input, and only then, so that a truncated sequence is reported and any
    /// trailing escape sequence is written.
    pub fn feed(
        &mut self,
        input: &[u8],
        last: bool,
        out: &mut dyn Write,
        origin: &Origin<'_>,
    ) -> Result<(), Error> {
        self.bytes_in += input.len() as u64;
        self.text.clear();
        self.decoder
            .decode_to_string(input, &mut self.text, last)
            .map_err(|MalformedError { offset, len }| {
                Error::Convert(ConvertError::Malformed {
                    source: origin.name.to_owned(),
                    offset: (origin.base + offset as u64).saturating_sub(u64::from(len)),
                    len,
                })
            })?;

        self.bytes.clear();
        self.encoder
            .encode_from_str(&self.text, &mut self.bytes, last)
            .map_err(|UnmappableError { character, .. }| {
                Error::Convert(ConvertError::Unmappable {
                    source: origin.name.to_owned(),
                    character,
                    encoding: self.to.name(),
                })
            })?;

        out.write_all(&self.bytes)?;
        self.bytes_out += self.bytes.len() as u64;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use charcode::{Bom, Malformed, Unmappable};

    fn convert(
        from: &'static Encoding,
        to: &'static Encoding,
        decode: DecodeOptions,
        encode: EncodeOptions,
        chunks: &[&[u8]],
    ) -> Result<(Vec<u8>, u64), Error> {
        let mut converter = Converter::new(from, to, decode, encode);
        let mut out = Vec::new();
        let mut base = 0u64;
        for chunk in chunks {
            let origin = Origin { name: "test", base };
            converter.feed(chunk, false, &mut out, &origin)?;
            base += chunk.len() as u64;
        }
        let origin = Origin { name: "test", base };
        converter.feed(&[], true, &mut out, &origin)?;
        let (decoded, encoded) = converter.tally();
        Ok((out, decoded.errors + encoded.errors))
    }

    fn strict(
        from: &'static Encoding,
        to: &'static Encoding,
        chunks: &[&[u8]],
    ) -> Result<(Vec<u8>, u64), Error> {
        convert(
            from,
            to,
            DecodeOptions::new().malformed(Malformed::Fail),
            EncodeOptions::new(),
            chunks,
        )
    }

    #[test]
    fn transcodes_between_legacy_encodings() {
        let (out, _) = strict(charcode::WINDOWS_1252, charcode::ISO_8859_2, &[b"caf\xE9"])
            .expect("both encodings have e-acute");
        assert_eq!(out, b"caf\xE9");

        let (out, _) = strict(charcode::SHIFT_JIS, charcode::UTF_8, &[b"\x93\xFA\x96\x7B"])
            .expect("valid Shift_JIS");
        assert_eq!(out, "\u{65E5}\u{672C}".as_bytes());
    }

    #[test]
    fn a_multi_byte_sequence_may_straddle_chunks() {
        let (out, _) = strict(charcode::SHIFT_JIS, charcode::UTF_8, &[b"\x93", b"\xFA"])
            .expect("the decoder carries the lead byte across");
        assert_eq!(out, "\u{65E5}".as_bytes());
    }

    #[test]
    fn a_byte_order_mark_wins_over_the_named_encoding() {
        let (out, _) = strict(
            charcode::WINDOWS_1252,
            charcode::UTF_8,
            &[b"\xEF\xBB\xBFcaf\xC3\xA9"],
        )
        .unwrap();
        assert_eq!(out, "caf\u{E9}".as_bytes());
    }

    #[test]
    fn failing_is_the_default_and_reports_where() {
        let error = strict(charcode::UTF_8, charcode::UTF_8, &[b"ok then \xFF more"]).unwrap_err();
        let Error::Convert(ConvertError::Malformed { offset, len, .. }) = error else {
            panic!("expected a malformed-input error, got {error}");
        };
        assert_eq!((offset, len), (8, 1));

        let error = strict(
            charcode::UTF_8,
            charcode::WINDOWS_1252,
            &["a\u{4E00}".as_bytes()],
        )
        .unwrap_err();
        let Error::Convert(ConvertError::Unmappable {
            character,
            encoding,
            ..
        }) = error
        else {
            panic!("expected an unmappable-character error, got {error}");
        };
        assert_eq!((character, encoding), ('\u{4E00}', "windows-1252"));
    }

    #[test]
    fn omitting_drops_and_counts() {
        let (out, errors) = convert(
            charcode::UTF_8,
            charcode::WINDOWS_1252,
            DecodeOptions::new().malformed(Malformed::Omit),
            EncodeOptions::new().unmappable(Unmappable::Omit),
            &[b"a\xFFb\xE4\xB8\x80c"],
        )
        .unwrap();
        assert_eq!(out, b"abc");
        assert_eq!(errors, 2);
    }

    #[test]
    fn each_escape_policy_writes_its_own_syntax() {
        let run = |unmappable| {
            convert(
                charcode::UTF_8,
                charcode::WINDOWS_1252,
                DecodeOptions::new(),
                EncodeOptions::new().unmappable(unmappable),
                &["a&b\u{4E00}c".as_bytes()],
            )
            .unwrap()
            .0
        };
        assert_eq!(run(Unmappable::Replace('?')), b"a&b?c");
        // The introducer is escaped so the reference reads back unambiguously.
        assert_eq!(run(Unmappable::Html), b"a&amp;b&#19968;c");
        assert_eq!(run(Unmappable::JsonEscape), b"a&b\\u4e00c");
    }

    #[test]
    fn stateful_output_is_flushed_at_the_end() {
        let (out, _) = strict(
            charcode::UTF_8,
            charcode::ISO_2022_JP,
            &["\u{65E5}".as_bytes()],
        )
        .unwrap();
        assert_eq!(out, b"\x1B$B\x46\x7C\x1B(B");
    }

    #[test]
    fn a_truncated_sequence_at_end_of_input_is_an_error() {
        let error = strict(charcode::UTF_8, charcode::UTF_8, &[b"good \xE2\x82"]).unwrap_err();
        assert!(matches!(
            error,
            Error::Convert(ConvertError::Malformed {
                offset: 5,
                len: 2,
                ..
            })
        ));
    }

    #[test]
    fn bom_handling_is_selectable() {
        let (out, _) = convert(
            charcode::UTF_8,
            charcode::UTF_8,
            DecodeOptions::new().bom(Bom::Ignore),
            EncodeOptions::new(),
            &[b"\xEF\xBB\xBFa"],
        )
        .unwrap();
        assert_eq!(out, "\u{FEFF}a".as_bytes());
    }

    #[test]
    fn transliteration_falls_back_to_the_unmappable_policy() {
        let (out, _) = convert(
            charcode::UTF_8,
            charcode::WINDOWS_1252,
            DecodeOptions::new(),
            EncodeOptions::new()
                .transliterate(true)
                .unmappable(Unmappable::Replace('?')),
            &["caf\u{E9} \u{101}bc \u{2190} \u{65E5}".as_bytes()],
        )
        .unwrap();
        // é and — are in windows-1252 already, so they are left alone; ā and ←
        // are not, and fold; 日 has no ASCII form and hits the fallback.
        assert_eq!(out, b"caf\xE9 abc <- ?");
    }
}
