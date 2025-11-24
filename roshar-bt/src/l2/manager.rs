use std::{collections::HashMap, str::FromStr};

use anyhow::{anyhow, Result};
use rust_decimal::{prelude::FromPrimitive, Decimal};

use crate::{
    source::EventProducer,
    types::{
        arbitrary_f64_qty_to_lot_size, price_to_tick, Event, OrderRequest, OrderStatus, OrderType,
    },
};

use super::{
    exchange::OrderId,
    fill::{FillModel, LevelChgFill},
    orderbook::{L2OrderBookCell, Qty, Tick},
    L2Config, L2OrderInternal,
};

pub struct OrderManager<F: FillModel> {
    fill_model: F,
    orders: HashMap<OrderId, L2OrderInternal>,
    orders_by_level: HashMap<Tick, Vec<OrderId>>,
    order_id_counter: OrderId,
    tick_size: Decimal,
    lot_size: Decimal,
    position: Qty,
}

impl OrderManager<LevelChgFill> {
    pub fn new_with_level_chg_fill<P: EventProducer>(config: &L2Config<P>) -> Self {
        let fill_model = LevelChgFill::new(config);
        Self {
            fill_model,
            orders: HashMap::with_capacity(100),
            orders_by_level: HashMap::with_capacity(100),
            order_id_counter: 0,
            tick_size: config.tick_size,
            lot_size: config.lot_size,
            position: Decimal::ZERO,
        }
    }
}

impl<F: FillModel> OrderManager<F> {
    pub fn get_order(&self, id: &OrderId) -> Option<&L2OrderInternal> {
        self.orders.get(id)
    }

    pub fn get_order_mut(&mut self, id: &OrderId) -> Option<&mut L2OrderInternal> {
        self.orders.get_mut(id)
    }

    pub fn get_order_prio(&self, id: &OrderId) -> Option<&Qty> {
        self.fill_model.get_prio(id)
    }

    pub fn new_order(&mut self, req: OrderRequest) -> OrderId {
        let id = self.order_id_counter;

        let decimal_qty = Decimal::from_f64(req.qty).expect("Unable to parse f64 from order.qty");
        let qty_by_lot_size = arbitrary_f64_qty_to_lot_size(decimal_qty, self.lot_size).normalize();

        let order = match req.typ {
            OrderType::Limit => {
                assert!(req.px.is_some());
                let px = unsafe { req.px.unwrap_unchecked() };
                let decimal_px = Decimal::from_f64(px).expect("Unable to parse req.px to Decimal");

                L2OrderInternal {
                    id,
                    side: req.side,
                    qty: qty_by_lot_size,
                    filled_qty: Decimal::ZERO,
                    order_px: decimal_px,
                    order_tick: price_to_tick(decimal_px, self.tick_size),
                    exec_px: Decimal::ZERO,
                    exec_tick: 0,
                    typ: req.typ,
                    status: OrderStatus::Working,
                }
            }
            OrderType::Market => L2OrderInternal {
                id,
                side: req.side,
                qty: qty_by_lot_size,
                filled_qty: Decimal::ZERO,
                order_px: Decimal::ZERO,
                order_tick: 0,
                exec_px: Decimal::ZERO,
                exec_tick: 0,
                typ: req.typ,
                status: OrderStatus::Working,
            },
        };
        let order_tick = price_to_tick(order.order_px, self.tick_size);

        self.orders.insert(id, order.clone());

        match self.orders_by_level.entry(order_tick) {
            std::collections::hash_map::Entry::Vacant(v) => {
                v.insert(vec![id]);
            }
            std::collections::hash_map::Entry::Occupied(mut v) => {
                let entry = v.get_mut();
                entry.push(id);
            }
        }

        self.fill_model.new_order(&id, qty_by_lot_size);

        self.order_id_counter += 1;

        id
    }

    pub fn cancel_order(&mut self, order_id: &OrderId) -> Result<()> {
        if let Some(order) = self.orders.get_mut(order_id) {
            order.status = OrderStatus::Cancelled;
            self.fill_model.cancel_order(order_id);

            let level = self
                .orders_by_level
                .get_mut(&order.order_tick)
                .expect("Oid present in self.orders but not self.orders_by_level");

            if let Some(index) = level.iter().position(|&x| x == *order_id) {
                level.remove(index);
                return Ok(());
            }
            return Err(anyhow!("Order qty missing from self.orders_by_level"));
        }
        Err(anyhow!("Did not find order"))
    }

    pub fn get_orders_by_level(&self, tick: Tick) -> Option<&Vec<OrderId>> {
        self.orders_by_level.get(&tick)
    }

    pub fn get_position(&self) -> Qty {
        self.position
    }

    pub fn get_order_id_position(&self) -> OrderId {
        self.order_id_counter
    }

    pub fn get_working_orders(&self) -> Vec<OrderId> {
        let mut res = Vec::new();
        for (id, order) in &self.orders {
            if order.status == OrderStatus::Working {
                res.push(*id);
            }
        }
        res
    }

    pub fn update_trade(&mut self, event: Event) -> Vec<OrderId> {
        let ev_px_decimal =
            Decimal::from_str(&event.px).expect("Unable to parse event.px to Decimal");
        let event_tick = price_to_tick(ev_px_decimal, self.tick_size);
        if let Some(tick_orders) = self.orders_by_level.get(&event_tick) {
            return self.fill_model.update_trade(event, tick_orders);
        }
        vec![]
    }

    pub fn update_level(
        &mut self,
        ev: &Event,
        fill_tracker: &mut Vec<OrderId>,
        orderbook: &L2OrderBookCell,
    ) {
        let ev_px_decimal = Decimal::from_str(&ev.px).expect("Unable to parse event.px to Decimal");
        let event_tick = price_to_tick(ev_px_decimal, self.tick_size);
        if let Some(tick_orders) = self.orders_by_level.get(&event_tick) {
            self.fill_model.update_level(ev, orderbook, tick_orders);

            for order_id in tick_orders {
                if let Some(order_prio) = self.fill_model.get_prio(order_id) {
                    if *order_prio < Decimal::ZERO {
                        let order = self.orders.get_mut(order_id).unwrap();
                        let order_qty_by_lot_size =
                            arbitrary_f64_qty_to_lot_size(order.qty, self.lot_size);

                        let filled_qty = (-order_prio).min(order_qty_by_lot_size);
                        order.filled_qty = filled_qty;
                        order.status = OrderStatus::Filled;

                        match order.side {
                            crate::types::Side::Buy => self.position += filled_qty,
                            crate::types::Side::Sell => self.position -= filled_qty,
                        }

                        fill_tracker.push(*order_id);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use rust_decimal::Decimal;

    use crate::exchanges::hyperliquid::HyperliquidParser;
    use crate::l2::manager::OrderManager;
    use crate::l2::orderbook::{L2OrderBook, L2OrderBookCell};
    use crate::l2::{L2Config, L2ConfigBuilder};
    use crate::types::{
        Event, OrderRequest, OrderStatus, Side, EVENT_TRADE_BUY, EVENT_TRADE_SELL,
        EVENT_UPDATE_LEVEL_ASK, EVENT_UPDATE_LEVEL_BID,
    };

    fn l2_orderbook() -> (L2OrderBookCell, L2Config<HyperliquidParser>) {
        let config = L2ConfigBuilder::new()
            .set_lot_size(1.0)
            .set_tick_size(1.0)
            .set_start_ts(100)
            .set_return_window(1)
            .set_parser(HyperliquidParser)
            .build()
            .unwrap();

        let ob = L2OrderBook::new_refcell(&config);
        ob.borrow_mut()
            .update_level(Side::Buy, 100, Decimal::from(100));
        ob.borrow_mut()
            .update_level(Side::Sell, 101, Decimal::from(100));

        (ob, config)
    }

    #[test]
    fn trade_increases_q_position() {
        let (ob, config) = l2_orderbook();
        let mut fill_tracker = Vec::new();

        let buy_order = OrderRequest::new(
            Side::Buy,
            100.0,
            Some(100.0),
            crate::types::OrderType::Limit,
        );

        let sell_order = OrderRequest::new(
            Side::Sell,
            100.0,
            Some(101.0),
            crate::types::OrderType::Limit,
        );

        let mut mgr = OrderManager::new_with_level_chg_fill(&config);
        mgr.new_order(buy_order);
        mgr.new_order(sell_order);

        //Market is 100/101 with 100 each side
        //Trade comes in for 10 on each side
        //Update comes in taking away 10 (so no cancel/adds between)
        let sell_trade = Event::new(EVENT_TRADE_SELL, 101, "100.0", "10.0");
        let buy_trade = Event::new(EVENT_TRADE_BUY, 101, "101.0", "10.0");
        mgr.update_trade(sell_trade);
        mgr.update_trade(buy_trade);

        let update_bid = Event::new(EVENT_UPDATE_LEVEL_BID, 101, "100.0", "90.0");
        let update_ask = Event::new(EVENT_UPDATE_LEVEL_BID, 101, "101.0", "90.0");
        mgr.update_level(&update_bid, &mut fill_tracker, &ob);
        mgr.update_level(&update_ask, &mut fill_tracker, &ob);

        assert!(fill_tracker.is_empty());
        assert!(mgr.get_order_prio(&0).unwrap().le(&Decimal::from(100)));
        assert!(mgr.get_order_prio(&1).unwrap().le(&Decimal::from(100)));
    }

    #[test]
    fn cancel_increases_q_position() {
        let (ob, config) = l2_orderbook();
        let mut fill_tracker = Vec::new();

        let buy_order = OrderRequest::new(
            Side::Buy,
            100.0,
            Some(100.0),
            crate::types::OrderType::Limit,
        );

        let sell_order = OrderRequest::new(
            Side::Sell,
            100.0,
            Some(101.0),
            crate::types::OrderType::Limit,
        );

        let mut mgr = OrderManager::new_with_level_chg_fill(&config);
        mgr.new_order(buy_order);
        mgr.new_order(sell_order);

        //Market is 100/101 with 100 each side
        //Update comes in taking away 10 implying cancel
        let update_bid = Event::new(EVENT_UPDATE_LEVEL_BID, 101, "100.0", "90.0");
        let update_ask = Event::new(EVENT_UPDATE_LEVEL_BID, 101, "101.0", "90.0");
        mgr.update_level(&update_bid, &mut fill_tracker, &ob);
        mgr.update_level(&update_ask, &mut fill_tracker, &ob);

        assert!(fill_tracker.is_empty());
        assert!(mgr.get_order_prio(&0).unwrap().le(&Decimal::from(100)));
        assert!(mgr.get_order_prio(&1).unwrap().le(&Decimal::from(100)));
    }

    #[test]
    fn order_fills_with_big_trade() {
        let (ob, config) = l2_orderbook();
        let mut fill_tracker = Vec::new();

        let buy_order = OrderRequest::new(
            Side::Buy,
            100.0,
            Some(100.0),
            crate::types::OrderType::Limit,
        );

        let sell_order = OrderRequest::new(
            Side::Sell,
            100.0,
            Some(101.0),
            crate::types::OrderType::Limit,
        );

        let mut mgr = OrderManager::new_with_level_chg_fill(&config);
        mgr.new_order(buy_order);
        mgr.new_order(sell_order);

        let sell_trade = Event::new(EVENT_TRADE_SELL, 101, "100.0", "100.0");
        let buy_trade = Event::new(EVENT_TRADE_BUY, 101, "101.0", "100.0");
        mgr.update_trade(sell_trade);
        mgr.update_trade(buy_trade);

        let update_bid = Event::new(EVENT_UPDATE_LEVEL_BID, 101, "100.0", "10.0");
        let update_ask = Event::new(EVENT_UPDATE_LEVEL_ASK, 101, "101.0", "10.0");
        mgr.update_level(&update_bid, &mut fill_tracker, &ob);
        mgr.update_level(&update_ask, &mut fill_tracker, &ob);

        assert!(!fill_tracker.is_empty());
        assert!(mgr.get_order(&0).unwrap().status == OrderStatus::Filled);
        assert!(mgr.get_order(&1).unwrap().status == OrderStatus::Filled);
    }

    #[test]
    fn cancel_order() {
        let (_ob, config) = l2_orderbook();

        let buy_order = OrderRequest::new(
            Side::Buy,
            100.0,
            Some(100.0),
            crate::types::OrderType::Limit,
        );

        let mut mgr = OrderManager::new_with_level_chg_fill(&config);
        let oid = mgr.new_order(buy_order);

        assert!(mgr.cancel_order(&oid).is_ok());

        let order = mgr.get_order(&oid).unwrap();
        assert!(order.status == OrderStatus::Cancelled);
        assert!(mgr.get_order_prio(&oid) == None);

        assert!(mgr.get_orders_by_level(100).unwrap().is_empty());
    }

    #[test]
    fn cancel_order_at_same_level() {
        let (_ob, config) = l2_orderbook();

        let buy_order_0 = OrderRequest::new(
            Side::Buy,
            100.0,
            Some(100.0),
            crate::types::OrderType::Limit,
        );

        let buy_order_1 = OrderRequest::new(
            Side::Buy,
            200.0,
            Some(100.0),
            crate::types::OrderType::Limit,
        );

        let mut mgr = OrderManager::new_with_level_chg_fill(&config);
        let oid_0 = mgr.new_order(buy_order_0);
        let oid_1 = mgr.new_order(buy_order_1);

        assert!(mgr.cancel_order(&oid_0).is_ok());

        let order_0 = mgr.get_order(&oid_0).unwrap();
        assert!(order_0.status == OrderStatus::Cancelled);
        assert!(mgr.get_order_prio(&oid_0) == None);

        let order_1 = mgr.get_order(&oid_1).unwrap();
        assert!(order_1.status != OrderStatus::Cancelled);
        assert!(mgr.get_order_prio(&oid_1).is_some());

        assert!(!mgr.get_orders_by_level(100).unwrap().is_empty());
    }

    #[test]
    fn position_is_tracked() {
        let (ob, config) = l2_orderbook();
        let mut fill_tracker = Vec::new();

        let buy_order =
            OrderRequest::new(Side::Buy, 10.0, Some(100.0), crate::types::OrderType::Limit);

        let sell_order = OrderRequest::new(
            Side::Sell,
            20.0,
            Some(101.0),
            crate::types::OrderType::Limit,
        );

        let mut mgr = OrderManager::new_with_level_chg_fill(&config);

        let _oid_buy = mgr.new_order(buy_order);
        let sell_trade = Event::new(EVENT_TRADE_SELL, 101, "100.0", "110.0");
        mgr.update_trade(sell_trade);
        let update_bid = Event::new(EVENT_UPDATE_LEVEL_BID, 101, "100.0", "10.0");
        mgr.update_level(&update_bid, &mut fill_tracker, &ob);
        assert!(mgr.get_position().eq(&Decimal::from(10)));

        let _oid_sell = mgr.new_order(sell_order);
        let buy_trade = Event::new(EVENT_TRADE_BUY, 101, "101.0", "120.0");
        mgr.update_trade(buy_trade);
        let update_ask = Event::new(EVENT_UPDATE_LEVEL_ASK, 101, "101.0", "10.00");
        mgr.update_level(&update_ask, &mut fill_tracker, &ob);
        assert!(mgr.get_position().eq(&Decimal::from(-10)));

        assert!(!fill_tracker.is_empty());
    }

    #[test]
    fn order_with_incorrect_lot_size_gets_rounded() {
        let (ob, config) = l2_orderbook();
        let mut fill_tracker = Vec::new();

        let buy_order = OrderRequest::new(
            Side::Buy,
            100.890923,
            Some(100.0),
            crate::types::OrderType::Limit,
        );
        let mut mgr = OrderManager::new_with_level_chg_fill(&config);
        mgr.new_order(buy_order);

        let sell_trade = Event::new(EVENT_TRADE_SELL, 101, "100.0", "100.0");
        mgr.update_trade(sell_trade);

        let update_bid = Event::new(EVENT_UPDATE_LEVEL_BID, 101, "100.0", "10.0");
        mgr.update_level(&update_bid, &mut fill_tracker, &ob);

        assert!(!fill_tracker.is_empty());
        assert!(mgr.get_order(&0).unwrap().status == OrderStatus::Filled);
        assert!(mgr.get_order(&0).unwrap().qty.eq(&Decimal::from(100)));
        //This is a higher level of coverage than mgr.tests but required as we need to check filled_qty format
        assert!(mgr.get_order(&0).unwrap().filled_qty.eq(&Decimal::from(90)));
    }
}
