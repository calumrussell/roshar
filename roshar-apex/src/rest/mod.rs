pub mod market;
pub mod trading;

pub use market::MarketApi;
pub use trading::TradingApi;

use crate::types::ApexEnvelope;
use serde::de::DeserializeOwned;

pub type ApiResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// Appends `k=v` pairs to `path`, skipping unset values. Values are not percent-encoded: the API-key
/// signature covers the query exactly as sent, and every parameter here is an id, symbol, number or
/// enum token.
pub(crate) struct Query {
    pub(crate) path: String,
    has_params: bool,
}

impl Query {
    pub(crate) fn new(path: &str) -> Self {
        Self {
            path: path.to_string(),
            has_params: false,
        }
    }

    pub(crate) fn param(mut self, key: &str, value: impl std::fmt::Display) -> Self {
        self.path.push(if self.has_params { '&' } else { '?' });
        self.path.push_str(key);
        self.path.push('=');
        self.path.push_str(&value.to_string());
        self.has_params = true;
        self
    }

    pub(crate) fn opt(self, key: &str, value: Option<impl std::fmt::Display>) -> Self {
        match value {
            Some(v) => self.param(key, v),
            None => self,
        }
    }
}

/// Parses a reply: non-2xx status, an unparseable body or an error envelope are errors.
pub(crate) async fn parse_reply<T: DeserializeOwned>(
    response: reqwest::Response,
    what: &str,
) -> ApiResult<T> {
    let status = response.status();
    let text = response.text().await?;
    if !status.is_success() {
        return Err(format!("ApeX {what} failed ({status}): {text}").into());
    }
    let envelope: ApexEnvelope<T> = serde_json::from_str(&text)
        .map_err(|e| format!("Failed to parse ApeX {what}: {e}: {text}"))?;
    Ok(envelope.into_result()?)
}
