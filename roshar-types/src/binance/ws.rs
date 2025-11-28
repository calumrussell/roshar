use chrono::DateTime;
use serde::{Deserialize, Serialize};

use crate::{DepthUpdate, Venue};

// WebSocket Message Type
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct BinanceWssMessage {
    pub id: u32,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Vec<String>>,
}

impl BinanceWssMessage {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("failed to serialize BinanceWssMessage")
    }

    pub fn ping() -> Self {
        Self {
            id: 1,
            method: "ping".to_string(),
            params: None,
        }
    }

    pub fn depth(symbol: &str) -> Self {
        Self {
            id: 1,
            method: "SUBSCRIBE".to_string(),
            params: Some(vec![format!("{}@depth@100ms", symbol.to_lowercase())]),
        }
    }

    pub fn depth_unsub(symbol: &str) -> Self {
        Self {
            id: 1,
            method: "UNSUBSCRIBE".to_string(),
            params: Some(vec![format!("{}@depth@100ms", symbol.to_lowercase())]),
        }
    }

    pub fn trades(symbol: &str) -> Self {
        Self {
            id: 1,
            method: "SUBSCRIBE".to_string(),
            params: Some(vec![format!("{}@trade", symbol.to_lowercase())]),
        }
    }

    pub fn trades_unsub(symbol: &str) -> Self {
        Self {
            id: 1,
            method: "UNSUBSCRIBE".to_string(),
            params: Some(vec![format!("{}@trade", symbol.to_lowercase())]),
        }
    }

    pub fn candle(symbol: &str) -> Self {
        Self {
            id: 1,
            method: "SUBSCRIBE".to_string(),
            params: Some(vec![format!("{}@kline_1m", symbol.to_lowercase())]),
        }
    }

    pub fn candle_unsub(symbol: &str) -> Self {
        Self {
            id: 1,
            method: "UNSUBSCRIBE".to_string(),
            params: Some(vec![format!("{}@kline_1m", symbol.to_lowercase())]),
        }
    }

    pub fn batch_depth(symbols: &[String]) -> Self {
        let params: Vec<String> = symbols
            .iter()
            .map(|symbol| format!("{}@depth@100ms", symbol.to_lowercase()))
            .collect();
        Self {
            id: 1,
            method: "SUBSCRIBE".to_string(),
            params: Some(params),
        }
    }

    pub fn batch_trades(symbols: &[String]) -> Self {
        let params: Vec<String> = symbols
            .iter()
            .map(|symbol| format!("{}@trade", symbol.to_lowercase()))
            .collect();
        Self {
            id: 1,
            method: "SUBSCRIBE".to_string(),
            params: Some(params),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct BinanceDepthDiffMessage {
    #[serde(rename = "e")]
    pub event_type: String, // "depthUpdate"
    #[serde(rename = "E")]
    pub event_time: u64,
    #[serde(rename = "s")]
    pub symbol: String,
    #[serde(rename = "U")]
    pub first_update_id: u64,
    #[serde(rename = "u")]
    pub final_update_id: u64,
    #[serde(rename = "b")]
    pub bids: Vec<[String; 2]>, // [price, quantity]
    #[serde(rename = "a")]
    pub asks: Vec<[String; 2]>, // [price, quantity]
    #[serde(rename = "pu")]
    pub previous_final_update_id: u64,
}

impl BinanceDepthDiffMessage {
    pub fn to_depth_updates(&self) -> Result<Vec<DepthUpdate>, crate::LocalOrderBookError> {
        let mut updates = Vec::new();
        let exchange_str = Venue::Binance.as_str();
        let coin_str = self.symbol.as_str();

        for bid in &self.bids {
            let price: f64 = bid[0].parse().map_err(|_| {
                crate::LocalOrderBookError::UnparseableInputs(
                    exchange_str.to_string(),
                    coin_str.to_string(),
                )
            })?;
            let quantity: f64 = bid[1].parse().map_err(|_| {
                crate::LocalOrderBookError::UnparseableInputs(
                    exchange_str.to_string(),
                    coin_str.to_string(),
                )
            })?;

            updates.push(DepthUpdate {
                time: self.event_time as i64,
                exchange: exchange_str.to_string(),
                side: false,
                coin: coin_str.to_string(),
                px: price,
                sz: quantity,
            });
        }

        for ask in &self.asks {
            let price: f64 = ask[0].parse().map_err(|_| {
                crate::LocalOrderBookError::UnparseableInputs(
                    exchange_str.to_string(),
                    coin_str.to_string(),
                )
            })?;
            let quantity: f64 = ask[1].parse().map_err(|_| {
                crate::LocalOrderBookError::UnparseableInputs(
                    exchange_str.to_string(),
                    coin_str.to_string(),
                )
            })?;

            updates.push(DepthUpdate {
                time: self.event_time as i64,
                exchange: exchange_str.to_string(),
                side: true,
                coin: coin_str.to_string(),
                px: price,
                sz: quantity,
            });
        }

        Ok(updates)
    }

    pub fn to_depth_update_data(&self) -> Vec<crate::DepthUpdateData> {
        let mut res = Vec::new();

        for bid in &self.bids {
            res.push(crate::DepthUpdateData {
                px: bid[0].clone(),
                qty: bid[1].clone(),
                time: self.event_time,
                time_ts: DateTime::from_timestamp_millis(self.event_time as i64)
                    .unwrap_or_default(),
                ticker: self.symbol.clone(),
                meta: format!(
                    "{{\"first_update_id\": {}, \"final_update_id\": {}}}",
                    self.first_update_id, self.final_update_id
                ),
                side: false, // bid side
                venue: Venue::Binance,
            });
        }

        for ask in &self.asks {
            res.push(crate::DepthUpdateData {
                px: ask[0].clone(),
                qty: ask[1].clone(),
                time: self.event_time,
                time_ts: DateTime::from_timestamp_millis(self.event_time as i64)
                    .unwrap_or_default(),
                ticker: self.symbol.clone(),
                meta: format!(
                    "{{\"first_update_id\": {}, \"final_update_id\": {}}}",
                    self.first_update_id, self.final_update_id
                ),
                side: true, // ask side
                venue: Venue::Binance,
            });
        }

        res
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct BinanceOrderBookSnapshot {
    #[serde(rename = "lastUpdateId")]
    pub last_update_id: u64,
    pub bids: Vec<[String; 2]>,
    pub asks: Vec<[String; 2]>,
}

impl BinanceOrderBookSnapshot {
    pub fn to_depth_updates(&self) -> Result<Vec<DepthUpdate>, crate::LocalOrderBookError> {
        let mut updates = Vec::new();
        let timestamp = chrono::Utc::now().timestamp_millis();

        for bid in &self.bids {
            let price: f64 = bid[0].parse().map_err(|_| {
                crate::LocalOrderBookError::UnparseableInputs(
                    Venue::Binance.to_string(),
                    String::new(),
                )
            })?;
            let quantity: f64 = bid[1].parse().map_err(|_| {
                crate::LocalOrderBookError::UnparseableInputs(
                    Venue::Binance.to_string(),
                    String::new(),
                )
            })?;

            if quantity > 0.0 {
                updates.push(DepthUpdate {
                    time: timestamp,
                    exchange: Venue::Binance.to_string(),
                    side: false,
                    coin: String::new(),
                    px: price,
                    sz: quantity,
                });
            }
        }

        for ask in &self.asks {
            let price: f64 = ask[0].parse().map_err(|_| {
                crate::LocalOrderBookError::UnparseableInputs(
                    Venue::Binance.to_string(),
                    String::new(),
                )
            })?;
            let quantity: f64 = ask[1].parse().map_err(|_| {
                crate::LocalOrderBookError::UnparseableInputs(
                    Venue::Binance.to_string(),
                    String::new(),
                )
            })?;

            if quantity > 0.0 {
                updates.push(DepthUpdate {
                    time: timestamp,
                    exchange: Venue::Binance.to_string(),
                    side: true,
                    coin: String::new(),
                    px: price,
                    sz: quantity,
                });
            }
        }

        Ok(updates)
    }

    pub fn to_depth_snapshot_data(&self) -> crate::DepthSnapshotData {
        let bid_prices: Vec<String> = self.bids.iter().map(|b| b[0].clone()).collect();
        let bid_sizes: Vec<String> = self.bids.iter().map(|b| b[1].clone()).collect();
        let ask_prices: Vec<String> = self.asks.iter().map(|a| a[0].clone()).collect();
        let ask_sizes: Vec<String> = self.asks.iter().map(|a| a[1].clone()).collect();

        crate::DepthSnapshotData {
            bid_prices,
            bid_sizes,
            ask_prices,
            ask_sizes,
            time: 0, // Binance snapshots don't have a timestamp field, will use current time
            time_ts: chrono::Utc::now(),
            ticker: String::new(), // Symbol not included in snapshot, will be set from context
            venue: crate::Venue::Binance,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct BinanceKlineData {
    #[serde(rename = "t")]
    pub open_time: u64, // Kline start time
    #[serde(rename = "T")]
    pub close_time: u64, // Kline close time
    #[serde(rename = "s")]
    pub symbol: String, // Symbol
    #[serde(rename = "i")]
    pub interval: String, // Interval
    #[serde(rename = "f")]
    pub first_trade_id: Option<i64>, // First trade ID (can be -1 when no trades)
    #[serde(rename = "L")]
    pub last_trade_id: Option<i64>, // Last trade ID (can be -1 when no trades)
    #[serde(rename = "o")]
    pub open: String, // Open price
    #[serde(rename = "c")]
    pub close: String, // Close price
    #[serde(rename = "h")]
    pub high: String, // High price
    #[serde(rename = "l")]
    pub low: String, // Low price
    #[serde(rename = "v")]
    pub volume: String, // Base asset volume
    #[serde(rename = "n")]
    pub trades: u64, // Number of trades
    #[serde(rename = "x")]
    pub closed: bool, // Is this kline closed?
    #[serde(rename = "q")]
    pub quote_volume: String, // Quote asset volume
    #[serde(rename = "V")]
    pub taker_buy_volume: String, // Taker buy base asset volume
    #[serde(rename = "Q")]
    pub taker_buy_quote_volume: String, // Taker buy quote asset volume
    #[serde(rename = "B")]
    pub ignore: String, // Ignore
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct BinanceCandleMessage {
    #[serde(rename = "e")]
    pub event_type: String, // "kline"
    #[serde(rename = "E")]
    pub event_time: u64, // Event time
    #[serde(rename = "s")]
    pub symbol: String, // Symbol
    #[serde(rename = "k")]
    pub kline: BinanceKlineData, // Kline data
}

impl BinanceCandleMessage {
    pub fn to_candle(&self) -> crate::Candle {
        let time = DateTime::from_timestamp_millis(self.kline.open_time as i64)
            .unwrap_or_else(|| {
                log::warn!(
                    "Invalid open_time timestamp {} for Binance candle symbol {}, using default time",
                    self.kline.open_time,
                    self.kline.symbol
                );
                DateTime::default()
            });

        let close_time = DateTime::from_timestamp_millis(self.kline.close_time as i64)
            .unwrap_or_else(|| {
                log::warn!(
                    "Invalid close_time timestamp {} for Binance candle symbol {}, using default time",
                    self.kline.close_time,
                    self.kline.symbol
                );
                DateTime::default()
            });

        crate::Candle {
            open: self.kline.open.clone(),
            high: self.kline.high.clone(),
            low: self.kline.low.clone(),
            close: self.kline.close.clone(),
            volume: self.kline.quote_volume.clone(),
            exchange: "binance".to_string(),
            time,
            close_time,
            coin: self.kline.symbol.clone(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct BinanceTradeMessage {
    #[serde(rename = "e")]
    pub event_type: String, // "trade"
    #[serde(rename = "E")]
    pub event_time: u64,
    #[serde(rename = "s")]
    pub symbol: String,
    #[serde(rename = "t")]
    pub trade_id: u64,
    #[serde(rename = "p")]
    pub price: String,
    #[serde(rename = "q")]
    pub qty: String,
    #[serde(rename = "T")]
    pub trade_time: u64,
    #[serde(rename = "X", skip_serializing_if = "Option::is_none")]
    pub order_type: Option<String>, // "MARKET", "LIMIT", etc. - optional field
    #[serde(rename = "m")]
    pub is_buyer_maker: bool,
}

impl BinanceTradeMessage {
    pub fn to_trade_data(&self) -> Vec<crate::TradeData> {
        let time_ts = DateTime::from_timestamp_millis(self.trade_time as i64)
            .unwrap_or_else(|| {
                log::warn!(
                    "Invalid trade_time timestamp {} for Binance trade symbol {}, using default time",
                    self.trade_time,
                    self.symbol
                );
                DateTime::default()
            });

        vec![crate::TradeData {
            px: self.price.clone(),
            qty: self.qty.clone(),
            time: self.trade_time,
            time_ts,
            ticker: self.symbol.clone(),
            meta: format!(
                "{{\"event_time\": {}, \"is_buyer_maker\": {}}}",
                self.event_time, self.is_buyer_maker
            ),
            side: !self.is_buyer_maker, // side indicates taker direction: true = sell taker, false = buy taker
            venue: Venue::Binance,
        }]
    }
}

// Order book management
use std::collections::VecDeque;

use crate::{LocalOrderBook, LocalOrderBookError};

#[derive(Clone)]
pub struct BinanceOrderBook {
    pub symbol: String,
    pub book: Option<LocalOrderBook>,
    pub counter: u64,
    snapshot: Option<BinanceOrderBookSnapshot>,
    event_buff: VecDeque<BinanceDepthDiffMessage>,
}

impl BinanceOrderBook {
    pub fn new(symbol: String) -> Self {
        Self {
            symbol,
            book: None,
            counter: 0,
            snapshot: None,
            event_buff: VecDeque::new(),
        }
    }

    /// Reset all order book state to handle reconnection
    /// Clears snapshot, book, counter, and event buffer to allow fresh snapshot fetch
    pub fn reset(&mut self) {
        self.snapshot = None;
        self.book = None;
        self.counter = 0;
        self.event_buff.clear();
    }

    /// Check if a snapshot is needed (no snapshot and no book)
    pub fn needs_snapshot(&self) -> bool {
        self.snapshot.is_none() && self.book.is_none()
    }

    /// Provide a snapshot fetched externally
    pub fn set_snapshot(&mut self, snapshot: BinanceOrderBookSnapshot) {
        // Clean old events from buffer before building orderbook
        while let Some(ev) = self.event_buff.front() {
            if ev.final_update_id < snapshot.last_update_id {
                self.event_buff.pop_front();
            } else {
                break;
            }
        }
        self.snapshot = Some(snapshot);
    }

    /// Build the order book from snapshot and buffered events
    fn build_order_book_from_snapshot(
        &mut self,
        snapshot: &BinanceOrderBookSnapshot,
    ) -> Result<(), LocalOrderBookError> {
        let mut updates = Vec::new();

        let exchange_str = Venue::Binance.as_str();
        let coin_str = self.symbol.as_str();
        let time = chrono::Utc::now().timestamp_millis();

        for bid in &snapshot.bids {
            if let (Ok(price), Ok(quantity)) = (bid[0].parse::<f64>(), bid[1].parse::<f64>()) {
                if quantity > 0.0 {
                    updates.push(DepthUpdate {
                        time,
                        exchange: exchange_str.to_string(),
                        side: false,
                        coin: coin_str.to_string(),
                        px: price,
                        sz: quantity,
                    });
                }
            } else {
                return Err(LocalOrderBookError::UnparseableInputs(
                    exchange_str.to_string(),
                    coin_str.to_string(),
                ));
            }
        }

        for ask in &snapshot.asks {
            if let (Ok(price), Ok(quantity)) = (ask[0].parse::<f64>(), ask[1].parse::<f64>()) {
                if quantity > 0.0 {
                    updates.push(DepthUpdate {
                        time,
                        exchange: exchange_str.to_string(),
                        side: true,
                        coin: coin_str.to_string(),
                        px: price,
                        sz: quantity,
                    });
                }
            } else {
                return Err(LocalOrderBookError::UnparseableInputs(
                    exchange_str.to_string(),
                    coin_str.to_string(),
                ));
            }
        }

        let mut local_book = LocalOrderBook::new(
            chrono::Utc::now().timestamp_millis(),
            "binance".to_string(),
            self.symbol.to_string(),
        );

        local_book.apply_updates(&updates, 50);

        // Start with snapshot's last_update_id, then update if we have buffered events
        let mut last_final_update_id: u64 = snapshot.last_update_id;
        while !self.event_buff.is_empty() {
            if let Some(event) = self.event_buff.pop_front() {
                last_final_update_id = event.final_update_id;
                if let Ok(updates) = event.to_depth_updates() {
                    local_book.apply_updates(&updates, 50);
                }
            }
        }

        self.counter = last_final_update_id;
        self.book = Some(local_book);
        self.event_buff.clear();

        Ok(())
    }

    pub fn new_update_diff(
        &mut self,
        diff: &BinanceDepthDiffMessage,
    ) -> Result<(), LocalOrderBookError> {
        let symbol = diff.symbol.clone();

        // Validate symbol
        if symbol != self.symbol {
            return Err(LocalOrderBookError::WrongSymbol(
                self.symbol.clone(),
                symbol,
            ));
        }

        // If no book yet, buffer the event
        if self.book.is_none() {
            self.event_buff.push_back(diff.clone());
        }

        if let Some(ref snapshot) = self.snapshot.clone() {
            if self.book.is_none() {
                let ev_count = self.event_buff.len();

                let should_clear = if ev_count == 0 {
                    diff.final_update_id > snapshot.last_update_id
                } else {
                    let front_ev = self.event_buff.front().unwrap();
                    !(front_ev.first_update_id <= snapshot.last_update_id
                        && front_ev.final_update_id >= snapshot.last_update_id)
                };

                if should_clear {
                    self.snapshot = None;
                    self.book = None;
                    self.counter = 0;
                    self.event_buff.clear();
                    return Ok(());
                }

                if diff.final_update_id < snapshot.last_update_id {
                    // Waiting for more messages
                    return Ok(());
                }

                // Build order book from snapshot
                self.build_order_book_from_snapshot(&snapshot)?;
            } else {
                // Accept message if it chains correctly:
                // 1. Exact match: previous_final_update_id == counter (normal case)
                // 2. Counter within range: first_update_id <= counter <= final_update_id (after snapshot)
                let is_valid_sequence = diff.previous_final_update_id == self.counter
                    || (diff.first_update_id <= self.counter
                        && self.counter <= diff.final_update_id);

                if is_valid_sequence {
                    if let Some(ref mut book) = self.book {
                        if let Ok(updates) = diff.to_depth_updates() {
                            book.apply_updates(&updates, 50);
                            self.counter = diff.final_update_id;
                        }
                    }
                } else {
                    // Sequence mismatch - reset state
                    self.snapshot = None;
                    self.book = None;
                    self.counter = 0;
                    self.event_buff.clear();
                }
            }
        }

        Ok(())
    }
}

// REST API response types (for reference/deserialization only, no client code)
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ExchangeInfo {
    pub timezone: String,
    #[serde(rename = "serverTime")]
    pub server_time: i64,
    pub symbols: Vec<SymbolInfo>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SymbolInfo {
    pub symbol: String,
    pub status: String,
    #[serde(rename = "contractType")]
    pub contract_type: String,
    #[serde(rename = "baseAsset")]
    pub base_asset: String,
    #[serde(rename = "quoteAsset")]
    pub quote_asset: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OpenInterestData {
    pub symbol: String,
    #[serde(rename = "openInterest")]
    pub open_interest: String,
    pub time: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TickerData {
    pub symbol: String,
    pub volume: String,
    #[serde(rename = "quoteVolume")]
    pub quote_volume: String,
    #[serde(rename = "lastPrice")]
    pub last_price: String,
    #[serde(rename = "priceChange")]
    pub price_change: String,
    #[serde(rename = "priceChangePercent")]
    pub price_change_percent: String,
    #[serde(rename = "highPrice")]
    pub high_price: String,
    #[serde(rename = "lowPrice")]
    pub low_price: String,
    #[serde(rename = "openPrice")]
    pub open_price: String,
    #[serde(rename = "weightedAvgPrice")]
    pub weighted_avg_price: String,
    #[serde(rename = "lastQty")]
    pub last_qty: String,
    #[serde(rename = "openTime")]
    pub open_time: i64,
    #[serde(rename = "closeTime")]
    pub close_time: i64,
    #[serde(rename = "firstId")]
    pub first_id: i64,
    #[serde(rename = "lastId")]
    pub last_id: i64,
    pub count: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wss_message_ping() {
        let msg = BinanceWssMessage::ping();
        let json = msg.to_json();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["method"], "ping");
        assert_eq!(parsed["id"], 1);
    }

    #[test]
    fn test_wss_message_depth() {
        let msg = BinanceWssMessage::depth("BTCUSDT");
        let json = msg.to_json();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["method"], "SUBSCRIBE");
        assert_eq!(parsed["params"][0], "btcusdt@depth@100ms");
    }

    #[test]
    fn test_wss_message_trades() {
        let msg = BinanceWssMessage::trades("BTCUSDT");
        let json = msg.to_json();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["method"], "SUBSCRIBE");
        assert_eq!(parsed["params"][0], "btcusdt@trade");
    }

    #[test]
    fn test_wss_message_candle() {
        let msg = BinanceWssMessage::candle("BTCUSDT");
        let json = msg.to_json();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["method"], "SUBSCRIBE");
        assert_eq!(parsed["params"][0], "btcusdt@kline_1m");
    }

    #[test]
    fn test_binance_candle_message_parsing() {
        let json_str = r#"{
            "e": "kline",
            "E": 1640995200000,
            "s": "BTCUSDT",
            "k": {
                "t": 1640995200000,
                "T": 1640995260000,
                "s": "BTCUSDT",
                "i": "1m",
                "f": 100,
                "L": 200,
                "o": "50000.00",
                "c": "50100.00",
                "h": "50200.00",
                "l": "49900.00",
                "v": "1000",
                "n": 100,
                "x": true,
                "q": "50050000.00",
                "V": "500",
                "Q": "25025000.00",
                "B": "123456"
            }
        }"#;

        let candle_message: BinanceCandleMessage = serde_json::from_str(json_str).unwrap();
        assert_eq!(candle_message.event_type, "kline");
        assert_eq!(candle_message.symbol, "BTCUSDT");
        assert_eq!(candle_message.kline.open, "50000.00");
        assert_eq!(candle_message.kline.close, "50100.00");
        assert_eq!(candle_message.kline.high, "50200.00");
        assert_eq!(candle_message.kline.low, "49900.00");
        assert_eq!(candle_message.kline.interval, "1m");
        assert!(candle_message.kline.closed);
    }

    #[test]
    fn test_binance_candle_to_candle_conversion() {
        let binance_candle = BinanceCandleMessage {
            event_type: "kline".to_string(),
            event_time: 1640995200000,
            symbol: "BTCUSDT".to_string(),
            kline: BinanceKlineData {
                open_time: 1640995200000,
                close_time: 1640995260000,
                symbol: "BTCUSDT".to_string(),
                interval: "1m".to_string(),
                first_trade_id: Some(100),
                last_trade_id: Some(200),
                open: "50000.00".to_string(),
                close: "50100.00".to_string(),
                high: "50200.00".to_string(),
                low: "49900.00".to_string(),
                volume: "1000".to_string(),
                trades: 100,
                closed: true,
                quote_volume: "50050000.00".to_string(),
                taker_buy_volume: "500".to_string(),
                taker_buy_quote_volume: "25025000.00".to_string(),
                ignore: "123456".to_string(),
            },
        };

        let candle = binance_candle.to_candle();
        assert_eq!(candle.open, "50000.00");
        assert_eq!(candle.close, "50100.00");
        assert_eq!(candle.high, "50200.00");
        assert_eq!(candle.low, "49900.00");
        assert_eq!(candle.volume, "50050000.00");
        assert_eq!(candle.exchange, "binance");
        assert_eq!(candle.coin, "BTCUSDT");
    }

    #[test]
    fn test_binance_snapshot_to_depth_updates() {
        let snapshot = BinanceOrderBookSnapshot {
            last_update_id: 1000,
            bids: vec![
                ["50000.0".to_string(), "1.0".to_string()],
                ["49999.0".to_string(), "2.0".to_string()],
            ],
            asks: vec![
                ["50001.0".to_string(), "1.5".to_string()],
                ["50002.0".to_string(), "2.5".to_string()],
            ],
        };

        let updates = snapshot.to_depth_updates().unwrap();

        assert_eq!(updates.len(), 4);

        let bid_updates: Vec<&DepthUpdate> = updates.iter().filter(|u| !u.side).collect();
        let ask_updates: Vec<&DepthUpdate> = updates.iter().filter(|u| u.side).collect();

        assert_eq!(bid_updates.len(), 2);
        assert_eq!(ask_updates.len(), 2);

        assert_eq!(bid_updates[0].px, 50000.0);
        assert_eq!(bid_updates[0].sz, 1.0);
        assert_eq!(ask_updates[0].px, 50001.0);
        assert_eq!(ask_updates[0].sz, 1.5);
    }

    fn create_test_snapshot() -> BinanceOrderBookSnapshot {
        BinanceOrderBookSnapshot {
            last_update_id: 1000,
            bids: vec![
                ["50000.0".to_string(), "1.0".to_string()],
                ["49999.0".to_string(), "2.0".to_string()],
            ],
            asks: vec![
                ["50001.0".to_string(), "1.5".to_string()],
                ["50002.0".to_string(), "2.5".to_string()],
            ],
        }
    }

    fn create_test_diff_message(
        symbol: &str,
        first_update_id: u64,
        final_update_id: u64,
        previous_final_update_id: u64,
    ) -> BinanceDepthDiffMessage {
        BinanceDepthDiffMessage {
            event_type: "depthUpdate".to_string(),
            event_time: 1234567890,
            symbol: symbol.to_string(),
            first_update_id,
            final_update_id,
            bids: vec![["50000.0".to_string(), "1.0".to_string()]],
            asks: vec![],
            previous_final_update_id,
        }
    }

    #[test]
    fn test_binance_order_book_new() {
        let order_book = BinanceOrderBook::new("BTCUSDT".to_string());
        assert_eq!(order_book.symbol, "BTCUSDT");
        assert!(order_book.book.is_none());
        assert_eq!(order_book.counter, 0);
        assert!(order_book.needs_snapshot());
    }

    #[test]
    fn test_binance_order_book_reset() {
        let mut order_book = BinanceOrderBook::new("BTCUSDT".to_string());
        order_book.set_snapshot(create_test_snapshot());
        order_book.counter = 1000;

        order_book.reset();

        assert!(order_book.book.is_none());
        assert_eq!(order_book.counter, 0);
        assert!(order_book.needs_snapshot());
    }

    #[test]
    fn test_binance_order_book_wrong_symbol() {
        let mut order_book = BinanceOrderBook::new("BTCUSDT".to_string());
        order_book.set_snapshot(create_test_snapshot());

        let eth_msg = create_test_diff_message("ETHUSDT", 1001, 1001, 1000);

        let result = order_book.new_update_diff(&eth_msg);
        assert!(result.is_err());
        if let Err(LocalOrderBookError::WrongSymbol(expected, received)) = result {
            assert_eq!(expected, "BTCUSDT");
            assert_eq!(received, "ETHUSDT");
        } else {
            panic!("Expected WrongSymbol error");
        }
    }

    #[test]
    fn test_binance_order_book_builds_from_snapshot() {
        let mut order_book = BinanceOrderBook::new("BTCUSDT".to_string());

        // First diff arrives, triggers need for snapshot
        let diff1 = create_test_diff_message("BTCUSDT", 999, 1000, 998);
        order_book.new_update_diff(&diff1).unwrap();
        assert!(order_book.needs_snapshot());

        // Snapshot arrives
        order_book.set_snapshot(create_test_snapshot());

        // Next diff builds the book
        let diff2 = create_test_diff_message("BTCUSDT", 1001, 1002, 1000);
        order_book.new_update_diff(&diff2).unwrap();

        assert!(order_book.book.is_some());
        assert!(!order_book.needs_snapshot());
    }

    #[test]
    fn test_binance_order_book_sequence_validation() {
        let mut order_book = BinanceOrderBook::new("BTCUSDT".to_string());

        // Setup with snapshot
        let diff1 = create_test_diff_message("BTCUSDT", 999, 1000, 998);
        order_book.new_update_diff(&diff1).unwrap();
        order_book.set_snapshot(create_test_snapshot());
        let diff2 = create_test_diff_message("BTCUSDT", 1001, 1002, 1000);
        order_book.new_update_diff(&diff2).unwrap();

        assert!(order_book.book.is_some());
        let counter_after_build = order_book.counter;

        // Valid sequence update
        let diff3 = create_test_diff_message("BTCUSDT", 1003, 1004, counter_after_build);
        order_book.new_update_diff(&diff3).unwrap();
        assert!(order_book.book.is_some());

        // Out of sequence update should reset
        let diff_bad = create_test_diff_message("BTCUSDT", 2000, 2001, 1999);
        order_book.new_update_diff(&diff_bad).unwrap();
        assert!(order_book.book.is_none());
        assert!(order_book.needs_snapshot());
    }
}
