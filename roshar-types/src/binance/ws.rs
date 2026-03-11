use serde::{Deserialize, Serialize};

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

    pub fn depth_unsub(symbol: &str) -> Self {
        Self {
            id: 1,
            method: "UNSUBSCRIBE".to_string(),
            params: Some(vec![format!("{}@depth@100ms", symbol.to_lowercase())]),
        }
    }

    pub fn trades_unsub(symbol: &str) -> Self {
        Self {
            id: 1,
            method: "UNSUBSCRIBE".to_string(),
            params: Some(vec![format!("{}@trade", symbol.to_lowercase())]),
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

    pub fn batch_candles(symbols: &[String]) -> Self {
        let params: Vec<String> = symbols
            .iter()
            .map(|symbol| format!("{}@kline_1m", symbol.to_lowercase()))
            .collect();
        Self {
            id: 1,
            method: "SUBSCRIBE".to_string(),
            params: Some(params),
        }
    }

    /// Subscribe to multiple streams in a single message
    /// streams should be in format like "btcusdt@depth@100ms", "ethusdt@trade", etc.
    pub fn batch_subscribe(streams: Vec<String>) -> Self {
        Self {
            id: 1,
            method: "SUBSCRIBE".to_string(),
            params: Some(streams),
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

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct BinanceOrderBookSnapshot {
    #[serde(rename = "lastUpdateId")]
    pub last_update_id: u64,
    pub bids: Vec<[String; 2]>,
    pub asks: Vec<[String; 2]>,
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
    fn test_wss_message_batch_depth() {
        let msg = BinanceWssMessage::batch_depth(&["BTCUSDT".to_string(), "ETHUSDT".to_string()]);
        let json = msg.to_json();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["method"], "SUBSCRIBE");
        assert_eq!(parsed["params"][0], "btcusdt@depth@100ms");
        assert_eq!(parsed["params"][1], "ethusdt@depth@100ms");
    }

    #[test]
    fn test_wss_message_batch_trades() {
        let msg = BinanceWssMessage::batch_trades(&["BTCUSDT".to_string(), "ETHUSDT".to_string()]);
        let json = msg.to_json();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["method"], "SUBSCRIBE");
        assert_eq!(parsed["params"][0], "btcusdt@trade");
        assert_eq!(parsed["params"][1], "ethusdt@trade");
    }

    #[test]
    fn test_wss_message_batch_candles() {
        let msg = BinanceWssMessage::batch_candles(&["BTCUSDT".to_string(), "ETHUSDT".to_string()]);
        let json = msg.to_json();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["method"], "SUBSCRIBE");
        assert_eq!(parsed["params"][0], "btcusdt@kline_1m");
        assert_eq!(parsed["params"][1], "ethusdt@kline_1m");
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
}
