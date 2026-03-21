use std::collections::HashMap;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;

use ::roshar_bt::chart::ChartData;
use ::roshar_bt::exchanges::hyperliquid::HyperliquidParser;
use ::roshar_bt::l2::backtest::{Backtest, LatencyModel};
use ::roshar_bt::l2::fill::LevelChgFill;
use ::roshar_bt::l2::L2ConfigBuilder;
use ::roshar_bt::source::{BufSource, EventFeed, MultiplexedFeed, ParsedFeed, VecEventFeed};
use ::roshar_bt::types::{
    Event, OrderRequest, OrderStatus, OrderType, Side, EVENT_CLEAR_SIDE_ASK, EVENT_CLEAR_SIDE_BID,
    EVENT_TRADE_BUY, EVENT_TRADE_SELL, EVENT_UPDATE_LEVEL_ASK, EVENT_UPDATE_LEVEL_BID,
};

/// Configuration for an L2 (order-book level) backtest.
///
/// Args:
///     tick_size: Minimum price increment.
///     lot_size: Minimum quantity increment.
///     start_ts: Simulation start timestamp in milliseconds.
///     return_window: Return window in seconds (used for Sharpe ratio annualization).
///     lines_read_per_tick: Number of feed lines to read per elapse call (default 100).
///     risk_free_rate: Annual risk-free rate for Sharpe ratio (default 0.02).
#[pyclass]
#[derive(Clone)]
struct L2Config {
    tick_size: f64,
    lot_size: f64,
    start_ts: i64,
    return_window: u64,
    lines_read_per_tick: usize,
    risk_free_rate: f64,
}

#[pymethods]
impl L2Config {
    #[new]
    #[pyo3(signature = (tick_size, lot_size, start_ts, return_window, lines_read_per_tick=100, risk_free_rate=0.02))]
    fn new(
        tick_size: f64,
        lot_size: f64,
        start_ts: i64,
        return_window: u64,
        lines_read_per_tick: usize,
        risk_free_rate: f64,
    ) -> Self {
        Self {
            tick_size,
            lot_size,
            start_ts,
            return_window,
            lines_read_per_tick,
            risk_free_rate,
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "L2Config(tick_size={}, lot_size={}, start_ts={}, return_window={})",
            self.tick_size, self.lot_size, self.start_ts, self.return_window,
        )
    }
}

/// Information about a submitted order.
#[pyclass]
#[derive(Clone)]
struct Order {
    #[pyo3(get)]
    id: u64,
    #[pyo3(get)]
    side: String,
    #[pyo3(get)]
    qty: f64,
    #[pyo3(get)]
    filled_qty: f64,
    #[pyo3(get)]
    order_px: f64,
    #[pyo3(get)]
    order_tick: i64,
    #[pyo3(get)]
    exec_px: f64,
    #[pyo3(get)]
    exec_tick: i64,
    #[pyo3(get)]
    order_type: String,
    #[pyo3(get)]
    status: String,
}

#[pymethods]
impl Order {
    fn __repr__(&self) -> String {
        format!(
            "Order(id={}, side='{}', qty={}, status='{}', order_px={}, exec_px={})",
            self.id, self.side, self.qty, self.status, self.order_px, self.exec_px,
        )
    }
}

impl From<::roshar_bt::l2::L2Order> for Order {
    fn from(o: ::roshar_bt::l2::L2Order) -> Self {
        Self {
            id: o.id,
            side: match o.side {
                Side::Buy => "buy".to_string(),
                Side::Sell => "sell".to_string(),
            },
            qty: o.qty,
            filled_qty: o.filled_qty,
            order_px: o.order_px,
            order_tick: o.order_tick,
            exec_px: o.exec_px,
            exec_tick: o.exec_tick,
            order_type: match o.typ {
                OrderType::Market => "market".to_string(),
                OrderType::Limit => "limit".to_string(),
            },
            status: match o.status {
                OrderStatus::Working => "working".to_string(),
                OrderStatus::Filled => "filled".to_string(),
                OrderStatus::Cancelled => "cancelled".to_string(),
            },
        }
    }
}

/// Performance metrics from a backtest.
#[pyclass]
#[derive(Clone)]
struct PerformanceMetrics {
    #[pyo3(get)]
    sharpe_ratio: f64,
    #[pyo3(get)]
    cumulative_return: f64,
}

#[pymethods]
impl PerformanceMetrics {
    fn __repr__(&self) -> String {
        format!(
            "PerformanceMetrics(sharpe_ratio={:.4}, cumulative_return={:.6})",
            self.sharpe_ratio, self.cumulative_return,
        )
    }
}

enum InnerBacktest {
    Mux(Backtest<MultiplexedFeed, LevelChgFill>),
    Vec(Backtest<VecEventFeed, LevelChgFill>),
}

/// Dispatch an immutable method call to whichever backtest variant is active.
macro_rules! dispatch {
    ($self:expr, $method:ident ( $($arg:expr),* )) => {
        match &$self.inner {
            InnerBacktest::Mux(bt) => bt.$method($($arg),*),
            InnerBacktest::Vec(bt) => bt.$method($($arg),*),
        }
    };
}

/// Dispatch a mutable method call to whichever backtest variant is active.
macro_rules! dispatch_mut {
    ($self:expr, $method:ident ( $($arg:expr),* )) => {
        match &mut $self.inner {
            InnerBacktest::Mux(bt) => bt.$method($($arg),*),
            InnerBacktest::Vec(bt) => bt.$method($($arg),*),
        }
    };
}

/// L2 backtest engine wrapping the Rust roshar-bt library.
///
/// Provides order-book level backtesting with realistic queue-position fill
/// simulation.
///
/// Create from file paths (Hyperliquid format)::
///
///     config = roshar_bt.L2Config(tick_size=0.1, lot_size=0.001, start_ts=0, return_window=60)
///     bt = roshar_bt.Backtest(config, {"btc": ["btc_book.log.gz"]})
///
/// Or from event data (e.g. fetched from ClickHouse)::
///
///     bt = roshar_bt.Backtest.from_events(
///         config,
///         snapshot={"bid_prices": [...], "bid_sizes": [...],
///                   "ask_prices": [...], "ask_sizes": [...], "time": 123456},
///         depth_updates=[("89500.0", "1.0", 123457, False), ...],
///         trades=[("89500.5", "0.5", 123458, False), ...],
///     )
#[pyclass(unsendable)]
struct PyBacktest {
    inner: InnerBacktest,
    ev_buf: Vec<Event>,
}

#[pymethods]
impl PyBacktest {
    /// Pre-allocate the internal event buffer to hold ``n`` events.
    ///
    /// Call this once before the main loop to avoid repeated resizing
    /// during ``elapse`` calls.
    fn reserve_event_buffer(&mut self, n: usize) {
        self.ev_buf.reserve(n);
    }

    /// Create a backtest from Hyperliquid websocket recording files.
    ///
    /// Args:
    ///     config: L2Config with tick/lot sizes and timing parameters.
    ///     data_paths: Dict mapping logical group names to lists of file paths.
    #[new]
    fn new(config: &L2Config, data_paths: HashMap<String, Vec<String>>) -> PyResult<Self> {
        let l2_config = build_l2_config(config)?;

        let mut feeds: Vec<Box<dyn EventFeed>> = Vec::new();
        for (_label, paths) in data_paths {
            if paths.is_empty() {
                continue;
            }
            let src = BufSource::new_files(paths);
            let feed = ParsedFeed::new(src, HyperliquidParser);
            feeds.push(Box::new(feed));
        }

        let mux = MultiplexedFeed::new(feeds);
        let bt = Backtest::new_with_level_chg_fill(&l2_config, mux);

        Ok(Self {
            inner: InnerBacktest::Mux(bt),
            ev_buf: Vec::new(),
        })
    }

    /// Create a backtest from event data (snapshots, depth updates, trades).
    ///
    /// Args:
    ///     config: L2Config with tick/lot sizes and timing parameters.
    ///     snapshot: Optional dict with keys ``bid_prices``, ``bid_sizes``,
    ///         ``ask_prices``, ``ask_sizes`` (lists of price/size strings) and
    ///         ``time`` (timestamp in ms). Seeds the initial orderbook.
    ///     depth_updates: List of ``(px, qty, time, is_ask)`` tuples. ``px``
    ///         and ``qty`` are strings, ``time`` is ms timestamp, ``is_ask``
    ///         is bool (True for ask side, False for bid side).
    ///     trades: List of ``(px, qty, time, is_sell)`` tuples. ``is_sell``
    ///         is True when the taker is selling, False when buying.
    #[staticmethod]
    #[pyo3(signature = (config, snapshot=None, depth_updates=None, trades=None))]
    fn from_events(
        config: &L2Config,
        snapshot: Option<HashMap<String, PyObject>>,
        depth_updates: Option<Vec<(String, String, i64, bool)>>,
        trades: Option<Vec<(String, String, i64, bool)>>,
    ) -> PyResult<Self> {
        let l2_config = build_l2_config(config)?;

        let mut events: Vec<Event> = Vec::new();

        // Build snapshot events
        if let Some(snap) = snapshot {
            Python::with_gil(|py| -> PyResult<()> {
                let time: i64 = snap
                    .get("time")
                    .ok_or_else(|| PyValueError::new_err("snapshot missing 'time'"))?
                    .extract(py)?;

                let bid_prices: Vec<String> = snap
                    .get("bid_prices")
                    .ok_or_else(|| PyValueError::new_err("snapshot missing 'bid_prices'"))?
                    .extract(py)?;
                let bid_sizes: Vec<String> = snap
                    .get("bid_sizes")
                    .ok_or_else(|| PyValueError::new_err("snapshot missing 'bid_sizes'"))?
                    .extract(py)?;
                let ask_prices: Vec<String> = snap
                    .get("ask_prices")
                    .ok_or_else(|| PyValueError::new_err("snapshot missing 'ask_prices'"))?
                    .extract(py)?;
                let ask_sizes: Vec<String> = snap
                    .get("ask_sizes")
                    .ok_or_else(|| PyValueError::new_err("snapshot missing 'ask_sizes'"))?
                    .extract(py)?;

                events.push(Event::new(EVENT_CLEAR_SIDE_BID, time, "0.0", "0.0"));
                events.push(Event::new(EVENT_CLEAR_SIDE_ASK, time, "0.0", "0.0"));

                for (px, qty) in bid_prices.iter().zip(bid_sizes.iter()) {
                    events.push(Event::new(EVENT_UPDATE_LEVEL_BID, time, px, qty));
                }
                for (px, qty) in ask_prices.iter().zip(ask_sizes.iter()) {
                    events.push(Event::new(EVENT_UPDATE_LEVEL_ASK, time, px, qty));
                }
                Ok(())
            })?;
        }

        // Build depth update events
        if let Some(updates) = depth_updates {
            for (px, qty, time, is_ask) in &updates {
                let typ = if *is_ask {
                    EVENT_UPDATE_LEVEL_ASK
                } else {
                    EVENT_UPDATE_LEVEL_BID
                };
                events.push(Event::new(typ, *time, px, qty));
            }
        }

        // Build trade events
        if let Some(trade_list) = trades {
            for (px, qty, time, is_sell) in &trade_list {
                let typ = if *is_sell {
                    EVENT_TRADE_SELL
                } else {
                    EVENT_TRADE_BUY
                };
                events.push(Event::new(typ, *time, px, qty));
            }
        }

        // Stable sort: snapshot events before updates/trades at same timestamp
        events.sort_by_key(|e| e.ts);
        let feed = VecEventFeed::new(events);
        let bt = Backtest::new_with_level_chg_fill(&l2_config, feed);

        Ok(Self {
            inner: InnerBacktest::Vec(bt),
            ev_buf: Vec::new(),
        })
    }

    /// Advance the simulation by ``ms`` milliseconds.
    ///
    /// Returns a list of ``(typ, ts, px, qty, symbol)`` tuples for the events
    /// processed during this tick, or ``None`` when the data feed is exhausted.
    fn elapse(&mut self, ms: u64) -> Option<Vec<(u64, i64, String, String, String)>> {
        let result = match &mut self.inner {
            InnerBacktest::Mux(bt) => bt.elapse_with_buffer(ms, &mut self.ev_buf),
            InnerBacktest::Vec(bt) => bt.elapse_with_buffer(ms, &mut self.ev_buf),
        };
        if result.is_ok() {
            Some(
                self.ev_buf
                    .iter()
                    .map(|e| (e.typ, e.ts, e.px.clone(), e.qty.clone(), e.symbol.clone()))
                    .collect(),
            )
        } else {
            None
        }
    }

    /// Current simulation timestamp in milliseconds.
    fn current_timestamp(&self) -> i64 {
        dispatch!(self, current_timestamp())
    }

    /// Best bid and ask prices for a symbol.
    ///
    /// Returns ``(0.0, 0.0)`` if the symbol has not been seen yet.
    fn bbo(&self, symbol: &str) -> (f64, f64) {
        dispatch!(self, bbo(symbol))
    }

    /// Current position size for a symbol (positive = long, negative = short).
    fn get_position(&mut self, symbol: &str) -> f64 {
        dispatch_mut!(self, get_position(symbol))
    }

    /// Get the quantity at a specific price level in the orderbook.
    ///
    /// Args:
    ///     symbol: The trading symbol.
    ///     price: The price level to query.
    ///     side: ``"buy"`` (bid) or ``"sell"`` (ask).
    ///
    /// Returns:
    ///     The quantity at that level, or 0.0 if the level does not exist.
    fn get_level(&self, symbol: &str, price: f64, side: &str) -> PyResult<f64> {
        let side = match side.to_lowercase().as_str() {
            "buy" => Side::Buy,
            "sell" => Side::Sell,
            other => {
                return Err(PyValueError::new_err(format!(
                    "Invalid side '{}': expected 'buy' or 'sell'",
                    other
                )))
            }
        };
        let price_dec = Decimal::try_from(price)
            .map_err(|e| PyValueError::new_err(format!("Invalid price: {}", e)))?;

        let qty = match &self.inner {
            InnerBacktest::Mux(bt) => bt
                .exchanges
                .get(symbol)
                .map(|e| e.get_level(price_dec, side))
                .unwrap_or(Decimal::ZERO),
            InnerBacktest::Vec(bt) => bt
                .exchanges
                .get(symbol)
                .map(|e| e.get_level(price_dec, side))
                .unwrap_or(Decimal::ZERO),
        };
        Ok(qty.to_f64().unwrap_or(0.0))
    }

    /// Submit a limit or market order.
    ///
    /// Args:
    ///     symbol: The trading symbol (e.g. ``"BTC"``).
    ///     side: ``"buy"`` or ``"sell"``.
    ///     qty: Order quantity.
    ///     px: Limit price. Omit or pass ``None`` for a market order.
    #[pyo3(signature = (symbol, side, qty, px=None))]
    fn submit_order(
        &mut self,
        symbol: &str,
        side: &str,
        qty: f64,
        px: Option<f64>,
    ) -> PyResult<()> {
        let side = match side.to_lowercase().as_str() {
            "buy" => Side::Buy,
            "sell" => Side::Sell,
            other => {
                return Err(PyValueError::new_err(format!(
                    "Invalid side '{}': expected 'buy' or 'sell'",
                    other
                )))
            }
        };

        let (order_type, order_px) = match px {
            Some(p) => (OrderType::Limit, Some(p)),
            None => (OrderType::Market, None),
        };

        let req = OrderRequest::new(side, qty, order_px, order_type);
        dispatch_mut!(self, submit_order(symbol, req));
        Ok(())
    }

    /// Cancel a working order.
    fn cancel_order(&mut self, symbol: &str, order_id: u64) -> PyResult<()> {
        dispatch_mut!(self, cancel_order(symbol, &order_id))
            .map_err(|e| PyValueError::new_err(e.to_string()))
    }

    /// Order IDs that were filled during the most recent ``elapse`` call.
    fn last_trades(&self, symbol: &str) -> Vec<u64> {
        dispatch!(self, last_trades(symbol)).to_vec()
    }

    /// IDs of all currently working (open) orders for a symbol.
    fn get_working_orders(&self, symbol: &str) -> Vec<u64> {
        dispatch!(self, get_working_orders(symbol))
    }

    /// Retrieve details of a specific order.
    fn get_order(&self, symbol: &str, order_id: u64) -> Option<Order> {
        dispatch!(self, get_order(symbol, &order_id)).map(|o| o.into())
    }

    /// Update portfolio-level performance metrics from current positions and prices.
    fn update_performance_metrics(&mut self) {
        dispatch_mut!(self, update_performance_metrics());
    }

    /// Compute and return performance metrics.
    fn get_performance_metrics(&self) -> PerformanceMetrics {
        let perf = dispatch!(self, get_performance_metrics());
        PerformanceMetrics {
            sharpe_ratio: perf.calculate_sharpe_ratio().to_f64().unwrap_or(0.0),
            cumulative_return: perf.get_cumulative_return().to_f64().unwrap_or(0.0),
        }
    }

    /// Generate a multi-panel chart and save to a file.
    fn generate_chart(&self, path: &str) -> PyResult<()> {
        let chart_data =
            ChartData::from_performance_metrics(dispatch!(self, get_performance_metrics()));
        chart_data
            .create_multi_chart(path)
            .map_err(|e| PyValueError::new_err(e.to_string()))
    }

    /// Set the order latency model.
    ///
    /// Args:
    ///     ms: Latency in milliseconds. ``0`` means instant execution.
    fn set_latency(&mut self, ms: u64) {
        let model = if ms == 0 {
            LatencyModel::Instant
        } else {
            LatencyModel::Fixed(ms)
        };
        match &mut self.inner {
            InnerBacktest::Mux(bt) => bt.set_latency_model(model),
            InnerBacktest::Vec(bt) => bt.set_latency_model(model),
        }
    }

    /// Return the cumulative returns history as a list of floats.
    fn get_cumulative_returns_history(&self) -> Vec<f64> {
        dispatch!(self, get_performance_metrics())
            .get_cumulative_returns_history()
            .iter()
            .map(|d| d.to_f64().unwrap_or(0.0))
            .collect()
    }

    /// Return the per-period returns history as a list of floats.
    fn get_returns_history(&self) -> Vec<f64> {
        dispatch!(self, get_performance_metrics())
            .get_returns_history()
            .iter()
            .map(|d| d.to_f64().unwrap_or(0.0))
            .collect()
    }

    /// Return the price history as a list of ``(timestamp, price)`` tuples.
    fn get_price_history(&self) -> Vec<(i64, f64)> {
        dispatch!(self, get_performance_metrics())
            .get_prices()
            .iter()
            .map(|(ts, d)| (*ts, d.to_f64().unwrap_or(0.0)))
            .collect()
    }

    /// Return the position history as a list of ``(timestamp, position)`` tuples.
    fn get_position_history(&self) -> Vec<(i64, f64)> {
        dispatch!(self, get_performance_metrics())
            .get_position_history()
            .iter()
            .map(|(ts, d)| (*ts, d.to_f64().unwrap_or(0.0)))
            .collect()
    }
}

fn build_l2_config(
    config: &L2Config,
) -> PyResult<::roshar_bt::l2::L2Config> {
    let mut builder = L2ConfigBuilder::new();
    builder
        .set_tick_size(config.tick_size)
        .set_lot_size(config.lot_size)
        .set_start_ts(config.start_ts)
        .set_return_window(config.return_window)
        .set_lines_read_per_tick(config.lines_read_per_tick)
        .set_risk_free_rate(config.risk_free_rate);

    builder
        .build()
        .map_err(|e| PyValueError::new_err(e.to_string()))
}

#[pymodule]
fn roshar_bt(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<L2Config>()?;
    m.add_class::<PyBacktest>()?;
    m.add_class::<Order>()?;
    m.add_class::<PerformanceMetrics>()?;
    Ok(())
}
