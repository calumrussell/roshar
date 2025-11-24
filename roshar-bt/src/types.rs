use std::str::FromStr;

use rust_decimal::{prelude::ToPrimitive, Decimal, MathematicalOps};

#[repr(u64)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TypFlag {
    LevelUpdate = 0b00000001,
    Clear = 0b00000010,
    Candle = 0b00000100,
    Trade = 0b00001000,
    ClearSide = 0b00010000,
}

#[repr(u64)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AttFlag {
    Null = 0b000,
    Buy = 0b001 << 32,
    Sell = 0b010 << 32,
}

pub const EVENT_UPDATE_LEVEL_BID: u64 = TypFlag::LevelUpdate as u64 | AttFlag::Buy as u64;
pub const EVENT_UPDATE_LEVEL_ASK: u64 = TypFlag::LevelUpdate as u64 | AttFlag::Sell as u64;

pub const EVENT_CLEAR_LEVEL_BID: u64 = TypFlag::Clear as u64 | AttFlag::Buy as u64;
pub const EVENT_CLEAR_LEVEL_ASK: u64 = TypFlag::Clear as u64 | AttFlag::Sell as u64;

pub const EVENT_CLEAR_SIDE_BID: u64 = TypFlag::ClearSide as u64 | AttFlag::Buy as u64;
pub const EVENT_CLEAR_SIDE_ASK: u64 = TypFlag::ClearSide as u64 | AttFlag::Sell as u64;

pub const EVENT_CLEAR_BOOK: u64 = TypFlag::Clear as u64 | AttFlag::Null as u64;
pub const EVENT_CANDLE: u64 = TypFlag::Candle as u64 | AttFlag::Null as u64;

pub const EVENT_TRADE_BUY: u64 = TypFlag::Trade as u64 | AttFlag::Buy as u64;
pub const EVENT_TRADE_SELL: u64 = TypFlag::Trade as u64 | AttFlag::Sell as u64;

#[repr(align(32))]
#[derive(Clone, Debug, PartialEq)]
pub struct Event {
    pub typ: u64,
    pub ts: i64,
    pub px: String,
    pub qty: String,
}

impl Event {
    pub fn new(typ: u64, ts: i64, px: &str, qty: &str) -> Self {
        Self {
            typ,
            ts,
            px: px.to_string(),
            qty: qty.to_string(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum OrderStatus {
    Working,
    Filled,
    Cancelled,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Side {
    Buy,
    Sell,
}

#[derive(Clone, Debug, PartialEq)]
pub enum OrderType {
    Market,
    Limit,
}

//Types denote a user-generated value that may not be formatted correctly
pub type UnformattedQty = f64;
pub type UnformattedPx = f64;

#[derive(Clone, Debug, PartialEq)]
pub struct OrderRequest {
    pub side: Side,
    pub qty: UnformattedQty,
    pub px: Option<UnformattedPx>,
    pub typ: OrderType,
    ts: Option<i64>,
}

impl OrderRequest {
    pub fn new(side: Side, qty: f64, px: Option<f64>, typ: OrderType) -> Self {
        Self {
            side,
            qty,
            px,
            typ,
            ts: None,
        }
    }

    pub fn set_time(&mut self, ts: i64) {
        self.ts = Some(ts);
    }

    pub fn get_time(&self) -> i64 {
        self.ts.expect("Tried unwrap of OrderRequest without time")
    }
}

pub fn price_to_tick(price: Decimal, tick_size: Decimal) -> i64 {
    let tick_decimal = price / tick_size;
    let rounded = tick_decimal.round();

    rounded.try_into().unwrap_or_default()
}

pub fn tick_to_price(tick: i64, tick_size: Decimal) -> Decimal {
    let price = Decimal::from(tick) * tick_size;
    let precision = tick_size
        .log10()
        .abs()
        .ceil()
        .to_i64()
        .expect("Failed to convert tick_size to i64");
    let factor = Decimal::from(10).powi(precision);
    (price * factor).round() / factor
}

pub fn arbitrary_f64_qty_to_lot_size(qty: Decimal, lot_size: Decimal) -> Decimal {
    (qty / lot_size).floor() * lot_size
}

#[derive(Clone, Debug)]
pub struct Candle {
    pub high: Decimal,
    pub low: Decimal,
    pub open: Decimal,
    pub close: Decimal,
    pub time: i64,
}

impl Candle {
    pub fn from_str(high: &str, low: &str, open: &str, close: &str, time: &i64) -> Self {
        Self {
            //TODO: this should be caught by caller
            high: Decimal::from_str(high).unwrap(),
            close: Decimal::from_str(close).unwrap(),
            low: Decimal::from_str(low).unwrap(),
            open: Decimal::from_str(open).unwrap(),
            time: time.clone(),
        }
    }
}
