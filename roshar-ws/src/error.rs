use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Invalid frame: {0}")]
    InvalidFrame(String),

    #[error("Incomplete frame: expected {expected} bytes, got {got}")]
    IncompleteFrame { expected: usize, got: usize },

    #[error("Frame too large: {size} bytes exceeds maximum of {max} bytes")]
    FrameTooLarge { size: usize, max: usize },

    #[error("Invalid opcode: {0}")]
    InvalidOpcode(u8),

    #[error("Invalid close code: {0}")]
    InvalidCloseCode(u16),

    #[error("Handshake error: {0}")]
    Handshake(String),

    #[error("Connection closed")]
    ConnectionClosed,

    #[error("TLS error: {0}")]
    Tls(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

