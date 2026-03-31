use std::collections::{HashMap, VecDeque};

use anyhow::{anyhow, Ok, Result};
use rust_decimal::prelude::{FromPrimitive, ToPrimitive};
use rust_decimal::Decimal;

use crate::performance::PerformanceMetrics;
use crate::source::{EventFeed, FeedState};
use crate::types::{
    Candle, Event, OrderRequest, EVENT_CLEAR_BOOK, EVENT_CLEAR_LEVEL_ASK, EVENT_CLEAR_LEVEL_BID,
    EVENT_CLEAR_SIDE_ASK, EVENT_CLEAR_SIDE_BID, EVENT_TRADE_BUY, EVENT_TRADE_SELL,
    EVENT_UPDATE_LEVEL_ASK, EVENT_UPDATE_LEVEL_BID,
};

use super::exchange::{Exchange, OrderId};
use super::fill::{FillModel, LevelChgFill};
use super::{L2Config, L2Order};

pub enum LatencyModel {
    Instant,
    Fixed(u64), // delay in milliseconds
}

impl LatencyModel {
    pub fn calc_delay(&self) -> u64 {
        match self {
            LatencyModel::Instant => 0,
            LatencyModel::Fixed(ms) => *ms,
        }
    }
}

/// Factory trait so `Backtest` can create per-symbol exchanges on demand without
/// knowing the concrete `FillModel` type at every call site.
pub trait ExchangeFactory {
    fn create(tick_size: Decimal, lot_size: Decimal) -> Self;
}

impl ExchangeFactory for Exchange<LevelChgFill> {
    fn create(tick_size: Decimal, lot_size: Decimal) -> Self {
        Exchange::new_with_level_chg_fill(tick_size, lot_size)
    }
}

pub struct Backtest<Feed: EventFeed, Fil: FillModel> {
    pub feed: Feed,
    /// Per-symbol exchanges (created lazily on first event for that symbol).
    pub exchanges: HashMap<String, Exchange<Fil>>,
    pub curr_ts: i64,
    pub ev_queue: VecDeque<Event>,
    pub line_chunk: usize,
    /// Pending orders: (symbol, OrderRequest).
    pub order_buffer: VecDeque<(String, OrderRequest)>,
    pub latency_model: LatencyModel,
    /// Per-symbol fill trackers cleared each tick.
    pub tick_fill_tracker: HashMap<String, Vec<OrderId>>,
    pub performance: PerformanceMetrics,
    candle_sink: VecDeque<Candle>,
    tick_size: Decimal,
    lot_size: Decimal,
    _fil: std::marker::PhantomData<Fil>,
}

impl<Feed: EventFeed> Backtest<Feed, LevelChgFill> {
    pub fn new_with_level_chg_fill(config: &L2Config, feed: Feed) -> Self {
        let performance = PerformanceMetrics::new(config.risk_free_rate, config.return_window);

        Self {
            feed,
            exchanges: HashMap::new(),
            curr_ts: config.start_ts,
            ev_queue: VecDeque::with_capacity(1_024),
            line_chunk: config.lines_read_per_tick,
            order_buffer: VecDeque::with_capacity(config.order_buffer_start_size),
            latency_model: LatencyModel::Instant,
            tick_fill_tracker: HashMap::new(),
            performance,
            candle_sink: VecDeque::new(),
            tick_size: config.tick_size,
            lot_size: config.lot_size,
            _fil: std::marker::PhantomData,
        }
    }

    pub fn set_latency_model(&mut self, model: LatencyModel) {
        self.latency_model = model;
    }

    pub fn get_working_orders(&self, symbol: &str) -> Vec<OrderId> {
        self.exchanges
            .get(symbol)
            .map(|e| e.get_working_orders())
            .unwrap_or_default()
    }

    pub fn get_performance_metrics(&self) -> &PerformanceMetrics {
        &self.performance
    }
}

impl<Feed: EventFeed, Fil: FillModel + 'static> Backtest<Feed, Fil>
where
    Exchange<Fil>: ExchangeFactory,
{
    pub fn current_timestamp(&self) -> i64 {
        self.curr_ts
    }

    pub fn last_trades(&self, symbol: &str) -> &[OrderId] {
        self.tick_fill_tracker
            .get(symbol)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Returns `(bid, ask)` for the given symbol, or `(0.0, 0.0)` if unknown.
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

    pub fn submit_order(&mut self, symbol: &str, mut req: OrderRequest) {
        req.set_time(self.curr_ts);
        self.order_buffer.push_back((symbol.to_string(), req));
    }

    pub fn cancel_order(&mut self, symbol: &str, order_id: &OrderId) -> Result<()> {
        if let Some(exch) = self.exchanges.get_mut(symbol) {
            return exch.cancel_order(order_id);
        }
        Err(anyhow!("Unknown symbol: {}", symbol))
    }

    pub fn get_position(&mut self, symbol: &str) -> f64 {
        self.exchanges
            .get_mut(symbol)
            .map(|e| {
                e.get_position()
                    .normalize()
                    .to_f64()
                    .expect("Unable to parse Decimal position to f64")
            })
            .unwrap_or(0.0)
    }

    pub fn update_performance_metrics(&mut self) {
        let mut positions: HashMap<String, Decimal> = HashMap::new();
        let mut prices: HashMap<String, Decimal> = HashMap::new();

        for (symbol, exch) in self.exchanges.iter_mut() {
            let pos = exch.get_position();
            positions.insert(symbol.clone(), pos);
            let (bid, ask) = exch.bbo();
            let mid = (bid + ask) / Decimal::TWO;
            prices.insert(symbol.clone(), mid);
        }

        self.performance
            .update_portfolio(self.curr_ts, &positions, &prices);
    }

    pub fn get_order_id_position(&self, symbol: &str) -> OrderId {
        self.exchanges
            .get(symbol)
            .map(|e| e.get_order_id_position())
            .unwrap_or(0)
    }

    pub fn get_order(&self, symbol: &str, order_id: &OrderId) -> Option<L2Order> {
        if let Some(exch) = self.exchanges.get(symbol) {
            if let Some(order) = exch.get_order(order_id) {
                return Some(order.clone().into());
            }
        }
        None
    }

    pub fn get_orders_by_level(&self, symbol: &str, price: f64) -> Option<&Vec<OrderId>> {
        let decimal_price = Decimal::from_f64(price).expect("Unable to parse f64 price to Decimal");
        self.exchanges
            .get(symbol)
            .and_then(|e| e.get_orders_by_level(decimal_price))
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

        // Clear per-symbol fill trackers.
        for tracker in self.tick_fill_tracker.values_mut() {
            tracker.clear();
        }

        let mut sim_ended = false;
        let end_time = self.curr_ts + ts as i64;
        // When true, loop back once more to flush any orders that cleared latency
        // at the final curr_ts, then return.
        let mut should_return = false;

        loop {
            // Submit buffered orders that have cleared latency.
            while let Some(front) = self.order_buffer.front() {
                if (front.1.get_time() + self.latency_model.calc_delay() as i64) <= self.curr_ts {
                    let (symbol, order) = self.order_buffer.pop_front().unwrap();
                    let tick_size = self.tick_size;
                    let lot_size = self.lot_size;
                    let exch = self
                        .exchanges
                        .entry(symbol)
                        .or_insert_with(|| Exchange::<Fil>::create(tick_size, lot_size));
                    exch.execute_user_order(order);
                } else {
                    break;
                }
            }

            if should_return {
                return Ok(());
            }

            if let Some(peek_event) = self.ev_queue.front() {
                if peek_event.ts <= end_time {
                    let event = self.ev_queue.pop_front().unwrap();
                    let symbol = event.symbol.clone();

                    self.curr_ts = event.ts;

                    let tick_size = self.tick_size;
                    let lot_size = self.lot_size;
                    let fill_tracker = self
                        .tick_fill_tracker
                        .entry(symbol.clone())
                        .or_insert_with(Vec::new);
                    let exch = self
                        .exchanges
                        .entry(symbol)
                        .or_insert_with(|| Exchange::<Fil>::create(tick_size, lot_size));

                    if events_out.is_some() {
                        match event.typ {
                            EVENT_CLEAR_BOOK => {
                                exch.clear();
                            }
                            EVENT_CLEAR_LEVEL_BID => {
                                exch.clear_bid_level(&event);
                            }
                            EVENT_CLEAR_LEVEL_ASK => {
                                exch.clear_ask_level(&event);
                            }
                            EVENT_UPDATE_LEVEL_BID => {
                                exch.update_level(event.clone(), fill_tracker);
                            }
                            EVENT_UPDATE_LEVEL_ASK => {
                                exch.update_level(event.clone(), fill_tracker);
                            }
                            EVENT_TRADE_BUY => {
                                exch.process_trade(event.clone());
                            }
                            EVENT_TRADE_SELL => {
                                exch.process_trade(event.clone());
                            }
                            EVENT_CLEAR_SIDE_BID => {
                                exch.clear_bid();
                            }
                            EVENT_CLEAR_SIDE_ASK => {
                                exch.clear_ask();
                            }
                            _ => (),
                        }
                        if let Some(buf) = events_out.as_deref_mut() {
                            buf.push(event);
                        }
                    } else {
                        match event.typ {
                            EVENT_CLEAR_BOOK => {
                                exch.clear();
                            }
                            EVENT_CLEAR_LEVEL_BID => {
                                exch.clear_bid_level(&event);
                            }
                            EVENT_CLEAR_LEVEL_ASK => {
                                exch.clear_ask_level(&event);
                            }
                            EVENT_UPDATE_LEVEL_BID => {
                                exch.update_level(event, fill_tracker);
                            }
                            EVENT_UPDATE_LEVEL_ASK => {
                                exch.update_level(event, fill_tracker);
                            }
                            EVENT_TRADE_BUY => {
                                exch.process_trade(event);
                            }
                            EVENT_TRADE_SELL => {
                                exch.process_trade(event);
                            }
                            EVENT_CLEAR_SIDE_BID => {
                                exch.clear_bid();
                            }
                            EVENT_CLEAR_SIDE_ASK => {
                                exch.clear_ask();
                            }
                            _ => (),
                        }
                    }

                    // Once we've reached end_time and no further events at this
                    // timestamp remain in the queue, loop back one more time to
                    // flush any buffered orders whose latency has now cleared.
                    if self.curr_ts >= end_time
                        && self.ev_queue.front().map_or(true, |e| e.ts > end_time)
                    {
                        should_return = true;
                    }
                } else {
                    // Next event is past elapse window — advance clock and loop
                    // back once to flush any orders that cleared latency at end_time.
                    self.curr_ts = end_time;
                    should_return = true;
                }
            } else {
                // Queue is empty — if we've already reached end_time we're done.
                if self.curr_ts >= end_time {
                    should_return = true;
                    continue;
                }

                if sim_ended {
                    return Err(anyhow!("No more events"));
                }

                match self
                    .feed
                    .fill(&mut self.ev_queue, &mut self.candle_sink, self.line_chunk)
                {
                    FeedState::Empty => {
                        sim_ended = true;
                    }
                    FeedState::Active => {}
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use rust_decimal::Decimal;

    use super::*;
    use crate::source::{StreamingEventFeed, VecEventFeed};
    use crate::types::{
        Event, EVENT_CLEAR_SIDE_ASK, EVENT_CLEAR_SIDE_BID, EVENT_TRADE_BUY, EVENT_UPDATE_LEVEL_ASK,
        EVENT_UPDATE_LEVEL_BID,
    };

    fn make_config(start_ts: i64) -> L2Config {
        L2Config::new(
            Decimal::new(1, 1),  // tick_size = 0.1
            Decimal::new(1, 2),  // lot_size  = 0.01
            1024,
            start_ts,
            100,
            10,
            Decimal::new(2, 2),  // risk_free_rate = 0.02
            Duration::from_secs(86400),
        )
    }

    fn ev(typ: u64, ts: i64, px: &str, qty: &str) -> Event {
        Event::new_with_symbol(typ, ts, px, qty, "BTC")
    }

    /// Core bug reproduction: multiple events sharing a timestamp that equals
    /// `end_time` must all be processed in a single `elapse` call.
    #[test]
    fn test_elapse_processes_all_events_at_boundary_timestamp() {
        let events = vec![
            ev(EVENT_CLEAR_SIDE_BID, 100, "0.0", "0.0"),
            ev(EVENT_CLEAR_SIDE_ASK, 100, "0.0", "0.0"),
            ev(EVENT_UPDATE_LEVEL_BID, 100, "99.0", "5.0"),
            ev(EVENT_UPDATE_LEVEL_ASK, 100, "101.0", "5.0"),
        ];
        let config = make_config(0);
        let mut bt = Backtest::new_with_level_chg_fill(&config, VecEventFeed::new(events));

        bt.elapse(100).unwrap();

        let (bid, ask) = bt.bbo("BTC");
        assert!(bid > 0.0, "bid should be non-zero after snapshot at boundary");
        assert!(ask > 0.0, "ask should be non-zero after snapshot at boundary");
    }

    /// Events past `end_time` must not be processed; a subsequent `elapse` picks
    /// them up.
    #[test]
    fn test_elapse_does_not_process_events_past_boundary() {
        let events = vec![
            ev(EVENT_UPDATE_LEVEL_BID, 100, "99.0", "5.0"),
            ev(EVENT_UPDATE_LEVEL_ASK, 100, "101.0", "5.0"),
            // ts=200 events — must remain unprocessed after the first elapse.
            // Use a higher bid and lower ask so the BBO visibly shifts.
            ev(EVENT_UPDATE_LEVEL_BID, 200, "100.0", "3.0"),
            ev(EVENT_UPDATE_LEVEL_ASK, 200, "100.5", "3.0"),
        ];
        let config = make_config(0);
        let mut bt = Backtest::new_with_level_chg_fill(&config, VecEventFeed::new(events));

        // First elapse: advance to ts=100
        bt.elapse(100).unwrap();
        let (bid, ask) = bt.bbo("BTC");
        assert_eq!(bid, 99.0, "bid should reflect ts=100 data");
        assert_eq!(ask, 101.0, "ask should reflect ts=100 data");
        assert_eq!(bt.current_timestamp(), 100);

        // Second elapse: advance to ts=200; new levels shift the BBO
        bt.elapse(100).unwrap();
        let (bid2, ask2) = bt.bbo("BTC");
        assert_eq!(bid2, 100.0, "bid should reflect ts=200 data (new best bid)");
        assert_eq!(ask2, 100.5, "ask should reflect ts=200 data (new best ask)");
        assert_eq!(bt.current_timestamp(), 200);
    }

    /// Simulates two consecutive snapshots. After each `elapse` the book must
    /// reflect exactly that snapshot's prices.
    #[test]
    fn test_elapse_with_snapshot_like_event_batch() {
        let events = vec![
            // Snapshot at ts=1000
            ev(EVENT_CLEAR_SIDE_BID, 1000, "0.0", "0.0"),
            ev(EVENT_CLEAR_SIDE_ASK, 1000, "0.0", "0.0"),
            ev(EVENT_UPDATE_LEVEL_BID, 1000, "99.0", "10.0"),
            ev(EVENT_UPDATE_LEVEL_ASK, 1000, "101.0", "10.0"),
            // Snapshot at ts=2000 (different prices)
            ev(EVENT_CLEAR_SIDE_BID, 2000, "0.0", "0.0"),
            ev(EVENT_CLEAR_SIDE_ASK, 2000, "0.0", "0.0"),
            ev(EVENT_UPDATE_LEVEL_BID, 2000, "199.0", "20.0"),
            ev(EVENT_UPDATE_LEVEL_ASK, 2000, "201.0", "20.0"),
        ];
        let config = make_config(0);
        let mut bt = Backtest::new_with_level_chg_fill(&config, VecEventFeed::new(events));

        bt.elapse(1000).unwrap();
        let (bid, ask) = bt.bbo("BTC");
        assert_eq!(bid, 99.0, "bid should match first snapshot");
        assert_eq!(ask, 101.0, "ask should match first snapshot");

        bt.elapse(1000).unwrap();
        let (bid2, ask2) = bt.bbo("BTC");
        assert_eq!(bid2, 199.0, "bid should match second snapshot");
        assert_eq!(ask2, 201.0, "ask should match second snapshot");
    }

    /// A single event exactly at the boundary must not be dropped.
    #[test]
    fn test_elapse_with_single_event_at_boundary() {
        let events = vec![ev(EVENT_UPDATE_LEVEL_BID, 100, "50.0", "1.0")];
        let config = make_config(0);
        let mut bt = Backtest::new_with_level_chg_fill(&config, VecEventFeed::new(events));

        bt.elapse(100).unwrap();

        let (bid, _ask) = bt.bbo("BTC");
        assert!(bid > 0.0, "single event at boundary should be processed");
        assert_eq!(bt.current_timestamp(), 100);
    }

    /// All event types at the same timestamp must be processed together.
    #[test]
    fn test_elapse_processes_interleaved_trades_and_levels_at_same_timestamp() {
        let ts = 500_i64;
        let events = vec![
            ev(EVENT_UPDATE_LEVEL_BID, ts, "99.0", "5.0"),
            ev(EVENT_UPDATE_LEVEL_ASK, ts, "101.0", "5.0"),
            ev(EVENT_TRADE_BUY, ts, "101.0", "1.0"),
        ];
        let config = make_config(0);
        let mut bt = Backtest::new_with_level_chg_fill(&config, VecEventFeed::new(events));

        bt.elapse(ts as u64).unwrap();

        let (bid, ask) = bt.bbo("BTC");
        assert!(bid > 0.0, "bid level should be set after processing");
        assert!(ask > 0.0, "ask level should be set after processing");
        assert_eq!(bt.current_timestamp(), ts, "all events at ts should be processed");
    }

    #[test]
    fn test_fixed_latency_delays_order_execution() {
        use crate::types::OrderType;
        use crate::types::Side;
        let events = vec![
            ev(EVENT_UPDATE_LEVEL_BID, 100, "99.0", "10.0"),
            ev(EVENT_UPDATE_LEVEL_ASK, 100, "101.0", "10.0"),
            ev(EVENT_UPDATE_LEVEL_BID, 200, "99.0", "10.0"),
            ev(EVENT_UPDATE_LEVEL_ASK, 200, "101.0", "10.0"),
        ];
        let config = make_config(0);
        let mut bt = Backtest::new_with_level_chg_fill(&config, VecEventFeed::new(events));
        bt.set_latency_model(LatencyModel::Fixed(50));

        bt.elapse(100).unwrap();

        bt.submit_order("BTC", OrderRequest::new(Side::Buy, 1.0, None, OrderType::Market));

        bt.elapse(49).unwrap();
        assert_eq!(bt.get_position("BTC"), 0.0, "order should not execute before latency elapses");

        bt.elapse(1).unwrap();
        assert!(bt.get_position("BTC") > 0.0, "order should execute after latency elapses");
    }

    #[test]
    fn test_instant_latency_executes_on_next_elapse() {
        use crate::types::OrderType;
        use crate::types::Side;
        let events = vec![
            ev(EVENT_UPDATE_LEVEL_BID, 100, "99.0", "10.0"),
            ev(EVENT_UPDATE_LEVEL_ASK, 100, "101.0", "10.0"),
            ev(EVENT_UPDATE_LEVEL_BID, 200, "99.0", "10.0"),
        ];
        let config = make_config(0);
        let mut bt = Backtest::new_with_level_chg_fill(&config, VecEventFeed::new(events));

        bt.elapse(100).unwrap();
        bt.submit_order("BTC", OrderRequest::new(Side::Buy, 1.0, None, OrderType::Market));

        bt.elapse(1).unwrap();
        assert!(bt.get_position("BTC") > 0.0);
    }

    #[test]
    fn test_multiple_orders_respect_individual_latency() {
        use crate::types::OrderType;
        use crate::types::Side;
        let events = vec![
            ev(EVENT_UPDATE_LEVEL_BID, 100, "99.0", "10.0"),
            ev(EVENT_UPDATE_LEVEL_ASK, 100, "101.0", "10.0"),
            ev(EVENT_UPDATE_LEVEL_BID, 200, "99.0", "10.0"),
            ev(EVENT_UPDATE_LEVEL_ASK, 200, "101.0", "10.0"),
            ev(EVENT_UPDATE_LEVEL_BID, 300, "99.0", "10.0"),
            ev(EVENT_UPDATE_LEVEL_ASK, 300, "101.0", "10.0"),
        ];
        let config = make_config(0);
        let mut bt = Backtest::new_with_level_chg_fill(&config, VecEventFeed::new(events));
        bt.set_latency_model(LatencyModel::Fixed(50));

        bt.elapse(100).unwrap();
        bt.submit_order("BTC", OrderRequest::new(Side::Buy, 1.0, None, OrderType::Market));

        bt.elapse(50).unwrap(); // ts=150
        let pos_after_first = bt.get_position("BTC");
        assert!(pos_after_first > 0.0, "first order should have executed");

        bt.submit_order("BTC", OrderRequest::new(Side::Sell, 1.0, None, OrderType::Market));

        bt.elapse(49).unwrap(); // ts=199
        assert!(bt.get_position("BTC") > 0.0, "second order should not execute yet");

        bt.elapse(1).unwrap(); // ts=200
        assert_eq!(bt.get_position("BTC"), 0.0, "second order should have executed");
    }

    #[test]
    fn test_order_fills_at_execution_time_price() {
        use crate::types::OrderType;
        use crate::types::Side;
        let events = vec![
            ev(EVENT_UPDATE_LEVEL_BID, 100, "99.0", "10.0"),
            ev(EVENT_UPDATE_LEVEL_ASK, 100, "101.0", "10.0"),
            ev(EVENT_CLEAR_SIDE_ASK, 150, "0.0", "0.0"),
            ev(EVENT_UPDATE_LEVEL_ASK, 150, "102.0", "10.0"),
            ev(EVENT_UPDATE_LEVEL_BID, 200, "99.0", "10.0"),
        ];
        let config = make_config(0);
        let mut bt = Backtest::new_with_level_chg_fill(&config, VecEventFeed::new(events));
        bt.set_latency_model(LatencyModel::Fixed(100));

        bt.elapse(100).unwrap();
        bt.submit_order("BTC", OrderRequest::new(Side::Buy, 1.0, None, OrderType::Market));

        bt.elapse(100).unwrap(); // ts=200
        let order = bt.get_order("BTC", &0).unwrap();
        assert_eq!(order.exec_px, 102.0);
    }

    #[test]
    fn test_fixed_zero_behaves_like_instant() {
        use crate::types::OrderType;
        use crate::types::Side;
        let events = vec![
            ev(EVENT_UPDATE_LEVEL_BID, 100, "99.0", "10.0"),
            ev(EVENT_UPDATE_LEVEL_ASK, 100, "101.0", "10.0"),
            ev(EVENT_UPDATE_LEVEL_BID, 200, "99.0", "10.0"),
        ];
        let config = make_config(0);
        let mut bt = Backtest::new_with_level_chg_fill(&config, VecEventFeed::new(events));
        bt.set_latency_model(LatencyModel::Fixed(0));

        bt.elapse(100).unwrap();
        bt.submit_order("BTC", OrderRequest::new(Side::Buy, 1.0, None, OrderType::Market));

        bt.elapse(1).unwrap();
        assert!(bt.get_position("BTC") > 0.0);
    }

    #[test]
    fn test_l2_backtest_with_streaming_feed_boundary_timestamp() {
        use crate::source::FeedState;
        use std::collections::VecDeque;

        let mut done = false;
        let producer =
            move |events: &mut VecDeque<Event>, _candles: &mut VecDeque<Candle>, _count| {
                if done {
                    FeedState::Empty
                } else {
                    events.push_back(ev(EVENT_CLEAR_SIDE_BID, 100, "0.0", "0.0"));
                    events.push_back(ev(EVENT_CLEAR_SIDE_ASK, 100, "0.0", "0.0"));
                    events.push_back(ev(EVENT_UPDATE_LEVEL_BID, 100, "99.0", "5.0"));
                    events.push_back(ev(EVENT_UPDATE_LEVEL_ASK, 100, "101.0", "5.0"));
                    done = true;
                    FeedState::Active
                }
            };

        let config = make_config(0);
        let mut bt = Backtest::new_with_level_chg_fill(&config, StreamingEventFeed::new(producer));
        bt.elapse(100).unwrap();

        let (bid, ask) = bt.bbo("BTC");
        assert_eq!(bid, 99.0);
        assert_eq!(ask, 101.0);
    }
}
