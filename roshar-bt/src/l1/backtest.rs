use std::collections::{HashMap, VecDeque};

use anyhow::{anyhow, Result};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;

use crate::chart::ChartData;
use crate::performance::PerformanceMetrics;
use crate::source::{EventFeed, FeedState};
use crate::types::{Candle, Event, OrderRequest, EVENT_CANDLE};

use super::exchange::{Exchange, OrderId};
use super::{L1Config, L1Order};

pub struct Backtest<F: EventFeed> {
    pub feed: F,
    /// Per-symbol exchanges (created on first event for that symbol).
    pub exchanges: HashMap<String, Exchange>,
    pub curr_ts: i64,
    pub ev_queue: VecDeque<Event>,
    pub line_chunk: usize,
    pub performance: PerformanceMetrics,
    /// Per-symbol positions.
    pub positions: HashMap<String, Decimal>,
    /// Per-symbol last received candle.
    pub last_candles: HashMap<String, Candle>,
    pub candle_queue: VecDeque<Candle>,
    tick_size: Decimal,
}

impl<F: EventFeed> Backtest<F> {
    pub fn new(config: &L1Config, feed: F) -> Self {
        let performance = PerformanceMetrics::new(config.risk_free_rate, config.return_window);

        Self {
            feed,
            exchanges: HashMap::new(),
            curr_ts: config.start_ts,
            ev_queue: VecDeque::with_capacity(1_024),
            line_chunk: config.lines_read_per_tick,
            performance,
            positions: HashMap::new(),
            last_candles: HashMap::new(),
            candle_queue: VecDeque::with_capacity(1_024),
            tick_size: config.tick_size,
        }
    }

    /// Returns the last candle for the given symbol, if any.
    pub fn last_candle(&self, symbol: &str) -> Option<&Candle> {
        self.last_candles.get(symbol)
    }

    pub fn current_timestamp(&self) -> i64 {
        self.curr_ts
    }

    pub fn get_order(&self, symbol: &str, oid: &OrderId) -> Option<L1Order> {
        if let Some(exch) = self.exchanges.get(symbol) {
            if let Some(order) = exch.get_order(oid) {
                return Some(order.clone().into());
            }
        }
        None
    }

    /// Returns `(bid, ask)` mid-price for the given symbol.
    pub fn bbo(&self, symbol: &str) -> (f64, f64) {
        if let Some(exch) = self.exchanges.get(symbol) {
            let (decimal_bid, decimal_ask) = exch.bbo();
            return (
                decimal_bid
                    .normalize()
                    .to_f64()
                    .expect("Failed to parse bid from Decimal to f64"),
                decimal_ask
                    .normalize()
                    .to_f64()
                    .expect("Failed to parse ask from Decimal to f64"),
            );
        }
        (0.0, 0.0)
    }

    /// Execute a market order for the given symbol.  Returns the order id.
    pub fn execute_market_order(&mut self, symbol: &str, order: OrderRequest) -> u64 {
        let tick_size = self.tick_size;
        let exch = self
            .exchanges
            .entry(symbol.to_string())
            .or_insert_with(|| Exchange::new(tick_size));

        let order_id = exch.execute_order(order);
        if let Some(executed_order) = exch.get_order(&order_id) {
            let qty = executed_order.qty;
            let position = self
                .positions
                .entry(symbol.to_string())
                .or_insert(Decimal::ZERO);
            match executed_order.side {
                crate::types::Side::Buy => *position += qty,
                crate::types::Side::Sell => *position -= qty,
            }
        }
        order_id
    }

    fn update_performance_metrics(&mut self) {
        // Collect current mid-prices per symbol.
        let mut prices: HashMap<String, Decimal> = HashMap::new();
        for (symbol, exch) in &self.exchanges {
            let (bid, ask) = exch.bbo();
            let mid = (bid + ask) / Decimal::TWO;
            prices.insert(symbol.clone(), mid);
        }
        self.performance
            .update_portfolio(self.curr_ts, &self.positions, &prices);
    }

    pub fn get_performance_metrics(&self) -> &PerformanceMetrics {
        &self.performance
    }

    /// Returns the position for the given symbol (0 if no position).
    pub fn get_position(&self, symbol: &str) -> f64 {
        self.positions
            .get(symbol)
            .copied()
            .unwrap_or(Decimal::ZERO)
            .to_f64()
            .unwrap_or(0.0)
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

impl<F: EventFeed> Backtest<F> {
    //For L1, the dataset sets the time so we step over each event
    pub fn step(&mut self) -> Result<()> {
        if self.ev_queue.front().is_none() {
            match self
                .feed
                .fill(&mut self.ev_queue, &mut self.candle_queue, self.line_chunk)
            {
                FeedState::Empty => {
                    if self.ev_queue.is_empty() {
                        return Err(anyhow!("No more events"));
                    }
                }
                FeedState::Active => {}
            }
        }

        if let Some(event) = self.ev_queue.pop_front() {
            self.curr_ts = event.ts;
            if event.typ == EVENT_CANDLE {
                let symbol = event.symbol.clone();
                let tick_size = self.tick_size;
                let exch = self
                    .exchanges
                    .entry(symbol)
                    .or_insert_with(|| Exchange::new(tick_size));
                exch.update_price(&event);
            }
        }

        if let Some(candle) = self.candle_queue.pop_front() {
            let symbol = candle.symbol.clone();
            self.last_candles.insert(symbol, candle);
        }

        self.update_performance_metrics();

        Ok(())
    }

    pub fn elapse(&mut self, ts: u64) -> Result<()> {
        self.elapse_inner(ts, None)
    }

    pub fn elapse_with_buffer(&mut self, ts: u64, events_out: &mut Vec<Event>) -> Result<()> {
        self.elapse_inner(ts, Some(events_out))
    }

    fn elapse_inner(&mut self, ts: u64, mut events_out: Option<&mut Vec<Event>>) -> Result<()> {
        if let Some(buf) = events_out.as_deref_mut() {
            buf.clear();
        }

        let mut sim_ended = false;
        let end_time = self.curr_ts + ts as i64;

        while self.curr_ts < end_time {
            if let Some(peek_event) = self.ev_queue.front() {
                if peek_event.ts <= end_time {
                    let event = self.ev_queue.pop_front().unwrap();
                    self.curr_ts = event.ts;
                    if event.typ == EVENT_CANDLE {
                        let symbol = event.symbol.clone();
                        let tick_size = self.tick_size;
                        let exch = self
                            .exchanges
                            .entry(symbol)
                            .or_insert_with(|| Exchange::new(tick_size));
                        exch.update_price(&event);
                    }
                    if let Some(buf) = events_out.as_deref_mut() {
                        buf.push(event);
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

                match self
                    .feed
                    .fill(&mut self.ev_queue, &mut self.candle_queue, self.line_chunk)
                {
                    FeedState::Empty => {
                        sim_ended = true;
                    }
                    FeedState::Active => {
                        // Drain candles into last_candles map.
                        while let Some(candle) = self.candle_queue.pop_front() {
                            let sym = candle.symbol.clone();
                            self.last_candles.insert(sym, candle);
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
        source::{EventVecSource, ParsedCandleFeed, StreamingEventFeed},
        types::{OrderStatus, Side},
    };

    use super::*;

    fn setup() -> Backtest<ParsedCandleFeed<EventVecSource, HyperliquidCandleParser>> {
        let ev0 = r#"1000000000000000000 {"channel":"candle","data":{"T":100,"c":"100.0","h":"100.0","i":"1m","l":"100.0","n":1,"o":"100.0","s":"AAVE","t":100,"v":"0.21"}}"#.to_string();
        let ev1 = r#"1010000000000000000 {"channel":"candle","data":{"T":101,"c":"104.0","h":"104.0","i":"1m","l":"104.0","n":1,"o":"104.0","s":"AAVE","t":101,"v":"0.21"}}"#.to_string();
        let ev2 = r#"1020000000000000000 {"channel":"candle","data":{"T":102,"c":"106.0","h":"106.0","i":"1m","l":"106.0","n":1,"o":"106.0","s":"AAVE","t":102,"v":"0.21"}}"#.to_string();

        let evs = vec![ev0, ev1, ev2];
        let src = EventVecSource::new(evs);
        let feed = ParsedCandleFeed::new(src, HyperliquidCandleParser::new());
        let cfg = L1ConfigBuilder::new()
            .set_tick_size(0.1)
            .set_start_ts(100)
            .set_return_window(1)
            .build()
            .unwrap();

        Backtest::new(&cfg, feed)
    }

    #[test]
    fn test_market_buy() {
        let mut bt = setup();

        let market_order =
            OrderRequest::new(Side::Buy, 50.0, None, crate::types::OrderType::Market);

        let _ = bt.elapse(1);

        let id = bt.execute_market_order("AAVE", market_order);
        let order = bt.get_order("AAVE", &id).unwrap();
        assert_eq!(order.status, OrderStatus::Filled);
        assert_eq!(order.exec_px, 104.0);
        assert_eq!(bt.get_position("AAVE"), 50.0);
    }

    #[test]
    fn test_market_sell() {
        let mut bt = setup();

        let market_order =
            OrderRequest::new(Side::Sell, 30.0, None, crate::types::OrderType::Market);

        let _ = bt.elapse(1);

        let id = bt.execute_market_order("AAVE", market_order);
        let order = bt.get_order("AAVE", &id).unwrap();
        assert_eq!(order.status, OrderStatus::Filled);
        assert_eq!(order.exec_px, 104.0);
        assert_eq!(bt.get_position("AAVE"), -30.0);
    }

    #[test]
    fn test_multiple_orders() {
        let mut bt = setup();

        // Buy 50 units
        let buy_order = OrderRequest::new(Side::Buy, 50.0, None, crate::types::OrderType::Market);
        let _ = bt.elapse(1);
        let _ = bt.execute_market_order("AAVE", buy_order);
        assert_eq!(bt.get_position("AAVE"), 50.0);

        // Sell 20 units
        let sell_order = OrderRequest::new(Side::Sell, 20.0, None, crate::types::OrderType::Market);
        let _ = bt.elapse(1);
        let _ = bt.execute_market_order("AAVE", sell_order);
        assert_eq!(bt.get_position("AAVE"), 30.0);

        // Buy another 10 units
        let buy_order2 = OrderRequest::new(Side::Buy, 10.0, None, crate::types::OrderType::Market);
        let _ = bt.elapse(1);
        let _ = bt.execute_market_order("AAVE", buy_order2);
        assert_eq!(bt.get_position("AAVE"), 40.0);
    }

    #[test]
    fn test_position_after_elapse() {
        let mut bt = setup();

        // Initial position should be 0
        assert_eq!(bt.get_position("AAVE"), 0.0);

        // Execute a buy order
        let buy_order = OrderRequest::new(Side::Buy, 25.0, None, crate::types::OrderType::Market);
        let _ = bt.elapse(1);
        let _ = bt.execute_market_order("AAVE", buy_order);
        assert_eq!(bt.get_position("AAVE"), 25.0);

        // Position should remain the same after elapse
        let _ = bt.elapse(1);
        assert_eq!(bt.get_position("AAVE"), 25.0);
    }

    #[test]
    fn test_multi_symbol_positions() {
        // Two symbols with separate candle data interleaved by timestamp
        let aave0 = r#"1000000000000000000 {"channel":"candle","data":{"T":100,"c":"100.0","h":"100.0","i":"1m","l":"100.0","n":1,"o":"100.0","s":"AAVE","t":100,"v":"0.21"}}"#.to_string();
        let btc0 =  r#"1005000000000000000 {"channel":"candle","data":{"T":200,"c":"50000.0","h":"50000.0","i":"1m","l":"50000.0","n":1,"o":"50000.0","s":"BTC","t":200,"v":"1.0"}}"#.to_string();
        let aave1 = r#"1010000000000000000 {"channel":"candle","data":{"T":101,"c":"104.0","h":"104.0","i":"1m","l":"104.0","n":1,"o":"104.0","s":"AAVE","t":101,"v":"0.21"}}"#.to_string();
        let btc1 =  r#"1015000000000000000 {"channel":"candle","data":{"T":201,"c":"51000.0","h":"51000.0","i":"1m","l":"51000.0","n":1,"o":"51000.0","s":"BTC","t":201,"v":"1.0"}}"#.to_string();

        use crate::source::MultiplexedFeed;

        let src_aave = EventVecSource::new(vec![aave0, aave1]);
        let src_btc = EventVecSource::new(vec![btc0, btc1]);

        let feed_aave: Box<dyn EventFeed> =
            Box::new(ParsedCandleFeed::new(src_aave, HyperliquidCandleParser::new()));
        let feed_btc: Box<dyn EventFeed> =
            Box::new(ParsedCandleFeed::new(src_btc, HyperliquidCandleParser::new()));

        let mux = MultiplexedFeed::new(vec![feed_aave, feed_btc]);

        let cfg = L1ConfigBuilder::new()
            .set_tick_size(0.1)
            .set_start_ts(0)
            .set_return_window(1)
            .build()
            .unwrap();

        let mut bt = Backtest::new(&cfg, mux);

        // Process events; each symbol's price updates separately.
        for _ in 0..4 {
            let _ = bt.step();
        }

        // Trade on AAVE after prices have been seen
        let aave_buy = OrderRequest::new(Side::Buy, 10.0, None, crate::types::OrderType::Market);
        let _ = bt.execute_market_order("AAVE", aave_buy);
        assert_eq!(bt.get_position("AAVE"), 10.0);
        assert_eq!(bt.get_position("BTC"), 0.0);

        let btc_sell = OrderRequest::new(Side::Sell, 2.0, None, crate::types::OrderType::Market);
        let _ = bt.execute_market_order("BTC", btc_sell);
        assert_eq!(bt.get_position("AAVE"), 10.0);
        assert_eq!(bt.get_position("BTC"), -2.0);
    }

    #[test]
    fn test_l1_backtest_with_streaming_feed_and_candles() {
        use crate::source::FeedState;
        use crate::types::{Candle, Event, EVENT_CANDLE};
        use std::collections::VecDeque;

        let mut done = false;
        let producer = move |events: &mut VecDeque<Event>, candles: &mut VecDeque<Candle>, _count| {
            if done {
                FeedState::Empty
            } else {
                events.push_back(Event::new_with_symbol(EVENT_CANDLE, 100, "100.0", "1.0", "AAVE"));
                events.push_back(Event::new_with_symbol(EVENT_CANDLE, 200, "101.0", "1.0", "AAVE"));
                candles.push_back(Candle::from_str_with_symbol("100.0", "100.0", "100.0", "100.0", &100, "AAVE"));
                candles.push_back(Candle::from_str_with_symbol("101.0", "99.0", "100.0", "101.0", &200, "AAVE"));
                done = true;
                FeedState::Active
            }
        };

        let feed = StreamingEventFeed::new(producer);
        let cfg = L1ConfigBuilder::new()
            .set_tick_size(0.1)
            .set_start_ts(0)
            .set_lines_read_per_tick(2)
            .set_return_window(1)
            .build()
            .unwrap();
        let mut bt = Backtest::new(&cfg, feed);

        bt.elapse(200).unwrap();
        let last = bt.last_candle("AAVE");
        assert!(last.is_some(), "last candle should be updated from streaming feed");
    }
}
