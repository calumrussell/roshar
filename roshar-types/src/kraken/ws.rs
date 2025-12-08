use chrono::DateTime;
use serde::{Deserialize, Serialize};

use crate::{DepthUpdate, LocalOrderBook, LocalOrderBookError, OrderBookState, Trade, Venue};

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

impl KrakenBookSnapshotMessage {
    pub fn is_snapshot(&self) -> bool {
        true
    }

    pub fn to_order_book_state(&self) -> OrderBookState {
        let mut book = OrderBookState::new(50);

        for level in &self.bids {
            if let Err(e) = book.set_bid(level.price, &level.qty.to_string()) {
                log::error!("Kraken: failed to set bid for {} at price {}: {}", self.product_id, level.price, e);
            }
        }

        for level in &self.asks {
            if let Err(e) = book.set_ask(level.price, &level.qty.to_string()) {
                log::error!("Kraken: failed to set ask for {} at price {}: {}", self.product_id, level.price, e);
            }
        }

        // Trim after all updates are processed
        book.trim();

        book
    }

    pub fn to_depth_snapshot_data(&self) -> crate::DepthSnapshotData {
        let bid_prices: Vec<String> = self.bids.iter().map(|b| b.price.to_string()).collect();
        let bid_sizes: Vec<String> = self.bids.iter().map(|b| b.qty.to_string()).collect();
        let ask_prices: Vec<String> = self.asks.iter().map(|a| a.price.to_string()).collect();
        let ask_sizes: Vec<String> = self.asks.iter().map(|a| a.qty.to_string()).collect();

        crate::DepthSnapshotData {
            bid_prices,
            bid_sizes,
            ask_prices,
            ask_sizes,
            time: self.timestamp as u64,
            time_ts: DateTime::from_timestamp_millis(self.timestamp).unwrap_or_default(),
            ticker: self.product_id.clone(),
            venue: Venue::Kraken,
        }
    }
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

impl KrakenBookDeltaMessage {
    pub fn to_depth_update(&self) -> DepthUpdate {
        let side = match self.side.as_str() {
            "sell" => true, // ask side
            "buy" => false, // bid side
            _ => false,     // default to bid side if unknown
        };

        DepthUpdate {
            time: self.timestamp,
            exchange: Venue::Kraken.as_str().to_string(),
            side,
            coin: self.product_id.clone(),
            px: self.price,
            sz: self.qty,
        }
    }

    pub fn to_depth_updates(&self) -> Vec<DepthUpdate> {
        vec![self.to_depth_update()]
    }

    pub fn to_depth_update_data(&self) -> Vec<crate::DepthUpdateData> {
        vec![crate::DepthUpdateData {
            px: self.price.to_string(),
            qty: self.qty.to_string(),
            time: self.timestamp as u64,
            time_ts: DateTime::from_timestamp_millis(self.timestamp).unwrap_or_default(),
            ticker: self.product_id.clone(),
            meta: format!("{{\"seq\": {}, \"feed\": \"{}\"}}", self.seq, self.feed),
            side: self.side == "sell", // true for sell, false for buy
            venue: Venue::Kraken,
        }]
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct KrakenSubscribeMessage {
    pub event: String,
    pub feed: String,
    pub product_ids: Vec<String>,
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

impl KrakenSingleTrade {
    pub fn to_trade(&self) -> Trade {
        Trade {
            time: self.time,
            exchange: Venue::Kraken.to_string(),
            side: self.side == "sell", // true for sell, false for buy
            coin: self.product_id.clone(),
            px: self.price,
            sz: self.qty,
        }
    }

    pub fn to_trade_data(&self) -> crate::TradeData {
        crate::TradeData {
            px: self.price.to_string(),
            qty: self.qty.to_string(),
            time: self.time as u64,
            time_ts: DateTime::from_timestamp_millis(self.time).unwrap_or_default(),
            ticker: self.product_id.clone(),
            meta: format!(
                "{{\"uid\": \"{}\", \"seq\": {}, \"type\": \"{}\"}}",
                self.uid, self.seq, self.type_field
            ),
            side: self.side == "sell", // true for sell, false for buy
            venue: Venue::Kraken,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KrakenTradeSnapshotMessage {
    pub feed: String,
    pub product_id: String,
    pub trades: Vec<KrakenSingleTrade>,
}

impl KrakenTradeSnapshotMessage {
    pub fn to_trades(&self) -> Vec<Trade> {
        self.trades.iter().map(|t| t.to_trade()).collect()
    }

    pub fn to_trade_data(&self) -> Vec<crate::TradeData> {
        self.trades.iter().map(|t| t.to_trade_data()).collect()
    }
}

// Use SingleTrade as TradeDeltaMessage to avoid duplication
pub type KrakenTradeDeltaMessage = KrakenSingleTrade;

// WebSocket Message Type
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct KrakenWssMessage {
    pub event: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feed: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product_ids: Option<Vec<String>>,
}

impl KrakenWssMessage {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("failed to serialize KrakenWssMessage")
    }

    pub fn ping() -> Self {
        Self {
            event: "ping".to_string(),
            feed: None,
            product_ids: None,
        }
    }

    pub fn depth(product_id: &str) -> Self {
        Self {
            event: "subscribe".to_string(),
            feed: Some("book".to_string()),
            product_ids: Some(vec![product_id.to_string()]),
        }
    }

    pub fn depth_unsub(product_id: &str) -> Self {
        Self {
            event: "unsubscribe".to_string(),
            feed: Some("book".to_string()),
            product_ids: Some(vec![product_id.to_string()]),
        }
    }

    pub fn trades(product_id: &str) -> Self {
        Self {
            event: "subscribe".to_string(),
            feed: Some("trade".to_string()),
            product_ids: Some(vec![product_id.to_string()]),
        }
    }

    pub fn trades_unsub(product_id: &str) -> Self {
        Self {
            event: "unsubscribe".to_string(),
            feed: Some("trade".to_string()),
            product_ids: Some(vec![product_id.to_string()]),
        }
    }
}

// Order book management
pub struct KrakenOrderBook {
    pub symbol: String,
    pub book: Option<OrderBookState>,
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

    /// Get a read-only view of the order book for calculations
    pub fn as_view(&self) -> Option<LocalOrderBook<'_>> {
        self.book.as_ref().map(|b| b.as_view())
    }

    pub fn new_update(&mut self, msg: &KrakenBookDeltaMessage) -> Result<(), LocalOrderBookError> {
        let coin = &msg.product_id;

        // Validate symbol
        if coin != &self.symbol {
            return Err(LocalOrderBookError::WrongSymbol(
                self.symbol.clone(),
                coin.clone(),
            ));
        }

        if self.book.is_some() {
            let msg_seq = msg.seq;
            let expected_seq = self.counter + 1;
            if msg_seq == expected_seq {
                if let Some(ref mut book) = self.book {
                    // Apply update directly
                    let size_str = msg.qty.to_string();
                    if msg.side == "sell" {
                        if let Err(e) = book.set_ask(msg.price, &size_str) {
                            log::error!("Kraken: failed to set ask update for {} at price {}: {}", coin, msg.price, e);
                        }
                    } else if let Err(e) = book.set_bid(msg.price, &size_str) {
                        log::error!("Kraken: failed to set bid update for {} at price {}: {}", coin, msg.price, e);
                    }
                    // Trim after update
                    book.trim();
                    self.counter = msg_seq;
                }
            } else {
                return Err(LocalOrderBookError::OutOfOrderUpdate(
                    Venue::Kraken.to_string(),
                    coin.clone(),
                    expected_seq,
                    msg_seq,
                ));
            }
        } else {
            return Err(LocalOrderBookError::BookUpdateBeforeSnapshot(
                Venue::Kraken.to_string(),
                coin.clone(),
            ));
        }
        Ok(())
    }

    pub fn new_snapshot(&mut self, msg: &KrakenBookSnapshotMessage) {
        let coin = &msg.product_id;

        // Validate symbol
        if coin != &self.symbol {
            // Silently ignore snapshots for wrong symbols
            return;
        }

        self.counter = msg.seq;
        self.book = Some(msg.to_order_book_state());
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

    #[test]
    fn test_kraken_order_book_snapshot_initialization() {
        let mut order_book = KrakenOrderBook::new("PI_XBTUSD".to_string());
        let snapshot = create_test_snapshot_message(1000, "PI_XBTUSD");

        order_book.new_snapshot(&snapshot);

        assert!(order_book.book.is_some());
        assert_eq!(order_book.counter, 1000);

        let view = order_book.as_view().unwrap();
        assert_eq!(view.bid_prices().len(), 2);
        assert_eq!(view.ask_prices().len(), 2);
        assert_eq!(view.bid_prices()[0], "50000");
        assert_eq!(view.ask_prices()[0], "50001");
    }

    #[test]
    fn test_kraken_order_book_in_sequence_updates() {
        let mut order_book = KrakenOrderBook::new("PI_XBTUSD".to_string());
        let snapshot = create_test_snapshot_message(1000, "PI_XBTUSD");
        order_book.new_snapshot(&snapshot);

        let delta1 = create_test_delta_message(1001, "PI_XBTUSD", "buy", 50003.0, 3.0);
        let delta2 = create_test_delta_message(1002, "PI_XBTUSD", "sell", 50004.0, 4.0);

        assert!(order_book.new_update(&delta1).is_ok());
        assert_eq!(order_book.counter, 1001);

        assert!(order_book.new_update(&delta2).is_ok());
        assert_eq!(order_book.counter, 1002);
    }

    #[test]
    fn test_kraken_order_book_update_before_snapshot_error() {
        let mut order_book = KrakenOrderBook::new("PI_XBTUSD".to_string());
        let delta = create_test_delta_message(1001, "PI_XBTUSD", "buy", 50003.0, 3.0);

        let result = order_book.new_update(&delta);
        assert!(result.is_err());
        if let Err(LocalOrderBookError::BookUpdateBeforeSnapshot(exchange, coin)) = result {
            assert_eq!(exchange, "kraken");
            assert_eq!(coin, "PI_XBTUSD");
        } else {
            panic!("Expected BookUpdateBeforeSnapshot error");
        }
    }

    #[test]
    fn test_kraken_order_book_wrong_symbol() {
        let mut order_book = KrakenOrderBook::new("PI_XBTUSD".to_string());
        let delta_eth = create_test_delta_message(1001, "PI_ETHUSD", "buy", 3000.0, 10.0);

        let result = order_book.new_update(&delta_eth);
        assert!(result.is_err());
        if let Err(LocalOrderBookError::WrongSymbol(expected, received)) = result {
            assert_eq!(expected, "PI_XBTUSD");
            assert_eq!(received, "PI_ETHUSD");
        } else {
            panic!("Expected WrongSymbol error");
        }
    }

    #[test]
    fn test_wss_message_ping() {
        let msg = KrakenWssMessage::ping();
        let json = msg.to_json();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["event"], "ping");
    }

    #[test]
    fn test_wss_message_depth() {
        let msg = KrakenWssMessage::depth("PI_XBTUSD");
        let json = msg.to_json();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["event"], "subscribe");
        assert_eq!(parsed["feed"], "book");
        assert_eq!(parsed["product_ids"][0], "PI_XBTUSD");
    }

    #[test]
    fn test_wss_message_trades() {
        let msg = KrakenWssMessage::trades("PI_XBTUSD");
        let json = msg.to_json();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["event"], "subscribe");
        assert_eq!(parsed["feed"], "trade");
        assert_eq!(parsed["product_ids"][0], "PI_XBTUSD");
    }
}
