//! GRVT wire types (`full` encoding). Field names and optionality follow
//! https://api-docs.grvt.io; fields marked "undocumented" were present in every live response
//! captured on 2026-11-15 (see `tests/fixtures`).

mod trading;

pub use trading::*;

use serde::{Deserialize, Serialize, Serializer};

pub(crate) fn ser_opt_u64_str<S: Serializer>(v: &Option<u64>, s: S) -> Result<S::Ok, S::Error> {
    match v {
        Some(n) => s.collect_str(n),
        None => s.serialize_none(),
    }
}

/// Error body of every REST endpoint: `{"code", "message", "status"}` (`request_id` only on
/// JSON-RPC-over-REST echoes).
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct GrvtError {
    pub code: i64,
    pub message: String,
    pub status: u16,
    #[serde(default)]
    pub request_id: Option<i64>,
}

/// `{"result": T}` wrapper used by most endpoints.
#[derive(Debug, Clone, Deserialize)]
pub struct GrvtResult<T> {
    pub result: T,
}

/// Cursor-paginated list. `next` is documented as optional (empty when there are no more pages).
#[derive(Debug, Clone, Deserialize)]
pub struct GrvtPage<T> {
    pub result: Vec<T>,
    #[serde(default)]
    pub next: Option<String>,
}

/// Reverse-chronological page window. Times are unix nanoseconds; unset fields use the
/// exchange defaults (start 0, end now, limit 500, max 1000).
#[derive(Debug, Clone, Default, Serialize)]
pub struct GrvtPageQuery {
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "ser_opt_u64_str"
    )]
    pub start_time: Option<u64>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "ser_opt_u64_str"
    )]
    pub end_time: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GrvtKind {
    Perpetual,
    Future,
    Call,
    Put,
    StablePerp,
    /// Undocumented; listed live (`GRVT_USDT_SpotSwap`, `USDC_USDT_SpotSwap`).
    SpotSwap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GrvtVenue {
    Orderbook,
    Rfq,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GrvtSettlementPeriod {
    Perpetual,
    Daily,
    Weekly,
    Monthly,
    Quarterly,
}

/// `InstrumentDisplay`.
#[derive(Debug, Clone, Deserialize)]
pub struct GrvtInstrument {
    pub instrument: String,
    /// Asset id signed as `OrderLeg.assetID`.
    pub instrument_hash: String,
    pub base: String,
    pub quote: String,
    pub kind: GrvtKind,
    pub venues: Vec<GrvtVenue>,
    /// Documented optional; absent on `SPOT_SWAP`.
    #[serde(default)]
    pub settlement_period: Option<GrvtSettlementPeriod>,
    /// Smallest base unit exponent (+3 = 0.001). Order sizes are signed as `size * 10^base_decimals`.
    pub base_decimals: i32,
    pub quote_decimals: i32,
    pub tick_size: String,
    pub min_size: String,
    pub create_time: String,
    #[serde(default)]
    pub max_position_size: Option<String>,
    #[serde(default)]
    pub funding_interval_hours: Option<u32>,
    #[serde(default)]
    pub adjusted_funding_rate_cap: Option<String>,
    #[serde(default)]
    pub adjusted_funding_rate_floor: Option<String>,
    pub min_notional: String,
    /// Undocumented: `CRYPTO`, `EQUITY`, `COMMODITY`, `COMMODITY_CME` seen live.
    pub asset_class: String,
    /// Undocumented: `ACTIVE`, `CLOSED`, `DELISTED` seen live.
    pub status: String,
    /// Undocumented, `STABLE_PERP` only.
    #[serde(default)]
    pub guaranteed_exit_band: Option<String>,
    /// Undocumented, `STABLE_PERP` only.
    #[serde(default)]
    pub spread_penalty_tolerance: Option<String>,
    /// Undocumented, `STABLE_PERP` only.
    #[serde(default)]
    pub funding_cap_adjustment_time: Option<String>,
    /// Undocumented, `STABLE_PERP` only.
    #[serde(default)]
    pub funding_cap_adjustment_value: Option<String>,
    /// Undocumented, `STABLE_PERP` only.
    #[serde(default)]
    pub triple_swap_day: Option<String>,
}

/// `full/v1/instruments` filters; unset fields mean "all".
#[derive(Debug, Clone, Default, Serialize)]
pub struct GrvtInstrumentsQuery {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<Vec<GrvtKind>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quote: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_active: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}

/// `CurrencyDetail`.
#[derive(Debug, Clone, Deserialize)]
pub struct GrvtCurrency {
    pub id: u32,
    pub symbol: String,
    pub balance_decimals: i32,
    pub quantity_multiplier: String,
}

/// `full/v1/margin_rules` response (not wrapped in `result`).
#[derive(Debug, Clone, Deserialize)]
pub struct GrvtMarginRules {
    pub instrument: String,
    pub max_position_size: String,
    pub risk_brackets: Vec<GrvtRiskBracket>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GrvtRiskBracket {
    pub tier: u32,
    pub notional_floor: String,
    /// Omitted for the last tier.
    #[serde(default)]
    pub notional_cap: Option<String>,
    pub maintenance_margin_rate: String,
    pub initial_margin_rate: String,
    pub max_leverage: u32,
    pub cumulative_maintenance_amount: String,
}

/// `MiniTicker`. Every field is documented optional (delta feeds carry only changed fields).
#[derive(Debug, Clone, Deserialize)]
pub struct GrvtMiniTicker {
    #[serde(default)]
    pub event_time: Option<String>,
    #[serde(default)]
    pub instrument: Option<String>,
    #[serde(default)]
    pub mark_price: Option<String>,
    #[serde(default)]
    pub index_price: Option<String>,
    #[serde(default)]
    pub last_price: Option<String>,
    #[serde(default)]
    pub last_size: Option<String>,
    #[serde(default)]
    pub mid_price: Option<String>,
    #[serde(default)]
    pub best_bid_price: Option<String>,
    #[serde(default)]
    pub best_bid_size: Option<String>,
    #[serde(default)]
    pub best_ask_price: Option<String>,
    #[serde(default)]
    pub best_ask_size: Option<String>,
}

/// `Ticker`. Every field is documented optional (delta feeds carry only changed fields).
#[derive(Debug, Clone, Deserialize)]
pub struct GrvtTicker {
    #[serde(default)]
    pub event_time: Option<String>,
    #[serde(default)]
    pub instrument: Option<String>,
    #[serde(default)]
    pub mark_price: Option<String>,
    #[serde(default)]
    pub index_price: Option<String>,
    #[serde(default)]
    pub last_price: Option<String>,
    #[serde(default)]
    pub last_size: Option<String>,
    #[serde(default)]
    pub mid_price: Option<String>,
    #[serde(default)]
    pub best_bid_price: Option<String>,
    #[serde(default)]
    pub best_bid_size: Option<String>,
    #[serde(default)]
    pub best_ask_price: Option<String>,
    #[serde(default)]
    pub best_ask_size: Option<String>,
    /// Deprecated by the exchange in favour of `funding_rate`.
    #[serde(default)]
    pub funding_rate_8h_curr: Option<String>,
    /// Deprecated by the exchange in favour of `funding_rate`.
    #[serde(default)]
    pub funding_rate_8h_avg: Option<String>,
    #[serde(default)]
    pub interest_rate: Option<String>,
    #[serde(default)]
    pub forward_price: Option<String>,
    #[serde(default)]
    pub buy_volume_24h_b: Option<String>,
    #[serde(default)]
    pub sell_volume_24h_b: Option<String>,
    #[serde(default)]
    pub buy_volume_24h_q: Option<String>,
    #[serde(default)]
    pub sell_volume_24h_q: Option<String>,
    #[serde(default)]
    pub high_price: Option<String>,
    #[serde(default)]
    pub low_price: Option<String>,
    #[serde(default)]
    pub open_price: Option<String>,
    #[serde(default)]
    pub open_interest: Option<String>,
    #[serde(default)]
    pub long_short_ratio: Option<String>,
    /// Current indicative funding rate for the active interval (percentage points).
    #[serde(default)]
    pub funding_rate: Option<String>,
    /// Undocumented; seen on the websocket ticker only.
    #[serde(default)]
    pub funding_interval_hours: Option<u32>,
    /// End of the current funding interval, unix nanoseconds.
    #[serde(default)]
    pub next_funding_time: Option<String>,
    /// Undocumented (`ACTIVE` live).
    #[serde(default)]
    pub status: Option<String>,
    /// Undocumented (`REGULAR` live).
    #[serde(default)]
    pub session: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GrvtOrderbookLevel {
    pub price: String,
    pub size: String,
    pub num_orders: u32,
}

/// `OrderbookLevels`.
#[derive(Debug, Clone, Deserialize)]
pub struct GrvtOrderbook {
    pub event_time: String,
    pub instrument: String,
    pub bids: Vec<GrvtOrderbookLevel>,
    pub asks: Vec<GrvtOrderbookLevel>,
}

/// Public `Trade`.
#[derive(Debug, Clone, Deserialize)]
pub struct GrvtTrade {
    pub event_time: String,
    pub instrument: String,
    pub is_taker_buyer: bool,
    pub size: String,
    pub price: String,
    #[serde(default)]
    pub mark_price: Option<String>,
    pub index_price: String,
    pub interest_rate: String,
    #[serde(default)]
    pub forward_price: Option<String>,
    pub trade_id: String,
    pub venue: GrvtVenue,
    pub is_rpi: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GrvtCandleInterval {
    #[serde(rename = "CI_1_M")]
    OneMinute,
    #[serde(rename = "CI_3_M")]
    ThreeMinutes,
    #[serde(rename = "CI_5_M")]
    FiveMinutes,
    #[serde(rename = "CI_15_M")]
    FifteenMinutes,
    #[serde(rename = "CI_30_M")]
    ThirtyMinutes,
    #[serde(rename = "CI_1_H")]
    OneHour,
    #[serde(rename = "CI_2_H")]
    TwoHours,
    #[serde(rename = "CI_4_H")]
    FourHours,
    #[serde(rename = "CI_6_H")]
    SixHours,
    #[serde(rename = "CI_8_H")]
    EightHours,
    #[serde(rename = "CI_12_H")]
    TwelveHours,
    #[serde(rename = "CI_1_D")]
    OneDay,
    #[serde(rename = "CI_3_D")]
    ThreeDays,
    #[serde(rename = "CI_5_D")]
    FiveDays,
    #[serde(rename = "CI_1_W")]
    OneWeek,
    #[serde(rename = "CI_2_W")]
    TwoWeeks,
    #[serde(rename = "CI_3_W")]
    ThreeWeeks,
    #[serde(rename = "CI_4_W")]
    FourWeeks,
}

impl GrvtCandleInterval {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OneMinute => "CI_1_M",
            Self::ThreeMinutes => "CI_3_M",
            Self::FiveMinutes => "CI_5_M",
            Self::FifteenMinutes => "CI_15_M",
            Self::ThirtyMinutes => "CI_30_M",
            Self::OneHour => "CI_1_H",
            Self::TwoHours => "CI_2_H",
            Self::FourHours => "CI_4_H",
            Self::SixHours => "CI_6_H",
            Self::EightHours => "CI_8_H",
            Self::TwelveHours => "CI_12_H",
            Self::OneDay => "CI_1_D",
            Self::ThreeDays => "CI_3_D",
            Self::FiveDays => "CI_5_D",
            Self::OneWeek => "CI_1_W",
            Self::TwoWeeks => "CI_2_W",
            Self::ThreeWeeks => "CI_3_W",
            Self::FourWeeks => "CI_4_W",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GrvtCandleType {
    Trade,
    Mark,
    Index,
    Mid,
}

impl GrvtCandleType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Trade => "TRADE",
            Self::Mark => "MARK",
            Self::Index => "INDEX",
            Self::Mid => "MID",
        }
    }
}

/// `Candlestick`.
#[derive(Debug, Clone, Deserialize)]
pub struct GrvtCandle {
    pub open_time: String,
    pub close_time: String,
    pub open: String,
    pub close: String,
    pub high: String,
    pub low: String,
    pub volume_b: String,
    pub volume_q: String,
    pub trades: u64,
    pub instrument: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GrvtFundingAggregation {
    FundingInterval,
    OneHourly,
    FourHourly,
    EightHourly,
}

/// `ApiFundingRate`.
#[derive(Debug, Clone, Deserialize)]
pub struct GrvtFundingRate {
    pub instrument: String,
    /// Percentage points.
    pub funding_rate: String,
    pub funding_time: String,
    pub mark_price: String,
    /// Deprecated by the exchange in favour of `funding_rate`.
    pub funding_rate_8_h_avg: String,
    pub funding_interval_hours: u32,
}

/// `GET /time` on the market-data host (linked from the docs; used by GRVT's examples to set
/// signature expirations).
#[derive(Debug, Clone, Deserialize)]
pub struct GrvtServerTime {
    /// Unix milliseconds.
    pub server_time: u64,
}

/// `SupportedAsset`.
#[derive(Debug, Clone, Deserialize)]
pub struct GrvtSupportedAsset {
    pub asset_id: u32,
    pub asset_code: String,
    pub balance_decimals: i32,
    /// Undocumented; present on every asset live.
    pub transfer_disabled: bool,
}

/// `SubAccountMode`. The docs list only `SINGLE_ASSET_MODE`; `MULTI_ASSET_MODE` is listed live.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GrvtSubAccountMode {
    SingleAssetMode,
    MultiAssetMode,
}

/// `FuturesWalletAssets`.
#[derive(Debug, Clone, Deserialize)]
pub struct GrvtFuturesWalletAssets {
    pub mode: GrvtSubAccountMode,
    pub supported_assets: Vec<GrvtSupportedAsset>,
}

/// `full/v1/supported_assets` response (not wrapped in `result`).
#[derive(Debug, Clone, Deserialize)]
pub struct GrvtSupportedAssets {
    pub funding: Vec<GrvtSupportedAsset>,
    pub spot: Vec<GrvtSupportedAsset>,
    pub futures: Vec<GrvtFuturesWalletAssets>,
}

/// `CDCStatus` (collateral deposit cap utilisation).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GrvtCdcStatus {
    Normal,
    ReduceOnly,
    AutoExchange,
}

/// `ApiCollateralAssetInfo`. Ratios and rates are percentage points.
#[derive(Debug, Clone, Deserialize)]
pub struct GrvtCollateralAssetInfo {
    pub currency: String,
    pub collateral_value_ratio: String,
    /// Native units; `"0.0"` means untracked/unlimited.
    pub collateral_deposit_cap: String,
    pub max_borrow_rate: String,
    pub manual_repayment_fee_rate: String,
    pub cdc_status: GrvtCdcStatus,
    /// Undocumented; present on every asset live.
    pub show_only_when_held: bool,
}

/// `full/v1/get_all_collateral_asset_info` response (not wrapped in `result`).
#[derive(Debug, Clone, Deserialize)]
pub struct GrvtCollateralAssets {
    pub assets: Vec<GrvtCollateralAssetInfo>,
}

// ---------------------------------------------------------------------------------------------
// Market-data request bodies (REST body = websocket JSON-RPC `params`)
// ---------------------------------------------------------------------------------------------

/// `{}` for parameterless endpoints.
#[derive(Debug, Clone, Copy, Default, Serialize)]
pub struct GrvtEmptyRequest {}

/// `instrument`, `mini`, `ticker`, `margin_rules`.
#[derive(Debug, Clone, Serialize)]
pub struct GrvtInstrumentRequest<'a> {
    pub instrument: &'a str,
}

/// `all_instruments`. Unset `kinds` uses the exchange default (`PERPETUAL` only).
#[derive(Debug, Clone, Serialize)]
pub struct GrvtAllInstrumentsRequest<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_active: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kinds: Option<&'a [GrvtKind]>,
}

/// `book`. `depth` must be 10, 50, 100 or 500 (else code 3031).
#[derive(Debug, Clone, Serialize)]
pub struct GrvtBookRequest<'a> {
    pub instrument: &'a str,
    pub depth: u32,
}

/// `trade` (`limit` max 1000).
#[derive(Debug, Clone, Serialize)]
pub struct GrvtTradeRequest<'a> {
    pub instrument: &'a str,
    pub limit: u32,
}

/// `trade_history`.
#[derive(Debug, Clone, Serialize)]
pub struct GrvtTradeHistoryRequest<'a> {
    pub instrument: &'a str,
    #[serde(flatten)]
    pub page: &'a GrvtPageQuery,
}

/// `kline`.
#[derive(Debug, Clone, Serialize)]
pub struct GrvtKlineRequest<'a> {
    pub instrument: &'a str,
    pub interval: GrvtCandleInterval,
    #[serde(rename = "type")]
    pub kind: GrvtCandleType,
    #[serde(flatten)]
    pub page: &'a GrvtPageQuery,
}

/// `funding`. Unset `agg_type` uses the exchange default (`FUNDING_INTERVAL`).
#[derive(Debug, Clone, Serialize)]
pub struct GrvtFundingRequest<'a> {
    pub instrument: &'a str,
    #[serde(flatten)]
    pub page: &'a GrvtPageQuery,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agg_type: Option<GrvtFundingAggregation>,
}

// ---------------------------------------------------------------------------------------------
// Websocket (JSON-RPC 2.0 on `/ws/full`)
// ---------------------------------------------------------------------------------------------

/// Public market-data streams.
pub const STREAM_MINI_TICKER_SNAPSHOT: &str = "v1.mini.s";
pub const STREAM_MINI_TICKER_DELTA: &str = "v1.mini.d";
pub const STREAM_TICKER_SNAPSHOT: &str = "v1.ticker.s";
pub const STREAM_TICKER_DELTA: &str = "v1.ticker.d";
pub const STREAM_BOOK_SNAPSHOT: &str = "v1.book.s";
pub const STREAM_BOOK_DELTA: &str = "v1.book.d";
pub const STREAM_TRADE: &str = "v1.trade";
pub const STREAM_CANDLE: &str = "v1.candle";
/// Private trading streams.
pub const STREAM_ORDER: &str = "v1.order";
pub const STREAM_ORDER_STATE: &str = "v1.state";
pub const STREAM_CANCEL_STATUS: &str = "v1.cancel";
pub const STREAM_FILL: &str = "v1.fill";
pub const STREAM_POSITION: &str = "v1.position";

/// Feed selectors (`primary@secondary`, fields joined by `-`).
pub mod selector {
    use super::{GrvtCandleInterval, GrvtCandleType};

    /// `v1.mini.s` / `v1.mini.d` / `v1.ticker.s` / `v1.ticker.d`: `{instrument}@{rate_ms}`.
    pub fn ticker(instrument: &str, rate_ms: u32) -> String {
        format!("{instrument}@{rate_ms}")
    }

    /// `v1.book.s`: `{instrument}@{rate_ms}-{depth}`.
    pub fn book_snapshot(instrument: &str, rate_ms: u32, depth: u32) -> String {
        format!("{instrument}@{rate_ms}-{depth}")
    }

    /// `v1.book.d`: `{instrument}@{rate_ms}` (unlimited depth).
    pub fn book_delta(instrument: &str, rate_ms: u32) -> String {
        format!("{instrument}@{rate_ms}")
    }

    /// `v1.trade`: `{instrument}@{limit}` (snapshot size on subscribe).
    pub fn trade(instrument: &str, limit: u32) -> String {
        format!("{instrument}@{limit}")
    }

    /// `v1.candle`: `{instrument}@{interval}-{type}`.
    pub fn candle(instrument: &str, interval: GrvtCandleInterval, kind: GrvtCandleType) -> String {
        format!("{instrument}@{}-{}", interval.as_str(), kind.as_str())
    }

    /// Private streams: `{sub_account_id}` or `{sub_account_id}-{instrument}`.
    pub fn account(sub_account_id: u64, instrument: Option<&str>) -> String {
        match instrument {
            Some(i) => format!("{sub_account_id}-{i}"),
            None => sub_account_id.to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct GrvtStreamParams<'a> {
    pub stream: &'a str,
    pub selectors: &'a [String],
}

/// JSON-RPC 2.0 request. `id` is optional; live (2026-11-16) a market-data query without it is
/// answered with `"id":0`.
#[derive(Debug, Clone, Serialize)]
pub struct GrvtRpcRequest<'a, P: Serialize> {
    pub jsonrpc: &'static str,
    pub method: &'a str,
    pub params: P,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<u64>,
}

impl<'a, P: Serialize> GrvtRpcRequest<'a, P> {
    pub fn new(method: &'a str, params: P, id: Option<u64>) -> Self {
        Self {
            jsonrpc: "2.0",
            method,
            params,
            id,
        }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("rpc request serialises")
    }
}

/// `{"jsonrpc":"2.0","method":"subscribe","params":{"stream","selectors"},"id"}`.
pub fn subscribe_message(stream: &str, selectors: &[String], id: Option<u64>) -> String {
    GrvtRpcRequest::new("subscribe", GrvtStreamParams { stream, selectors }, id).to_json()
}

/// `{"jsonrpc":"2.0","method":"unsubscribe","params":{"stream","selectors"},"id"}`.
pub fn unsubscribe_message(stream: &str, selectors: &[String], id: Option<u64>) -> String {
    GrvtRpcRequest::new("unsubscribe", GrvtStreamParams { stream, selectors }, id).to_json()
}

/// JSON-RPC error object.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct GrvtRpcError {
    pub code: i64,
    pub message: String,
}

/// JSON-RPC response: exactly one of `result` / `error` is present.
#[derive(Debug, Clone, Deserialize)]
pub struct GrvtRpcResponse<R> {
    pub jsonrpc: String,
    #[serde(default = "none")]
    pub result: Option<R>,
    #[serde(default)]
    pub error: Option<GrvtRpcError>,
    #[serde(default)]
    pub id: Option<u64>,
    /// Echoed on subscribe/unsubscribe replies; absent from the order RPC reply in the SDK's
    /// recorded sample (`grvt_ccxt_ws.py`).
    #[serde(default)]
    pub method: Option<String>,
}

fn none<R>() -> Option<R> {
    None
}

impl<R> GrvtRpcResponse<R> {
    /// `result`, or the exchange's error code and message.
    pub fn into_result(self) -> Result<R, GrvtRpcError> {
        match (self.result, self.error) {
            (Some(r), None) => Ok(r),
            (_, Some(e)) => Err(e),
            (None, None) => Err(GrvtRpcError {
                code: 0,
                message: "response has neither result nor error".to_string(),
            }),
        }
    }
}

/// `WSSubscribeResult`.
#[derive(Debug, Clone, Deserialize)]
pub struct GrvtSubscribeResult {
    pub stream: String,
    pub subs: Vec<String>,
    pub unsubs: Vec<String>,
    pub num_snapshots: Vec<u64>,
    pub first_sequence_number: Vec<String>,
    /// Undocumented; present live.
    #[serde(default)]
    pub latest_sequence_number: Option<Vec<String>>,
}

/// `WSUnsubscribeResult`.
#[derive(Debug, Clone, Deserialize)]
pub struct GrvtUnsubscribeResult {
    pub stream: String,
    pub unsubs: Vec<String>,
}

/// Stream payload. Snapshots have `sequence_number` `"0"`; deltas count up by one per
/// stream/selector on a connection.
#[derive(Debug, Clone, Deserialize)]
pub struct GrvtWsFeed<T> {
    pub stream: String,
    pub selector: String,
    pub sequence_number: String,
    pub feed: T,
    pub prev_sequence_number: String,
}
