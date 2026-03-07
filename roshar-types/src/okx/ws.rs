use chrono::DateTime;
use serde::{Deserialize, Serialize};

use crate::{OrderBookState, Trade, Venue};

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
                channel: "books5".to_string(),
                inst_id: coin.to_string(),
            }],
        }
    }

    pub fn depth_unsub(coin: &str) -> Self {
        Self {
            op: "unsubscribe".to_string(),
            args: vec![OkxWssArg {
                channel: "books5".to_string(),
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

// --- Depth (books5) push data ---

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OkxDepthMessage {
    pub arg: OkxWssArg,
    pub data: Vec<OkxDepthBookData>,
}

impl OkxDepthMessage {
    /// Extract the instId from the arg field
    pub fn inst_id(&self) -> &str {
        &self.arg.inst_id
    }

    /// Convert the books5 snapshot to an OrderBookState.
    /// books5 is always a full 5-level snapshot, no delta management needed.
    pub fn to_order_book_state(&self) -> OrderBookState {
        let mut book = OrderBookState::new(5);

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
        assert_eq!(parsed["args"][0]["channel"], "books5");
        assert_eq!(parsed["args"][0]["instId"], "BTC-USDT-SWAP");
    }

    #[test]
    fn test_okx_wss_message_depth_unsubscribe() {
        let msg = OkxWssMessage::depth_unsub("BTC-USDT-SWAP");
        let json = msg.to_json();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["op"], "unsubscribe");
        assert_eq!(parsed["args"][0]["channel"], "books5");
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
            "arg": {"channel": "books5", "instId": "BTC-USDT-SWAP"},
            "data": [{
                "asks": [["41006.8", "0.60038921", "0", "1"], ["41007.0", "0.5", "0", "2"]],
                "bids": [["41006.3", "0.20572000", "0", "1"], ["41006.0", "1.0", "0", "3"]],
                "ts": "1597026383085"
            }]
        }"#;

        let msg: OkxDepthMessage = serde_json::from_str(json).unwrap();
        assert_eq!(msg.inst_id(), "BTC-USDT-SWAP");
        assert_eq!(msg.data[0].asks.len(), 2);
        assert_eq!(msg.data[0].bids.len(), 2);

        let book = msg.to_order_book_state();
        let (bid, ask) = book.get_bbo();
        assert!(bid.is_some());
        assert!(ask.is_some());
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
