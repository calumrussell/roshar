pub mod market;
pub mod trade;

pub use market::MarketApi;
pub use trade::TradeApi;

use crate::types::{SynthetixEnvelope, SynthetixPlainError};
use serde::de::DeserializeOwned;

pub(crate) type ApiResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

pub const SYNTHETIX_API_URL: &str = "https://papi.synthetix.io";
pub const SYNTHETIX_INFO_URL: &str = "https://papi.synthetix.io/v1/info";
pub const SYNTHETIX_TRADE_URL: &str = "https://papi.synthetix.io/v1/trade";

/// Parses a `{"status":"ok"|"error",…}` body. Error envelopes (sent with HTTP 4xx) surface the
/// exchange's code and message; anything else that is not an envelope (e.g. the 403 page sent
/// without a User-Agent) is reported with the HTTP status and body.
pub(crate) async fn read_envelope<T: DeserializeOwned>(
    response: reqwest::Response,
    what: &str,
) -> ApiResult<T> {
    let status = response.status();
    let text = response.text().await?;
    match serde_json::from_str::<SynthetixEnvelope<T>>(&text) {
        Ok(env) => Ok(env.into_result()?),
        Err(e) if status.is_success() => {
            Err(format!("Failed to parse Synthetix {what}: {e}: {text}").into())
        }
        Err(_) => Err(format!("Synthetix {what} failed ({status}): {text}").into()),
    }
}

/// Parses a body of the plain (non-enveloped) `GET` endpoints; errors there are
/// `{"error":"…"}`.
pub(crate) async fn read_plain<T: DeserializeOwned>(
    response: reqwest::Response,
    what: &str,
) -> ApiResult<T> {
    let status = response.status();
    let text = response.text().await?;
    if !status.is_success() {
        return match serde_json::from_str::<SynthetixPlainError>(&text) {
            Ok(e) => Err(format!("Synthetix {what} failed ({status}): {}", e.error).into()),
            Err(_) => Err(format!("Synthetix {what} failed ({status}): {text}").into()),
        };
    }
    serde_json::from_str::<T>(&text)
        .map_err(|e| format!("Failed to parse Synthetix {what}: {e}: {text}").into())
}
