use anyhow::anyhow;
use chrono::DateTime;
use log::error;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::{Candle, DepthUpdate, LocalOrderBook, LocalOrderBookError, Trade, Venue};

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

// WebSocket API
pub struct WssApi;

impl WssApi {
    pub fn ping() -> String {
        serde_json::to_string(&ByBitMessage {
            req_id: "100001".to_string(),
            op: "ping".to_string(),
            args: None,
            ret_msg: None,
            success: None,
        })
        .expect("Invalid ping")
    }

    pub fn depth(coin: &str) -> String {
        let subscribe = ByBitMessage {
            req_id: "test".to_string(),
            op: "subscribe".to_string(),
            args: Some(vec![format!("orderbook.50.{coin}")]),
            ret_msg: None,
            success: None,
        };
        serde_json::to_string(&subscribe).expect("invalid depth subscription")
    }

    pub fn depth_unsub(coin: &str) -> String {
        let subscribe = ByBitMessage {
            req_id: "test".to_string(),
            op: "unsubscribe".to_string(),
            args: Some(vec![format!("orderbook.50.{coin}")]),
            ret_msg: None,
            success: None,
        };
        serde_json::to_string(&subscribe).expect("invalid depth subscription")
    }

    pub fn trades(coin: &str) -> String {
        let subscribe = ByBitMessage {
            req_id: "test".to_string(),
            op: "subscribe".to_string(),
            args: Some(vec![format!("publicTrade.{}", coin)]),
            ret_msg: None,
            success: None,
        };
        serde_json::to_string(&subscribe).expect("invalid trades subscription")
    }

    pub fn trades_unsub(coin: &str) -> String {
        let subscribe = ByBitMessage {
            req_id: "test".to_string(),
            op: "unsubscribe".to_string(),
            args: Some(vec![format!("publicTrade.{}", coin)]),
            ret_msg: None,
            success: None,
        };
        serde_json::to_string(&subscribe).expect("invalid trades subscription")
    }

    pub fn candle(coin: &str) -> String {
        let subscribe = ByBitMessage {
            req_id: "test".to_string(),
            op: "subscribe".to_string(),
            args: Some(vec![format!("kline.1.{}", coin)]),
            ret_msg: None,
            success: None,
        };
        serde_json::to_string(&subscribe).expect("invalid candle subscription")
    }
}

// Conversion implementations

impl From<ByBitTradesMessage> for Vec<Trade> {
    fn from(value: ByBitTradesMessage) -> Self {
        let mut vals = vec![];

        for trade in value.data {
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
}

// New From implementations for daily partitioned tables
impl From<ByBitTradesMessage> for Vec<crate::TradeData> {
    fn from(value: ByBitTradesMessage) -> Self {
        value
            .data
            .into_iter()
            .map(|trade| crate::TradeData {
                px: trade.p,
                qty: trade.v,
                time: trade.trade_time,
                time_ts: DateTime::from_timestamp_millis(trade.trade_time as i64)
                    .unwrap_or_default(),
                ticker: trade.s,
                meta: format!(
                    "{{\"i\": \"{}\", \"L\": \"{}\", \"BT\": {}, \"RPI\": {}}}",
                    trade.i,
                    trade.tick_direction.unwrap_or("None".to_string()),
                    trade.is_block_trade,
                    trade.is_rpi_trade.unwrap_or(false)
                ),
                side: trade.side == "Sell", // true for sell, false for buy
                venue: Venue::ByBit,
            })
            .collect()
    }
}

impl From<ByBitDepthMessage> for Vec<crate::DepthUpdateData> {
    fn from(value: ByBitDepthMessage) -> Self {
        let mut res = Vec::new();

        for bid in &value.data.b {
            res.push(crate::DepthUpdateData {
                px: bid[0].clone(),
                qty: bid[1].clone(),
                time: value.ts,
                time_ts: DateTime::from_timestamp_millis(value.ts as i64).unwrap_or_default(),
                ticker: value.data.s.clone(),
                meta: format!(
                    "{{\"u\": {}, \"cts\": {}, \"seq\": {}}}",
                    value.data.u, value.cts, value.data.seq
                ),
                side: false, // bid side
                venue: Venue::ByBit,
            });
        }

        for ask in &value.data.a {
            res.push(crate::DepthUpdateData {
                px: ask[0].clone(),
                qty: ask[1].clone(),
                time: value.ts,
                time_ts: DateTime::from_timestamp_millis(value.ts as i64).unwrap_or_default(),
                ticker: value.data.s.clone(),
                meta: format!(
                    "{{\"u\": {}, \"cts\": {}, \"seq\": {}}}",
                    value.data.u, value.cts, value.data.seq
                ),
                side: true, // ask side
                venue: Venue::ByBit,
            });
        }

        res
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

impl From<ByBitDepthMessage> for LocalOrderBook {
    fn from(value: ByBitDepthMessage) -> Self {
        use ordered_float::OrderedFloat;
        use std::cmp::Reverse;

        let mut bids = BTreeMap::new();
        let mut asks = BTreeMap::new();

        for level in value.data.b {
            let formatted_px = remove_trailing_zeros(level.first().unwrap());
            let formatted_sz = level.get(1).unwrap().to_string();

            if let Ok(price) = formatted_px.parse::<f64>() {
                bids.insert(Reverse(OrderedFloat(price)), formatted_sz.as_str().into());
            }
        }

        for level in value.data.a {
            let formatted_px = remove_trailing_zeros(level.first().unwrap());
            let formatted_sz = level.get(1).unwrap().to_string();

            if let Ok(price) = formatted_px.parse::<f64>() {
                asks.insert(OrderedFloat(price), formatted_sz.as_str().into());
            }
        }

        let coin = value.data.s;

        Self {
            bids,
            asks,
            last_update: value.ts as i64,
            last_update_ts: DateTime::from_timestamp_millis(value.ts as i64).unwrap_or_default(),
            exchange: Venue::ByBit.to_string().into(),
            coin: coin.as_str().into(),
        }
    }
}

impl TryFrom<ByBitDepthMessage> for Vec<DepthUpdate> {
    type Error = anyhow::Error;

    fn try_from(value: ByBitDepthMessage) -> std::result::Result<Self, Self::Error> {
        let mut vals = vec![];

        if value.snapshot_type != "delta" {
            return Err(anyhow!("Not a partial update"));
        }

        let exchange_str = Venue::ByBit.as_str();
        let coin_str = value.data.s.as_str();

        for bid in &value.data.b {
            let internal = DepthUpdate {
                time: value.ts as i64,
                exchange: exchange_str.to_string(),
                side: false,
                coin: coin_str.to_string(),
                px: bid.first().unwrap().parse::<f64>().unwrap(),
                sz: bid.get(1).unwrap().parse::<f64>().unwrap(),
            };
            vals.push(internal);
        }

        for ask in &value.data.a {
            let internal = DepthUpdate {
                time: value.ts as i64,
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
}

// Candle conversion
impl From<ByBitCandleMessage> for Vec<Candle> {
    fn from(value: ByBitCandleMessage) -> Self {
        let mut candles = Vec::new();

        for candle_data in value.data {
            let candle = Candle {
                open: candle_data.open,
                high: candle_data.high,
                low: candle_data.low,
                close: candle_data.close,
                volume: candle_data.volume,
                exchange: Venue::ByBit.to_string(),
                time: DateTime::from_timestamp_millis(candle_data.start_time as i64)
                    .unwrap_or_default(),
                close_time: DateTime::from_timestamp_millis(candle_data.end_time as i64)
                    .unwrap_or_default(),
                coin: value
                    .topic
                    .split('.')
                    .nth(2)
                    .unwrap_or("unknown")
                    .to_string(),
            };
            candles.push(candle);
        }

        candles
    }
}

// Order book management
pub struct BybitOrderBook {
    pub symbol: String,
    pub book: Option<LocalOrderBook>,
}

impl BybitOrderBook {
    pub fn new(symbol: String) -> Self {
        Self { symbol, book: None }
    }

    pub fn new_update(&mut self, msg: ByBitDepthMessage) -> Result<(), LocalOrderBookError> {
        let coin = msg.data.s.clone();

        // Validate symbol
        if coin != self.symbol {
            return Err(LocalOrderBookError::WrongSymbol(self.symbol.clone(), coin));
        }

        if msg.is_full_update() {
            let local: LocalOrderBook = msg.clone().into();
            self.book = Some(local);
            return Ok(());
        }

        if let Some(ref mut book) = self.book {
            let updates: Result<Vec<DepthUpdate>, _> = msg.try_into();
            if let Ok(updates) = updates {
                book.apply_updates(&updates, 50);
            }
        } else {
            return Err(LocalOrderBookError::BookUpdateBeforeSnapshot(
                Venue::ByBit.to_string(),
                coin,
            ));
        }

        let validation_result = if let Some(ref book) = self.book {
            let (bid, ask) = book.get_bbo();
            let bid_dec: Result<Decimal, LocalOrderBookError> = match bid.parse() {
                Ok(val) => Ok(val),
                Err(_) => {
                    error!("Bybit unparseable bid: {:?}", bid);
                    Err(LocalOrderBookError::UnparseableInputs(
                        Venue::ByBit.to_string(),
                        coin.clone(),
                    ))
                }
            };
            let ask_dec: Result<Decimal, LocalOrderBookError> = match ask.parse() {
                Ok(val) => Ok(val),
                Err(_) => {
                    error!("Bybit unparseable ask: {:?}", ask);
                    Err(LocalOrderBookError::UnparseableInputs(
                        Venue::ByBit.to_string(),
                        coin.clone(),
                    ))
                }
            };

            if let (Ok(bid), Ok(ask)) = (&bid_dec, &ask_dec) {
                if bid > ask {
                    Err(LocalOrderBookError::BidAboveAsk(
                        bid.to_string(),
                        ask.to_string(),
                        Venue::ByBit.to_string(),
                        coin.clone(),
                    ))
                } else {
                    Ok(())
                }
            } else if bid_dec.is_err() {
                bid_dec.map(|_| ())
            } else {
                ask_dec.map(|_| ())
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
        // Test that is_full_update() correctly identifies snapshot vs delta messages
        let snapshot_msg = create_test_snapshot_message("BTCUSDT");
        let delta_msg = create_test_delta_message("BTCUSDT");

        assert!(snapshot_msg.is_full_update());
        assert!(!delta_msg.is_full_update());
    }

    #[tokio::test]
    async fn test_bybit_order_book_snapshot_processing() {
        // Test that snapshot messages create new order books in the BybitOrderBook
        let mut order_book = BybitOrderBook::new("BTCUSDT".to_string());
        let snapshot_msg = create_test_snapshot_message("BTCUSDT");

        let result = order_book.new_update(snapshot_msg);
        assert!(result.is_ok());
        assert!(order_book.book.is_some());

        let book = order_book.book.as_ref().unwrap();
        assert_eq!(book.test_bid_prices().len(), 2);
        assert_eq!(book.test_ask_prices().len(), 2);
    }

    #[tokio::test]
    async fn test_bybit_order_book_delta_update_on_existing_book() {
        // Test that delta messages update existing order books
        let mut order_book = BybitOrderBook::new("BTCUSDT".to_string());
        let snapshot_msg = create_test_snapshot_message("BTCUSDT");
        let delta_msg = create_test_delta_message("BTCUSDT");

        order_book.new_update(snapshot_msg).unwrap();

        let result = order_book.new_update(delta_msg);
        assert!(result.is_ok());
        assert!(order_book.book.is_some());
    }

    #[tokio::test]
    async fn test_bybit_order_book_delta_update_before_snapshot_error() {
        // Test that delta messages on non-existent symbols return BookUpdateBeforeSnapshot error
        let mut order_book = BybitOrderBook::new("BTCUSDT".to_string());
        let delta_msg = create_test_delta_message("BTCUSDT");

        let result = order_book.new_update(delta_msg);
        assert!(result.is_err());
        if let Err(LocalOrderBookError::BookUpdateBeforeSnapshot(exchange, coin)) = result {
            assert_eq!(exchange, Venue::ByBit.to_string());
            assert_eq!(coin, "BTCUSDT");
        } else {
            panic!("Expected BookUpdateBeforeSnapshot error");
        }
    }

    #[tokio::test]
    async fn test_bybit_order_book_wrong_symbol() {
        // Test that messages for wrong symbol return WrongSymbol error
        let mut order_book = BybitOrderBook::new("BTCUSDT".to_string());
        let eth_snapshot = create_test_snapshot_message("ETHUSDT");

        let result = order_book.new_update(eth_snapshot);
        assert!(result.is_err());
        if let Err(LocalOrderBookError::WrongSymbol(expected, received)) = result {
            assert_eq!(expected, "BTCUSDT");
            assert_eq!(received, "ETHUSDT");
        } else {
            panic!("Expected WrongSymbol error");
        }
    }

    #[tokio::test]
    async fn test_bybit_order_book_snapshot_overwrites_existing() {
        // Test that a new snapshot message overwrites an existing order book
        let mut order_book = BybitOrderBook::new("BTCUSDT".to_string());
        let snapshot1 = create_test_snapshot_message("BTCUSDT");
        let mut snapshot2 = create_test_snapshot_message("BTCUSDT");

        snapshot2.data.b = vec![["51000.0".to_string(), "5.0".to_string()]];

        order_book.new_update(snapshot1).unwrap();
        // Verify first snapshot created the book
        assert!(order_book.book.is_some());

        order_book.new_update(snapshot2).unwrap();
        let book_after = order_book.book.as_ref().unwrap();

        // Verify the second snapshot overwrote the first one
        assert_eq!(book_after.test_bid_prices()[0], "51000");
    }
}
