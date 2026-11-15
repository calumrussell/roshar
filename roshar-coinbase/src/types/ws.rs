use serde::{Deserialize, Serialize};

/// Subscribe/unsubscribe message for Coinbase Advanced Trade WebSocket.
///
/// Coinbase requires a subscribe message within 5 seconds of connecting,
/// otherwise the connection is closed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoinbaseWssMessage {
    #[serde(rename = "type")]
    pub msg_type: String,
    pub channel: String,
    pub product_ids: Vec<String>,
}

impl CoinbaseWssMessage {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("failed to serialize CoinbaseWssMessage")
    }

    /// Subscribe to the level2 orderbook channel for a product.
    pub fn depth(product_id: &str) -> Self {
        Self {
            msg_type: "subscribe".to_string(),
            channel: "level2".to_string(),
            product_ids: vec![product_id.to_string()],
        }
    }

    pub fn depth_unsub(product_id: &str) -> Self {
        Self {
            msg_type: "unsubscribe".to_string(),
            channel: "level2".to_string(),
            product_ids: vec![product_id.to_string()],
        }
    }

    /// Subscribe to the market_trades channel for a product.
    pub fn trades(product_id: &str) -> Self {
        Self {
            msg_type: "subscribe".to_string(),
            channel: "market_trades".to_string(),
            product_ids: vec![product_id.to_string()],
        }
    }

    pub fn trades_unsub(product_id: &str) -> Self {
        Self {
            msg_type: "unsubscribe".to_string(),
            channel: "market_trades".to_string(),
            product_ids: vec![product_id.to_string()],
        }
    }

    /// Subscribe to the heartbeats channel to keep connections alive when
    /// market data is sparse. The product_ids list can be empty.
    pub fn heartbeats(product_ids: &[&str]) -> Self {
        Self {
            msg_type: "subscribe".to_string(),
            channel: "heartbeats".to_string(),
            product_ids: product_ids.iter().map(|s| s.to_string()).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_depth_subscribe() {
        let msg = CoinbaseWssMessage::depth("BTC-USD");
        let json = msg.to_json();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["type"], "subscribe");
        assert_eq!(parsed["channel"], "level2");
        assert_eq!(parsed["product_ids"][0], "BTC-USD");
    }

    #[test]
    fn test_trades_subscribe() {
        let msg = CoinbaseWssMessage::trades("BTC-USD");
        let json = msg.to_json();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["type"], "subscribe");
        assert_eq!(parsed["channel"], "market_trades");
        assert_eq!(parsed["product_ids"][0], "BTC-USD");
    }
}
