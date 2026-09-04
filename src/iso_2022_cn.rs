//! ISO-2022-CN (RFC 1922): ASCII, GB 2312 and CNS 11643 planes 1 and 2.
//!
//! Like ISO-2022-KR, this is a stateful 7-bit encoding the WHATWG Encoding
//! Standard refuses outright, and that refusal is preserved:
//! [`Encoding::for_whatwg_label`](crate::Encoding::for_whatwg_label) still
//! answers `replacement`, so compiling this in cannot widen what a label off
//! the network selects.  It is reachable only through
//! [`Encoding::for_label`](crate::Encoding::for_label), for reading archived
//! Chinese mail and news.
//!
//! Three sets are designated rather than one.  `ESC $ ) A` puts GB 2312 in G1
//! and `ESC $ ) G` puts CNS 11643 plane 1 there; `ESC $ * H` puts plane 2 in
//! G2.  SO and SI shift G1 in and out, and a single shift, `ESC N`, takes one
//! character from G2.  The RFC scopes every designation to its own line, so a
//! line feed clears all three and returns to ASCII.
//!
//! ISO-2022-CN-EXT adds ISO-IR-165 and CNS 11643 planes 3 to 7.  Those need
//! mapping data with no authoritative published source, so its designators are
//! errors here rather than guesses.
//!
//! The decoder is deliberately strict where implementations differ: an
//! unrecognized escape is an error rather than passed through as literal
//! bytes, which is the leniency the smuggling attacks rely on.  A single shift
//! is likewise single, covering exactly the character after it; glibc's
//! encoder writes runs of plane 2 behind one `ESC N` and its decoder reads
//! them back, but the RFC gives SS2 no such reach, and treating it as locking
//! would let two bytes that look like ASCII stand for something else.
//!
//! Every one of the 20 962 byte sequences the three sets admit decodes here to
//! what glibc's `iconv -f ISO-2022-CN` gives.

use crate::ascii::ascii_prefix_len_capped;
use crate::euc_cn;
use crate::index;
use crate::result::{DecoderResult, EncoderResult};
use crate::sink::{ByteSink, DECODER_HEADROOM, ENCODER_HEADROOM, Pushback};
use crate::tables::cns11643::{
    CNS_PLANE1_DECODE, CNS_PLANE1_ENCODE_BUCKETS, CNS_PLANE1_ENCODE_CODE_POINTS,
    CNS_PLANE1_ENCODE_POINTERS, CNS_PLANE2_DECODE, CNS_PLANE2_ENCODE_BUCKETS,
    CNS_PLANE2_ENCODE_CODE_POINTS, CNS_PLANE2_ENCODE_POINTERS,
};

const SO: u8 = 0x0E;
const SI: u8 = 0x0F;
const ESC: u8 = 0x1B;

/// `ESC $ ) A`: GB 2312 into G1.
const DESIGNATE_GB2312: [u8; 4] = [ESC, 0x24, 0x29, 0x41];
/// `ESC $ ) G`: CNS 11643 plane 1 into G1.
const DESIGNATE_CNS1: [u8; 4] = [ESC, 0x24, 0x29, 0x47];
/// `ESC $ * H`: CNS 11643 plane 2 into G2.
const DESIGNATE_CNS2: [u8; 4] = [ESC, 0x24, 0x2A, 0x48];
/// `ESC N`: one character from G2.
const SS2: [u8; 2] = [ESC, 0x4E];

/// The set designated into G1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum G1 {
    #[default]
    None,
    Gb2312,
    Cns1,
}

/// `the code point for pointer` in whichever set is in G1.
#[inline]
fn g1_code_point(set: G1, lead: u8, trail: u8) -> Option<u32> {
    match set {
        G1::None => None,
        // GB 2312's own bytes, with the high bit set back on.
        G1::Gb2312 => euc_cn::code_point(lead | 0x80, trail | 0x80),
        G1::Cns1 => index::code_point(
            &CNS_PLANE1_DECODE,
            (usize::from(lead) - 0x21) * 94 + (usize::from(trail) - 0x21),
        ),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    /// Reading single bytes, whatever the shift state.
    Ground,
    /// Holding the lead byte of a pair from G1.
    Trail(u8),
    /// Holding the lead byte of a pair from G2, after a single shift.
    TrailSs2(u8),
    /// Expecting the trail byte of a single-shifted character.
    Ss2,
    /// Part-way through an escape; the count is how many bytes have been read
    /// after the ESC, which `seen` holds.
    Escape(u8),
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Iso2022CnDecoder {
    state: State,
    g1: G1,
    /// Whether CNS 11643 plane 2 is in G2; nothing else may be.
    g2: bool,
    shifted: bool,
    /// The bytes read after an ESC, while `state` is `Escape`.  The longest
    /// escape here is four bytes, so three follow the ESC.
    seen: [u8; 3],
    pending: Pushback,
}

impl Default for Iso2022CnDecoder {
    fn default() -> Self {
        Iso2022CnDecoder {
            state: State::Ground,
            g1: G1::None,
            g2: false,
            shifted: false,
            seen: [0; 3],
            pending: Pushback::default(),
        }
    }
}

/// Every escape this encoding has, longest last.
const ESCAPES: [&[u8]; 4] = [&SS2, &DESIGNATE_GB2312, &DESIGNATE_CNS1, &DESIGNATE_CNS2];

impl Iso2022CnDecoder {
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
                    .position(|&b| b == SO || b == SI || b == ESC || b == b'\n')
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
                (State::Trail(_) | State::TrailSs2(_) | State::Ss2, None) => {
                    self.state = State::Ground;
                    return (DecoderResult::Malformed(1), read);
                }
                (State::Escape(matched), None) => {
                    self.restore_escape(usize::from(matched));
                    return (DecoderResult::Malformed(1), read);
                }

                // --- part-way through an escape --------------------------
                (State::Escape(matched), Some(b)) => {
                    let matched = usize::from(matched);
                    self.seen[matched] = b;
                    let after = &self.seen[..matched + 1];
                    // The escapes all begin with ESC, so compare what follows.
                    let Some(escape) = ESCAPES.iter().find(|escape| escape[1..].starts_with(after))
                    else {
                        // Not one of ours.  Leave the offending byte for the
                        // next pass and re-read whatever of the prefix followed
                        // the ESC.
                        if from_pending {
                            self.pending.unread();
                        } else {
                            read -= 1;
                        }
                        self.restore_escape(matched);
                        return (DecoderResult::Malformed(1), read);
                    };
                    if after.len() + 1 < escape.len() {
                        self.state = State::Escape(matched as u8 + 1);
                        continue;
                    }
                    self.state = State::Ground;
                    if *escape == DESIGNATE_GB2312 {
                        self.g1 = G1::Gb2312;
                    } else if *escape == DESIGNATE_CNS1 {
                        self.g1 = G1::Cns1;
                    } else if *escape == DESIGNATE_CNS2 {
                        self.g2 = true;
                    } else if self.g2 {
                        self.state = State::Ss2;
                    } else {
                        // A single shift with nothing designated into G2.
                        return (DecoderResult::Malformed(2), read);
                    }
                }

                // --- the bytes of a character ----------------------------
                (State::Ss2, Some(b @ 0x21..=0x7E)) => self.state = State::TrailSs2(b),
                (State::Ss2, Some(_)) => {
                    self.state = State::Ground;
                    return (DecoderResult::Malformed(1), read);
                }
                (State::Trail(lead), Some(b @ 0x21..=0x7E)) => {
                    self.state = State::Ground;
                    match g1_code_point(self.g1, lead, b) {
                        Some(code_point) => sink.write_code_point(code_point),
                        None => return (DecoderResult::Malformed(2), read),
                    }
                }
                (State::TrailSs2(lead), Some(b @ 0x21..=0x7E)) => {
                    self.state = State::Ground;
                    let pointer = (usize::from(lead) - 0x21) * 94 + (usize::from(b) - 0x21);
                    match index::code_point(&CNS_PLANE2_DECODE, pointer) {
                        Some(code_point) => sink.write_code_point(code_point),
                        None => return (DecoderResult::Malformed(2), read),
                    }
                }
                (State::Trail(_) | State::TrailSs2(_), Some(_)) => {
                    self.state = State::Ground;
                    return (DecoderResult::Malformed(2), read);
                }

                // --- single bytes ---------------------------------------
                (State::Ground, Some(ESC)) => self.state = State::Escape(0),
                (State::Ground, Some(SO)) => {
                    if self.g1 == G1::None {
                        return (DecoderResult::Malformed(1), read);
                    }
                    self.shifted = true;
                }
                (State::Ground, Some(SI)) => self.shifted = false,
                // The RFC scopes a designation to its line, so the state a
                // truncated line leaves behind cannot reach the next one.
                (State::Ground, Some(b @ b'\n')) => {
                    self.shifted = false;
                    self.g1 = G1::None;
                    self.g2 = false;
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

    /// Puts back the escape prefix that turned out not to be one of ours.  The
    /// ESC itself is consumed by the error; the bytes after it are text.
    fn restore_escape(&mut self, matched: usize) {
        self.state = State::Ground;
        if matched > 0 {
            self.pending.set(&self.seen[..matched]);
        }
    }
}

/// Where a character can be written, in the order the encoder tries them.
enum Target {
    Ascii(u8),
    G1(G1, u16),
    G2(u16),
}

fn target_of(c: char) -> Option<Target> {
    let scalar = u32::from(c);
    if c.is_ascii() {
        return Some(Target::Ascii(scalar as u8));
    }
    if let Some((lead, trail)) = euc_cn::bytes(scalar) {
        let pointer = u16::from(lead - 0xA1) * 94 + u16::from(trail - 0xA1);
        return Some(Target::G1(G1::Gb2312, pointer));
    }
    if let Some(pointer) = index::pointer(
        &CNS_PLANE1_ENCODE_CODE_POINTS,
        &CNS_PLANE1_ENCODE_POINTERS,
        &CNS_PLANE1_ENCODE_BUCKETS,
        scalar,
    ) {
        return Some(Target::G1(G1::Cns1, pointer));
    }
    index::pointer(
        &CNS_PLANE2_ENCODE_CODE_POINTS,
        &CNS_PLANE2_ENCODE_POINTERS,
        &CNS_PLANE2_ENCODE_BUCKETS,
        scalar,
    )
    .map(Target::G2)
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Iso2022CnEncoder {
    g1: G1,
    g2: bool,
    shifted: bool,
}

impl Iso2022CnEncoder {
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

            let Some(target) = target_of(c) else {
                if self.shifted {
                    sink.write_byte(SI);
                    self.shifted = false;
                    continue;
                }
                read += width;
                return (EncoderResult::Unmappable(c), read);
            };

            match target {
                Target::Ascii(byte) => {
                    if self.shifted {
                        sink.write_byte(SI);
                        self.shifted = false;
                        continue;
                    }
                    sink.write_byte(byte);
                    read += width;
                    // A line feed ends every designation's scope, so the next
                    // line has to name its sets again.
                    if byte == b'\n' {
                        self.g1 = G1::None;
                        self.g2 = false;
                    }
                }
                Target::G1(set, pointer) => {
                    if self.g1 != set {
                        // Redesignating G1 mid-run is legal, but keeping each
                        // shifted run to one set costs two bytes and leaves
                        // less for a lenient decoder to get wrong.
                        if self.shifted {
                            sink.write_byte(SI);
                            self.shifted = false;
                            continue;
                        }
                        sink.write_slice(match set {
                            G1::Cns1 => &DESIGNATE_CNS1,
                            _ => &DESIGNATE_GB2312,
                        });
                        self.g1 = set;
                        continue;
                    }
                    if !self.shifted {
                        sink.write_byte(SO);
                        self.shifted = true;
                        continue;
                    }
                    sink.write_slice(&[(pointer / 94) as u8 + 0x21, (pointer % 94) as u8 + 0x21]);
                    read += width;
                }
                Target::G2(pointer) => {
                    if !self.g2 {
                        sink.write_slice(&DESIGNATE_CNS2);
                        self.g2 = true;
                        continue;
                    }
                    // The single shift reaches G2 from either shift state.
                    sink.write_slice(&SS2);
                    sink.write_slice(&[(pointer / 94) as u8 + 0x21, (pointer % 94) as u8 + 0x21]);
                    read += width;
                }
            }
        }
    }
}
