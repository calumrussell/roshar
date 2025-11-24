use std::collections::{HashMap, VecDeque};

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

use crate::source::{CandleProducer, CandleWriter, EventProducer, EventWriter};
use crate::types::{
    Candle, Event, EVENT_CANDLE, EVENT_CLEAR_SIDE_ASK, EVENT_CLEAR_SIDE_BID, EVENT_TRADE_BUY,
    EVENT_TRADE_SELL, EVENT_UPDATE_LEVEL_ASK, EVENT_UPDATE_LEVEL_BID,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HyperliquidCandleData {
    #[serde(rename = "T")]
    t_end: i64,
    c: String,
    h: String,
    i: String,
    l: String,
    n: i64,
    o: String,
    s: String,
    #[serde(rename = "t")]
    t_start: i64,
    v: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HyperliquidCandleMessage {
    channel: String,
    data: HyperliquidCandleData,
}

impl From<HyperliquidCandleMessage> for Candle {
    fn from(value: HyperliquidCandleMessage) -> Self {
        Candle::from_str(
            &value.data.h,
            &value.data.l,
            &value.data.o,
            &value.data.c,
            &value.data.t_end,
        )
    }
}

impl CandleWriter for HyperliquidCandleMessage {
    fn write_to_candle_queue(&self, candles: &mut VecDeque<Candle>) {
        let candle = Candle::from_str(
            &self.data.h,
            &self.data.l,
            &self.data.o,
            &self.data.c,
            &self.data.t_end,
        );
        candles.push_back(candle);
    }
}

impl EventWriter for HyperliquidCandleMessage {
    fn write_to_queue(&self, evs: &mut VecDeque<Event>) {
        evs.push_back(Event::new(
            EVENT_CANDLE,
            self.data.t_start,
            self.data.c.as_str(),
            "0.0",
        ));
    }
}

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

impl EventWriter for HyperliquidBookMessage {
    fn write_to_queue(&self, evs: &mut VecDeque<Event>) {
        evs.push_back(Event::new(
            EVENT_CLEAR_SIDE_ASK,
            self.data.time as i64,
            "0.0",
            "0.0",
        ));
        evs.push_back(Event::new(
            EVENT_CLEAR_SIDE_BID,
            self.data.time as i64,
            "0.0",
            "0.0",
        ));

        for level in &self.data.levels[0] {
            evs.push_back(Event::new(
                EVENT_UPDATE_LEVEL_BID,
                self.data.time as i64,
                level.px.as_str(),
                level.sz.as_str(),
            ));
        }

        for level in &self.data.levels[1] {
            evs.push_back(Event::new(
                EVENT_UPDATE_LEVEL_ASK,
                self.data.time as i64,
                level.px.as_str(),
                level.sz.as_str(),
            ));
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct HyperliquidTrade {
    coin: String,
    hash: String,
    px: String,
    side: String,
    sz: String,
    tid: u64,
    time: u64,
    users: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HyperliquidTradesMessage {
    channel: String,
    data: Vec<HyperliquidTrade>,
}

impl EventWriter for HyperliquidTradesMessage {
    fn write_to_queue(&self, evs: &mut VecDeque<Event>) {
        for trade in self.data.iter() {
            let event_type = if trade.side == "B" {
                EVENT_TRADE_BUY
            } else {
                EVENT_TRADE_SELL
            };

            evs.push_back(Event::new(
                event_type,
                trade.time as i64,
                trade.px.as_str(),
                trade.sz.as_str(),
            ));
        }
    }
}

#[derive(Clone)]
pub struct HyperliquidCandleParser {
    last_candle_event_prod: HashMap<String, HyperliquidCandleMessage>,
    last_candle_candle_prod: HashMap<String, HyperliquidCandleMessage>,
}

impl HyperliquidCandleParser {
    pub fn new() -> Self {
        Self {
            last_candle_event_prod: HashMap::new(),
            last_candle_candle_prod: HashMap::new(),
        }
    }
}

impl Default for HyperliquidCandleParser {
    fn default() -> Self {
        Self::new()
    }
}

impl EventProducer for HyperliquidCandleParser {
    fn parse_line(
        &mut self,
        line: &str,
        evs: &mut VecDeque<Event>,
    ) -> Result<(), serde_json::Error> {
        if line.is_empty() {
            return Ok(());
        }
        let (_recv_ts, data) = line.split_at(20);
        let msg = serde_json::from_str::<HyperliquidCandleMessage>(data)?;

        let coin = msg.data.s.clone();
        match self.last_candle_event_prod.entry(coin) {
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(msg.clone());
                Ok(())
            }
            std::collections::hash_map::Entry::Occupied(mut entry) => {
                let value = entry.get();
                if value.data.t_end != msg.data.t_end {
                    value.write_to_queue(evs);
                }
                entry.insert(msg);
                Ok(())
            }
        }
    }
}

impl CandleProducer for HyperliquidCandleParser {
    type ToCandle = HyperliquidCandleMessage;
    fn parse_candle(
        &mut self,
        line: &str,
        candles: &mut VecDeque<Candle>,
    ) -> Result<(), serde_json::Error> {
        let (_recv_ts, data) = line.split_at(20);
        let msg = serde_json::from_str::<HyperliquidCandleMessage>(data)?;

        let coin = msg.data.s.clone();
        match self.last_candle_candle_prod.entry(coin) {
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(msg.clone());
                Ok(())
            }
            std::collections::hash_map::Entry::Occupied(mut entry) => {
                let value = entry.get();
                if value.data.t_end != msg.data.t_end {
                    value.write_to_candle_queue(candles);
                }
                entry.insert(msg);
                Ok(())
            }
        }
    }
}

#[derive(Clone)]
pub struct HyperliquidParser;

impl EventProducer for HyperliquidParser {
    fn parse_line(
        &mut self,
        line: &str,
        evs: &mut VecDeque<Event>,
    ) -> Result<(), serde_json::Error> {
        let (_recv_ts, data) = line.split_at(20);
        serde_json::from_str::<HyperliquidBookMessage>(data)
            .map(|v| v.write_to_queue(evs))
            .or_else(|_| {
                serde_json::from_str::<HyperliquidTradesMessage>(data)
                    .map(|v| v.write_to_queue(evs))
            })?;
        Ok(())
    }
}
