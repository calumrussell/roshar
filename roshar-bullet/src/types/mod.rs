//! Wire types for the Bullet trading API (Binance-FAPI-shaped REST, JSON websocket).
//!
//! Field names and optionality follow the OpenAPI spec at
//! `https://tradingapi.bullet.xyz/docs/rest/openapi.json`; every type is checked against live
//! captures in `tests/fixtures`. Decimal values stay strings, as the API sends them. Timestamps
//! are unix microseconds unless a field says otherwise.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub mod ws;

// ── Errors ────────────────────────────────────────────────────────────────────

/// `ApiErrorResponse`: `{"status": 400, "message": "...", "details"?, "error_id"?}`.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct BulletApiError {
    pub status: u16,
    pub message: String,
    /// Validation rule, or rollup error details; shape varies by error.
    #[serde(default)]
    pub details: Option<serde_json::Value>,
    #[serde(default)]
    pub error_id: Option<String>,
}

// ── Market data ───────────────────────────────────────────────────────────────

/// `GET /fapi/v1/time`.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BulletServerTime {
    pub server_time: i64,
}

/// `GET /fapi/v1/exchangeInfo`.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BulletExchangeInfo {
    pub assets: Vec<BulletAsset>,
    /// Signing domain of every transaction (hex, `0x`-prefixed). Nullable in the spec.
    pub chain_hash: Option<String>,
    pub chain_info: Option<BulletChainInfo>,
    pub global_config: BulletGlobalConfig,
    pub global_vault_config: BulletGlobalVaultConfig,
    /// Always empty: limits are not exposed through the Binance schema.
    pub rate_limits: Vec<BulletRateLimit>,
    pub symbols: Vec<BulletSymbol>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BulletAsset {
    pub asset: String,
    pub asset_id: u16,
    pub decimals: u8,
    pub margin_available: bool,
    /// Absent for assets without a bank token.
    #[serde(default)]
    pub token_id: Option<String>,
    /// Absent for assets without a deposit limit.
    #[serde(default)]
    pub user_deposit_limit: Option<String>,
    pub withdraw_fee: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BulletChainInfo {
    pub address_prefix: String,
    pub chain_id: u64,
    pub chain_name: String,
    pub gas_token_id: String,
    pub hyperlane_domain: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BulletGlobalConfig {
    pub max_orders_per_user: u16,
    pub max_trigger_orders_per_user: u16,
    pub max_trigger_orders_to_execute_per_msg: u16,
    pub min_notional_twap_value: String,
    pub min_notional_twap_value_per_order: String,
    pub twap_execution_interval_seconds: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BulletGlobalVaultConfig {
    pub creation_fee_usdc: String,
    pub leader_minimum_holding_percentage: u8,
    pub min_deposit_value: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BulletRateLimit {
    pub interval: String,
    pub interval_num: i32,
    pub limit: i32,
    pub rate_limit_type: String,
}

/// One market of `exchangeInfo.symbols`. `market_id` is the `u16` order calls take.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BulletSymbol {
    pub market_id: u16,
    pub symbol: String,
    pub pair: String,
    /// `CryptoPerp`, `RwaPerpUsEquity`, `RwaPerpCommodities`, ...
    pub contract_type: String,
    pub delivery_date: i64,
    pub onboard_date: i64,
    /// `TRADING`, `POST_ONLY`, `CANCEL_ONLY`, `HALT`, `CLOSE`.
    pub status: String,
    pub base_asset: String,
    pub base_asset_id: u16,
    pub quote_asset: String,
    pub margin_asset: String,
    pub price_precision: u8,
    pub quantity_precision: u8,
    pub base_asset_precision: u8,
    pub quote_precision: u8,
    pub underlying_type: String,
    pub underlying_sub_type: Vec<String>,
    pub settle_plan: i32,
    pub trigger_protect: String,
    pub filters: Vec<BulletFilter>,
    pub order_types: Vec<String>,
    pub time_in_force: Vec<String>,
    pub liquidation_fee: String,
    pub market_take_bound: String,
    /// Per fee tier (0-9).
    pub maker_fee_bps: Vec<String>,
    pub taker_fee_bps: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(tag = "filterType")]
pub enum BulletFilter {
    #[serde(rename = "PRICE_FILTER", rename_all = "camelCase")]
    PriceFilter {
        tick_size: String,
        min_price: String,
        max_price: String,
    },
    #[serde(rename = "LOT_SIZE", rename_all = "camelCase")]
    LotSize {
        step_size: String,
        min_qty: String,
        max_qty: String,
    },
    #[serde(rename = "MAX_NUM_ORDERS")]
    MaxNumOrders { limit: u16 },
    #[serde(rename = "MAX_NUM_ALGO_ORDERS")]
    MaxNumAlgoOrders { limit: u16 },
    #[serde(rename = "PERCENT_PRICE", rename_all = "camelCase")]
    PercentPrice {
        multiplier_up: String,
        multiplier_down: String,
        multiplier_decimal: String,
    },
}

/// `[price, qty]`.
pub type BulletLevel = (String, String);

/// `GET /fapi/v1/depth`.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct BulletOrderBook {
    #[serde(rename = "lastUpdateId")]
    pub last_update_id: u64,
    /// Event time (µs).
    #[serde(rename = "E")]
    pub event_time: i64,
    /// Chain time of the newest book state reflected (µs).
    #[serde(rename = "T")]
    pub transaction_time: i64,
    pub bids: Vec<BulletLevel>,
    pub asks: Vec<BulletLevel>,
}

/// `GET /fapi/v1/trades`.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BulletTrade {
    pub id: i64,
    pub price: String,
    pub qty: String,
    pub quote_qty: String,
    pub time: i64,
    pub is_buyer_maker: bool,
}

/// `GET /fapi/v1/ticker/24hr` (always every market).
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BulletTicker24hr {
    pub symbol: String,
    pub price_change: String,
    pub price_change_percent: String,
    pub weighted_avg_price: String,
    pub last_price: String,
    pub open_price: String,
    pub high_price: String,
    pub low_price: String,
    pub volume: String,
    pub quote_volume: String,
    pub open_time: i64,
    pub close_time: i64,
    pub count: i64,
}

/// `GET /fapi/v1/ticker/price`.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct BulletPriceTicker {
    pub symbol: String,
    pub price: String,
    pub time: i64,
}

/// `GET /fapi/v1/premiumIndex`: mark, index and current funding. Rates are on the 8h basis.
/// Live responses carry `nextFundingTime` and `time` in milliseconds, unlike the rest of the API.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BulletPremiumIndex {
    pub symbol: String,
    pub mark_price: String,
    pub index_price: String,
    pub last_funding_rate: String,
    pub interest_rate: String,
    pub estimated_funding_rate: String,
    pub next_funding_time: i64,
    pub time: i64,
}

/// `GET /fapi/v1/fundingRate`: one row per hourly settlement, oldest first.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BulletFundingRate {
    pub symbol: String,
    pub funding_rate: String,
    /// Hour start (µs).
    pub funding_time: i64,
}

/// `GET /fapi/v1/fundingInfo`.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BulletFundingInfo {
    pub symbol: String,
    pub adjusted_funding_rate_cap: String,
    pub adjusted_funding_rate_floor: String,
    pub funding_interval_hours: i32,
    pub funding_multiplier: String,
    pub funding_spread_rate: String,
}

/// `GET /fapi/v1/openInterest`.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BulletOpenInterest {
    pub symbol: String,
    pub open_interest: String,
    pub time: i64,
}

/// `GET /fapi/v1/symbolConfig`.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BulletSymbolConfig {
    pub symbol: String,
    pub margin_type: String,
    pub is_auto_add_margin: bool,
    pub leverage: i32,
    pub max_notional_value: String,
}

/// `GET /fapi/v1/leverageBracket`.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BulletLeverageBracket {
    pub symbol: String,
    pub brackets: Vec<BulletBracket>,
    #[serde(default)]
    pub notional_coef: Option<f64>,
}

/// Numbers, not strings, in the spec and on the wire.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BulletBracket {
    pub bracket: u32,
    pub initial_leverage: u32,
    pub notional_cap: f64,
    pub notional_floor: f64,
    pub maint_margin_ratio: f64,
    pub cum: f64,
}

/// `GET /api/v1/oraclePrices`: one entry per registered asset, including collateral assets
/// without a market.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BulletOraclePrice {
    pub asset_id: u16,
    pub asset: String,
    /// Stored oracle price; `"0"` for uninitialised entries.
    pub oracle_price: String,
    /// Not required and nullable in the spec: no update recorded.
    #[serde(default)]
    pub last_update_timestamp: Option<i64>,
}

/// `GET /fapi/v1/insuranceBalance`.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct BulletInsuranceBalance {
    /// Markets the fund covers.
    pub symbols: Vec<String>,
    pub assets: Vec<BulletInsuranceAsset>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BulletInsuranceAsset {
    pub asset: String,
    pub margin_balance: String,
    pub update_time: i64,
}

/// `GET /fapi/v1/borrowLendPools`: spot borrow/lend pool state per asset.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BulletBorrowLendPool {
    pub asset: String,
    pub asset_id: u16,
    pub is_active: bool,
    pub last_update_timestamp: i64,
    pub rate_params: BulletRateParams,
    pub deposit_limit: String,
    pub borrow_limit: String,
    pub max_utilization_rate: String,
    pub available_amount: String,
    pub borrowed_amount: String,
    pub cumulative_deposit_rate: String,
    pub cumulative_borrow_rate: String,
    pub accumulated_protocol_fees: String,
    pub unclaimed_protocol_fees: String,
    pub interest_fee_tenth_bps: u64,
}

/// Interest-rate curve of a borrow/lend pool (snake_case on the wire).
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct BulletRateParams {
    pub optimal_utilization_rate: String,
    pub min_borrow_rate: String,
    pub optimal_borrow_rate: String,
    pub max_borrow_rate: String,
}

/// `GET /api/v1/riskEngineConfig`: the rollup's versioned risk-engine state, verbatim
/// (snake_case, externally tagged by version). The spec types it only as `object`; this follows
/// the live shape. An unknown version is a parse error. Map keys are market ids (perp tables and
/// pricing), asset ids (borrow/lend, oracle) or notional floors (leverage tiers).
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub enum BulletRiskEngineConfig {
    V1(BulletRiskEngineConfigV1),
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct BulletRiskEngineConfigV1 {
    pub margin_config: BulletMarginConfig,
    pub pricing_engine: BulletPricingEngine,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub enum BulletMarginConfig {
    V0(BulletMarginConfigV0),
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct BulletMarginConfigV0 {
    pub perp_liquidation_config: BulletPerpLiquidationConfig,
    /// By market id.
    pub perp_leverage_tables: BTreeMap<u16, BulletPerpLeverageTable>,
    /// By asset id.
    pub borrow_lend_risk_configs: BTreeMap<u16, BulletBorrowLendRiskConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct BulletPerpLiquidationConfig {
    pub liquidation_ioc_buffer: String,
    pub backstop_liquidation_threshold: String,
    pub liquidation_fee: String,
    pub liquidation_protocol_reward_ratio: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct BulletPerpLeverageTable {
    /// Keyed by the tier's notional floor as the API writes it (e.g. `"500000"`).
    pub tiers: BTreeMap<String, BulletLeverageTier>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct BulletLeverageTier {
    pub max_leverage: u32,
    pub mmr: String,
    pub maintenance_amount: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct BulletBorrowLendRiskConfig {
    pub liquidation_total_reward_ratio: String,
    pub protocol_reward_ratio: String,
    pub liability_liquidation_limit_ratio: String,
    pub asset_weight: String,
    pub initial_liability_weight: String,
    pub maintenance_liability_weight: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub enum BulletPricingEngine {
    V1(BulletPricingEngineV1),
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct BulletPricingEngineV1 {
    pub pricing_frequency_seconds: u64,
    pub relaxed_pricing_frequency_seconds: u64,
    /// By asset id.
    pub oracle_pricing: BTreeMap<u16, BulletOraclePricing>,
    /// By market id.
    pub perp_pricing: BTreeMap<u16, BulletPerpPricing>,
    /// By market id (RWA markets).
    pub rwa_pricing_config: BTreeMap<u16, BulletRwaPricingConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct BulletOraclePricing {
    pub price: String,
    pub last_update_timestamp: i64,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct BulletPerpPricing {
    pub market_id: u16,
    pub mark_price: BulletMarkPriceState,
    pub accum_premium_index: BulletAccumPremiumIndex,
    pub funding_meta: BulletFundingMeta,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct BulletMarkPriceState {
    pub price: String,
    pub last_oracle_diff_ema: String,
    pub last_median_cex_price: String,
    pub last_update_timestamp: i64,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct BulletAccumPremiumIndex {
    pub weighted_premium_sum: String,
    pub duration_sum: u64,
    pub last_update_timestamp: i64,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct BulletFundingMeta {
    pub rate: String,
    pub accum_per_lot: String,
    pub last_update_timestamp: i64,
    pub config: BulletFundingRateConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct BulletFundingRateConfig {
    pub interest_rate: String,
    pub min_interest_rate_clamp: String,
    pub max_interest_rate_clamp: String,
    pub min_funding_rate_clamp: String,
    pub max_funding_rate_clamp: String,
}

/// `Option` field whose key must be present (value may be `null`).
fn nullable<'de, D, T>(d: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(d)
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct BulletRwaPricingConfig {
    /// `null` on every live market (key always sent); assumed a decimal string when set.
    #[serde(deserialize_with = "nullable")]
    pub reference_price: Option<String>,
    pub oracle_threshold_ratio: String,
    pub up_bound_n: u32,
    pub down_bound_n: u32,
    pub max_bound_n: u32,
    pub funding_dampening_multiplier: String,
    pub internal_pricing_time_constant: u64,
}

/// `GET /api/v1/cmc/contracts`: CoinMarketCap listing feed (`/contracts` with `/contract_specs`
/// merged). Ids are `BASE-PERP`; USDT is taken at par.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct BulletCmcContract {
    pub ticker_id: String,
    pub base_currency: String,
    pub quote_currency: String,
    pub last_price: String,
    pub base_volume: String,
    pub quote_volume: String,
    #[serde(rename = "USD_volume")]
    pub usd_volume: String,
    pub bid: String,
    pub ask: String,
    pub high: String,
    pub low: String,
    pub product_type: String,
    pub open_interest: String,
    pub open_interest_usd: String,
    pub open_interest_methodology: String,
    pub index_price: String,
    pub funding_rate: String,
    pub next_funding_rate: String,
    /// Unix milliseconds.
    pub next_funding_rate_timestamp: i64,
    pub contract_type: String,
    pub contract_price: String,
    pub contract_price_currency: String,
}

/// `GET /api/v1/coingecko/contracts`: CoinGecko listing feed.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct BulletCoinGeckoContract {
    pub ticker_id: String,
    pub base_currency: String,
    pub target_currency: String,
    pub last_price: String,
    pub base_volume: String,
    pub target_volume: String,
    pub bid: String,
    pub ask: String,
    pub high: String,
    pub low: String,
    pub product_type: String,
    pub open_interest: String,
    pub open_interest_usd: String,
    pub long_open_interest: String,
    pub long_open_interest_usd: String,
    pub short_open_interest: String,
    pub short_open_interest_usd: String,
    pub index_price: String,
    pub index_currency: String,
    pub funding_rate: String,
    pub funding_interval_minutes: i32,
    pub next_funding_rate: String,
    /// Unix seconds (live), unlike the CMC feed's milliseconds.
    pub next_funding_rate_timestamp: i64,
    pub contract_type: String,
    pub contract_price_currency: String,
}

/// `GET /api/v1/cmc/orderbook/{ticker_id}` and `/api/v1/coingecko/orderbook`.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct BulletListingOrderBook {
    pub ticker_id: String,
    /// Sequencer time, Unix milliseconds.
    pub timestamp: i64,
    /// `[price, quantity]`, best first.
    pub bids: Vec<(String, String)>,
    pub asks: Vec<(String, String)>,
}

// ── Account (read by address, no authentication) ──────────────────────────────

/// `GET /fapi/v3/account`. Totals are USD values at oracle prices.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BulletAccount {
    pub total_initial_margin: String,
    pub total_maint_margin: String,
    pub total_wallet_balance: String,
    pub total_unrealized_profit: String,
    pub total_margin_balance: String,
    pub total_position_initial_margin: String,
    pub total_open_order_initial_margin: String,
    pub total_cross_wallet_balance: String,
    pub total_cross_un_pnl: String,
    pub available_balance: String,
    pub max_withdraw_amount: String,
    /// `null` without cross positions or when no move liquidates the account.
    #[serde(default)]
    pub max_adverse_move: Option<String>,
    /// `NONE`, `LP`, `SUPER_LP`, `ASSET_WEIGHT_LP`, `ASSET_WEIGHT_SUPER_LP`.
    pub margin_discount: String,
    pub fee_tier: i32,
    pub fee_discount_bps: i32,
    pub assets: Vec<BulletAccountAsset>,
    pub positions: Vec<BulletPosition>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BulletAccountAsset {
    pub asset: String,
    pub asset_id: u16,
    /// In asset units.
    pub wallet_balance: String,
    pub update_time: i64,
}

/// A position row of `GET /fapi/v3/account` and `GET /fapi/v2/positionRisk`.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BulletPosition {
    pub symbol: String,
    pub market_id: u16,
    pub unrealized_profit: String,
    pub entry_price: String,
    pub initial_margin: String,
    pub maint_margin: String,
    pub margin_type: String,
    pub isolated_margin: String,
    pub isolated_wallet: String,
    pub notional: String,
    pub leverage: i32,
    pub max_notional_value: String,
    pub mark_price: String,
    pub liquidation_price: String,
    pub position_side: String,
    /// Signed; negative is short.
    pub position_amt: String,
    pub update_time: i64,
}

/// `GET /fapi/v3/balance`.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BulletBalance {
    pub account_alias: String,
    pub asset: String,
    pub balance: String,
    pub cross_wallet_balance: String,
    pub cross_un_pnl: String,
    pub available_balance: String,
    pub max_withdraw_amount: String,
    pub margin_available: bool,
    pub update_time: i64,
}

/// `GET /fapi/v1/openOrders` / `GET /fapi/v1/openOrder`. Regular and TP/SL rows draw from
/// separate id counters: key by (`order_id`, `is_trigger`).
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BulletOpenOrder {
    pub order_id: u64,
    pub symbol: String,
    pub market_id: u16,
    pub status: String,
    /// Decimal `u64` as a string; `null`/absent without a client order id.
    #[serde(default)]
    pub client_order_id: Option<String>,
    pub price: String,
    pub avg_price: String,
    pub orig_qty: String,
    pub executed_qty: String,
    pub cum_qty: String,
    pub cum_quote: String,
    pub time_in_force: String,
    pub order_type: String,
    pub reduce_only: bool,
    pub close_position: bool,
    /// `BID` / `ASK`.
    pub side: String,
    pub position_side: String,
    #[serde(default)]
    pub stop_price: Option<String>,
    pub working_type: String,
    pub price_protect: bool,
    pub orig_type: String,
    pub price_match: String,
    pub self_trade_prevention_mode: String,
    pub good_till_date: u64,
    pub update_time: i64,
    pub is_trigger: bool,
    /// Omitted when the order has no reserved TP/SL.
    #[serde(default)]
    pub pending_tpsl: Option<BulletPendingTpsl>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BulletPendingTpsl {
    pub dynamic_size: bool,
    #[serde(default)]
    pub tp: Option<BulletTpslLeg>,
    #[serde(default)]
    pub sl: Option<BulletTpslLeg>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BulletTpslLeg {
    pub trigger_order_id: u64,
    pub order_price: String,
    pub trigger_price: String,
    pub price_condition: String,
    pub order_type: String,
    pub time_in_force: String,
}

/// A page of a cursor-paginated history endpoint. `next_cursor` is absent on the last page.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BulletPage<T> {
    pub data: Vec<T>,
    #[serde(default)]
    pub next_cursor: Option<String>,
}

/// `GET /fapi/v1/allOrders` row.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BulletHistoricalOrder {
    pub order_id: u64,
    pub symbol: String,
    /// `NEW`, `PARTIALLY_FILLED`, `FILLED`, `CANCELED`, `EXPIRED`.
    pub status: String,
    pub price: String,
    pub orig_qty: String,
    pub executed_qty: String,
    pub avg_price: String,
    /// `BUY` / `SELL`.
    pub side: String,
    #[serde(rename = "type")]
    pub order_type: String,
    pub time: i64,
    pub update_time: i64,
    pub reduce_only: bool,
    pub close_position: bool,
    pub position_side: String,
    pub tx_hashes: Vec<String>,
}

/// `GET /fapi/v1/userTrades` row.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BulletUserTrade {
    pub symbol: String,
    pub id: u64,
    pub order_id: u64,
    #[serde(default)]
    pub client_order_id: Option<u64>,
    pub side: String,
    pub price: String,
    pub qty: String,
    pub quote_qty: String,
    pub realized_pnl: String,
    pub commission: String,
    pub commission_asset: String,
    pub time: i64,
    pub position_side: String,
    pub buyer: bool,
    pub maker: bool,
    pub is_degen: bool,
    pub address: String,
    pub tx_hash: String,
}

/// `GET /fapi/v1/userFundingPayments` row. The balance moves by `funding_applied - fee`.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BulletFundingPayment {
    pub symbol: String,
    pub market_id: u16,
    /// Gross payment, positive when received.
    pub funding_applied: String,
    pub fee: String,
    pub tx_hash: String,
    pub event_number: u64,
    pub time: i64,
}

/// `GET /fapi/v1/commissionRate`: fractions of notional.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BulletCommissionRate {
    pub symbol: String,
    pub maker_commission_rate: String,
    pub taker_commission_rate: String,
}

/// `GET /api/v1/delegateOf`: the account a delegate key trades for.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BulletDelegateOf {
    pub parent: String,
    pub name: String,
    pub flags: i32,
    #[serde(default)]
    pub expires_at: Option<i64>,
}

/// Selects one order for `GET /fapi/v1/openOrder`; the three ids are mutually exclusive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BulletOrderRef {
    OrderId(u64),
    ClientOrderId(u64),
    /// A TP/SL trigger order.
    TriggerOrderId(u64),
}

/// Common filters of the cursor-paginated history endpoints. The first request needs
/// `address`; follow-ups pass `cursor` (which encodes the address and filters).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BulletHistoryQuery<'a> {
    pub address: Option<&'a str>,
    pub symbol: Option<&'a str>,
    /// Unix µs, inclusive.
    pub start_time: Option<i64>,
    pub end_time: Option<i64>,
    /// Default 500, max 1000.
    pub limit: Option<u32>,
    pub cursor: Option<&'a str>,
}

// ── Transaction submission ────────────────────────────────────────────────────

/// `POST /tx/submit` request.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct BulletSubmitTxRequest {
    /// Base64 borsh `Transaction`.
    pub body: String,
}

/// `POST /tx/submit` response.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct BulletSubmitTxResponse {
    /// Transaction hash.
    pub id: String,
    pub status: BulletTxStatus,
    #[serde(default)]
    pub events: Option<Vec<BulletLedgerEvent>>,
    #[serde(default)]
    pub receipt: Option<BulletTxReceipt>,
    #[serde(default)]
    pub tx_number: Option<u64>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum BulletTxStatus {
    Unknown,
    Dropped,
    Submitted,
    Published,
    Processed,
    Finalized,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct BulletLedgerEvent {
    pub key: String,
    pub module: BulletModuleRef,
    pub number: u64,
    #[serde(default)]
    pub tx_hash: Option<String>,
    #[serde(rename = "type")]
    pub event_type: String,
    pub value: serde_json::Value,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct BulletModuleRef {
    pub name: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct BulletTxReceipt {
    pub result: BulletTxResult,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum BulletTxResult {
    Successful,
    Reverted,
    Skipped,
}
