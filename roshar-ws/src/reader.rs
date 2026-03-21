use crate::error::{Error, Result};
use crate::frame::FrameRef;
use bytes::{Bytes, BytesMut};
use tokio::io::{AsyncRead, AsyncReadExt};

/// Internal reader type that can be either plain TCP or TLS
pub(crate) enum ReaderType {
    Plain(Box<dyn AsyncRead + Unpin + Send>),
    Tls(Box<dyn AsyncRead + Unpin + Send>),
}

/// WebSocket frame reader
///
/// Handles reading and parsing WebSocket frames from the underlying stream.
pub struct Reader {
    reader: ReaderType,
    read_buffer: BytesMut,
    max_frame_size: usize,
}

impl Reader {
    /// Create a new reader from a reader type
    pub(crate) fn new(reader: ReaderType, max_frame_size: usize) -> Self {
        Self {
            reader,
            read_buffer: BytesMut::with_capacity(2 * 1024 * 1024), // 2MB buffer: large enough that refills are rare, keeping P99 low
            max_frame_size,
        }
    }

    /// Create a new reader from an AsyncRead stream (for testing)
    #[cfg(test)]
    pub fn from_stream(stream: Box<dyn AsyncRead + Unpin + Send>, max_frame_size: usize) -> Self {
        Self::new(ReaderType::Plain(stream), max_frame_size)
    }

    /// Create a new reader from an AsyncRead stream (public for integration tests)
    #[doc(hidden)]
    pub fn from_stream_public(
        stream: Box<dyn AsyncRead + Unpin + Send>,
        max_frame_size: usize,
    ) -> Self {
        Self::new(ReaderType::Plain(stream), max_frame_size)
    }

    /// Read bytes from the stream into the buffer until we have at least `needed` bytes
    async fn read_until(&mut self, needed: usize) -> Result<()> {
        while self.read_buffer.len() < needed {
            // Ensure adequate spare capacity. Since the buffer is sole owner
            // (no shared Bytes from split_to/freeze), reserve() compacts via
            // memmove instead of reallocating.
            let spare = self.read_buffer.capacity() - self.read_buffer.len();
            let deficit = needed - self.read_buffer.len();
            if spare < deficit {
                self.read_buffer.reserve(deficit.max(65536));
            }

            let n = match &mut self.reader {
                ReaderType::Plain(r) => {
                    r.read_buf(&mut self.read_buffer).await.map_err(Error::Io)?
                }
                ReaderType::Tls(tls) => tls
                    .read_buf(&mut self.read_buffer)
                    .await
                    .map_err(Error::Io)?,
            };

            if n == 0 {
                if self.read_buffer.is_empty() {
                    return Err(Error::ConnectionClosed);
                }
                return Err(Error::IncompleteFrame {
                    expected: needed,
                    got: self.read_buffer.len(),
                });
            }
        }

        Ok(())
    }

    /// Read the next frame asynchronously using incremental reading
    ///
    /// This follows the fastwebsockets pattern: read incrementally, just enough
    /// bytes for each parsing step, rather than trying to fill the buffer completely.
    pub async fn read_frame(&mut self) -> Result<FrameRef> {
        use crate::frame::parse_frame_header;
        use bytes::Buf;

        // Step 1: Read first 2 bytes to get basic header info
        self.read_until(2).await?;

        // Parse what we can from the first 2 bytes to determine what else we need
        let second_byte = self.read_buffer[1];
        let masked = (second_byte & 0x80) != 0;
        let length_code = second_byte & 0x7F;

        // Determine how many more bytes we need for the header
        let extra_length_bytes = match length_code {
            126 => 2,
            127 => 8,
            _ => 0,
        };
        let mask_bytes = if masked { 4 } else { 0 };
        let header_bytes_needed = 2 + extra_length_bytes + mask_bytes;

        // Step 2: Read the rest of the header (extended length + mask if needed)
        self.read_until(header_bytes_needed).await?;

        // Now we can parse the full header
        let header = parse_frame_header(&self.read_buffer, self.max_frame_size)?;

        let total_needed = header.header_len + header.payload_len;

        // Step 3: Read the payload
        self.read_until(total_needed).await?;

        // Extract payload by copying to an independent allocation. This keeps the
        // read buffer as sole owner of its backing memory, so future reserve() calls
        // compact via cheap memmove instead of reallocating the entire buffer.
        // This trades a small per-frame memcpy for eliminating periodic reallocation
        // spikes that dominate P99 latency.
        let payload = if let Some(mask) = header.mask {
            let masked_data = &self.read_buffer[header.header_len..total_needed];
            let mut payload_bytes = Vec::with_capacity(header.payload_len);
            // SAFETY: we immediately write all bytes below via XOR copy
            unsafe { payload_bytes.set_len(header.payload_len) };

            // Unmask using chunked processing for better performance
            let chunks = header.payload_len / 8;
            let remainder = header.payload_len % 8;

            for chunk_idx in 0..chunks {
                let base = chunk_idx * 8;
                payload_bytes[base] = masked_data[base] ^ mask[0];
                payload_bytes[base + 1] = masked_data[base + 1] ^ mask[1];
                payload_bytes[base + 2] = masked_data[base + 2] ^ mask[2];
                payload_bytes[base + 3] = masked_data[base + 3] ^ mask[3];
                payload_bytes[base + 4] = masked_data[base + 4] ^ mask[0];
                payload_bytes[base + 5] = masked_data[base + 5] ^ mask[1];
                payload_bytes[base + 6] = masked_data[base + 6] ^ mask[2];
                payload_bytes[base + 7] = masked_data[base + 7] ^ mask[3];
            }

            let base = chunks * 8;
            for i in 0..remainder {
                payload_bytes[base + i] = masked_data[base + i] ^ mask[i & 3];
            }

            Bytes::from(payload_bytes)
        } else {
            Bytes::copy_from_slice(&self.read_buffer[header.header_len..total_needed])
        };

        // Advance past consumed frame - buffer remains sole owner of its allocation
        self.read_buffer.advance(total_needed);

        Ok(FrameRef::new(header.opcode, header.fin, payload))
    }

    /// Read data for handshake into BytesMut (optimized version using read_buf)
    pub(crate) async fn read_handshake_buf(&mut self, buf: &mut BytesMut) -> Result<usize> {
        match &mut self.reader {
            ReaderType::Plain(r) => r.read_buf(buf).await.map_err(Error::Io),
            ReaderType::Tls(tls) => tls.read_buf(buf).await.map_err(Error::Io),
        }
    }
}
