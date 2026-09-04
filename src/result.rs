//! The result types reported by the streaming [`Decoder`](crate::Decoder) and
//! [`Encoder`](crate::Encoder).

/// Why a conversion that substitutes errors stopped.
///
/// Both variants are ordinary control flow: `InputEmpty` means the caller should
/// supply more input, `OutputFull` means it should drain the output buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CoderResult {
    /// All of the input was consumed.
    InputEmpty,
    /// The output buffer ran out of room before the input was consumed.
    OutputFull,
}

/// Why a decode that reports errors stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DecoderResult {
    /// All of the input was consumed.
    InputEmpty,
    /// The output buffer ran out of room before the input was consumed.
    OutputFull,
    /// A malformed byte sequence of the given length was found and consumed.
    ///
    /// The bytes are counted from the start of the malformed sequence, which may
    /// have begun in an earlier call, so the length can exceed the number of bytes
    /// read by the call that reports it.  Decoding continues from the byte after
    /// the sequence; a caller substituting errors writes one U+FFFD per report.
    Malformed(u8),
}

/// Why an encode that reports errors stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EncoderResult {
    /// All of the input was consumed.
    InputEmpty,
    /// The output buffer ran out of room before the input was consumed.
    OutputFull,
    /// The given character has no representation in the target encoding, and was
    /// consumed without producing output.
    Unmappable(char),
}

impl DecoderResult {
    /// Converts to a [`CoderResult`], panicking on `Malformed`.
    pub(crate) fn as_coder_result(self) -> CoderResult {
        match self {
            DecoderResult::InputEmpty => CoderResult::InputEmpty,
            DecoderResult::OutputFull => CoderResult::OutputFull,
            DecoderResult::Malformed(_) => unreachable!("errors are handled by the caller"),
        }
    }
}

impl EncoderResult {
    pub(crate) fn as_coder_result(self) -> CoderResult {
        match self {
            EncoderResult::InputEmpty => CoderResult::InputEmpty,
            EncoderResult::OutputFull => CoderResult::OutputFull,
            EncoderResult::Unmappable(_) => unreachable!("errors are handled by the caller"),
        }
    }
}
