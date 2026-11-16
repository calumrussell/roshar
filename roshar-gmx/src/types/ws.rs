//! GMX API stream (`wss://{chain}.gmxapi.io/v1/stream`), as used by `@gmx-io/sdk` `WsStreamClient`.
//!
//! Requests: `{"op":"subscribe"|"unsubscribe","channels":[...]}`. The server answers each with
//! `{"op":"ack","channels":[...]}` listing the connection's current subscriptions, or
//! `{"op":"error","message":...,"channels":[...]}` for refused channels. Data arrives as
//! `{"ch":...,"type":"snapshot","serverTs":...,"originTs"?:...,"data":...}`: every frame is a full
//! snapshot of the channel. Public channels only; there are no account channels.

use super::market::{GmxCandlePeriod, GmxMinMaxPrice, GmxOhlcvCandle};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// A stream channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GmxWsChannel<'a> {
    /// `prices`: oracle min/max price of every token, keyed by token address.
    Prices,
    /// `candles:{symbol}:{period}`: the current candle of a token symbol such as `ETH`
    /// (market symbols like `BTC/USD [BTC-USDC]` are refused).
    Candles {
        symbol: &'a str,
        period: GmxCandlePeriod,
    },
}

impl GmxWsChannel<'_> {
    pub fn name(&self) -> String {
        match self {
            Self::Prices => "prices".to_string(),
            Self::Candles { symbol, period } => format!("candles:{symbol}:{}", period.as_str()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum GmxWsOp {
    Subscribe,
    Unsubscribe,
}

/// Client request frame.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GmxWsRequest {
    pub op: GmxWsOp,
    pub channels: Vec<String>,
}

impl GmxWsRequest {
    pub fn subscribe(channels: &[GmxWsChannel<'_>]) -> Self {
        Self {
            op: GmxWsOp::Subscribe,
            channels: channels.iter().map(GmxWsChannel::name).collect(),
        }
    }

    pub fn unsubscribe(channels: &[GmxWsChannel<'_>]) -> Self {
        Self {
            op: GmxWsOp::Unsubscribe,
            channels: channels.iter().map(GmxWsChannel::name).collect(),
        }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("GmxWsRequest serializes")
    }
}

/// Server control frame.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "op", rename_all = "lowercase")]
pub enum GmxWsControl {
    /// Current subscriptions of the connection after the request.
    Ack { channels: Vec<String> },
    /// Channels the server refused, e.g. `"unknown channel(s) ignored"`.
    Error {
        message: String,
        channels: Vec<String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum GmxWsFrameType {
    Snapshot,
}

/// Server data frame.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GmxWsSnapshot {
    pub ch: String,
    #[serde(rename = "type")]
    pub kind: GmxWsFrameType,
    /// Server send time, unix ms.
    pub server_ts: u64,
    /// Observation time of the data, unix ms (the `prices` channel only).
    #[serde(default)]
    pub origin_ts: Option<u64>,
    pub data: Value,
}

impl GmxWsSnapshot {
    /// `data` of a `prices` frame.
    pub fn prices(&self) -> serde_json::Result<HashMap<String, GmxMinMaxPrice>> {
        HashMap::<String, GmxMinMaxPrice>::deserialize(&self.data)
    }

    /// `data` of a `candles:*` frame (`timestamp` is the candle open, unix ms).
    pub fn candle(&self) -> serde_json::Result<GmxOhlcvCandle> {
        GmxOhlcvCandle::deserialize(&self.data)
    }
}

/// Any server frame.
#[derive(Debug, Clone)]
pub enum GmxWsFrame {
    Control(GmxWsControl),
    Snapshot(GmxWsSnapshot),
}

impl GmxWsFrame {
    /// Parses a text frame: frames with `op` are control frames, all others snapshots.
    pub fn parse(text: &str) -> serde_json::Result<Self> {
        let value: Value = serde_json::from_str(text)?;
        if value.get("op").is_some() {
            GmxWsControl::deserialize(value).map(Self::Control)
        } else {
            GmxWsSnapshot::deserialize(value).map(Self::Snapshot)
        }
    }
}
