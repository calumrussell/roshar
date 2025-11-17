use crate::error::{Error, Result};
use crate::frame::FrameRef;
use bytes::BytesMut;
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
            read_buffer: BytesMut::with_capacity(262144), // 256KB buffer to reduce reallocations
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
    pub fn from_stream_public(stream: Box<dyn AsyncRead + Unpin + Send>, max_frame_size: usize) -> Self {
        Self::new(ReaderType::Plain(stream), max_frame_size)
    }
    



    /// Read bytes from the stream into the buffer until we have at least `needed` bytes
    async fn read_until(&mut self, needed: usize) -> Result<()> {
        while self.read_buffer.len() < needed {
            let n = match &mut self.reader {
                ReaderType::Plain(r) => {
                    r.read_buf(&mut self.read_buffer).await.map_err(|e| Error::Io(e))?
                }
                ReaderType::Tls(tls) => {
                    tls.read_buf(&mut self.read_buffer).await.map_err(|e| Error::Io(e))?
                }
            };
            
            if n == 0 {
                if self.read_buffer.len() == 0 {
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
        
        // Ensure we have capacity
        if self.read_buffer.capacity() - self.read_buffer.len() < 65536 {
            self.read_buffer.reserve(131072);
        }
        
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
        
        // Step 3: Reserve space for payload + next frame header (14 bytes max)
        // This allows read_buf to naturally read ahead and get the next frame header
        // in the same syscall, reducing overhead for the next frame read
        const MAX_HEADER_SIZE: usize = 14;
        let total_needed = header.header_len + header.payload_len;
        let read_ahead_target = total_needed + MAX_HEADER_SIZE;
        
        // Ensure we have capacity for read-ahead
        if self.read_buffer.capacity() < read_ahead_target {
            self.read_buffer.reserve(read_ahead_target);
        }
        
        // Step 4: Read the payload. read_buf will naturally read ahead if more data
        // is available, potentially getting the next frame header in the same syscall.
        // We read until we have at least total_needed, but read_buf may read more.
        self.read_until(total_needed).await?;
        
        // Now we have the complete frame - extract it
        let mut frame_data = self.read_buffer.split_to(total_needed);
        
        // Skip the header to get to the payload
        frame_data.advance(header.header_len);
        
        // Unmask if needed
        if let Some(mask) = header.mask {
            // Unmask the payload in place - optimized to avoid modulo and process in chunks
            let payload = &mut frame_data[..header.payload_len];
            
            // Process 8 bytes at a time for better performance
            let chunks = payload.len() / 8;
            let remainder = payload.len() % 8;
            
            for chunk_idx in 0..chunks {
                let base = chunk_idx * 8;
                payload[base] ^= mask[0];
                payload[base + 1] ^= mask[1];
                payload[base + 2] ^= mask[2];
                payload[base + 3] ^= mask[3];
                payload[base + 4] ^= mask[0];
                payload[base + 5] ^= mask[1];
                payload[base + 6] ^= mask[2];
                payload[base + 7] ^= mask[3];
            }
            
            // Handle remaining bytes
            let base = chunks * 8;
            for i in 0..remainder {
                payload[base + i] ^= mask[i & 3];
            }
        }
        
        // Extract the payload as Bytes (zero-copy if possible)
        let payload = frame_data.freeze();
        
        Ok(FrameRef::new(header.opcode, header.fin, payload))
    }

    /// Read data for handshake into BytesMut (optimized version using read_buf)
    pub(crate) async fn read_handshake_buf(&mut self, buf: &mut BytesMut) -> Result<usize> {
        match &mut self.reader {
            ReaderType::Plain(r) => {
                r.read_buf(buf).await.map_err(|e| Error::Io(e))
            }
            ReaderType::Tls(tls) => {
                tls.read_buf(buf).await.map_err(|e| Error::Io(e))
            }
        }
    }
}


