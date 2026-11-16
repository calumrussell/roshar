//! Wire types, named and shaped as Orderly defines them.
//!
//! Market types are built from live responses (`tests/fixtures`). A field is `Option` only where
//! the docs mark it optional/nullable for some symbols (e.g. Predict-only fields) or live responses
//! show it `null`/absent. Numbers are `f64`; timestamps are unix ms unless noted.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// For fields that are always sent but may be `null`: the key must be present (no `default`).
/// Plain `Option` fields (no `deserialize_with`) are the ones live data shows absent.
fn nullable<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}

pub mod trading;

// ── REST envelope ────────────────────────────────────────────────────────────

/// `meta` of paginated responses.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyPageMeta {
    pub total: u64,
    pub records_per_page: u32,
    /// From 1.
    pub current_page: u32,
}

/// `data` of paginated responses: `{"rows": [...], "meta": {...}}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyPage<T> {
    pub rows: Vec<T>,
    pub meta: OrderlyPageMeta,
}

// ── Public market data ───────────────────────────────────────────────────────

/// Symbol and order rules from `GET /v1/public/info[/{symbol}]`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlySymbol {
    pub symbol: String,
    pub display_symbol_name: String,
    /// Owning builder for builder-listed markets; `null` for Orderly's own markets.
    pub broker_id: Option<String>,
    /// `ACTIVE`, ...
    pub status: String,
    pub quote_min: f64,
    pub quote_max: f64,
    /// Price tick.
    pub quote_tick: f64,
    pub base_min: f64,
    pub base_max: f64,
    /// Quantity step.
    pub base_tick: f64,
    pub min_notional: f64,
    /// Max deviation of a limit price from the mark price.
    pub price_range: f64,
    pub price_scope: f64,
    pub std_liquidation_fee: f64,
    pub liquidator_fee: f64,
    pub claim_insurance_fund_discount: f64,
    /// Hours.
    pub funding_period: u32,
    pub cap_funding: f64,
    pub floor_funding: f64,
    pub interest_rate: f64,
    pub cap_ir: f64,
    pub floor_ir: f64,
    pub created_time: u64,
    pub updated_time: u64,
    pub liquidation_tier: u32,
    pub mark_index_price_deviation_floor: f64,
    pub mark_index_price_deviation_cap: f64,
    pub global_max_oi_cap: f64,
    pub base_mmr: f64,
    pub base_imr: f64,
    pub imr_factor: f64,
    pub deviation_factor: f64,
    pub is_pretge: bool,
    /// Session-based (RWA) markets only.
    pub base_imr_on_market: Option<f64>,
    /// Session-based (RWA) markets only.
    pub base_imr_off_market: Option<f64>,
    /// Session-based (RWA) markets only.
    pub deviation_factor_on_market: Option<f64>,
    /// Session-based (RWA) markets only.
    pub deviation_factor_off_market: Option<f64>,
}

/// Market snapshot from `GET /v1/public/futures[/{symbol}]`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyFuture {
    pub symbol: String,
    pub display_symbol_name: String,
    pub broker_id: Option<String>,
    pub status: String,
    pub index_price: f64,
    pub mark_price: f64,
    pub sum_unitary_funding: f64,
    pub est_funding_rate: f64,
    pub last_funding_rate: f64,
    pub next_funding_time: u64,
    pub open_interest: f64,
    pub is_pretge: bool,
    #[serde(rename = "24h_open")]
    pub open_24h: f64,
    #[serde(rename = "24h_close")]
    pub close_24h: f64,
    #[serde(rename = "24h_high")]
    pub high_24h: f64,
    #[serde(rename = "24h_low")]
    pub low_24h: f64,
    /// Base units.
    #[serde(rename = "24h_volume")]
    pub volume_24h: f64,
    /// Quote units.
    #[serde(rename = "24h_amount")]
    pub amount_24h: f64,
}

/// Public trade from `GET /v1/public/market_trades`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyMarketTrade {
    pub symbol: String,
    /// Taker side: `BUY` / `SELL`.
    pub side: String,
    pub executed_price: f64,
    pub executed_quantity: f64,
    pub executed_timestamp: u64,
}

/// Predicted funding from `GET /v1/public/funding_rates` and `/v1/public/funding_rate/{symbol}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyFundingRate {
    pub symbol: String,
    /// Rolling estimate for the current period.
    pub est_funding_rate: f64,
    pub est_funding_rate_timestamp: u64,
    pub last_funding_rate: f64,
    pub last_funding_rate_timestamp: u64,
    pub next_funding_time: u64,
    pub sum_unitary_funding: f64,
}

/// Settled funding from `GET /v1/public/funding_rate_history`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyFundingRateHistory {
    pub symbol: String,
    pub funding_rate: f64,
    pub funding_rate_timestamp: u64,
    pub next_funding_time: u64,
}

/// One window of `GET /v1/public/market_info/funding_history`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyFundingStat {
    /// Summed funding rate over the window.
    pub rate: f64,
    /// Count of positive funding periods.
    pub positive: u32,
    /// Count of negative funding periods.
    pub negative: u32,
}

/// Windows of `GET /v1/public/market_info/funding_history`. A window is `null` when the symbol
/// is younger than it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyFundingWindows {
    pub last: OrderlyFundingStat,
    #[serde(rename = "1d")]
    pub d1: Option<OrderlyFundingStat>,
    #[serde(rename = "3d")]
    pub d3: Option<OrderlyFundingStat>,
    #[serde(rename = "7d")]
    pub d7: Option<OrderlyFundingStat>,
    #[serde(rename = "14d")]
    pub d14: Option<OrderlyFundingStat>,
    #[serde(rename = "30d")]
    pub d30: Option<OrderlyFundingStat>,
    #[serde(rename = "90d")]
    pub d90: Option<OrderlyFundingStat>,
    #[serde(rename = "180d")]
    pub d180: Option<OrderlyFundingStat>,
}

/// Row of `GET /v1/public/market_info/funding_history`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyFundingSummary {
    pub symbol: String,
    /// `YYYY-MM-DD HH:MM:SS` (UTC).
    pub data_start_time: String,
    pub funding: OrderlyFundingWindows,
}

/// Row of `GET /v1/public/market_info/price_changes` and of the `price_changes` topic. A window
/// the symbol is younger than is `null` over REST and absent from websocket pushes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyPriceChanges {
    pub symbol: String,
    pub last_price: f64,
    #[serde(rename = "5m")]
    pub m5: Option<f64>,
    #[serde(rename = "30m")]
    pub m30: Option<f64>,
    #[serde(rename = "1h")]
    pub h1: Option<f64>,
    #[serde(rename = "4h")]
    pub h4: Option<f64>,
    #[serde(rename = "24h")]
    pub h24: Option<f64>,
    #[serde(rename = "3d")]
    pub d3: Option<f64>,
    #[serde(rename = "7d")]
    pub d7: Option<f64>,
    #[serde(rename = "30d")]
    pub d30: Option<f64>,
}

/// Row of `GET /v1/public/market_info/traders_open_interests`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyTradersOpenInterest {
    pub symbol: String,
    pub long_oi: f64,
    /// Negative.
    pub short_oi: f64,
}

/// TradingView-style bars from `GET /v1/tv/kline_history` and `GET /v1/tv/history` (not
/// wrapped in the `success`/`data` envelope). Arrays are column-aligned; `s` is `ok` or
/// `no_data` (all arrays empty).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyTvBars {
    pub s: String,
    /// Bar open time, unix seconds.
    pub t: Vec<i64>,
    pub o: Vec<f64>,
    pub h: Vec<f64>,
    pub l: Vec<f64>,
    pub c: Vec<f64>,
    /// Base volume.
    pub v: Vec<f64>,
    /// Quote volume.
    pub a: Vec<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyScheduledMaintenance {
    #[serde(rename = "startTime")]
    pub start_time: u64,
    #[serde(rename = "endTime")]
    pub end_time: u64,
}

/// `GET /v1/public/system_info`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlySystemInfo {
    /// `0` normal, `2` maintenance.
    pub status: i32,
    pub msg: String,
    pub scheduled_maintenance: Option<OrderlyScheduledMaintenance>,
}

// ── Market data behind an Orderly key ────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyBookLevel {
    pub price: f64,
    pub quantity: f64,
}

/// `GET /v1/orderbook/{symbol}`; both sides best price first.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyOrderbook {
    pub asks: Vec<OrderlyBookLevel>,
    pub bids: Vec<OrderlyBookLevel>,
    /// Predict symbols only.
    pub down_asks: Option<Vec<OrderlyBookLevel>>,
    /// Predict symbols only.
    pub down_bids: Option<Vec<OrderlyBookLevel>>,
    pub timestamp: u64,
}

/// Row of `GET /v1/kline`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyKline {
    pub symbol: String,
    /// Interval, e.g. `1m`.
    #[serde(rename = "type")]
    pub kline_type: String,
    pub open: f64,
    pub close: f64,
    pub low: f64,
    pub high: f64,
    pub volume: f64,
    pub amount: f64,
    pub start_timestamp: u64,
    pub end_timestamp: u64,
}

// ── More public market data ──────────────────────────────────────────────────

/// One point of `price_list` (`GET /v1/public/market_info/history_charts` and the
/// `history_charts_{interval}` topic).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyChartPoint {
    pub ts: u64,
    /// Close price.
    pub price: f64,
}

/// Row of `GET /v1/public/market_info/history_charts` and of the `history_charts_{interval}`
/// topic. Newest point first.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyHistoryChart {
    pub symbol: String,
    pub price_list: Vec<OrderlyChartPoint>,
}

/// Row of `GET /v1/public/index_price` (ascending by `timestamp`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyIndexPricePoint {
    pub timestamp: u64,
    pub index_price: f64,
}

/// Row of `GET /v1/public/index_price_source`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyIndexPriceSource {
    /// Index symbol, e.g. `SPOT_BTC_USDC`.
    pub symbol: String,
    /// Venues feeding the index, e.g. `binance`.
    pub sources: Vec<String>,
    pub updated_time: u64,
}

/// Position row of `GET /v1/public/insurancefund`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyInsuranceFundPosition {
    pub symbol: String,
    pub timestamp: u64,
    pub position_qty: f64,
    pub cost_position: f64,
    pub last_sum_unitary_funding: f64,
    pub settle_price: f64,
    pub average_open_price: f64,
    pub pnl_24_h: f64,
    pub fee_24_h: f64,
    pub mark_price: f64,
}

/// `GET /v1/public/insurancefund`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyInsuranceFund {
    pub rows: Vec<OrderlyInsuranceFundPosition>,
    pub min_insurance_fund_initial_margin_ratio: f64,
    pub min_insurance_fund_margin_ratio: f64,
    pub margin_ratio: f64,
    pub total_collateral_value: f64,
    pub balance: f64,
    pub free_collateral: f64,
    pub total_account_value: f64,
    pub total_pnl_24_h: f64,
}

/// `GET /v1/public/config`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyLeverageConfig {
    /// Comma-separated leverage choices, e.g. `1,2,3,4,5,10,...`.
    pub available_futures_leverage: String,
}

/// `GET /v1/public/leverage`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyMaxLeverage {
    /// Decimal text, e.g. `100`.
    pub max_futures_leverage: String,
}

/// Position of a row of `GET /v1/public/liquidated_positions`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyLiquidatedPerp {
    pub symbol: String,
    pub position_qty: f64,
    pub liquidator_fee: f64,
    pub cost_position_transfer: f64,
    /// Absent live on some rows (e.g. multi-position liquidations).
    pub transfer_price: Option<f64>,
    pub insurance_fund_fee: f64,
    pub abs_liquidation_fee: f64,
    pub abs_insurance_fund_fee: f64,
    pub mark_price: f64,
}

/// Row of `GET /v1/public/liquidated_positions`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyLiquidatedPosition {
    pub timestamp: u64,
    /// e.g. `liquidated`.
    #[serde(rename = "type")]
    pub liquidation_type: String,
    pub liquidation_id: u64,
    pub transfer_amount_to_insurance_fund: f64,
    pub positions_by_perp: Vec<OrderlyLiquidatedPerp>,
    pub margin_ratio: f64,
    pub account_mmr: f64,
    pub position_notional: f64,
    pub collateral_value: f64,
    /// `CROSS` / `ISOLATED`.
    pub margin_mode: String,
}

/// Position of a row of `GET /v1/public/liquidation`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyLiquidationPerp {
    pub symbol: String,
    pub position_qty: f64,
    pub liquidator_fee: f64,
}

/// Row of `GET /v1/public/liquidation` (positions currently up for liquidation). Typed from the
/// documented schema; live responses were empty.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyLiquidation {
    pub timestamp: u64,
    #[serde(rename = "type")]
    pub liquidation_type: String,
    pub liquidation_id: u64,
    pub positions_by_perp: Vec<OrderlyLiquidationPerp>,
}

/// Per-chain details of a row of `GET /v1/public/token`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyTokenChain {
    pub chain_id: String,
    /// Empty for the chain's native token.
    pub contract_address: String,
    pub decimals: u32,
    pub withdrawal_fee: f64,
    pub cross_chain_withdrawal_fee: f64,
    pub display_name: String,
    pub exclusive_deposit_supported: bool,
}

/// Row of `GET /v1/public/token` (collateral tokens).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyToken {
    pub token: String,
    pub token_hash: String,
    pub decimals: u32,
    pub minimum_withdraw_amount: f64,
    pub base_weight: f64,
    /// `null` for some tokens (e.g. USDC).
    #[serde(deserialize_with = "nullable")]
    pub discount_factor: Option<f64>,
    pub haircut: f64,
    pub user_max_qty: f64,
    pub is_collateral: bool,
    pub on_chain_swap: bool,
    pub chain_details: Vec<OrderlyTokenChain>,
}

/// Row of `GET /v1/public/chain_info`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyChainInfo {
    pub name: String,
    pub public_rpc_url: String,
    pub chain_id: String,
    pub currency_symbol: String,
    pub currency_decimal: u32,
    pub explorer_base_url: String,
    pub vault_address: String,
    pub broker_ids: Vec<String>,
}

/// Orderly Predict round from `GET /v1/public/binary/info`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyBinaryRound {
    pub symbol: String,
    /// `BINARY`.
    pub product_type: String,
    pub status: String,
    /// Nullable per the docs.
    #[serde(deserialize_with = "nullable")]
    pub activated_at: Option<u64>,
    pub observation_start_at: u64,
    pub expire_at: u64,
    pub index_price_symbol: String,
    pub settlement_index_symbol: String,
    pub fee_model: String,
    pub quote_min: f64,
    pub quote_max: f64,
    pub quote_tick: f64,
    pub base_min: f64,
    pub base_max: f64,
    pub base_tick: f64,
    pub min_notional: f64,
    /// `null` until observed.
    #[serde(deserialize_with = "nullable")]
    pub s0: Option<f64>,
    /// `null` until observed.
    #[serde(deserialize_with = "nullable")]
    pub s1: Option<f64>,
    /// `UP` / `DOWN`; `null` until the result is published.
    #[serde(deserialize_with = "nullable")]
    pub outcome: Option<String>,
    #[serde(deserialize_with = "nullable")]
    pub settlement_price_per_share: Option<f64>,
    #[serde(deserialize_with = "nullable")]
    pub result_committed_at: Option<u64>,
    pub rule_version: String,
    pub funding_period: f64,
    pub cap_funding: f64,
    pub floor_funding: f64,
    pub interest_rate: f64,
    pub cap_ir: f64,
    pub floor_ir: f64,
    pub mark_index_price_deviation_floor: f64,
    pub mark_index_price_deviation_cap: f64,
    pub imr_factor: f64,
    pub deviation_factor: f64,
}

/// One observation of `price_evidence`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyBinaryEvidence {
    pub price: Option<f64>,
    pub timestamp: Option<u64>,
    pub source: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyBinaryPriceEvidence {
    pub s0: Option<OrderlyBinaryEvidence>,
    pub s1: Option<OrderlyBinaryEvidence>,
}

/// `GET /v1/public/binary/info/{symbol}`: the round plus settlement evidence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyBinaryRoundDetail {
    #[serde(flatten)]
    pub round: OrderlyBinaryRound,
    #[serde(deserialize_with = "nullable")]
    pub price_evidence: Option<OrderlyBinaryPriceEvidence>,
    /// Unspecified object in the docs; kept raw.
    #[serde(deserialize_with = "nullable")]
    pub manual_correction: Option<serde_json::Value>,
}

/// `GET /v1/tv/symbol_info` (TradingView UDF, not enveloped). Column-aligned arrays, one entry
/// per symbol. Fields the docs list but the exchange does not send are not modelled.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyTvSymbolInfo {
    pub s: String,
    pub symbol: Vec<String>,
    pub currency: Vec<String>,
    pub description: Vec<String>,
    pub minmovement: Vec<f64>,
    pub pointvalue: Vec<f64>,
    pub pricescale: Vec<u64>,
    pub timezone: Vec<String>,
    #[serde(rename = "type")]
    pub symbol_type: Vec<String>,
    #[serde(rename = "base-currency")]
    pub base_currency: Vec<String>,
    #[serde(rename = "exchange-listed")]
    pub exchange_listed: Vec<String>,
    #[serde(rename = "exchange-traded")]
    pub exchange_traded: Vec<String>,
    #[serde(rename = "has-intraday")]
    pub has_intraday: Vec<bool>,
    #[serde(rename = "session-regular")]
    pub session_regular: Vec<String>,
    #[serde(rename = "has-daily")]
    pub has_daily: Vec<bool>,
}

/// `GET /v1/tv/config` as the exchange sends it (TradingView UDF datafeed config, not
/// enveloped). The documented trading-terminal schema (`s`, `d`) is not what the server returns.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyTvConfig {
    pub supported_resolutions: Vec<String>,
    pub supports_marks: bool,
    pub supports_timescale_marks: bool,
    pub supports_group_request: bool,
    pub supports_search: bool,
}

// ── Public Info API (`POST /v1/public/query`) ────────────────────────────────
//
// Monetary values are decimal strings, kept as text.

/// `data` of paginated query types: `{"rows": [...], "next_cursor": ...}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyQueryPage<T> {
    pub rows: Vec<T>,
    /// Opaque; `null` on the last page.
    #[serde(deserialize_with = "nullable")]
    pub next_cursor: Option<String>,
}

/// Row of `marketSummary.markets`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyQueryMarket {
    pub symbol: String,
    pub mark_price: String,
    pub index_price: String,
    #[serde(rename = "24h_open")]
    pub open_24h: String,
    #[serde(rename = "24h_close")]
    pub close_24h: String,
    #[serde(rename = "24h_high")]
    pub high_24h: String,
    #[serde(rename = "24h_low")]
    pub low_24h: String,
    #[serde(rename = "24h_volume")]
    pub volume_24h: String,
    #[serde(rename = "24h_amount")]
    pub amount_24h: String,
    pub open_interest: String,
    pub last_funding_rate: String,
    /// `null` until the first estimate.
    #[serde(deserialize_with = "nullable")]
    pub est_funding_rate: Option<String>,
    pub next_funding_time: u64,
    /// `null` when that side of the book is empty.
    #[serde(deserialize_with = "nullable")]
    pub bid_price: Option<String>,
    /// `null` when that side of the book is empty.
    #[serde(deserialize_with = "nullable")]
    pub ask_price: Option<String>,
    pub max_leverage: String,
    pub min_notional: String,
    pub quote_tick: String,
    pub base_tick: String,
}

/// `marketSummary`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyQueryMarketSummary {
    /// `null` if no data.
    #[serde(deserialize_with = "nullable")]
    pub total_24h_volume: Option<String>,
    /// `null` if no symbol has open-interest data.
    #[serde(deserialize_with = "nullable")]
    pub total_open_interest: Option<String>,
    pub markets: Vec<OrderlyQueryMarket>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyQueryLevel {
    pub price: String,
    pub quantity: String,
}

/// `orderbook`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyQueryOrderbook {
    pub symbol: String,
    /// `null` if either side is empty.
    #[serde(deserialize_with = "nullable")]
    pub mid_price: Option<String>,
    /// `null` if either side is empty.
    #[serde(deserialize_with = "nullable")]
    pub spread: Option<String>,
    pub asks: Vec<OrderlyQueryLevel>,
    pub bids: Vec<OrderlyQueryLevel>,
    pub ts: u64,
}

/// Row of `candles`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyQueryCandle {
    pub open: String,
    pub high: String,
    pub low: String,
    pub close: String,
    /// Base volume.
    pub volume: String,
    /// Open time.
    pub timestamp: u64,
}

/// Row of `marketTrades` (taker side only).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyQueryTrade {
    pub symbol: String,
    pub side: String,
    pub executed_price: String,
    pub executed_quantity: String,
    pub address: String,
    pub account_id: String,
    pub broker_id: String,
    pub executed_timestamp: u64,
}

/// Row of `liquidations`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyQueryLiquidation {
    pub symbol: String,
    /// `LONG` / `SHORT` (the liquidated side).
    pub side: String,
    pub position_qty: String,
    pub notional: String,
    pub mark_price: String,
    pub est_liq_price: String,
    /// `null` if the user lookup fails.
    #[serde(deserialize_with = "nullable")]
    pub address: Option<String>,
    #[serde(deserialize_with = "nullable")]
    pub account_id: Option<String>,
    #[serde(deserialize_with = "nullable")]
    pub broker_id: Option<String>,
    pub timestamp: u64,
}

/// Row of `fundingRateHistory`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyQueryFundingRate {
    pub funding_rate: String,
    pub mark_price: String,
    pub funding_rate_timestamp: u64,
}

/// Venue entry of `fundingComparison`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyQueryVenueFunding {
    pub name: String,
    pub last: String,
    #[serde(rename = "1d")]
    pub d1: String,
    #[serde(rename = "7d")]
    pub d7: String,
    #[serde(rename = "30d")]
    pub d30: String,
}

/// Row of `fundingComparison`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyQueryFundingComparison {
    pub symbol: String,
    pub next_funding_time: u64,
    pub exchanges: Vec<OrderlyQueryVenueFunding>,
}

/// `marketDetail.market_info` (a subset of the `marketSummary` row, as sent live).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyQueryMarketInfo {
    pub mark_price: String,
    pub index_price: String,
    pub last_funding_rate: String,
    #[serde(deserialize_with = "nullable")]
    pub est_funding_rate: Option<String>,
    pub next_funding_time: u64,
    pub open_interest: String,
    #[serde(rename = "24h_volume")]
    pub volume_24h: String,
    #[serde(rename = "24h_high")]
    pub high_24h: String,
    #[serde(rename = "24h_low")]
    pub low_24h: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyQueryBook {
    pub asks: Vec<OrderlyQueryLevel>,
    pub bids: Vec<OrderlyQueryLevel>,
}

/// `marketDetail`. Sections left out of `include` are absent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyQueryMarketDetail {
    pub symbol: String,
    pub market_info: Option<OrderlyQueryMarketInfo>,
    pub orderbook: Option<OrderlyQueryBook>,
    pub recent_trades: Option<Vec<OrderlyQueryTrade>>,
    pub funding_history: Option<Vec<OrderlyQueryFundingRate>>,
    pub candles: Option<Vec<OrderlyQueryCandle>>,
}

// ── Account lookup (public) ──────────────────────────────────────────────────

/// `GET /v1/get_account`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyAccountLookup {
    pub user_id: u64,
    pub account_id: String,
}

/// `GET /v1/get_orderly_key`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyKeyLookup {
    pub orderly_key: String,
    /// e.g. `read,trading`.
    pub scope: String,
    pub expiration: u64,
    pub tag: Option<String>,
}

/// `POST /v1/orderly_key`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyAddKeyResult {
    pub id: Option<u64>,
    pub orderly_key: String,
}

// ── Websocket payloads (public) ──────────────────────────────────────────────

/// Topic push: `{"topic": "...", "ts": ..., "data": ...}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyWsPush<T> {
    pub topic: String,
    pub ts: u64,
    pub data: T,
}

/// Reply to `subscribe`/`unsubscribe`/`auth`/`request`. Errors carry `errorMsg` and may omit
/// `event`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyWsResponse {
    pub id: Option<String>,
    pub event: Option<String>,
    pub success: bool,
    pub ts: u64,
    #[serde(rename = "errorMsg")]
    pub error_msg: Option<String>,
}

/// `{"event": "ping"|"pong", "ts": ...}`. The server pings every 10 s.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyWsHeartbeat {
    pub event: String,
    pub ts: u64,
}

/// `{symbol}@orderbook` (100 levels, every 1 s). Levels are `[price, quantity]`, best first.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyWsBook {
    pub symbol: String,
    pub asks: Vec<[f64; 2]>,
    pub bids: Vec<[f64; 2]>,
    /// Predict symbols only.
    pub down_asks: Option<Vec<[f64; 2]>>,
    /// Predict symbols only.
    pub down_bids: Option<Vec<[f64; 2]>>,
}

/// `{symbol}@orderbookupdate` (every 200 ms). A level with quantity 0 is removed. `prev_ts` is
/// the `ts` of the previous update; a gap means resync.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyWsBookUpdate {
    pub symbol: String,
    #[serde(rename = "prevTs")]
    pub prev_ts: u64,
    pub asks: Vec<[f64; 2]>,
    pub bids: Vec<[f64; 2]>,
    /// Predict symbols only.
    pub down_asks: Option<Vec<[f64; 2]>>,
    /// Predict symbols only.
    pub down_bids: Option<Vec<[f64; 2]>>,
}

/// `data` of the reply to `{"event": "request", "params": {"type": "orderbook", ...}}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyWsBookSnapshot {
    pub symbol: String,
    pub ts: u64,
    pub asks: Vec<[f64; 2]>,
    pub bids: Vec<[f64; 2]>,
}

/// Reply to a websocket orderbook `request`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyWsBookRequestReply {
    pub id: String,
    pub event: String,
    pub success: bool,
    pub ts: u64,
    pub data: OrderlyWsBookSnapshot,
}

/// `{symbol}@trade`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyWsTrade {
    pub symbol: String,
    pub price: f64,
    pub size: f64,
    /// Taker side.
    pub side: String,
}

/// `{symbol}@ticker` and each row of `tickers` (rolling 24 h).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyWsTicker {
    pub symbol: String,
    pub open: f64,
    pub close: f64,
    pub high: f64,
    pub low: f64,
    pub volume: f64,
    pub amount: f64,
    pub count: u64,
}

/// `{symbol}@bbo` and each row of `bbos`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyWsBbo {
    pub symbol: String,
    pub ask: f64,
    #[serde(rename = "askSize")]
    pub ask_size: f64,
    pub bid: f64,
    #[serde(rename = "bidSize")]
    pub bid_size: f64,
    /// Predict symbols only.
    pub down_bid: Option<f64>,
    /// Predict symbols only.
    pub down_ask: Option<f64>,
}

/// `{symbol}@kline_{interval}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyWsKline {
    pub symbol: String,
    #[serde(rename = "type")]
    pub interval: String,
    pub open: f64,
    pub close: f64,
    pub high: f64,
    pub low: f64,
    pub volume: f64,
    pub amount: f64,
    #[serde(rename = "startTime")]
    pub start_time: u64,
    #[serde(rename = "endTime")]
    pub end_time: u64,
}

/// `{symbol}@markprice`, `{symbol}@indexprice` and each row of `markprices` / `indexprices`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyWsPrice {
    pub symbol: String,
    pub price: f64,
}

/// `{symbol}@openinterest`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyWsOpenInterest {
    pub symbol: String,
    #[serde(rename = "openInterest")]
    pub open_interest: f64,
}

/// `{symbol}@estfundingrate` (every 15 s).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyWsEstFundingRate {
    pub symbol: String,
    #[serde(rename = "fundingRate")]
    pub funding_rate: f64,
    /// Next funding time.
    #[serde(rename = "fundingTs")]
    pub funding_ts: u64,
}

/// Position of a `liquidation` push row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyWsLiquidationPerp {
    pub symbol: String,
    #[serde(rename = "positionQty")]
    pub position_qty: f64,
    #[serde(rename = "liquidatorFee")]
    pub liquidator_fee: f64,
}

/// Row of the `liquidation` topic (positions up for liquidation; pushed on change). Typed from
/// the documented example; no push arrived during live capture.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyWsLiquidation {
    #[serde(rename = "liquidationId")]
    pub liquidation_id: u64,
    pub timestamp: u64,
    #[serde(rename = "type")]
    pub liquidation_type: String,
    #[serde(rename = "positionsByPerp")]
    pub positions_by_perp: Vec<OrderlyWsLiquidationPerp>,
}

/// Any frame of the public websocket, dispatched on `topic` (pushes) or `event`/`success`
/// (replies, heartbeats). Builder-scoped tickers (`{broker}${symbol}@ticker`,
/// `{broker}$tickers`) parse as `Ticker` / `Tickers`; the push's `topic` tells them apart.
#[derive(Debug, Clone, PartialEq)]
pub enum OrderlyWsMessage {
    /// `{"event": "ping"|"pong", "ts"}`.
    Heartbeat(OrderlyWsHeartbeat),
    /// Subscribe/unsubscribe acks and errors (and failed `request`s).
    Response(OrderlyWsResponse),
    /// Reply to a successful orderbook `request`.
    BookRequest(OrderlyWsBookRequestReply),
    Orderbook(OrderlyWsPush<OrderlyWsBook>),
    OrderbookUpdate(OrderlyWsPush<OrderlyWsBookUpdate>),
    Trade(OrderlyWsPush<OrderlyWsTrade>),
    Ticker(OrderlyWsPush<OrderlyWsTicker>),
    Tickers(OrderlyWsPush<Vec<OrderlyWsTicker>>),
    Bbo(OrderlyWsPush<OrderlyWsBbo>),
    Bbos(OrderlyWsPush<Vec<OrderlyWsBbo>>),
    Kline(OrderlyWsPush<OrderlyWsKline>),
    EstFundingRate(OrderlyWsPush<OrderlyWsEstFundingRate>),
    MarkPrice(OrderlyWsPush<OrderlyWsPrice>),
    MarkPrices(OrderlyWsPush<Vec<OrderlyWsPrice>>),
    IndexPrice(OrderlyWsPush<OrderlyWsPrice>),
    IndexPrices(OrderlyWsPush<Vec<OrderlyWsPrice>>),
    OpenInterest(OrderlyWsPush<OrderlyWsOpenInterest>),
    PriceChanges(OrderlyWsPush<Vec<OrderlyPriceChanges>>),
    TradersOpenInterests(OrderlyWsPush<Vec<OrderlyTradersOpenInterest>>),
    HistoryCharts(OrderlyWsPush<Vec<OrderlyHistoryChart>>),
    Liquidation(OrderlyWsPush<Vec<OrderlyWsLiquidation>>),
    MaintenanceStatus(OrderlyWsPush<Vec<OrderlySystemInfo>>),
}

impl OrderlyWsMessage {
    /// Parses one text frame. Unknown topics and unrecognised frames are errors.
    pub fn parse(text: &str) -> Result<Self, serde_json::Error> {
        use serde::de::Error as _;
        use serde_json::{from_value as v2, Value};
        let v: Value = serde_json::from_str(text)?;
        let obj = v
            .as_object()
            .ok_or_else(|| serde_json::Error::custom("websocket frame is not a JSON object"))?;
        if let Some(topic) = obj.get("topic").and_then(Value::as_str) {
            let topic = topic.to_string();
            let unknown = || serde_json::Error::custom(format!("unknown topic {topic:?}"));
            return Ok(match topic.rsplit_once('@').map(|(_, s)| s) {
                Some("orderbook") => Self::Orderbook(v2(v)?),
                Some("orderbookupdate") => Self::OrderbookUpdate(v2(v)?),
                Some("trade") => Self::Trade(v2(v)?),
                Some("ticker") => Self::Ticker(v2(v)?),
                Some("bbo") => Self::Bbo(v2(v)?),
                Some("estfundingrate") => Self::EstFundingRate(v2(v)?),
                Some("markprice") => Self::MarkPrice(v2(v)?),
                Some("indexprice") => Self::IndexPrice(v2(v)?),
                Some("openinterest") => Self::OpenInterest(v2(v)?),
                Some(s) if s.starts_with("kline_") => Self::Kline(v2(v)?),
                Some(_) => return Err(unknown()),
                None => match topic.as_str() {
                    "tickers" => Self::Tickers(v2(v)?),
                    t if t.ends_with("$tickers") => Self::Tickers(v2(v)?),
                    "bbos" => Self::Bbos(v2(v)?),
                    "markprices" => Self::MarkPrices(v2(v)?),
                    "indexprices" => Self::IndexPrices(v2(v)?),
                    "price_changes" => Self::PriceChanges(v2(v)?),
                    "traders_open_interests" => Self::TradersOpenInterests(v2(v)?),
                    t if t.starts_with("history_charts_") => Self::HistoryCharts(v2(v)?),
                    "liquidation" => Self::Liquidation(v2(v)?),
                    "maintenance_status" => Self::MaintenanceStatus(v2(v)?),
                    _ => return Err(unknown()),
                },
            });
        }
        let event = obj.get("event").and_then(Value::as_str).map(str::to_owned);
        let has_data = obj.contains_key("data");
        if obj.contains_key("success") {
            if event.as_deref() == Some("request") && has_data {
                return Ok(Self::BookRequest(v2(v)?));
            }
            return Ok(Self::Response(v2(v)?));
        }
        match event.as_deref() {
            Some("ping") | Some("pong") => Ok(Self::Heartbeat(v2(v)?)),
            _ => Err(serde_json::Error::custom(format!(
                "unrecognised websocket frame: {text}"
            ))),
        }
    }
}

/// Per-symbol map used by `GET /v1/client/info` (`imr_factor`, `max_notional`).
pub type OrderlySymbolMap = HashMap<String, f64>;
