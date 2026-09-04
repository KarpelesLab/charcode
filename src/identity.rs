//! The encodings that map byte `n` to U+`n`, and nothing else.
//!
//! ISO-8859-1 is that map over all 256 bytes; US-ASCII is the same over the
//! low 128, with every byte above rejected.  Neither needs a table, and neither
//! is what the WHATWG Encoding Standard resolves their labels to — it sends
//! `iso-8859-1` and `ascii` to windows-1252, which turns byte 0x80 into a euro
//! sign.  See [`Encoding::for_whatwg_label`](crate::Encoding::for_whatwg_label).

use crate::ascii::ascii_prefix_len_capped;
use crate::result::{DecoderResult, EncoderResult};
use crate::sink::{ByteSink, DECODER_HEADROOM, ENCODER_HEADROOM};

/// One past the last byte this encoding maps: 0x100 for ISO-8859-1, 0x80 for
/// US-ASCII.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Identity {
    limit: u16,
}

impl Identity {
    pub(crate) const fn latin1() -> Self {
        Identity { limit: 0x100 }
    }

    pub(crate) const fn ascii() -> Self {
        Identity { limit: 0x80 }
    }

    pub(crate) fn decode(&mut self, src: &[u8], sink: &mut ByteSink) -> (DecoderResult, usize) {
        let mut read = 0usize;
        loop {
            if !sink.has_room(DECODER_HEADROOM) {
                return (DecoderResult::OutputFull, read);
            }
            let rest = &src[read..];
            let run = ascii_prefix_len_capped(rest, sink.room());
            if run > 0 {
                sink.write_slice(&rest[..run]);
                read += run;
                continue;
            }
            let Some(byte) = rest.first().copied() else {
                return (DecoderResult::InputEmpty, read);
            };
            if u16::from(byte) >= self.limit {
                read += 1;
                return (DecoderResult::Malformed(1), read);
            }
            // Only ISO-8859-1 gets here, for a byte in 0x80..=0xFF.
            read += 1;
            sink.write_code_point(u32::from(byte));
        }
    }

    pub(crate) fn encode(&mut self, src: &str, sink: &mut ByteSink) -> (EncoderResult, usize) {
        let mut read = 0usize;
        loop {
            if !sink.has_room(ENCODER_HEADROOM) {
                return (EncoderResult::OutputFull, read);
            }
            let rest = &src[read..];
            let run = ascii_prefix_len_capped(rest.as_bytes(), sink.room());
            if run > 0 {
                sink.write_slice(&rest.as_bytes()[..run]);
                read += run;
                continue;
            }
            let Some(c) = rest.chars().next() else {
                return (EncoderResult::InputEmpty, read);
            };
            read += c.len_utf8();
            let scalar = u32::from(c);
            if scalar >= u32::from(self.limit) {
                return (EncoderResult::Unmappable(c), read);
            }
            sink.write_byte(scalar as u8);
        }
    }
}
