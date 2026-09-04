//! SCSU, the Standard Compression Scheme for Unicode (UTS #6).
//!
//! Not a charset: a compression scheme that expresses all of Unicode, aiming
//! at the size a legacy single-byte encoding would take.  It needs no table.
//! Instead it keeps eight *windows* of 128 code points each, and a byte from
//! 0x80 to 0xFF names a character in whichever window is active.  Tag bytes
//! move a window, switch windows, or drop into plain big-endian UTF-16 for a
//! run the windows cannot cover.
//!
//! Text in one alphabet therefore costs about a byte a character whatever the
//! alphabet is, and ASCII costs exactly a byte a character, since the initial
//! state makes U+0020 to U+00FF plus CR, LF and TAB come out as ISO 8859-1.
//!
//! Every constant here is from UTS #6: the static window positions (Table 4),
//! the default dynamic ones (Table 5), the window offset table (Table 3), and
//! the two tag tables.
//!
//! The scheme leaves an encoder wide latitude — many byte sequences decode to
//! the same text — so what this one writes is one of the valid answers, not
//! the only one.  It is a compressing encoder rather than the minimal one the
//! specification sketches: it moves a window when that pays for itself, and
//! falls back to UTF-16 only for text no window can reach, which is chiefly
//! the CJK ideographs at U+3400 to U+DFFF that the offset table skips.

use crate::result::{DecoderResult, EncoderResult};
use crate::sink::{ByteSink, DECODER_HEADROOM, ENCODER_HEADROOM};

/// Table 4: the fixed positions of the eight static windows.
const STATIC: [u32; 8] = [
    0x0000, 0x0080, 0x0100, 0x0300, 0x2000, 0x2080, 0x2100, 0x3000,
];

/// Table 5: where the eight dynamic windows start before anything moves them.
const DEFAULT_DYNAMIC: [u32; 8] = [
    0x0080, 0x00C0, 0x0400, 0x0600, 0x0900, 0x3040, 0x30A0, 0xFF00,
];

// Table 6: the tags of single-byte mode.  0x00, 0x09, 0x0A, 0x0D and 0x20 to
// 0x7F pass through; 0x0C is reserved.
const SQ0: u8 = 0x01;
const SDX: u8 = 0x0B;
const SQU: u8 = 0x0E;
const SCU: u8 = 0x0F;
const SC0: u8 = 0x10;
const SD0: u8 = 0x18;

// Table 7: the tags of Unicode mode.  Everything else is the high byte of a
// UTF-16 code unit; 0xF2 is reserved.
const UC0: u8 = 0xE0;
const UD0: u8 = 0xE8;
const UQU: u8 = 0xF0;
const UDX: u8 = 0xF1;

/// Table 3: where the byte after a window-definition tag puts the window.
///
/// The first stretch is half-blocks from U+0080 up, the second the ones from
/// U+E000 up, and the tail names the seven scripts that straddle a half-block
/// boundary.  The gap between them is why the CJK ideographs have no window.
fn window_offset(index: u8) -> Option<u32> {
    match index {
        0x01..=0x67 => Some(u32::from(index) * 0x80),
        0x68..=0xA7 => Some(u32::from(index) * 0x80 + 0xAC00),
        0xF9 => Some(0x00C0),
        0xFA => Some(0x0250),
        0xFB => Some(0x0370),
        0xFC => Some(0x0530),
        0xFD => Some(0x3040),
        0xFE => Some(0x30A0),
        0xFF => Some(0xFF60),
        // 0x00 and 0xA8 to 0xF8 are reserved.
        _ => None,
    }
}

/// The index that puts a window where `scalar` can be reached from it, if the
/// table has one.  Prefers the half-block entries, which is what an encoder
/// wants: they tile, so the choice is forced and a neighbouring character is
/// likely to land in the same window.
fn window_index(scalar: u32) -> Option<u8> {
    match scalar {
        0x0080..=0x33FF => Some((scalar / 0x80) as u8),
        0xE000..=0xFFFF => Some(((scalar - 0xAC00) / 0x80) as u8),
        _ => None,
    }
}

/// Where an extended window lands, from the two bytes after SDX or UDX.
#[inline]
fn extended_offset(high: u8, low: u8) -> u32 {
    0x10000 + 0x80 * ((u32::from(high & 0x1F) << 8) | u32::from(low))
}

/// The eight dynamic windows, and which is active.
#[derive(Debug, Clone, Copy)]
struct Windows {
    offset: [u32; 8],
    active: usize,
}

impl Default for Windows {
    fn default() -> Self {
        Windows {
            offset: DEFAULT_DYNAMIC,
            active: 0,
        }
    }
}

impl Windows {
    /// The character a byte from 0x80 to 0xFF names in window `n`.
    #[inline]
    fn code_point(&self, n: usize, byte: u8) -> u32 {
        self.offset[n] + u32::from(byte - 0x80)
    }

    /// The byte that would name `scalar` in window `n`, if it is in range.
    #[inline]
    fn byte_for(&self, n: usize, scalar: u32) -> Option<u8> {
        scalar
            .checked_sub(self.offset[n])
            .filter(|&d| d < 0x80)
            .map(|d| d as u8 + 0x80)
    }
}

/// What the decoder is part-way through.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    /// Single-byte mode, between characters.
    Ground,
    /// `SQn` seen; the next byte is one character from static or dynamic `n`.
    Quote(u8),
    /// `SQU` or `UQU` seen, with the pair of bytes still to come.  The flag
    /// says whether to return to Unicode mode after it.
    QuoteUnicode(bool),
    QuoteUnicodeLow(bool, u8),
    /// `SDn` or `UDn` seen; the next byte is the offset table index.
    Define(u8),
    /// `SDX` or `UDX` seen, with two argument bytes still to come.
    DefineExtended,
    DefineExtendedLow(u8),
    /// Unicode mode, between code units.
    Unicode,
    /// Unicode mode, holding the high byte of a code unit.
    UnicodeLow(u8),
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ScsuDecoder {
    state: State,
    windows: Windows,
    /// A high surrogate waiting for its low half.  The scheme lets the two
    /// halves arrive by different mechanisms, so this outlives any one state.
    high_surrogate: u16,
}

impl Default for ScsuDecoder {
    fn default() -> Self {
        ScsuDecoder {
            state: State::Ground,
            windows: Windows::default(),
            high_surrogate: 0,
        }
    }
}

/// What emitting one UTF-16 code unit did.
enum Emitted {
    /// Written, or held back as the high half of a surrogate pair.
    Ok,
    /// A surrogate that cannot stand where it is.
    Malformed,
}

impl ScsuDecoder {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Writes one UTF-16 code unit, pairing surrogates across whichever
    /// mechanisms produced them.
    fn emit_unit(&mut self, unit: u16, sink: &mut ByteSink) -> Emitted {
        if self.high_surrogate != 0 {
            let high = core::mem::replace(&mut self.high_surrogate, 0);
            if (0xDC00..=0xDFFF).contains(&unit) {
                let scalar =
                    0x10000 + ((u32::from(high) - 0xD800) << 10) + (u32::from(unit) - 0xDC00);
                sink.write_code_point(scalar);
                return Emitted::Ok;
            }
            // The high half had no low half after all.
            return Emitted::Malformed;
        }
        match unit {
            0xD800..=0xDBFF => {
                self.high_surrogate = unit;
                Emitted::Ok
            }
            0xDC00..=0xDFFF => Emitted::Malformed,
            _ => {
                sink.write_code_point(u32::from(unit));
                Emitted::Ok
            }
        }
    }

    /// Writes a scalar value that cannot be a surrogate, which is every
    /// character a window produces.
    fn emit_scalar(&mut self, scalar: u32, sink: &mut ByteSink) -> Emitted {
        debug_assert!(!(0xD800..=0xDFFF).contains(&scalar));
        if self.high_surrogate != 0 {
            self.high_surrogate = 0;
            return Emitted::Malformed;
        }
        sink.write_code_point(scalar);
        Emitted::Ok
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
                // A half-finished tag, or a high surrogate with no low half.
                if self.state != State::Ground && self.state != State::Unicode {
                    let unicode = matches!(
                        self.state,
                        State::Unicode | State::UnicodeLow(_) | State::QuoteUnicode(true)
                    );
                    self.state = if unicode {
                        State::Unicode
                    } else {
                        State::Ground
                    };
                    self.high_surrogate = 0;
                    return (DecoderResult::Malformed(1), read);
                }
                if self.high_surrogate != 0 {
                    self.high_surrogate = 0;
                    return (DecoderResult::Malformed(1), read);
                }
                return (DecoderResult::InputEmpty, read);
            };
            read += 1;

            let emitted = match self.state {
                // --- single-byte mode ------------------------------------
                State::Ground => match byte {
                    0x00 | 0x09 | 0x0A | 0x0D | 0x20..=0x7F => {
                        self.emit_scalar(u32::from(byte), sink)
                    }
                    SQ0..=0x08 => {
                        self.state = State::Quote(byte - SQ0);
                        continue;
                    }
                    SDX => {
                        self.state = State::DefineExtended;
                        continue;
                    }
                    SQU => {
                        self.state = State::QuoteUnicode(false);
                        continue;
                    }
                    SCU => {
                        self.state = State::Unicode;
                        continue;
                    }
                    SC0..=0x17 => {
                        self.windows.active = usize::from(byte - SC0);
                        continue;
                    }
                    SD0..=0x1F => {
                        self.state = State::Define(byte - SD0);
                        continue;
                    }
                    // 0x0B is SDX, 0x0C is reserved, and the rest is a window.
                    0x0C => return (DecoderResult::Malformed(1), read),
                    _ => {
                        let scalar = self.windows.code_point(self.windows.active, byte);
                        self.emit_scalar(scalar, sink)
                    }
                },

                // --- the byte after SQn ----------------------------------
                State::Quote(n) => {
                    self.state = State::Ground;
                    let n = usize::from(n);
                    if byte < 0x80 {
                        self.emit_scalar(STATIC[n] + u32::from(byte), sink)
                    } else {
                        self.emit_scalar(self.windows.code_point(n, byte), sink)
                    }
                }

                // --- the two bytes after SQU or UQU ----------------------
                State::QuoteUnicode(unicode) => {
                    self.state = State::QuoteUnicodeLow(unicode, byte);
                    continue;
                }
                State::QuoteUnicodeLow(unicode, high) => {
                    self.state = if unicode {
                        State::Unicode
                    } else {
                        State::Ground
                    };
                    self.emit_unit((u16::from(high) << 8) | u16::from(byte), sink)
                }

                // --- the byte after SDn or UDn ---------------------------
                State::Define(n) => {
                    self.state = State::Ground;
                    let Some(offset) = window_offset(byte) else {
                        return (DecoderResult::Malformed(1), read);
                    };
                    self.windows.offset[usize::from(n)] = offset;
                    self.windows.active = usize::from(n);
                    continue;
                }

                // --- the two bytes after SDX or UDX ----------------------
                State::DefineExtended => {
                    self.state = State::DefineExtendedLow(byte);
                    continue;
                }
                State::DefineExtendedLow(high) => {
                    self.state = State::Ground;
                    let n = usize::from(high >> 5);
                    self.windows.offset[n] = extended_offset(high, byte);
                    self.windows.active = n;
                    continue;
                }

                // --- Unicode mode ----------------------------------------
                State::Unicode => match byte {
                    UC0..=0xE7 => {
                        self.windows.active = usize::from(byte - UC0);
                        self.state = State::Ground;
                        continue;
                    }
                    UD0..=0xEF => {
                        self.state = State::Define(byte - UD0);
                        continue;
                    }
                    UQU => {
                        self.state = State::QuoteUnicode(true);
                        continue;
                    }
                    UDX => {
                        self.state = State::DefineExtended;
                        continue;
                    }
                    0xF2 => return (DecoderResult::Malformed(1), read),
                    _ => {
                        self.state = State::UnicodeLow(byte);
                        continue;
                    }
                },
                State::UnicodeLow(high) => {
                    self.state = State::Unicode;
                    self.emit_unit((u16::from(high) << 8) | u16::from(byte), sink)
                }
            };

            if let Emitted::Malformed = emitted {
                return (DecoderResult::Malformed(1), read);
            }
        }
    }
}

/// Which mode the encoder is in, and hence which tags it may write.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Mode {
    #[default]
    SingleByte,
    Unicode,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ScsuEncoder {
    mode: Mode,
    windows: Windows,
    /// Which window to move next.  Rotating rather than choosing keeps the
    /// encoder from ever thrashing one window while seven sit unused.
    next_to_move: usize,
}

impl Default for ScsuEncoder {
    fn default() -> Self {
        ScsuEncoder {
            mode: Mode::SingleByte,
            windows: Windows::default(),
            next_to_move: 0,
        }
    }
}

impl ScsuEncoder {
    /// The window already holding `scalar`, active one first.
    fn window_holding(&self, scalar: u32) -> Option<(usize, u8)> {
        let active = self.windows.active;
        if let Some(byte) = self.windows.byte_for(active, scalar) {
            return Some((active, byte));
        }
        (0..8).find_map(|n| self.windows.byte_for(n, scalar).map(|byte| (n, byte)))
    }

    /// The static window holding `scalar`, for a one-off quote.
    fn static_holding(scalar: u32) -> Option<(usize, u8)> {
        // Static window 0 is ASCII, which is for quoting the tag bytes only.
        (1..8).find_map(|n| {
            scalar
                .checked_sub(STATIC[n])
                .filter(|&d| d < 0x80)
                .map(|d| (n, d as u8))
        })
    }

    pub(crate) fn encode(
        &mut self,
        src: &str,
        sink: &mut ByteSink,
        _last: bool,
    ) -> (EncoderResult, usize) {
        let mut read = 0usize;
        loop {
            if !sink.has_room(ENCODER_HEADROOM) {
                return (EncoderResult::OutputFull, read);
            }
            let Some(c) = src[read..].chars().next() else {
                return (EncoderResult::InputEmpty, read);
            };
            let width = c.len_utf8();
            let scalar = u32::from(c);

            // The characters single-byte mode passes through untouched, which
            // is what makes plain ASCII cost one byte each.
            let passes_through = matches!(scalar, 0x00 | 0x09 | 0x0A | 0x0D | 0x20..=0x7F);
            if self.mode == Mode::Unicode {
                // Leave Unicode mode as soon as a window can hold the
                // character, since one byte then does what two did.
                if passes_through || self.window_holding(scalar).is_some() {
                    let n = self
                        .window_holding(scalar)
                        .map_or(self.windows.active, |(n, _)| n);
                    sink.write_byte(UC0 + n as u8);
                    self.windows.active = n;
                    self.mode = Mode::SingleByte;
                    continue;
                }
                // A window would have to move; do that from Unicode mode too,
                // rather than paying two bytes a character for a whole run.
                if let Some(index) = window_index(scalar) {
                    let n = self.take_window();
                    sink.write_slice(&[UD0 + n as u8, index]);
                    self.windows.offset[n] =
                        window_offset(index).expect("index came from the table");
                    self.windows.active = n;
                    self.mode = Mode::SingleByte;
                    continue;
                }
                if scalar > 0xFFFF {
                    let n = self.take_window();
                    let (high, low) = self.extended_arguments(n, scalar);
                    sink.write_slice(&[UDX, high, low]);
                    self.mode = Mode::SingleByte;
                    continue;
                }
                // Big-endian UTF-16, quoted where the high byte would be read
                // as a tag.
                let unit = scalar as u16;
                if matches!((unit >> 8) as u8, UC0..=0xF2) {
                    sink.write_byte(UQU);
                }
                sink.write_slice(&unit.to_be_bytes());
                read += width;
                continue;
            }

            if passes_through {
                sink.write_byte(scalar as u8);
                read += width;
                continue;
            }
            // A control byte that would be read as a tag, quoted from static
            // window 0.  SQ0 exists for exactly this.
            if scalar < 0x20 {
                sink.write_slice(&[SQ0, scalar as u8]);
                read += width;
                continue;
            }
            if let Some((n, byte)) = self.window_holding(scalar) {
                if n != self.windows.active {
                    sink.write_byte(SC0 + n as u8);
                    self.windows.active = n;
                }
                sink.write_byte(byte);
                read += width;
                continue;
            }
            if let Some((n, byte)) = Self::static_holding(scalar) {
                sink.write_slice(&[SQ0 + n as u8, byte]);
                read += width;
                continue;
            }
            if let Some(index) = window_index(scalar) {
                let n = self.take_window();
                sink.write_slice(&[SD0 + n as u8, index]);
                self.windows.offset[n] = window_offset(index).expect("index came from the table");
                self.windows.active = n;
                continue;
            }
            if scalar > 0xFFFF {
                let n = self.take_window();
                let (high, low) = self.extended_arguments(n, scalar);
                sink.write_slice(&[SDX, high, low]);
                continue;
            }
            // Nothing else reaches it: the offset table skips U+3400 to
            // U+DFFF, which is most of the CJK ideographs.
            sink.write_byte(SCU);
            self.mode = Mode::Unicode;
        }
    }

    /// The window to move next, round-robin.
    fn take_window(&mut self) -> usize {
        let n = self.next_to_move;
        self.next_to_move = (n + 1) % 8;
        n
    }

    /// The two argument bytes that put extended window `n` over `scalar`, and
    /// the offset they imply, recorded.
    fn extended_arguments(&mut self, n: usize, scalar: u32) -> (u8, u8) {
        let block = (scalar - 0x10000) / 0x80;
        let high = ((n as u32) << 5) as u8 | ((block >> 8) & 0x1F) as u8;
        let low = (block & 0xFF) as u8;
        self.windows.offset[n] = extended_offset(high, low);
        self.windows.active = n;
        (high, low)
    }
}
