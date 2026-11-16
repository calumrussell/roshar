pub mod market;
pub mod trading;

pub use market::MarketApi;
pub use trading::TradingApi;

use crate::types::GrvtError;
use serde::de::DeserializeOwned;
use std::fmt;

pub(crate) type ApiResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// A non-2xx GRVT reply. `body` holds the parsed `{"code","message","status"}` envelope when the
/// reply had one, else `raw` holds the body text.
#[derive(Debug, Clone)]
pub struct GrvtApiError {
    pub what: String,
    pub http_status: u16,
    pub body: Option<GrvtError>,
    pub raw: String,
}

impl fmt::Display for GrvtApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.body {
            Some(e) => write!(
                f,
                "GRVT {} failed (HTTP {}): code {}: {}",
                self.what, self.http_status, e.code, e.message
            ),
            None => write!(
                f,
                "GRVT {} failed (HTTP {}): {}",
                self.what, self.http_status, self.raw
            ),
        }
    }
}

impl std::error::Error for GrvtApiError {}

/// Turns a reply into `T`, or a [`GrvtApiError`] for non-2xx statuses.
pub(crate) fn parse_reply<T: DeserializeOwned>(
    what: &str,
    status: u16,
    text: &str,
) -> ApiResult<T> {
    if !(200..300).contains(&status) {
        return Err(Box::new(GrvtApiError {
            what: what.to_string(),
            http_status: status,
            body: serde_json::from_str(text).ok(),
            raw: text.to_string(),
        }));
    }
    serde_json::from_str(text).map_err(|e| format!("Failed to parse GRVT {what}: {e}").into())
}
