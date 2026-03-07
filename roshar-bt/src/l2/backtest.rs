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
}

impl LatencyModel {
    pub fn calc_delay(&self) -> u64 {
        match self {
            LatencyModel::Instant => 0,
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
        // Clear per-symbol fill trackers.
        for tracker in self.tick_fill_tracker.values_mut() {
            tracker.clear();
        }

        let mut sim_ended = false;
        let end_time = self.curr_ts + ts as i64;

        while self.curr_ts < end_time {
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
                    .fill(&mut self.ev_queue, &mut self.candle_sink, self.line_chunk)
                {
                    FeedState::Empty => {
                        sim_ended = true;
                    }
                    FeedState::Active => {}
                }
            }
        }
        Ok(())
    }
}
