//! Dispatch from an [`Encoding`](crate::Encoding) to its decoder and encoder.
//!
//! The variants are enums rather than trait objects so that a `Decoder` stays a
//! plain value: constructing one never allocates, which matters for the streaming
//! API and for `no_std` users.

#[cfg(feature = "big5")]
use crate::big5::{Big5Decoder, Big5Encoder};
#[cfg(feature = "big5")]
use crate::big5_1984::{Big5_1984Decoder, Big5_1984Encoder};
#[cfg(feature = "gb18030")]
use crate::euc_cn::{Gb2312Decoder, Gb2312Encoder};
#[cfg(feature = "euc-jp")]
use crate::euc_jp::{EucJpDecoder, EucJpEncoder};
#[cfg(feature = "euc-kr")]
use crate::euc_kr::{EucKrDecoder, EucKrEncoder};
#[cfg(feature = "full-byte")]
use crate::full_byte::{FullByteDecoder, FullByteEncoder};
#[cfg(feature = "gb18030")]
use crate::gb18030::{Gb18030Decoder, Gb18030Encoder};
use crate::identity::Identity;
#[cfg(feature = "iso-2022-jp")]
use crate::iso_2022_jp::{Iso2022JpDecoder, Iso2022JpEncoder};
#[cfg(feature = "iso-2022-kr")]
use crate::iso_2022_kr::{Iso2022KrDecoder, Iso2022KrEncoder};
use crate::replacement::ReplacementDecoder;
use crate::result::{DecoderResult, EncoderResult};
#[cfg(feature = "shift-jis")]
use crate::shift_jis::{ShiftJisDecoder, ShiftJisEncoder};
#[cfg(feature = "shift-jis")]
use crate::shift_jis_1997::{ShiftJis1997Decoder, ShiftJis1997Encoder};
#[cfg(feature = "half-byte")]
use crate::single_byte::{SingleByteDecoder, SingleByteEncoder};
use crate::sink::ByteSink;
#[cfg(feature = "unicode-extras")]
use crate::utf_7::{Utf7Decoder, Utf7Encoder};
use crate::utf_8::{Utf8Decoder, Utf8Encoder};
use crate::utf_16::Utf16Decoder;
#[cfg(feature = "unicode-extras")]
use crate::utf_32::{Utf32Decoder, Utf32Encoder};
use crate::x_user_defined::{XUserDefinedDecoder, XUserDefinedEncoder};

/// Which algorithm an encoding uses, plus the index tables it needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum VariantEncoding {
    Utf8,
    /// Decode table, then the encode table's code points and their bytes.
    #[cfg(feature = "half-byte")]
    SingleByte(&'static [u16; 128], &'static [u16], &'static [u8]),
    /// The same, for an encoding whose low half is not plain ASCII.
    #[cfg(feature = "full-byte")]
    FullByte(&'static [u16; 256], &'static [u16], &'static [u8]),
    Utf16Be,
    Utf16Le,
    /// ISO-8859-1 and US-ASCII: byte `n` is U+`n`, up to the limit.
    Identity(Identity),
    #[cfg(feature = "unicode-extras")]
    Utf32Be,
    #[cfg(feature = "unicode-extras")]
    Utf32Le,
    #[cfg(feature = "unicode-extras")]
    Utf7,
    #[cfg(feature = "gb18030")]
    Gb2312,
    #[cfg(feature = "gb18030")]
    Gb18030 {
        is_gbk: bool,
    },
    #[cfg(feature = "big5")]
    Big5,
    /// Big5 as standardised, rather than the standard's extended index.
    #[cfg(feature = "big5")]
    Big5_1984,
    #[cfg(feature = "shift-jis")]
    ShiftJis1997,
    #[cfg(feature = "euc-jp")]
    EucJp,
    #[cfg(feature = "iso-2022-jp")]
    Iso2022Jp,
    #[cfg(feature = "iso-2022-kr")]
    Iso2022Kr,
    #[cfg(feature = "shift-jis")]
    ShiftJis,
    #[cfg(feature = "euc-kr")]
    EucKr,
    Replacement,
    XUserDefined,
}

impl VariantEncoding {
    pub(crate) fn new_decoder(self) -> VariantDecoder {
        match self {
            VariantEncoding::Utf8 => VariantDecoder::Utf8(Utf8Decoder::new()),
            #[cfg(feature = "half-byte")]
            VariantEncoding::SingleByte(table, _, _) => {
                VariantDecoder::SingleByte(SingleByteDecoder::new(table))
            }
            #[cfg(feature = "full-byte")]
            VariantEncoding::FullByte(table, _, _) => {
                VariantDecoder::FullByte(FullByteDecoder::new(table))
            }
            VariantEncoding::Identity(map) => VariantDecoder::Identity(map),
            VariantEncoding::Utf16Be => VariantDecoder::Utf16(Utf16Decoder::new(true)),
            VariantEncoding::Utf16Le => VariantDecoder::Utf16(Utf16Decoder::new(false)),
            #[cfg(feature = "unicode-extras")]
            VariantEncoding::Utf32Be => VariantDecoder::Utf32(Utf32Decoder::new(true)),
            #[cfg(feature = "unicode-extras")]
            VariantEncoding::Utf32Le => VariantDecoder::Utf32(Utf32Decoder::new(false)),
            #[cfg(feature = "unicode-extras")]
            VariantEncoding::Utf7 => VariantDecoder::Utf7(Utf7Decoder::new()),
            // GBK and gb18030 share a decoder.
            #[cfg(feature = "gb18030")]
            VariantEncoding::Gb2312 => VariantDecoder::Gb2312(Gb2312Decoder::new()),
            #[cfg(feature = "gb18030")]
            VariantEncoding::Gb18030 { .. } => VariantDecoder::Gb18030(Gb18030Decoder::new()),
            #[cfg(feature = "big5")]
            VariantEncoding::Big5 => VariantDecoder::Big5(Big5Decoder::new()),
            #[cfg(feature = "big5")]
            VariantEncoding::Big5_1984 => VariantDecoder::Big5_1984(Big5_1984Decoder::new()),
            #[cfg(feature = "shift-jis")]
            VariantEncoding::ShiftJis1997 => {
                VariantDecoder::ShiftJis1997(ShiftJis1997Decoder::new())
            }
            #[cfg(feature = "euc-jp")]
            VariantEncoding::EucJp => VariantDecoder::EucJp(EucJpDecoder::new()),
            #[cfg(feature = "iso-2022-jp")]
            VariantEncoding::Iso2022Jp => VariantDecoder::Iso2022Jp(Iso2022JpDecoder::new()),
            #[cfg(feature = "iso-2022-kr")]
            VariantEncoding::Iso2022Kr => VariantDecoder::Iso2022Kr(Iso2022KrDecoder::new()),
            #[cfg(feature = "shift-jis")]
            VariantEncoding::ShiftJis => VariantDecoder::ShiftJis(ShiftJisDecoder::new()),
            #[cfg(feature = "euc-kr")]
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
            #[cfg(feature = "half-byte")]
            VariantEncoding::SingleByte(_, code_points, bytes) => {
                VariantEncoder::SingleByte(SingleByteEncoder::new(code_points, bytes))
            }
            #[cfg(feature = "full-byte")]
            VariantEncoding::FullByte(_, code_points, bytes) => {
                VariantEncoder::FullByte(FullByteEncoder::new(code_points, bytes))
            }
            #[cfg(feature = "gb18030")]
            VariantEncoding::Gb2312 => VariantEncoder::Gb2312(Gb2312Encoder),
            #[cfg(feature = "gb18030")]
            VariantEncoding::Gb18030 { is_gbk } => {
                VariantEncoder::Gb18030(Gb18030Encoder::new(is_gbk))
            }
            #[cfg(feature = "big5")]
            VariantEncoding::Big5 => VariantEncoder::Big5(Big5Encoder),
            #[cfg(feature = "big5")]
            VariantEncoding::Big5_1984 => VariantEncoder::Big5_1984(Big5_1984Encoder),
            #[cfg(feature = "shift-jis")]
            VariantEncoding::ShiftJis1997 => VariantEncoder::ShiftJis1997(ShiftJis1997Encoder),
            #[cfg(feature = "euc-jp")]
            VariantEncoding::EucJp => VariantEncoder::EucJp(EucJpEncoder),
            #[cfg(feature = "iso-2022-jp")]
            VariantEncoding::Iso2022Jp => VariantEncoder::Iso2022Jp(Iso2022JpEncoder::default()),
            #[cfg(feature = "iso-2022-kr")]
            VariantEncoding::Iso2022Kr => VariantEncoder::Iso2022Kr(Iso2022KrEncoder::default()),
            #[cfg(feature = "shift-jis")]
            VariantEncoding::ShiftJis => VariantEncoder::ShiftJis(ShiftJisEncoder),
            #[cfg(feature = "euc-kr")]
            VariantEncoding::EucKr => VariantEncoder::EucKr(EucKrEncoder),
            VariantEncoding::Identity(map) => VariantEncoder::Identity(map),
            VariantEncoding::XUserDefined => VariantEncoder::XUserDefined(XUserDefinedEncoder),
            #[cfg(feature = "unicode-extras")]
            VariantEncoding::Utf32Be => VariantEncoder::Utf32(Utf32Encoder::new(true)),
            #[cfg(feature = "unicode-extras")]
            VariantEncoding::Utf32Le => VariantEncoder::Utf32(Utf32Encoder::new(false)),
            #[cfg(feature = "unicode-extras")]
            VariantEncoding::Utf7 => VariantEncoder::Utf7(Utf7Encoder::default()),
            VariantEncoding::Utf8
            | VariantEncoding::Utf16Be
            | VariantEncoding::Utf16Le
            | VariantEncoding::Replacement => VariantEncoder::Utf8(Utf8Encoder),
        }
    }

    // These are `match` rather than `matches!` because a pattern cannot carry a
    // `#[cfg]`, and the variants they name come and go with the features.
    pub(crate) fn is_single_byte(self) -> bool {
        match self {
            #[cfg(feature = "half-byte")]
            VariantEncoding::SingleByte(..) => true,
            #[cfg(feature = "full-byte")]
            VariantEncoding::FullByte(..) => true,
            VariantEncoding::XUserDefined | VariantEncoding::Identity(_) => true,
            _ => false,
        }
    }

    /// Whether a byte below 0x80 always stands for the corresponding ASCII
    /// character, in every state the decoder can be in.
    pub(crate) fn is_ascii_compatible(self) -> bool {
        match self {
            VariantEncoding::Utf16Be | VariantEncoding::Utf16Le | VariantEncoding::Replacement => {
                false
            }
            #[cfg(feature = "iso-2022-jp")]
            VariantEncoding::Iso2022Jp => false,
            #[cfg(feature = "iso-2022-kr")]
            VariantEncoding::Iso2022Kr => false,
            // A full-byte table may reassign bytes below 0x80, and the EBCDIC
            // pages permute the range entirely.
            #[cfg(feature = "full-byte")]
            VariantEncoding::FullByte(..) => false,
            // JIS X 0201's Roman set is not ASCII: 0x5C is the yen sign and
            // 0x7E the overline.
            #[cfg(feature = "shift-jis")]
            VariantEncoding::ShiftJis1997 => false,
            // UTF-32 is not byte-oriented, and a run of ASCII bytes in UTF-7
            // can stand for arbitrary text.
            #[cfg(feature = "unicode-extras")]
            VariantEncoding::Utf32Be | VariantEncoding::Utf32Le | VariantEncoding::Utf7 => false,
            _ => true,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum VariantDecoder {
    Utf8(Utf8Decoder),
    #[cfg(feature = "half-byte")]
    SingleByte(SingleByteDecoder),
    #[cfg(feature = "full-byte")]
    FullByte(FullByteDecoder),
    Utf16(Utf16Decoder),
    Identity(Identity),
    #[cfg(feature = "unicode-extras")]
    Utf32(Utf32Decoder),
    #[cfg(feature = "unicode-extras")]
    Utf7(Utf7Decoder),
    #[cfg(feature = "gb18030")]
    Gb2312(Gb2312Decoder),
    #[cfg(feature = "gb18030")]
    Gb18030(Gb18030Decoder),
    #[cfg(feature = "big5")]
    Big5(Big5Decoder),
    #[cfg(feature = "big5")]
    Big5_1984(Big5_1984Decoder),
    #[cfg(feature = "shift-jis")]
    ShiftJis1997(ShiftJis1997Decoder),
    #[cfg(feature = "euc-jp")]
    EucJp(EucJpDecoder),
    #[cfg(feature = "iso-2022-jp")]
    Iso2022Jp(Iso2022JpDecoder),
    #[cfg(feature = "iso-2022-kr")]
    Iso2022Kr(Iso2022KrDecoder),
    #[cfg(feature = "shift-jis")]
    ShiftJis(ShiftJisDecoder),
    #[cfg(feature = "euc-kr")]
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
            #[cfg(feature = "half-byte")]
            VariantDecoder::SingleByte(d) => d.decode(src, sink),
            #[cfg(feature = "full-byte")]
            VariantDecoder::FullByte(d) => d.decode(src, sink),
            VariantDecoder::Utf16(d) => d.decode(src, sink, last),
            VariantDecoder::Identity(d) => d.decode(src, sink),
            #[cfg(feature = "unicode-extras")]
            VariantDecoder::Utf32(d) => d.decode(src, sink, last),
            #[cfg(feature = "unicode-extras")]
            VariantDecoder::Utf7(d) => d.decode(src, sink, last),
            #[cfg(feature = "gb18030")]
            VariantDecoder::Gb2312(d) => d.decode(src, sink, last),
            #[cfg(feature = "gb18030")]
            VariantDecoder::Gb18030(d) => d.decode(src, sink, last),
            #[cfg(feature = "big5")]
            VariantDecoder::Big5(d) => d.decode(src, sink, last),
            #[cfg(feature = "big5")]
            VariantDecoder::Big5_1984(d) => d.decode(src, sink, last),
            #[cfg(feature = "shift-jis")]
            VariantDecoder::ShiftJis1997(d) => d.decode(src, sink, last),
            #[cfg(feature = "euc-jp")]
            VariantDecoder::EucJp(d) => d.decode(src, sink, last),
            #[cfg(feature = "iso-2022-jp")]
            VariantDecoder::Iso2022Jp(d) => d.decode(src, sink, last),
            #[cfg(feature = "iso-2022-kr")]
            VariantDecoder::Iso2022Kr(d) => d.decode(src, sink, last),
            #[cfg(feature = "shift-jis")]
            VariantDecoder::ShiftJis(d) => d.decode(src, sink, last),
            #[cfg(feature = "euc-kr")]
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
    Identity(Identity),
    #[cfg(feature = "unicode-extras")]
    Utf32(Utf32Encoder),
    #[cfg(feature = "unicode-extras")]
    Utf7(Utf7Encoder),
    #[cfg(feature = "half-byte")]
    SingleByte(SingleByteEncoder),
    #[cfg(feature = "full-byte")]
    FullByte(FullByteEncoder),
    #[cfg(feature = "gb18030")]
    Gb2312(Gb2312Encoder),
    #[cfg(feature = "gb18030")]
    Gb18030(Gb18030Encoder),
    #[cfg(feature = "big5")]
    Big5(Big5Encoder),
    #[cfg(feature = "big5")]
    Big5_1984(Big5_1984Encoder),
    #[cfg(feature = "shift-jis")]
    ShiftJis1997(ShiftJis1997Encoder),
    #[cfg(feature = "euc-jp")]
    EucJp(EucJpEncoder),
    #[cfg(feature = "iso-2022-jp")]
    Iso2022Jp(Iso2022JpEncoder),
    #[cfg(feature = "iso-2022-kr")]
    Iso2022Kr(Iso2022KrEncoder),
    #[cfg(feature = "shift-jis")]
    ShiftJis(ShiftJisEncoder),
    #[cfg(feature = "euc-kr")]
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
        // Only ISO-2022-JP needs to know, and it may be compiled out.
        let _ = last;
        match self {
            VariantEncoder::Utf8(e) => e.encode(src, sink),
            VariantEncoder::Identity(e) => e.encode(src, sink),
            #[cfg(feature = "unicode-extras")]
            VariantEncoder::Utf32(e) => e.encode(src, sink),
            #[cfg(feature = "unicode-extras")]
            VariantEncoder::Utf7(e) => e.encode(src, sink, last),
            #[cfg(feature = "half-byte")]
            VariantEncoder::SingleByte(e) => e.encode(src, sink),
            #[cfg(feature = "full-byte")]
            VariantEncoder::FullByte(e) => e.encode(src, sink),
            #[cfg(feature = "gb18030")]
            VariantEncoder::Gb2312(e) => e.encode(src, sink),
            #[cfg(feature = "gb18030")]
            VariantEncoder::Gb18030(e) => e.encode(src, sink),
            #[cfg(feature = "big5")]
            VariantEncoder::Big5(e) => e.encode(src, sink),
            #[cfg(feature = "big5")]
            VariantEncoder::Big5_1984(e) => e.encode(src, sink),
            #[cfg(feature = "shift-jis")]
            VariantEncoder::ShiftJis1997(e) => e.encode(src, sink),
            #[cfg(feature = "euc-jp")]
            VariantEncoder::EucJp(e) => e.encode(src, sink),
            #[cfg(feature = "iso-2022-jp")]
            VariantEncoder::Iso2022Jp(e) => e.encode(src, sink, last),
            #[cfg(feature = "iso-2022-kr")]
            VariantEncoder::Iso2022Kr(e) => e.encode(src, sink, last),
            #[cfg(feature = "shift-jis")]
            VariantEncoder::ShiftJis(e) => e.encode(src, sink),
            #[cfg(feature = "euc-kr")]
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
            #[cfg(feature = "iso-2022-jp")]
            VariantEncoder::Iso2022Jp(_) => byte_length.checked_mul(4)?.checked_add(3),
            // A shift before every ASCII byte, plus the designator and a final
            // shift back.
            #[cfg(feature = "iso-2022-kr")]
            VariantEncoder::Iso2022Kr(_) => byte_length.checked_mul(2)?.checked_add(5),
            // A two-byte UTF-8 character can need a four-byte gb18030 sequence.
            #[cfg(feature = "gb18030")]
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
