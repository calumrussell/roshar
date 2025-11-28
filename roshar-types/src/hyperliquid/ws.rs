use chrono::DateTime;
use serde::{Deserialize, Serialize};

use crate::{LocalOrderBook, Trade, Venue};

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

impl HyperliquidBookMessage {
    pub fn to_local_order_book(&self) -> LocalOrderBook {
        use ordered_float::OrderedFloat;
        use std::cmp::Reverse;

        let mut bids = std::collections::BTreeMap::new();
        let mut asks = std::collections::BTreeMap::new();

        for level in &self.data.levels[0] {
            if let Ok(px) = level.px.parse::<f64>() {
                bids.insert(Reverse(OrderedFloat(px)), level.sz.as_str().into());
            }
        }

        for level in &self.data.levels[1] {
            if let Ok(px) = level.px.parse::<f64>() {
                asks.insert(OrderedFloat(px), level.sz.as_str().into());
            }
        }

        LocalOrderBook {
            bids,
            asks,
            last_update: self.data.time as i64,
            last_update_ts: DateTime::from_timestamp_millis(self.data.time as i64)
                .unwrap_or_default(),
            exchange: Venue::Hyperliquid.to_string().into(),
            coin: self.data.coin.as_str().into(),
        }
    }

    pub fn to_depth_updates(&self) -> Vec<crate::DepthUpdateData> {
        let mut res = Vec::new();

        if let Some(bids) = self.data.levels.first() {
            for bid in bids {
                res.push(crate::DepthUpdateData {
                    px: bid.px.clone(),
                    qty: bid.sz.clone(),
                    time: self.data.time,
                    time_ts: DateTime::from_timestamp_millis(self.data.time as i64)
                        .unwrap_or_default(),
                    ticker: self.data.coin.clone(),
                    meta: format!("{{\"n\": {}}}", bid.n),
                    side: false,
                    venue: Venue::Hyperliquid,
                });
            }
        }

        if let Some(asks) = self.data.levels.get(1) {
            for ask in asks {
                res.push(crate::DepthUpdateData {
                    px: ask.px.clone(),
                    qty: ask.sz.clone(),
                    time: self.data.time,
                    time_ts: DateTime::from_timestamp_millis(self.data.time as i64)
                        .unwrap_or_default(),
                    ticker: self.data.coin.clone(),
                    meta: format!("{{\"n\": {}}}", ask.n),
                    side: true,
                    venue: Venue::Hyperliquid,
                });
            }
        }

        res
    }
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

impl HyperliquidTradesMessage {
    pub fn to_trades(&self) -> Vec<Trade> {
        let mut vals = Vec::with_capacity(self.data.len());

        for trade in &self.data {
            let px = match trade.px.parse::<f64>() {
                Ok(price) => price,
                Err(_) => continue,
            };

            let sz = match trade.sz.parse::<f64>() {
                Ok(size) => size,
                Err(_) => continue,
            };

            vals.push(Trade {
                time: trade.time as i64,
                exchange: Venue::Hyperliquid.to_string(),
                side: trade.side == "A",
                coin: trade.coin.clone(),
                px,
                sz,
            });
        }
        vals
    }

    pub fn to_trade_data(&self) -> Vec<crate::TradeData> {
        self.data
            .iter()
            .map(|trade| crate::TradeData {
                px: trade.px.clone(),
                qty: trade.sz.clone(),
                time: trade.time,
                time_ts: DateTime::from_timestamp_millis(trade.time as i64).unwrap_or_default(),
                ticker: trade.coin.clone(),
                meta: format!(
                    "{{\"tid\": {}, \"hash\": \"{}\", \"users\": {:?}}}",
                    trade.tid, trade.hash, trade.users
                ),
                side: trade.side == "A",
                venue: Venue::Hyperliquid,
            })
            .collect()
    }
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

impl HyperliquidCandleMessage {
    pub fn to_candle(&self) -> crate::Candle {
        crate::Candle {
            open: self.data.o.clone(),
            high: self.data.h.clone(),
            low: self.data.l.clone(),
            close: self.data.c.clone(),
            volume: self.data.v.clone(),
            exchange: Venue::Hyperliquid.to_string(),
            time: DateTime::from_timestamp_millis(self.data.t as i64).unwrap_or_default(),
            close_time: DateTime::from_timestamp_millis(self.data.close_time as i64)
                .unwrap_or_default(),
            coin: self.data.s.clone(),
        }
    }
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

pub struct HlOrderBook {
    pub symbol: String,
    pub book: Option<LocalOrderBook>,
}

impl HlOrderBook {
    pub fn new(symbol: String) -> Self {
        Self { symbol, book: None }
    }

    pub fn new_message(
        &mut self,
        msg: &HyperliquidBookMessage,
    ) -> Result<(), crate::LocalOrderBookError> {
        if msg.data.coin != self.symbol {
            return Err(crate::LocalOrderBookError::WrongSymbol(
                self.symbol.clone(),
                msg.data.coin.clone(),
            ));
        }

        self.book = Some(msg.to_local_order_book());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

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

    #[test]
    fn test_book_message_to_local_order_book() {
        let book_msg = HyperliquidBookMessage {
            channel: "book".to_string(),
            data: HyperliquidBook {
                coin: "ETH".to_string(),
                levels: vec![
                    vec![HyperliquidBookLevel {
                        n: 1,
                        px: "2000.50".to_string(),
                        sz: "1.5".to_string(),
                    }],
                    vec![HyperliquidBookLevel {
                        n: 1,
                        px: "2001.00".to_string(),
                        sz: "2.0".to_string(),
                    }],
                ],
                time: 1640995200000,
            },
        };

        let local_book = book_msg.to_local_order_book();
        assert_eq!(local_book.test_coin(), "ETH");
        assert_eq!(local_book.test_exchange(), Venue::Hyperliquid.to_string());
        assert_eq!(local_book.test_bid_prices().len(), 1);
        assert_eq!(local_book.test_ask_prices().len(), 1);
        assert_eq!(local_book.test_bid_prices()[0], "2000.5");
        assert_eq!(local_book.test_ask_prices()[0], "2001");
    }

    #[test]
    fn test_hyperliquid_trades_message_to_trades() {
        let trades_msg = HyperliquidTradesMessage {
            channel: "trades".to_string(),
            data: vec![HyperliquidTrade {
                coin: "BTC".to_string(),
                hash: "abc123".to_string(),
                px: "50000.00".to_string(),
                side: "B".to_string(),
                sz: "0.1".to_string(),
                tid: 12345,
                time: 1640995200000,
                users: vec!["user1".to_string()],
            }],
        };

        let trades = trades_msg.to_trades();
        assert_eq!(trades.len(), 1);

        let trade = &trades[0];
        assert_eq!(trade.coin, "BTC");
        assert_eq!(trade.exchange, Venue::Hyperliquid.to_string());
        assert_eq!(trade.px, 50000.00);
        assert_eq!(trade.sz, 0.1);
        assert!(!trade.side); // "B" for buy = false in our side convention
        assert_eq!(trade.time, 1640995200000);
    }

    #[test]
    fn test_hl_order_book_new() {
        let order_book = HlOrderBook::new("BTC".to_string());
        assert!(order_book.book.is_none());
    }

    #[test]
    fn test_hl_order_book_new_message() {
        let mut order_book = HlOrderBook::new("BTC".to_string());

        let book_msg = HyperliquidBookMessage {
            channel: "book".to_string(),
            data: HyperliquidBook {
                coin: "BTC".to_string(),
                levels: vec![
                    vec![HyperliquidBookLevel {
                        n: 1,
                        px: "50000.00".to_string(),
                        sz: "1.0".to_string(),
                    }],
                    vec![HyperliquidBookLevel {
                        n: 1,
                        px: "50100.00".to_string(),
                        sz: "0.5".to_string(),
                    }],
                ],
                time: 1640995200000,
            },
        };

        assert!(order_book.new_message(&book_msg).is_ok());
        assert!(order_book.book.is_some());

        let book = order_book.book.as_ref().unwrap();
        let bbo = book.get_bbo();
        assert_eq!(bbo.0, "50000");
        assert_eq!(bbo.1, "50100");
    }

    #[test]
    fn test_hl_order_book_wrong_symbol() {
        let mut order_book = HlOrderBook::new("BTC".to_string());

        let book_msg = HyperliquidBookMessage {
            channel: "book".to_string(),
            data: HyperliquidBook {
                coin: "ETH".to_string(),
                levels: vec![
                    vec![HyperliquidBookLevel {
                        n: 1,
                        px: "50000.00".to_string(),
                        sz: "1.0".to_string(),
                    }],
                    vec![HyperliquidBookLevel {
                        n: 1,
                        px: "50100.00".to_string(),
                        sz: "0.5".to_string(),
                    }],
                ],
                time: 1640995200000,
            },
        };

        let result = order_book.new_message(&book_msg);
        assert!(result.is_err());
        if let Err(crate::LocalOrderBookError::WrongSymbol(expected, received)) = result {
            assert_eq!(expected, "BTC");
            assert_eq!(received, "ETH");
        } else {
            panic!("Expected WrongSymbol error");
        }
    }
}
