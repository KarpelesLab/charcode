//! The `replacement` encoding, which maps any non-empty input to a single error.
//!
//! It exists so that labels for encodings that are unsafe to support (HZ-GB-2312,
//! the ISO-2022 variants other than ISO-2022-JP) resolve to something that cannot
//! be abused rather than to a real decoder.

use crate::result::DecoderResult;
use crate::sink::ByteSink;

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct ReplacementDecoder {
    error_returned: bool,
}

impl ReplacementDecoder {
    pub(crate) fn decode(&mut self, src: &[u8], _sink: &mut ByteSink) -> (DecoderResult, usize) {
        if src.is_empty() {
            return (DecoderResult::InputEmpty, 0);
        }
        if !self.error_returned {
            self.error_returned = true;
            return (DecoderResult::Malformed(1), 0);
        }
        // The handler is finished; the rest of the stream is discarded.
        (DecoderResult::InputEmpty, src.len())
    }
}
