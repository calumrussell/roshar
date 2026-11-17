//! Wire types for the public API (`POST /v1/info`, the aggregator `GET` endpoints, the info
//! websocket) and the shared response envelope. Account and trading types are in [`trading`].
//!
//! Field optionality follows live responses (captured 2026-11-15, see `tests/fixtures/`) and the
//! docs: a field is an `Option` only where the docs or the official SDK call it optional or
//! nullable.

pub mod trading;

use serde::de::DeserializeOwned;
use serde::Deserialize;
use std::collections::HashMap;

// ── Envelope ────────────────────────────────────────────────────────────────

/// `error` object of a failed `/v1/info` or `/v1/trade` response (and of websocket errors).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SynthetixApiError {
    /// e.g. `VALIDATION_ERROR`, `UNAUTHORIZED`, `RATE_LIMIT_EXCEEDED`.
    pub code: String,
    pub message: String,
    /// `REQUEST`, `AUTH`, `RATE_LIMIT`, `TRADING`, `SYSTEM`. Absent in some documented examples.
    pub category: Option<String>,
    /// Absent in some documented examples.
    pub retryable: Option<bool>,
    #[serde(default)]
    pub details: Option<serde_json::Value>,
}

impl std::fmt::Display for SynthetixApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Synthetix error {}: {}", self.code, self.message)
    }
}

impl std::error::Error for SynthetixApiError {}

/// `{"status":"ok","response":…}` / `{"status":"error","error":{…}}`.
#[derive(Debug, Deserialize)]
#[serde(tag = "status", rename_all = "lowercase")]
pub enum SynthetixEnvelope<T> {
    Ok { response: T },
    Error { error: SynthetixApiError },
}

impl<T> SynthetixEnvelope<T> {
    pub fn into_result(self) -> Result<T, SynthetixApiError> {
        match self {
            SynthetixEnvelope::Ok { response } => Ok(response),
            SynthetixEnvelope::Error { error } => Err(error),
        }
    }
}

/// `rateLimit` / `ipRateLimit` snapshot carried on response envelopes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SynthetixRateLimitSnapshot {
    pub requests_cap: u64,
    pub remaining_tokens: u64,
}

// ── POST /v1/info ───────────────────────────────────────────────────────────

/// `getMarkets` item.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SynthetixMarket {
    pub symbol: String,
    pub description: String,
    pub base_asset: String,
    pub quote_asset: String,
    pub settle_asset: String,
    pub is_open: bool,
    pub is_close_only: bool,
    pub price_exponent: u32,
    pub quantity_exponent: u32,
    pub price_increment: String,
    pub min_order_size: String,
    pub order_size_increment: String,
    pub contract_size: u64,
    pub max_market_order_size: String,
    pub max_limit_order_size: String,
    pub min_order_price: String,
    pub limit_order_price_cap_ratio: String,
    pub limit_order_price_floor_ratio: String,
    pub market_order_price_cap_ratio: String,
    pub market_order_price_floor_ratio: String,
    pub liquidation_clearance_fee: String,
    pub min_notional_value: String,
    pub maintenance_margin_tiers: Vec<SynthetixMarginTier>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SynthetixMarginTier {
    pub min_position_size: String,
    /// `""` = unlimited.
    pub max_position_size: String,
    pub max_leverage: u32,
    pub initial_margin_requirement: String,
    pub maintenance_margin_requirement: String,
    pub maintenance_deduction_value: String,
}

/// `getMarketPrices` value (the response is keyed by symbol).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SynthetixMarketPrice {
    pub symbol: String,
    pub best_bid: String,
    pub best_ask: String,
    pub mark_price: String,
    pub index_price: String,
    pub last_price: String,
    /// Deprecated by the exchange in favour of `period_stats["24h"]`.
    pub prev_day_price: String,
    /// Deprecated by the exchange in favour of `period_stats["24h"]`.
    pub volume24h: String,
    /// Deprecated by the exchange in favour of `period_stats["24h"]`.
    pub quote_volume24h: String,
    pub funding_rate: String,
    pub open_interest: String,
    pub timestamp: u64,
    /// SDK: optional; each window (`"1h"`, `"24h"`, `"7d"`) present only with a usable reference price.
    pub period_stats: Option<HashMap<String, SynthetixPeriodStats>>,
    /// SDK: present only for session markets whose trading-hours state is known.
    pub on_hours: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SynthetixPeriodStats {
    pub reference_price: String,
    pub volume: String,
    pub quote_volume: String,
}

/// `[price, quantity]` decimal strings.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct SynthetixLevel(pub String, pub String);

/// `getOrderbook`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SynthetixOrderBook {
    pub bids: Vec<SynthetixLevel>,
    pub asks: Vec<SynthetixLevel>,
}

/// `getLastTrades` response.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SynthetixLastTrades {
    pub trades: Vec<SynthetixPublicTrade>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SynthetixPublicTrade {
    pub trade_id: String,
    pub symbol: String,
    /// Taker side.
    pub side: String,
    pub price: String,
    pub quantity: String,
    pub timestamp: u64,
    pub is_maker: bool,
}

/// `getFundingRate`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SynthetixFundingRate {
    pub symbol: String,
    pub estimated_funding_rate: String,
    pub last_settlement_rate: String,
    pub last_settlement_time: u64,
    pub next_funding_time: u64,
    /// Milliseconds.
    pub funding_interval: u64,
}

/// `getFundingRateHistory`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SynthetixFundingRateHistory {
    pub symbol: String,
    pub funding_rates: Vec<SynthetixFundingRateEntry>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SynthetixFundingRateEntry {
    pub funding_rate: String,
    pub funding_time: u64,
    pub applied_at: u64,
}

/// `getCandles`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SynthetixCandles {
    pub symbol: String,
    pub interval: String,
    pub candles: Vec<SynthetixCandle>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SynthetixCandle {
    pub open_time: u64,
    pub close_time: u64,
    pub open_price: String,
    pub high_price: String,
    pub low_price: String,
    pub close_price: String,
    pub volume: String,
    pub quote_volume: String,
    pub trade_count: u64,
}

/// `getOpenInterest` item.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SynthetixOpenInterest {
    pub symbol: String,
    /// `""` when `error_message` is set.
    pub open_interest: String,
    pub long_open_interest: String,
    pub short_open_interest: String,
    pub timestamp: u64,
    /// SDK: present only when this market's lookup failed.
    pub error_message: Option<String>,
}

/// `getCollaterals` item.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SynthetixCollateralConfig {
    pub collateral: String,
    pub deposit_cap: String,
    pub lltv: String,
    pub ltv: String,
    /// SDK: may be empty.
    pub market: String,
    pub tiers: Vec<SynthetixCollateralTier>,
    pub withdraw_fee: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SynthetixCollateralTier {
    pub id: u64,
    pub haircut: String,
    pub max_amount: String,
    pub min_amount: String,
    pub name: String,
    pub value_addition: String,
    pub value_ratio: String,
}

/// `getTiers` item.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SynthetixTier {
    pub tier_id: String,
    pub tier_type: String,
    pub tier_name: String,
    pub min_trade_volume: String,
    pub maker_fee_rate: String,
    pub taker_fee_rate: String,
    pub max_sub_accounts: u32,
    pub max_orders_per_market: u32,
    pub max_total_orders: u32,
    pub max_borrow_capacity: String,
}

/// `getSubAccountIds` with `includeDelegations: true`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SynthetixSubAccountIdsWithDelegations {
    pub sub_account_ids: Vec<String>,
    pub managed_sub_account_ids: Vec<String>,
    pub delegated_sub_account_ids: Vec<String>,
}

// ── GET endpoints ───────────────────────────────────────────────────────────

/// `GET /v1/info/contracts` item (aggregator format, snake_case).
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SynthetixContract {
    pub ticker_id: String,
    pub base_currency: String,
    pub target_currency: String,
    pub index_currency: String,
    pub product_type: String,
    pub last_price: String,
    pub index_price: String,
    pub base_volume: String,
    pub target_volume: String,
    pub funding_rate: String,
    pub next_funding_rate: String,
    /// Unix seconds.
    pub next_funding_rate_timestamp: u64,
    pub funding_interval_minutes: String,
    pub open_interest: String,
    pub open_interest_usd: String,
    pub long_open_interest: String,
    pub long_open_interest_usd: String,
    pub short_open_interest: String,
    pub short_open_interest_usd: String,
}

/// `GET /v1/info/contract_specs` item.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SynthetixContractSpec {
    pub ticker_id: String,
    pub contract_type: String,
    pub contract_price_currency: String,
}

/// `GET /v1/info/orderbook`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SynthetixAggregatorOrderBook {
    pub ticker_id: String,
    pub timestamp: u64,
    pub bids: Vec<SynthetixLevel>,
    pub asks: Vec<SynthetixLevel>,
}

/// Error body of the aggregator `GET` endpoints: `{"error":"…"}`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SynthetixPlainError {
    pub error: String,
}

/// `GET /v1/status` (not enveloped).
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SynthetixStatus {
    pub maintenance: bool,
    /// SDK: present only while maintenance is on.
    pub message: Option<String>,
}

/// `GET /v1/exchange/status` response.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SynthetixExchangeStatus {
    pub exchange_status: String,
    pub timestamp_ms: u64,
}

// ── Info websocket ──────────────────────────────────────────────────────────

/// Reply to a websocket request (`subscribe`, `unsubscribe`, `post`, `auth`, `ping`).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SynthetixWsReply {
    pub id: String,
    pub request_id: String,
    /// HTTP-style status: 200 on success.
    pub status: u16,
    pub timestamp: u64,
    /// Present on replies to rate-limited requests (`post`), absent on `ping`/`subscribe`.
    pub ip_rate_limit: Option<SynthetixRateLimitSnapshot>,
    pub result: Option<serde_json::Value>,
    pub error: Option<SynthetixWsError>,
}

impl SynthetixWsReply {
    /// Payload of an info-socket `post` reply (`result.response`), typed as the action's response
    /// (see [`InfoPost`](crate::ws::InfoPost)). An error reply returns its [`SynthetixWsError`];
    /// a `result.status` other than `"success"` or a payload that does not parse as `T` is an
    /// error too.
    pub fn into_post_response<T: DeserializeOwned>(
        self,
    ) -> Result<T, Box<dyn std::error::Error + Send + Sync>> {
        if let Some(e) = self.error {
            return Err(Box::new(e));
        }
        let Some(result) = self.result else {
            return Err(format!(
                "Synthetix websocket reply {} has neither result nor error",
                self.id
            )
            .into());
        };
        let post: SynthetixWsPostResult<T> = serde_json::from_value(result).map_err(|e| {
            format!(
                "Failed to parse Synthetix websocket post reply {}: {e}",
                self.id
            )
        })?;
        if post.status != "success" {
            return Err(format!(
                "Synthetix websocket post reply {} has status {}",
                self.id, post.status
            )
            .into());
        }
        Ok(post.response)
    }
}

/// Websocket `error` object (`code` is numeric here, `errorCode` is the string code).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SynthetixWsError {
    pub code: u16,
    pub error_code: String,
    pub category: String,
    pub message: String,
    pub retryable: bool,
    /// e.g. `{"symbol":"NOPE-USDT"}` on "unknown market"; absent on most errors.
    #[serde(default)]
    pub details: Option<serde_json::Value>,
}

impl std::fmt::Display for SynthetixWsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Synthetix websocket error {} {}: {}",
            self.code, self.error_code, self.message
        )
    }
}

impl std::error::Error for SynthetixWsError {}

/// `orderbook_depth_update` push. `kind` is `"snapshot"` / `"diff"` in diff mode and absent in
/// snapshot-mode subscriptions.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SynthetixWsOrderBook {
    pub channel: String,
    #[serde(rename = "type")]
    pub kind: Option<String>,
    pub meseq: u64,
    /// `null` / absent on snapshots.
    pub prev_meseq: Option<u64>,
    /// Matching-engine time, Unix µs as a string.
    pub met: String,
    pub checksum: String,
    pub data: SynthetixWsOrderBookData,
    pub timestamp: u64,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SynthetixWsOrderBookData {
    pub symbol: String,
    pub bids: Vec<SynthetixWsLevel>,
    pub asks: Vec<SynthetixWsLevel>,
}

/// `quantity: "0"` removes the level in a diff.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SynthetixWsLevel {
    pub price: String,
    pub quantity: String,
}

/// `trade` push.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SynthetixWsTrade {
    pub channel: String,
    /// Docs: omitted when zero.
    pub meseq: Option<u64>,
    /// Docs: omitted when zero.
    pub met: Option<String>,
    /// Docs: present only when `true`.
    pub provenance_missing: Option<bool>,
    pub data: SynthetixPublicTrade,
    pub timestamp: u64,
}

/// `market_price_update` push: one price type per message.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SynthetixWsMarketPrice {
    pub channel: String,
    /// Present (`true`) on the initial snapshot after subscribing.
    pub is_snapshot: Option<bool>,
    pub seq: u64,
    pub data: SynthetixWsMarketPriceData,
    pub timestamp: u64,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SynthetixWsMarketPriceData {
    pub symbol: String,
    pub price: String,
    /// RFC 3339.
    pub last_update_time: String,
    /// `mark`, `last`, `index` or `mid`.
    pub update_type: String,
    /// Sent on `mark` updates only (live 2026-11-16; not in the docs).
    pub funding_rate: Option<String>,
    /// Unix ms. Sent on `mark` updates only (live 2026-11-16; not in the docs).
    pub next_funding_time: Option<u64>,
}

/// `candle_update` push (snake_case fields, RFC 3339 times).
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SynthetixWsCandle {
    pub channel: String,
    pub seq: u64,
    pub data: SynthetixWsCandleData,
    pub timestamp: u64,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SynthetixWsCandleData {
    pub symbol: String,
    pub timeframe: String,
    pub open_time: String,
    pub close_time: String,
    pub open_price: String,
    pub high_price: String,
    pub low_price: String,
    pub close_price: String,
    pub volume: String,
    pub quote_volume: String,
    pub trade_count: u64,
}

/// Info-websocket `post` result: `{"response": …, "status": "success"}`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SynthetixWsPostResult<T> {
    pub response: T,
    pub status: String,
}
