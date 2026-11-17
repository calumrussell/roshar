pub mod market;
pub mod trading;

pub use market::MarketApi;
pub use trading::{SignedRequest, TradingApi};

use crate::types::EdgexResponse;
use serde::de::DeserializeOwned;

pub(crate) type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// Percent-encodes a query value. Unreserved characters and `,` (the list separator the SDK
/// sends unencoded) pass through.
pub(crate) fn encode_query_value(v: &str) -> String {
    let mut out = String::with_capacity(v.len());
    for b in v.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b',' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

pub(crate) fn build_query(params: &[(&str, String)]) -> String {
    params
        .iter()
        .map(|(k, v)| format!("{k}={}", encode_query_value(v)))
        .collect::<Vec<_>>()
        .join("&")
}

fn parse_envelope<T: DeserializeOwned>(
    status: u16,
    text: &str,
    what: &str,
) -> Result<Option<T>, BoxError> {
    let reply: EdgexResponse<T> = serde_json::from_str(text).map_err(|e| -> BoxError {
        format!("edgeX {what} failed (HTTP {status}): unparseable reply: {e}: {text}").into()
    })?;
    let data = reply
        .into_result()
        .map_err(|e| -> BoxError { Box::new(e) })?;
    if status != 200 {
        return Err(format!("edgeX {what} failed (HTTP {status}): {text}").into());
    }
    Ok(data)
}

/// Parses a reply whose `data` must be present.
pub(crate) fn parse_data<T: DeserializeOwned>(
    status: u16,
    text: &str,
    what: &str,
) -> Result<T, BoxError> {
    parse_envelope::<T>(status, text, what)?
        .ok_or_else(|| format!("edgeX {what} reply has no data: {text}").into())
}

/// Parses a reply of a void endpoint (`data: null`).
pub(crate) fn parse_void(status: u16, text: &str, what: &str) -> Result<(), BoxError> {
    parse_envelope::<serde_json::Value>(status, text, what).map(|_| ())
}

pub(crate) async fn read_reply(resp: reqwest::Response) -> Result<(u16, String), BoxError> {
    let status = resp.status().as_u16();
    let text = resp.text().await?;
    Ok((status, text))
}
