//! Decoder for `?compress=deflate` websocket connections.
//!
//! With `?compress=deflate` the server sends subscription data as binary frames of raw DEFLATE
//! (no zlib header). The compression state carries across frames, so one decoder must see every
//! binary frame of a connection, in order. Text frames (acks, `pong`, errors) are not compressed.
//! <https://docs.pacifica.fi/api-documentation/api/websocket>

use flate2::{Decompress, FlushDecompress, Status};

/// One per connection. Call [`reset`](Self::reset) when the connection is re-established
/// (`roshar_ws_mgr::Message::SuccessfulHandshake`), since a new connection starts a new stream.
pub struct PacificaInflater {
    inner: Decompress,
}

impl Default for PacificaInflater {
    fn default() -> Self {
        Self::new()
    }
}

impl PacificaInflater {
    pub fn new() -> Self {
        Self {
            inner: Decompress::new(false),
        }
    }

    pub fn reset(&mut self) {
        self.inner.reset(false);
    }

    /// Decompresses one binary frame into the JSON text it carries.
    pub fn decompress(&mut self, frame: &[u8]) -> Result<String, String> {
        let mut out = Vec::with_capacity(frame.len().saturating_mul(4).max(256));
        let mut input = frame;
        loop {
            if out.len() == out.capacity() {
                out.reserve(out.capacity());
            }
            let in_before = self.inner.total_in();
            let out_before = out.len();
            let status = self
                .inner
                .decompress_vec(input, &mut out, FlushDecompress::Sync)
                .map_err(|e| format!("Pacifica deflate frame: {e}"))?;
            let consumed = (self.inner.total_in() - in_before) as usize;
            input = &input[consumed..];
            let produced = out.len() - out_before;
            // Done once all input is consumed and the output buffer was not filled (no pending
            // output left inside the decompressor).
            if input.is_empty() && out.len() < out.capacity() {
                break;
            }
            if status == Status::StreamEnd {
                if input.is_empty() {
                    break;
                }
                return Err("Pacifica deflate stream ended mid-frame".to_string());
            }
            if consumed == 0 && produced == 0 && out.len() < out.capacity() {
                return Err("Pacifica deflate frame made no progress".to_string());
            }
        }
        String::from_utf8(out).map_err(|e| format!("Pacifica deflate frame is not UTF-8: {e}"))
    }
}
