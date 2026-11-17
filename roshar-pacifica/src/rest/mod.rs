pub mod market;
pub mod trading;
pub use market::MarketApi;
pub use trading::TradingApi;

use crate::types::PacificaResponse;
use serde::de::DeserializeOwned;

pub(crate) const MAINNET_BASE_URL: &str = "https://api.pacifica.fi/api/v1";
pub(crate) const TESTNET_BASE_URL: &str = "https://test-api.pacifica.fi/api/v1";

pub(crate) type BoxError = Box<dyn std::error::Error + Send + Sync>;

pub(crate) fn base_url(is_mainnet: bool) -> &'static str {
    if is_mainnet {
        MAINNET_BASE_URL
    } else {
        TESTNET_BASE_URL
    }
}

/// Parses a REST reply's envelope. A non-2xx status or `success: false` is an error.
pub(crate) fn parse_envelope<T: DeserializeOwned>(
    status_ok: bool,
    status: impl std::fmt::Display,
    text: &str,
    what: &str,
) -> Result<PacificaResponse<T>, BoxError> {
    if !status_ok {
        return Err(format!("Pacifica {what} failed ({status}): {text}").into());
    }
    let reply: PacificaResponse<T> = serde_json::from_str(text)
        .map_err(|e| format!("Failed to parse Pacifica {what} reply: {e}: {text}"))?;
    if !reply.success {
        return Err(format!("Pacifica {what} failed ({status}): {text}").into());
    }
    Ok(reply)
}

/// Parses a REST reply and returns its `data`, which must be present.
pub(crate) fn parse_data<T: DeserializeOwned>(
    status_ok: bool,
    status: impl std::fmt::Display,
    text: &str,
    what: &str,
) -> Result<T, BoxError> {
    parse_envelope::<T>(status_ok, status, text, what)?
        .data
        .ok_or_else(|| format!("Pacifica {what} reply has no data: {text}").into())
}
