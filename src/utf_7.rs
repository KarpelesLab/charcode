//! UTF-7, as defined by RFC 2152.
//!
//! The Encoding Standard leaves UTF-7 out on purpose: because a run of ASCII
//! bytes can stand for arbitrary text, it has repeatedly been used to smuggle
//! markup past a filter that only inspects the bytes.  It is here for reading
//! archived mail and old IMAP folder names, and
//! [`Encoding::for_whatwg_label`](crate::Encoding::for_whatwg_label) will never
//! return it, so enabling this cannot widen what a label off the network can
//! select.
//!
//! The encoder is deliberately conservative: it emits literally only RFC 2152's
//! Set D plus space, tab, carriage return and line feed, and encodes everything
//! else, so its output cannot be read two ways.

use crate::result::{DecoderResult, EncoderResult};
use crate::sink::{ByteSink, DECODER_HEADROOM, ENCODER_HEADROOM};

const BASE64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// The 6-bit value of a modified-base64 byte.
fn base64_value(byte: u8) -> Option<u32> {
    let value = match byte {
        b'A'..=b'Z' => byte - b'A',
        b'a'..=b'z' => byte - b'a' + 26,
        b'0'..=b'9' => byte - b'0' + 52,
        b'+' => 62,
        b'/' => 63,
        _ => return None,
    };
    Some(u32::from(value))
}

/// RFC 2152's Set D, plus the whitespace that may always appear directly.
///
/// Set O — the punctuation RFC 2152 says *may* be written directly — is
/// deliberately excluded from the encoder, though the decoder accepts it.
fn is_direct(c: char) -> bool {
    c.is_ascii_alphanumeric()
        || matches!(c, '\'' | '(' | ')' | ',' | '-' | '.' | '/' | ':' | '?')
        || matches!(c, ' ' | '\t' | '\r' | '\n')
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Utf7Decoder {
    in_base64: bool,
    /// Whether any base64 byte has been seen since the `+`, which is what tells
    /// the `+-` escape from the end of a run.
    seen_base64: bool,
    bits: u32,
    bit_count: u8,
    high_surrogate: Option<u16>,
}

impl Utf7Decoder {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Leaves base64 mode.  Returns false if the run ended mid-character or with
    /// non-zero padding bits, both of which RFC 2152 forbids.
    fn end_run(&mut self) -> bool {
        let clean = self.bits == 0 && self.bit_count < 6 && self.high_surrogate.is_none();
        self.in_base64 = false;
        self.seen_base64 = false;
        self.bits = 0;
        self.bit_count = 0;
        self.high_surrogate = None;
        clean
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
                if self.in_base64 && !self.end_run() {
                    return (DecoderResult::Malformed(1), read);
                }
                return (DecoderResult::InputEmpty, read);
            };
            read += 1;

            if !self.in_base64 {
                if byte == b'+' {
                    self.in_base64 = true;
                    self.seen_base64 = false;
                } else if byte.is_ascii() {
                    sink.write_byte(byte);
                } else {
                    return (DecoderResult::Malformed(1), read);
                }
                continue;
            }

            if let Some(value) = base64_value(byte) {
                self.seen_base64 = true;
                self.bits = (self.bits << 6) | value;
                self.bit_count += 6;
                if self.bit_count < 16 {
                    continue;
                }
                self.bit_count -= 16;
                let unit = (self.bits >> self.bit_count) as u16;
                self.bits &= (1 << self.bit_count) - 1;
                match self.high_surrogate.take() {
                    Some(high) => {
                        if !(0xDC00..=0xDFFF).contains(&unit) {
                            self.end_run();
                            return (DecoderResult::Malformed(1), read);
                        }
                        let scalar = 0x1_0000
                            + ((u32::from(high) - 0xD800) << 10)
                            + (u32::from(unit) - 0xDC00);
                        sink.write_code_point(scalar);
                    }
                    None => match unit {
                        0xD800..=0xDBFF => self.high_surrogate = Some(unit),
                        0xDC00..=0xDFFF => {
                            self.end_run();
                            return (DecoderResult::Malformed(1), read);
                        }
                        _ => sink.write_code_point(u32::from(unit)),
                    },
                }
                continue;
            }

            // Anything else ends the run.
            let escaped_plus = !self.seen_base64 && byte == b'-';
            let absorbed = byte == b'-';
            let clean = self.end_run();
            if escaped_plus {
                sink.write_byte(b'+');
                continue;
            }
            if !clean {
                return (DecoderResult::Malformed(1), read);
            }
            if !absorbed {
                // The byte belongs to the text that follows the run.
                read -= 1;
            }
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Utf7Encoder {
    in_base64: bool,
    bits: u32,
    bit_count: u8,
    /// The trailing surrogate of a supplementary character whose leading
    /// surrogate has already been written.
    pending_low: Option<u16>,
}

impl Utf7Encoder {
    /// Writes one UTF-16 code unit, which costs at most three base64 bytes.
    fn push_unit(&mut self, unit: u16, sink: &mut ByteSink) {
        self.bits = (self.bits << 16) | u32::from(unit);
        self.bit_count += 16;
        while self.bit_count >= 6 {
            self.bit_count -= 6;
            let index = (self.bits >> self.bit_count) & 0x3F;
            sink.write_byte(BASE64[index as usize]);
        }
        self.bits &= (1 << self.bit_count) - 1;
    }

    /// Flushes any partial base64 group and closes the run with `-`.
    fn end_run(&mut self, sink: &mut ByteSink) {
        if self.bit_count > 0 {
            let index = (self.bits << (6 - self.bit_count)) & 0x3F;
            sink.write_byte(BASE64[index as usize]);
        }
        sink.write_byte(b'-');
        self.in_base64 = false;
        self.bits = 0;
        self.bit_count = 0;
    }

    pub(crate) fn encode(
        &mut self,
        src: &str,
        sink: &mut ByteSink,
        last: bool,
    ) -> (EncoderResult, usize) {
        let mut read = 0usize;
        loop {
            if !sink.has_room(ENCODER_HEADROOM) {
                return (EncoderResult::OutputFull, read);
            }

            // Finish a supplementary character split across iterations, so that
            // one pass never writes more than the headroom.
            if let Some(low) = self.pending_low.take() {
                self.push_unit(low, sink);
                continue;
            }

            let Some(c) = src[read..].chars().next() else {
                if last && self.in_base64 {
                    self.end_run(sink);
                }
                return (EncoderResult::InputEmpty, read);
            };

            if is_direct(c) {
                if self.in_base64 {
                    self.end_run(sink);
                    continue;
                }
                sink.write_byte(c as u8);
                read += c.len_utf8();
                continue;
            }
            if c == '+' && !self.in_base64 {
                sink.write_slice(b"+-");
                read += 1;
                continue;
            }
            if !self.in_base64 {
                sink.write_byte(b'+');
                self.in_base64 = true;
            }
            read += c.len_utf8();
            let mut units = [0u16; 2];
            let units = c.encode_utf16(&mut units);
            self.push_unit(units[0], sink);
            if let Some(&low) = units.get(1) {
                self.pending_low = Some(low);
            }
        }
    }
}
