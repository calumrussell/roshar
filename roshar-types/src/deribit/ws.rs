use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeribitWssMessage {
    pub jsonrpc: String,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<DeribitParams>,
    pub id: u64,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(untagged)]
pub enum DeribitParams {
    Channels { channels: Vec<String> },
    Heartbeat { interval: u32 },
}

impl DeribitWssMessage {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("failed to serialize DeribitWssMessage")
    }

    pub fn subscribe(channels: &[String], id: u64) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            method: "public/subscribe".to_string(),
            params: Some(DeribitParams::Channels {
                channels: channels.to_vec(),
            }),
            id,
        }
    }

    pub fn unsubscribe(channels: &[String], id: u64) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            method: "public/unsubscribe".to_string(),
            params: Some(DeribitParams::Channels {
                channels: channels.to_vec(),
            }),
            id,
        }
    }

    pub fn set_heartbeat(interval: u32, id: u64) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            method: "public/set_heartbeat".to_string(),
            params: Some(DeribitParams::Heartbeat { interval }),
            id,
        }
    }

    pub fn test(id: u64) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            method: "public/test".to_string(),
            params: None,
            id,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeribitSubscriptionMessage {
    pub jsonrpc: String,
    pub method: String,
    pub params: DeribitSubscriptionParams,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeribitSubscriptionParams {
    pub channel: String,
    pub data: DeribitData,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(untagged)]
pub enum DeribitData {
    Book(DeribitOrderBookData),
    Trades(Vec<DeribitTradeData>),
    Ticker(DeribitTickerData),
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeribitOrderBookData {
    pub timestamp: u64,
    pub instrument_name: String,
    pub change_id: u64,
    pub prev_change_id: Option<u64>,
    pub bids: Vec<[serde_json::Value; 3]>, // ["new"|"change"|"delete", price, amount]
    pub asks: Vec<[serde_json::Value; 3]>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeribitTradeData {
    pub trade_seq: u64,
    pub trade_id: String,
    pub timestamp: u64,
    pub price: f64,
    pub amount: f64,
    pub direction: String,
    pub tick_direction: u8,
    pub instrument_name: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeribitTickerData {
    pub timestamp: u64,
    pub instrument_name: String,
    pub state: String,
    pub last_price: Option<f64>,
    pub index_price: f64,
    pub mark_price: f64,
    pub best_bid_price: Option<f64>,
    pub best_bid_amount: Option<f64>,
    pub best_ask_price: Option<f64>,
    pub best_ask_amount: Option<f64>,
    pub open_interest: f64,
    pub min_price: Option<f64>,
    pub max_price: Option<f64>,
    pub settlement_price: Option<f64>,
    pub funding_8h: Option<f64>,
    pub current_funding: Option<f64>,
    // Option specific fields
    pub bid_iv: Option<f64>,
    pub ask_iv: Option<f64>,
    pub mark_iv: Option<f64>,
    pub underlying_index: Option<String>,
    pub underlying_price: Option<f64>,
    pub interest_rate: Option<f64>,
    pub greeks: Option<DeribitGreeks>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeribitGreeks {
    pub delta: f64,
    pub gamma: f64,
    pub rho: f64,
    pub theta: f64,
    pub vega: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deribit_wss_message_subscribe() {
        let msg = DeribitWssMessage::subscribe(&["BTC-PERPETUAL".to_string()], 1);
        let json = msg.to_json();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["method"], "public/subscribe");
        assert_eq!(parsed["params"]["channels"][0], "BTC-PERPETUAL");
        assert_eq!(parsed["id"], 1);
    }

    #[test]
    fn test_deribit_wss_message_set_heartbeat() {
        let msg = DeribitWssMessage::set_heartbeat(30, 1);
        let json = msg.to_json();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["method"], "public/set_heartbeat");
        assert_eq!(parsed["params"]["interval"], 30);
    }

    #[test]
    fn test_deribit_subscription_orderbook_parsing() {
        let json_str = r#"{
            "jsonrpc": "2.0",
            "method": "subscription",
            "params": {
                "channel": "book.BTC-PERPETUAL.100ms",
                "data": {
                    "timestamp": 1535098298227,
                    "instrument_name": "BTC-PERPETUAL",
                    "change_id": 123457,
                    "prev_change_id": 123456,
                    "bids": [["new", 50000.0, 10.5]],
                    "asks": [["new", 50001.0, 8.3]]
                }
            }
        }"#;

        let msg: DeribitSubscriptionMessage = serde_json::from_str(json_str).unwrap();
        assert_eq!(msg.method, "subscription");
        if let DeribitData::Book(book) = msg.params.data {
            assert_eq!(book.instrument_name, "BTC-PERPETUAL");
            assert_eq!(book.bids.len(), 1);
            assert_eq!(book.bids[0][0], "new");
            assert_eq!(book.bids[0][1], 50000.0);
        } else {
            panic!("Expected Book data");
        }
    }

    #[test]
    fn test_deribit_subscription_trades_parsing() {
        let json_str = r#"{
            "jsonrpc": "2.0",
            "method": "subscription",
            "params": {
                "channel": "trades.BTC-PERPETUAL.100ms",
                "data": [
                    {
                        "trade_seq": 30289442,
                        "trade_id": "48079269",
                        "timestamp": 1590484512188,
                        "price": 8950.0,
                        "amount": 10.0,
                        "direction": "sell",
                        "tick_direction": 2,
                        "instrument_name": "BTC-PERPETUAL"
                    }
                ]
            }
        }"#;

        let msg: DeribitSubscriptionMessage = serde_json::from_str(json_str).unwrap();
        if let DeribitData::Trades(trades) = msg.params.data {
            assert_eq!(trades.len(), 1);
            assert_eq!(trades[0].instrument_name, "BTC-PERPETUAL");
            assert_eq!(trades[0].price, 8950.0);
        } else {
            panic!("Expected Trades data");
        }
    }

    #[test]
    fn test_deribit_subscription_ticker_parsing() {
        let json_str = r#"{
            "jsonrpc": "2.0",
            "method": "subscription",
            "params": {
                "channel": "ticker.BTC-27MAR26-60000-C.100ms",
                "data": {
                    "timestamp": 1535098298227,
                    "state": "open",
                    "instrument_name": "BTC-27MAR26-60000-C",
                    "index_price": 50000.0,
                    "mark_price": 1500.0,
                    "open_interest": 100.5,
                    "bid_iv": 60.5,
                    "ask_iv": 61.2,
                    "mark_iv": 60.8,
                    "greeks": {
                        "delta": 0.5,
                        "gamma": 0.0001,
                        "rho": 0.02,
                        "theta": -5.5,
                        "vega": 10.2
                    }
                }
            }
        }"#;

        let msg: DeribitSubscriptionMessage = serde_json::from_str(json_str).unwrap();
        if let DeribitData::Ticker(ticker) = msg.params.data {
            assert_eq!(ticker.instrument_name, "BTC-27MAR26-60000-C");
            assert_eq!(ticker.mark_iv, Some(60.8));
            let greeks = ticker.greeks.unwrap();
            assert_eq!(greeks.delta, 0.5);
            assert_eq!(greeks.theta, -5.5);
        } else {
            panic!("Expected Ticker data");
        }
    }
}
