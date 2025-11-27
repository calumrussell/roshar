use anyhow::anyhow;
use chrono::DateTime;
use serde::{Deserialize, Serialize};

use crate::{DepthUpdate, LocalOrderBook, Trade, Venue};

// ByBit API Request Structures
#[derive(Debug, Deserialize, Serialize)]
pub struct ByBitSpotMessage {
    pub req_id: String,
    pub op: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub args: Option<Vec<String>>,
}

// ByBit Depth Structures
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ByBitSpotDepthMessage {
    pub topic: String,
    #[serde(rename = "type")]
    pub snapshot_type: String,
    pub ts: u64,
    pub data: ByBitSpotDepthBookData,
    pub cts: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ByBitSpotDepthBookData {
    pub s: String,
    pub b: Vec<[String; 2]>,
    pub a: Vec<[String; 2]>,
    pub u: u64,
    pub seq: u64,
}

// ByBit Trade Structures
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ByBitSpotTradesMessage {
    pub topic: String,
    #[serde(rename = "type")]
    pub snapshot_type: String,
    pub ts: u64,
    pub data: Vec<ByBitSpotTradesData>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ByBitSpotTradesData {
    #[serde(rename = "T")]
    pub trade_time: u64,
    pub s: String, // Symbol (e.g., "BTCUSDT")
    #[serde(rename = "S")]
    pub side: String, // Side of the trade (e.g., "Buy" or "Sell")
    pub v: String, // Volume (quantity traded)
    pub p: String, // Price
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "L")]
    pub tick_direction: Option<String>, // Tick direction (e.g., "PlusTick"), perps/futs
    pub i: String, // Trade ID
    #[serde(rename = "BT")]
    pub is_block_trade: bool, // Whether the trade is a block trade
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "RPI")]
    pub is_rpi_trade: Option<bool>, // Whether it is a RPI trade or not
}

// Implementations
impl ByBitSpotDepthMessage {
    pub fn is_full_update(&self) -> bool {
        self.snapshot_type == "snapshot"
    }
}

pub struct ByBitSpot {}

impl ByBitSpot {
    pub fn ping() -> String {
        serde_json::to_string(&ByBitSpotMessage {
            req_id: "100001".to_string(),
            op: "ping".to_string(),
            args: None,
        })
        .expect("Invalid ping")
    }

    pub fn depth(coin: &str) -> String {
        let subscribe = ByBitSpotMessage {
            req_id: "test".to_string(),
            op: "subscribe".to_string(),
            args: Some(vec![format!("orderbook.50.{coin}")]),
        };
        serde_json::to_string(&subscribe).expect("invalid depth subscription")
    }

    pub fn trades(coin: &str) -> String {
        let subscribe = ByBitSpotMessage {
            req_id: "test".to_string(),
            op: "subscribe".to_string(),
            args: Some(vec![format!("publicTrade.{}", coin)]),
        };
        serde_json::to_string(&subscribe).expect("invalid trades subscription")
    }
}

impl From<ByBitSpotTradesMessage> for Vec<Trade> {
    fn from(value: ByBitSpotTradesMessage) -> Self {
        let mut vals = vec![];
        let exchange_str = Venue::ByBitSpot.as_str();

        for trade in value.data {
            let internal = Trade {
                time: trade.trade_time as i64,
                exchange: exchange_str.to_string(),
                side: trade.side.eq("Sell"),
                coin: trade.s.clone(),
                px: trade.p.parse::<f64>().unwrap(),
                sz: trade.v.parse::<f64>().unwrap(),
            };
            vals.push(internal);
        }
        vals
    }
}

impl From<ByBitSpotDepthMessage> for LocalOrderBook {
    fn from(value: ByBitSpotDepthMessage) -> Self {
        use ordered_float::OrderedFloat;
        use std::cmp::Reverse;

        let mut bids = std::collections::BTreeMap::new();
        let mut asks = std::collections::BTreeMap::new();

        for level in value.data.b {
            if let (Some(px_str), Some(sz_str)) = (level.first(), level.get(1)) {
                if let Ok(px) = px_str.parse::<f64>() {
                    bids.insert(Reverse(OrderedFloat(px)), sz_str.as_str().into());
                }
            }
        }

        for level in value.data.a {
            if let (Some(px_str), Some(sz_str)) = (level.first(), level.get(1)) {
                if let Ok(px) = px_str.parse::<f64>() {
                    asks.insert(OrderedFloat(px), sz_str.as_str().into());
                }
            }
        }

        let coin = value.data.s;

        Self {
            bids,
            asks,
            last_update: value.ts as i64,
            last_update_ts: DateTime::from_timestamp_millis(value.ts as i64).unwrap_or_default(),
            exchange: "bybit-spot".into(),
            coin: coin.as_str().into(),
        }
    }
}

impl TryFrom<ByBitSpotDepthMessage> for Vec<DepthUpdate> {
    type Error = anyhow::Error;

    fn try_from(value: ByBitSpotDepthMessage) -> std::result::Result<Self, Self::Error> {
        let mut vals = vec![];

        if value.snapshot_type != "delta" {
            return Err(anyhow!("Not a partial update"));
        }

        let exchange_str = Venue::ByBitSpot.as_str();
        let coin_str = value.data.s.as_str();

        for bid in value.data.b {
            let internal = DepthUpdate {
                time: value.ts as i64,
                exchange: exchange_str.to_string(),
                side: false,
                coin: coin_str.to_string(),
                px: bid.first().unwrap().parse::<f64>().unwrap(),
                sz: bid.get(1).unwrap().parse::<f64>().unwrap(),
            };
            vals.push(internal);
        }

        for ask in value.data.a {
            let internal = DepthUpdate {
                time: value.ts as i64,
                exchange: exchange_str.to_string(),
                side: true,
                coin: coin_str.to_string(),
                px: ask.first().unwrap().parse::<f64>().unwrap(),
                sz: ask.get(1).unwrap().parse::<f64>().unwrap(),
            };
            vals.push(internal);
        }
        Ok(vals)
    }
}
