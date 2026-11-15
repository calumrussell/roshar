use serde::{Deserialize, Serialize};

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
        assert_eq!(msg.data.len(), 1);
        assert_eq!(msg.data[0].inst_id, "BTC-USDT-SWAP");
        assert_eq!(msg.data[0].px, "50000.5");
        assert_eq!(msg.data[0].sz, "1");
        assert_eq!(msg.data[0].side, "buy");
    }
}
