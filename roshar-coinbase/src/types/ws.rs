use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::CoinbaseTradeSide;

/// Subscribe/unsubscribe message for the Advanced Trade market data WebSocket
/// (`wss://advanced-trade-ws.coinbase.com`).
///
/// One message covers one channel and any number of products. A JWT is optional on the public
/// channels and is not sent. Coinbase closes the connection if no subscribe arrives within
/// 5 seconds. Unauthenticated messages are limited to 8 per second per IP. A subscribe to an
/// unknown channel, or a product channel without `product_ids`, gets the error
/// `authentication failure`; unknown product ids are dropped silently (the `subscriptions`
/// reply omits them).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoinbaseWssMessage {
    #[serde(rename = "type")]
    pub msg_type: String,
    pub channel: String,
    /// Required on product channels. Absent for `heartbeats`, whose schema has no `product_ids`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product_ids: Option<Vec<String>>,
}

impl CoinbaseWssMessage {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("failed to serialize CoinbaseWssMessage")
    }

    fn with_products(msg_type: &str, channel: &str, product_ids: &[&str]) -> Self {
        Self {
            msg_type: msg_type.to_string(),
            channel: channel.to_string(),
            product_ids: Some(product_ids.iter().map(|s| s.to_string()).collect()),
        }
    }

    /// Subscribe to the level2 orderbook channel. Messages arrive on channel `l2_data`.
    pub fn depth(product_ids: &[&str]) -> Self {
        Self::with_products("subscribe", "level2", product_ids)
    }

    /// Unsubscribe from the level2 channel. An empty list unsubscribes from the whole channel.
    pub fn depth_unsub(product_ids: &[&str]) -> Self {
        Self::with_products("unsubscribe", "level2", product_ids)
    }

    /// Subscribe to the market_trades channel.
    pub fn trades(product_ids: &[&str]) -> Self {
        Self::with_products("subscribe", "market_trades", product_ids)
    }

    /// Unsubscribe from the market_trades channel. An empty list unsubscribes from the whole
    /// channel.
    pub fn trades_unsub(product_ids: &[&str]) -> Self {
        Self::with_products("unsubscribe", "market_trades", product_ids)
    }

    /// Subscribe to the ticker channel (an update on every match, with best bid/ask).
    pub fn ticker(product_ids: &[&str]) -> Self {
        Self::with_products("subscribe", "ticker", product_ids)
    }

    pub fn ticker_unsub(product_ids: &[&str]) -> Self {
        Self::with_products("unsubscribe", "ticker", product_ids)
    }

    /// Subscribe to the ticker_batch channel (ticker every 5 s, without best bid/ask).
    pub fn ticker_batch(product_ids: &[&str]) -> Self {
        Self::with_products("subscribe", "ticker_batch", product_ids)
    }

    pub fn ticker_batch_unsub(product_ids: &[&str]) -> Self {
        Self::with_products("unsubscribe", "ticker_batch", product_ids)
    }

    /// Subscribe to the candles channel (five-minute candles, updated every second).
    pub fn candles(product_ids: &[&str]) -> Self {
        Self::with_products("subscribe", "candles", product_ids)
    }

    pub fn candles_unsub(product_ids: &[&str]) -> Self {
        Self::with_products("unsubscribe", "candles", product_ids)
    }

    /// Subscribe to the status channel (product metadata and trading status).
    pub fn status(product_ids: &[&str]) -> Self {
        Self::with_products("subscribe", "status", product_ids)
    }

    pub fn status_unsub(product_ids: &[&str]) -> Self {
        Self::with_products("unsubscribe", "status", product_ids)
    }

    /// Subscribe to the heartbeats channel. Coinbase closes most channels after 60-90 seconds
    /// without updates; heartbeats arrive every second and keep the connection open.
    pub fn heartbeats() -> Self {
        Self {
            msg_type: "subscribe".to_string(),
            channel: "heartbeats".to_string(),
            product_ids: None,
        }
    }

    pub fn heartbeats_unsub() -> Self {
        Self {
            msg_type: "unsubscribe".to_string(),
            channel: "heartbeats".to_string(),
            product_ids: None,
        }
    }
}

/// A message received on the market data WebSocket.
#[derive(Debug, Clone)]
pub enum CoinbaseWsMessage {
    Channel(CoinbaseWsChannelMessage),
    /// `{"type":"error","message":"..."}`, e.g. `authentication failure` for an invalid
    /// subscribe.
    Error(CoinbaseWsError),
}

impl CoinbaseWsMessage {
    /// Parse one text frame. A frame that is neither a known channel message nor an error frame
    /// returns the channel-message parse error.
    pub fn parse(text: &str) -> Result<Self, serde_json::Error> {
        match serde_json::from_str::<CoinbaseWsChannelMessage>(text) {
            Ok(m) => Ok(Self::Channel(m)),
            Err(e) => match serde_json::from_str::<CoinbaseWsErrorFrame>(text) {
                Ok(CoinbaseWsErrorFrame::Error(err)) => Ok(Self::Error(err)),
                Err(_) => Err(e),
            },
        }
    }
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum CoinbaseWsErrorFrame {
    Error(CoinbaseWsError),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoinbaseWsError {
    pub message: String,
}

/// Channel messages, keyed by the envelope `channel` field.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "channel")]
pub enum CoinbaseWsChannelMessage {
    /// Data of the `level2` subscription.
    #[serde(rename = "l2_data")]
    L2Data(CoinbaseWsEnvelope<CoinbaseWsL2Event>),
    #[serde(rename = "market_trades")]
    MarketTrades(CoinbaseWsEnvelope<CoinbaseWsMarketTradesEvent>),
    #[serde(rename = "ticker")]
    Ticker(CoinbaseWsEnvelope<CoinbaseWsTickerEvent>),
    #[serde(rename = "ticker_batch")]
    TickerBatch(CoinbaseWsEnvelope<CoinbaseWsTickerBatchEvent>),
    #[serde(rename = "candles")]
    Candles(CoinbaseWsEnvelope<CoinbaseWsCandlesEvent>),
    #[serde(rename = "status")]
    Status(CoinbaseWsEnvelope<CoinbaseWsStatusEvent>),
    #[serde(rename = "heartbeats")]
    Heartbeats(CoinbaseWsEnvelope<CoinbaseWsHeartbeatEvent>),
    /// Sent after every subscribe/unsubscribe with the full current subscription set.
    #[serde(rename = "subscriptions")]
    Subscriptions(CoinbaseWsEnvelope<CoinbaseWsSubscriptionsEvent>),
}

/// Common envelope of channel messages.
#[derive(Debug, Clone, Deserialize)]
pub struct CoinbaseWsEnvelope<E> {
    /// RFC 3339 server send time.
    pub timestamp: String,
    /// Per-connection sequence number, shared by all channels; a gap means a dropped message.
    pub sequence_num: u64,
    pub events: Vec<E>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CoinbaseWsEventType {
    Snapshot,
    Update,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CoinbaseWsL2Event {
    #[serde(rename = "type")]
    pub event_type: CoinbaseWsEventType,
    pub product_id: String,
    pub updates: Vec<CoinbaseWsL2Update>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CoinbaseWsL2Side {
    Bid,
    Offer,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CoinbaseWsL2Update {
    pub side: CoinbaseWsL2Side,
    /// RFC 3339 matching-engine time.
    pub event_time: String,
    pub price_level: String,
    /// New size at the level (not a delta). "0" removes the level.
    pub new_quantity: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CoinbaseWsMarketTradesEvent {
    #[serde(rename = "type")]
    pub event_type: CoinbaseWsEventType,
    pub trades: Vec<CoinbaseWsMarketTrade>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CoinbaseWsMarketTrade {
    pub product_id: String,
    pub trade_id: String,
    pub price: String,
    pub size: String,
    /// RFC 3339.
    pub time: String,
    /// The maker side of the trade.
    pub side: CoinbaseTradeSide,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CoinbaseWsTickerEvent {
    #[serde(rename = "type")]
    pub event_type: CoinbaseWsEventType,
    pub tickers: Vec<CoinbaseWsTicker>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CoinbaseWsTicker {
    /// Always "ticker".
    #[serde(rename = "type")]
    pub ticker_type: String,
    pub product_id: String,
    pub price: String,
    pub volume_24_h: String,
    pub low_24_h: String,
    pub high_24_h: String,
    pub low_52_w: String,
    pub high_52_w: String,
    pub price_percent_chg_24_h: String,
    pub best_bid: String,
    pub best_ask: String,
    pub best_bid_quantity: String,
    pub best_ask_quantity: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CoinbaseWsTickerBatchEvent {
    #[serde(rename = "type")]
    pub event_type: CoinbaseWsEventType,
    pub tickers: Vec<CoinbaseWsTickerBatch>,
}

/// `ticker_batch` ticker: the `ticker` fields without best bid/ask.
#[derive(Debug, Clone, Deserialize)]
pub struct CoinbaseWsTickerBatch {
    /// Always "ticker".
    #[serde(rename = "type")]
    pub ticker_type: String,
    pub product_id: String,
    pub price: String,
    pub volume_24_h: String,
    pub low_24_h: String,
    pub high_24_h: String,
    pub low_52_w: String,
    pub high_52_w: String,
    pub price_percent_chg_24_h: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CoinbaseWsCandlesEvent {
    #[serde(rename = "type")]
    pub event_type: CoinbaseWsEventType,
    pub candles: Vec<CoinbaseWsCandle>,
}

/// Five-minute candle.
#[derive(Debug, Clone, Deserialize)]
pub struct CoinbaseWsCandle {
    /// UNIX seconds of the bucket start.
    pub start: String,
    pub high: String,
    pub low: String,
    pub open: String,
    pub close: String,
    pub volume: String,
    pub product_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CoinbaseWsStatusEvent {
    #[serde(rename = "type")]
    pub event_type: CoinbaseWsEventType,
    pub products: Vec<CoinbaseWsProductStatus>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CoinbaseWsProductStatus {
    /// For example "SPOT".
    pub product_type: String,
    pub id: String,
    pub base_currency: String,
    pub quote_currency: String,
    pub base_increment: String,
    pub quote_increment: String,
    pub display_name: String,
    /// For example "online".
    pub status: String,
    pub status_message: String,
    pub min_market_funds: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CoinbaseWsHeartbeatEvent {
    /// Go-formatted server time, e.g. "2026-11-16 11:03:52.097902815 +0000 UTC m=+286861.2".
    pub current_time: String,
    pub heartbeat_counter: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CoinbaseWsSubscriptionsEvent {
    /// Channel -> subscribed product ids. `heartbeats` maps to `["heartbeats"]`. Empty when the
    /// last request subscribed nothing valid.
    pub subscriptions: HashMap<String, Vec<String>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_depth_subscribe() {
        let msg = CoinbaseWssMessage::depth(&["BTC-USD", "ETH-USD"]);
        let parsed: serde_json::Value = serde_json::from_str(&msg.to_json()).unwrap();
        assert_eq!(parsed["type"], "subscribe");
        assert_eq!(parsed["channel"], "level2");
        assert_eq!(parsed["product_ids"][0], "BTC-USD");
        assert_eq!(parsed["product_ids"][1], "ETH-USD");
    }

    #[test]
    fn test_trades_subscribe() {
        let msg = CoinbaseWssMessage::trades(&["BTC-USD"]);
        let parsed: serde_json::Value = serde_json::from_str(&msg.to_json()).unwrap();
        assert_eq!(parsed["type"], "subscribe");
        assert_eq!(parsed["channel"], "market_trades");
        assert_eq!(parsed["product_ids"][0], "BTC-USD");
    }

    #[test]
    fn test_heartbeats_has_no_product_ids() {
        let json = CoinbaseWssMessage::heartbeats().to_json();
        assert_eq!(json, r#"{"type":"subscribe","channel":"heartbeats"}"#);
    }

    #[test]
    fn test_new_channel_builders() {
        let cases = [
            (
                CoinbaseWssMessage::ticker(&["BTC-USD"]),
                "subscribe",
                "ticker",
            ),
            (
                CoinbaseWssMessage::ticker_unsub(&["BTC-USD"]),
                "unsubscribe",
                "ticker",
            ),
            (
                CoinbaseWssMessage::ticker_batch(&["BTC-USD"]),
                "subscribe",
                "ticker_batch",
            ),
            (
                CoinbaseWssMessage::ticker_batch_unsub(&["BTC-USD"]),
                "unsubscribe",
                "ticker_batch",
            ),
            (
                CoinbaseWssMessage::candles(&["BTC-USD"]),
                "subscribe",
                "candles",
            ),
            (
                CoinbaseWssMessage::candles_unsub(&["BTC-USD"]),
                "unsubscribe",
                "candles",
            ),
            (
                CoinbaseWssMessage::status(&["BTC-USD"]),
                "subscribe",
                "status",
            ),
            (
                CoinbaseWssMessage::status_unsub(&["BTC-USD"]),
                "unsubscribe",
                "status",
            ),
        ];
        for (msg, ty, ch) in cases {
            let v: serde_json::Value = serde_json::from_str(&msg.to_json()).unwrap();
            assert_eq!(v["type"], ty);
            assert_eq!(v["channel"], ch);
            assert_eq!(v["product_ids"][0], "BTC-USD");
        }
    }
}
