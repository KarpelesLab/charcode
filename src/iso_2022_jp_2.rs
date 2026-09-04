//! ISO-2022-JP-2 (RFC 1554), which designates six sets rather than three.
//!
//! It is ISO-2022-JP with four more character sets reachable by escape:
//! GB 2312-80, KS X 1001, JIS X 0212, and the right halves of ISO 8859-1 and
//! ISO 8859-7.  The first three go into G0 and are read directly; the last two
//! go into G2 and are reached one character at a time by the single shift
//! `ESC N`, which is what lets a 7-bit mail transport carry Latin, Greek,
//! Japanese, Chinese and Korean in one message.
//!
//! It brings no table of its own: every set is one another encoding already
//! carries, which is why the feature needs all of them.
//!
//! Of the 35 942 sequences glibc's `iconv -f ISO-2022-JP-2` accepts, this
//! decodes 35 935 the same way.  The seven it does not are deliberate: U+000E,
//! U+000F and U+001B are refused rather than passed through, since those are
//! the codes the eight-bit ISO 2022 forms shift with and the leniency an
//! attack needs, and KS X 1001 cell 2-72 is one glibc maps to U+327E where the
//! standard's index EUC-KR, Python's `euc_kr` and Microsoft's 949 all leave it
//! undefined.  Nor is `ESC ( I` here: glibc reads it as half-width katakana,
//! but RFC 1554 has no such designation, and neither does RFC 1468 — that is
//! the mode [`X_WHATWG_ISO_2022_JP`](crate::X_WHATWG_ISO_2022_JP) has.
//!
//! The encoder writes byte-identical output to glibc's for every one of the
//! 18 731 characters both can represent, choosing among the six sets the same
//! way.  glibc reaches 99 more, and each is one this refuses on purpose: the
//! three codes above, the C1 controls — which glibc writes as a single shift
//! over a G2 byte below 0x20, outside the 96 the set has — the KS X 1001 cell,
//! and the half-width katakana behind `ESC ( I`.
//!
//! As in ISO-2022-JP, two escapes in a row with nothing between them are an
//! error, which is what stops an escape being smuggled through a filter that
//! inspects only the decoded text.

use crate::ascii::ascii_prefix_len_capped;
use crate::euc_cn;
use crate::euc_kr::{ksx1001_bytes, ksx1001_code_point};
use crate::index;
use crate::jis0208_1997;
use crate::result::{DecoderResult, EncoderResult};
use crate::sink::{ByteSink, DECODER_HEADROOM, ENCODER_HEADROOM, Pushback};
use crate::tables::jis::{
    JIS0212_DECODE, JIS0212_ENCODE_BUCKETS, JIS0212_ENCODE_CODE_POINTS, JIS0212_ENCODE_POINTERS,
};
use crate::tables::single_byte::ISO_8859_7_DECODE;

const ESC: u8 = 0x1B;

/// The set designated into G0, which bytes 0x21 to 0x7E are read as.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum G0 {
    #[default]
    Ascii,
    /// JIS X 0201's Roman set: ASCII but for the yen sign and the overline.
    Roman,
    Jis0208,
    Jis0212,
    Gb2312,
    Ksx1001,
}

impl G0 {
    /// Whether the set is read two bytes at a time.
    fn is_double(self) -> bool {
        !matches!(self, G0::Ascii | G0::Roman)
    }

    /// The escape that designates it.
    fn escape(self) -> &'static [u8] {
        match self {
            G0::Ascii => &[ESC, 0x28, 0x42],
            G0::Roman => &[ESC, 0x28, 0x4A],
            G0::Jis0208 => &[ESC, 0x24, 0x42],
            G0::Gb2312 => &[ESC, 0x24, 0x41],
            G0::Ksx1001 => &[ESC, 0x24, 0x28, 0x43],
            G0::Jis0212 => &[ESC, 0x24, 0x28, 0x44],
        }
    }
}

/// The 96-character set designated into G2, reached only by a single shift.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum G2 {
    #[default]
    None,
    /// The right half of ISO 8859-1, which is Unicode's own first 256.
    Latin1,
    Greek,
}

impl G2 {
    fn escape(self) -> &'static [u8] {
        match self {
            G2::Greek => &[ESC, 0x2E, 0x46],
            _ => &[ESC, 0x2E, 0x41],
        }
    }
}

/// Every escape this encoding has.  `ESC $ @` is JIS X 0208-1978, which every
/// implementation reads as JIS X 0208.
const ESCAPES: [(&[u8], Designation); 10] = [
    (&[ESC, 0x4E], Designation::SingleShift),
    (&[ESC, 0x28, 0x42], Designation::G0(G0::Ascii)),
    (&[ESC, 0x28, 0x4A], Designation::G0(G0::Roman)),
    (&[ESC, 0x24, 0x40], Designation::G0(G0::Jis0208)),
    (&[ESC, 0x24, 0x42], Designation::G0(G0::Jis0208)),
    (&[ESC, 0x24, 0x41], Designation::G0(G0::Gb2312)),
    (&[ESC, 0x24, 0x28, 0x43], Designation::G0(G0::Ksx1001)),
    (&[ESC, 0x24, 0x28, 0x44], Designation::G0(G0::Jis0212)),
    (&[ESC, 0x2E, 0x41], Designation::G2(G2::Latin1)),
    (&[ESC, 0x2E, 0x46], Designation::G2(G2::Greek)),
];

#[derive(Debug, Clone, Copy)]
enum Designation {
    G0(G0),
    G2(G2),
    SingleShift,
}

/// `the code point for a byte pair` in whichever set is in G0.
#[inline]
fn g0_code_point(set: G0, lead: u8, trail: u8) -> Option<u32> {
    let pointer = (usize::from(lead) - 0x21) * 94 + (usize::from(trail) - 0x21);
    match set {
        G0::Ascii | G0::Roman => None,
        G0::Jis0208 => jis0208_1997::code_point(pointer),
        G0::Jis0212 => index::code_point(&JIS0212_DECODE, pointer),
        // GB 2312's own bytes, with the high bits set back on.
        G0::Gb2312 => euc_cn::code_point(lead | 0x80, trail | 0x80),
        G0::Ksx1001 => ksx1001_code_point(lead, trail),
    }
}

/// `the code point for a byte` in whichever 96-set is in G2.  A 96-set covers
/// 0x20 to 0x7F, standing for 0xA0 to 0xFF of the eight-bit charset.
#[inline]
fn g2_code_point(set: G2, byte: u8) -> Option<u32> {
    if !(0x20..=0x7F).contains(&byte) {
        return None;
    }
    match set {
        G2::None => None,
        // ISO 8859-1 is Unicode's own first 256 code points.
        G2::Latin1 => Some(u32::from(byte) + 0x80),
        G2::Greek => match ISO_8859_7_DECODE[usize::from(byte)] {
            0 => None,
            code_point => Some(u32::from(code_point)),
        },
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    /// Reading single bytes in whatever G0 holds.
    Ground,
    /// Holding the lead byte of a pair from G0.
    Trail(u8),
    /// Expecting the one byte a single shift covers.
    Ss2,
    /// Part-way through an escape; the count is how many bytes have been read
    /// after the ESC, which `seen` holds.
    Escape(u8),
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Iso2022Jp2Decoder {
    state: State,
    g0: G0,
    g2: G2,
    /// Whether an escape has been seen with no output since; two escapes in a
    /// row are an error.
    output: bool,
    /// The bytes read after an ESC.  The longest escape here is four bytes.
    seen: [u8; 3],
    pending: Pushback,
}

impl Default for Iso2022Jp2Decoder {
    fn default() -> Self {
        Iso2022Jp2Decoder {
            state: State::Ground,
            g0: G0::Ascii,
            g2: G2::None,
            output: false,
            seen: [0; 3],
            pending: Pushback::default(),
        }
    }
}

impl Iso2022Jp2Decoder {
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
            if self.state == State::Ground && self.g0 == G0::Ascii && self.pending.is_empty() {
                let rest = &src[read..];
                let run = ascii_prefix_len_capped(rest, sink.room());
                let run = rest[..run]
                    .iter()
                    .position(|&b| b == 0x0E || b == 0x0F || b == ESC)
                    .unwrap_or(run);
                if run > 0 {
                    sink.write_slice(&rest[..run]);
                    self.output = false;
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
                (State::Ground, None) => return (DecoderResult::InputEmpty, read),
                (State::Trail(_) | State::Ss2, None) => {
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
                    let Some((escape, designation)) = ESCAPES
                        .iter()
                        .find(|(escape, _)| escape[1..].starts_with(after))
                    else {
                        // Not one of ours.  Leave the offending byte for the
                        // next pass and re-read the prefix that followed ESC.
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
                    match designation {
                        Designation::SingleShift => {
                            if self.g2 == G2::None {
                                return (DecoderResult::Malformed(2), read);
                            }
                            self.state = State::Ss2;
                            // A single shift produces a character, so it is not
                            // the second of two bare escapes.
                            self.output = false;
                            continue;
                        }
                        Designation::G0(set) => self.g0 = *set,
                        Designation::G2(set) => self.g2 = *set,
                    }
                    let had_output = self.output;
                    self.output = true;
                    if had_output {
                        return (DecoderResult::Malformed(1), read);
                    }
                }

                // --- the bytes of a character ----------------------------
                (State::Ss2, Some(b)) => {
                    self.state = State::Ground;
                    match g2_code_point(self.g2, b) {
                        Some(code_point) => sink.write_code_point(code_point),
                        None => return (DecoderResult::Malformed(1), read),
                    }
                }
                (State::Trail(_), Some(ESC)) => {
                    self.state = State::Escape(0);
                    return (DecoderResult::Malformed(1), read);
                }
                (State::Trail(lead), Some(b @ 0x21..=0x7E)) => {
                    self.state = State::Ground;
                    match g0_code_point(self.g0, lead, b) {
                        Some(code_point) => sink.write_code_point(code_point),
                        None => return (DecoderResult::Malformed(2), read),
                    }
                }
                (State::Trail(_), Some(_)) => {
                    self.state = State::Ground;
                    return (DecoderResult::Malformed(2), read);
                }

                // --- single bytes ---------------------------------------
                (State::Ground, Some(ESC)) => self.state = State::Escape(0),
                (State::Ground, Some(b @ 0x21..=0x7E)) if self.g0.is_double() => {
                    self.output = false;
                    self.state = State::Trail(b);
                }
                (State::Ground, Some(0x5C)) if self.g0 == G0::Roman => {
                    self.output = false;
                    sink.write_code_point(0x00A5);
                }
                (State::Ground, Some(0x7E)) if self.g0 == G0::Roman => {
                    self.output = false;
                    sink.write_code_point(0x203E);
                }
                // SO and SI belong to the eight-bit ISO 2022 forms, not this
                // one; passing them through is the leniency an attack needs.
                (State::Ground, Some(b)) if b < 0x80 && b != 0x0E && b != 0x0F => {
                    self.output = false;
                    sink.write_byte(b);
                }
                (State::Ground, Some(_)) => {
                    self.output = false;
                    return (DecoderResult::Malformed(1), read);
                }
            }
        }
    }

    /// Puts back the escape prefix that turned out not to be one of ours.  The
    /// ESC itself is consumed by the error; the bytes after it are text.
    fn restore_escape(&mut self, matched: usize) {
        self.state = State::Ground;
        self.output = false;
        if matched > 0 {
            self.pending.set(&self.seen[..matched]);
        }
    }
}

/// The largest code point ISO 8859-7 holds.
const GREEK_MAX: u16 = {
    let (mut max, mut byte) = (0, 0x20);
    while byte < ISO_8859_7_DECODE.len() {
        if ISO_8859_7_DECODE[byte] > max {
            max = ISO_8859_7_DECODE[byte];
        }
        byte += 1;
    }
    max
};

/// Where a character can be written, in the order the encoder tries them.
enum Target {
    /// A byte written as it stands, in ASCII or JIS X 0201's Roman set.
    Single(G0, u8),
    Double(G0, u8, u8),
    /// One byte from G2, behind a single shift.
    Shifted(G2, u8),
}

fn target_of(c: char) -> Option<Target> {
    let scalar = u32::from(c);
    match scalar {
        // JIS X 0201's Roman set differs from ASCII at exactly these two, and
        // is the cheapest way to write either.
        0x00A5 => return Some(Target::Single(G0::Roman, 0x5C)),
        0x203E => return Some(Target::Single(G0::Roman, 0x7E)),
        _ if c.is_ascii() => return Some(Target::Single(G0::Ascii, scalar as u8)),
        _ => {}
    }
    let double = |set: G0, pointer: u16| {
        Target::Double(
            set,
            (pointer / 94) as u8 + 0x21,
            (pointer % 94) as u8 + 0x21,
        )
    };
    if let Some(pointer) = jis0208_1997::pointer(scalar) {
        return Some(double(G0::Jis0208, pointer));
    }
    if let Some(pointer) = index::pointer(
        &JIS0212_ENCODE_CODE_POINTS,
        &JIS0212_ENCODE_POINTERS,
        &JIS0212_ENCODE_BUCKETS,
        scalar,
    ) {
        return Some(double(G0::Jis0212, pointer));
    }
    // The two 96-sets before the Chinese and Korean ones: a Latin-1 or Greek
    // character belongs in the Latin or Greek set, even where GB 2312 and
    // KS X 1001 happen to carry it too.  glibc chooses the same way.
    if (0xA0..=0xFF).contains(&scalar) {
        return Some(Target::Shifted(G2::Latin1, (scalar - 0x80) as u8));
    }
    // ISO 8859-7 is the one set here with no encode index, so this walks its
    // 96 cells.  The bound keeps that off the path every Chinese or Korean
    // character takes, which is almost all of them.
    if scalar <= u32::from(GREEK_MAX)
        && let Some(byte) =
            (0xA0..=0xFFusize).find(|&byte| u32::from(ISO_8859_7_DECODE[byte - 0x80]) == scalar)
    {
        return Some(Target::Shifted(G2::Greek, (byte - 0x80) as u8));
    }
    if let Some((lead, trail)) = euc_cn::bytes(scalar) {
        return Some(Target::Double(G0::Gb2312, lead & 0x7F, trail & 0x7F));
    }
    ksx1001_bytes(scalar).map(|(lead, trail)| Target::Double(G0::Ksx1001, lead, trail))
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Iso2022Jp2Encoder {
    g0: G0,
    g2: G2,
}

impl Iso2022Jp2Encoder {
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
                if last && self.g0 != G0::Ascii {
                    sink.write_slice(G0::Ascii.escape());
                    self.g0 = G0::Ascii;
                }
                return (EncoderResult::InputEmpty, read);
            };

            if self.g0 == G0::Ascii {
                let run = ascii_prefix_len_capped(rest.as_bytes(), sink.room());
                let run = rest.as_bytes()[..run]
                    .iter()
                    .position(|&b| b == 0x0E || b == 0x0F || b == ESC)
                    .unwrap_or(run);
                if run > 0 {
                    sink.write_slice(&rest.as_bytes()[..run]);
                    read += run;
                    continue;
                }
            }
            let width = c.len_utf8();

            // Never let an escape or a shift code through: written literally
            // they would forge the structure the decoder reads.  Reported as
            // U+FFFD so a caller substituting errors cannot be tricked into
            // emitting one either.
            if matches!(u32::from(c), 0x0E | 0x0F | 0x1B) {
                read += width;
                return (EncoderResult::Unmappable(char::REPLACEMENT_CHARACTER), read);
            }

            let Some(target) = target_of(c) else {
                // A line has to end in ASCII, and so does an unmappable
                // character's report: the caller may write a replacement.
                if self.g0 != G0::Ascii {
                    sink.write_slice(G0::Ascii.escape());
                    self.g0 = G0::Ascii;
                    continue;
                }
                read += width;
                return (EncoderResult::Unmappable(c), read);
            };

            match target {
                Target::Single(set, byte) => {
                    if self.g0 != set {
                        sink.write_slice(set.escape());
                        self.g0 = set;
                        continue;
                    }
                    sink.write_byte(byte);
                    read += width;
                }
                Target::Double(set, lead, trail) => {
                    if self.g0 != set {
                        sink.write_slice(set.escape());
                        self.g0 = set;
                        continue;
                    }
                    sink.write_slice(&[lead, trail]);
                    read += width;
                }
                Target::Shifted(set, byte) => {
                    if self.g2 != set {
                        sink.write_slice(set.escape());
                        self.g2 = set;
                        continue;
                    }
                    sink.write_slice(&[ESC, 0x4E, byte]);
                    read += width;
                }
            }
        }
    }
}
