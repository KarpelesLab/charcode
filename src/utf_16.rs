//! The shared UTF-16BE/LE decoder.  The standard defines no UTF-16 encoder; both
//! encodings encode as UTF-8 (see `get an output encoding`).

use crate::result::DecoderResult;
use crate::sink::{ByteSink, DECODER_HEADROOM};

#[derive(Debug, Clone, Copy)]
pub(crate) struct Utf16Decoder {
    big_endian: bool,
    leading_byte: Option<u8>,
    leading_surrogate: Option<u16>,
}

impl Utf16Decoder {
    pub(crate) fn new(big_endian: bool) -> Self {
        Utf16Decoder {
            big_endian,
            leading_byte: None,
            leading_surrogate: None,
        }
    }

    fn pending_len(&self) -> u8 {
        u8::from(self.leading_surrogate.is_some()) * 2 + u8::from(self.leading_byte.is_some())
    }

    pub(crate) fn decode(
        &mut self,
        src: &[u8],
        sink: &mut ByteSink,
        last: bool,
    ) -> (DecoderResult, usize) {
        let mut read = 0usize;
        loop {
            if !sink.has_room(DECODER_HEADROOM) {
                return (DecoderResult::OutputFull, read);
            }

            let Some(byte) = src.get(read).copied() else {
                if !last {
                    return (DecoderResult::InputEmpty, read);
                }
                let bad = self.pending_len();
                if bad == 0 {
                    return (DecoderResult::InputEmpty, read);
                }
                self.leading_byte = None;
                self.leading_surrogate = None;
                return (DecoderResult::Malformed(bad), read);
            };
            read += 1;

            let Some(lead) = self.leading_byte else {
                self.leading_byte = Some(byte);
                continue;
            };
            self.leading_byte = None;

            let code_unit = if self.big_endian {
                (u16::from(lead) << 8) | u16::from(byte)
            } else {
                (u16::from(byte) << 8) | u16::from(lead)
            };

            if let Some(high) = self.leading_surrogate {
                self.leading_surrogate = None;
                if (0xDC00..=0xDFFF).contains(&code_unit) {
                    let scalar = 0x1_0000
                        + ((u32::from(high) - 0xD800) << 10)
                        + (u32::from(code_unit) - 0xDC00);
                    sink.write_code_point(scalar);
                    continue;
                }
                // Put the two bytes of this code unit back; restoring them is the
                // same as never having consumed the second one.
                self.leading_byte = Some(lead);
                read -= 1;
                return (DecoderResult::Malformed(2), read);
            }

            match code_unit {
                0xD800..=0xDBFF => self.leading_surrogate = Some(code_unit),
                0xDC00..=0xDFFF => return (DecoderResult::Malformed(2), read),
                _ => sink.write_code_point(u32::from(code_unit)),
            }
        }
    }
}
