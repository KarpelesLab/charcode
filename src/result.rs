//! The result types reported by the streaming [`Decoder`](crate::Decoder) and
//! [`Encoder`](crate::Encoder).
//!
//! `Malformed` and `Unmappable` appear only under the failing policies; every
//! other [`Malformed`](crate::Malformed) or [`Unmappable`](crate::Unmappable)
//! setting handles the problem and keeps going.

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
