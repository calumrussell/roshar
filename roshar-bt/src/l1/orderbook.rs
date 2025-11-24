use rust_decimal::Decimal;

use super::exchange::Price;
use crate::types::{price_to_tick, tick_to_price};

pub type Tick = i64;
pub type Qty = Decimal;

pub struct OrderBook {
    best_ask: Tick,
    best_bid: Tick,
    tick_size: Decimal,
}

impl OrderBook {
    pub fn new(tick_size: Decimal) -> Self {
        Self {
            best_ask: i64::MAX,
            best_bid: i64::MIN,
            tick_size,
        }
    }

    pub fn get_best_ask(&self) -> Price {
        tick_to_price(self.best_ask, self.tick_size)
    }

    pub fn get_best_bid(&self) -> Price {
        tick_to_price(self.best_bid, self.tick_size)
    }

    pub fn update_price(&mut self, bid: Price, ask: Price) {
        self.best_bid = price_to_tick(bid, self.tick_size);
        self.best_ask = price_to_tick(ask, self.tick_size);
    }
}
