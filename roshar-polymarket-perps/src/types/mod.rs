//! Wire types for the Polymarket Perps REST API (`/v1/info/*`), as the exchange sends them.
//! Decimals stay strings; timestamps are Unix milliseconds.

pub mod account;
pub mod ws;

pub use account::*;
pub use ws::*;

use serde::{Deserialize, Serialize};

/// `tif`. The SDK places `gtc`, `gtd`, `ioc` and `fok` orders; the OpenAPI enum lists the other
/// three.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TimeInForce {
    Gtc,
    Gtd,
    Ioc,
    Fok,
}

impl TimeInForce {
    pub fn as_str(self) -> &'static str {
        match self {
            TimeInForce::Gtc => "gtc",
            TimeInForce::Gtd => "gtd",
            TimeInForce::Ioc => "ioc",
            TimeInForce::Fok => "fok",
        }
    }
}

/// Taker side of a trade / side of a fill.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    Long,
    Short,
}

/// `interval` of `/v1/info/klines`, `/v1/info/mark-history` and the `klines::{iid}::{interval}`
/// channel.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KlineInterval {
    S1,
    M1,
    M5,
    M15,
    M30,
    H1,
    H4,
    H6,
    H12,
    D1,
    W1,
}

impl KlineInterval {
    pub fn as_str(self) -> &'static str {
        match self {
            KlineInterval::S1 => "1s",
            KlineInterval::M1 => "1m",
            KlineInterval::M5 => "5m",
            KlineInterval::M15 => "15m",
            KlineInterval::M30 => "30m",
            KlineInterval::H1 => "1h",
            KlineInterval::H4 => "4h",
            KlineInterval::H6 => "6h",
            KlineInterval::H12 => "12h",
            KlineInterval::D1 => "1d",
            KlineInterval::W1 => "1w",
        }
    }
}

/// `GET /v1/info/ping`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct PingResponse {
    pub status: String,
}

/// `GET /v1/info/time`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct ServerTime {
    pub time: i64,
}

/// `GET /v1/info/exchange`: the EIP-712 domain and exchange mode.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct ExchangeInfo {
    pub name: String,
    pub version: String,
    pub chain_id: u64,
    pub contract: String,
    /// True while only cancellations are accepted (maintenance).
    pub cancel_only: bool,
    pub engine_version: String,
}

/// `GET /v1/info/assets`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct CollateralAsset {
    pub asset: String,
    pub address: String,
    pub decimals: u32,
    pub collateral_ratio: String,
    pub withdrawal_fee: String,
}

/// `GET /v1/info/instruments`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Instrument {
    pub instrument_id: u32,
    /// `perpetual`.
    pub instrument_type: String,
    /// `equity`, `commodity`, `index`, `crypto`; live data also has `pre_ipo`.
    pub category: String,
    pub isolated_only: bool,
    pub symbol: String,
    /// Optional presentation label.
    pub display_symbol: Option<String>,
    pub base_asset: String,
    pub quote_asset: String,
    pub funding_interval: String,
    pub quantity_decimals: u32,
    pub price_decimals: u32,
    pub price_bounds: String,
    pub liquidation_fee: String,
    pub max_order_count: u32,
    pub min_notional: String,
    pub max_market_notional: String,
    pub max_limit_notional: String,
    pub max_leverage: u32,
    pub risk_tiers: Vec<RiskTier>,
    /// Advisory display time; `null` when hidden.
    pub ui_live_time: Option<i64>,
    /// New orders must be reduce-only.
    pub close_only: bool,
    /// Present once the instrument is permanently settled (retired).
    pub settlement: Option<InstrumentSettlement>,
    /// Logo URLs; absent until a logo has been uploaded (docs), not in the OpenAPI schema.
    pub logo: Option<InstrumentLogo>,
    /// Promotional fee schedule; `null` for most instruments. Live field, not in the OpenAPI
    /// schema.
    pub fee_override: Option<FeeOverride>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct RiskTier {
    pub lower_bound: String,
    pub max_leverage: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct InstrumentSettlement {
    pub sequence: u64,
    pub timestamp: i64,
    pub price: String,
    pub insurance_debit: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct InstrumentLogo {
    pub light: String,
    pub dark: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct FeeOverride {
    pub start_time: i64,
    pub end_time: Option<i64>,
    pub tiers: Vec<FeeRates>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct FeeRates {
    pub taker_fee_rate: String,
    pub maker_fee_rate: String,
}

/// `GET /v1/info/tickers`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Ticker {
    pub instrument_id: u32,
    pub symbol: String,
    pub index_price: String,
    pub mark_price: String,
    pub last_price: String,
    pub mid_price: String,
    /// One-sided open interest in contracts.
    pub open_interest: String,
    /// Live field, not in the OpenAPI schema.
    pub long_count: u64,
    /// Live field, not in the OpenAPI schema.
    pub short_count: u64,
    /// Current funding rate.
    pub funding_rate: String,
    /// Next funding time.
    pub next_funding: i64,
    pub timestamp: i64,
}

/// One candle: `[open_time, open, high, low, close, volume, trades]`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(from = "(i64, String, String, String, String, String, u64)")]
pub struct Kline {
    pub open_time: i64,
    pub open: String,
    pub high: String,
    pub low: String,
    pub close: String,
    /// Base-unit volume.
    pub volume: String,
    pub trades: u64,
}

impl From<(i64, String, String, String, String, String, u64)> for Kline {
    fn from(t: (i64, String, String, String, String, String, u64)) -> Self {
        Kline {
            open_time: t.0,
            open: t.1,
            high: t.2,
            low: t.3,
            close: t.4,
            volume: t.5,
            trades: t.6,
        }
    }
}

/// `GET /v1/info/statistics`: rolling 24 h statistics.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Statistic {
    pub instrument_id: u32,
    pub symbol: String,
    /// Quote-asset notional.
    pub volume: String,
    pub open_price: String,
    pub klines: Vec<Kline>,
}

/// `GET /v1/info/exchange-stats`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct ExchangeStatistics {
    pub start_timestamp: i64,
    pub end_timestamp: i64,
    pub volume: String,
    /// `null` when no complete sample is available.
    pub open_interest: Option<String>,
    /// `null` when no complete sample is available.
    pub open_interest_timestamp: Option<i64>,
    pub fees: String,
}

/// `GET /v1/info/klines` (at most 1000 per page).
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Klines {
    pub data: Vec<Kline>,
    pub more: bool,
}

/// `[bucket_open_time, last_mark_price]`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(from = "(i64, String)")]
pub struct MarkPoint {
    pub time: i64,
    pub mark_price: String,
}

impl From<(i64, String)> for MarkPoint {
    fn from(t: (i64, String)) -> Self {
        MarkPoint {
            time: t.0,
            mark_price: t.1,
        }
    }
}

/// `GET /v1/info/mark-history` (at most 1000 per page).
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct MarkHistory {
    pub data: Vec<MarkPoint>,
    pub more: bool,
}

/// `GET /v1/info/bbo`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Bbo {
    pub instrument_id: u32,
    pub bid_price: String,
    pub bid_quantity: String,
    pub ask_price: String,
    pub ask_quantity: String,
    pub timestamp: i64,
}

/// `[price, quantity]`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(from = "(String, String)")]
pub struct BookLevel {
    pub price: String,
    pub quantity: String,
}

impl From<(String, String)> for BookLevel {
    fn from(t: (String, String)) -> Self {
        BookLevel {
            price: t.0,
            quantity: t.1,
        }
    }
}

/// `GET /v1/info/book`. An unknown instrument returns empty sides, not an error.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Book {
    pub instrument_id: u32,
    /// Highest price first.
    pub bids: Vec<BookLevel>,
    /// Lowest price first.
    pub asks: Vec<BookLevel>,
    pub timestamp: i64,
    pub sequence: u64,
}

/// `GET /v1/info/index`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct IndexPrice {
    pub asset: String,
    pub index_price: String,
    pub constituents: Vec<IndexConstituent>,
    pub ts: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct IndexConstituent {
    pub source: String,
    pub symbol: String,
    pub weight: String,
    pub price: String,
}

/// `GET /v1/info/trades` (at most 100 per page).
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Trades {
    pub data: Vec<Trade>,
    pub more: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Trade {
    pub trade_id: u64,
    pub instrument_id: u32,
    pub side: Side,
    pub price: String,
    pub quantity: String,
    pub timestamp: i64,
    /// On-chain hash, `"0x"` until mined.
    pub hash: String,
    /// Closes a position at the final settlement price.
    pub settlement: bool,
}

/// `GET /v1/info/funding` (at most 100 per page).
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct FundingHistory {
    pub data: Vec<FundingRate>,
    pub more: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct FundingRate {
    pub funding_rate: String,
    pub timestamp: i64,
}

/// `GET /v1/info/fees`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct FeesInfo {
    pub fee_schedule: Vec<FeeScheduleEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct FeeScheduleEntry {
    pub instrument_type: String,
    pub category: String,
    pub taker_fee_rate: String,
    pub maker_fee_rate: String,
    pub tiers: Vec<FeeTier>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct FeeTier {
    pub min_volume_30d: String,
    pub taker_fee_rate: String,
    pub maker_fee_rate: String,
}

/// `GET /v1/info/limit-tiers`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct LimitTier {
    pub min_volume_14d: String,
    /// Legacy; not used for request-rate enforcement.
    pub rate_per_minute_limit: u64,
    /// Legacy; not used for request-rate enforcement.
    pub rate_burst_limit: u64,
    pub actions_per_minute_limit: u64,
    pub actions_burst_limit: u64,
    pub open_orders_limit: u64,
    pub messages_per_minute: u64,
}

/// `GET /v1/info/registered`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Registration {
    pub registered: bool,
}
