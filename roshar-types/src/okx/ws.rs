use chrono::DateTime;
use serde::{Deserialize, Serialize};

use crate::{LocalOrderBookError, OrderBookState, Trade, Venue};

// --- Subscribe/Unsubscribe message types ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OkxWssArg {
    pub channel: String,
    #[serde(rename = "instId")]
    pub inst_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OkxWssMessage {
    pub op: String,
    pub args: Vec<OkxWssArg>,
}

impl OkxWssMessage {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("failed to serialize OkxWssMessage")
    }

    /// OKX uses text-frame pings: send the literal string "ping"
    pub fn ping() -> String {
        "ping".to_string()
    }

    pub fn depth(coin: &str) -> Self {
        Self {
            op: "subscribe".to_string(),
            args: vec![OkxWssArg {
                channel: "books".to_string(),
                inst_id: coin.to_string(),
            }],
        }
    }

    pub fn depth_unsub(coin: &str) -> Self {
        Self {
            op: "unsubscribe".to_string(),
            args: vec![OkxWssArg {
                channel: "books".to_string(),
                inst_id: coin.to_string(),
            }],
        }
    }

    pub fn trades(coin: &str) -> Self {
        Self {
            op: "subscribe".to_string(),
            args: vec![OkxWssArg {
                channel: "trades".to_string(),
                inst_id: coin.to_string(),
            }],
        }
    }

    pub fn trades_unsub(coin: &str) -> Self {
        Self {
            op: "unsubscribe".to_string(),
            args: vec![OkxWssArg {
                channel: "trades".to_string(),
                inst_id: coin.to_string(),
            }],
        }
    }
}

// --- Depth (books) push data ---

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OkxDepthMessage {
    pub arg: OkxWssArg,
    /// "snapshot" for the initial full book, "update" for incremental deltas
    pub action: String,
    pub data: Vec<OkxDepthBookData>,
}

impl OkxDepthMessage {
    /// Extract the instId from the arg field
    pub fn inst_id(&self) -> &str {
        &self.arg.inst_id
    }

    /// Returns true if this message is a full snapshot (not a delta update)
    pub fn is_snapshot(&self) -> bool {
        self.action == "snapshot"
    }

    /// Build a fresh OrderBookState from a snapshot message.
    pub fn to_order_book_state(&self) -> OrderBookState {
        let mut book = OrderBookState::new(400);

        if let Some(data) = self.data.first() {
            for level in &data.bids {
                // [price, size, deprecated, numOrders]
                if let Ok(price) = level[0].parse::<f64>() {
                    if let Err(e) = book.set_bid(price, &level[1]) {
                        log::error!(
                            "OKX: failed to set bid for {} at price {}: {}",
                            self.arg.inst_id,
                            price,
                            e
                        );
                    }
                }
            }

            for level in &data.asks {
                if let Ok(price) = level[0].parse::<f64>() {
                    if let Err(e) = book.set_ask(price, &level[1]) {
                        log::error!(
                            "OKX: failed to set ask for {} at price {}: {}",
                            self.arg.inst_id,
                            price,
                            e
                        );
                    }
                }
            }

            book.trim();
        }

        book
    }

    /// Apply a delta update to an existing OrderBookState in place.
    /// Levels with size "0" are removed; all others are set/updated.
    pub fn apply_delta(&self, book: &mut OrderBookState) {
        if let Some(data) = self.data.first() {
            for level in &data.bids {
                if let Ok(price) = level[0].parse::<f64>() {
                    if level[1] == "0" {
                        book.remove_bid(price);
                    } else if let Err(e) = book.set_bid(price, &level[1]) {
                        log::error!(
                            "OKX: failed to update bid for {} at price {}: {}",
                            self.arg.inst_id,
                            price,
                            e
                        );
                    }
                }
            }

            for level in &data.asks {
                if let Ok(price) = level[0].parse::<f64>() {
                    if level[1] == "0" {
                        book.remove_ask(price);
                    } else if let Err(e) = book.set_ask(price, &level[1]) {
                        log::error!(
                            "OKX: failed to update ask for {} at price {}: {}",
                            self.arg.inst_id,
                            price,
                            e
                        );
                    }
                }
            }
        }
    }

    pub fn to_depth_update_data(&self) -> Vec<crate::DepthUpdateData> {
        let mut res = Vec::new();

        if let Some(data) = self.data.first() {
            let ts: u64 = data.ts.parse().unwrap_or(0);

            for bid in &data.bids {
                res.push(crate::DepthUpdateData {
                    px: bid[0].clone(),
                    qty: bid[1].clone(),
                    time: ts,
                    time_ts: DateTime::from_timestamp_millis(ts as i64).unwrap_or_default(),
                    ticker: self.arg.inst_id.clone(),
                    meta: String::new(),
                    side: false, // bid side
                    venue: Venue::Okx,
                });
            }

            for ask in &data.asks {
                res.push(crate::DepthUpdateData {
                    px: ask[0].clone(),
                    qty: ask[1].clone(),
                    time: ts,
                    time_ts: DateTime::from_timestamp_millis(ts as i64).unwrap_or_default(),
                    ticker: self.arg.inst_id.clone(),
                    meta: String::new(),
                    side: true, // ask side
                    venue: Venue::Okx,
                });
            }
        }

        res
    }

    pub fn to_depth_snapshot_data(&self) -> Option<crate::DepthSnapshotData> {
        let data = self.data.first()?;
        let ts: u64 = data.ts.parse().unwrap_or(0);

        Some(crate::DepthSnapshotData {
            bid_prices: data.bids.iter().map(|l| l[0].clone()).collect(),
            bid_sizes: data.bids.iter().map(|l| l[1].clone()).collect(),
            ask_prices: data.asks.iter().map(|l| l[0].clone()).collect(),
            ask_sizes: data.asks.iter().map(|l| l[1].clone()).collect(),
            time: ts,
            time_ts: DateTime::from_timestamp_millis(ts as i64).unwrap_or_default(),
            ticker: self.arg.inst_id.clone(),
            venue: Venue::Okx,
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OkxDepthBookData {
    pub asks: Vec<[String; 4]>,
    pub bids: Vec<[String; 4]>,
    pub ts: String,
}

// --- Trades push data ---

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OkxTradesMessage {
    pub arg: OkxWssArg,
    pub data: Vec<OkxTradeData>,
}

impl OkxTradesMessage {
    pub fn to_trades(&self) -> Vec<Trade> {
        self.data
            .iter()
            .map(|trade| {
                let ts: i64 = trade.ts.parse().unwrap_or(0);
                Trade {
                    time: ts,
                    exchange: Venue::Okx.to_string(),
                    side: trade.side == "sell",
                    coin: trade.inst_id.clone(),
                    px: trade.px.parse::<f64>().unwrap_or(0.0),
                    sz: trade.sz.parse::<f64>().unwrap_or(0.0),
                }
            })
            .collect()
    }

    pub fn to_trade_data(&self) -> Vec<crate::TradeData> {
        self.data
            .iter()
            .map(|trade| {
                let ts: u64 = trade.ts.parse().unwrap_or(0);
                crate::TradeData {
                    px: trade.px.clone(),
                    qty: trade.sz.clone(),
                    time: ts,
                    time_ts: DateTime::from_timestamp_millis(ts as i64).unwrap_or_default(),
                    ticker: trade.inst_id.clone(),
                    meta: format!("{{\"tradeId\": \"{}\"}}", trade.trade_id),
                    side: trade.side == "sell",
                    venue: Venue::Okx,
                }
            })
            .collect()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OkxTradeData {
    #[serde(rename = "instId")]
    pub inst_id: String,
    #[serde(rename = "tradeId")]
    pub trade_id: String,
    pub px: String,
    pub sz: String,
    pub side: String,
    pub ts: String,
}

// --- Order book management ---

pub struct OkxOrderBook {
    pub symbol: String,
    pub book: Option<OrderBookState>,
}

impl OkxOrderBook {
    pub fn new(symbol: String) -> Self {
        Self { symbol, book: None }
    }

    pub fn new_snapshot(&mut self, msg: &OkxDepthMessage) {
        self.book = Some(msg.to_order_book_state());
    }

    pub fn new_update(&mut self, msg: &OkxDepthMessage) -> Result<(), LocalOrderBookError> {
        let coin = msg.inst_id().to_string();

        if self.symbol != coin {
            return Err(LocalOrderBookError::WrongSymbol(
                self.symbol.clone(),
                coin,
            ));
        }

        if let Some(ref mut book) = self.book {
            msg.apply_delta(book);
        } else {
            return Err(LocalOrderBookError::BookUpdateBeforeSnapshot(
                Venue::Okx.to_string(),
                coin,
            ));
        }

        // Validate BBO: if bid > ask the book has become corrupted, reset it
        let validation_result = if let Some(ref book) = self.book {
            let (bid, ask) = book.get_bbo();
            match (bid, ask) {
                (Some(b), Some(a)) if b > a => Err(LocalOrderBookError::BidAboveAsk(
                    b.to_string(),
                    a.to_string(),
                    Venue::Okx.to_string(),
                    coin,
                )),
                _ => Ok(()),
            }
        } else {
            Ok(())
        };

        if validation_result.is_err() {
            self.book = None;
        }

        validation_result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_okx_wss_message_ping() {
        assert_eq!(OkxWssMessage::ping(), "ping");
    }

    #[test]
    fn test_okx_wss_message_depth_subscribe() {
        let msg = OkxWssMessage::depth("BTC-USDT-SWAP");
        let json = msg.to_json();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["op"], "subscribe");
        assert_eq!(parsed["args"][0]["channel"], "books");
        assert_eq!(parsed["args"][0]["instId"], "BTC-USDT-SWAP");
    }

    #[test]
    fn test_okx_wss_message_depth_unsubscribe() {
        let msg = OkxWssMessage::depth_unsub("BTC-USDT-SWAP");
        let json = msg.to_json();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["op"], "unsubscribe");
        assert_eq!(parsed["args"][0]["channel"], "books");
        assert_eq!(parsed["args"][0]["instId"], "BTC-USDT-SWAP");
    }

    #[test]
    fn test_okx_wss_message_trades_subscribe() {
        let msg = OkxWssMessage::trades("BTC-USDT-SWAP");
        let json = msg.to_json();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["op"], "subscribe");
        assert_eq!(parsed["args"][0]["channel"], "trades");
        assert_eq!(parsed["args"][0]["instId"], "BTC-USDT-SWAP");
    }

    #[test]
    fn test_okx_depth_message_parsing() {
        let json = r#"{
            "arg": {"channel": "books", "instId": "BTC-USDT-SWAP"},
            "action": "snapshot",
            "data": [{
                "asks": [["41006.8", "0.60038921", "0", "1"], ["41007.0", "0.5", "0", "2"]],
                "bids": [["41006.3", "0.20572000", "0", "1"], ["41006.0", "1.0", "0", "3"]],
                "ts": "1597026383085"
            }]
        }"#;

        let msg: OkxDepthMessage = serde_json::from_str(json).unwrap();
        assert_eq!(msg.inst_id(), "BTC-USDT-SWAP");
        assert!(msg.is_snapshot());
        assert_eq!(msg.data[0].asks.len(), 2);
        assert_eq!(msg.data[0].bids.len(), 2);

        let book = msg.to_order_book_state();
        let (bid, ask) = book.get_bbo();
        assert!(bid.is_some());
        assert!(ask.is_some());
    }

    #[test]
    fn test_okx_depth_message_delta_apply() {
        // Build initial snapshot
        let snapshot_json = r#"{
            "arg": {"channel": "books", "instId": "BTC-USDT-SWAP"},
            "action": "snapshot",
            "data": [{
                "asks": [["41006.8", "0.6", "0", "1"], ["41007.0", "0.5", "0", "2"]],
                "bids": [["41006.3", "0.2", "0", "1"], ["41006.0", "1.0", "0", "3"]],
                "ts": "1597026383085"
            }]
        }"#;
        let snapshot: OkxDepthMessage = serde_json::from_str(snapshot_json).unwrap();
        let mut book = snapshot.to_order_book_state();

        // Apply a delta: remove one ask, update one bid, add a new bid
        let delta_json = r#"{
            "arg": {"channel": "books", "instId": "BTC-USDT-SWAP"},
            "action": "update",
            "data": [{
                "asks": [["41006.8", "0", "0", "1"]],
                "bids": [["41006.3", "0.5", "0", "1"]],
                "ts": "1597026383090"
            }]
        }"#;
        let delta: OkxDepthMessage = serde_json::from_str(delta_json).unwrap();
        assert!(!delta.is_snapshot());
        delta.apply_delta(&mut book);

        // ask at 41006.8 should be removed, 41007.0 should remain
        let (bid, ask) = book.get_bbo();
        assert!(bid.is_some());
        assert_eq!(ask, Some(41007.0));
    }

    #[test]
    fn test_okx_depth_message_to_depth_update_data() {
        let json = r#"{
            "arg": {"channel": "books", "instId": "BTC-USDT-SWAP"},
            "action": "update",
            "data": [{
                "asks": [["41006.8", "0.5", "0", "1"]],
                "bids": [["41006.3", "0.2", "0", "1"], ["41005.0", "1.0", "0", "3"]],
                "ts": "1597026383085"
            }]
        }"#;

        let msg: OkxDepthMessage = serde_json::from_str(json).unwrap();
        let updates = msg.to_depth_update_data();
        assert_eq!(updates.len(), 3); // 2 bids + 1 ask
        assert_eq!(updates[0].ticker, "BTC-USDT-SWAP");
        assert!(!updates[0].side); // bid
        assert_eq!(updates[0].px, "41006.3");
        assert!(!updates[1].side); // bid
        assert!(updates[2].side); // ask
        assert_eq!(updates[2].px, "41006.8");
        assert_eq!(updates[0].venue, Venue::Okx);
    }

    #[test]
    fn test_okx_trades_message_parsing() {
        let json = r#"{
            "arg": {"channel": "trades", "instId": "BTC-USDT-SWAP"},
            "data": [{
                "instId": "BTC-USDT-SWAP",
                "tradeId": "123456",
                "px": "50000.5",
                "sz": "1",
                "side": "buy",
                "ts": "1620000000000"
            }]
        }"#;

        let msg: OkxTradesMessage = serde_json::from_str(json).unwrap();
        let trades = msg.to_trades();
        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].coin, "BTC-USDT-SWAP");
        assert!(!trades[0].side); // buy = false
        assert_eq!(trades[0].px, 50000.5);
        assert_eq!(trades[0].sz, 1.0);
    }

    #[test]
    fn test_okx_trades_message_to_trade_data() {
        let json = r#"{
            "arg": {"channel": "trades", "instId": "BTC-USDT-SWAP"},
            "data": [{
                "instId": "BTC-USDT-SWAP",
                "tradeId": "123456",
                "px": "50000.5",
                "sz": "1",
                "side": "sell",
                "ts": "1620000000000"
            }]
        }"#;

        let msg: OkxTradesMessage = serde_json::from_str(json).unwrap();
        let trade_data = msg.to_trade_data();
        assert_eq!(trade_data.len(), 1);
        assert_eq!(trade_data[0].ticker, "BTC-USDT-SWAP");
        assert!(trade_data[0].side); // sell = true
        assert_eq!(trade_data[0].venue, Venue::Okx);
    }
}
