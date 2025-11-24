use std::panic;
use std::str::FromStr;

use anyhow::Result;
use rust_decimal::{dec, Decimal};

use crate::source::EventProducer;
use crate::types::{
    arbitrary_f64_qty_to_lot_size, price_to_tick, Event, OrderRequest, OrderStatus, OrderType,
    Side, EVENT_UPDATE_LEVEL_ASK, EVENT_UPDATE_LEVEL_BID,
};

use super::fill::{FillModel, LevelChgFill};
use super::manager::OrderManager;
use super::orderbook::{L2OrderBook, L2OrderBookCell, Qty};
use super::{L2Config, L2OrderInternal};

pub type Price = Decimal;
pub type OrderId = u64;

pub struct Exchange<F: FillModel> {
    tick_size: Decimal,
    lot_size: Decimal,
    orderbook: L2OrderBookCell,
    order_manager: OrderManager<F>,
}

impl Exchange<LevelChgFill> {
    pub fn new_with_level_chg_fill<P: EventProducer>(config: &L2Config<P>) -> Self {
        let order_manager = OrderManager::new_with_level_chg_fill(config);
        let orderbook = L2OrderBook::new_refcell(config);
        Self {
            tick_size: config.tick_size,
            lot_size: config.lot_size,
            orderbook,
            order_manager,
        }
    }
}

impl<F: FillModel> Exchange<F> {
    pub fn clear_bid(&mut self) {
        self.orderbook.borrow_mut().clear_bid();
    }

    pub fn clear_ask(&mut self) {
        self.orderbook.borrow_mut().clear_ask();
    }

    pub fn clear_bid_level(&mut self, ev: &Event) {
        if let Ok(px) = Decimal::from_str(&ev.px) {
            let tick = price_to_tick(px, self.tick_size);
            self.orderbook.borrow_mut().clear_bid_level(tick);
        }
    }

    pub fn clear_ask_level(&mut self, ev: &Event) {
        if let Ok(px) = Decimal::from_str(&ev.px) {
            let tick = price_to_tick(px, self.tick_size);
            self.orderbook.borrow_mut().clear_ask_level(tick);
        }
    }

    pub fn clear(&mut self) {
        self.orderbook.borrow_mut().clear();
    }

    pub fn bbo(&self) -> (Price, Price) {
        let best_bid = self.orderbook.borrow().get_best_bid();
        let best_ask = self.orderbook.borrow().get_best_ask();
        (best_bid, best_ask)
    }

    fn execute_market_order(&mut self, order: &mut L2OrderInternal) {
        let opposite_side = match order.side {
            Side::Buy => Side::Sell,
            Side::Sell => Side::Buy,
        };

        let best_price = match order.side {
            Side::Buy => self.orderbook.borrow().get_best_ask(),
            Side::Sell => self.orderbook.borrow().get_best_bid(),
        };
        let best_tick = price_to_tick(best_price, self.tick_size);

        if best_price.eq(&dec!(0.0)) {
            return;
        }

        let level_qty = self
            .orderbook
            .borrow()
            .get_level(opposite_side.clone(), best_tick);

        let qty_by_lot_size = arbitrary_f64_qty_to_lot_size(order.qty, self.lot_size);

        if level_qty >= qty_by_lot_size {
            order.filled_qty = qty_by_lot_size;
            order.status = OrderStatus::Filled;
            order.exec_px = best_price;
            order.exec_tick = price_to_tick(best_price, self.tick_size);
        }
    }

    pub fn process_trade(&mut self, trade: Event) {
        self.order_manager.update_trade(trade);
    }

    pub fn update_level(&mut self, update: Event, fill_tracker: &mut Vec<OrderId>) {
        let side = match update.typ {
            EVENT_UPDATE_LEVEL_BID => Side::Buy,
            EVENT_UPDATE_LEVEL_ASK => Side::Sell,
            _ => panic!("update_level should only be called with UPDATE_LEVEL events"),
        };

        let decimal_px =
            Decimal::from_str(&update.px).expect("Unable to parse event.px to Decimal");
        let decimal_qty =
            Decimal::from_str(&update.qty).expect("Unable to parse event.qty to Decimal");
        let qty_by_lot_size = arbitrary_f64_qty_to_lot_size(decimal_qty, self.lot_size);

        let tick = price_to_tick(decimal_px, self.tick_size);
        self.orderbook
            .borrow_mut()
            .update_level(side, tick, qty_by_lot_size);
        self.order_manager
            .update_level(&update, fill_tracker, &self.orderbook);
    }

    pub fn execute_user_order(&mut self, req: OrderRequest) {
        //Lot size checks happen further down call stack in fill model
        let oid = self.order_manager.new_order(req);

        unsafe {
            let order_ptr = match self.order_manager.get_order_mut(&oid) {
                Some(order) => {
                    if order.typ != OrderType::Market {
                        return;
                    }
                    order as *mut _
                }
                None => return,
            };

            // Now we can use self again
            self.execute_market_order(&mut *order_ptr);
        }
    }

    pub fn cancel_order(&mut self, order_id: &OrderId) -> Result<()> {
        //Latency means we can only cancel orders that have been received by the exchange and
        //assigned an order id
        self.order_manager.cancel_order(order_id)
    }

    pub fn get_position(&mut self) -> Qty {
        self.order_manager.get_position()
    }

    pub fn get_level(&self, price: Price, side: Side) -> Qty {
        let tick = price_to_tick(price, self.tick_size);
        self.orderbook.borrow().get_level(side, tick)
    }

    pub fn get_order_id_position(&self) -> OrderId {
        self.order_manager.get_order_id_position()
    }

    pub fn get_orders_by_level(&self, price: Price) -> Option<&Vec<OrderId>> {
        let tick = price_to_tick(price, self.tick_size);
        self.order_manager.get_orders_by_level(tick)
    }

    pub fn get_order(&self, id: &OrderId) -> Option<&L2OrderInternal> {
        self.order_manager.get_order(id)
    }

    pub fn get_working_orders(&self) -> Vec<OrderId> {
        self.order_manager.get_working_orders()
    }
}

#[cfg(test)]
mod tests {
    use crate::exchanges::hyperliquid::HyperliquidParser;
    use crate::l2::fill::LevelChgFill;
    use crate::l2::L2ConfigBuilder;
    use crate::types::EVENT_CLEAR_LEVEL_ASK;

    use super::*;

    fn setup_basic_orderbook() -> Exchange<LevelChgFill> {
        let config: L2Config<HyperliquidParser> = L2ConfigBuilder::new()
            .set_lot_size(1.0)
            .set_tick_size(0.01)
            .set_start_ts(100)
            .set_return_window(1)
            .set_parser(HyperliquidParser)
            .build()
            .unwrap();

        let mut fill_tracker = Vec::new();

        let bid_event = Event::new(EVENT_UPDATE_LEVEL_BID, 100, "9.99", "100.0");
        let ask_event = Event::new(EVENT_UPDATE_LEVEL_ASK, 100, "10.01", "100.0");

        let mut exchange = Exchange::new_with_level_chg_fill(&config);
        exchange.update_level(bid_event, &mut fill_tracker);
        exchange.update_level(ask_event, &mut fill_tracker);
        exchange
    }

    #[test]
    fn test_market_buy_with_sufficient_liquidity() {
        let mut exchange = setup_basic_orderbook();

        let market_order = OrderRequest::new(Side::Buy, 50.0, None, OrderType::Market);

        exchange.execute_user_order(market_order);
        let order = exchange.get_order(&0).unwrap();
        assert_eq!(order.status, OrderStatus::Filled);
        assert_eq!(order.exec_px, dec!(10.01));
        assert_eq!(order.filled_qty, dec!(50.0));
    }

    #[test]
    fn test_market_sell_with_sufficient_liquidity() {
        let mut exchange = setup_basic_orderbook();

        let market_order = OrderRequest::new(Side::Sell, 50.0, None, OrderType::Market);

        exchange.execute_user_order(market_order);
        let order = exchange.get_order(&0).unwrap();
        assert_eq!(order.status, OrderStatus::Filled);
        assert_eq!(order.exec_px, dec!(9.99));
        assert_eq!(order.filled_qty, dec!(50.0));
    }

    #[test]
    fn test_market_buy_without_sufficient_liquidity() {
        let mut exchange = setup_basic_orderbook();

        let market_order = OrderRequest::new(Side::Buy, 150.0, None, OrderType::Market);

        exchange.execute_user_order(market_order);

        let order = exchange.get_order(&0).unwrap();
        assert_eq!(order.status, OrderStatus::Working);
        assert_eq!(order.exec_px, Decimal::ZERO);

        let best_ask_price = exchange.orderbook.borrow().get_best_ask();
        let best_ask_tick = price_to_tick(best_ask_price, dec!(0.01));
        assert_eq!(best_ask_price, dec!(10.01));

        let remaining_qty = exchange
            .orderbook
            .borrow()
            .get_level(Side::Sell, best_ask_tick);
        assert_eq!(remaining_qty, dec!(100)); // Unchanged
    }

    #[test]
    fn test_market_order_with_empty_orderbook() {
        let config: L2Config<HyperliquidParser> = L2ConfigBuilder::new()
            .set_lot_size(1.0)
            .set_tick_size(1.0)
            .set_start_ts(100)
            .set_return_window(1)
            .set_parser(HyperliquidParser)
            .build()
            .unwrap();

        let mut exchange = Exchange::new_with_level_chg_fill(&config);
        let market_order = OrderRequest::new(Side::Buy, 100.0, None, OrderType::Market);

        exchange.execute_user_order(market_order);

        let order = exchange.get_order(&0).unwrap();
        assert_eq!(order.status, OrderStatus::Working);
        assert_eq!(order.exec_px, Decimal::ZERO);

        assert_eq!(exchange.orderbook.borrow().get_best_ask(), Decimal::ZERO);
        assert_eq!(exchange.orderbook.borrow().get_best_bid(), Decimal::ZERO);
    }

    #[test]
    fn test_market_order_with_incorrect_lot_size() {
        let mut exchange = setup_basic_orderbook();

        let market_order = OrderRequest::new(Side::Buy, 100.547807, None, OrderType::Market);

        exchange.execute_user_order(market_order);

        let order = exchange.get_order(&0).unwrap();
        assert_eq!(order.status, OrderStatus::Filled);
        assert_eq!(order.exec_px, dec!(10.01));
        assert_eq!(order.filled_qty, dec!(100));
    }

    #[test]
    fn test_clear_order_with_bad_price_formatting() {
        let mut exchange = setup_basic_orderbook();

        let level_before = exchange.get_level(dec!(10.01), Side::Sell);
        assert_ne!(level_before, Decimal::ZERO);

        //Rounds up to 10.02, so doesn't clear 10.01
        let ask_event_wrong = Event::new(EVENT_CLEAR_LEVEL_ASK, 100, "10.01678780", "0.0");
        exchange.clear_ask_level(&ask_event_wrong);
        let level_after_wrong = exchange.get_level(dec!(10.01), Side::Sell);
        assert_ne!(level_after_wrong, Decimal::ZERO);

        //Rounds down to 10.01, does clear 10.01
        let ask_event_right = Event::new(EVENT_CLEAR_LEVEL_ASK, 100, "10.0111234154", "0.0");
        exchange.clear_ask_level(&ask_event_right);
        let level_after_right = exchange.get_level(dec!(10.01), Side::Sell);
        assert_eq!(level_after_right, Decimal::ZERO);
    }

    #[test]
    fn test_update_level_with_bad_price_and_qty_formatting() {
        let mut exchange = setup_basic_orderbook();
        let mut fill_tracker = Vec::new();

        let level_before = exchange.get_level(dec!(10.01), Side::Sell);
        assert_ne!(level_before, Decimal::ZERO);

        //Rounds up to 10.02 and down to 10, doesn't update level
        let ask_event_wrong = Event::new(EVENT_UPDATE_LEVEL_ASK, 100, "10.01678780", "10.00005");
        exchange.update_level(ask_event_wrong, &mut fill_tracker);
        let level_after_wrong = exchange.get_level(dec!(10.01), Side::Sell);
        assert_eq!(level_after_wrong, dec!(100));

        //Rounds down to 10.01 and down to 10, updates level
        let ask_event_round_down =
            Event::new(EVENT_UPDATE_LEVEL_ASK, 100, "10.0111234154", "10.00005");
        exchange.update_level(ask_event_round_down, &mut fill_tracker);
        let level_after_round_down = exchange.get_level(dec!(10.01), Side::Sell);
        assert_eq!(level_after_round_down, dec!(10));

        //Rounds down to 10.01 and down to 11, updates level
        let ask_event_round_floor =
            Event::new(EVENT_UPDATE_LEVEL_ASK, 100, "10.0111234154", "11.6578499");
        exchange.update_level(ask_event_round_floor, &mut fill_tracker);
        let level_after_round_floor = exchange.get_level(dec!(10.01), Side::Sell);
        assert_eq!(level_after_round_floor, dec!(11));
    }
}
