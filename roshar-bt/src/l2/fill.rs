use std::collections::HashMap;
use std::str::FromStr;

use rust_decimal::prelude::ToPrimitive;
use rust_decimal::{Decimal, MathematicalOps};

use crate::source::EventProducer;
use crate::types::{
    arbitrary_f64_qty_to_lot_size, price_to_tick, Event, EVENT_UPDATE_LEVEL_ASK,
    EVENT_UPDATE_LEVEL_BID,
};

use super::exchange::OrderId;
use super::orderbook::{L2OrderBookCell, Qty};
use super::L2Config;

pub trait FillModel {
    fn update_trade(&mut self, event: Event, tick_orders: &[OrderId]) -> Vec<OrderId>;
    fn update_level(&mut self, ev: &Event, orderbook: &L2OrderBookCell, tick_orders: &[OrderId]);
    fn cancel_order(&mut self, id: &OrderId);
    fn new_order(&mut self, id: &OrderId, qty: Qty);
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
    pub fn new<P: EventProducer>(config: &L2Config<P>) -> Self {
        Self {
            orders_by_fill_data: HashMap::with_capacity(100),
            tick_size: config.tick_size,
            lot_size: config.lot_size,
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
                //Theoretically not a user input so should be rounded but we double-check here in case exchange sends bad data
                let event_qty_by_lot_size =
                    arbitrary_f64_qty_to_lot_size(ev_qty_decimal, self.lot_size);
                let mut chg = prev_qty - event_qty_by_lot_size;

                for tick_order in tick_orders {
                    let order_fill_data = self.orders_by_fill_data.get_mut(tick_order).unwrap();
                    chg -= order_fill_data.trade_qty_tmp;
                    order_fill_data.trade_qty_tmp = Decimal::ZERO;

                    if chg.le(&Decimal::ZERO) {
                        order_fill_data.cum_qty_chg =
                            order_fill_data.cum_qty_chg.min(ev_qty_decimal);
                        continue;
                    }

                    let front = order_fill_data.cum_qty_chg;
                    let back = prev_qty - front;
                    let prob_func = |x: Decimal| -> Decimal { x.powf(3.0) };

                    let mut prob = prob_func(back) / (prob_func(back) + prob_func(front));
                    let prob_f64 = prob
                        .to_f64()
                        .expect("Unable to parse prob to f64 from Decimal");
                    if prob_f64.is_infinite() {
                        prob = Decimal::from(1);
                    }

                    let est_front = front - (Decimal::from(1) - prob) * chg
                        + (back - prob * chg).min(Decimal::ZERO);
                    order_fill_data.cum_qty_chg = est_front.min(event_qty_by_lot_size);
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

            //Don't need convert this because, in theory, this isn't a user input and will be rounded for lot size
            user_fill_data.cum_qty_chg -= ev_qty_decimal;
            user_fill_data.trade_qty_tmp -= ev_qty_decimal;

            res.push(*tick_order);
        }

        res
    }

    fn cancel_order(&mut self, id: &OrderId) {
        self.orders_by_fill_data.remove(id);
    }

    fn new_order(&mut self, id: &OrderId, qty: Qty) {
        let data = LevelChgFillData {
            trade_qty_tmp: Decimal::from(0),
            cum_qty_chg: qty,
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
