//! ISO-2022-KR (RFC 1557).
//!
//! The WHATWG Encoding Standard refuses this label, mapping it to
//! `replacement`, because a stateful 7-bit encoding whose shifted runs consume
//! ASCII bytes two at a time can hide markup from a filter that only inspects
//! bytes.  That refusal is preserved:
//! [`Encoding::for_whatwg_label`](crate::Encoding::for_whatwg_label) still
//! answers `replacement` here, so compiling this in cannot widen what a label
//! off the network selects.  It is reachable only through
//! [`Encoding::for_label`](crate::Encoding::for_label), for reading archived
//! Korean mail.
//!
//! The decoder is deliberately strict where implementations differ: an
//! unrecognized escape is an error rather than passed through as literal
//! bytes, which is the leniency the attack relies on.
//!
//! No table of its own — the doubly-shifted bytes are EUC-KR's with the high
//! bit cleared, so the KS X 1001 accessors in [`crate::euc_kr`] answer directly.

use crate::ascii::ascii_prefix_len_capped;
use crate::euc_kr::{ksx1001_bytes, ksx1001_code_point};
use crate::result::{DecoderResult, EncoderResult};
use crate::sink::{ByteSink, DECODER_HEADROOM, ENCODER_HEADROOM, Pushback};

/// `ESC $ ) C`, the only escape the encoding has.
const DESIGNATOR: [u8; 4] = [0x1B, 0x24, 0x29, 0x43];
const SO: u8 = 0x0E;
const SI: u8 = 0x0F;
const ESC: u8 = 0x1B;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    /// Reading single bytes, whatever the shift state.
    Ground,
    /// Shifted out, holding the lead byte of a pair.
    Trail(u8),
    /// Part-way through `ESC $ ) C`; the count is how much matched.
    Escape(u8),
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Iso2022KrDecoder {
    state: State,
    shifted: bool,
    pending: Pushback,
}

impl Default for Iso2022KrDecoder {
    fn default() -> Self {
        Iso2022KrDecoder {
            state: State::Ground,
            shifted: false,
            pending: Pushback::default(),
        }
    }
}

impl Iso2022KrDecoder {
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

            // Plain ASCII runs, which is most of any real message.
            if self.state == State::Ground && !self.shifted && self.pending.is_empty() {
                let rest = &src[read..];
                let run = ascii_prefix_len_capped(rest, sink.room());
                let run = rest[..run]
                    .iter()
                    .position(|&b| b == SO || b == SI || b == ESC)
                    .unwrap_or(run);
                if run > 0 {
                    sink.write_slice(&rest[..run]);
                    read += run;
                    continue;
                }
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

            match (self.state, byte) {
                // --- end of stream --------------------------------------
                // Ending shifted out is not an error; ending mid-character is.
                (State::Ground, None) => return (DecoderResult::InputEmpty, read),
                (State::Trail(_), None) => {
                    self.state = State::Ground;
                    return (DecoderResult::Malformed(1), read);
                }
                (State::Escape(matched), None) => {
                    self.restore_escape(matched);
                    return (DecoderResult::Malformed(1), read);
                }

                // --- part-way through the designator ---------------------
                (State::Escape(matched), Some(b)) if b == DESIGNATOR[usize::from(matched)] => {
                    self.state = if usize::from(matched) + 1 == DESIGNATOR.len() {
                        // Recognized, and a pure no-op: it neither shifts nor
                        // emits, and may appear any number of times.
                        State::Ground
                    } else {
                        State::Escape(matched + 1)
                    };
                }
                (State::Escape(matched), Some(_)) => {
                    // Not our escape.  Leave the offending byte for the next
                    // pass and re-read whatever of the prefix followed the ESC.
                    if from_pending {
                        self.pending.unread();
                    } else {
                        read -= 1;
                    }
                    self.restore_escape(matched);
                    return (DecoderResult::Malformed(1), read);
                }

                // --- the trail byte of a pair ----------------------------
                (State::Trail(_), Some(ESC)) => {
                    self.state = State::Escape(1);
                    return (DecoderResult::Malformed(1), read);
                }
                (State::Trail(lead), Some(b @ 0x21..=0x7E)) => {
                    self.state = State::Ground;
                    match ksx1001_code_point(lead, b) {
                        Some(code_point) => sink.write_code_point(code_point),
                        None => return (DecoderResult::Malformed(2), read),
                    }
                }
                (State::Trail(_), Some(_)) => {
                    self.state = State::Ground;
                    return (DecoderResult::Malformed(2), read);
                }

                // --- single bytes ---------------------------------------
                (State::Ground, Some(ESC)) => self.state = State::Escape(1),
                (State::Ground, Some(SO)) => self.shifted = true,
                (State::Ground, Some(SI)) => self.shifted = false,
                // A line ends in ASCII, per the RFC.  Dropping back rather than
                // erroring also stops an unterminated run swallowing the next
                // line, which is the shape the smuggling attacks take.
                (State::Ground, Some(b @ b'\n')) => {
                    self.shifted = false;
                    sink.write_byte(b);
                }
                (State::Ground, Some(b)) if !self.shifted && b.is_ascii() => sink.write_byte(b),
                (State::Ground, Some(b @ 0x21..=0x7E)) if self.shifted => {
                    self.state = State::Trail(b);
                }
                (State::Ground, Some(_)) => return (DecoderResult::Malformed(1), read),
            }
        }
    }

    /// Puts back the escape prefix that turned out not to be the designator.
    fn restore_escape(&mut self, matched: u8) {
        self.state = State::Ground;
        // `matched` counts the ESC itself, which is consumed by the error.
        if matched > 1 {
            self.pending.set(&DESIGNATOR[1..usize::from(matched)]);
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Iso2022KrEncoder {
    designated: bool,
    shifted: bool,
}

impl Iso2022KrEncoder {
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
                if last && self.shifted {
                    sink.write_byte(SI);
                    self.shifted = false;
                }
                return (EncoderResult::InputEmpty, read);
            };
            let width = c.len_utf8();

            // Never let a shift or escape code through: written literally they
            // would forge the very structure the decoder reads.
            if matches!(u32::from(c), 0x0E | 0x0F | 0x1B) {
                read += width;
                return (EncoderResult::Unmappable(char::REPLACEMENT_CHARACTER), read);
            }
            if c.is_ascii() {
                if self.shifted {
                    sink.write_byte(SI);
                    self.shifted = false;
                    continue;
                }
                sink.write_byte(c as u8);
                read += width;
                continue;
            }

            let Some((lead, trail)) = ksx1001_bytes(u32::from(c)) else {
                if self.shifted {
                    sink.write_byte(SI);
                    self.shifted = false;
                    continue;
                }
                read += width;
                return (EncoderResult::Unmappable(c), read);
            };

            if !self.designated {
                sink.write_slice(&DESIGNATOR);
                self.designated = true;
                continue;
            }
            if !self.shifted {
                sink.write_byte(SO);
                self.shifted = true;
                continue;
            }
            sink.write_slice(&[lead, trail]);
            read += width;
        }
    }
}
