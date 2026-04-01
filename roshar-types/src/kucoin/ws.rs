use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KuCoinWssMessage {
    pub id: String,
    #[serde(rename = "type")]
    pub msg_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topic: Option<String>,
    #[serde(rename = "privateChannel", skip_serializing_if = "Option::is_none")]
    pub private_channel: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response: Option<bool>,
}

impl KuCoinWssMessage {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("failed to serialize KuCoinWssMessage")
    }

    pub fn ping() -> Self {
        Self {
            id: "1".to_string(),
            msg_type: "ping".to_string(),
            topic: None,
            private_channel: None,
            response: None,
        }
    }

    pub fn depth(symbol: &str) -> Self {
        Self {
            id: "1".to_string(),
            msg_type: "subscribe".to_string(),
            topic: Some(format!("/contractMarket/level2:{}", symbol)),
            private_channel: Some(false),
            response: Some(true),
        }
    }

    pub fn depth_unsub(symbol: &str) -> Self {
        Self {
            id: "1".to_string(),
            msg_type: "unsubscribe".to_string(),
            topic: Some(format!("/contractMarket/level2:{}", symbol)),
            private_channel: Some(false),
            response: Some(true),
        }
    }

    pub fn trades(symbol: &str) -> Self {
        Self {
            id: "1".to_string(),
            msg_type: "subscribe".to_string(),
            topic: Some(format!("/contractMarket/execution:{}", symbol)),
            private_channel: Some(false),
            response: Some(true),
        }
    }

    pub fn trades_unsub(symbol: &str) -> Self {
        Self {
            id: "1".to_string(),
            msg_type: "unsubscribe".to_string(),
            topic: Some(format!("/contractMarket/execution:{}", symbol)),
            private_channel: Some(false),
            response: Some(true),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ping_serializes_correctly() {
        let msg = KuCoinWssMessage::ping();
        let json = msg.to_json();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["type"], "ping");
        assert_eq!(parsed["id"], "1");
        assert!(parsed.get("topic").is_none());
    }

    #[test]
    fn test_depth_subscribe() {
        let msg = KuCoinWssMessage::depth("XBTUSDTM");
        let json = msg.to_json();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["type"], "subscribe");
        assert_eq!(parsed["topic"], "/contractMarket/level2:XBTUSDTM");
        assert_eq!(parsed["privateChannel"], false);
        assert_eq!(parsed["response"], true);
    }

    #[test]
    fn test_trades_subscribe() {
        let msg = KuCoinWssMessage::trades("XBTUSDTM");
        let json = msg.to_json();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["type"], "subscribe");
        assert_eq!(parsed["topic"], "/contractMarket/execution:XBTUSDTM");
    }
}
