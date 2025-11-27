use std::collections::VecDeque;

use anyhow::{anyhow, Result};
use log::warn;
use rust_decimal::prelude::{FromPrimitive, ToPrimitive};
use rust_decimal::Decimal;

use crate::chart::ChartData;
use crate::performance::PerformanceMetrics;
use crate::source::{CandleProducer, EventProducer, EventSrc, EventSrcState};
use crate::types::{Candle, Event, OrderRequest, EVENT_CANDLE};

use super::exchange::{Exchange, OrderId};
use super::{L1Config, L1Order};

pub struct Backtest<S: EventSrc, P: EventProducer + CandleProducer> {
    pub src: S,
    pub exch: Exchange,
    pub curr_ts: i64,
    pub ev_queue: VecDeque<Event>,
    pub line_chunk: usize,
    pub performance: PerformanceMetrics,
    pub position: Decimal,
    pub parser: P,
    pub buf: String,
    pub last_candle: Option<Candle>,
    pub candle_queue: VecDeque<Candle>,
}

impl<S: EventSrc, P: EventProducer + CandleProducer> Backtest<S, P> {
    pub fn new(config: &L1Config<P>, src: S) -> Self {
        let performance = PerformanceMetrics::new(config.risk_free_rate, config.return_window);

        Self {
            src,
            exch: Exchange::new(config),
            curr_ts: config.start_ts,
            ev_queue: VecDeque::with_capacity(1_024),
            line_chunk: config.lines_read_per_tick,
            performance,
            position: Decimal::ZERO,
            parser: config.parser.clone(),
            buf: String::with_capacity(1_024),
            last_candle: None,
            candle_queue: VecDeque::with_capacity(1_024),
        }
    }

    pub fn last_candle(&self) -> &Option<Candle> {
        &self.last_candle
    }

    pub fn current_timestamp(&self) -> i64 {
        self.curr_ts
    }

    pub fn get_order(&self, oid: &OrderId) -> Option<L1Order> {
        if let Some(order) = self.exch.get_order(oid) {
            return Some(order.clone().into());
        }
        None
    }

    pub fn bbo(&self) -> (f64, f64) {
        let (decimal_bid, decimal_ask) = self.exch.bbo();
        (
            decimal_bid
                .normalize()
                .to_f64()
                .expect("Failed to parse bid from Decimal to f64"),
            decimal_ask
                .normalize()
                .to_f64()
                .expect("Failed to parse ask from Decimal to f64"),
        )
    }

    pub fn execute_market_order(&mut self, order: OrderRequest) -> u64 {
        let order_id = self.exch.execute_order(order);
        if let Some(executed_order) = self.exch.get_order(&order_id) {
            let qty = executed_order.qty;
            match executed_order.side {
                crate::types::Side::Buy => self.position += qty,
                crate::types::Side::Sell => self.position -= qty,
            }
        }
        order_id
    }

    fn update_performance_metrics(&mut self) {
        let (bid, ask) = self.bbo();

        let mid_price = (bid + ask) / 2.0;
        let mid_price_decimal = Decimal::from_f64(mid_price).unwrap();

        self.performance
            .update(self.curr_ts, self.position, mid_price_decimal);
    }

    pub fn get_performance_metrics(&self) -> &PerformanceMetrics {
        &self.performance
    }

    pub fn get_position(&self) -> f64 {
        self.position.to_f64().unwrap()
    }

    pub fn generate_chart(&self, output_path: &str) -> Result<()> {
        let chart_data = ChartData::from_performance_metrics(&self.performance);
        chart_data
            .create_multi_chart(output_path)
            .map_err(|e| anyhow!("Failed to create chart: {}", e))?;
        Ok(())
    }

    pub fn get_chart_data(&self) -> ChartData {
        ChartData::from_performance_metrics(&self.performance)
    }
}

impl<S: EventSrc, P: EventProducer + CandleProducer> Backtest<S, P> {
    //For L1, the dataset sets the time so we step over each event
    pub fn step(&mut self) -> Result<()> {
        if self.ev_queue.front().is_none() {
            for i in 0..self.line_chunk {
                if let Some(src_state) = self.src.pop(&mut self.buf) {
                    match src_state {
                        EventSrcState::Empty => {
                            if self.ev_queue.is_empty() {
                                return Err(anyhow!("No more events"));
                            }
                        }
                        EventSrcState::Active => {
                            //TODO: state is shared between thse two parsers
                            if let Err(e) =
                                self.parser.parse_candle(&self.buf, &mut self.candle_queue)
                            {
                                warn!("Failed to parse line: {}", e);
                            }

                            if let Err(e) = self.parser.parse_line(&self.buf, &mut self.ev_queue) {
                                warn!("Failed to parse line: {}", e);
                            }
                        }
                    }
                }
            }
        }

        if let Some(event) = self.ev_queue.pop_front() {
            self.curr_ts = event.ts;
            if event.typ == EVENT_CANDLE {
                self.exch.update_price(&event);
            }
        }

        if let Some(candle) = self.candle_queue.pop_front() {
            self.last_candle = Some(candle);
        }

        self.update_performance_metrics();

        Ok(())
    }

    pub fn elapse(&mut self, ts: u64) -> Result<()> {
        let mut sim_ended = false;
        let end_time = self.curr_ts + ts as i64;

        while self.curr_ts < end_time {
            if let Some(peek_event) = self.ev_queue.front() {
                if peek_event.ts <= end_time {
                    let event = self.ev_queue.pop_front().unwrap();
                    self.curr_ts = event.ts;
                    if event.typ == EVENT_CANDLE {
                        self.exch.update_price(&event);
                    }
                } else {
                    //We have events in the queue but their timestamp is past elapse
                    self.curr_ts = end_time;
                    return Ok(());
                }
            } else {
                if sim_ended {
                    return Err(anyhow!("No more events"));
                }

                for _i in 0..self.line_chunk {
                    if let Some(src_state) = self.src.pop(&mut self.buf) {
                        match src_state {
                            EventSrcState::Empty => {
                                sim_ended = true;
                            }
                            EventSrcState::Active => {
                                let _ = self.parser.parse_line(&self.buf, &mut self.ev_queue);
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        exchanges::hyperliquid::HyperliquidCandleParser,
        l1::L1ConfigBuilder,
        source::EventVecSource,
        types::{OrderStatus, Side},
    };

    use super::*;

    fn setup() -> Backtest<EventVecSource, HyperliquidCandleParser> {
        let ev0 = r#"1000000000000000000 {"channel":"candle","data":{"T":100,"c":"100.0","h":"100.0","i":"1m","l":"100.0","n":1,"o":"100.0","s":"AAVE","t":100,"v":"0.21"}}"#.to_string();
        let ev1 = r#"1010000000000000000 {"channel":"candle","data":{"T":101,"c":"104.0","h":"104.0","i":"1m","l":"104.0","n":1,"o":"104.0","s":"AAVE","t":101,"v":"0.21"}}"#.to_string();
        let ev2 = r#"1020000000000000000 {"channel":"candle","data":{"T":102,"c":"106.0","h":"106.0","i":"1m","l":"106.0","n":1,"o":"106.0","s":"AAVE","t":102,"v":"0.21"}}"#.to_string();

        let evs = vec![ev0, ev1, ev2];
        let src = EventVecSource::new(evs);
        let cfg = L1ConfigBuilder::new()
            .set_tick_size(0.1)
            .set_start_ts(100)
            .set_return_window(1)
            .set_parser(HyperliquidCandleParser::new())
            .build()
            .unwrap();

        Backtest::new(&cfg, src)
    }

    #[test]
    fn test_market_buy() {
        let mut bt = setup();

        let market_order =
            OrderRequest::new(Side::Buy, 50.0, None, crate::types::OrderType::Market);

        let _ = bt.elapse(1);

        let id = bt.execute_market_order(market_order);
        let order = bt.get_order(&id).unwrap();
        assert_eq!(order.status, OrderStatus::Filled);
        assert_eq!(order.exec_px, 104.0);
        assert_eq!(bt.get_position(), 50.0);
    }

    #[test]
    fn test_market_sell() {
        let mut bt = setup();

        let market_order =
            OrderRequest::new(Side::Sell, 30.0, None, crate::types::OrderType::Market);

        let _ = bt.elapse(1);

        let id = bt.execute_market_order(market_order);
        let order = bt.get_order(&id).unwrap();
        assert_eq!(order.status, OrderStatus::Filled);
        assert_eq!(order.exec_px, 104.0);
        assert_eq!(bt.get_position(), -30.0);
    }

    #[test]
    fn test_multiple_orders() {
        let mut bt = setup();

        // Buy 50 units
        let buy_order = OrderRequest::new(Side::Buy, 50.0, None, crate::types::OrderType::Market);
        let _ = bt.elapse(1);
        let _ = bt.execute_market_order(buy_order);
        assert_eq!(bt.get_position(), 50.0);

        // Sell 20 units
        let sell_order = OrderRequest::new(Side::Sell, 20.0, None, crate::types::OrderType::Market);
        let _ = bt.elapse(1);
        let _ = bt.execute_market_order(sell_order);
        assert_eq!(bt.get_position(), 30.0);

        // Buy another 10 units
        let buy_order2 = OrderRequest::new(Side::Buy, 10.0, None, crate::types::OrderType::Market);
        let _ = bt.elapse(1);
        let _ = bt.execute_market_order(buy_order2);
        assert_eq!(bt.get_position(), 40.0);
    }

    #[test]
    fn test_position_after_elapse() {
        let mut bt = setup();

        // Initial position should be 0
        assert_eq!(bt.get_position(), 0.0);

        // Execute a buy order
        let buy_order = OrderRequest::new(Side::Buy, 25.0, None, crate::types::OrderType::Market);
        let _ = bt.elapse(1);
        let _ = bt.execute_market_order(buy_order);
        assert_eq!(bt.get_position(), 25.0);

        // Position should remain the same after elapse
        let _ = bt.elapse(1);
        assert_eq!(bt.get_position(), 25.0);
    }
}
