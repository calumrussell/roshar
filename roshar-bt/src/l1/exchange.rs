use std::{collections::HashMap, str::FromStr};

use rust_decimal::prelude::FromPrimitive;
use rust_decimal::Decimal;

use crate::source::EventProducer;
use crate::types::{price_to_tick, Event, OrderRequest, OrderStatus, Side};

use super::orderbook::OrderBook;
use super::{L1Config, L1OrderInternal};

pub type Price = Decimal;
pub type OrderId = u64;

pub struct Exchange {
    tick_size: Decimal,
    orderbook: OrderBook,
    orders: HashMap<OrderId, L1OrderInternal>,
    order_id: OrderId,
}

impl Exchange {
    pub fn new<P: EventProducer>(config: &L1Config<P>) -> Self {
        Self {
            tick_size: config.tick_size,
            orderbook: OrderBook::new(config.tick_size),
            orders: HashMap::new(),
            order_id: 0,
        }
    }

    pub fn bbo(&self) -> (Price, Price) {
        let best_bid = self.orderbook.get_best_bid();
        let best_ask = self.orderbook.get_best_ask();
        (best_bid, best_ask)
    }

    pub fn update_price(&mut self, ev: &Event) {
        let decimal_px = Decimal::from_str(&ev.px).expect("Unable to parse ev.px to Decimal");
        self.orderbook.update_price(decimal_px, decimal_px);
    }

    pub fn get_order(&self, oid: &OrderId) -> Option<&L1OrderInternal> {
        self.orders.get(oid)
    }

    pub fn execute_order(&mut self, order: OrderRequest) -> u64 {
        let best_price = match order.side {
            Side::Buy => self.orderbook.get_best_ask(),
            Side::Sell => self.orderbook.get_best_bid(),
        };

        let decimal_qty =
            Decimal::from_f64(order.qty).expect("Unable to parse order.qty to Decimal");

        let oid = self.order_id;
        let order = L1OrderInternal {
            id: oid,
            exec_px: best_price,
            exec_tick: price_to_tick(best_price, self.tick_size),
            side: order.side,
            qty: decimal_qty,
            status: OrderStatus::Filled,
        };

        self.orders.insert(oid, order);
        self.order_id += 1;
        oid
    }
}
