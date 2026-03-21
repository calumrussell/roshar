use serde::{Deserialize, Serialize};

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bybit_depth_message_is_full_update() {
        let snapshot_msg = ByBitDepthMessage {
            topic: "orderbook.50.BTCUSDT".to_string(),
            snapshot_type: "snapshot".to_string(),
            ts: 1234567890,
            data: ByBitDepthBookData {
                s: "BTCUSDT".to_string(),
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
        };

        let delta_msg = ByBitDepthMessage {
            topic: "orderbook.50.BTCUSDT".to_string(),
            snapshot_type: "delta".to_string(),
            ts: 1234567892,
            data: ByBitDepthBookData {
                s: "BTCUSDT".to_string(),
                b: vec![["49998.0".to_string(), "3.0".to_string()]],
                a: vec![["50003.0".to_string(), "4.0".to_string()]],
                u: 1001,
                seq: 12346,
            },
            cts: 1234567893,
        };

        assert!(snapshot_msg.is_full_update());
        assert!(!delta_msg.is_full_update());
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
