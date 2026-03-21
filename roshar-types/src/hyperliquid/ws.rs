use serde::{Deserialize, Serialize};

// Hyperliquid Book Structures
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HyperliquidBookLevel {
    pub n: u32,
    pub px: String,
    pub sz: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HyperliquidBook {
    pub coin: String,
    pub levels: Vec<Vec<HyperliquidBookLevel>>,
    pub time: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HyperliquidBookMessage {
    pub channel: String,
    pub data: HyperliquidBook,
}

// Hyperliquid Trade Structures
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HyperliquidTrade {
    pub coin: String,
    pub hash: String,
    pub px: String,         // Price as a string to preserve precision
    pub side: String,       // "B" for buy, "A" for sell
    pub sz: String,         // Size as a string to preserve precision
    pub tid: u64,           // Trade ID
    pub time: u64,          // Trade timestamp in milliseconds
    pub users: Vec<String>, // List of users involved in the trade
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HyperliquidTradesMessage {
    pub channel: String,
    pub data: Vec<HyperliquidTrade>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HyperliquidCandleData {
    #[serde(rename = "T")]
    pub close_time: u64, // Close time (epoch in millis)
    pub c: String, // Close price
    pub h: String, // High price
    pub i: String, // Interval (e.g., "1m")
    pub l: String, // Low price
    pub n: u32,    // Number of trades
    pub o: String, // Open price
    pub s: String, // Symbol (e.g., "ETH")
    pub t: u64,    // Open time (epoch in millis)
    pub v: String, // Volume
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HyperliquidCandleMessage {
    pub channel: String,
    pub data: HyperliquidCandleData,
}

// Hyperliquid User Fills Structures
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct HyperliquidUserFill {
    pub coin: String,
    pub px: String,
    pub sz: String,
    pub side: String,
    pub time: u64,
    pub start_position: String,
    pub dir: String,
    pub closed_pnl: String,
    pub hash: String,
    pub oid: u64,
    pub crossed: bool,
    pub fee: String,
    pub tid: u64,
    pub fee_token: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub liquidation: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub builder_fee: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct HyperliquidUserFillsData {
    #[serde(default)]
    pub is_snapshot: Option<bool>,
    pub user: String,
    pub fills: Vec<HyperliquidUserFill>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HyperliquidUserFillsMessage {
    pub channel: String,
    pub data: HyperliquidUserFillsData,
}

// Hyperliquid Order Updates Structures
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct WsBasicOrder {
    pub coin: String,
    pub side: String,
    #[serde(rename = "limitPx")]
    pub limit_px: String,
    pub sz: String,
    pub oid: u64,
    pub timestamp: u64,
    #[serde(rename = "origSz")]
    pub orig_sz: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cloid: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WsOrder {
    pub order: WsBasicOrder,
    pub status: String,
    #[serde(rename = "statusTimestamp")]
    pub status_timestamp: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HyperliquidOrderUpdatesMessage {
    pub channel: String,
    pub data: Vec<WsOrder>,
}

// Hyperliquid BBO Structures
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WsLevel {
    pub px: String,
    pub sz: String,
    pub n: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HyperliquidBbo {
    pub coin: String,
    pub time: u64,
    pub bbo: [Option<WsLevel>; 2], // [bid, ask]
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HyperliquidBboMessage {
    pub channel: String,
    pub data: HyperliquidBbo,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct HyperliquidWssSubscription {
    #[serde(rename = "type")]
    pub typ: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interval: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coin: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
}

// Hyperliquid WebSocket Message Types
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct HyperliquidWssMessage {
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subscription: Option<HyperliquidWssSubscription>,
}

impl HyperliquidWssMessage {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("failed to serialize HyperliquidWssMessage")
    }

    pub fn ping() -> Self {
        Self {
            method: "ping".to_string(),
            subscription: None,
        }
    }

    pub fn all_mids() -> Self {
        Self {
            method: "subscribe".to_string(),
            subscription: Some(HyperliquidWssSubscription {
                typ: "allMids".into(),
                interval: None,
                coin: None,
                user: None,
            }),
        }
    }

    pub fn l2_book(coin: &str) -> Self {
        Self {
            method: "subscribe".to_string(),
            subscription: Some(HyperliquidWssSubscription {
                typ: "l2Book".into(),
                interval: None,
                coin: Some(coin.into()),
                user: None,
            }),
        }
    }

    pub fn l2_book_unsub(coin: &str) -> Self {
        Self {
            method: "unsubscribe".to_string(),
            subscription: Some(HyperliquidWssSubscription {
                typ: "l2Book".into(),
                interval: None,
                coin: Some(coin.into()),
                user: None,
            }),
        }
    }

    pub fn candle(coin: &str) -> Self {
        Self {
            method: "subscribe".to_string(),
            subscription: Some(HyperliquidWssSubscription {
                typ: "candle".into(),
                interval: Some("1m".into()),
                coin: Some(coin.into()),
                user: None,
            }),
        }
    }

    pub fn candle_unsub(coin: &str) -> Self {
        Self {
            method: "unsubscribe".to_string(),
            subscription: Some(HyperliquidWssSubscription {
                typ: "candle".into(),
                interval: Some("1m".into()),
                coin: Some(coin.into()),
                user: None,
            }),
        }
    }

    pub fn trades(coin: &str) -> Self {
        Self {
            method: "subscribe".to_string(),
            subscription: Some(HyperliquidWssSubscription {
                typ: "trades".into(),
                interval: None,
                coin: Some(coin.into()),
                user: None,
            }),
        }
    }

    pub fn trades_unsub(coin: &str) -> Self {
        Self {
            method: "unsubscribe".to_string(),
            subscription: Some(HyperliquidWssSubscription {
                typ: "trades".into(),
                interval: None,
                coin: Some(coin.into()),
                user: None,
            }),
        }
    }

    pub fn user_fills(user_address: &str) -> Self {
        Self {
            method: "subscribe".to_string(),
            subscription: Some(HyperliquidWssSubscription {
                typ: "userFills".into(),
                interval: None,
                coin: None,
                user: Some(user_address.into()),
            }),
        }
    }

    pub fn user_fills_unsub(user_address: &str) -> Self {
        Self {
            method: "unsubscribe".to_string(),
            subscription: Some(HyperliquidWssSubscription {
                typ: "userFills".into(),
                interval: None,
                coin: None,
                user: Some(user_address.into()),
            }),
        }
    }

    pub fn order_updates(user_address: &str) -> Self {
        Self {
            method: "subscribe".to_string(),
            subscription: Some(HyperliquidWssSubscription {
                typ: "orderUpdates".into(),
                interval: None,
                coin: None,
                user: Some(user_address.into()),
            }),
        }
    }

    pub fn order_updates_unsub(user_address: &str) -> Self {
        Self {
            method: "unsubscribe".to_string(),
            subscription: Some(HyperliquidWssSubscription {
                typ: "orderUpdates".into(),
                interval: None,
                coin: None,
                user: Some(user_address.into()),
            }),
        }
    }

    pub fn bbo(coin: &str) -> Self {
        Self {
            method: "subscribe".to_string(),
            subscription: Some(HyperliquidWssSubscription {
                typ: "bbo".into(),
                interval: None,
                coin: Some(coin.into()),
                user: None,
            }),
        }
    }

    pub fn bbo_unsub(coin: &str) -> Self {
        Self {
            method: "unsubscribe".to_string(),
            subscription: Some(HyperliquidWssSubscription {
                typ: "bbo".into(),
                interval: None,
                coin: Some(coin.into()),
                user: None,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wss_message_ping() {
        let msg = HyperliquidWssMessage::ping();
        let json = msg.to_json();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["method"], "ping");
        assert!(parsed["subscription"].is_null());
    }

    #[test]
    fn test_wss_message_candle() {
        let coin = "ETH";
        let msg = HyperliquidWssMessage::candle(coin);
        let json = msg.to_json();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["method"], "subscribe");
        assert_eq!(parsed["subscription"]["type"], "candle");
        assert_eq!(parsed["subscription"]["coin"], coin);
        assert_eq!(parsed["subscription"]["interval"], "1m");
    }

    #[test]
    fn test_wss_message_l2_book() {
        let coin = "BTC";
        let msg = HyperliquidWssMessage::l2_book(coin);
        let json = msg.to_json();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["method"], "subscribe");
        assert_eq!(parsed["subscription"]["type"], "l2Book");
        assert_eq!(parsed["subscription"]["coin"], coin);
        assert!(parsed["subscription"]["interval"].is_null());
    }

    #[test]
    fn test_wss_message_trades() {
        let coin = "SOL";
        let msg = HyperliquidWssMessage::trades(coin);
        let json = msg.to_json();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["method"], "subscribe");
        assert_eq!(parsed["subscription"]["type"], "trades");
        assert_eq!(parsed["subscription"]["coin"], coin);
        assert!(parsed["subscription"]["interval"].is_null());
    }

    #[test]
    fn test_wss_message_all_mids() {
        let msg = HyperliquidWssMessage::all_mids();
        assert_eq!(msg.method, "subscribe");
        let sub = msg.subscription.unwrap();
        assert_eq!(sub.typ, "allMids");
        assert!(sub.coin.is_none());
        assert!(sub.interval.is_none());
    }
}
