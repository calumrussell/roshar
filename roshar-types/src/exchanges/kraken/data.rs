use anyhow::anyhow;
use chrono::DateTime;
use serde::{Deserialize, Serialize};

use crate::{DepthUpdate, LocalOrderBook, LocalOrderBookError, Trade, Venue};

// Kraken Derivatives Book Structures
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KrakenBookLevel {
    pub price: f64,
    pub qty: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KrakenBookSnapshotMessage {
    pub feed: String,
    pub product_id: String,
    pub timestamp: i64,
    pub seq: i64,
    #[serde(rename = "tickSize")]
    pub tick_size: Option<String>,
    pub bids: Vec<KrakenBookLevel>,
    pub asks: Vec<KrakenBookLevel>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KrakenBookDeltaMessage {
    pub feed: String,
    pub product_id: String,
    pub timestamp: i64,
    pub seq: i64,
    pub side: String, // "buy" or "sell"
    pub price: f64,
    pub qty: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct KrakenSubscribeMessage {
    pub event: String,
    pub feed: String,
    pub product_ids: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct KrakenPingRequest {
    event: String,
}

// Helper methods for BookSnapshot
impl KrakenBookSnapshotMessage {
    pub fn is_snapshot(&self) -> bool {
        true
    }
}

// Conversion implementations
impl From<KrakenBookSnapshotMessage> for LocalOrderBook {
    fn from(value: KrakenBookSnapshotMessage) -> Self {
        use ordered_float::OrderedFloat;
        use std::cmp::Reverse;

        let mut bids = std::collections::BTreeMap::new();
        let mut asks = std::collections::BTreeMap::new();

        // Process bids
        for level in &value.bids {
            bids.insert(
                Reverse(OrderedFloat(level.price)),
                level.qty.to_string().into(),
            );
        }

        // Process asks
        for level in &value.asks {
            asks.insert(OrderedFloat(level.price), level.qty.to_string().into());
        }

        Self {
            bids,
            asks,
            last_update: value.timestamp,
            last_update_ts: DateTime::from_timestamp_millis(value.timestamp).unwrap_or_default(),
            exchange: Venue::Kraken.to_string().into(),
            coin: value.product_id.as_str().into(),
        }
    }
}

impl TryFrom<KrakenBookSnapshotMessage> for Vec<DepthUpdate> {
    type Error = anyhow::Error;

    fn try_from(_value: KrakenBookSnapshotMessage) -> std::result::Result<Self, Self::Error> {
        Err(anyhow!("Doesn't convert to DepthUpdate"))
    }
}

impl From<KrakenBookDeltaMessage> for DepthUpdate {
    fn from(value: KrakenBookDeltaMessage) -> Self {
        let side = match value.side.as_str() {
            "sell" => true, // ask side
            "buy" => false, // bid side
            _ => false,     // default to bid side if unknown
        };

        Self {
            time: value.timestamp,
            exchange: Venue::Kraken.as_str().to_string(),
            side,
            coin: value.product_id,
            px: value.price,
            sz: value.qty,
        }
    }
}

impl TryFrom<KrakenBookDeltaMessage> for Vec<DepthUpdate> {
    type Error = anyhow::Error;

    fn try_from(value: KrakenBookDeltaMessage) -> std::result::Result<Self, Self::Error> {
        let value: DepthUpdate = value.into();
        Ok(vec![value])
    }
}

impl From<KrakenBookDeltaMessage> for LocalOrderBook {
    fn from(value: KrakenBookDeltaMessage) -> Self {
        // Create a minimal LocalOrderBook just to satisfy the type constraints
        // The actual DepthUpdate handling is done via the try_into function
        Self {
            bids: std::collections::BTreeMap::new(),
            asks: std::collections::BTreeMap::new(),
            last_update: value.timestamp,
            last_update_ts: DateTime::from_timestamp_millis(value.timestamp).unwrap_or_default(),
            exchange: Venue::Kraken.to_string().into(),
            coin: value.product_id.as_str().into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KrakenSingleTrade {
    pub feed: String,
    pub product_id: String,
    pub uid: String,
    pub side: String, // "buy" or "sell"
    #[serde(rename = "type")]
    pub type_field: String, // "fill", "liquidation", "termination", or "block"
    pub seq: i64,
    pub time: i64,
    pub qty: f64,
    pub price: f64,
}

impl From<KrakenSingleTrade> for Trade {
    fn from(value: KrakenSingleTrade) -> Self {
        Self {
            time: value.time,
            exchange: Venue::Kraken.to_string(),
            side: value.side == "sell", // true for sell, false for buy
            coin: value.product_id,
            px: value.price,
            sz: value.qty,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KrakenTradeSnapshotMessage {
    pub feed: String,
    pub product_id: String,
    pub trades: Vec<KrakenSingleTrade>,
}

impl From<KrakenTradeSnapshotMessage> for Vec<Trade> {
    fn from(value: KrakenTradeSnapshotMessage) -> Self {
        value.trades.into_iter().map(Trade::from).collect()
    }
}

// Use SingleTrade as TradeDeltaMessage to avoid duplication
pub type KrakenTradeDeltaMessage = KrakenSingleTrade;

// CompressedData conversions

// New From implementations for daily partitioned tables
impl From<KrakenTradeSnapshotMessage> for Vec<crate::TradeData> {
    fn from(value: KrakenTradeSnapshotMessage) -> Self {
        value
            .trades
            .into_iter()
            .map(|trade| crate::TradeData {
                px: trade.price.to_string(),
                qty: trade.qty.to_string(),
                time: trade.time as u64,
                time_ts: DateTime::from_timestamp_millis(trade.time).unwrap_or_default(),
                ticker: trade.product_id,
                meta: format!(
                    "{{\"uid\": \"{}\", \"seq\": {}, \"type\": \"{}\"}}",
                    trade.uid, trade.seq, trade.type_field
                ),
                side: trade.side == "sell", // true for sell, false for buy
                venue: Venue::Kraken,
            })
            .collect()
    }
}

impl From<KrakenTradeDeltaMessage> for Vec<crate::TradeData> {
    fn from(value: KrakenTradeDeltaMessage) -> Self {
        vec![crate::TradeData {
            px: value.price.to_string(),
            qty: value.qty.to_string(),
            time: value.time as u64,
            time_ts: DateTime::from_timestamp_millis(value.time).unwrap_or_default(),
            ticker: value.product_id,
            meta: format!(
                "{{\"uid\": \"{}\", \"seq\": {}, \"type\": \"{}\"}}",
                value.uid, value.seq, value.type_field
            ),
            side: value.side == "sell", // true for sell, false for buy
            venue: Venue::Kraken,
        }]
    }
}

impl From<KrakenBookDeltaMessage> for Vec<crate::DepthUpdateData> {
    fn from(value: KrakenBookDeltaMessage) -> Self {
        vec![crate::DepthUpdateData {
            px: value.price.to_string(),
            qty: value.qty.to_string(),
            time: value.timestamp as u64,
            time_ts: DateTime::from_timestamp_millis(value.timestamp).unwrap_or_default(),
            ticker: value.product_id,
            meta: format!("{{\"seq\": {}, \"feed\": \"{}\"}}", value.seq, value.feed),
            side: value.side == "sell", // true for sell, false for buy
            venue: Venue::Kraken,
        }]
    }
}

impl From<KrakenBookSnapshotMessage> for crate::DepthSnapshotData {
    fn from(value: KrakenBookSnapshotMessage) -> Self {
        let bid_prices: Vec<String> = value.bids.iter().map(|b| b.price.to_string()).collect();
        let bid_sizes: Vec<String> = value.bids.iter().map(|b| b.qty.to_string()).collect();
        let ask_prices: Vec<String> = value.asks.iter().map(|a| a.price.to_string()).collect();
        let ask_sizes: Vec<String> = value.asks.iter().map(|a| a.qty.to_string()).collect();

        crate::DepthSnapshotData {
            bid_prices,
            bid_sizes,
            ask_prices,
            ask_sizes,
            time: value.timestamp as u64,
            time_ts: DateTime::from_timestamp_millis(value.timestamp).unwrap_or_default(),
            ticker: value.product_id,
            venue: Venue::Kraken,
        }
    }
}

pub struct KrakenOrderBook {
    pub symbol: String,
    pub book: Option<LocalOrderBook>,
    pub counter: i64,
}

impl KrakenOrderBook {
    pub fn new(symbol: String) -> Self {
        Self {
            symbol,
            book: None,
            counter: 0,
        }
    }

    pub fn new_update(&mut self, msg: KrakenBookDeltaMessage) -> Result<(), LocalOrderBookError> {
        let coin = msg.product_id.clone();

        // Validate symbol
        if coin != self.symbol {
            return Err(LocalOrderBookError::WrongSymbol(self.symbol.clone(), coin));
        }

        if self.book.is_some() {
            let msg_seq = msg.seq;
            let expected_seq = self.counter + 1;
            if msg_seq == expected_seq {
                if let Some(ref mut book) = self.book
                    && let Ok(updates) = msg.try_into()
                {
                    book.apply_updates(&updates, 50);
                    self.counter = msg_seq;
                }
            } else {
                return Err(LocalOrderBookError::OutOfOrderUpdate(
                    Venue::Kraken.to_string(),
                    coin,
                    expected_seq,
                    msg_seq,
                ));
            }
        } else {
            return Err(LocalOrderBookError::BookUpdateBeforeSnapshot(
                Venue::Kraken.to_string(),
                coin,
            ));
        }
        Ok(())
    }

    pub fn new_snapshot(&mut self, msg: KrakenBookSnapshotMessage) {
        let coin = msg.product_id.clone();

        // Validate symbol
        if coin != self.symbol {
            // Silently ignore snapshots for wrong symbols (or could return error)
            return;
        }

        self.counter = msg.seq;
        self.book = Some(msg.into());
    }
}

pub struct WssApi {}

impl WssApi {
    pub fn ping() -> String {
        let ping = KrakenPingRequest {
            event: "ping".to_string(),
        };

        serde_json::to_string(&ping).expect("invalid ping message")
    }

    pub fn depth(product_id: &str) -> String {
        let subscribe = KrakenSubscribeMessage {
            event: "subscribe".to_string(),
            feed: "book".to_string(),
            product_ids: vec![product_id.to_string()],
        };

        serde_json::to_string(&subscribe).expect("invalid depth subscription")
    }

    pub fn depth_unsub(product_id: &str) -> String {
        let subscribe = KrakenSubscribeMessage {
            event: "unsubscribe".to_string(),
            feed: "book".to_string(),
            product_ids: vec![product_id.to_string()],
        };

        serde_json::to_string(&subscribe).expect("invalid depth subscription")
    }

    pub fn trades(product_id: &str) -> String {
        let subscribe = KrakenSubscribeMessage {
            event: "subscribe".to_string(),
            feed: "trade".to_string(),
            product_ids: vec![product_id.to_string()],
        };

        serde_json::to_string(&subscribe).expect("invalid trades subscription")
    }

    pub fn trades_unsub(product_id: &str) -> String {
        let subscribe = KrakenSubscribeMessage {
            event: "unsubscribe".to_string(),
            feed: "trade".to_string(),
            product_ids: vec![product_id.to_string()],
        };

        serde_json::to_string(&subscribe).expect("invalid trades subscription")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_snapshot_message(seq: i64, product_id: &str) -> KrakenBookSnapshotMessage {
        KrakenBookSnapshotMessage {
            feed: "book_snapshot".to_string(),
            product_id: product_id.to_string(),
            timestamp: 1234567890,
            seq,
            tick_size: Some("0.1".to_string()),
            bids: vec![
                KrakenBookLevel {
                    price: 50000.0,
                    qty: 1.0,
                },
                KrakenBookLevel {
                    price: 49999.0,
                    qty: 2.0,
                },
            ],
            asks: vec![
                KrakenBookLevel {
                    price: 50001.0,
                    qty: 1.5,
                },
                KrakenBookLevel {
                    price: 50002.0,
                    qty: 2.5,
                },
            ],
        }
    }

    fn create_test_delta_message(
        seq: i64,
        product_id: &str,
        side: &str,
        price: f64,
        qty: f64,
    ) -> KrakenBookDeltaMessage {
        KrakenBookDeltaMessage {
            feed: "book".to_string(),
            product_id: product_id.to_string(),
            timestamp: 1234567890 + seq,
            seq,
            side: side.to_string(),
            price,
            qty,
        }
    }

    #[tokio::test]
    async fn test_kraken_order_book_snapshot_initialization() {
        // Test that a snapshot message correctly initializes the order book state
        // with proper sequence tracking and empty message cache
        let mut order_book = KrakenOrderBook::new("PI_XBTUSD".to_string());
        let snapshot = create_test_snapshot_message(1000, "PI_XBTUSD");

        order_book.new_snapshot(snapshot);

        assert!(order_book.book.is_some());
        assert_eq!(order_book.counter, 1000);

        let book = order_book.book.as_ref().unwrap();
        assert_eq!(book.test_bid_prices().len(), 2);
        assert_eq!(book.test_ask_prices().len(), 2);
        assert_eq!(book.test_bid_prices()[0], "50000");
        assert_eq!(book.test_ask_prices()[0], "50001");
    }

    #[tokio::test]
    async fn test_kraken_order_book_in_sequence_updates() {
        // Test that delta messages arriving in correct sequence (seq + 1) are processed
        // immediately without caching and advance the sequence counter
        let mut order_book = KrakenOrderBook::new("PI_XBTUSD".to_string());
        let snapshot = create_test_snapshot_message(1000, "PI_XBTUSD");
        order_book.new_snapshot(snapshot);

        let delta1 = create_test_delta_message(1001, "PI_XBTUSD", "buy", 50003.0, 3.0);
        let delta2 = create_test_delta_message(1002, "PI_XBTUSD", "sell", 50004.0, 4.0);

        assert!(order_book.new_update(delta1).is_ok());
        assert_eq!(order_book.counter, 1001);

        assert!(order_book.new_update(delta2).is_ok());
        assert_eq!(order_book.counter, 1002);
    }

    #[tokio::test]
    async fn test_kraken_order_book_update_before_snapshot_error() {
        // Test that LocalOrderBookError::BookUpdateBeforeSnapshot is returned when
        // attempting to update a symbol that doesn't have an initial snapshot
        let mut order_book = KrakenOrderBook::new("PI_XBTUSD".to_string());
        let delta = create_test_delta_message(1001, "PI_XBTUSD", "buy", 50003.0, 3.0);

        let result = order_book.new_update(delta);
        assert!(result.is_err());
        if let Err(LocalOrderBookError::BookUpdateBeforeSnapshot(exchange, coin)) = result {
            assert_eq!(exchange, "kraken");
            assert_eq!(coin, "PI_XBTUSD");
        } else {
            panic!("Expected BookUpdateBeforeSnapshot error");
        }
    }

    #[tokio::test]
    async fn test_kraken_order_book_wrong_symbol() {
        // Test that messages for wrong symbol return WrongSymbol error
        let mut order_book = KrakenOrderBook::new("PI_XBTUSD".to_string());
        let delta_eth = create_test_delta_message(1001, "PI_ETHUSD", "buy", 3000.0, 10.0);

        let result = order_book.new_update(delta_eth);
        assert!(result.is_err());
        if let Err(LocalOrderBookError::WrongSymbol(expected, received)) = result {
            assert_eq!(expected, "PI_XBTUSD");
            assert_eq!(received, "PI_ETHUSD");
        } else {
            panic!("Expected WrongSymbol error");
        }
    }
}
