//! JSON text frames on the Paradex WebSocket.
//!
//! With SBE negotiated, channel data for SBE-encoded channels arrives as
//! binary frames ([`crate::sbe`]); everything else is JSON-RPC 2.0 text:
//! subscribe/unsubscribe/modify_subscription acks, responses to `auth` and `order.*` methods,
//! errors, and `subscription` notifications for channels Paradex does not
//! encode in SBE (e.g. `system_status`, `balance_events`, `transfers`, and
//! private channels on accounts not served SBE).
//!
//! Docs: <https://docs.paradex.trade/ws/general-information/subscription-channels>,
//! <https://docs.paradex.trade/ws/general-information/error-handling>,
//! <https://docs.paradex.trade/ws/web-socket-channels/auth/auth>.

use crate::types::{ParadexMarketEvent, ParadexSystemState};
use anyhow::{anyhow, bail, Context, Result};
use serde::Deserialize;
use serde_json::Value;

/// Server timing fields carried on every JSON-RPC response (μs).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParadexWsTiming {
    pub us_in: i64,
    pub us_diff: i64,
    /// Present on every live response; omitted from the docs' subscribe
    /// response example.
    pub us_out: Option<i64>,
}

/// Acknowledgement of `subscribe`, `unsubscribe` or `modify_subscription`
/// (`result.channel`). The acks share one shape; correlate by `id`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParadexWsChannelAck {
    pub id: u64,
    pub channel: String,
    /// Scoped `markets_summary` subscribe ack: the market filter.
    pub markets: Option<Vec<String>>,
    /// `modify_subscription` ack: markets added (omitted when none were requested).
    pub add: Option<Vec<String>>,
    /// `modify_subscription` ack: markets removed (omitted when none were requested).
    pub remove: Option<Vec<String>>,
    pub timing: ParadexWsTiming,
}

/// Any other successful JSON-RPC response (`auth`, `order.*`, ...). The
/// result is kept as JSON; correlate by `id` and decode with the matching
/// typed helper (e.g. [`ParadexWsResult::auth`]).
#[derive(Debug, Clone, PartialEq)]
pub struct ParadexWsResult {
    pub id: u64,
    pub result: Value,
    pub timing: ParadexWsTiming,
}

/// Result of the `auth` method.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ParadexWsAuthResult {
    /// Opaque server node identifier; documented as a non-required property.
    #[serde(default)]
    pub node_id: Option<String>,
}

impl ParadexWsResult {
    /// Interpret this result as the response to an `auth` request.
    pub fn auth(&self) -> Result<ParadexWsAuthResult> {
        if !self.result.is_object() {
            bail!("Paradex auth result is not an object: {}", self.result);
        }
        ParadexWsAuthResult::deserialize(&self.result).context("Paradex auth result")
    }
}

/// JSON-RPC error response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParadexWsError {
    /// Request id; `0` when the server could not parse the request.
    pub id: u64,
    /// JSON-RPC code (-32700..-32600) or Paradex code (e.g. 40111, 40301).
    pub code: i64,
    pub message: String,
    /// Extra details; documented as optional.
    pub data: Option<String>,
    pub timing: ParadexWsTiming,
}

/// `{"method":"subscription","params":{"channel":..,"data":..}}` for a
/// channel delivered as JSON. `data` is left untyped here; decode it with the
/// typed helper matching `channel`.
#[derive(Debug, Clone, PartialEq)]
pub struct ParadexWsNotification {
    pub channel: String,
    pub data: Value,
}

impl ParadexWsNotification {
    /// Data of a `system_status` notification.
    pub fn system_status(&self) -> Result<ParadexSystemState> {
        if self.channel != "system_status" {
            bail!(
                "Paradex notification on {:?} is not system_status",
                self.channel
            );
        }
        ParadexSystemState::deserialize(&self.data).context("Paradex system_status data")
    }

    /// Data of a `markets` / `markets.{market}` notification.
    pub fn market_event(&self) -> Result<ParadexMarketEvent> {
        if self.channel != "markets" && !self.channel.starts_with("markets.") {
            bail!("Paradex notification on {:?} is not markets", self.channel);
        }
        ParadexMarketEvent::deserialize(&self.data).context("Paradex markets data")
    }
}

/// A parsed Paradex WebSocket JSON text frame.
#[derive(Debug, Clone, PartialEq)]
pub enum ParadexWsControl {
    ChannelAck(ParadexWsChannelAck),
    Result(ParadexWsResult),
    Error(ParadexWsError),
    Notification(ParadexWsNotification),
}

#[derive(Deserialize)]
struct RawFrame {
    jsonrpc: String,
    id: Option<u64>,
    method: Option<String>,
    params: Option<RawParams>,
    result: Option<Value>,
    error: Option<RawError>,
    #[serde(rename = "usIn")]
    us_in: Option<i64>,
    #[serde(rename = "usDiff")]
    us_diff: Option<i64>,
    #[serde(rename = "usOut")]
    us_out: Option<i64>,
}

#[derive(Deserialize)]
struct RawParams {
    channel: String,
    data: Value,
}

#[derive(Deserialize)]
struct RawError {
    code: i64,
    message: String,
    data: Option<String>,
}

#[derive(Deserialize)]
struct RawAck {
    channel: String,
    markets: Option<Vec<String>>,
    add: Option<Vec<String>>,
    remove: Option<Vec<String>>,
}

/// Parse one JSON text frame.
pub fn parse(text: &str) -> Result<ParadexWsControl> {
    let raw: RawFrame = serde_json::from_str(text).context("Paradex WS JSON frame")?;
    if raw.jsonrpc != "2.0" {
        bail!(
            "Paradex WS frame has jsonrpc {:?}, expected \"2.0\"",
            raw.jsonrpc
        );
    }

    if let Some(method) = raw.method {
        if method != "subscription" {
            bail!("Paradex WS frame has unknown method {:?}", method);
        }
        let params = raw
            .params
            .ok_or_else(|| anyhow!("Paradex WS subscription frame missing params"))?;
        return Ok(ParadexWsControl::Notification(ParadexWsNotification {
            channel: params.channel,
            data: params.data,
        }));
    }

    let id = raw
        .id
        .ok_or_else(|| anyhow!("Paradex WS response missing id"))?;
    let timing = ParadexWsTiming {
        us_in: raw
            .us_in
            .ok_or_else(|| anyhow!("Paradex WS response missing usIn"))?,
        us_diff: raw
            .us_diff
            .ok_or_else(|| anyhow!("Paradex WS response missing usDiff"))?,
        us_out: raw.us_out,
    };

    match (raw.result, raw.error) {
        (Some(_), Some(_)) => bail!("Paradex WS response has both result and error"),
        (None, None) => bail!("Paradex WS response has neither result nor error"),
        (None, Some(e)) => Ok(ParadexWsControl::Error(ParadexWsError {
            id,
            code: e.code,
            message: e.message,
            data: e.data,
            timing,
        })),
        (Some(result), None) => {
            if result.get("channel").is_some() {
                let ack = RawAck::deserialize(&result).context("Paradex WS channel ack")?;
                Ok(ParadexWsControl::ChannelAck(ParadexWsChannelAck {
                    id,
                    channel: ack.channel,
                    markets: ack.markets,
                    add: ack.add,
                    remove: ack.remove,
                    timing,
                }))
            } else {
                Ok(ParadexWsControl::Result(ParadexWsResult {
                    id,
                    result,
                    timing,
                }))
            }
        }
    }
}
