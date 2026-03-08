use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

use rust_decimal::{prelude::FromPrimitive, Decimal};

use crate::types::{tick_to_price, Side};

use super::exchange::Price;

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
    pub fn new(tick_size: Decimal) -> Self {
        Self {
            ask_depth: BTreeMap::new(),
            bid_depth: BTreeMap::new(),
            tick_size,
        }
    }

    pub fn new_refcell(tick_size: Decimal) -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Self::new(tick_size)))
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
        let depth = match side {
            Side::Buy => &mut self.bid_depth,
            Side::Sell => &mut self.ask_depth,
        };

        if qty.is_zero() {
            depth.remove(&tick);
        } else {
            depth.insert(tick, qty);
        }
    }
}

#[cfg(test)]
mod tests {
    use rust_decimal::Decimal;
    use rust_decimal::dec;

    use crate::types::{tick_to_price, Side};

    use super::L2OrderBook;

    #[test]
    fn test_update_level_zero_qty_removes_entry() {
        let mut book = L2OrderBook::new(dec!(0.01));
        book.update_level(Side::Buy, 1000, dec!(5.0));
        assert_eq!(book.get_level(Side::Buy, 1000), dec!(5.0));

        book.update_level(Side::Buy, 1000, dec!(0));
        assert_eq!(book.get_level(Side::Buy, 1000), Decimal::ZERO);

        book.update_level(Side::Buy, 999, dec!(3.0));
        assert_eq!(book.get_best_bid(), tick_to_price(999, dec!(0.01)));
    }

    #[test]
    fn test_best_bid_not_corrupted_by_zero_qty() {
        let mut book = L2OrderBook::new(dec!(0.1));
        book.update_level(Side::Buy, 1000, dec!(10.0));
        book.update_level(Side::Buy, 5000, dec!(1.0));
        assert_eq!(book.get_best_bid(), tick_to_price(5000, dec!(0.1)));

        book.update_level(Side::Buy, 5000, dec!(0));
        assert_eq!(book.get_best_bid(), tick_to_price(1000, dec!(0.1)));
    }

    #[test]
    fn test_best_ask_not_corrupted_by_zero_qty() {
        let mut book = L2OrderBook::new(dec!(0.1));
        book.update_level(Side::Sell, 2000, dec!(10.0));
        book.update_level(Side::Sell, 500, dec!(1.0));
        assert_eq!(book.get_best_ask(), tick_to_price(500, dec!(0.1)));

        book.update_level(Side::Sell, 500, dec!(0));
        assert_eq!(book.get_best_ask(), tick_to_price(2000, dec!(0.1)));
    }

    #[test]
    fn test_zero_qty_update_on_nonexistent_level() {
        let mut book = L2OrderBook::new(dec!(0.01));
        book.update_level(Side::Buy, 1000, dec!(0));
        book.update_level(Side::Buy, 1000, dec!(0));
        assert_eq!(book.get_best_bid(), Decimal::ZERO);
    }

    #[test]
    fn test_get_level_returns_zero_after_removal() {
        let mut book = L2OrderBook::new(dec!(0.01));
        book.update_level(Side::Sell, 1000, dec!(50.0));
        assert_eq!(book.get_level(Side::Sell, 1000), dec!(50.0));

        book.update_level(Side::Sell, 1000, dec!(0));
        assert_eq!(book.get_level(Side::Sell, 1000), Decimal::ZERO);
    }

    #[test]
    fn test_bbo_stable_after_many_level_additions_and_removals() {
        let mut book = L2OrderBook::new(dec!(0.1));

        book.update_level(Side::Buy, 895230, dec!(1.0));
        book.update_level(Side::Sell, 895240, dec!(1.0));

        for tick in (1..1000).chain(900000..901000) {
            book.update_level(Side::Buy, tick, dec!(0.5));
        }
        for tick in (1..1000).chain(900000..901000) {
            book.update_level(Side::Buy, tick, dec!(0));
        }

        assert_eq!(book.get_best_bid(), tick_to_price(895230, dec!(0.1)));
        assert_eq!(book.get_best_ask(), tick_to_price(895240, dec!(0.1)));
    }
}
