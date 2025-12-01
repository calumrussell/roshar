use std::collections::HashMap;

use chrono::Utc;
use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

pub struct AuthApi;

impl AuthApi {
    const RECV_WINDOW: &'static str = "5000";

    pub fn generate_timestamp() -> String {
        Utc::now().timestamp_millis().to_string()
    }

    pub fn get_signature(
        api_secret: &str,
        timestamp: &str,
        api_key: &str,
        recv_window: &str,
        payload: &str,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        // Create the signature string: timestamp + api_key + recv_window + payload
        let signature_string = format!("{timestamp}{api_key}{recv_window}{payload}");

        // Create HMAC-SHA256
        let mut mac = HmacSha256::new_from_slice(api_secret.as_bytes())?;
        mac.update(signature_string.as_bytes());
        let result = mac.finalize();

        // Convert to lowercase hex
        Ok(hex::encode(result.into_bytes()))
    }

    fn get_env() -> (String, String) {
        let api_key = std::env::var("BYBIT_API_KEY").expect("BYBIT_API_KEY required");
        let api_secret = std::env::var("BYBIT_API_SECRET").expect("BYBIT_API_SECRET required");

        (api_key, api_secret)
    }

    pub fn create_headers_post(
        json_body: &str,
    ) -> Result<HashMap<String, String>, Box<dyn std::error::Error + Send + Sync>> {
        let (api_key, api_secret) = Self::get_env();
        let timestamp = Self::generate_timestamp();
        let recv_window = Self::RECV_WINDOW;

        let signature =
            Self::get_signature(&api_secret, &timestamp, &api_key, recv_window, json_body)?;

        let mut headers = HashMap::new();
        headers.insert("X-BAPI-API-KEY".to_string(), api_key);
        headers.insert("X-BAPI-TIMESTAMP".to_string(), timestamp);
        headers.insert("X-BAPI-SIGN".to_string(), signature);
        headers.insert("X-BAPI-RECV-WINDOW".to_string(), recv_window.to_string());
        headers.insert("Content-Type".to_string(), "application/json".to_string());

        Ok(headers)
    }

    pub fn is_testnet() -> bool {
        std::env::var("BYBIT_TESTNET")
            .unwrap_or_else(|_| "false".to_string())
            .parse()
            .unwrap_or(false)
    }

    pub fn get_base_url() -> String {
        if Self::is_testnet() {
            "https://api-testnet.bybit.com".to_string()
        } else {
            "https://api.bybit.com".to_string()
        }
    }
}
