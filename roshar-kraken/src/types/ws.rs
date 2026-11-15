use serde::{Deserialize, Serialize};

// Kraken Derivatives Book Structures
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KrakenBookLevel {
    pub price: f64,
    pub qty: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KrakenBookSnapshotMessage {
    pub feed: String,
    pub product_id: String,
    pub timestamp: i64,
    pub seq: i64,
    #[serde(rename = "tickSize")]
    pub tick_size: Option<String>,
    pub bids: Vec<KrakenBookLevel>,
    pub asks: Vec<KrakenBookLevel>,
}

impl KrakenBookSnapshotMessage {
    pub fn is_snapshot(&self) -> bool {
        true
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KrakenBookDeltaMessage {
    pub feed: String,
    pub product_id: String,
    pub timestamp: i64,
    pub seq: i64,
    pub side: String, // "buy" or "sell"
    pub price: f64,
    pub qty: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct KrakenSubscribeMessage {
    pub event: String,
    pub feed: String,
    pub product_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KrakenSingleTrade {
    pub feed: String,
    pub product_id: String,
    pub uid: String,
    pub side: String, // "buy" or "sell"
    #[serde(rename = "type")]
    pub type_field: String, // "fill", "liquidation", "termination", or "block"
    pub seq: i64,
    pub time: i64,
    pub qty: f64,
    pub price: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KrakenTradeSnapshotMessage {
    pub feed: String,
    pub product_id: String,
    pub trades: Vec<KrakenSingleTrade>,
}

// Use SingleTrade as TradeDeltaMessage to avoid duplication
pub type KrakenTradeDeltaMessage = KrakenSingleTrade;

// WebSocket Message Type
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct KrakenWssMessage {
    pub event: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feed: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product_ids: Option<Vec<String>>,
}

impl KrakenWssMessage {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("failed to serialize KrakenWssMessage")
    }

    pub fn ping() -> Self {
        Self {
            event: "ping".to_string(),
            feed: None,
            product_ids: None,
        }
    }

    pub fn depth(product_id: &str) -> Self {
        Self {
            event: "subscribe".to_string(),
            feed: Some("book".to_string()),
            product_ids: Some(vec![product_id.to_string()]),
        }
    }

    pub fn depth_unsub(product_id: &str) -> Self {
        Self {
            event: "unsubscribe".to_string(),
            feed: Some("book".to_string()),
            product_ids: Some(vec![product_id.to_string()]),
        }
    }

    pub fn trades(product_id: &str) -> Self {
        Self {
            event: "subscribe".to_string(),
            feed: Some("trade".to_string()),
            product_ids: Some(vec![product_id.to_string()]),
        }
    }

    pub fn trades_unsub(product_id: &str) -> Self {
        Self {
            event: "unsubscribe".to_string(),
            feed: Some("trade".to_string()),
            product_ids: Some(vec![product_id.to_string()]),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wss_message_ping() {
        let msg = KrakenWssMessage::ping();
        let json = msg.to_json();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["event"], "ping");
    }

    #[test]
    fn test_wss_message_depth() {
        let msg = KrakenWssMessage::depth("PI_XBTUSD");
        let json = msg.to_json();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["event"], "subscribe");
        assert_eq!(parsed["feed"], "book");
        assert_eq!(parsed["product_ids"][0], "PI_XBTUSD");
    }

    #[test]
    fn test_wss_message_trades() {
        let msg = KrakenWssMessage::trades("PI_XBTUSD");
        let json = msg.to_json();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["event"], "subscribe");
        assert_eq!(parsed["feed"], "trade");
        assert_eq!(parsed["product_ids"][0], "PI_XBTUSD");
    }
}
