pub mod market;
pub mod spot;

pub use market::MarketApi;
pub use spot::SpotMarketApi;

use crate::types::{BitgetResponse, BitgetStatus};
use crate::BITGET_REST_URL;
use roshar_http::RateLimitedClient;
use serde::de::DeserializeOwned;
use std::fmt::Display;

pub(crate) type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// Builds `BITGET_REST_URL + path ? k=v&...`, skipping `None` values.
pub(crate) struct Query {
    url: String,
    has_query: bool,
}

impl Query {
    pub(crate) fn new(path: &str) -> Self {
        Self {
            url: format!("{BITGET_REST_URL}{path}"),
            has_query: false,
        }
    }

    pub(crate) fn param(mut self, key: &str, value: impl Display) -> Self {
        use std::fmt::Write;
        let sep = if self.has_query { '&' } else { '?' };
        self.has_query = true;
        // Writing to a String cannot fail.
        let _ = write!(self.url, "{sep}{key}={value}");
        self
    }

    pub(crate) fn opt(self, key: &str, value: Option<impl Display>) -> Self {
        match value {
            Some(v) => self.param(key, v),
            None => self,
        }
    }

    pub(crate) fn build(self) -> String {
        self.url
    }
}

/// GET `url` and return the `data` of a successful (`code == "00000"`) reply.
/// On error Bitget sends `data: null`, so `code`/`msg` are checked before `data` is parsed.
pub(crate) async fn get_data<T: DeserializeOwned>(
    client: &RateLimitedClient,
    url: &str,
    what: &str,
) -> Result<T, BoxError> {
    let response = client.get(url).await?;
    let status = response.status();
    let text = response.text().await?;
    parse_data(&text, status.as_u16(), what)
}

/// Parse a Bitget REST body: surface `code`/`msg` on error, then parse `data` strictly.
pub(crate) fn parse_data<T: DeserializeOwned>(
    text: &str,
    http_status: u16,
    what: &str,
) -> Result<T, BoxError> {
    let head: BitgetStatus = serde_json::from_str(text).map_err(|e| {
        format!("Failed to parse Bitget {what} response (HTTP {http_status}): {e}: {text}")
    })?;
    if head.code != "00000" {
        return Err(format!(
            "Bitget API error {} (HTTP {http_status}): {}",
            head.code, head.msg
        )
        .into());
    }
    if !(200..300).contains(&http_status) {
        return Err(format!("Bitget API request failed with status: {http_status}").into());
    }

    let parsed: BitgetResponse<T> = serde_json::from_str(text)
        .map_err(|e| format!("Failed to parse Bitget {what} response: {e}"))?;
    Ok(parsed.data)
}
