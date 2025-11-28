use crate::error::{Error, Result};
use crate::frame::Frame;
use tokio::io::{AsyncWrite, AsyncWriteExt};

/// Internal writer type that can be either plain TCP or TLS
pub(crate) enum WriterType {
    Plain(Box<dyn AsyncWrite + Unpin + Send>),
    Tls(Box<dyn AsyncWrite + Unpin + Send>),
}

/// WebSocket frame writer
///
/// Handles writing WebSocket frames to the underlying stream.
pub struct Writer {
    writer: WriterType,
}

impl Writer {
    /// Create a new writer from a writer type
    pub(crate) fn new(writer: WriterType) -> Self {
        Self { writer }
    }

    /// Send a frame
    ///
    /// This writes the frame directly to the writer without creating an intermediate buffer,
    /// avoiding unnecessary copies of the frame payload.
    pub async fn send_frame(&mut self, frame: &Frame) -> Result<()> {
        Self::write_frame_internal(&mut self.writer, frame).await?;
        Ok(())
    }

    /// Write a frame directly to the writer without intermediate buffers
    async fn write_frame_internal(writer: &mut WriterType, frame: &Frame) -> Result<()> {
        // Write header bytes
        let mut header = [0u8; 14];
        let mut header_len = 2;

        // First byte: FIN, RSV, Opcode
        let mut first_byte = frame.opcode as u8;
        if frame.fin {
            first_byte |= 0x80;
        }
        if frame.rsv[0] {
            first_byte |= 0x40;
        }
        if frame.rsv[1] {
            first_byte |= 0x20;
        }
        if frame.rsv[2] {
            first_byte |= 0x10;
        }
        header[0] = first_byte;

        // Second byte: MASK, payload length
        let payload_len = frame.payload.len();
        let mut second_byte = if frame.masked { 0x80 } else { 0x00 };

        if payload_len < 126 {
            second_byte |= payload_len as u8;
            header[1] = second_byte;
        } else if payload_len < 65536 {
            second_byte |= 126;
            header[1] = second_byte;
            header[2] = ((payload_len >> 8) & 0xFF) as u8;
            header[3] = (payload_len & 0xFF) as u8;
            header_len = 4;
        } else {
            second_byte |= 127;
            header[1] = second_byte;
            // For 64-bit, we write 0s for the first 4 bytes
            header[2] = 0;
            header[3] = 0;
            header[4] = 0;
            header[5] = 0;
            header[6] = ((payload_len >> 24) & 0xFF) as u8;
            header[7] = ((payload_len >> 16) & 0xFF) as u8;
            header[8] = ((payload_len >> 8) & 0xFF) as u8;
            header[9] = (payload_len & 0xFF) as u8;
            header_len = 10;
        }

        // Write header
        Self::write_all_internal(writer, &header[..header_len]).await?;

        // Write masking key if present
        if let Some(mask) = frame.mask {
            Self::write_all_internal(writer, &mask).await?;
        }

        // Write payload - mask in place if needed
        if let Some(mask) = frame.mask {
            // For masked frames, we need to mask the payload
            // We'll write in chunks to avoid allocating a full copy
            const CHUNK_SIZE: usize = 8192;
            let mut offset = 0;
            let mut mask_offset = 0;

            while offset < frame.payload.len() {
                let chunk_size = (frame.payload.len() - offset).min(CHUNK_SIZE);
                let mut chunk = vec![0u8; chunk_size];

                // Apply mask to chunk
                for i in 0..chunk_size {
                    chunk[i] = frame.payload[offset + i] ^ mask[(mask_offset + i) % 4];
                }

                Self::write_all_internal(writer, &chunk).await?;
                offset += chunk_size;
                mask_offset = (mask_offset + chunk_size) % 4;
            }
        } else {
            // For unmasked frames, write payload directly (zero-copy)
            Self::write_all_internal(writer, &frame.payload).await?;
        }

        Ok(())
    }

    async fn write_all_internal(writer: &mut WriterType, buf: &[u8]) -> Result<()> {
        match writer {
            WriterType::Plain(w) => w.write_all(buf).await.map_err(|e| Error::Io(e)),
            WriterType::Tls(tls) => tls.write_all(buf).await.map_err(|e| Error::Io(e)),
        }
    }

    /// Write data for handshake (used during connection)
    pub(crate) async fn write_handshake(&mut self, buf: &[u8]) -> Result<()> {
        Self::write_all_internal(&mut self.writer, buf).await
    }
}
