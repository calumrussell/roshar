use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use rust_decimal::prelude::ToPrimitive;
use std::collections::HashMap;

use ::roshar_bt::chart::ChartData;
use ::roshar_bt::exchanges::hyperliquid::HyperliquidParser;
use ::roshar_bt::l2::backtest::{Backtest, LatencyModel};
use ::roshar_bt::l2::fill::LevelChgFill;
use ::roshar_bt::l2::L2ConfigBuilder;
use ::roshar_bt::source::{BufSource, EventFeed, MultiplexedFeed, ParsedFeed};
use ::roshar_bt::types::{OrderRequest, OrderStatus, OrderType, Side};

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

/// L2 backtest engine wrapping the Rust roshar-bt library.
///
/// Provides order-book level backtesting with realistic queue-position fill
/// simulation. Data is loaded from Hyperliquid websocket recordings.
///
/// Args:
///     config: L2Config with tick/lot sizes and timing parameters.
///     data_paths: Dict mapping logical group names to lists of file paths.
///         Files can be plain text or gzip-compressed (.gz). The symbol for
///         each event is extracted from the data itself.
///
/// Example::
///
///     config = roshar_bt.L2Config(tick_size=0.1, lot_size=0.001, start_ts=0, return_window=60)
///     bt = roshar_bt.Backtest(config, {"btc": ["btc_book.log.gz"], "eth": ["eth_book.log.gz"]})
///     while bt.elapse(60000):
///         bid, ask = bt.bbo("BTC")
///         bt.submit_order("BTC", "buy", 1.0, px=99.0)
///         bt.update_performance_metrics()
///     metrics = bt.get_performance_metrics()
#[pyclass(unsendable)]
struct PyBacktest {
    inner: Backtest<MultiplexedFeed, LevelChgFill>,
}

#[pymethods]
impl PyBacktest {
    #[new]
    fn new(config: &L2Config, data_paths: HashMap<String, Vec<String>>) -> PyResult<Self> {
        let mut builder = L2ConfigBuilder::new();
        builder
            .set_tick_size(config.tick_size)
            .set_lot_size(config.lot_size)
            .set_start_ts(config.start_ts)
            .set_return_window(config.return_window)
            .set_lines_read_per_tick(config.lines_read_per_tick)
            .set_risk_free_rate(config.risk_free_rate);

        let l2_config = builder
            .build()
            .map_err(|e| PyValueError::new_err(e.to_string()))?;

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

        Ok(Self { inner: bt })
    }

    /// Advance the simulation by ``ms`` milliseconds.
    ///
    /// Processes all events within the time window and flushes any orders
    /// that have cleared latency. Returns ``True`` on success, ``False``
    /// when the data feed is exhausted.
    fn elapse(&mut self, ms: u64) -> bool {
        self.inner.elapse(ms).is_ok()
    }

    /// Current simulation timestamp in milliseconds.
    fn current_timestamp(&self) -> i64 {
        self.inner.current_timestamp()
    }

    /// Best bid and ask prices for a symbol.
    ///
    /// Returns ``(0.0, 0.0)`` if the symbol has not been seen yet.
    fn bbo(&self, symbol: &str) -> (f64, f64) {
        self.inner.bbo(symbol)
    }

    /// Current position size for a symbol (positive = long, negative = short).
    fn get_position(&mut self, symbol: &str) -> f64 {
        self.inner.get_position(symbol)
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
        self.inner.submit_order(symbol, req);
        Ok(())
    }

    /// Cancel a working order.
    ///
    /// Raises ``ValueError`` if the symbol is unknown or the order cannot be
    /// cancelled.
    fn cancel_order(&mut self, symbol: &str, order_id: u64) -> PyResult<()> {
        self.inner
            .cancel_order(symbol, &order_id)
            .map_err(|e| PyValueError::new_err(e.to_string()))
    }

    /// Order IDs that were filled during the most recent ``elapse`` call.
    fn last_trades(&self, symbol: &str) -> Vec<u64> {
        self.inner.last_trades(symbol).to_vec()
    }

    /// IDs of all currently working (open) orders for a symbol.
    fn get_working_orders(&self, symbol: &str) -> Vec<u64> {
        self.inner.get_working_orders(symbol)
    }

    /// Retrieve details of a specific order.
    ///
    /// Returns ``None`` if the symbol or order ID is unknown.
    fn get_order(&self, symbol: &str, order_id: u64) -> Option<Order> {
        self.inner
            .get_order(symbol, &order_id)
            .map(|o| o.into())
    }

    /// Update portfolio-level performance metrics from current positions and prices.
    ///
    /// Call this after each ``elapse`` to track cumulative returns and Sharpe ratio.
    fn update_performance_metrics(&mut self) {
        self.inner.update_performance_metrics();
    }

    /// Compute and return performance metrics.
    fn get_performance_metrics(&self) -> PerformanceMetrics {
        let perf = self.inner.get_performance_metrics();
        PerformanceMetrics {
            sharpe_ratio: perf.calculate_sharpe_ratio().to_f64().unwrap_or(0.0),
            cumulative_return: perf.get_cumulative_return().to_f64().unwrap_or(0.0),
        }
    }

    /// Generate a multi-panel chart (price + cumulative return, position) and save to a file.
    fn generate_chart(&self, path: &str) -> PyResult<()> {
        let chart_data =
            ChartData::from_performance_metrics(self.inner.get_performance_metrics());
        chart_data
            .create_multi_chart(path)
            .map_err(|e| PyValueError::new_err(e.to_string()))
    }

    /// Set the order latency model.
    ///
    /// Args:
    ///     ms: Latency in milliseconds. ``0`` means instant execution on the
    ///         next ``elapse`` call.
    fn set_latency(&mut self, ms: u64) {
        if ms == 0 {
            self.inner.set_latency_model(LatencyModel::Instant);
        } else {
            self.inner.set_latency_model(LatencyModel::Fixed(ms));
        }
    }

    /// Return the cumulative returns history as a list of floats.
    fn get_cumulative_returns_history(&self) -> Vec<f64> {
        self.inner
            .get_performance_metrics()
            .get_cumulative_returns_history()
            .iter()
            .map(|d| d.to_f64().unwrap_or(0.0))
            .collect()
    }

    /// Return the per-period returns history as a list of floats.
    fn get_returns_history(&self) -> Vec<f64> {
        self.inner
            .get_performance_metrics()
            .get_returns_history()
            .iter()
            .map(|d| d.to_f64().unwrap_or(0.0))
            .collect()
    }

    /// Return the price history as a list of ``(timestamp, price)`` tuples.
    fn get_price_history(&self) -> Vec<(i64, f64)> {
        self.inner
            .get_performance_metrics()
            .get_prices()
            .iter()
            .map(|(ts, d)| (*ts, d.to_f64().unwrap_or(0.0)))
            .collect()
    }

    /// Return the position history as a list of ``(timestamp, position)`` tuples.
    fn get_position_history(&self) -> Vec<(i64, f64)> {
        self.inner
            .get_performance_metrics()
            .get_position_history()
            .iter()
            .map(|(ts, d)| (*ts, d.to_f64().unwrap_or(0.0)))
            .collect()
    }
}

#[pymodule]
fn roshar_bt(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<L2Config>()?;
    m.add_class::<PyBacktest>()?;
    m.add_class::<Order>()?;
    m.add_class::<PerformanceMetrics>()?;
    Ok(())
}
