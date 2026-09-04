//! x-user-defined: ASCII below 0x80, and the private use area above it.

use crate::ascii::ascii_prefix_len_capped;
use crate::result::{DecoderResult, EncoderResult};
use crate::sink::{ByteSink, DECODER_HEADROOM, ENCODER_HEADROOM};

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct XUserDefinedDecoder;

impl XUserDefinedDecoder {
    pub(crate) fn decode(&mut self, src: &[u8], sink: &mut ByteSink) -> (DecoderResult, usize) {
        let mut read = 0usize;
        loop {
            if !sink.has_room(DECODER_HEADROOM) {
                return (DecoderResult::OutputFull, read);
            }
            let rest = &src[read..];
            if rest.is_empty() {
                return (DecoderResult::InputEmpty, read);
            }
            let run = ascii_prefix_len_capped(rest, sink.room());
            if run > 0 {
                sink.write_slice(&rest[..run]);
                read += run;
                continue;
            }
            sink.write_code_point(0xF780 + u32::from(rest[0]) - 0x80);
            read += 1;
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct XUserDefinedEncoder;

impl XUserDefinedEncoder {
    pub(crate) fn encode(&mut self, src: &str, sink: &mut ByteSink) -> (EncoderResult, usize) {
        let mut read = 0usize;
        loop {
            if !sink.has_room(ENCODER_HEADROOM) {
                return (EncoderResult::OutputFull, read);
            }
            let rest = &src[read..];
            let Some(c) = rest.chars().next() else {
                return (EncoderResult::InputEmpty, read);
            };
            let run = ascii_prefix_len_capped(rest.as_bytes(), sink.room());
            if run > 0 {
                sink.write_slice(&rest.as_bytes()[..run]);
                read += run;
                continue;
            }
            read += c.len_utf8();
            match u32::from(c) {
                scalar @ 0xF780..=0xF7FF => sink.write_byte((scalar - 0xF780 + 0x80) as u8),
                _ => return (EncoderResult::Unmappable(c), read),
            }
        }
    }
}
