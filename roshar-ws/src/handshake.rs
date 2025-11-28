use crate::error::{Error, Result};
use bytes::{BufMut, Bytes, BytesMut};
use std::collections::HashMap;

/// Generate a random WebSocket key for the handshake
pub fn generate_key() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let bytes: [u8; 16] = rng.gen();
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// Perform WebSocket handshake
pub fn create_handshake_request(uri: &str, host: &str, key: &str) -> Bytes {
    let mut request = BytesMut::new();

    request.put_slice(b"GET ");
    request.put_slice(uri.as_bytes());
    request.put_slice(b" HTTP/1.1\r\n");
    request.put_slice(b"Host: ");
    request.put_slice(host.as_bytes());
    request.put_slice(b"\r\n");
    request.put_slice(b"Upgrade: websocket\r\n");
    request.put_slice(b"Connection: Upgrade\r\n");
    request.put_slice(b"Sec-WebSocket-Key: ");
    request.put_slice(key.as_bytes());
    request.put_slice(b"\r\n");
    request.put_slice(b"Sec-WebSocket-Version: 13\r\n");
    request.put_slice(b"\r\n");

    request.freeze()
}

/// Parse handshake response
pub fn parse_handshake_response(response: &[u8]) -> Result<()> {
    let response_str = std::str::from_utf8(response)
        .map_err(|e| Error::Handshake(format!("Invalid UTF-8 in response: {}", e)))?;

    let mut lines = response_str.lines();
    let status_line = lines
        .next()
        .ok_or_else(|| Error::Handshake("Empty response".to_string()))?;

    if !status_line.contains("101") {
        return Err(Error::Handshake(format!(
            "Expected 101 Switching Protocols, got: {}",
            status_line
        )));
    }

    let mut headers = HashMap::new();
    for line in lines {
        if line.is_empty() {
            break;
        }
        if let Some((key, value)) = line.split_once(':') {
            headers.insert(key.trim().to_lowercase(), value.trim().to_string());
        }
    }

    // Verify required headers
    if headers.get("upgrade").map(|s| s.to_lowercase()) != Some("websocket".to_string()) {
        return Err(Error::Handshake(
            "Missing or invalid Upgrade header".to_string(),
        ));
    }

    if headers.get("connection").map(|s| s.to_lowercase()) != Some("upgrade".to_string()) {
        return Err(Error::Handshake(
            "Missing or invalid Connection header".to_string(),
        ));
    }

    Ok(())
}
