use anyhow::anyhow;
use chrono::DateTime;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::{DepthUpdate, LocalOrderBook, LocalOrderBookError, Trade, Venue};

// MEX Order Book Structures
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MexOrder {
    pub p: String, // Price as a string to preserve precision
    pub v: String, // Volume as a string to preserve precision
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MexDepth {
    #[serde(default)]
    pub asks: Vec<MexOrder>, // List of ask orders
    #[serde(default)]
    pub bids: Vec<MexOrder>, // List of bid orders
    pub e: String, // Event type
    pub r: String, // Some reference number or ID
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MexDepthMessage {
    pub c: String,   // Channel name
    pub d: MexDepth, // Depth data (bids and asks)
    pub s: String,   // Symbol (e.g., BTCUSDT)
    pub t: u64,      // Timestamp in milliseconds
}

// MEX Trade Structures
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MexDeal {
    #[serde(rename = "S")]
    pub s: u8, // Side or type of the deal (assuming an integer)
    pub p: String, // Price as a string to preserve precision
    pub t: u64,    // Timestamp in milliseconds
    pub v: String, // Volume as a string to preserve precision
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MexDeals {
    pub deals: Vec<MexDeal>, // List of deals
    pub e: String,           // Event type
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MexDealsMessage {
    pub c: String,   // Channel name
    pub d: MexDeals, // Deals data
    pub s: String,   // Symbol (e.g., BTCUSDT)
    pub t: u64,      // Timestamp in milliseconds
}

// MEX API Request Structures
#[derive(Debug, Deserialize, Serialize)]
pub struct MexMessage {
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Vec<String>>,
}

// Implementations
impl MexDepthMessage {
    pub fn is_full_update(&self) -> bool {
        let partial_str = "spot@public.limit.depth.v3.api";
        self.c.starts_with(partial_str)
    }
}

pub struct Mex {}

impl Mex {
    pub fn ping() -> String {
        serde_json::to_string(&MexMessage {
            method: "PING".to_string(),
            params: None,
        })
        .expect("invalid pong")
    }

    pub fn depth(coin: &str) -> String {
        let subscribe = MexMessage {
            method: "SUBSCRIPTION".to_string(),
            params: Some(vec![
                format!("spot@public.limit.depth.v3.api@{}@20", coin),
                format!("spot@public.increase.depth.v3.api@{}", coin),
            ]),
        };
        serde_json::to_string(&subscribe).expect("invalid depth subscription")
    }

    pub fn trades(coin: &str) -> String {
        let subscribe = MexMessage {
            method: "SUBSCRIPTION".to_string(),
            params: Some(vec![format!("spot@public.deals.v3.api@{}", coin)]),
        };
        serde_json::to_string(&subscribe).expect("invalid trades subscription")
    }
}

// Conversion implementations
impl From<MexDealsMessage> for Vec<Trade> {
    fn from(value: MexDealsMessage) -> Self {
        let mut vals = vec![];

        let deals = value.d.deals;
        for deal in deals {
            let internal = Trade {
                time: deal.t as i64,
                exchange: Venue::Mex.to_string(),
                side: deal.s == 2,
                coin: value.s.clone(),
                px: deal.p.parse::<f64>().unwrap(),
                sz: deal.v.parse::<f64>().unwrap(),
            };
            vals.push(internal);
        }

        vals
    }
}

impl From<MexDepthMessage> for LocalOrderBook {
    fn from(value: MexDepthMessage) -> Self {
        use ordered_float::OrderedFloat;
        use std::cmp::Reverse;

        let mut bids = std::collections::BTreeMap::new();
        let mut asks = std::collections::BTreeMap::new();

        for level in &value.d.bids {
            if let Ok(px) = level.p.parse::<f64>() {
                bids.insert(Reverse(OrderedFloat(px)), level.v.as_str().into());
            }
        }

        for level in &value.d.asks {
            if let Ok(px) = level.p.parse::<f64>() {
                asks.insert(OrderedFloat(px), level.v.as_str().into());
            }
        }

        Self {
            bids,
            asks,
            last_update: value.t as i64,
            last_update_ts: DateTime::from_timestamp_millis(value.t as i64).unwrap_or_default(),
            exchange: Venue::Mex.to_string().into(),
            coin: value.s.as_str().into(),
        }
    }
}

impl TryFrom<MexDepthMessage> for Vec<DepthUpdate> {
    type Error = anyhow::Error;

    fn try_from(value: MexDepthMessage) -> Result<Self, Self::Error> {
        let mut vals = vec![];

        let is_full_depth = value.d.asks.len() == 20 && value.d.bids.len() == 20;
        if is_full_depth {
            return Err(anyhow!("Not a partial update"));
        }

        let exchange_str = Venue::Mex.as_str();
        let coin_str = value.s.as_str();

        for bid in value.d.bids {
            let internal = DepthUpdate {
                time: value.t as i64,
                exchange: exchange_str.to_string(),
                side: false,
                coin: coin_str.to_string(),
                px: bid.p.parse::<f64>().unwrap(),
                sz: bid.v.parse::<f64>().unwrap(),
            };
            vals.push(internal);
        }

        for ask in value.d.asks {
            let internal = DepthUpdate {
                time: value.t as i64,
                exchange: exchange_str.to_string(),
                side: true,
                coin: coin_str.to_string(),
                px: ask.p.parse::<f64>().unwrap(),
                sz: ask.v.parse::<f64>().unwrap(),
            };
            vals.push(internal);
        }
        Ok(vals)
    }
}

// New From implementations for daily partitioned tables
impl From<MexDealsMessage> for Vec<crate::TradeData> {
    fn from(value: MexDealsMessage) -> Self {
        value
            .d
            .deals
            .into_iter()
            .map(|trade| crate::TradeData {
                px: trade.p,
                qty: trade.v,
                time: value.t,
                time_ts: DateTime::from_timestamp_millis(value.t as i64).unwrap_or_default(),
                ticker: value.s.clone(),
                meta: format!("{{\"e\": \"{}\"}}", value.d.e),
                side: trade.s == 2, // 2 for sell, 1 for buy
                venue: Venue::Mex,
            })
            .collect()
    }
}

impl From<MexDepthMessage> for Vec<crate::DepthUpdateData> {
    fn from(value: MexDepthMessage) -> Self {
        let mut res = Vec::new();

        for bid in &value.d.bids {
            res.push(crate::DepthUpdateData {
                px: bid.p.clone(),
                qty: bid.v.clone(),
                time: value.t,
                time_ts: DateTime::from_timestamp_millis(value.t as i64).unwrap_or_default(),
                ticker: value.s.clone(),
                meta: format!("{{\"e\": \"{}\", \"r\": \"{}\"}}", value.d.e, value.d.r),
                side: false, // bid side
                venue: Venue::Mex,
            });
        }

        for ask in &value.d.asks {
            res.push(crate::DepthUpdateData {
                px: ask.p.clone(),
                qty: ask.v.clone(),
                time: value.t,
                time_ts: DateTime::from_timestamp_millis(value.t as i64).unwrap_or_default(),
                ticker: value.s.clone(),
                meta: format!("{{\"e\": \"{}\", \"r\": \"{}\"}}", value.d.e, value.d.r),
                side: true, // ask side
                venue: Venue::Mex,
            });
        }

        res
    }
}

pub struct MexOrderBook {
    pub book: HashMap<String, LocalOrderBook>,
}

impl Default for MexOrderBook {
    fn default() -> Self {
        Self::new()
    }
}

impl MexOrderBook {
    pub fn new() -> Self {
        Self {
            book: HashMap::new(),
        }
    }

    pub async fn new_message(&mut self, msg: MexDepthMessage) -> Result<(), LocalOrderBookError> {
        let coin = msg.s.clone();

        if msg.is_full_update() {
            let ob: LocalOrderBook = msg.into();
            self.book.insert(coin, ob);
        } else if let Some(book) = self.book.get_mut(&coin) {
            if let Ok(updates) = msg.try_into() {
                book.apply_updates(&updates, 50);
            }
        } else {
            return Err(LocalOrderBookError::BookUpdateBeforeSnapshot(
                "mex".to_string(),
                coin,
            ));
        }
        Ok(())
    }
}
