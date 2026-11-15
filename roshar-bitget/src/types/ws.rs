use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BitgetWssArg {
    #[serde(rename = "instType")]
    pub inst_type: String,
    pub channel: String,
    #[serde(rename = "instId")]
    pub inst_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BitgetWssMessage {
    pub op: String,
    pub args: Vec<BitgetWssArg>,
}

impl BitgetWssMessage {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("failed to serialize BitgetWssMessage")
    }

    /// Bitget uses a plain text "ping" (server responds with "pong")
    pub fn ping() -> String {
        "ping".to_string()
    }

    /// Full orderbook (books channel) for a USDT-margined perpetual
    pub fn depth(symbol: &str) -> Self {
        Self {
            op: "subscribe".to_string(),
            args: vec![BitgetWssArg {
                inst_type: "USDT-FUTURES".to_string(),
                channel: "books".to_string(),
                inst_id: symbol.to_string(),
            }],
        }
    }

    pub fn depth_unsub(symbol: &str) -> Self {
        Self {
            op: "unsubscribe".to_string(),
            args: vec![BitgetWssArg {
                inst_type: "USDT-FUTURES".to_string(),
                channel: "books".to_string(),
                inst_id: symbol.to_string(),
            }],
        }
    }

    pub fn trades(symbol: &str) -> Self {
        Self {
            op: "subscribe".to_string(),
            args: vec![BitgetWssArg {
                inst_type: "USDT-FUTURES".to_string(),
                channel: "trade".to_string(),
                inst_id: symbol.to_string(),
            }],
        }
    }

    pub fn trades_unsub(symbol: &str) -> Self {
        Self {
            op: "unsubscribe".to_string(),
            args: vec![BitgetWssArg {
                inst_type: "USDT-FUTURES".to_string(),
                channel: "trade".to_string(),
                inst_id: symbol.to_string(),
            }],
        }
    }

    pub fn tickers(symbol: &str) -> Self {
        Self {
            op: "subscribe".to_string(),
            args: vec![BitgetWssArg {
                inst_type: "USDT-FUTURES".to_string(),
                channel: "ticker".to_string(),
                inst_id: symbol.to_string(),
            }],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ping_is_text() {
        assert_eq!(BitgetWssMessage::ping(), "ping");
    }

    #[test]
    fn test_depth_subscribe() {
        let msg = BitgetWssMessage::depth("BTCUSDT");
        let json = msg.to_json();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["op"], "subscribe");
        assert_eq!(parsed["args"][0]["channel"], "books");
        assert_eq!(parsed["args"][0]["instId"], "BTCUSDT");
        assert_eq!(parsed["args"][0]["instType"], "USDT-FUTURES");
    }

    #[test]
    fn test_trades_subscribe() {
        let msg = BitgetWssMessage::trades("BTCUSDT");
        let json = msg.to_json();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["op"], "subscribe");
        assert_eq!(parsed["args"][0]["channel"], "trade");
        assert_eq!(parsed["args"][0]["instId"], "BTCUSDT");
    }
}
