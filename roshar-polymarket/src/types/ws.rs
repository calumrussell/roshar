//! CLOB market websocket (`wss://ws-subscriptions-clob.polymarket.com/ws/market`) frames.
//!
//! Spec: https://docs.polymarket.com/api-reference/wss/market (AsyncAPI
//! https://docs.polymarket.com/asyncapi.json). Behaviour below was checked live on 2026-11-16:
//!
//! - The first client frame must be a [`MarketWsSubscription`]. Anything else, including the text
//!   `PING`, closes the socket with 1008 `invalid subscription payload`.
//! - The server answers the subscription with one JSON array of `book` snapshots (one per known
//!   asset; `[]` when no id is known), then streams single JSON objects.
//! - A second `type: "market"` frame is answered with the text `INVALID OPERATION`; use
//!   [`MarketWsUpdate`] to change the asset set. Each `subscribe` update is answered with another
//!   array of `book` snapshots.
//! - The text `PING` is answered with the text `PONG`.
//! - Decimals and millisecond timestamps are JSON strings; they are parsed strictly to `f64` /
//!   `u64`.

use crate::types::de::{f64_from_str, opt_f64_from_str, opt_f64_from_str_empty_none, u64_from_str};
use serde::{de, Deserialize, Serialize};

/// Text heartbeat; the server answers `PONG`. Must not be sent before the subscription frame.
pub const MARKET_WS_PING: &str = "PING";
/// Server reply to [`MARKET_WS_PING`].
pub const MARKET_WS_PONG: &str = "PONG";
/// Server reply to a frame it does not accept after the subscription (e.g. a second
/// subscription frame).
pub const MARKET_WS_INVALID_OPERATION: &str = "INVALID OPERATION";

/// Opening frame: `{"type": "market", "assets_ids": [...], ...}`. `None` fields are omitted so the
/// server default applies.
#[derive(Debug, Clone, Default, Serialize)]
pub struct MarketWsSubscription {
    /// CLOB token ids.
    pub assets_ids: Vec<String>,
    /// Server default `true`: send a `book` snapshot per asset on subscribe. Only honoured on the
    /// opening frame.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initial_dump: Option<bool>,
    /// Documented as 1, 2 or 3 (default 2). Live, levels 1-3 produced the same frames, and the
    /// server did not reject 4.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub level: Option<u8>,
    /// Server default `false`. `true` adds `best_bid_ask`, `new_market` and `market_resolved`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_feature_enabled: Option<bool>,
}

impl MarketWsSubscription {
    pub fn new(assets_ids: &[&str]) -> Self {
        Self {
            assets_ids: assets_ids.iter().map(|s| s.to_string()).collect(),
            ..Default::default()
        }
    }

    pub fn to_json(&self) -> String {
        #[derive(Serialize)]
        struct Frame<'a> {
            #[serde(rename = "type")]
            kind: &'static str,
            #[serde(flatten)]
            sub: &'a MarketWsSubscription,
        }
        serde_json::to_string(&Frame {
            kind: "market",
            sub: self,
        })
        .expect("subscription serializes")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum MarketWsOperation {
    Subscribe,
    Unsubscribe,
}

/// Change the asset set of an open connection: `{"operation": ..., "assets_ids": [...]}`.
#[derive(Debug, Clone, Serialize)]
pub struct MarketWsUpdate {
    pub operation: MarketWsOperation,
    pub assets_ids: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub level: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_feature_enabled: Option<bool>,
}

impl MarketWsUpdate {
    pub fn new(operation: MarketWsOperation, assets_ids: &[&str]) -> Self {
        Self {
            operation,
            assets_ids: assets_ids.iter().map(|s| s.to_string()).collect(),
            level: None,
            custom_feature_enabled: None,
        }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("update serializes")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum MarketWsSide {
    #[serde(rename = "BUY")]
    Buy,
    #[serde(rename = "SELL")]
    Sell,
}

/// One text frame from the market channel.
#[derive(Debug, Clone, PartialEq)]
pub enum MarketWsMessage {
    /// Reply to `PING`.
    Pong,
    /// Reply to a rejected frame (`INVALID OPERATION`).
    InvalidOperation,
    /// A JSON array: the `book` snapshots answering a subscription or `subscribe` update. Empty
    /// when none of the requested ids is known.
    Batch(Vec<MarketWsEvent>),
    /// A single JSON event.
    Event(MarketWsEvent),
}

impl MarketWsMessage {
    /// Parses one text frame. Unknown text and unknown `event_type`s are errors.
    pub fn parse(text: &str) -> Result<Self, serde_json::Error> {
        match text {
            MARKET_WS_PONG => return Ok(Self::Pong),
            MARKET_WS_INVALID_OPERATION => return Ok(Self::InvalidOperation),
            _ => {}
        }
        if text.trim_start().starts_with('[') {
            serde_json::from_str(text).map(Self::Batch)
        } else if text.trim_start().starts_with('{') {
            serde_json::from_str(text).map(Self::Event)
        } else {
            Err(de::Error::custom(format!(
                "unexpected market websocket text frame: {text}"
            )))
        }
    }
}

/// Market channel event, tagged by `event_type`.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(tag = "event_type", rename_all = "snake_case")]
pub enum MarketWsEvent {
    Book(MarketWsBook),
    PriceChange(MarketWsPriceChange),
    LastTradePrice(MarketWsLastTradePrice),
    TickSizeChange(MarketWsTickSizeChange),
    /// Needs `custom_feature_enabled`.
    BestBidAsk(MarketWsBestBidAsk),
    /// Needs `custom_feature_enabled`. Sent for every new market, not only subscribed ones.
    NewMarket(MarketWsNewMarket),
    /// Needs `custom_feature_enabled`.
    MarketResolved(MarketWsMarketResolved),
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct MarketWsLevel {
    #[serde(deserialize_with = "f64_from_str")]
    pub price: f64,
    #[serde(deserialize_with = "f64_from_str")]
    pub size: f64,
}

/// Full aggregated book of one asset. Sent on subscribe and after each trade.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct MarketWsBook {
    /// Condition id.
    pub market: String,
    pub asset_id: String,
    /// Milliseconds.
    #[serde(deserialize_with = "u64_from_str")]
    pub timestamp: u64,
    pub hash: String,
    /// Live order: ascending price, so the best bid is last.
    pub bids: Vec<MarketWsLevel>,
    /// Live order: descending price, so the best ask is last.
    pub asks: Vec<MarketWsLevel>,
    /// Sent only on subscription snapshots (not on post-trade snapshots).
    #[serde(default, deserialize_with = "opt_f64_from_str")]
    pub tick_size: Option<f64>,
    /// Sent only on subscription snapshots; `""` (here `None`) when the asset has never traded.
    #[serde(default, deserialize_with = "opt_f64_from_str_empty_none")]
    pub last_trade_price: Option<f64>,
}

/// Level deltas; each item carries the asset's new best bid/ask.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct MarketWsPriceChange {
    pub market: String,
    pub price_changes: Vec<MarketWsPriceChangeItem>,
    #[serde(deserialize_with = "u64_from_str")]
    pub timestamp: u64,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct MarketWsPriceChangeItem {
    pub asset_id: String,
    #[serde(deserialize_with = "f64_from_str")]
    pub price: f64,
    /// New aggregate size at `price`; `0` removes the level.
    #[serde(deserialize_with = "f64_from_str")]
    pub size: f64,
    pub side: MarketWsSide,
    pub hash: String,
    /// `0` when the bid side is empty.
    #[serde(deserialize_with = "f64_from_str")]
    pub best_bid: f64,
    #[serde(deserialize_with = "f64_from_str")]
    pub best_ask: f64,
}

/// A trade.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct MarketWsLastTradePrice {
    pub market: String,
    pub asset_id: String,
    #[serde(deserialize_with = "f64_from_str")]
    pub price: f64,
    #[serde(deserialize_with = "f64_from_str")]
    pub size: f64,
    #[serde(deserialize_with = "f64_from_str")]
    pub fee_rate_bps: f64,
    /// Taker side.
    pub side: MarketWsSide,
    #[serde(deserialize_with = "u64_from_str")]
    pub timestamp: u64,
    pub transaction_hash: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct MarketWsTickSizeChange {
    pub market: String,
    pub asset_id: String,
    #[serde(deserialize_with = "f64_from_str")]
    pub old_tick_size: f64,
    #[serde(deserialize_with = "f64_from_str")]
    pub new_tick_size: f64,
    #[serde(deserialize_with = "u64_from_str")]
    pub timestamp: u64,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct MarketWsBestBidAsk {
    pub market: String,
    pub asset_id: String,
    #[serde(deserialize_with = "f64_from_str")]
    pub best_bid: f64,
    #[serde(deserialize_with = "f64_from_str")]
    pub best_ask: f64,
    #[serde(deserialize_with = "f64_from_str")]
    pub spread: f64,
    #[serde(deserialize_with = "u64_from_str")]
    pub timestamp: u64,
}

/// Parent event of a `new_market` / `market_resolved` (`EventMessage`).
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct MarketWsEventMessage {
    pub id: String,
    pub ticker: String,
    pub slug: String,
    pub title: String,
    pub description: String,
}

/// Fee schedule of a new market. Not in the AsyncAPI spec; sent live.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct MarketWsFeeSchedule {
    #[serde(deserialize_with = "f64_from_str")]
    pub exponent: f64,
    #[serde(deserialize_with = "f64_from_str")]
    pub rate: f64,
    pub taker_only: bool,
    #[serde(deserialize_with = "f64_from_str")]
    pub rebate_rate: f64,
}

/// A market was created. Fields the spec lists as optional are `Option`; live frames carried all
/// of them, with `""` for the sports fields of non-sports markets.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct MarketWsNewMarket {
    /// Gamma market id.
    pub id: String,
    pub question: String,
    /// Condition id.
    pub market: String,
    pub slug: String,
    pub assets_ids: Vec<String>,
    pub outcomes: Vec<String>,
    #[serde(deserialize_with = "u64_from_str")]
    pub timestamp: u64,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub event_message: Option<MarketWsEventMessage>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    #[serde(default)]
    pub condition_id: Option<String>,
    #[serde(default)]
    pub active: Option<bool>,
    #[serde(default)]
    pub clob_token_ids: Option<Vec<String>>,
    #[serde(default)]
    pub sports_market_type: Option<String>,
    #[serde(default)]
    pub line: Option<String>,
    /// RFC 3339 or `""`.
    #[serde(default)]
    pub game_start_time: Option<String>,
    #[serde(default, deserialize_with = "opt_f64_from_str")]
    pub order_price_min_tick_size: Option<f64>,
    #[serde(default)]
    pub group_item_title: Option<String>,
    /// Not in the spec. Base taker fee as sent (e.g. `"1000"`).
    #[serde(default, deserialize_with = "opt_f64_from_str")]
    pub taker_base_fee: Option<f64>,
    /// Not in the spec.
    #[serde(default)]
    pub fees_enabled: Option<bool>,
    /// Not in the spec.
    #[serde(default)]
    pub fee_schedule: Option<MarketWsFeeSchedule>,
}

/// A market resolved.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct MarketWsMarketResolved {
    /// Gamma market id.
    pub id: String,
    /// Condition id.
    pub market: String,
    pub assets_ids: Vec<String>,
    pub winning_asset_id: String,
    pub winning_outcome: String,
    #[serde(deserialize_with = "u64_from_str")]
    pub timestamp: u64,
    #[serde(default)]
    pub event_message: Option<MarketWsEventMessage>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subscription_frames() {
        let sub = MarketWsSubscription::new(&["1", "2"]);
        assert_eq!(sub.to_json(), r#"{"type":"market","assets_ids":["1","2"]}"#);
        let sub = MarketWsSubscription {
            custom_feature_enabled: Some(true),
            initial_dump: Some(false),
            level: Some(2),
            ..MarketWsSubscription::new(&["1"])
        };
        assert_eq!(
            sub.to_json(),
            r#"{"type":"market","assets_ids":["1"],"initial_dump":false,"level":2,"custom_feature_enabled":true}"#
        );
        let up = MarketWsUpdate::new(MarketWsOperation::Unsubscribe, &["3"]);
        assert_eq!(
            up.to_json(),
            r#"{"operation":"unsubscribe","assets_ids":["3"]}"#
        );
    }

    #[test]
    fn text_frames() {
        assert_eq!(
            MarketWsMessage::parse("PONG").unwrap(),
            MarketWsMessage::Pong
        );
        assert_eq!(
            MarketWsMessage::parse("INVALID OPERATION").unwrap(),
            MarketWsMessage::InvalidOperation
        );
        assert_eq!(
            MarketWsMessage::parse("[]").unwrap(),
            MarketWsMessage::Batch(vec![])
        );
        assert!(MarketWsMessage::parse("HELLO").is_err());
        assert!(MarketWsMessage::parse(r#"{"event_type":"nope"}"#).is_err());
    }
}
