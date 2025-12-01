use std::collections::HashMap;

use base64::{engine::general_purpose, Engine as _};
use chrono::Utc;
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256, Sha512};

type HmacSha512 = Hmac<Sha512>;

pub struct AuthApi;

impl AuthApi {
    pub fn generate_nonce() -> i64 {
        Utc::now().timestamp_millis()
    }

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

    fn get_env() -> (String, String) {
        let private_key = std::env::var("KRAKEN_PRIVATE_KEY").unwrap();
        let public_key = std::env::var("KRAKEN_PUBLIC_KEY").unwrap();

        (public_key, private_key)
    }

    pub fn create_headers(
        endpoint: &str,
        post_data: &str,
    ) -> Result<HashMap<String, String>, Box<dyn std::error::Error>> {
        let (public_key, private_key) = Self::get_env();
        let nonce = Self::generate_nonce().to_string();

        let signature = Self::get_signature(&private_key, &nonce, post_data, endpoint)?;

        let mut headers = HashMap::new();
        headers.insert("APIKey".to_string(), public_key);
        headers.insert("Authent".to_string(), signature);
        headers.insert("Nonce".to_string(), nonce);
        headers.insert("Content-Type".to_string(), "application/json".to_string());

        Ok(headers)
    }
}
