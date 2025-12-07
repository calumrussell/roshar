use chrono::DateTime;
use serde::{Deserialize, Serialize};

use crate::{Candle, DepthUpdate, LocalOrderBook, LocalOrderBookError, OrderBookState, Trade, Venue};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ByBitMessage {
    pub req_id: String,
    pub op: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub args: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub success: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ret_msg: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ByBitDepthMessage {
    pub topic: String,
    #[serde(rename = "type")]
    pub snapshot_type: String,
    pub ts: u64,
    pub data: ByBitDepthBookData,
    pub cts: u64,
}

impl ByBitDepthMessage {
    pub fn is_full_update(&self) -> bool {
        self.snapshot_type == "snapshot"
    }

    pub fn to_order_book_state(&self) -> OrderBookState {
        let mut book = OrderBookState::new(50);

        for level in &self.data.b {
            let formatted_px = remove_trailing_zeros(level.first().unwrap());
            let formatted_sz = level.get(1).unwrap();

            if let Ok(price) = formatted_px.parse::<f64>() {
                if let Err(e) = book.set_bid(price, formatted_sz) {
                    log::error!("ByBit: failed to set bid for {} at price {}: {}", self.data.s, price, e);
                }
            }
        }

        for level in &self.data.a {
            let formatted_px = remove_trailing_zeros(level.first().unwrap());
            let formatted_sz = level.get(1).unwrap();

            if let Ok(price) = formatted_px.parse::<f64>() {
                if let Err(e) = book.set_ask(price, formatted_sz) {
                    log::error!("ByBit: failed to set ask for {} at price {}: {}", self.data.s, price, e);
                }
            }
        }

        book
    }

    pub fn to_depth_updates(&self) -> Result<Vec<DepthUpdate>, LocalOrderBookError> {
        if self.snapshot_type != "delta" {
            return Err(LocalOrderBookError::NotPartialUpdate(
                Venue::ByBit.to_string(),
                self.data.s.clone(),
            ));
        }

        let mut vals = Vec::new();
        let exchange_str = Venue::ByBit.as_str();
        let coin_str = self.data.s.as_str();

        for bid in &self.data.b {
            let internal = DepthUpdate {
                time: self.ts as i64,
                exchange: exchange_str.to_string(),
                side: false,
                coin: coin_str.to_string(),
                px: bid.first().unwrap().parse::<f64>().unwrap(),
                sz: bid.get(1).unwrap().parse::<f64>().unwrap(),
            };
            vals.push(internal);
        }

        for ask in &self.data.a {
            let internal = DepthUpdate {
                time: self.ts as i64,
                exchange: exchange_str.to_string(),
                side: true,
                coin: coin_str.to_string(),
                px: ask.first().unwrap().parse::<f64>().unwrap(),
                sz: ask.get(1).unwrap().parse::<f64>().unwrap(),
            };
            vals.push(internal);
        }

        Ok(vals)
    }

    pub fn to_depth_update_data(&self) -> Vec<crate::DepthUpdateData> {
        let mut res = Vec::new();

        for bid in &self.data.b {
            res.push(crate::DepthUpdateData {
                px: bid[0].clone(),
                qty: bid[1].clone(),
                time: self.ts,
                time_ts: DateTime::from_timestamp_millis(self.ts as i64).unwrap_or_default(),
                ticker: self.data.s.clone(),
                meta: format!(
                    "{{\"u\": {}, \"cts\": {}, \"seq\": {}}}",
                    self.data.u, self.cts, self.data.seq
                ),
                side: false, // bid side
                venue: Venue::ByBit,
            });
        }

        for ask in &self.data.a {
            res.push(crate::DepthUpdateData {
                px: ask[0].clone(),
                qty: ask[1].clone(),
                time: self.ts,
                time_ts: DateTime::from_timestamp_millis(self.ts as i64).unwrap_or_default(),
                ticker: self.data.s.clone(),
                meta: format!(
                    "{{\"u\": {}, \"cts\": {}, \"seq\": {}}}",
                    self.data.u, self.cts, self.data.seq
                ),
                side: true, // ask side
                venue: Venue::ByBit,
            });
        }

        res
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ByBitDepthBookData {
    pub s: String,
    pub b: Vec<[String; 2]>,
    pub a: Vec<[String; 2]>,
    pub u: u64,
    pub seq: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ByBitTradesMessage {
    pub topic: String,
    #[serde(rename = "type")]
    pub snapshot_type: String,
    pub ts: u64,
    pub data: Vec<ByBitTradesData>,
}

impl ByBitTradesMessage {
    pub fn to_trades(&self) -> Vec<Trade> {
        let mut vals = Vec::with_capacity(self.data.len());

        for trade in &self.data {
            let internal = Trade {
                time: trade.trade_time as i64,
                exchange: Venue::ByBit.to_string(),
                side: trade.side.eq("Sell"),
                coin: trade.s.clone(),
                px: trade.p.parse::<f64>().unwrap(),
                sz: trade.v.parse::<f64>().unwrap(),
            };
            vals.push(internal);
        }
        vals
    }

    pub fn to_trade_data(&self) -> Vec<crate::TradeData> {
        self.data
            .iter()
            .map(|trade| crate::TradeData {
                px: trade.p.clone(),
                qty: trade.v.clone(),
                time: trade.trade_time,
                time_ts: DateTime::from_timestamp_millis(trade.trade_time as i64)
                    .unwrap_or_default(),
                ticker: trade.s.clone(),
                meta: format!(
                    "{{\"i\": \"{}\", \"L\": \"{}\", \"BT\": {}, \"RPI\": {}}}",
                    trade.i,
                    trade.tick_direction.as_deref().unwrap_or("None"),
                    trade.is_block_trade,
                    trade.is_rpi_trade.unwrap_or(false)
                ),
                side: trade.side == "Sell", // true for sell, false for buy
                venue: Venue::ByBit,
            })
            .collect()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ByBitTradesData {
    #[serde(rename = "T")]
    pub trade_time: u64,
    pub s: String, // Symbol (e.g., "BTCUSDT")
    #[serde(rename = "S")]
    pub side: String, // Side of the trade (e.g., "Buy" or "Sell")
    pub v: String, // Volume (quantity traded)
    pub p: String, // Price
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "L")]
    pub tick_direction: Option<String>, // Tick direction (e.g., "PlusTick"), perps/futs
    pub i: String, // Trade ID
    #[serde(rename = "BT")]
    pub is_block_trade: bool, // Whether the trade is a block trade
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "RPI")]
    pub is_rpi_trade: Option<bool>, // Whether it is a RPI trade or not
}

// ByBit Candle/Kline Structures
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ByBitCandleMessage {
    pub topic: String,
    #[serde(rename = "type")]
    pub snapshot_type: String,
    pub ts: u64,
    pub data: Vec<ByBitCandleData>,
}

impl ByBitCandleMessage {
    pub fn to_candles(&self) -> Vec<Candle> {
        self.data
            .iter()
            .map(|candle_data| Candle {
                open: candle_data.open.clone(),
                high: candle_data.high.clone(),
                low: candle_data.low.clone(),
                close: candle_data.close.clone(),
                volume: candle_data.volume.clone(),
                exchange: Venue::ByBit.to_string(),
                time: DateTime::from_timestamp_millis(candle_data.start_time as i64)
                    .unwrap_or_default(),
                close_time: DateTime::from_timestamp_millis(candle_data.end_time as i64)
                    .unwrap_or_default(),
                coin: self
                    .topic
                    .split('.')
                    .nth(2)
                    .unwrap_or("unknown")
                    .to_string(),
            })
            .collect()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ByBitCandleData {
    #[serde(rename = "start")]
    pub start_time: u64, // Start time in milliseconds
    #[serde(rename = "end")]
    pub end_time: u64, // End time in milliseconds
    #[serde(rename = "interval")]
    pub interval: String, // Kline interval (e.g., "1")
    #[serde(rename = "open")]
    pub open: String, // Open price
    #[serde(rename = "high")]
    pub high: String, // High price
    #[serde(rename = "low")]
    pub low: String, // Low price
    #[serde(rename = "close")]
    pub close: String, // Close price
    #[serde(rename = "volume")]
    pub volume: String, // Trading volume
    #[serde(rename = "turnover")]
    pub turnover: String, // Trading turnover
    #[serde(rename = "confirm")]
    pub confirm: bool, // Whether the kline is confirmed
    #[serde(rename = "timestamp")]
    pub timestamp: u64, // Timestamp
}

// WebSocket Message Type
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ByBitWssMessage {
    pub req_id: String,
    pub op: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub args: Option<Vec<String>>,
}

impl ByBitWssMessage {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("failed to serialize ByBitWssMessage")
    }

    pub fn ping() -> Self {
        Self {
            req_id: "100001".to_string(),
            op: "ping".to_string(),
            args: None,
        }
    }

    pub fn depth(coin: &str) -> Self {
        Self {
            req_id: "test".to_string(),
            op: "subscribe".to_string(),
            args: Some(vec![format!("orderbook.50.{coin}")]),
        }
    }

    pub fn depth_unsub(coin: &str) -> Self {
        Self {
            req_id: "test".to_string(),
            op: "unsubscribe".to_string(),
            args: Some(vec![format!("orderbook.50.{coin}")]),
        }
    }

    pub fn trades(coin: &str) -> Self {
        Self {
            req_id: "test".to_string(),
            op: "subscribe".to_string(),
            args: Some(vec![format!("publicTrade.{}", coin)]),
        }
    }

    pub fn trades_unsub(coin: &str) -> Self {
        Self {
            req_id: "test".to_string(),
            op: "unsubscribe".to_string(),
            args: Some(vec![format!("publicTrade.{}", coin)]),
        }
    }

    pub fn candle(coin: &str) -> Self {
        Self {
            req_id: "test".to_string(),
            op: "subscribe".to_string(),
            args: Some(vec![format!("kline.1.{}", coin)]),
        }
    }
}

fn remove_trailing_zeros(s: &str) -> String {
    if s.contains('.') {
        let mut result = s.trim_end_matches('0').to_string();
        if result.ends_with('.') {
            result = result.trim_end_matches('.').to_string();
        }
        result
    } else {
        s.to_string()
    }
}

// Order book management
pub struct BybitOrderBook {
    pub symbol: String,
    pub book: Option<OrderBookState>,
}

impl BybitOrderBook {
    pub fn new(symbol: String) -> Self {
        Self { symbol, book: None }
    }

    /// Get a read-only view of the order book for calculations
    pub fn as_view(&self) -> Option<LocalOrderBook<'_>> {
        self.book.as_ref().map(|b| b.as_view())
    }

    pub fn new_update(&mut self, msg: &ByBitDepthMessage) -> Result<(), LocalOrderBookError> {
        let coin = msg.data.s.clone();

        // Validate symbol
        if coin != self.symbol {
            return Err(LocalOrderBookError::WrongSymbol(self.symbol.clone(), coin));
        }

        if msg.is_full_update() {
            let book = msg.to_order_book_state();
            self.book = Some(book);
            return Ok(());
        }

        if let Some(ref mut book) = self.book {
            // Apply updates directly
            for bid in &msg.data.b {
                let formatted_px = remove_trailing_zeros(bid.first().unwrap());
                if let Ok(price) = formatted_px.parse::<f64>() {
                    if let Err(e) = book.set_bid(price, bid.get(1).unwrap()) {
                        log::error!("ByBit: failed to set bid update for {} at price {}: {}", coin, price, e);
                    }
                }
            }
            for ask in &msg.data.a {
                let formatted_px = remove_trailing_zeros(ask.first().unwrap());
                if let Ok(price) = formatted_px.parse::<f64>() {
                    if let Err(e) = book.set_ask(price, ask.get(1).unwrap()) {
                        log::error!("ByBit: failed to set ask update for {} at price {}: {}", coin, price, e);
                    }
                }
            }
        } else {
            return Err(LocalOrderBookError::BookUpdateBeforeSnapshot(
                Venue::ByBit.to_string(),
                coin,
            ));
        }

        // Validate BBO
        let validation_result = if let Some(ref book) = self.book {
            let (bid, ask) = book.get_bbo();
            match (bid, ask) {
                (Some(b), Some(a)) if b > a => Err(LocalOrderBookError::BidAboveAsk(
                    b.to_string(),
                    a.to_string(),
                    Venue::ByBit.to_string(),
                    coin.clone(),
                )),
                _ => Ok(()),
            }
        } else {
            Err(LocalOrderBookError::BookUpdateBeforeSnapshot(
                Venue::ByBit.to_string(),
                coin.clone(),
            ))
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

    fn create_test_snapshot_message(symbol: &str) -> ByBitDepthMessage {
        ByBitDepthMessage {
            topic: format!("orderbook.50.{symbol}"),
            snapshot_type: "snapshot".to_string(),
            ts: 1234567890,
            data: ByBitDepthBookData {
                s: symbol.to_string(),
                b: vec![
                    ["50000.0".to_string(), "1.0".to_string()],
                    ["49999.0".to_string(), "2.0".to_string()],
                ],
                a: vec![
                    ["50001.0".to_string(), "1.5".to_string()],
                    ["50002.0".to_string(), "2.5".to_string()],
                ],
                u: 1000,
                seq: 12345,
            },
            cts: 1234567891,
        }
    }

    fn create_test_delta_message(symbol: &str) -> ByBitDepthMessage {
        ByBitDepthMessage {
            topic: format!("orderbook.50.{symbol}"),
            snapshot_type: "delta".to_string(),
            ts: 1234567892,
            data: ByBitDepthBookData {
                s: symbol.to_string(),
                b: vec![["49998.0".to_string(), "3.0".to_string()]],
                a: vec![["50003.0".to_string(), "4.0".to_string()]],
                u: 1001,
                seq: 12346,
            },
            cts: 1234567893,
        }
    }

    #[test]
    fn test_bybit_depth_message_is_full_update() {
        let snapshot_msg = create_test_snapshot_message("BTCUSDT");
        let delta_msg = create_test_delta_message("BTCUSDT");

        assert!(snapshot_msg.is_full_update());
        assert!(!delta_msg.is_full_update());
    }

    #[test]
    fn test_bybit_order_book_snapshot_processing() {
        let mut order_book = BybitOrderBook::new("BTCUSDT".to_string());
        let snapshot_msg = create_test_snapshot_message("BTCUSDT");

        let result = order_book.new_update(&snapshot_msg);
        assert!(result.is_ok());
        assert!(order_book.book.is_some());

        let view = order_book.as_view().unwrap();
        assert_eq!(view.bid_prices().len(), 2);
        assert_eq!(view.ask_prices().len(), 2);
    }

    #[test]
    fn test_bybit_order_book_delta_update_on_existing_book() {
        let mut order_book = BybitOrderBook::new("BTCUSDT".to_string());
        let snapshot_msg = create_test_snapshot_message("BTCUSDT");
        let delta_msg = create_test_delta_message("BTCUSDT");

        order_book.new_update(&snapshot_msg).unwrap();

        let result = order_book.new_update(&delta_msg);
        assert!(result.is_ok());
        assert!(order_book.book.is_some());
    }

    #[test]
    fn test_bybit_order_book_delta_update_before_snapshot_error() {
        let mut order_book = BybitOrderBook::new("BTCUSDT".to_string());
        let delta_msg = create_test_delta_message("BTCUSDT");

        let result = order_book.new_update(&delta_msg);
        assert!(result.is_err());
        if let Err(LocalOrderBookError::BookUpdateBeforeSnapshot(exchange, coin)) = result {
            assert_eq!(exchange, Venue::ByBit.to_string());
            assert_eq!(coin, "BTCUSDT");
        } else {
            panic!("Expected BookUpdateBeforeSnapshot error");
        }
    }

    #[test]
    fn test_bybit_order_book_wrong_symbol() {
        let mut order_book = BybitOrderBook::new("BTCUSDT".to_string());
        let eth_snapshot = create_test_snapshot_message("ETHUSDT");

        let result = order_book.new_update(&eth_snapshot);
        assert!(result.is_err());
        if let Err(LocalOrderBookError::WrongSymbol(expected, received)) = result {
            assert_eq!(expected, "BTCUSDT");
            assert_eq!(received, "ETHUSDT");
        } else {
            panic!("Expected WrongSymbol error");
        }
    }

    #[test]
    fn test_bybit_order_book_snapshot_overwrites_existing() {
        let mut order_book = BybitOrderBook::new("BTCUSDT".to_string());
        let snapshot1 = create_test_snapshot_message("BTCUSDT");
        let mut snapshot2 = create_test_snapshot_message("BTCUSDT");

        snapshot2.data.b = vec![["51000.0".to_string(), "5.0".to_string()]];

        order_book.new_update(&snapshot1).unwrap();
        assert!(order_book.book.is_some());

        order_book.new_update(&snapshot2).unwrap();
        let view = order_book.as_view().unwrap();

        assert_eq!(view.bid_prices()[0], "51000");
    }

    #[test]
    fn test_wss_message_ping() {
        let msg = ByBitWssMessage::ping();
        let json = msg.to_json();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["op"], "ping");
        assert_eq!(parsed["req_id"], "100001");
    }

    #[test]
    fn test_wss_message_depth() {
        let msg = ByBitWssMessage::depth("BTCUSDT");
        let json = msg.to_json();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["op"], "subscribe");
        assert_eq!(parsed["args"][0], "orderbook.50.BTCUSDT");
    }

    #[test]
    fn test_wss_message_trades() {
        let msg = ByBitWssMessage::trades("BTCUSDT");
        let json = msg.to_json();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["op"], "subscribe");
        assert_eq!(parsed["args"][0], "publicTrade.BTCUSDT");
    }
}
