use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

use rust_decimal::{prelude::FromPrimitive, Decimal};

use crate::source::EventProducer;
use crate::types::{tick_to_price, Side};

use super::exchange::Price;
use super::L2Config;

pub type Tick = i64;
pub type Qty = Decimal;

/// `L2OrderBook` is not modified by user orders. Order executions are calculated probabilistically
/// by comparing the order with some other state, one of which is current `L2OrderBook`.
pub struct L2OrderBook {
    ask_depth: BTreeMap<Tick, Qty>,
    bid_depth: BTreeMap<Tick, Qty>,
    tick_size: Decimal,
}

pub type L2OrderBookCell = Rc<RefCell<L2OrderBook>>;

impl L2OrderBook {
    pub fn new<P: EventProducer>(config: &L2Config<P>) -> Self {
        Self {
            ask_depth: BTreeMap::new(),
            bid_depth: BTreeMap::new(),
            tick_size: config.tick_size,
        }
    }

    pub fn new_refcell<P: EventProducer>(config: &L2Config<P>) -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Self::new(config)))
    }

    pub fn get_best_ask(&self) -> Price {
        if let Some(first) = self.ask_depth.first_key_value() {
            return tick_to_price(*first.0, self.tick_size);
        }
        Decimal::from_i64(0).unwrap()
    }

    pub fn get_best_bid(&self) -> Price {
        if let Some(first) = self.bid_depth.last_key_value() {
            return tick_to_price(*first.0, self.tick_size);
        }
        Decimal::from_i64(0).unwrap()
    }

    pub fn get_level(&self, side: Side, tick: Tick) -> Qty {
        match side {
            Side::Buy => *self.bid_depth.get(&tick).unwrap_or(&Decimal::ZERO),
            Side::Sell => *self.ask_depth.get(&tick).unwrap_or(&Decimal::ZERO),
        }
    }

    pub fn clear_bid(&mut self) {
        self.bid_depth.clear();
    }

    pub fn clear_ask(&mut self) {
        self.ask_depth.clear();
    }

    pub fn clear_bid_level(&mut self, tick: Tick) {
        self.bid_depth.remove(&tick);
    }

    pub fn clear_ask_level(&mut self, tick: Tick) {
        self.ask_depth.remove(&tick);
    }

    pub fn clear(&mut self) {
        self.clear_bid();
        self.clear_ask();
    }

    pub fn update_level(&mut self, side: Side, tick: Tick, qty: Qty) {
        match side {
            Side::Buy => {
                self.bid_depth.insert(tick, qty);
            }
            Side::Sell => {
                self.ask_depth.insert(tick, qty);
            }
        }
    }
}
