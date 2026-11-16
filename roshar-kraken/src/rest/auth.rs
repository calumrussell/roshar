use std::collections::HashMap;

use base64::{engine::general_purpose, Engine as _};
use chrono::Utc;
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256, Sha512};

type HmacSha512 = Hmac<Sha512>;

/// Kraken Futures API key pair. The library never reads these from the environment.
#[derive(Clone)]
pub struct KrakenCredentials {
    pub public_key: String,
    /// Base64-encoded API secret.
    pub private_key: String,
}

impl KrakenCredentials {
    pub fn new(public_key: impl Into<String>, private_key: impl Into<String>) -> Self {
        Self {
            public_key: public_key.into(),
            private_key: private_key.into(),
        }
    }
}

impl std::fmt::Debug for KrakenCredentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KrakenCredentials")
            .field("public_key", &self.public_key)
            .field("private_key", &"<redacted>")
            .finish()
    }
}

pub(crate) struct AuthApi;

impl AuthApi {
    pub fn generate_nonce() -> i64 {
        Utc::now().timestamp_millis()
    }

    /// `Authent` per https://docs.kraken.com/exchange/guides/futures/rest#authent:
    /// base64(HMAC-SHA512(base64decode(secret), SHA256(postData + nonce + endpointPath))), where
    /// `endpointPath` drops the `/derivatives` prefix (as krakenfx/api-go `derivatives.Sign` does).
    /// `data` must be the url-encoded string exactly as sent.
    pub fn get_signature(
        private_key: &str,
        nonce: &str,
        data: &str,
        path: &str,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let processed_path = path.strip_prefix("/derivatives").unwrap_or(path);
        let message_string = format!("{data}{nonce}{processed_path}");

        let mut hasher = Sha256::new();
        hasher.update(message_string.as_bytes());
        let hash = hasher.finalize();

        Self::sign(private_key, &hash)
    }

    fn sign(private_key: &str, message: &[u8]) -> Result<String, Box<dyn std::error::Error>> {
        // Decode the base64 private key
        let key = general_purpose::STANDARD.decode(private_key)?;

        // Create HMAC-SHA512
        let mut mac = HmacSha512::new_from_slice(&key)?;
        mac.update(message);
        let result = mac.finalize();

        // Return base64 encoded signature
        Ok(general_purpose::STANDARD.encode(result.into_bytes()))
    }

    pub fn create_headers(
        credentials: &KrakenCredentials,
        endpoint: &str,
        post_data: &str,
    ) -> Result<HashMap<String, String>, Box<dyn std::error::Error>> {
        let nonce = Self::generate_nonce().to_string();

        let signature = Self::get_signature(&credentials.private_key, &nonce, post_data, endpoint)?;

        let mut headers = HashMap::new();
        headers.insert("APIKey".to_string(), credentials.public_key.clone());
        headers.insert("Authent".to_string(), signature);
        headers.insert("Nonce".to_string(), nonce);

        Ok(headers)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Vector from the official docs (Derivatives WebSocket "Sign challenge" example), which uses
    /// the same SHA-256 then HMAC-SHA-512 pipeline as `Authent` with empty nonce and path:
    /// https://docs.kraken.com/exchange/guides/futures/websockets#sign-challenge
    #[test]
    fn matches_documented_vector() {
        let secret =
            "7zxMEF5p/Z8l2p2U7Ghv6x14Af+Fx+92tPgUdVQ748FOIrEoT9bgT+bTRfXc5pz8na+hL/QdrCVG7bh9KpT0eMTm";
        let sig =
            AuthApi::get_signature(secret, "", "c100b894-1729-464d-ace1-52dbce11db42", "").unwrap();
        assert_eq!(
            sig,
            "4JEpF3ix66GA2B+ooK128Ift4XQVtc137N9yeg4Kqsn9PI0Kpzbysl9M1IeCEdjg0zl00wkVqcsnG4bmnlMb3A=="
        );
    }

    #[test]
    fn strips_derivatives_prefix() {
        let secret =
            "7zxMEF5p/Z8l2p2U7Ghv6x14Af+Fx+92tPgUdVQ748FOIrEoT9bgT+bTRfXc5pz8na+hL/QdrCVG7bh9KpT0eMTm";
        let a =
            AuthApi::get_signature(secret, "1", "x=1", "/derivatives/api/v3/sendorder").unwrap();
        let b = AuthApi::get_signature(secret, "1", "x=1", "/api/v3/sendorder").unwrap();
        assert_eq!(a, b);
    }
}
