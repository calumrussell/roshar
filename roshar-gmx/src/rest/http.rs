use crate::types::GmxApiError;
use roshar_http::RateLimitedClient;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fmt::Display;

pub(crate) type ApiResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// Builds `{base}{path}?k=v&...`, percent-encoding values like `encodeURIComponent`.
/// Array parameters repeat the key (`symbols=a&symbols=b`), as the SDK's `query-string` does.
pub(crate) struct Query {
    pub url: String,
    has_params: bool,
}

impl Query {
    pub fn new(base: &str, path: &str) -> Self {
        Self {
            url: format!("{}{path}", base.trim_end_matches('/')),
            has_params: false,
        }
    }

    pub fn param(mut self, key: &str, value: impl Display) -> Self {
        self.url.push(if self.has_params { '&' } else { '?' });
        self.url.push_str(key);
        self.url.push('=');
        encode_into(&mut self.url, &value.to_string());
        self.has_params = true;
        self
    }

    pub fn opt(self, key: &str, value: Option<impl Display>) -> Self {
        match value {
            Some(v) => self.param(key, v),
            None => self,
        }
    }

    pub fn list<T: Display>(mut self, key: &str, values: &[T]) -> Self {
        for v in values {
            self = self.param(key, v);
        }
        self
    }
}

fn encode_into(out: &mut String, s: &str) {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    for &b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => {
                out.push('%');
                out.push(HEX[(b >> 4) as usize] as char);
                out.push(HEX[(b & 0xf) as usize] as char);
            }
        }
    }
}

async fn read<T: DeserializeOwned>(response: reqwest::Response, what: &str) -> ApiResult<T> {
    let status = response.status();
    let text = response.text().await?;
    if !status.is_success() {
        return Err(Box::new(GmxApiError::from_body(status.as_u16(), &text)));
    }
    serde_json::from_str(&text).map_err(|e| format!("Failed to parse GMX {what}: {e}").into())
}

pub(crate) async fn get_json<T: DeserializeOwned>(
    client: &RateLimitedClient,
    url: &str,
    what: &str,
) -> ApiResult<T> {
    let response = client.get(url).await?;
    read(response, what).await
}

pub(crate) async fn post_json<B: Serialize, T: DeserializeOwned>(
    client: &RateLimitedClient,
    url: &str,
    body: &B,
    what: &str,
) -> ApiResult<T> {
    let body = serde_json::to_string(body)?;
    let response = client
        .post(url)
        .await
        .header("content-type", "application/json")
        .body(body)
        .send()
        .await?;
    read(response, what).await
}
