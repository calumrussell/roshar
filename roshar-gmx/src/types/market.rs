//! GMX API (`https://{chain}.gmxapi.io/v1`) read models, built from the published OpenAPI spec
//! (`/swagger.json`) and checked against live responses. Fields the spec lists as optional are
//! `Option`; everything else is required. Unknown fields are ignored: the API documents its
//! responses as extensible.
//!
//! Big numbers stay strings exactly as sent: USD values, prices and factors are 30-decimal fixed
//! point, token amounts are in token decimals.

use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;

/// For fields that are nullable but always sent: unlike a bare `Option<T>`, a missing key is an
/// error (use with `deserialize_with` and no `default`).
fn nullable<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}

/// Candle period accepted by `/v1/prices/ohlcv`, the oracle `/prices/candles` and the
/// `candles:{symbol}:{period}` stream channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum GmxCandlePeriod {
    #[serde(rename = "1m")]
    M1,
    #[serde(rename = "5m")]
    M5,
    #[serde(rename = "15m")]
    M15,
    #[serde(rename = "1h")]
    H1,
    #[serde(rename = "4h")]
    H4,
    #[serde(rename = "1d")]
    D1,
}

impl GmxCandlePeriod {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::M1 => "1m",
            Self::M5 => "5m",
            Self::M15 => "15m",
            Self::H1 => "1h",
            Self::H4 => "4h",
            Self::D1 => "1d",
        }
    }
}

/// `ApiParameterPeriod`: the `period` / `averageBy` values of `/v1/rates`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum GmxPeriod {
    #[serde(rename = "1d")]
    D1,
    #[serde(rename = "7d")]
    D7,
    #[serde(rename = "30d")]
    D30,
    #[serde(rename = "90d")]
    D90,
    #[serde(rename = "180d")]
    D180,
    #[serde(rename = "1y")]
    Y1,
    #[serde(rename = "total")]
    Total,
}

impl GmxPeriod {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::D1 => "1d",
            Self::D7 => "7d",
            Self::D30 => "30d",
            Self::D90 => "90d",
            Self::D180 => "180d",
            Self::Y1 => "1y",
            Self::Total => "total",
        }
    }
}

/// Position side, as the API spells it (`direction`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum GmxDirection {
    Long,
    Short,
}

impl GmxDirection {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Long => "long",
            Self::Short => "short",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum GmxMarketDataStatus {
    Fresh,
    Stale,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum GmxJitDataStatus {
    Available,
    Stale,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum GmxLimitingFactor {
    Reserve,
    OpenInterest,
    Both,
    NotApplicable,
}

/// `{minPrice, maxPrice}`: 30-decimal USD per unit of the token's smallest denomination.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GmxMinMaxPrice {
    pub min_price: String,
    pub max_price: String,
}

/// `GET /v1/prices/tickers`; also the data of the `prices` stream channel. Keyed by token address.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GmxTokenPrices {
    pub prices: HashMap<String, GmxMinMaxPrice>,
    /// Observation time of the oldest price in the snapshot, unix ms.
    pub origin_ts: u64,
}

/// `GET /v1/trades` and `POST /v1/trades/search`. Pass `next_cursor` back as `cursor` while
/// `has_more`.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GmxTradesList {
    pub has_more: bool,
    /// Nullable but always sent (spec: required).
    #[serde(deserialize_with = "nullable")]
    pub next_cursor: Option<String>,
    pub trades: Vec<GmxTradeAction>,
}

/// Query of `GET /v1/trades`. `since` / `until` are whole unix seconds (the API rejects ms).
#[derive(Debug, Clone, Default)]
pub struct GmxTradesQuery {
    pub address: Option<String>,
    pub symbol: Option<String>,
    pub market_address: Option<String>,
    pub since: Option<u64>,
    pub until: Option<u64>,
    /// Trade event names, e.g. `OrderExecuted`, `OrderCancelled`.
    pub actions: Vec<String>,
    pub limit: Option<u32>,
    pub cursor: Option<String>,
}

/// `TradeDirection` of a `/v1/trades/search` market filter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum GmxTradeDirection {
    Long,
    Short,
    Swap,
    Any,
}

/// `MarketDirectionFilter`: one market + side (+ collateral) tuple of `/v1/trades/search`.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GmxMarketDirectionFilter {
    pub market_address: String,
    pub direction: GmxTradeDirection,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collateral_address: Option<String>,
}

/// `OrderEventCombination`: an order type / event filter of `/v1/trades/search`.
/// `order_type` is the contract enum (see [`GmxOrder`]); `event_name` e.g. `OrderExecuted`.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GmxOrderEventCombination {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order_type: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_deposit_or_withdraw: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_twap: Option<bool>,
}

/// Body of `POST /v1/trades/search` (`TradesSearchRequest`). The API requires `address` unless
/// `for_all_accounts` is `Some(true)` (400 "address is required when forAllAccounts is not
/// true"); with `for_all_accounts` it is the protocol-wide trade tape. Timestamps accept unix
/// seconds or ms. The live API rejects `limit` outside 1..=300 (the spec says 1..=1000).
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GmxTradesSearchRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub for_all_accounts: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from_timestamp: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to_timestamp: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub markets_directions: Option<Vec<GmxMarketDirectionFilter>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order_event_combinations: Option<Vec<GmxOrderEventCombination>>,
    /// `false` (exchange default) hides market-order `OrderCreated` events.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub show_debug_values: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
}

// ── Oracle API (`https://{chain}-api.gmxinfra.io`) ──────────────────────────────────────────

/// Row of oracle `GET /prices/tickers`. Prices are in oracle units: divide by
/// `10^(30 - tokenDecimals)` for USD. `updated_at` is unix ms, `timestamp` unix seconds.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GmxOracleTicker {
    pub token_address: String,
    pub token_symbol: String,
    pub min_price: String,
    pub max_price: String,
    pub updated_at: u64,
    pub timestamp: u64,
}

/// Oracle `GET /prices/candles`. Candles are newest first.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GmxOracleCandles {
    pub period: GmxCandlePeriod,
    pub candles: Vec<GmxOracleCandle>,
}

/// `[timestamp (unix s), open, high, low, close]`.
#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
pub struct GmxOracleCandle(pub u64, pub f64, pub f64, pub f64, pub f64);

impl GmxOracleCandle {
    pub fn timestamp(&self) -> u64 {
        self.0
    }
    pub fn open(&self) -> f64 {
        self.1
    }
    pub fn high(&self) -> f64 {
        self.2
    }
    pub fn low(&self) -> f64 {
        self.3
    }
    pub fn close(&self) -> f64 {
        self.4
    }
}

/// Market row of oracle `GET /markets`. `listing_date` is ISO 8601.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GmxOracleMarket {
    pub name: String,
    pub market_token: String,
    pub index_token: String,
    pub long_token: String,
    pub short_token: String,
    pub is_listed: bool,
    pub listing_date: String,
}

/// Oracle `GET /markets`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GmxOracleMarkets {
    pub markets: Vec<GmxOracleMarket>,
}

/// Market row of oracle `GET /markets/info`: open interest and liquidity (30-decimal USD), pool
/// amounts (token decimals), and rates. Swap-only markets carry no rate fields.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GmxOracleMarketInfo {
    pub name: String,
    pub market_token: String,
    pub index_token: String,
    pub long_token: String,
    pub short_token: String,
    pub is_listed: bool,
    pub listing_date: String,
    pub open_interest_long: String,
    pub open_interest_short: String,
    pub available_liquidity_long: String,
    pub available_liquidity_short: String,
    pub pool_amount_long: String,
    pub pool_amount_short: String,
    #[serde(default)]
    pub funding_rate_long: Option<String>,
    #[serde(default)]
    pub funding_rate_short: Option<String>,
    #[serde(default)]
    pub borrowing_rate_long: Option<String>,
    #[serde(default)]
    pub borrowing_rate_short: Option<String>,
    #[serde(default)]
    pub net_rate_long: Option<String>,
    #[serde(default)]
    pub net_rate_short: Option<String>,
}

/// Oracle `GET /markets/info`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GmxOracleMarketsInfo {
    pub markets: Vec<GmxOracleMarketInfo>,
}

/// Market status of oracle `GET /markets/state` (docs: open, closed or unknown).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum GmxOracleMarketStatus {
    Open,
    Closed,
    Unknown,
}

/// Market row of oracle `GET /markets/state`. `closing_price` and the transition timestamp are
/// null for every market in the captures, so their non-null types are inferred.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GmxOracleMarketState {
    pub market_token: String,
    pub is_market_closed: bool,
    pub status: GmxOracleMarketStatus,
    #[serde(deserialize_with = "nullable")]
    pub closing_price: Option<String>,
    #[serde(deserialize_with = "nullable")]
    pub latest_market_status_transition_timestamp: Option<u64>,
}

/// Oracle `GET /markets/state`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GmxOracleMarketsState {
    pub markets: Vec<GmxOracleMarketState>,
}

/// Token row of oracle `GET /tokens`. `synthetic` is absent for non-synthetic tokens.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GmxOracleToken {
    pub symbol: String,
    pub address: String,
    pub decimals: u8,
    #[serde(default)]
    pub synthetic: Option<bool>,
}

/// Oracle `GET /tokens`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GmxOracleTokens {
    pub tokens: Vec<GmxOracleToken>,
}

/// Row of oracle `GET /signed_prices/latest`. Fields that are `null` in live data are `Option`
/// but must be present. Live rows (Data Streams `realtimeFeed`) carry the report in `blob` and
/// prices in `min_price_full` / `max_price_full`, with the legacy signer fields null.
/// `created_at` is ISO 8601 (the docs show a number); block timestamps are unix seconds.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GmxOracleSignedPrice {
    pub id: String,
    pub token_symbol: String,
    pub token_address: String,
    #[serde(deserialize_with = "nullable")]
    pub min_price: Option<String>,
    #[serde(deserialize_with = "nullable")]
    pub max_price: Option<String>,
    pub min_price_full: String,
    pub max_price_full: String,
    #[serde(deserialize_with = "nullable")]
    pub oracle_decimals: Option<u32>,
    #[serde(deserialize_with = "nullable")]
    pub signer: Option<String>,
    #[serde(deserialize_with = "nullable")]
    pub signature: Option<String>,
    #[serde(deserialize_with = "nullable")]
    pub signature_without_block_hash: Option<String>,
    #[serde(deserialize_with = "nullable")]
    pub min_block_number: Option<u64>,
    pub min_block_timestamp: u64,
    #[serde(deserialize_with = "nullable")]
    pub min_block_hash: Option<String>,
    #[serde(deserialize_with = "nullable")]
    pub max_block_number: Option<u64>,
    pub max_block_timestamp: u64,
    #[serde(deserialize_with = "nullable")]
    pub max_block_hash: Option<String>,
    pub created_at: String,
    pub oracle_keeper_key: String,
    #[serde(deserialize_with = "nullable")]
    pub oracle_keeper_record_id: Option<String>,
    pub oracle_keeper_fetch_type: String,
    pub oracle_type: String,
    pub blob: String,
    pub is_valid: bool,
    #[serde(deserialize_with = "nullable")]
    pub invalid_reason: Option<String>,
    pub market_status: u8,
    pub feed_version: String,
    #[serde(deserialize_with = "nullable")]
    pub staleness_seconds: Option<f64>,
}

/// Oracle `GET /signed_prices/latest`.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GmxOracleSignedPrices {
    pub signed_prices: Vec<GmxOracleSignedPrice>,
}

/// Oracle `GET /ping`. Live answers `{"message":"ok"}` (the docs show `{"ok":true}`).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GmxOraclePing {
    pub message: String,
}

// ── Generated from the OpenAPI schemas ───────────────────────────────────────────────────────

/// One leverage tier of a market (`/markets` `leverageTiers`).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GmxLeverageTier {
    pub max_leverage: String,
    pub min_collateral_factor: String,
    #[serde(default)]
    pub max_position_size: Option<String>,
}

/// Market catalog row from `GET /v1/markets`. USD values are 30-decimal fixed point.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GmxMarket {
    pub symbol: String,
    pub market_token_address: String,
    pub index_token_address: String,
    pub long_token_address: String,
    pub short_token_address: String,
    pub is_listed: bool,
    pub is_spot_only: bool,
    pub leverage_tiers: Vec<GmxLeverageTier>,
    pub min_position_size_usd: String,
    pub min_collateral_usd: String,
    #[serde(default)]
    pub listing_date: Option<u64>,
}

/// Market configuration and near-live state from `GET /v1/markets/info`.
///
/// Factors and USD values are 30-decimal fixed point; token amounts are in token decimals.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GmxMarketInfo {
    pub data: String,
    pub name: String,
    pub market_token_address: String,
    pub index_token_address: String,
    pub long_token_address: String,
    pub short_token_address: String,
    pub is_disabled: bool,
    pub is_same_collaterals: bool,
    pub is_spot_only: bool,
    pub reserve_factor_long: String,
    pub reserve_factor_short: String,
    pub open_interest_reserve_factor_long: String,
    pub open_interest_reserve_factor_short: String,
    pub max_open_interest_long: String,
    pub max_open_interest_short: String,
    pub max_collateral_sum_long_token_long: String,
    pub max_collateral_sum_long_token_short: String,
    pub max_collateral_sum_short_token_long: String,
    pub max_collateral_sum_short_token_short: String,
    pub min_position_impact_pool_amount: String,
    pub position_impact_pool_distribution_rate: String,
    pub min_collateral_factor: String,
    pub min_collateral_factor_for_liquidation: String,
    pub min_collateral_factor_for_open_interest_long: String,
    pub min_collateral_factor_for_open_interest_short: String,
    pub max_pnl_factor_for_traders_long: String,
    pub max_pnl_factor_for_traders_short: String,
    #[serde(default)]
    pub max_pnl_factor_for_deposits_long: Option<String>,
    #[serde(default)]
    pub max_pnl_factor_for_deposits_short: Option<String>,
    #[serde(default)]
    pub max_pnl_factor_for_withdrawals_long: Option<String>,
    #[serde(default)]
    pub max_pnl_factor_for_withdrawals_short: Option<String>,
    pub position_fee_factor_for_balance_was_improved: String,
    pub position_fee_factor_for_balance_was_not_improved: String,
    pub position_impact_factor_positive: String,
    pub position_impact_factor_negative: String,
    pub max_position_impact_factor_positive: String,
    pub max_position_impact_factor_negative: String,
    pub max_position_impact_factor_for_liquidations: String,
    pub borrowing_factor_long: String,
    pub borrowing_factor_short: String,
    pub borrowing_exponent_factor_long: String,
    pub borrowing_exponent_factor_short: String,
    pub funding_factor: String,
    pub funding_exponent_factor: String,
    pub funding_increase_factor_per_second: String,
    pub min_funding_increase_rate_per_second: String,
    pub funding_decrease_factor_per_second: String,
    pub threshold_for_stable_funding: String,
    pub threshold_for_decrease_funding: String,
    pub min_funding_factor_per_second_long: String,
    pub min_funding_factor_per_second_short: String,
    pub max_funding_factor_per_second_long: String,
    pub max_funding_factor_per_second_short: String,
    pub max_lendable_impact_factor: String,
    pub max_lendable_impact_factor_for_withdrawals: String,
    pub max_lendable_impact_usd: String,
    pub lent_position_impact_pool_amount: String,
    pub position_impact_exponent_factor_positive: String,
    pub position_impact_exponent_factor_negative: String,
    pub swap_fee_factor_for_balance_was_improved: String,
    pub swap_fee_factor_for_balance_was_not_improved: String,
    pub atomic_swap_fee_factor: String,
    #[serde(default)]
    pub withdrawal_fee_factor_balance_was_improved: Option<String>,
    #[serde(default)]
    pub withdrawal_fee_factor_balance_was_not_improved: Option<String>,
    pub swap_impact_factor_positive: String,
    pub swap_impact_factor_negative: String,
    pub swap_impact_exponent_factor: String,
    pub max_long_pool_amount: String,
    pub max_short_pool_amount: String,
    pub max_long_pool_usd_for_deposit: String,
    pub max_short_pool_usd_for_deposit: String,
    pub virtual_market_id: String,
    pub virtual_index_token_id: String,
    pub virtual_long_token_id: String,
    pub virtual_short_token_id: String,
    pub use_open_interest_in_tokens_for_balance: bool,
    pub long_pool_amount: String,
    pub short_pool_amount: String,
    pub pool_value_max: String,
    pub pool_value_min: String,
    pub position_impact_pool_amount: String,
    pub long_interest_usd: String,
    pub short_interest_usd: String,
    #[serde(default)]
    pub long_interest_usd_using_long_token: Option<String>,
    #[serde(default)]
    pub long_interest_usd_using_short_token: Option<String>,
    #[serde(default)]
    pub short_interest_usd_using_long_token: Option<String>,
    #[serde(default)]
    pub short_interest_usd_using_short_token: Option<String>,
    pub long_interest_in_tokens: String,
    pub short_interest_in_tokens: String,
    #[serde(default)]
    pub long_interest_in_tokens_using_long_token: Option<String>,
    #[serde(default)]
    pub long_interest_in_tokens_using_short_token: Option<String>,
    #[serde(default)]
    pub short_interest_in_tokens_using_long_token: Option<String>,
    #[serde(default)]
    pub short_interest_in_tokens_using_short_token: Option<String>,
    pub longs_pay_shorts: bool,
    pub virtual_pool_amount_for_long_token: String,
    pub virtual_pool_amount_for_short_token: String,
    pub virtual_inventory_for_positions: String,
    pub virtual_inventory_for_positions_in_tokens: String,
    pub total_borrowing_fees: String,
    pub funding_factor_per_second: String,
    pub borrowing_factor_per_second_for_longs: String,
    pub borrowing_factor_per_second_for_shorts: String,
    pub swap_impact_pool_amount_long: String,
    pub swap_impact_pool_amount_short: String,
}

/// `GET /v1/markets/config`: the configuration half of [`GmxMarketInfo`] (no pool, OI, funding
/// or borrowing state).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GmxMarketConfig {
    pub data: String,
    pub name: String,
    pub market_token_address: String,
    pub index_token_address: String,
    pub long_token_address: String,
    pub short_token_address: String,
    pub is_disabled: bool,
    pub is_same_collaterals: bool,
    pub is_spot_only: bool,
    pub reserve_factor_long: String,
    pub reserve_factor_short: String,
    pub open_interest_reserve_factor_long: String,
    pub open_interest_reserve_factor_short: String,
    pub max_open_interest_long: String,
    pub max_open_interest_short: String,
    pub max_collateral_sum_long_token_long: String,
    pub max_collateral_sum_long_token_short: String,
    pub max_collateral_sum_short_token_long: String,
    pub max_collateral_sum_short_token_short: String,
    pub min_position_impact_pool_amount: String,
    pub position_impact_pool_distribution_rate: String,
    pub min_collateral_factor: String,
    pub min_collateral_factor_for_liquidation: String,
    pub min_collateral_factor_for_open_interest_long: String,
    pub min_collateral_factor_for_open_interest_short: String,
    pub max_pnl_factor_for_traders_long: String,
    pub max_pnl_factor_for_traders_short: String,
    #[serde(default)]
    pub max_pnl_factor_for_deposits_long: Option<String>,
    #[serde(default)]
    pub max_pnl_factor_for_deposits_short: Option<String>,
    #[serde(default)]
    pub max_pnl_factor_for_withdrawals_long: Option<String>,
    #[serde(default)]
    pub max_pnl_factor_for_withdrawals_short: Option<String>,
    pub position_fee_factor_for_balance_was_improved: String,
    pub position_fee_factor_for_balance_was_not_improved: String,
    pub position_impact_factor_positive: String,
    pub position_impact_factor_negative: String,
    pub max_position_impact_factor_positive: String,
    pub max_position_impact_factor_negative: String,
    pub max_position_impact_factor_for_liquidations: String,
    pub borrowing_factor_long: String,
    pub borrowing_factor_short: String,
    pub borrowing_exponent_factor_long: String,
    pub borrowing_exponent_factor_short: String,
    pub funding_factor: String,
    pub funding_exponent_factor: String,
    pub funding_increase_factor_per_second: String,
    pub min_funding_increase_rate_per_second: String,
    pub funding_decrease_factor_per_second: String,
    pub threshold_for_stable_funding: String,
    pub threshold_for_decrease_funding: String,
    pub min_funding_factor_per_second_long: String,
    pub min_funding_factor_per_second_short: String,
    pub max_funding_factor_per_second_long: String,
    pub max_funding_factor_per_second_short: String,
    pub max_lendable_impact_factor: String,
    pub max_lendable_impact_factor_for_withdrawals: String,
    pub max_lendable_impact_usd: String,
    pub lent_position_impact_pool_amount: String,
    pub position_impact_exponent_factor_positive: String,
    pub position_impact_exponent_factor_negative: String,
    pub swap_fee_factor_for_balance_was_improved: String,
    pub swap_fee_factor_for_balance_was_not_improved: String,
    pub atomic_swap_fee_factor: String,
    #[serde(default)]
    pub withdrawal_fee_factor_balance_was_improved: Option<String>,
    #[serde(default)]
    pub withdrawal_fee_factor_balance_was_not_improved: Option<String>,
    pub swap_impact_factor_positive: String,
    pub swap_impact_factor_negative: String,
    pub swap_impact_exponent_factor: String,
    pub max_long_pool_amount: String,
    pub max_short_pool_amount: String,
    pub max_long_pool_usd_for_deposit: String,
    pub max_short_pool_usd_for_deposit: String,
    pub virtual_market_id: String,
    pub virtual_index_token_id: String,
    pub virtual_long_token_id: String,
    pub virtual_short_token_id: String,
    pub use_open_interest_in_tokens_for_balance: bool,
}

/// `GET /v1/markets/values`: the live-state half of [`GmxMarketInfo`] (pool, OI, funding,
/// borrowing). `updated_at` (unix ms) is nullable and was null for one market in the capture.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GmxMarketValues {
    pub market_token_address: String,
    pub long_pool_amount: String,
    pub short_pool_amount: String,
    pub pool_value_max: String,
    pub pool_value_min: String,
    pub position_impact_pool_amount: String,
    pub swap_impact_pool_amount_long: String,
    pub swap_impact_pool_amount_short: String,
    pub long_interest_usd: String,
    pub short_interest_usd: String,
    pub long_interest_in_tokens: String,
    pub short_interest_in_tokens: String,
    #[serde(default)]
    pub long_interest_usd_using_long_token: Option<String>,
    #[serde(default)]
    pub long_interest_usd_using_short_token: Option<String>,
    #[serde(default)]
    pub short_interest_usd_using_long_token: Option<String>,
    #[serde(default)]
    pub short_interest_usd_using_short_token: Option<String>,
    #[serde(default)]
    pub long_interest_in_tokens_using_long_token: Option<String>,
    #[serde(default)]
    pub long_interest_in_tokens_using_short_token: Option<String>,
    #[serde(default)]
    pub short_interest_in_tokens_using_long_token: Option<String>,
    #[serde(default)]
    pub short_interest_in_tokens_using_short_token: Option<String>,
    pub longs_pay_shorts: bool,
    pub virtual_pool_amount_for_long_token: String,
    pub virtual_pool_amount_for_short_token: String,
    pub virtual_inventory_for_positions: String,
    pub virtual_inventory_for_positions_in_tokens: String,
    pub total_borrowing_fees: String,
    pub funding_factor_per_second: String,
    pub borrowing_factor_per_second_for_longs: String,
    pub borrowing_factor_per_second_for_shorts: String,
    #[serde(deserialize_with = "nullable")]
    pub updated_at: Option<u64>,
}

/// `GET /v1/risk-oracle/markets`: active markets as named by the risk oracle.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GmxRiskOracleMarket {
    pub name: String,
    pub market_token: String,
}

/// JIT-aware increase capacity (`GET /v1/markets/trading-capacity`, ticker `capacityLong/Short`).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GmxTradingCapacity {
    pub available_liquidity: String,
    pub base_available_liquidity: String,
    pub jit_available_liquidity: String,
    pub limiting_factor: GmxLimitingFactor,
    pub jit_data_status: GmxJitDataStatus,
    pub market_data_status: GmxMarketDataStatus,
}

/// Market ticker from `GET /v1/markets/tickers`. Prices and USD values are 30-decimal fixed point;
/// funding, borrowing and net rates are per-second factors (30 decimals).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GmxMarketTicker {
    pub symbol: String,
    pub market_token_address: String,
    pub min_price: String,
    pub max_price: String,
    pub mark_price: String,
    pub high24h: String,
    pub low24h: String,
    pub open24h: String,
    pub close24h: String,
    pub price_change24h: String,
    pub price_change_percent24h_bps: String,
    pub long_interest_in_tokens: String,
    pub short_interest_in_tokens: String,
    pub long_interest_usd: String,
    pub short_interest_usd: String,
    pub long_interest_usd_mark: String,
    pub short_interest_usd_mark: String,
    pub available_liquidity_long: String,
    pub available_liquidity_short: String,
    pub pool_amount_long_usd: String,
    pub pool_amount_short_usd: String,
    pub funding_rate_long: String,
    pub funding_rate_short: String,
    pub borrowing_rate_long: String,
    pub borrowing_rate_short: String,
    pub net_rate_long: String,
    pub net_rate_short: String,
    pub capacity_long: GmxTradingCapacity,
    pub capacity_short: GmxTradingCapacity,
}

/// Token from `GET /v1/tokens`. The optional fields are UI metadata present on some tokens only.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GmxToken {
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
    pub address: String,
    #[serde(default)]
    pub is_native: Option<bool>,
    #[serde(default)]
    pub is_shortable: Option<bool>,
    #[serde(default)]
    pub categories: Option<Vec<String>>,
    #[serde(default)]
    pub image_url: Option<String>,
    #[serde(default)]
    pub is_v1_available: Option<bool>,
    #[serde(default)]
    pub wrapped_address: Option<String>,
    #[serde(default)]
    pub asset_symbol: Option<String>,
    #[serde(default)]
    pub base_symbol: Option<String>,
    #[serde(default)]
    pub explorer_url: Option<String>,
    #[serde(default)]
    pub is_permit_supported: Option<bool>,
    #[serde(default)]
    pub contract_version: Option<String>,
    #[serde(default)]
    pub is_platform_token: Option<bool>,
    #[serde(default)]
    pub visual_multiplier: Option<u64>,
    #[serde(default)]
    pub visual_prefix: Option<String>,
    #[serde(default)]
    pub metamask_symbol: Option<String>,
    #[serde(default)]
    pub explorer_symbol: Option<String>,
    #[serde(default)]
    pub reserves_url: Option<String>,
    #[serde(default)]
    pub is_permit_disabled: Option<bool>,
    #[serde(default)]
    pub is_usdg: Option<bool>,
    #[serde(default)]
    pub is_wrapped: Option<bool>,
    #[serde(default)]
    pub is_stable: Option<bool>,
    #[serde(default)]
    pub is_synthetic: Option<bool>,
    #[serde(default)]
    pub is_temp_hidden: Option<bool>,
    #[serde(default)]
    pub is_chart_disabled: Option<bool>,
    #[serde(default)]
    pub is_platform_trading_token: Option<bool>,
    #[serde(default)]
    pub search_aliases: Option<Vec<String>>,
}

/// Token with current prices from `GET /v1/tokens/info`.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GmxTokenInfo {
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
    pub address: String,
    #[serde(default)]
    pub is_native: Option<bool>,
    #[serde(default)]
    pub is_shortable: Option<bool>,
    #[serde(default)]
    pub categories: Option<Vec<String>>,
    #[serde(default)]
    pub image_url: Option<String>,
    #[serde(default)]
    pub is_v1_available: Option<bool>,
    #[serde(default)]
    pub wrapped_address: Option<String>,
    pub prices: GmxMinMaxPrice,
    #[serde(default)]
    pub has_price_feed_provider: Option<bool>,
    #[serde(default)]
    pub asset_symbol: Option<String>,
    #[serde(default)]
    pub base_symbol: Option<String>,
    #[serde(default)]
    pub explorer_url: Option<String>,
    #[serde(default)]
    pub is_permit_supported: Option<bool>,
    #[serde(default)]
    pub contract_version: Option<String>,
    #[serde(default)]
    pub visual_multiplier: Option<u64>,
    #[serde(default)]
    pub visual_prefix: Option<String>,
    #[serde(default)]
    pub metamask_symbol: Option<String>,
    #[serde(default)]
    pub explorer_symbol: Option<String>,
    #[serde(default)]
    pub reserves_url: Option<String>,
    #[serde(default)]
    pub is_permit_disabled: Option<bool>,
    #[serde(default)]
    pub is_usdg: Option<bool>,
    #[serde(default)]
    pub is_wrapped: Option<bool>,
    #[serde(default)]
    pub is_stable: Option<bool>,
    #[serde(default)]
    pub is_synthetic: Option<bool>,
    #[serde(default)]
    pub is_temp_hidden: Option<bool>,
    #[serde(default)]
    pub is_chart_disabled: Option<bool>,
    #[serde(default)]
    pub is_platform_token: Option<bool>,
    #[serde(default)]
    pub is_platform_trading_token: Option<bool>,
    #[serde(default)]
    pub search_aliases: Option<Vec<String>>,
    #[serde(default)]
    pub wallet_balance: Option<String>,
    #[serde(default)]
    pub gmx_account_balance: Option<String>,
    #[serde(default)]
    pub source_chain_balance: Option<String>,
    #[serde(default)]
    pub balance_type: Option<u8>,
    #[serde(default)]
    pub balance: Option<String>,
    #[serde(default)]
    pub total_supply: Option<String>,
}

/// Candle from `GET /v1/prices/ohlcv`. `timestamp` is unix ms; prices are decimal USD strings.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GmxOhlcvCandle {
    pub timestamp: u64,
    pub open: String,
    pub high: String,
    pub low: String,
    pub close: String,
}

/// Hourly rate snapshot (`GET /v1/rates`). `timestamp` is unix seconds; rates are 30-decimal factors.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GmxRatesSnapshot {
    pub net_rate_long: String,
    pub net_rate_short: String,
    pub funding_rate_long: String,
    pub funding_rate_short: String,
    pub borrowing_rate_long: String,
    pub borrowing_rate_short: String,
    pub timestamp: u64,
}

/// Funding/borrowing rate history of one market from `GET /v1/rates`.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GmxMarketRates {
    pub market_address: String,
    pub rates_snapshots: Vec<GmxRatesSnapshot>,
}

/// Pair-feed row from `GET /v1/pairs`. USD values are plain numbers; perpetual rows carry funding,
/// `Spot` rows do not.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GmxPair {
    pub ticker_id: String,
    pub base_currency: String,
    pub target_currency: String,
    pub product_type: String,
    pub last_price: f64,
    #[serde(default)]
    pub high: Option<f64>,
    #[serde(default)]
    pub low: Option<f64>,
    pub base_volume: f64,
    pub target_volume: f64,
    pub pool_id: String,
    #[serde(default)]
    pub liquidity_in_usd: Option<f64>,
    #[serde(default)]
    pub open_interest: Option<f64>,
    #[serde(default)]
    pub long_open_interest: Option<f64>,
    #[serde(default)]
    pub short_open_interest: Option<f64>,
    #[serde(default)]
    pub funding_rate: Option<f64>,
    #[serde(default)]
    pub next_funding_rate: Option<f64>,
    #[serde(default)]
    pub next_funding_rate_timestamp: Option<u64>,
    #[serde(default)]
    pub funding_interval_minutes: Option<u64>,
    #[serde(default)]
    pub volume_usd: Option<f64>,
}

/// Open order from `GET /v1/orders`, `/v1/orders/{key}` and position `relatedOrders`.
///
/// `order_type`: 0 MarketSwap, 1 LimitSwap, 2 MarketIncrease, 3 LimitIncrease, 4 MarketDecrease,
/// 5 LimitDecrease, 6 StopLossDecrease, 7 Liquidation, 8 StopIncrease. `trigger_price` and
/// `acceptable_price` are in contract price format (30-decimal USD / 10^indexTokenDecimals).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GmxOrder {
    pub key: String,
    pub account: String,
    pub receiver: String,
    pub cancellation_receiver: String,
    pub callback_contract: String,
    pub ui_fee_receiver: String,
    pub market_address: String,
    pub initial_collateral_token_address: String,
    pub swap_path: Vec<String>,
    pub order_type: u8,
    pub decrease_position_swap_type: u8,
    pub size_delta_usd: String,
    pub initial_collateral_delta_amount: String,
    pub trigger_price: String,
    pub acceptable_price: String,
    pub execution_fee: String,
    pub callback_gas_limit: String,
    pub min_output_amount: String,
    pub ui_fee_factor: Option<String>,
    pub updated_at_time: String,
    pub valid_from_time: String,
    pub src_chain_id: String,
    pub is_long: bool,
    pub should_unwrap_native_token: bool,
    pub is_frozen: bool,
    pub auto_cancel: bool,
    pub data_list: Vec<String>,
    pub request_id: Option<String>,
}

/// Open position from `GET /v1/positions` and `/v1/positions/{key}`. USD values are 30-decimal
/// fixed point. `key` is `account:market:collateral:isLong`; `contract_key` is the bytes32 hash.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GmxPosition {
    pub key: String,
    pub contract_key: String,
    pub account: String,
    pub market_address: String,
    pub collateral_token_address: String,
    pub size_in_usd: String,
    pub size_in_tokens: String,
    pub collateral_amount: String,
    pub pending_borrowing_fees_usd: String,
    pub increased_at_time: String,
    pub decreased_at_time: String,
    pub is_long: bool,
    pub funding_fee_amount: String,
    pub claimable_long_token_amount: String,
    pub claimable_short_token_amount: String,
    pub pnl: String,
    pub position_fee_amount: String,
    pub trader_discount_amount: String,
    pub ui_fee_amount: String,
    pub pending_impact_amount: String,
    pub position_value_in_usd: String,
    pub data: String,
    pub index_name: String,
    pub pool_name: String,
    pub mark_price: String,
    #[serde(default)]
    pub entry_price: Option<String>,
    #[serde(default)]
    pub liquidation_price: Option<String>,
    pub collateral_usd: String,
    pub remaining_collateral_usd: String,
    pub remaining_collateral_amount: String,
    pub has_low_collateral: bool,
    #[serde(default)]
    pub leverage: Option<String>,
    #[serde(default)]
    pub leverage_with_pnl: Option<String>,
    #[serde(default)]
    pub leverage_without_pnl: Option<String>,
    pub pnl_percentage: String,
    pub pnl_after_fees: String,
    pub pnl_after_fees_percentage: String,
    pub net_value_after_all_fees: String,
    pub pnl_after_all_fees: String,
    pub pnl_after_all_fees_percentage: String,
    pub net_value: String,
    pub net_price_imapct_delta_usd: String,
    pub price_impact_diff_usd: String,
    pub pending_impact_usd: String,
    pub close_price_impact_delta_usd: String,
    pub closing_fee_usd: String,
    pub ui_fee_usd: String,
    pub pending_funding_fees_usd: String,
    pub pending_claimable_funding_fees_usd: String,
    #[serde(default)]
    pub related_orders: Option<Vec<GmxOrder>>,
    #[serde(default)]
    pub is_opening: Option<bool>,
}

/// Trade-history action from `GET /v1/trades` (`TradesListResponse.trades`).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GmxTradeAction {
    pub id: String,
    pub event_name: String,
    pub account: String,
    #[serde(default)]
    pub market_address: Option<String>,
    pub swap_path: Vec<String>,
    pub initial_collateral_token_address: String,
    pub initial_collateral_delta_amount: String,
    #[serde(default)]
    pub size_delta_usd: Option<String>,
    #[serde(default)]
    pub trigger_price: Option<String>,
    #[serde(default)]
    pub acceptable_price: Option<String>,
    #[serde(default)]
    pub min_output_amount: Option<String>,
    #[serde(default)]
    pub index_token_price_min: Option<String>,
    #[serde(default)]
    pub index_token_price_max: Option<String>,
    pub order_type: u8,
    pub order_key: String,
    #[serde(default)]
    pub is_long: Option<bool>,
    #[serde(default)]
    pub should_unwrap_native_token: Option<bool>,
    #[serde(default)]
    pub decrease_position_swap_type: Option<u8>,
    #[serde(default)]
    pub src_chain_id: Option<String>,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub reason_bytes: Option<String>,
    pub timestamp: u64,
    pub transaction_hash: String,
    #[serde(default)]
    pub execution_amount_out: Option<String>,
    #[serde(default)]
    pub swap_impact_usd: Option<String>,
    #[serde(default)]
    pub swap_fee_usd: Option<String>,
    #[serde(default)]
    pub size_delta_in_tokens: Option<String>,
    #[serde(default)]
    pub execution_price: Option<String>,
    #[serde(default)]
    pub price_impact_usd: Option<String>,
    #[serde(default)]
    pub position_fee_amount: Option<String>,
    #[serde(default)]
    pub trader_discount_amount: Option<String>,
    #[serde(default)]
    pub borrowing_fee_amount: Option<String>,
    #[serde(default)]
    pub funding_fee_amount: Option<String>,
    #[serde(default)]
    pub pnl_usd: Option<String>,
    #[serde(default)]
    pub collateral_token_price_max: Option<String>,
    #[serde(default)]
    pub collateral_token_price_min: Option<String>,
    #[serde(default)]
    pub collateral_total_cost_amount: Option<String>,
    #[serde(default)]
    pub price_impact_diff_usd: Option<String>,
    #[serde(default)]
    pub liquidation_fee_amount: Option<String>,
    #[serde(default)]
    pub base_pnl_usd: Option<String>,
    #[serde(default)]
    pub total_impact_usd: Option<String>,
    #[serde(default)]
    pub proportional_pending_impact_usd: Option<String>,
    #[serde(default)]
    pub twap_group_id: Option<String>,
    #[serde(default)]
    pub number_of_parts: Option<u64>,
}

/// Wallet token balance from `GET /v1/balances/wallet`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GmxWalletBalance {
    pub symbol: String,
    pub address: String,
    pub decimals: u8,
    pub balance: String,
}
