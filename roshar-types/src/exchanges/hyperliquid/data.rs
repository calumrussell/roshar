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

// Hyperliquid API Request Structures
#[derive(Debug, Deserialize, Serialize)]
pub struct HyperliquidMessage {
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subscription: Option<HyperliquidSubscriptionType>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct HyperliquidSubscriptionType {
    #[serde(rename(serialize = "type"))]
    typ: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    interval: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    coin: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    user: Option<String>,
}

// Implementations
impl HyperliquidSubscriptionType {
    #[allow(dead_code)]
    pub fn all_mids() -> Self {
        Self {
            typ: "allMids".into(),
            interval: None,
            coin: None,
            user: None,
        }
    }

    pub fn l2_book(coin: &str) -> Self {
        Self {
            typ: "l2Book".into(),
            interval: None,
            coin: Some(coin.into()),
            user: None,
        }
    }

    pub fn candle(coin: &str) -> Self {
        Self {
            typ: "candle".into(),
            interval: Some("1m".into()),
            coin: Some(coin.into()),
            user: None,
        }
    }

    pub fn trades(coin: &str) -> Self {
        Self {
            typ: "trades".into(),
            interval: None,
            coin: Some(coin.into()),
            user: None,
        }
    }

    pub fn user_fills(user_address: &str) -> Self {
        Self {
            typ: "userFills".into(),
            interval: None,
            coin: None,
            user: Some(user_address.into()),
        }
    }

    pub fn order_updates(user_address: &str) -> Self {
        Self {
            typ: "orderUpdates".into(),
            interval: None,
            coin: None,
            user: Some(user_address.into()),
        }
    }

    pub fn bbo(coin: &str) -> Self {
        Self {
            typ: "bbo".into(),
            interval: None,
            coin: Some(coin.into()),
            user: None,
        }
    }
}

// Conversion implementations
impl From<HyperliquidBookMessage> for LocalOrderBook {
    fn from(value: HyperliquidBookMessage) -> Self {
        use ordered_float::OrderedFloat;
        use std::cmp::Reverse;

        let mut bids = std::collections::BTreeMap::new();
        let mut asks = std::collections::BTreeMap::new();

        for level in &value.data.levels[0] {
            if let Ok(px) = level.px.parse::<f64>() {
                bids.insert(Reverse(OrderedFloat(px)), level.sz.as_str().into());
            }
        }

        for level in &value.data.levels[1] {
            if let Ok(px) = level.px.parse::<f64>() {
                asks.insert(OrderedFloat(px), level.sz.as_str().into());
            }
        }

        Self {
            bids,
            asks,
            last_update: value.data.time as i64,
            last_update_ts: DateTime::from_timestamp_millis(value.data.time as i64)
                .unwrap_or_default(),
            exchange: Venue::Hyperliquid.to_string().into(),
            coin: value.data.coin.as_str().into(),
        }
    }
}

impl From<HyperliquidTradesMessage> for Vec<Trade> {
    fn from(value: HyperliquidTradesMessage) -> Self {
        let mut vals = vec![];

        for trade in value.data {
            // Parse price and size, skipping invalid trades
            let px = match trade.px.parse::<f64>() {
                Ok(price) => price,
                Err(_) => {
                    eprintln!("Warning: Skipping trade with invalid price: {}", trade.px);
                    continue;
                }
            };

            let sz = match trade.sz.parse::<f64>() {
                Ok(size) => size,
                Err(_) => {
                    eprintln!("Warning: Skipping trade with invalid size: {}", trade.sz);
                    continue;
                }
            };

            let internal = Trade {
                time: trade.time as i64,
                exchange: Venue::Hyperliquid.to_string(),
                side: trade.side.eq("A"),
                coin: trade.coin.clone(),
                px,
                sz,
            };
            vals.push(internal);
        }
        vals
    }
}

// CompressedData conversions

// New From implementations for daily partitioned tables
impl From<HyperliquidTradesMessage> for Vec<crate::TradeData> {
    fn from(value: HyperliquidTradesMessage) -> Self {
        value
            .data
            .into_iter()
            .map(|trade| crate::TradeData {
                px: trade.px,
                qty: trade.sz,
                time: trade.time,
                time_ts: DateTime::from_timestamp_millis(trade.time as i64).unwrap_or_default(),
                ticker: trade.coin,
                meta: format!(
                    "{{\"tid\": {}, \"hash\": \"{}\", \"users\": {:?}}}",
                    trade.tid, trade.hash, trade.users
                ),
                side: trade.side.eq("A"), // "A" for ask/sell, "B" for bid/buy
                venue: Venue::Hyperliquid,
            })
            .collect()
    }
}

impl From<HyperliquidBookMessage> for Vec<crate::DepthUpdateData> {
    fn from(value: HyperliquidBookMessage) -> Self {
        let mut res = Vec::new();

        // levels[0] = bids, levels[1] = asks
        if let Some(bids) = value.data.levels.first() {
            for bid in bids {
                res.push(crate::DepthUpdateData {
                    px: bid.px.clone(),
                    qty: bid.sz.clone(),
                    time: value.data.time,
                    time_ts: DateTime::from_timestamp_millis(value.data.time as i64)
                        .unwrap_or_default(),
                    ticker: value.data.coin.clone(),
                    meta: format!("{{\"n\": {}}}", bid.n),
                    side: false, // bid side
                    venue: Venue::Hyperliquid,
                });
            }
        }

        if let Some(asks) = value.data.levels.get(1) {
            for ask in asks {
                res.push(crate::DepthUpdateData {
                    px: ask.px.clone(),
                    qty: ask.sz.clone(),
                    time: value.data.time,
                    time_ts: DateTime::from_timestamp_millis(value.data.time as i64)
                        .unwrap_or_default(),
                    ticker: value.data.coin.clone(),
                    meta: format!("{{\"n\": {}}}", ask.n),
                    side: true, // ask side
                    venue: Venue::Hyperliquid,
                });
            }
        }

        res
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
        msg: HyperliquidBookMessage,
    ) -> Result<(), crate::LocalOrderBookError> {
        let coin = msg.data.coin.clone();

        // Validate symbol
        if coin != self.symbol {
            return Err(crate::LocalOrderBookError::WrongSymbol(
                self.symbol.clone(),
                coin,
            ));
        }

        let ob: LocalOrderBook = msg.into();
        self.book = Some(ob);
        Ok(())
    }
}

pub struct WssApi;

impl WssApi {
    pub fn ping() -> String {
        serde_json::to_string(&HyperliquidMessage {
            method: "ping".to_string(),
            subscription: None,
        })
        .expect("invalid pong")
    }

    pub fn candle(coin: &str) -> String {
        let subscribe = HyperliquidMessage {
            method: "subscribe".to_string(),
            subscription: Some(HyperliquidSubscriptionType::candle(coin)),
        };
        serde_json::to_string(&subscribe).expect("invalid candle subscription")
    }

    pub fn depth(coin: &str) -> String {
        let subscribe = HyperliquidMessage {
            method: "subscribe".to_string(),
            subscription: Some(HyperliquidSubscriptionType::l2_book(coin)),
        };
        serde_json::to_string(&subscribe).expect("invalid depth subscription")
    }

    pub fn depth_unsub(coin: &str) -> String {
        let subscribe = HyperliquidMessage {
            method: "unsubscribe".to_string(),
            subscription: Some(HyperliquidSubscriptionType::l2_book(coin)),
        };
        serde_json::to_string(&subscribe).expect("invalid depth subscription")
    }

    pub fn trades(coin: &str) -> String {
        let subscribe = HyperliquidMessage {
            method: "subscribe".to_string(),
            subscription: Some(HyperliquidSubscriptionType::trades(coin)),
        };
        serde_json::to_string(&subscribe).expect("invalid trades subscription")
    }

    pub fn trades_unsub(coin: &str) -> String {
        let subscribe = HyperliquidMessage {
            method: "unsubscribe".to_string(),
            subscription: Some(HyperliquidSubscriptionType::trades(coin)),
        };
        serde_json::to_string(&subscribe).expect("invalid trades subscription")
    }

    pub fn user_fills(user_address: &str) -> String {
        let subscribe = HyperliquidMessage {
            method: "subscribe".to_string(),
            subscription: Some(HyperliquidSubscriptionType::user_fills(user_address)),
        };
        serde_json::to_string(&subscribe).expect("invalid userFills subscription")
    }

    pub fn user_fills_unsub(user_address: &str) -> String {
        let subscribe = HyperliquidMessage {
            method: "unsubscribe".to_string(),
            subscription: Some(HyperliquidSubscriptionType::user_fills(user_address)),
        };
        serde_json::to_string(&subscribe).expect("invalid userFills unsubscription")
    }

    pub fn order_updates(user_address: &str) -> String {
        let subscribe = HyperliquidMessage {
            method: "subscribe".to_string(),
            subscription: Some(HyperliquidSubscriptionType::order_updates(user_address)),
        };
        serde_json::to_string(&subscribe).expect("invalid orderUpdates subscription")
    }

    pub fn order_updates_unsub(user_address: &str) -> String {
        let subscribe = HyperliquidMessage {
            method: "unsubscribe".to_string(),
            subscription: Some(HyperliquidSubscriptionType::order_updates(user_address)),
        };
        serde_json::to_string(&subscribe).expect("invalid orderUpdates unsubscription")
    }

    pub fn bbo(coin: &str) -> String {
        let subscribe = HyperliquidMessage {
            method: "subscribe".to_string(),
            subscription: Some(HyperliquidSubscriptionType::bbo(coin)),
        };
        serde_json::to_string(&subscribe).expect("invalid BBO subscription")
    }

    pub fn bbo_unsub(coin: &str) -> String {
        let subscribe = HyperliquidMessage {
            method: "unsubscribe".to_string(),
            subscription: Some(HyperliquidSubscriptionType::bbo(coin)),
        };
        serde_json::to_string(&subscribe).expect("invalid BBO unsubscription")
    }
}

impl From<HyperliquidCandleMessage> for crate::Candle {
    fn from(value: HyperliquidCandleMessage) -> Self {
        Self {
            open: value.data.o,
            high: value.data.h,
            low: value.data.l,
            close: value.data.c,
            volume: value.data.v,
            exchange: Venue::Hyperliquid.to_string(),
            time: DateTime::from_timestamp_millis(value.data.t as i64).unwrap_or_default(),
            close_time: DateTime::from_timestamp_millis(value.data.close_time as i64)
                .unwrap_or_default(),
            coin: value.data.s,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    #[test]
    fn test_wss_api_ping() {
        let ping_msg = WssApi::ping();
        let parsed: serde_json::Value = serde_json::from_str(&ping_msg).unwrap();

        assert_eq!(parsed["method"], "ping");
        assert!(parsed["subscription"].is_null());
    }

    #[test]
    fn test_wss_api_candle() {
        let coin = "ETH";
        let candle_msg = WssApi::candle(coin);
        let parsed: serde_json::Value = serde_json::from_str(&candle_msg).unwrap();

        assert_eq!(parsed["method"], "subscribe");
        assert_eq!(parsed["subscription"]["type"], "candle");
        assert_eq!(parsed["subscription"]["coin"], coin);
        assert_eq!(parsed["subscription"]["interval"], "1m");
    }

    #[test]
    fn test_wss_api_depth() {
        let coin = "BTC";
        let depth_msg = WssApi::depth(coin);
        let parsed: serde_json::Value = serde_json::from_str(&depth_msg).unwrap();

        assert_eq!(parsed["method"], "subscribe");
        assert_eq!(parsed["subscription"]["type"], "l2Book");
        assert_eq!(parsed["subscription"]["coin"], coin);
        assert!(parsed["subscription"]["interval"].is_null());
    }

    #[test]
    fn test_wss_api_trades() {
        let coin = "SOL";
        let trades_msg = WssApi::trades(coin);
        let parsed: serde_json::Value = serde_json::from_str(&trades_msg).unwrap();

        assert_eq!(parsed["method"], "subscribe");
        assert_eq!(parsed["subscription"]["type"], "trades");
        assert_eq!(parsed["subscription"]["coin"], coin);
        assert!(parsed["subscription"]["interval"].is_null());
    }

    #[test]
    fn test_hyperliquid_subscription_type_all_mids() {
        let subscription = HyperliquidSubscriptionType::all_mids();
        assert_eq!(subscription.typ, "allMids");
        assert!(subscription.coin.is_none());
        assert!(subscription.interval.is_none());
    }

    #[test]
    fn test_hyperliquid_subscription_type_l2_book() {
        let coin = "AVAX";
        let subscription = HyperliquidSubscriptionType::l2_book(coin);
        assert_eq!(subscription.typ, "l2Book");
        assert_eq!(subscription.coin, Some(coin.to_string()));
        assert!(subscription.interval.is_none());
    }

    #[tokio::test]
    async fn test_hyperliquid_book_message_to_local_order_book() {
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

        let local_book: LocalOrderBook = book_msg.into();
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

        let trades: Vec<Trade> = trades_msg.into();
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

    #[tokio::test]
    async fn test_hl_order_book_new_message() {
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

        assert!(order_book.new_message(book_msg).is_ok());
        assert!(order_book.book.is_some());

        let book = order_book.book.as_ref().unwrap();
        let bbo = book.get_bbo();
        assert_eq!(bbo.0, "50000");
        assert_eq!(bbo.1, "50100");
    }

    #[tokio::test]
    async fn test_hl_order_book_wrong_symbol() {
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

        let result = order_book.new_message(book_msg);
        assert!(result.is_err());
        if let Err(crate::LocalOrderBookError::WrongSymbol(expected, received)) = result {
            assert_eq!(expected, "BTC");
            assert_eq!(received, "ETH");
        } else {
            panic!("Expected WrongSymbol error");
        }
    }
}
