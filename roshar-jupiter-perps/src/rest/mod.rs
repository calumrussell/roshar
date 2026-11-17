pub mod market;
pub mod trading;

pub use market::MarketApi;
pub use trading::TradingApi;

use crate::types::{ErrorBody, JupiterPerpsError};
use serde::de::DeserializeOwned;
use std::fmt::Display;

/// Base URL of the Perps API. The OpenAPI document is served at `/v2/docs` and the official CLI
/// calls `/v2`; the document's `servers` entry still says `/v1`, which answers identically.
pub const JUPITER_PERPS_API_URL: &str = "https://perps-api.jup.ag/v2";

pub(crate) type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// Reads a reply. A non-2xx status with the API's error body becomes [`JupiterPerpsError`];
/// anything else that is not the expected JSON is an error with the raw body.
pub(crate) async fn parse<T: DeserializeOwned>(
    response: reqwest::Response,
    what: &str,
) -> Result<T, BoxError> {
    let status = response.status();
    let text = response.text().await?;
    if !status.is_success() {
        let error: BoxError = match serde_json::from_str::<ErrorBody>(&text) {
            Ok(body) => Box::new(JupiterPerpsError {
                status: status.as_u16(),
                code: body.code,
                message: body.message,
                metadata: body.metadata,
            }),
            Err(_) => format!("Jupiter Perps {what} failed ({status}): {text}").into(),
        };
        return Err(error);
    }
    serde_json::from_str(&text)
        .map_err(|e| format!("Failed to parse Jupiter Perps {what}: {e}: {text}").into())
}

/// Builds `{base}/{path}?k=v&...`, skipping unset parameters. Values are base58 keys, integers
/// and enum literals, which need no escaping.
pub(crate) struct Query {
    pub(crate) url: String,
    has_params: bool,
}

impl Query {
    pub(crate) fn new(base_url: &str, path: &str) -> Self {
        Self {
            url: format!("{base_url}/{path}"),
            has_params: false,
        }
    }

    pub(crate) fn param(mut self, key: &str, value: impl Display) -> Self {
        self.url.push(if self.has_params { '&' } else { '?' });
        self.url.push_str(key);
        self.url.push('=');
        self.url.push_str(&value.to_string());
        self.has_params = true;
        self
    }

    pub(crate) fn opt(self, key: &str, value: Option<impl Display>) -> Self {
        match value {
            Some(v) => self.param(key, v),
            None => self,
        }
    }
}
