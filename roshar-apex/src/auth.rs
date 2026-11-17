//! API-key request signing (`APEX-SIGNATURE`) and the private websocket login message.
//!
//! `signature = base64(HMAC-SHA256(key = base64(secret), timestamp + METHOD + path + data))`, where
//! `path` includes `/api` and, for GETs, the query string, and `data` is the POST form sorted by key
//! as `k=v` pairs joined with `&` (empty for GETs). The HMAC key is the base64 *encoding* of the
//! secret string, as the SDK does.

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use hmac::{Hmac, Mac};
use serde::Serialize;
use sha2::Sha256;

/// Request path the private websocket login signs.
pub const APEX_WS_LOGIN_PATH: &str = "/ws/accounts";

/// API key triple issued at registration (`apiKey.key`, `.secret`, `.passphrase`).
#[derive(Clone)]
pub struct ApexApiCredentials {
    pub key: String,
    pub secret: String,
    pub passphrase: String,
}

impl std::fmt::Debug for ApexApiCredentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ApexApiCredentials")
            .field("key", &self.key)
            .finish_non_exhaustive()
    }
}

impl ApexApiCredentials {
    /// `APEX-SIGNATURE` for a request. `timestamp_ms` is also sent as `APEX-TIMESTAMP`. `form` is the
    /// POST body as sent (any order); pass `&[]` for GETs and put the query in `request_path`.
    pub fn sign_request(
        &self,
        timestamp_ms: u64,
        method: &str,
        request_path: &str,
        form: &[(&str, String)],
    ) -> String {
        let mut sorted: Vec<&(&str, String)> = form.iter().collect();
        sorted.sort_by(|a, b| a.0.cmp(b.0));
        let mut message = format!("{timestamp_ms}{method}{request_path}");
        for (i, (key, value)) in sorted.into_iter().enumerate() {
            if i > 0 {
                message.push('&');
            }
            message.push_str(key);
            message.push('=');
            message.push_str(value);
        }
        self.hmac(&message)
    }

    fn hmac(&self, message: &str) -> String {
        let key = STANDARD.encode(self.secret.as_bytes());
        let mut mac =
            Hmac::<Sha256>::new_from_slice(key.as_bytes()).expect("HMAC accepts any key length");
        mac.update(message.as_bytes());
        STANDARD.encode(mac.finalize().into_bytes())
    }

    /// Private websocket login frame: `{"op":"login","args":["<request json>"]}`, signing
    /// `timestamp + "GET" + "/ws/accounts"`. `topics` are subscribed on login (e.g.
    /// `ws_zk_accounts_v3`, `ws_notify_v1`).
    pub fn ws_login_message(&self, timestamp_ms: u64, topics: &[&str]) -> String {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct LoginRequest<'a> {
            #[serde(rename = "type")]
            kind: &'a str,
            topics: &'a [&'a str],
            http_method: &'a str,
            request_path: &'a str,
            api_key: &'a str,
            passphrase: &'a str,
            timestamp: u64,
            signature: String,
        }
        let request = LoginRequest {
            kind: "login",
            topics,
            http_method: "GET",
            request_path: APEX_WS_LOGIN_PATH,
            api_key: &self.key,
            passphrase: &self.passphrase,
            timestamp: timestamp_ms,
            signature: self.sign_request(timestamp_ms, "GET", APEX_WS_LOGIN_PATH, &[]),
        };
        let inner = serde_json::to_string(&request).expect("login request serializes");
        serde_json::json!({ "op": "login", "args": [inner] }).to_string()
    }
}
