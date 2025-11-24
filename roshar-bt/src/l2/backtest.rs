use std::collections::VecDeque;

use anyhow::{anyhow, Ok, Result};
use log::warn;
use rust_decimal::prelude::{FromPrimitive, ToPrimitive};
use rust_decimal::Decimal;

use crate::performance::PerformanceMetrics;
use crate::source::{EventProducer, EventSrc, EventSrcState};
use crate::types::{
    Event, OrderRequest, EVENT_CLEAR_BOOK, EVENT_CLEAR_LEVEL_ASK, EVENT_CLEAR_LEVEL_BID,
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

pub struct Backtest<S: EventSrc, F: FillModel, P: EventProducer> {
    pub src: S,
    pub exch: Exchange<F>,
    pub curr_ts: i64,
    pub ev_queue: VecDeque<Event>,
    pub line_chunk: usize,
    pub order_buffer: VecDeque<OrderRequest>,
    pub latency_model: LatencyModel,
    pub tick_fill_tracker: Vec<OrderId>,
    pub performance: PerformanceMetrics,
    pub buf: String,
    pub parser: P,
}

impl<S: EventSrc, P: EventProducer> Backtest<S, LevelChgFill, P> {
    pub fn new_with_level_chg_fill(config: &L2Config<P>, src: S) -> Self {
        let exch = Exchange::new_with_level_chg_fill(config);
        let performance = PerformanceMetrics::new(config.risk_free_rate, config.return_window);

        Self {
            src,
            exch,
            curr_ts: config.start_ts,
            ev_queue: VecDeque::with_capacity(1_024),
            line_chunk: config.lines_read_per_tick,
            order_buffer: VecDeque::with_capacity(config.order_buffer_start_size),
            latency_model: LatencyModel::Instant,
            tick_fill_tracker: Vec::with_capacity(config.tick_fill_tracker_start_size),
            performance,
            buf: String::with_capacity(1_024),
            parser: config.parser.clone(),
        }
    }

    pub fn get_working_orders(&self) -> Vec<OrderId> {
        self.exch.get_working_orders()
    }

    pub fn get_performance_metrics(&self) -> &PerformanceMetrics {
        &self.performance
    }
}

impl<S: EventSrc, F: FillModel, P: EventProducer> Backtest<S, F, P> {
    pub fn current_timestamp(&self) -> i64 {
        self.curr_ts
    }

    pub fn last_trades(&self) -> &[OrderId] {
        &self.tick_fill_tracker
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

    pub fn submit_order(&mut self, mut req: OrderRequest) {
        req.set_time(self.curr_ts);
        self.order_buffer.push_back(req);
    }

    pub fn cancel_order(&mut self, order_id: &OrderId) -> Result<()> {
        self.exch.cancel_order(order_id)
    }

    pub fn get_position(&mut self) -> f64 {
        self.exch
            .get_position()
            .normalize()
            .to_f64()
            .expect("Unable to parse Decimal position to f64")
    }

    pub fn update_performance_metrics(&mut self) {
        let position = self.exch.get_position();
        let (bid, ask) = self.bbo();
        let mid_price = Decimal::from_f64((bid + ask) / 2.0).unwrap();
        self.performance.update(self.curr_ts, position, mid_price);
    }

    pub fn get_order_id_position(&self) -> OrderId {
        self.exch.get_order_id_position()
    }

    pub fn get_order(&self, order_id: &OrderId) -> Option<L2Order> {
        if let Some(order) = self.exch.get_order(order_id) {
            return Some(order.clone().into());
        }
        None
    }

    pub fn get_orders_by_level(&self, price: f64) -> Option<&Vec<OrderId>> {
        let decimal_price = Decimal::from_f64(price).expect("Unable to parse f64 price to Decimal");
        self.exch.get_orders_by_level(decimal_price)
    }

    pub fn elapse(&mut self, ts: u64) -> Result<()> {
        self.tick_fill_tracker.clear();
        let mut sim_ended = false;
        let end_time = self.curr_ts + ts as i64;

        while self.curr_ts < end_time {
            while let Some(front) = self.order_buffer.front() {
                if (front.get_time() + self.latency_model.calc_delay() as i64) <= self.curr_ts {
                    let order = self.order_buffer.pop_front().unwrap();
                    self.exch.execute_user_order(order);
                } else {
                    break;
                }
            }

            if let Some(peek_event) = self.ev_queue.front() {
                if peek_event.ts <= end_time {
                    let event = self.ev_queue.pop_front().unwrap();

                    self.curr_ts = event.ts;
                    match event.typ {
                        EVENT_CLEAR_BOOK => {
                            self.exch.clear();
                        }
                        EVENT_CLEAR_LEVEL_BID => {
                            self.exch.clear_bid_level(&event);
                        }
                        EVENT_CLEAR_LEVEL_ASK => {
                            self.exch.clear_ask_level(&event);
                        }
                        EVENT_UPDATE_LEVEL_BID => {
                            self.exch.update_level(event, &mut self.tick_fill_tracker);
                        }
                        EVENT_UPDATE_LEVEL_ASK => {
                            self.exch.update_level(event, &mut self.tick_fill_tracker);
                        }
                        EVENT_TRADE_BUY => {
                            self.exch.process_trade(event);
                        }
                        EVENT_TRADE_SELL => {
                            self.exch.process_trade(event);
                        }
                        EVENT_CLEAR_SIDE_BID => {
                            self.exch.clear_bid();
                        }
                        EVENT_CLEAR_SIDE_ASK => {
                            self.exch.clear_ask();
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

                //There are no events in the queue, so we need to fetch some more
                for _i in 0..self.line_chunk {
                    if let Some(src_state) = self.src.pop(&mut self.buf) {
                        match src_state {
                            EventSrcState::Empty => {
                                sim_ended = true;
                            }
                            EventSrcState::Active => {
                                if let Err(e) =
                                    self.parser.parse_line(&self.buf, &mut self.ev_queue)
                                {
                                    warn!("Failed to parse line: {}", e);
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }
}
