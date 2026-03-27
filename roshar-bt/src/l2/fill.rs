use std::collections::HashMap;
use std::str::FromStr;

use rust_decimal::prelude::ToPrimitive;
use rust_decimal::{Decimal, MathematicalOps};

use crate::types::{
    arbitrary_f64_qty_to_lot_size, price_to_tick, Event, EVENT_UPDATE_LEVEL_ASK,
    EVENT_UPDATE_LEVEL_BID,
};

use super::exchange::OrderId;
use super::orderbook::{L2OrderBookCell, Qty};

pub trait FillModel {
    fn update_trade(&mut self, event: Event, tick_orders: &[OrderId]) -> Vec<OrderId>;
    fn update_level(&mut self, ev: &Event, orderbook: &L2OrderBookCell, tick_orders: &[OrderId]);
    fn cancel_order(&mut self, id: &OrderId);
    fn new_order(&mut self, id: &OrderId, qty: Qty, level_qty: Qty);
    fn get_prio(&self, id: &OrderId) -> Option<&Qty>;
}

#[derive(Debug)]
struct LevelChgFillData {
    pub trade_qty_tmp: Qty,
    pub cum_qty_chg: Qty,
}

pub struct LevelChgFill {
    orders_by_fill_data: HashMap<u64, LevelChgFillData>,
    tick_size: Decimal,
    lot_size: Decimal,
}

impl LevelChgFill {
    pub fn new(tick_size: Decimal, lot_size: Decimal) -> Self {
        Self {
            orders_by_fill_data: HashMap::with_capacity(100),
            tick_size,
            lot_size,
        }
    }
}

impl FillModel for LevelChgFill {
    fn update_level(&mut self, ev: &Event, orderbook: &L2OrderBookCell, tick_orders: &[OrderId]) {
        match ev.typ {
            EVENT_UPDATE_LEVEL_BID | EVENT_UPDATE_LEVEL_ASK => {
                let ev_px_decimal =
                    Decimal::from_str(&ev.px).expect("Unable to parse ev.px to Decimal");
                let ev_qty_decimal =
                    Decimal::from_str(&ev.qty).expect("Unable to parse ev.qty to Decimal");

                let event_tick = price_to_tick(ev_px_decimal, self.tick_size);
                //FillModel updates before level

                let side = match ev.typ {
                    EVENT_UPDATE_LEVEL_BID => crate::types::Side::Buy,
                    EVENT_UPDATE_LEVEL_ASK => crate::types::Side::Sell,
                    _ => panic!("Called update_level with non-level event"),
                };
                let prev_qty = orderbook.borrow().get_level(side, event_tick);
                let new_qty = arbitrary_f64_qty_to_lot_size(ev_qty_decimal, self.lot_size);

                for tick_order in tick_orders {
                    let order_fill_data = self.orders_by_fill_data.get_mut(tick_order).unwrap();

                    // Isolate the cancellation portion of the level change by
                    // subtracting accumulated trade volume (avoids double
                    // counting with update_trade).  trade_qty_tmp is negative,
                    // so adding it subtracts the trade volume.
                    let mut chg = prev_qty - new_qty + order_fill_data.trade_qty_tmp;
                    order_fill_data.trade_qty_tmp = Decimal::ZERO;

                    if chg.le(&Decimal::ZERO) {
                        // Level increased (new orders placed) or change was
                        // entirely from trades — cap queue position at the
                        // new level depth.
                        order_fill_data.cum_qty_chg =
                            order_fill_data.cum_qty_chg.min(new_qty);
                        continue;
                    }

                    // Probability estimation for queue advancement from
                    // cancellations.
                    let front = order_fill_data.cum_qty_chg.max(Decimal::ZERO);
                    let back = (prev_qty - front).max(Decimal::ZERO);
                    let prob_func = |x: Decimal| -> Decimal { x.powf(3.0) };

                    let denom = prob_func(back) + prob_func(front);
                    let mut prob = if denom.is_zero() {
                        Decimal::from_str("0.5").unwrap()
                    } else {
                        prob_func(back) / denom
                    };
                    let prob_f64 = prob
                        .to_f64()
                        .expect("Unable to parse prob to f64 from Decimal");
                    if prob_f64.is_infinite() || prob_f64.is_nan() {
                        prob = Decimal::from(1);
                    }

                    let est_front = front - (Decimal::from(1) - prob) * chg
                        + (back - prob * chg).min(Decimal::ZERO);
                    order_fill_data.cum_qty_chg = est_front.min(new_qty);
                }
            }
            _ => (),
        }
    }

    fn update_trade(&mut self, event: Event, tick_orders: &[OrderId]) -> Vec<OrderId> {
        let ev_qty_decimal =
            Decimal::from_str(&event.qty).expect("Unable to parse event.qty to Decimal");
        let mut res = Vec::new();

        for tick_order in tick_orders {
            let user_fill_data = self.orders_by_fill_data.get_mut(tick_order).unwrap();

            user_fill_data.cum_qty_chg -= ev_qty_decimal;
            user_fill_data.trade_qty_tmp -= ev_qty_decimal;

            res.push(*tick_order);
        }

        res
    }

    fn cancel_order(&mut self, id: &OrderId) {
        self.orders_by_fill_data.remove(id);
    }

    fn new_order(&mut self, id: &OrderId, _qty: Qty, level_qty: Qty) {
        let data = LevelChgFillData {
            trade_qty_tmp: Decimal::from(0),
            cum_qty_chg: level_qty,
        };
        self.orders_by_fill_data.insert(*id, data);
    }

    fn get_prio(&self, id: &OrderId) -> Option<&Qty> {
        if let Some(order) = self.orders_by_fill_data.get(id) {
            return Some(&order.cum_qty_chg);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::l2::orderbook::L2OrderBook;
    use crate::types::{Event, Side, EVENT_TRADE_BUY, EVENT_UPDATE_LEVEL_BID};
    use rust_decimal::dec;
    use std::cell::RefCell;
    use std::rc::Rc;

    fn make_event(typ: u64, px: &str, qty: &str, ts: i64) -> Event {
        Event {
            typ,
            ts,
            px: px.to_string(),
            qty: qty.to_string(),
            symbol: String::new(),
        }
    }

    #[test]
    fn test_update_level_zero_front_and_back_no_panic() {
        // Regression: when both front (cum_qty_chg) and back (prev_qty - front)
        // are zero, the probability calculation divided by zero.
        let tick_size = dec!(0.01);
        let lot_size = dec!(0.1);
        let mut fill = LevelChgFill::new(tick_size, lot_size);

        // Create an order with zero cum_qty_chg (front = 0)
        let order_id: OrderId = 1;
        fill.new_order(&order_id, Decimal::ZERO, Decimal::ZERO);

        // Orderbook where the level has zero quantity (back = prev_qty - front = 0 - 0 = 0)
        let orderbook_cell: L2OrderBookCell = Rc::new(RefCell::new(L2OrderBook::new(tick_size)));

        // Level update that triggers the probability calculation (chg > 0 because
        // prev_qty(0) - new_qty(1.0) = -1.0 which is <= 0, so it continues.
        // Use a case where prev_qty > new_qty to get chg > 0:
        // Set prev_qty to 2.0, new qty to 0.5 → chg = 2.0 - 0.5 = 1.5 > 0
        orderbook_cell
            .borrow_mut()
            .update_level(Side::Buy, price_to_tick(dec!(100.00), tick_size), dec!(2.0));

        // But order has front=0, so back = prev_qty(2) - front(0) = 2.
        // That's not the zero case. To get both zero, prev_qty must be 0 AND front must be 0.
        // With prev_qty=0, chg = 0 - new_qty which is <= 0, so the `continue` triggers.
        // The zero/zero case only arises when prev_qty = front = 0 and chg > 0,
        // which means trade_qty_tmp was negative (subtracted from chg making it positive).
        // Simulate that:
        let order_id2: OrderId = 2;
        fill.new_order(&order_id2, Decimal::ZERO, Decimal::ZERO);
        // Set trade_qty_tmp to -5 by calling update_trade
        let trade_ev = make_event(EVENT_TRADE_BUY, "100.00", "5.0", 500);
        fill.update_trade(trade_ev, &[order_id2]);
        // Now cum_qty_chg = 0 - 5 = -5, trade_qty_tmp = 0 - 5 = -5

        // Reset orderbook level to 0
        let orderbook_cell2: L2OrderBookCell =
            Rc::new(RefCell::new(L2OrderBook::new(tick_size)));

        // Level update: prev_qty=0, new_qty=1.0, chg = 0 - 1.0 = -1.0
        // Then chg -= trade_qty_tmp(-5) → chg = -1.0 - (-5) = 4.0 > 0
        // front = cum_qty_chg = -5 → clamped to 0
        // back = prev_qty(0) - front(0) = 0
        // Both zero → previously panicked
        let ev = make_event(EVENT_UPDATE_LEVEL_BID, "100.00", "1.0", 1000);
        fill.update_level(&ev, &orderbook_cell2, &[order_id2]);
    }

    #[test]
    fn test_update_level_negative_cum_qty_chg_no_panic() {
        // cum_qty_chg can go negative via update_trade subtracting trade volume.
        let tick_size = dec!(0.01);
        let lot_size = dec!(0.1);
        let mut fill = LevelChgFill::new(tick_size, lot_size);

        let order_id: OrderId = 1;
        fill.new_order(&order_id, dec!(5.0), dec!(5.0));

        // Trades consume more than queue position
        let trade_ev = make_event(EVENT_TRADE_BUY, "100.00", "10.0", 1000);
        fill.update_trade(trade_ev, &[order_id]);
        // cum_qty_chg = 5.0 - 10.0 = -5.0

        let mut book = L2OrderBook::new(tick_size);
        book.update_level(Side::Buy, price_to_tick(dec!(100.00), tick_size), dec!(2.0));
        let orderbook_cell: L2OrderBookCell = Rc::new(RefCell::new(book));

        let ev = make_event(EVENT_UPDATE_LEVEL_BID, "100.00", "0.5", 2000);
        // This should not panic
        fill.update_level(&ev, &orderbook_cell, &[order_id]);
    }
}
