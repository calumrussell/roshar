use crate::types::{
    Event, EVENT_TRADE_BUY, EVENT_TRADE_SELL, EVENT_UPDATE_LEVEL_ASK, EVENT_UPDATE_LEVEL_BID,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KrakenBookLevel {
    pub price: String,
    pub qty: String,
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

impl From<KrakenBookSnapshotMessage> for Vec<Event> {
    fn from(val: KrakenBookSnapshotMessage) -> Self {
        let mut events = Vec::new();

        for bid in val.bids {
            let event = Event::new(
                EVENT_UPDATE_LEVEL_BID,
                val.timestamp,
                bid.price.as_str(),
                bid.qty.as_str(),
            );
            events.push(event);
        }

        for ask in val.asks {
            let event = Event::new(
                EVENT_UPDATE_LEVEL_ASK,
                val.timestamp,
                ask.price.as_str(),
                ask.qty.as_str(),
            );
            events.push(event);
        }
        events
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KrakenBookDeltaMessage {
    pub feed: String,
    pub product_id: String,
    pub timestamp: i64,
    pub seq: i64,
    pub side: String,
    pub price: String,
    pub qty: String,
}

impl From<KrakenBookDeltaMessage> for Vec<Event> {
    fn from(val: KrakenBookDeltaMessage) -> Self {
        let mut events = Vec::new();

        let event_type = if val.side == "buy" {
            EVENT_UPDATE_LEVEL_BID
        } else {
            EVENT_UPDATE_LEVEL_ASK
        };

        let event = Event::new(
            event_type,
            val.timestamp,
            val.price.as_str(),
            val.qty.as_str(),
        );

        events.push(event);
        events
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KrakenSingleTrade {
    pub feed: String,
    pub product_id: String,
    pub uid: String,
    pub side: String,
    #[serde(rename = "type")]
    pub type_field: String,
    pub seq: i64,
    pub time: i64,
    pub qty: String,
    pub price: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KrakenTradeSnapshotMessage {
    pub feed: String,
    pub product_id: String,
    pub trades: Vec<KrakenSingleTrade>,
}

impl From<KrakenTradeSnapshotMessage> for Vec<Event> {
    fn from(val: KrakenTradeSnapshotMessage) -> Self {
        let mut events = Vec::new();

        for trade in val.trades {
            let event_type = if trade.side == "buy" {
                EVENT_TRADE_BUY
            } else {
                EVENT_TRADE_SELL
            };

            let event = Event::new(
                event_type,
                trade.time,
                trade.price.as_str(),
                trade.qty.as_str(),
            );

            events.push(event);
        }

        events
    }
}
