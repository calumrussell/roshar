//! Archive (indexer, `POST /archive/v1`) request and reply types.

use super::gateway::{NadoPerpBalance, NadoPerpProduct, NadoSpotBalance, NadoSpotProduct};
use super::num::{I128, U128, U64};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Archive error body (HTTP 4xx): `{"error", "error_code"}`.
#[derive(Debug, Clone, Deserialize)]
pub struct NadoArchiveError {
    pub error: String,
    pub error_code: i64,
}

/// `candlesticks` request. Results are newest first.
#[derive(Debug, Clone, Serialize)]
pub struct NadoCandlesticksQuery {
    pub product_id: u32,
    /// Seconds; one of the documented granularities (60, 300, 900, 3600, 7200, 14400, 86400,
    /// 604800, 2419200).
    pub granularity: u32,
    /// Unix seconds; only candles with `timestamp <= max_time`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_time: Option<u64>,
    /// Server default 100, max 500.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NadoCandle {
    pub product_id: u32,
    pub granularity: u32,
    pub submission_idx: U64,
    /// Candle start, unix seconds.
    pub timestamp: U64,
    pub open_x18: I128,
    pub high_x18: I128,
    pub low_x18: I128,
    pub close_x18: I128,
    /// Base volume, x18.
    pub volume: I128,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NadoCandlesticks {
    pub candlesticks: Vec<NadoCandle>,
}

/// `funding_rate` reply (latest predicted 24h rate).
#[derive(Debug, Clone, Deserialize)]
pub struct NadoFundingRate {
    pub product_id: u32,
    pub funding_rate_x18: I128,
    /// Unix seconds.
    pub update_time: U64,
    /// Omitted for markets not using the premium-index formula.
    #[serde(default)]
    pub premium_x18: Option<I128>,
}

/// `funding_rates` reply, keyed by product id (as a string).
pub type NadoFundingRates = HashMap<String, NadoFundingRate>;

/// `funding_rate_history` request. Results are oldest first.
#[derive(Debug, Clone, Serialize)]
pub struct NadoFundingRateHistoryQuery {
    pub product_id: u32,
    /// Unix seconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_time: Option<u64>,
    /// Unix seconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_time: Option<u64>,
    /// Server default 100, max 1000.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}

/// A realized hourly funding rate.
#[derive(Debug, Clone, Deserialize)]
pub struct NadoFundingRateHistoryEntry {
    pub product_id: u32,
    /// Unix seconds.
    pub timestamp: U64,
    pub funding_rate_x18: I128,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NadoFundingRateHistory {
    pub funding_rates: Vec<NadoFundingRateHistoryEntry>,
}

/// `price` reply (one perp) and the values of the `perp_prices` reply.
#[derive(Debug, Clone, Deserialize)]
pub struct NadoPerpPrices {
    pub product_id: u32,
    pub index_price_x18: I128,
    pub mark_price_x18: I128,
    /// Unix seconds.
    pub update_time: U64,
}

/// `perp_prices` reply, keyed by product id (as a string).
pub type NadoPerpPricesMap = HashMap<String, NadoPerpPrices>;

#[derive(Debug, Clone, Deserialize)]
pub struct NadoOraclePrice {
    pub product_id: u32,
    pub oracle_price_x18: I128,
    /// Unix seconds.
    pub update_time: U64,
}

/// `oracle_price` reply.
#[derive(Debug, Clone, Deserialize)]
pub struct NadoOraclePrices {
    pub prices: Vec<NadoOraclePrice>,
}

/// Product balance snapshot in archive `pre_balance` / `post_balance`:
/// `{"spot": {...}}` or `{"perp": {...}}`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NadoProductBalance {
    Spot {
        product_id: u32,
        balance: NadoSpotBalance,
    },
    Perp {
        product_id: u32,
        balance: NadoPerpBalance,
    },
}

#[derive(Debug, Clone, Deserialize)]
pub struct NadoBalanceSnapshot {
    pub base: NadoProductBalance,
    /// `null` in live replies for both spot and perp orders.
    pub quote: Option<NadoProductBalance>,
}

/// `matches` request. Provide `subaccounts` and/or `product_ids`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct NadoMatchesQuery {
    /// bytes32 hex subaccounts, max 5.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subaccounts: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product_ids: Option<Vec<u32>>,
    /// Only matches with `submission_idx <= idx`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idx: Option<u64>,
    /// Unix seconds; used when `idx` is absent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_time: Option<u64>,
    /// Server default 100, max 500.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub isolated: Option<bool>,
}

/// The order a match belongs to, as signed.
#[derive(Debug, Clone, Deserialize)]
pub struct NadoMatchOrder {
    pub sender: String,
    #[serde(rename = "priceX18")]
    pub price_x18: I128,
    pub amount: I128,
    pub expiration: U64,
    pub nonce: U64,
    pub appendix: U128,
}

/// One fill (`matches` reply).
#[derive(Debug, Clone, Deserialize)]
pub struct NadoMatch {
    pub digest: String,
    pub order: NadoMatchOrder,
    pub base_filled: I128,
    pub quote_filled: I128,
    /// Trading + sequencer fees.
    pub fee: I128,
    pub builder_fee: I128,
    pub sequencer_fee: I128,
    pub cumulative_fee: I128,
    pub cumulative_base_filled: I128,
    pub cumulative_quote_filled: I128,
    pub closed_amount: I128,
    pub realized_pnl: I128,
    pub closed_net_entry: I128,
    /// Isolated orders only; `null` for cross margin.
    pub margin: Option<I128>,
    /// Not in the docs; present in live replies.
    pub net_entry_unrealized: I128,
    /// Not in the docs; present in live replies.
    pub net_entry_cumulative: I128,
    pub submission_idx: U64,
    pub isolated: bool,
    pub is_taker: bool,
    pub pre_balance: NadoBalanceSnapshot,
    pub post_balance: NadoBalanceSnapshot,
}

/// A transaction referenced by `matches` / archive replies. `tx` is one of several variants
/// (`match_orders`, `match_orders_v2`, ...) and is kept as raw JSON.
#[derive(Debug, Clone, Deserialize)]
pub struct NadoArchiveTx {
    pub tx: serde_json::Value,
    pub submission_idx: U64,
    /// Unix seconds.
    pub timestamp: U64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NadoMatches {
    pub matches: Vec<NadoMatch>,
    pub txs: Vec<NadoArchiveTx>,
}

/// Archive `orders` request: either by `subaccounts` (with optional filters) or by `digests`
/// alone.
#[derive(Debug, Clone, Default, Serialize)]
pub struct NadoArchiveOrdersQuery {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subaccounts: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product_ids: Option<Vec<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idx: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_time: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub digests: Option<Vec<String>>,
    /// `price_trigger`, `time_trigger`, `none`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trigger_types: Option<Vec<String>>,
    /// Server default 100, max 500.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub isolated: Option<bool>,
}

/// A historical (matched) order.
#[derive(Debug, Clone, Deserialize)]
pub struct NadoArchiveOrder {
    pub digest: String,
    pub subaccount: String,
    pub product_id: u32,
    pub submission_idx: U64,
    pub last_fill_submission_idx: U64,
    pub amount: I128,
    pub price_x18: I128,
    pub base_filled: I128,
    pub quote_filled: I128,
    pub fee: I128,
    pub builder_fee: I128,
    pub closed_amount: I128,
    pub realized_pnl: I128,
    pub closed_net_entry: I128,
    /// Isolated orders only; `null` for cross margin.
    pub closed_margin: Option<I128>,
    pub first_fill_timestamp: U64,
    pub last_fill_timestamp: U64,
    pub expiration: U64,
    pub nonce: U64,
    pub appendix: U128,
    pub isolated: bool,
    pub pre_balance: NadoBalanceSnapshot,
    pub post_balance: NadoBalanceSnapshot,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NadoArchiveOrders {
    pub orders: Vec<NadoArchiveOrder>,
}

/// `interest_and_funding` request.
#[derive(Debug, Clone, Serialize)]
pub struct NadoInterestAndFundingQuery {
    pub subaccount: String,
    pub product_ids: Vec<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_idx: Option<u64>,
    /// Required; max 100.
    pub limit: u32,
}

/// An interest (spot) or funding (perp) payment.
#[derive(Debug, Clone, Deserialize)]
pub struct NadoPayment {
    pub product_id: u32,
    pub idx: U64,
    /// Unix seconds.
    pub timestamp: U64,
    pub amount: I128,
    pub balance_amount: I128,
    pub rate_x18: I128,
    pub oracle_price_x18: I128,
    /// Not in the docs; present in live replies.
    pub isolated: bool,
    /// Not in the docs; `null` in live replies for cross-margin payments.
    pub isolated_product_id: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NadoInterestAndFunding {
    pub interest_payments: Vec<NadoPayment>,
    pub funding_payments: Vec<NadoPayment>,
    /// Pass as `max_idx` for the next page; `null` when there are no more records.
    pub next_idx: Option<U64>,
}

/// Product state in archive snapshots: `{"spot": {..}}` or `{"perp": {..}}`, the same objects as
/// the gateway `all_products` entries.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NadoProductState {
    Spot(NadoSpotProduct),
    Perp(NadoPerpProduct),
}

/// One product snapshot (`products` and `product_snapshots` replies).
#[derive(Debug, Clone, Deserialize)]
pub struct NadoProductSnapshot {
    pub product_id: u32,
    /// Transaction that produced the snapshot; matches `NadoArchiveTx::submission_idx`.
    pub submission_idx: U64,
    pub product: NadoProductState,
}

/// `products` request: snapshots of one product, newest first.
#[derive(Debug, Clone, Serialize)]
pub struct NadoProductHistoryQuery {
    pub product_id: u32,
    /// Only snapshots with `submission_idx <= idx`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idx: Option<u64>,
    /// Unix seconds; used when `idx` is absent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_time: Option<u64>,
    /// Server default 100, max 500.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}

/// `products` reply, both lists newest first. `txs` are the transactions that produced the
/// snapshots (e.g. `{"update_price": {..}}`).
#[derive(Debug, Clone, Deserialize)]
pub struct NadoProductHistory {
    pub products: Vec<NadoProductSnapshot>,
    pub txs: Vec<NadoArchiveTx>,
}

/// `product_snapshots` reply: latest snapshot (at or before `max_time`) keyed by product id (as
/// a string).
pub type NadoProductSnapshots = HashMap<String, NadoProductSnapshot>;

/// `interval` of the snapshot queries. Results are newest first.
#[derive(Debug, Clone, Serialize)]
pub struct NadoSnapshotInterval {
    /// Number of snapshots (`count * product_ids.len() < 2000`).
    pub count: u32,
    /// Seconds between snapshots.
    pub granularity: u32,
    /// Unix seconds; server default now.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_time: Option<u64>,
}

/// `oracle_snapshots`, `market_snapshots` and `edge_market_snapshots` request.
#[derive(Debug, Clone, Serialize)]
pub struct NadoSnapshotsQuery {
    pub interval: NadoSnapshotInterval,
    /// Server default: all products.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product_ids: Option<Vec<u32>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NadoOracleSnapshot {
    /// Unix seconds.
    pub timestamp: u64,
    /// Product id (as a string) to oracle price x18.
    pub oracle_prices: HashMap<String, I128>,
}

/// `oracle_snapshots` reply.
#[derive(Debug, Clone, Deserialize)]
pub struct NadoOracleSnapshots {
    pub snapshots: Vec<NadoOracleSnapshot>,
}

/// One market snapshot. Per-product maps are keyed by product id (as a string) and hold only
/// the products the field applies to (e.g. `open_interests` and `funding_rates` for perps).
#[derive(Debug, Clone, Deserialize)]
pub struct NadoMarketSnapshot {
    /// Unix seconds; the nearest transaction at or before the interval point.
    pub timestamp: u64,
    pub cumulative_users: u64,
    pub daily_active_users: u64,
    pub cumulative_trades: HashMap<String, u64>,
    /// Quote volume, x18.
    pub cumulative_volumes: HashMap<String, I128>,
    /// Base size, x18.
    pub cumulative_trade_sizes: HashMap<String, I128>,
    pub cumulative_taker_fees: HashMap<String, I128>,
    pub cumulative_sequencer_fees: HashMap<String, I128>,
    pub cumulative_maker_fees: HashMap<String, I128>,
    pub cumulative_liquidation_amounts: HashMap<String, I128>,
    pub open_interests: HashMap<String, I128>,
    pub total_deposits: HashMap<String, I128>,
    pub total_borrows: HashMap<String, I128>,
    /// Realized hourly funding rates, x18.
    pub funding_rates: HashMap<String, I128>,
    pub deposit_rates: HashMap<String, I128>,
    pub borrow_rates: HashMap<String, I128>,
    pub cumulative_inflows: HashMap<String, I128>,
    pub cumulative_outflows: HashMap<String, I128>,
    /// Not in the docs; present in live replies.
    pub oracle_prices: HashMap<String, I128>,
    pub tvl: I128,
}

/// `market_snapshots` reply.
#[derive(Debug, Clone, Deserialize)]
pub struct NadoMarketSnapshots {
    pub snapshots: Vec<NadoMarketSnapshot>,
}

/// `edge_market_snapshots` reply: snapshots keyed by chain id (as a string).
#[derive(Debug, Clone, Deserialize)]
pub struct NadoEdgeMarketSnapshots {
    pub snapshots: HashMap<String, Vec<NadoMarketSnapshot>>,
}

/// One entry of the `liquidation_feed` reply: a subaccount eligible for liquidation.
#[derive(Debug, Clone, Deserialize)]
pub struct NadoLiquidatable {
    pub subaccount: String,
    /// Unix seconds.
    pub update_time: u64,
}

/// `quote_price` reply: USDT0 price in USD, x18.
#[derive(Debug, Clone, Deserialize)]
pub struct NadoQuotePrice {
    pub price_x18: I128,
}

/// `backlog` reply (sequencer backlog).
#[derive(Debug, Clone, Deserialize)]
pub struct NadoBacklog {
    pub total_txs: U64,
    pub total_submissions: U64,
    /// `total_txs - total_submissions`.
    pub backlog_size: I128,
    /// Unix seconds.
    pub updated_at: U64,
    /// Decimal string; `null` when unavailable.
    #[serde(deserialize_with = "opt_f64_str")]
    pub backlog_eta_in_seconds: Option<f64>,
    /// Decimal string; `null` when unavailable.
    #[serde(deserialize_with = "opt_f64_str")]
    pub txs_per_second: Option<f64>,
}

/// A nullable decimal sent as a JSON string. The field must be present.
fn opt_f64_str<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<f64>, D::Error> {
    match Option::<String>::deserialize(d)? {
        None => Ok(None),
        Some(s) => s
            .parse::<f64>()
            .map(Some)
            .map_err(|e| serde::de::Error::custom(format_args!("invalid decimal {s:?}: {e}"))),
    }
}
