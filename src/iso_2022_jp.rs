//! ISO-2022-JP, the one stateful encoding in the standard.
//!
//! Both halves are escape-driven state machines: the decoder tracks the escape it
//! is in the middle of parsing separately from the mode that escape selected, and
//! the encoder emits an escape whenever the character at hand needs a different
//! mode than the one currently active.

use crate::ascii::ascii_prefix_len_str;
use crate::index;
use crate::result::{DecoderResult, EncoderResult};
use crate::sink::{ByteSink, DECODER_HEADROOM, ENCODER_HEADROOM, Pushback};
use crate::tables::jis::{
    ISO_2022_JP_KATAKANA, JIS0208_DECODE, JIS0208_ENCODE_CODE_POINTS, JIS0208_ENCODE_POINTERS,
};

const ESCAPE_ASCII: [u8; 3] = [0x1B, 0x28, 0x42];
const ESCAPE_ROMAN: [u8; 3] = [0x1B, 0x28, 0x4A];
const ESCAPE_JIS0208: [u8; 3] = [0x1B, 0x24, 0x42];

/// The mode an escape sequence selects, which the decoder returns to after a
/// malformed escape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Ascii,
    Roman,
    Katakana,
    LeadingByte,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DecoderState {
    Mode(Mode),
    TrailingByte,
    EscapeStart,
    Escape,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Iso2022JpDecoder {
    state: DecoderState,
    output_mode: Mode,
    leading: u8,
    /// Whether an escape has been seen with no output since; two escapes in a row
    /// are an error, which is what keeps escape sequences from being smuggled
    /// through a filter that only inspects the decoded text.
    output: bool,
    pending: Pushback,
}

impl Default for Iso2022JpDecoder {
    fn default() -> Self {
        Iso2022JpDecoder {
            state: DecoderState::Mode(Mode::Ascii),
            output_mode: Mode::Ascii,
            leading: 0,
            output: false,
            pending: Pushback::default(),
        }
    }
}

impl Iso2022JpDecoder {
    pub(crate) fn new() -> Self {
        Self::default()
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

            let (byte, from_pending) = match self.pending.next() {
                Some(byte) => (Some(byte), true),
                None => {
                    let byte = src.get(read).copied();
                    if byte.is_some() {
                        read += 1;
                    }
                    (byte, false)
                }
            };
            if byte.is_none() && !last {
                return (DecoderResult::InputEmpty, read);
            }

            match self.state {
                DecoderState::Mode(Mode::Ascii) => match byte {
                    Some(0x1B) => self.state = DecoderState::EscapeStart,
                    Some(b) if b < 0x80 && b != 0x0E && b != 0x0F => {
                        self.output = false;
                        sink.write_byte(b);
                    }
                    None => return (DecoderResult::InputEmpty, read),
                    Some(_) => {
                        self.output = false;
                        return (DecoderResult::Malformed(1), read);
                    }
                },
                DecoderState::Mode(Mode::Roman) => match byte {
                    Some(0x1B) => self.state = DecoderState::EscapeStart,
                    Some(0x5C) => {
                        self.output = false;
                        sink.write_code_point(0x00A5);
                    }
                    Some(0x7E) => {
                        self.output = false;
                        sink.write_code_point(0x203E);
                    }
                    Some(b) if b < 0x80 && b != 0x0E && b != 0x0F => {
                        self.output = false;
                        sink.write_byte(b);
                    }
                    None => return (DecoderResult::InputEmpty, read),
                    Some(_) => {
                        self.output = false;
                        return (DecoderResult::Malformed(1), read);
                    }
                },
                DecoderState::Mode(Mode::Katakana) => match byte {
                    Some(0x1B) => self.state = DecoderState::EscapeStart,
                    Some(b @ 0x21..=0x5F) => {
                        self.output = false;
                        sink.write_code_point(0xFF61 - 0x21 + u32::from(b));
                    }
                    None => return (DecoderResult::InputEmpty, read),
                    Some(_) => {
                        self.output = false;
                        return (DecoderResult::Malformed(1), read);
                    }
                },
                DecoderState::Mode(Mode::LeadingByte) => match byte {
                    Some(0x1B) => self.state = DecoderState::EscapeStart,
                    Some(b @ 0x21..=0x7E) => {
                        self.output = false;
                        self.leading = b;
                        self.state = DecoderState::TrailingByte;
                    }
                    None => return (DecoderResult::InputEmpty, read),
                    Some(_) => {
                        self.output = false;
                        return (DecoderResult::Malformed(1), read);
                    }
                },
                DecoderState::TrailingByte => match byte {
                    Some(0x1B) => {
                        self.state = DecoderState::EscapeStart;
                        return (DecoderResult::Malformed(1), read);
                    }
                    Some(b @ 0x21..=0x7E) => {
                        self.state = DecoderState::Mode(Mode::LeadingByte);
                        let pointer =
                            (usize::from(self.leading) - 0x21) * 94 + (usize::from(b) - 0x21);
                        match index::code_point(&JIS0208_DECODE, pointer) {
                            Some(code_point) => sink.write_code_point(code_point),
                            None => return (DecoderResult::Malformed(2), read),
                        }
                    }
                    None => {
                        self.state = DecoderState::Mode(Mode::LeadingByte);
                        return (DecoderResult::Malformed(1), read);
                    }
                    Some(_) => {
                        self.state = DecoderState::Mode(Mode::LeadingByte);
                        return (DecoderResult::Malformed(2), read);
                    }
                },
                DecoderState::EscapeStart => match byte {
                    Some(b @ (0x24 | 0x28)) => {
                        self.leading = b;
                        self.state = DecoderState::Escape;
                    }
                    other => {
                        if other.is_some() {
                            if from_pending {
                                self.pending.unread();
                            } else {
                                read -= 1;
                            }
                        }
                        self.output = false;
                        self.state = DecoderState::Mode(self.output_mode);
                        return (DecoderResult::Malformed(1), read);
                    }
                },
                DecoderState::Escape => {
                    let leading = self.leading;
                    self.leading = 0;
                    let mode = match (leading, byte) {
                        (0x28, Some(0x42)) => Some(Mode::Ascii),
                        (0x28, Some(0x4A)) => Some(Mode::Roman),
                        (0x28, Some(0x49)) => Some(Mode::Katakana),
                        (0x24, Some(0x40 | 0x42)) => Some(Mode::LeadingByte),
                        _ => None,
                    };
                    if let Some(mode) = mode {
                        self.state = DecoderState::Mode(mode);
                        self.output_mode = mode;
                        let had_output = self.output;
                        self.output = true;
                        if had_output {
                            return (DecoderResult::Malformed(1), read);
                        }
                        continue;
                    }
                    debug_assert!(!from_pending, "escape bytes never come from the pushback");
                    if byte.is_some() {
                        read -= 1;
                    }
                    self.pending.set(&[leading]);
                    self.output = false;
                    self.state = DecoderState::Mode(self.output_mode);
                    return (DecoderResult::Malformed(1), read);
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EncoderState {
    Ascii,
    Roman,
    Jis0208,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Iso2022JpEncoder {
    state: EncoderState,
}

impl Default for Iso2022JpEncoder {
    fn default() -> Self {
        Iso2022JpEncoder {
            state: EncoderState::Ascii,
        }
    }
}

impl Iso2022JpEncoder {
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
            let rest = &src[read..];
            let Some(c) = rest.chars().next() else {
                if last && self.state != EncoderState::Ascii {
                    self.state = EncoderState::Ascii;
                    sink.write_slice(&ESCAPE_ASCII);
                }
                return (EncoderResult::InputEmpty, read);
            };

            if self.state == EncoderState::Ascii {
                let run = core::cmp::min(ascii_prefix_len_str(rest), sink.room());
                let run = rest.as_bytes()[..run]
                    .iter()
                    .position(|&b| b == 0x0E || b == 0x0F || b == 0x1B)
                    .unwrap_or(run);
                if run > 0 {
                    sink.write_slice(&rest.as_bytes()[..run]);
                    read += run;
                    continue;
                }
            }

            let scalar = u32::from(c);
            let width = c.len_utf8();

            if matches!(self.state, EncoderState::Ascii | EncoderState::Roman)
                && matches!(scalar, 0x000E | 0x000F | 0x001B)
            {
                // Reported as U+FFFD rather than the character itself so that a
                // caller substituting errors cannot be tricked into emitting an
                // escape sequence.
                read += width;
                return (EncoderResult::Unmappable(char::REPLACEMENT_CHARACTER), read);
            }
            if self.state == EncoderState::Ascii && c.is_ascii() {
                sink.write_byte(scalar as u8);
                read += width;
                continue;
            }
            if self.state == EncoderState::Roman {
                match scalar {
                    0x005C | 0x007E => {}
                    0x00A5 => {
                        sink.write_byte(0x5C);
                        read += width;
                        continue;
                    }
                    0x203E => {
                        sink.write_byte(0x7E);
                        read += width;
                        continue;
                    }
                    _ if c.is_ascii() => {
                        sink.write_byte(scalar as u8);
                        read += width;
                        continue;
                    }
                    _ => {}
                }
            }
            // The remaining arms switch mode and re-examine the same character.
            if c.is_ascii() && self.state != EncoderState::Ascii {
                self.state = EncoderState::Ascii;
                sink.write_slice(&ESCAPE_ASCII);
                continue;
            }
            if matches!(scalar, 0x00A5 | 0x203E) && self.state != EncoderState::Roman {
                self.state = EncoderState::Roman;
                sink.write_slice(&ESCAPE_ROMAN);
                continue;
            }

            let scalar = match scalar {
                0x2212 => 0xFF0D,
                halfwidth @ 0xFF61..=0xFF9F => {
                    u32::from(ISO_2022_JP_KATAKANA[(halfwidth - 0xFF61) as usize])
                }
                other => other,
            };

            let Some(pointer) = index::pointer(
                &JIS0208_ENCODE_CODE_POINTS,
                &JIS0208_ENCODE_POINTERS,
                scalar,
            ) else {
                if self.state == EncoderState::Jis0208 {
                    self.state = EncoderState::Ascii;
                    sink.write_slice(&ESCAPE_ASCII);
                    continue;
                }
                read += width;
                return (EncoderResult::Unmappable(c), read);
            };
            if self.state != EncoderState::Jis0208 {
                self.state = EncoderState::Jis0208;
                sink.write_slice(&ESCAPE_JIS0208);
                continue;
            }
            let pointer = u32::from(pointer);
            sink.write_slice(&[(pointer / 94 + 0x21) as u8, (pointer % 94 + 0x21) as u8]);
            read += width;
        }
    }
}
