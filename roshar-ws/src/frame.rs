use crate::error::{Error, Result};
use bytes::{BufMut, Bytes, BytesMut};

/// Parsed frame header information
pub(crate) struct FrameHeader {
    pub(crate) fin: bool,
    pub(crate) opcode: Opcode,
    pub(crate) masked: bool,
    pub(crate) payload_len: usize,
    pub(crate) header_len: usize,
    pub(crate) mask: Option<[u8; 4]>,
}

/// Parse the WebSocket frame header from a buffer
///
/// Returns the parsed header information and the total header length.
/// Returns `Error::IncompleteFrame` if the header is incomplete.
pub(crate) fn parse_frame_header(buf: &[u8], max_frame_size: usize) -> Result<FrameHeader> {
    if buf.len() < 2 {
        return Err(Error::IncompleteFrame {
            expected: 2,
            got: buf.len(),
        });
    }

    let first_byte = buf[0];
    let fin = (first_byte & 0x80) != 0;
    let opcode = Opcode::try_from(first_byte & 0x0F)?;

    let second_byte = buf[1];
    let masked = (second_byte & 0x80) != 0;
    let mut payload_len = (second_byte & 0x7F) as usize;

    let mut header_len = 2;

    // Parse extended payload length
    if payload_len == 126 {
        if buf.len() < 4 {
            return Err(Error::IncompleteFrame {
                expected: 4,
                got: buf.len(),
            });
        }
        payload_len = u16::from_be_bytes([buf[2], buf[3]]) as usize;
        header_len = 4;
    } else if payload_len == 127 {
        if buf.len() < 10 {
            return Err(Error::IncompleteFrame {
                expected: 10,
                got: buf.len(),
            });
        }
        let len_bytes = &buf[2..10];
        if len_bytes[0] != 0 || len_bytes[1] != 0 || len_bytes[2] != 0 || len_bytes[3] != 0 {
            return Err(Error::InvalidFrame(
                "64-bit payload length not fully supported".to_string(),
            ));
        }
        payload_len = u32::from_be_bytes([
            len_bytes[4],
            len_bytes[5],
            len_bytes[6],
            len_bytes[7],
        ]) as usize;
        header_len = 10;
    }

    // Check frame size limit
    if payload_len > max_frame_size {
        return Err(Error::FrameTooLarge {
            size: payload_len,
            max: max_frame_size,
        });
    }

    // Parse masking key if present
    let mask = if masked {
        if buf.len() < header_len + 4 {
            return Err(Error::IncompleteFrame {
                expected: header_len + 4,
                got: buf.len(),
            });
        }
        let mask_bytes = &buf[header_len..header_len + 4];
        header_len += 4;
        Some([mask_bytes[0], mask_bytes[1], mask_bytes[2], mask_bytes[3]])
    } else {
        None
    };

    Ok(FrameHeader {
        fin,
        opcode,
        masked,
        payload_len,
        header_len,
        mask,
    })
}

/// WebSocket frame opcodes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Opcode {
    Continuation = 0x0,
    Text = 0x1,
    Binary = 0x2,
    Close = 0x8,
    Ping = 0x9,
    Pong = 0xA,
}

impl TryFrom<u8> for Opcode {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self> {
        match value {
            0x0 => Ok(Opcode::Continuation),
            0x1 => Ok(Opcode::Text),
            0x2 => Ok(Opcode::Binary),
            0x8 => Ok(Opcode::Close),
            0x9 => Ok(Opcode::Ping),
            0xA => Ok(Opcode::Pong),
            _ => Err(Error::InvalidOpcode(value)),
        }
    }
}

/// A reference to a WebSocket frame.
///
/// This struct uses zero-copy Bytes for the payload to avoid allocations.
/// The payload remains valid until the next call to `read_frame()` or `poll_frame()`.
#[derive(Debug)]
pub struct FrameRef {
    /// Frame opcode.
    pub opcode: Opcode,
    /// FIN bit (true if this is the final fragment).
    pub fin: bool,
    /// Frame payload data (zero-copy Bytes).
    pub payload: Bytes,
}

impl FrameRef {
    /// Create a new FrameRef.
    pub fn new(opcode: Opcode, fin: bool, payload: Bytes) -> Self {
        Self {
            opcode,
            fin,
            payload,
        }
    }

    /// Check if this frame is complete (FIN bit set).
    pub fn is_final(&self) -> bool {
        self.fin
    }

    /// Check if this is a control frame.
    pub fn is_control(&self) -> bool {
        matches!(self.opcode, Opcode::Close | Opcode::Ping | Opcode::Pong)
    }
}

/// WebSocket frame with zero-copy payload (for sending)
///
/// This frame structure is used for sending frames.
#[derive(Debug, Clone)]
pub struct Frame {
    /// FIN bit - indicates if this is the final fragment
    pub fin: bool,
    /// RSV1, RSV2, RSV3 bits (reserved for extensions)
    pub rsv: [bool; 3],
    /// Frame opcode
    pub opcode: Opcode,
    /// MASK bit (must be true for client-to-server frames)
    pub masked: bool,
    /// Masking key (4 bytes, only present if masked is true)
    pub mask: Option<[u8; 4]>,
    /// Frame payload
    pub payload: Bytes,
}

impl Frame {
    /// Parse a WebSocket frame from bytes
    ///
    /// Returns the frame and the number of bytes consumed.
    /// If the frame is incomplete, returns `Error::IncompleteFrame`.
    pub fn parse(buf: &[u8], max_frame_size: usize) -> Result<(Self, usize)> {
        let header = parse_frame_header(buf, max_frame_size)?;
        
        // Check if we have the full payload
        let total_len = header.header_len + header.payload_len;
        if buf.len() < total_len {
            return Err(Error::IncompleteFrame {
                expected: total_len,
                got: buf.len(),
            });
        }

        // Extract and unmask payload
        let payload = if let Some(mask) = header.mask {
            // Unmask during copy to avoid double allocation
            let mut payload_bytes = Vec::with_capacity(header.payload_len);
            let masked_data = &buf[header.header_len..total_len];
            for (i, &byte) in masked_data.iter().enumerate() {
                payload_bytes.push(byte ^ mask[i % 4]);
            }
            Bytes::from(payload_bytes)
        } else {
            // Zero-copy for unmasked frames
            Bytes::copy_from_slice(&buf[header.header_len..total_len])
        };

        let first_byte = buf[0];
        Ok((
            Frame {
                fin: header.fin,
                rsv: [
                    (first_byte & 0x40) != 0,
                    (first_byte & 0x20) != 0,
                    (first_byte & 0x10) != 0,
                ],
                opcode: header.opcode,
                masked: header.masked,
                mask: header.mask,
                payload,
            },
            total_len,
        ))
    }

    /// Serialize this frame to bytes
    pub fn serialize(&self) -> BytesMut {
        let mut buf = BytesMut::with_capacity(14 + self.payload.len());

        // First byte: FIN, RSV, Opcode
        let mut first_byte = self.opcode as u8;
        if self.fin {
            first_byte |= 0x80;
        }
        if self.rsv[0] {
            first_byte |= 0x40;
        }
        if self.rsv[1] {
            first_byte |= 0x20;
        }
        if self.rsv[2] {
            first_byte |= 0x10;
        }
        buf.put_u8(first_byte);

        // Second byte: MASK, payload length
        let payload_len = self.payload.len();
        let mut second_byte = if self.masked { 0x80 } else { 0x00 };

        if payload_len < 126 {
            second_byte |= payload_len as u8;
            buf.put_u8(second_byte);
        } else if payload_len < 65536 {
            second_byte |= 126;
            buf.put_u8(second_byte);
            buf.put_u16(payload_len as u16);
        } else {
            second_byte |= 127;
            buf.put_u8(second_byte);
            // For 64-bit, we write 0s for the first 4 bytes
            buf.put_u32(0);
            buf.put_u32(payload_len as u32);
        }

        // Masking key
        if let Some(mask) = self.mask {
            buf.put_slice(&mask);
        }

        // Payload (apply mask if needed)
        if let Some(mask) = self.mask {
            // Mask during write to avoid double allocation
            // Reserve space for masked payload
            let payload_start = buf.len();
            buf.reserve(self.payload.len());
            unsafe {
                // Extend buffer without initializing
                buf.set_len(payload_start + self.payload.len());
            }
            // Apply mask directly into buffer
            for (i, &byte) in self.payload.iter().enumerate() {
                buf[payload_start + i] = byte ^ mask[i % 4];
            }
        } else {
            buf.put_slice(&self.payload);
        }

        buf
    }

    /// Create a text frame with automatic masking for client-to-server communication
    ///
    /// This is a convenience method that generates a random mask and creates
    /// a properly formatted text frame for sending from client to server.
    pub fn text(payload: impl Into<Bytes>) -> Self {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let mask = rng.gen::<[u8; 4]>();
        
        Self {
            fin: true,
            rsv: [false, false, false],
            opcode: Opcode::Text,
            masked: true,
            mask: Some(mask),
            payload: payload.into(),
        }
    }

    /// Create a binary frame with automatic masking for client-to-server communication
    pub fn binary(payload: impl Into<Bytes>) -> Self {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let mask = rng.gen::<[u8; 4]>();
        
        Self {
            fin: true,
            rsv: [false, false, false],
            opcode: Opcode::Binary,
            masked: true,
            mask: Some(mask),
            payload: payload.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_frame() {
        // Simple unmasked text frame: "Hello"
        let data = vec![0x81, 0x05, 0x48, 0x65, 0x6c, 0x6c, 0x6f];
        let (frame, consumed) = Frame::parse(&data, 1024).unwrap();
        assert_eq!(consumed, 7);
        assert!(frame.fin);
        assert_eq!(frame.opcode, Opcode::Text);
        assert!(!frame.masked);
        assert_eq!(frame.payload, Bytes::from("Hello"));
    }

    #[test]
    fn test_parse_masked_frame() {
        // Masked text frame: "Hello" with mask [0x37, 0xfa, 0x21, 0x3d]
        let data = vec![
            0x81, 0x85, 0x37, 0xfa, 0x21, 0x3d, 0x7f, 0x9f, 0x4d, 0x51, 0x58,
        ];
        let (frame, consumed) = Frame::parse(&data, 1024).unwrap();
        assert_eq!(consumed, 11);
        assert!(frame.fin);
        assert_eq!(frame.opcode, Opcode::Text);
        assert!(frame.masked);
        assert_eq!(frame.mask, Some([0x37, 0xfa, 0x21, 0x3d]));
        assert_eq!(frame.payload, Bytes::from("Hello"));
    }
}

