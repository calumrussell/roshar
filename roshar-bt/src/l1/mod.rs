use anyhow::{anyhow, Result};
use rust_decimal::prelude::{FromPrimitive, ToPrimitive};
use rust_decimal::Decimal;
use std::time::Duration;

use crate::source::EventProducer;
use crate::types::{OrderStatus, Side};

pub mod backtest;
pub mod exchange;
pub mod orderbook;

pub use backtest::Backtest;

#[derive(Clone, Debug, PartialEq)]
pub struct L1OrderInternal {
    pub id: u64,
    pub side: Side,
    pub qty: Decimal,
    pub exec_px: Decimal,
    pub exec_tick: i64,
    pub status: OrderStatus,
}

#[derive(Clone, Debug, PartialEq)]
pub struct L1Order {
    pub id: u64,
    pub side: Side,
    pub qty: f64,
    pub exec_px: f64,
    pub exec_tick: i64,
    pub status: OrderStatus,
}

impl From<L1OrderInternal> for L1Order {
    fn from(value: L1OrderInternal) -> Self {
        let qty_f64 = value
            .qty
            .to_f64()
            .expect("Unable to convert qty from Decimal to f64");
        let exec_px_f64 = value
            .exec_px
            .to_f64()
            .expect("Unable to convert exec_px from Decimal to f64");

        Self {
            id: value.id,
            side: value.side,
            qty: qty_f64,
            exec_px: exec_px_f64,
            exec_tick: value.exec_tick,
            status: value.status,
        }
    }
}

pub struct L1ConfigBuilder<P: EventProducer> {
    tick_size: Option<f64>,
    lines_read_per_tick: usize,
    start_ts: Option<i64>,
    risk_free_rate: f64,
    return_window: Option<Duration>,
    parser: Option<P>,
}

impl<P: EventProducer> L1ConfigBuilder<P> {
    pub fn new() -> Self {
        Self {
            tick_size: None,
            lines_read_per_tick: 100,
            start_ts: None,
            risk_free_rate: 0.02,
            return_window: None,
            parser: None,
        }
    }

    pub fn set_parser(&mut self, parser: P) -> &mut Self {
        self.parser = Some(parser);
        self
    }

    pub fn set_tick_size(&mut self, tick_size: f64) -> &mut Self {
        self.tick_size = Some(tick_size);
        self
    }

    pub fn set_start_ts(&mut self, start_ts: i64) -> &mut Self {
        self.start_ts = Some(start_ts);
        self
    }

    pub fn set_lines_read_per_tick(&mut self, lines_read_per_tick: usize) -> &mut Self {
        self.lines_read_per_tick = lines_read_per_tick;
        self
    }

    pub fn set_risk_free_rate(&mut self, risk_free_rate: f64) -> &mut Self {
        self.risk_free_rate = risk_free_rate;
        self
    }

    pub fn set_return_window(&mut self, return_window_seconds: u64) -> &mut Self {
        self.return_window = Some(Duration::from_secs(return_window_seconds));
        self
    }

    pub fn build(&mut self) -> Result<L1Config<P>> {
        if self.tick_size.is_none()
            || self.start_ts.is_none()
            || self.return_window.is_none()
            || self.parser.is_none()
        {
            return Err(anyhow!("Missing required argument"));
        }

        let tick_size_decimal = Decimal::from_f64(self.tick_size.unwrap())
            .expect("Unable to parse tick_size as Decimal");
        let risk_free_rate_decimal = Decimal::from_f64(self.risk_free_rate)
            .expect("Unable to parse risk_free_rate as Decimal");

        let parser = self.parser.take().unwrap();

        let config = L1Config {
            tick_size: tick_size_decimal,
            lines_read_per_tick: self.lines_read_per_tick,
            start_ts: self.start_ts.unwrap(),
            risk_free_rate: risk_free_rate_decimal,
            return_window: self.return_window.unwrap(),
            parser,
        };

        Ok(config)
    }
}

impl<P: EventProducer> Default for L1ConfigBuilder<P> {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
pub struct L1Config<P: EventProducer> {
    pub tick_size: Decimal,
    pub lines_read_per_tick: usize,
    pub start_ts: i64,
    pub risk_free_rate: Decimal,
    pub return_window: Duration,
    pub parser: P,
}

impl<P: EventProducer> L1Config<P> {
    pub fn get_tick_size(&self) -> f64 {
        self.tick_size.to_f64().unwrap()
    }
}
