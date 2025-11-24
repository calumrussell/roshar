use crate::types::{
    Event, EVENT_TRADE_BUY, EVENT_TRADE_SELL, EVENT_UPDATE_LEVEL_ASK, EVENT_UPDATE_LEVEL_BID,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct ByBitMessage {
    pub req_id: String,
    pub op: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub args: Option<Vec<String>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ByBitDepthMessage {
    pub topic: String,
    #[serde(rename = "type")]
    pub snapshot_type: String,
    pub ts: u64,
    pub data: ByBitDepthBookData,
    pub cts: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ByBitDepthBookData {
    pub s: String,
    pub b: Vec<[String; 2]>,
    pub a: Vec<[String; 2]>,
    pub u: u64,
    pub seq: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ByBitTradesMessage {
    pub topic: String,
    #[serde(rename = "type")]
    pub snapshot_type: String,
    pub ts: u64,
    pub data: Vec<ByBitTradesData>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ByBitTradesData {
    #[serde(rename = "T")]
    pub trade_time: u64,
    pub s: String,
    #[serde(rename = "S")]
    pub side: String,
    pub v: String,
    pub p: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "L")]
    pub tick_direction: Option<String>,
    pub i: String,
    #[serde(rename = "BT")]
    pub is_block_trade: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "RPI")]
    pub is_rpi_trade: Option<bool>,
}

impl From<ByBitDepthMessage> for Vec<Event> {
    fn from(val: ByBitDepthMessage) -> Self {
        let mut events = Vec::new();

        for bid in val.data.b {
            let event = Event::new(
                EVENT_UPDATE_LEVEL_BID,
                val.ts as i64,
                bid[0].as_str(),
                bid[1].as_str(),
            );
            events.push(event);
        }

        for ask in val.data.a {
            let event = Event::new(
                EVENT_UPDATE_LEVEL_ASK,
                val.ts as i64,
                ask[0].as_str(),
                ask[1].as_str(),
            );
            events.push(event);
        }

        events
    }
}

impl From<ByBitTradesMessage> for Vec<Event> {
    fn from(val: ByBitTradesMessage) -> Self {
        let mut events = Vec::new();

        for trade in val.data {
            let event = Event::new(
                if trade.side == "Buy" {
                    EVENT_TRADE_BUY
                } else {
                    EVENT_TRADE_SELL
                },
                val.ts as i64,
                trade.p.as_str(),
                trade.v.as_str(),
            );
            events.push(event);
        }

        events
    }
}
