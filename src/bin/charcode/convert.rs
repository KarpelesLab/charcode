//! The streaming conversion itself: decode a chunk of input, encode the text it
//! produced, write the bytes out, repeat.

use std::fmt;
use std::io::{self, Write};

use charcode::{Decoder, Encoder, Encoding};

use crate::args::OnError;

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
                 Use -c to omit it, or --substitute to write a numeric character reference.",
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

impl From<ConvertError> for Error {
    fn from(e: ConvertError) -> Self {
        Error::Convert(e)
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

/// How much had to be dropped or replaced along the way.
#[derive(Debug, Default, Clone, Copy)]
pub struct Tally {
    pub omitted_malformed: u64,
    pub omitted_unmappable: u64,
    pub substituted_malformed: bool,
    pub substituted_unmappable: bool,
    pub bytes_in: u64,
    pub bytes_out: u64,
}

pub struct Converter {
    decoder: Decoder,
    encoder: Encoder,
    to: &'static Encoding,
    on_error: OnError,
    text: String,
    bytes: Vec<u8>,
    pub tally: Tally,
}

impl Converter {
    pub fn new(from: &'static Encoding, to: &'static Encoding, on_error: OnError) -> Converter {
        Converter {
            // A byte order mark names the encoding more reliably than a label
            // does, which is why the standard lets it win; -f says what to
            // assume in its absence.
            decoder: from.new_decoder(),
            encoder: to.new_encoder(),
            to: to.output_encoding(),
            on_error,
            text: String::new(),
            bytes: Vec::new(),
            tally: Tally::default(),
        }
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
        self.tally.bytes_in += input.len() as u64;
        self.decode(input, last, origin)?;
        self.encode(last, origin)?;
        out.write_all(&self.bytes)?;
        self.tally.bytes_out += self.bytes.len() as u64;
        Ok(())
    }

    fn decode(
        &mut self,
        input: &[u8],
        last: bool,
        origin: &Origin<'_>,
    ) -> Result<(), ConvertError> {
        self.text.clear();
        if self.on_error == OnError::Substitute {
            if self.decoder.decode_to_string(input, &mut self.text, last) {
                self.tally.substituted_malformed = true;
            }
            return Ok(());
        }
        let mut pos = 0;
        loop {
            match self.decoder.decode_to_string_without_replacement(
                &input[pos..],
                &mut self.text,
                last,
            ) {
                Ok(()) => return Ok(()),
                Err(e) => {
                    // `offset` is where decoding got to within the slice, just
                    // past the malformed sequence.
                    let end = origin.base + (pos + e.offset) as u64;
                    if self.on_error == OnError::Fail {
                        return Err(ConvertError::Malformed {
                            source: origin.name.to_owned(),
                            offset: end.saturating_sub(u64::from(e.len)),
                            len: e.len,
                        });
                    }
                    self.tally.omitted_malformed += 1;
                    pos += e.offset;
                }
            }
        }
    }

    fn encode(&mut self, last: bool, origin: &Origin<'_>) -> Result<(), ConvertError> {
        self.bytes.clear();
        if self.on_error == OnError::Substitute {
            if self
                .encoder
                .encode_from_str(&self.text, &mut self.bytes, last)
            {
                self.tally.substituted_unmappable = true;
            }
            return Ok(());
        }
        let mut pos = 0;
        loop {
            match self.encoder.encode_from_str_without_replacement(
                &self.text[pos..],
                &mut self.bytes,
                last,
            ) {
                Ok(()) => return Ok(()),
                Err(e) => {
                    if self.on_error == OnError::Fail {
                        return Err(ConvertError::Unmappable {
                            source: origin.name.to_owned(),
                            character: e.character,
                            encoding: self.to.name(),
                        });
                    }
                    self.tally.omitted_unmappable += 1;
                    pos += e.offset;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn convert(
        from: &'static Encoding,
        to: &'static Encoding,
        on_error: OnError,
        chunks: &[&[u8]],
    ) -> Result<(Vec<u8>, Tally), Error> {
        let mut converter = Converter::new(from, to, on_error);
        let mut out = Vec::new();
        let origin = Origin {
            name: "test",
            base: 0,
        };
        let mut base = 0u64;
        for chunk in chunks {
            let origin = Origin { base, ..origin };
            converter.feed(chunk, false, &mut out, &origin)?;
            base += chunk.len() as u64;
        }
        let origin = Origin { base, ..origin };
        converter.feed(&[], true, &mut out, &origin)?;
        Ok((out, converter.tally))
    }

    #[test]
    fn transcodes_between_legacy_encodings() {
        let (out, _) = convert(
            charcode::WINDOWS_1252,
            charcode::ISO_8859_2,
            OnError::Fail,
            &[b"caf\xE9"],
        )
        .expect("both encodings have e-acute");
        assert_eq!(out, b"caf\xE9");

        let (out, _) = convert(
            charcode::SHIFT_JIS,
            charcode::UTF_8,
            OnError::Fail,
            &[b"\x93\xFA\x96\x7B"],
        )
        .expect("valid Shift_JIS");
        assert_eq!(out, "\u{65E5}\u{672C}".as_bytes());
    }

    #[test]
    fn a_multi_byte_sequence_may_straddle_chunks() {
        let (out, _) = convert(
            charcode::SHIFT_JIS,
            charcode::UTF_8,
            OnError::Fail,
            &[b"\x93", b"\xFA"],
        )
        .expect("the decoder carries the lead byte across");
        assert_eq!(out, "\u{65E5}".as_bytes());
    }

    #[test]
    fn a_byte_order_mark_wins_over_the_named_encoding() {
        let (out, _) = convert(
            charcode::WINDOWS_1252,
            charcode::UTF_8,
            OnError::Fail,
            &[b"\xEF\xBB\xBFcaf\xC3\xA9"],
        )
        .unwrap();
        assert_eq!(out, "caf\u{E9}".as_bytes());
    }

    #[test]
    fn failing_is_the_default_and_reports_where() {
        let error = convert(
            charcode::UTF_8,
            charcode::UTF_8,
            OnError::Fail,
            &[b"ok then \xFF more"],
        )
        .unwrap_err();
        let Error::Convert(ConvertError::Malformed { offset, len, .. }) = error else {
            panic!("expected a malformed-input error, got {error}");
        };
        assert_eq!((offset, len), (8, 1));

        let error = convert(
            charcode::UTF_8,
            charcode::WINDOWS_1252,
            OnError::Fail,
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
        let (out, tally) = convert(
            charcode::UTF_8,
            charcode::WINDOWS_1252,
            OnError::Omit,
            &[b"a\xFFb\xE4\xB8\x80c"],
        )
        .unwrap();
        assert_eq!(out, b"abc");
        assert_eq!(tally.omitted_malformed, 1);
        assert_eq!(tally.omitted_unmappable, 1);
    }

    #[test]
    fn substituting_matches_the_standards_web_behaviour() {
        let (out, tally) = convert(
            charcode::UTF_8,
            charcode::WINDOWS_1252,
            OnError::Substitute,
            &[b"a\xFFb\xE4\xB8\x80c"],
        )
        .unwrap();
        // U+FFFD is itself unmappable in windows-1252, so it becomes a reference.
        assert_eq!(out, b"a&#65533;b&#19968;c");
        assert!(tally.substituted_malformed);
        assert!(tally.substituted_unmappable);
    }

    #[test]
    fn stateful_output_is_flushed_at_the_end() {
        let (out, _) = convert(
            charcode::UTF_8,
            charcode::ISO_2022_JP,
            OnError::Fail,
            &["\u{65E5}".as_bytes()],
        )
        .unwrap();
        // The trailing escape back to ASCII only appears because of the final
        // `last = true` call.
        assert_eq!(out, b"\x1B$B\x46\x7C\x1B(B");
    }

    #[test]
    fn a_truncated_sequence_at_end_of_input_is_an_error() {
        let error = convert(
            charcode::UTF_8,
            charcode::UTF_8,
            OnError::Fail,
            &[b"good \xE2\x82"],
        )
        .unwrap_err();
        assert!(matches!(
            error,
            Error::Convert(ConvertError::Malformed {
                offset: 5,
                len: 2,
                ..
            })
        ));
    }
}
