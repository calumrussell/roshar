use anyhow::{Result, anyhow};
use chrono::DateTime;
use serde::{Deserialize, Serialize};

use crate::{DepthUpdate, LocalOrderBook, Trade, Venue};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KrakenSpotLevel {
    pub price: f64,
    pub qty: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KrakenSpotBookData {
    pub symbol: String,
    pub bids: Vec<KrakenSpotLevel>,
    pub asks: Vec<KrakenSpotLevel>,
    pub checksum: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KrakenSpotBookMessage {
    pub channel: String,
    #[serde(rename = "type")]
    pub msg_type: String, // "snapshot" or "update"
    pub data: Vec<KrakenSpotBookData>,
}

// Kraken Spot Trade Structures
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KrakenSpotTradeData {
    pub price: f64,
    pub qty: f64,
    pub side: String, // "buy" or "sell"
    pub timestamp: String,
    pub ord_type: String, // "limit" or "market"
    pub trade_id: i64,
    pub symbol: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KrakenSpotTradeMessage {
    pub channel: String,
    #[serde(rename = "type")]
    pub msg_type: String,
    pub data: Vec<KrakenSpotTradeData>,
}

// Kraken Spot OHLC (Candle) Structures
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KrakenSpotOhlcData {
    pub symbol: String,
    pub open: String,
    pub high: String,
    pub low: String,
    pub close: String,
    pub volume: String,
    pub vwap: String,
    pub trades: u64,
    pub interval: u32,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KrakenSpotOhlcMessage {
    pub channel: String,
    #[serde(rename = "type")]
    pub msg_type: String,
    pub data: Vec<KrakenSpotOhlcData>,
}

// Kraken API Request Structures
#[derive(Debug, Serialize, Deserialize)]
struct KrakenSpotMessage {
    method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    req_id: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<KrakenSpotSubscribeParams>,
}

#[derive(Debug, Serialize, Deserialize)]
struct KrakenSpotSubscribeParams {
    channel: String,
    symbol: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    depth: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    interval: Option<u32>,
}

// Implementations
impl KrakenSpotBookMessage {
    pub fn is_snapshot(&self) -> bool {
        self.msg_type == "snapshot"
    }

    pub fn is_full_update(&self) -> bool {
        self.is_snapshot()
    }
}

pub struct KrakenSpot {}

impl KrakenSpot {
    pub fn ping() -> String {
        serde_json::to_string(&KrakenSpotMessage {
            method: "ping".to_string(),
            req_id: Some(1),
            params: None,
        })
        .expect("Invalid ping")
    }

    pub fn depth(coin: &str) -> String {
        let subscribe = KrakenSpotMessage {
            method: "subscribe".to_string(),
            req_id: Some(1),
            params: Some(KrakenSpotSubscribeParams {
                channel: "book".to_string(),
                symbol: vec![coin.to_string()],
                depth: Some(25),
                interval: None,
            }),
        };

        serde_json::to_string(&subscribe).expect("invalid depth subscription")
    }

    pub fn trades(coin: &str) -> String {
        let subscribe = KrakenSpotMessage {
            method: "subscribe".to_string(),
            req_id: Some(1),
            params: Some(KrakenSpotSubscribeParams {
                channel: "trade".to_string(),
                symbol: vec![coin.to_string()],
                depth: None,
                interval: None,
            }),
        };

        serde_json::to_string(&subscribe).expect("invalid trades subscription")
    }

    pub fn candle(coin: &str) -> String {
        let subscribe = KrakenSpotMessage {
            method: "subscribe".to_string(),
            req_id: Some(1),
            params: Some(KrakenSpotSubscribeParams {
                channel: "ohlc".to_string(),
                symbol: vec![coin.to_string()],
                depth: None,
                interval: Some(1), // 1 minute candles
            }),
        };

        serde_json::to_string(&subscribe).expect("invalid ohlc subscription")
    }
}

// Conversion implementations
impl From<KrakenSpotBookMessage> for LocalOrderBook {
    fn from(value: KrakenSpotBookMessage) -> Self {
        use ordered_float::OrderedFloat;
        use std::cmp::Reverse;

        let mut bids = std::collections::BTreeMap::new();
        let mut asks = std::collections::BTreeMap::new();
        let mut coin = compact_str::CompactString::new("");
        let mut timestamp: i64 = 0;

        if let Some(book_data) = value.data.first() {
            coin = book_data.symbol.as_str().into();

            // Process bids
            for level in &book_data.bids {
                bids.insert(
                    Reverse(OrderedFloat(level.price)),
                    level.qty.to_string().into(),
                );
            }

            // Process asks
            for level in &book_data.asks {
                asks.insert(OrderedFloat(level.price), level.qty.to_string().into());
            }

            // Extract timestamp if available
            if let Some(ts_str) = &book_data.timestamp {
                // Parse RFC3339 timestamp to i64 (microseconds since epoch)
                if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(ts_str) {
                    timestamp = dt.timestamp_millis();
                }
            }
        }

        Self {
            bids,
            asks,
            last_update: timestamp,
            last_update_ts: DateTime::from_timestamp_millis(timestamp).unwrap_or_default(),
            exchange: "kraken-spot".into(),
            coin,
        }
    }
}

impl From<KrakenSpotTradeMessage> for Vec<Trade> {
    fn from(value: KrakenSpotTradeMessage) -> Self {
        let mut trades = Vec::new();

        for trade in &value.data {
            // Parse RFC3339 timestamp to i64 (microseconds since epoch)
            let timestamp = if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&trade.timestamp) {
                dt.timestamp_millis()
            } else {
                0 // Default to 0 if parsing fails
            };

            let internal = Trade {
                time: timestamp,
                exchange: "kraken-spot".to_string(),
                side: trade.side == "sell", // true for sell, false for buy
                coin: trade.symbol.clone(),
                px: trade.price,
                sz: trade.qty,
            };

            trades.push(internal);
        }

        trades
    }
}

impl TryFrom<KrakenSpotBookMessage> for Vec<DepthUpdate> {
    type Error = anyhow::Error;

    fn try_from(value: KrakenSpotBookMessage) -> Result<Self, Self::Error> {
        let mut updates = Vec::new();

        // If it's a snapshot, we don't generate depth updates (these are handled separately)
        if value.is_snapshot() {
            return Err(anyhow!("Not a partial update"));
        }

        // For Kraken updates
        if let Some(book_data) = value.data.first() {
            let exchange_str = Venue::KrakenSpot.as_str();
            let coin_str = book_data.symbol.as_str();
            let timestamp = if let Some(ts_str) = &book_data.timestamp {
                // Parse RFC3339 timestamp to i64 (milliseconds since epoch)
                if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(ts_str) {
                    dt.timestamp_millis()
                } else {
                    0 // Default to 0 if parsing fails
                }
            } else {
                0 // Default to 0 if no timestamp
            };

            // Process bids
            for level in &book_data.bids {
                let update = DepthUpdate {
                    time: timestamp,
                    exchange: exchange_str.to_string(),
                    side: false, // bids
                    coin: coin_str.to_string(),
                    px: level.price,
                    sz: level.qty,
                };
                updates.push(update);
            }

            // Process asks
            for level in &book_data.asks {
                let update = DepthUpdate {
                    time: timestamp,
                    exchange: exchange_str.to_string(),
                    side: true, // asks
                    coin: coin_str.to_string(),
                    px: level.price,
                    sz: level.qty,
                };
                updates.push(update);
            }
        }

        Ok(updates)
    }
}
