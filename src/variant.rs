//! Dispatch from an [`Encoding`](crate::Encoding) to its decoder and encoder.
//!
//! The variants are enums rather than trait objects so that a `Decoder` stays a
//! plain value: constructing one never allocates, which matters for the streaming
//! API and for `no_std` users.

use crate::big5::{Big5Decoder, Big5Encoder};
use crate::euc_jp::{EucJpDecoder, EucJpEncoder};
use crate::euc_kr::{EucKrDecoder, EucKrEncoder};
use crate::gb18030::{Gb18030Decoder, Gb18030Encoder};
use crate::iso_2022_jp::{Iso2022JpDecoder, Iso2022JpEncoder};
use crate::replacement::ReplacementDecoder;
use crate::result::{DecoderResult, EncoderResult};
use crate::shift_jis::{ShiftJisDecoder, ShiftJisEncoder};
use crate::single_byte::{SingleByteDecoder, SingleByteEncoder};
use crate::sink::ByteSink;
use crate::utf_8::{Utf8Decoder, Utf8Encoder};
use crate::utf_16::Utf16Decoder;
use crate::x_user_defined::{XUserDefinedDecoder, XUserDefinedEncoder};

/// Which algorithm an encoding uses, plus the index tables it needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum VariantEncoding {
    Utf8,
    /// Decode table, then the encode table's code points and their bytes.
    SingleByte(&'static [u16; 128], &'static [u16], &'static [u8]),
    Utf16Be,
    Utf16Le,
    Gb18030 {
        is_gbk: bool,
    },
    Big5,
    EucJp,
    Iso2022Jp,
    ShiftJis,
    EucKr,
    Replacement,
    XUserDefined,
}

impl VariantEncoding {
    pub(crate) fn new_decoder(self) -> VariantDecoder {
        match self {
            VariantEncoding::Utf8 => VariantDecoder::Utf8(Utf8Decoder::new()),
            VariantEncoding::SingleByte(table, _, _) => {
                VariantDecoder::SingleByte(SingleByteDecoder::new(table))
            }
            VariantEncoding::Utf16Be => VariantDecoder::Utf16(Utf16Decoder::new(true)),
            VariantEncoding::Utf16Le => VariantDecoder::Utf16(Utf16Decoder::new(false)),
            // GBK and gb18030 share a decoder.
            VariantEncoding::Gb18030 { .. } => VariantDecoder::Gb18030(Gb18030Decoder::new()),
            VariantEncoding::Big5 => VariantDecoder::Big5(Big5Decoder::new()),
            VariantEncoding::EucJp => VariantDecoder::EucJp(EucJpDecoder::new()),
            VariantEncoding::Iso2022Jp => VariantDecoder::Iso2022Jp(Iso2022JpDecoder::new()),
            VariantEncoding::ShiftJis => VariantDecoder::ShiftJis(ShiftJisDecoder::new()),
            VariantEncoding::EucKr => VariantDecoder::EucKr(EucKrDecoder::new()),
            VariantEncoding::Replacement => {
                VariantDecoder::Replacement(ReplacementDecoder::default())
            }
            VariantEncoding::XUserDefined => VariantDecoder::XUserDefined(XUserDefinedDecoder),
        }
    }

    /// The encoder for this encoding.
    ///
    /// `replacement`, UTF-16BE and UTF-16LE have no encoder of their own; callers
    /// are expected to have mapped them to UTF-8 with `get an output encoding`
    /// first, which [`Encoding::new_encoder`](crate::Encoding::new_encoder) does.
    pub(crate) fn new_encoder(self) -> VariantEncoder {
        match self {
            VariantEncoding::SingleByte(_, code_points, bytes) => {
                VariantEncoder::SingleByte(SingleByteEncoder::new(code_points, bytes))
            }
            VariantEncoding::Gb18030 { is_gbk } => {
                VariantEncoder::Gb18030(Gb18030Encoder::new(is_gbk))
            }
            VariantEncoding::Big5 => VariantEncoder::Big5(Big5Encoder),
            VariantEncoding::EucJp => VariantEncoder::EucJp(EucJpEncoder),
            VariantEncoding::Iso2022Jp => VariantEncoder::Iso2022Jp(Iso2022JpEncoder::default()),
            VariantEncoding::ShiftJis => VariantEncoder::ShiftJis(ShiftJisEncoder),
            VariantEncoding::EucKr => VariantEncoder::EucKr(EucKrEncoder),
            VariantEncoding::XUserDefined => VariantEncoder::XUserDefined(XUserDefinedEncoder),
            VariantEncoding::Utf8
            | VariantEncoding::Utf16Be
            | VariantEncoding::Utf16Le
            | VariantEncoding::Replacement => VariantEncoder::Utf8(Utf8Encoder),
        }
    }

    pub(crate) fn is_single_byte(self) -> bool {
        matches!(
            self,
            VariantEncoding::SingleByte(..) | VariantEncoding::XUserDefined
        )
    }

    /// Whether a byte below 0x80 always stands for the corresponding ASCII
    /// character, in every state the decoder can be in.
    pub(crate) fn is_ascii_compatible(self) -> bool {
        !matches!(
            self,
            VariantEncoding::Utf16Be
                | VariantEncoding::Utf16Le
                | VariantEncoding::Replacement
                | VariantEncoding::Iso2022Jp
        )
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum VariantDecoder {
    Utf8(Utf8Decoder),
    SingleByte(SingleByteDecoder),
    Utf16(Utf16Decoder),
    Gb18030(Gb18030Decoder),
    Big5(Big5Decoder),
    EucJp(EucJpDecoder),
    Iso2022Jp(Iso2022JpDecoder),
    ShiftJis(ShiftJisDecoder),
    EucKr(EucKrDecoder),
    Replacement(ReplacementDecoder),
    XUserDefined(XUserDefinedDecoder),
}

impl VariantDecoder {
    pub(crate) fn decode(
        &mut self,
        src: &[u8],
        sink: &mut ByteSink,
        last: bool,
    ) -> (DecoderResult, usize) {
        match self {
            VariantDecoder::Utf8(d) => d.decode(src, sink, last),
            VariantDecoder::SingleByte(d) => d.decode(src, sink),
            VariantDecoder::Utf16(d) => d.decode(src, sink, last),
            VariantDecoder::Gb18030(d) => d.decode(src, sink, last),
            VariantDecoder::Big5(d) => d.decode(src, sink, last),
            VariantDecoder::EucJp(d) => d.decode(src, sink, last),
            VariantDecoder::Iso2022Jp(d) => d.decode(src, sink, last),
            VariantDecoder::ShiftJis(d) => d.decode(src, sink, last),
            VariantDecoder::EucKr(d) => d.decode(src, sink, last),
            VariantDecoder::Replacement(d) => d.decode(src, sink),
            VariantDecoder::XUserDefined(d) => d.decode(src, sink),
        }
    }

    /// An upper bound on the UTF-8 bytes `byte_length` input bytes can produce.
    ///
    /// Every decoder emits at most one scalar value per input byte, except Big5,
    /// which can emit a base letter plus a combining mark for a two-byte sequence.
    /// Three bytes per input byte covers the worst case of a stream of errors, and
    /// the constant covers state carried in from an earlier call.
    pub(crate) fn max_utf8_buffer_length(&self, byte_length: usize) -> Option<usize> {
        byte_length.checked_mul(3)?.checked_add(8)
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum VariantEncoder {
    Utf8(Utf8Encoder),
    SingleByte(SingleByteEncoder),
    Gb18030(Gb18030Encoder),
    Big5(Big5Encoder),
    EucJp(EucJpEncoder),
    Iso2022Jp(Iso2022JpEncoder),
    ShiftJis(ShiftJisEncoder),
    EucKr(EucKrEncoder),
    XUserDefined(XUserDefinedEncoder),
}

impl VariantEncoder {
    pub(crate) fn encode(
        &mut self,
        src: &str,
        sink: &mut ByteSink,
        last: bool,
    ) -> (EncoderResult, usize) {
        match self {
            VariantEncoder::Utf8(e) => e.encode(src, sink),
            VariantEncoder::SingleByte(e) => e.encode(src, sink),
            VariantEncoder::Gb18030(e) => e.encode(src, sink),
            VariantEncoder::Big5(e) => e.encode(src, sink),
            VariantEncoder::EucJp(e) => e.encode(src, sink),
            VariantEncoder::Iso2022Jp(e) => e.encode(src, sink, last),
            VariantEncoder::ShiftJis(e) => e.encode(src, sink),
            VariantEncoder::EucKr(e) => e.encode(src, sink),
            VariantEncoder::XUserDefined(e) => e.encode(src, sink),
        }
    }

    /// An upper bound on the bytes `byte_length` UTF-8 bytes can encode to,
    /// assuming every character is representable.
    pub(crate) fn max_buffer_length_from_utf8_without_replacement(
        &self,
        byte_length: usize,
    ) -> Option<usize> {
        match self {
            // An escape costs three bytes and can be needed before every character,
            // plus a final one to return to ASCII.
            VariantEncoder::Iso2022Jp(_) => byte_length.checked_mul(4)?.checked_add(3),
            // A two-byte UTF-8 character can need a four-byte gb18030 sequence.
            VariantEncoder::Gb18030(_) => byte_length.checked_mul(2)?.checked_add(4),
            // Everywhere else a character never encodes to more bytes than its
            // UTF-8 form takes.
            _ => byte_length.checked_add(4),
        }
    }

    /// An upper bound that also allows for every character being replaced by a
    /// decimal numeric character reference, the longest of which is 10 bytes.
    pub(crate) fn max_buffer_length_from_utf8(&self, byte_length: usize) -> Option<usize> {
        self.max_buffer_length_from_utf8_without_replacement(byte_length)?
            .checked_add(byte_length.checked_mul(7)?)
    }
}
