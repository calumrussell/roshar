//! Data API v2 (`https://data-api.polymarket.com/v2`) wire types, market-level routes only.
//!
//! Spec: https://data-api.polymarket.com/v2/openapi.json. Every route answers
//! `{ "data": ..., "pagination"?: {...} }`; errors are `{ "error", "code", "parameter"?,
//! "retryable", "trace_id" }`. Field optionality was checked against full live responses
//! (2026-11-16): keys the spec marks nullable are always sent (`null` when unset), so they are
//! `Option` with a required key; keys the spec says are omitted are `#[serde(default)]`.

use crate::rest::Query;
use crate::types::de::nullable;
use serde::{Deserialize, Serialize};

/// `{ "data": [...], "pagination": {...} }`: keyset-paged routes (`/v2/trades`, `/v2/holders`,
/// `/v2/prices-history`). Follow `pagination.next_cursor` until it is `None`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DataPage<T> {
    pub data: Vec<T>,
    pub pagination: DataPagination,
}

/// `{ "data": ... }`: non-paged routes.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DataEnvelope<T> {
    pub data: T,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DataPagination {
    /// Page size this page was served with.
    pub limit: u32,
    /// Running item offset (display only; the cursor drives paging).
    pub offset: u64,
    pub has_more: bool,
    /// Opaque cursor for the next page; `None` on the last page.
    #[serde(deserialize_with = "nullable")]
    pub next_cursor: Option<String>,
}

/// Error body of every Data API route (`ErrorResponse`). Returned as the body of a
/// [`crate::PolymarketApiError::Status`]; parse it with `serde_json::from_str`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DataError {
    pub error: String,
    /// E.g. `invalid_request`, `request_timeout`.
    pub code: String,
    #[serde(default)]
    pub parameter: Option<String>,
    pub retryable: bool,
    pub trace_id: String,
}

/// `BUY` / `SELL`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum DataSide {
    #[serde(rename = "BUY")]
    Buy,
    #[serde(rename = "SELL")]
    Sell,
}

impl DataSide {
    fn as_str(self) -> &'static str {
        match self {
            Self::Buy => "BUY",
            Self::Sell => "SELL",
        }
    }
}

/// `filter_type` of `/v2/trades`: what `filter_amount` is measured in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataTradeFilterType {
    Cash,
    Tokens,
}

impl DataTradeFilterType {
    fn as_str(self) -> &'static str {
        match self {
            Self::Cash => "CASH",
            Self::Tokens => "TOKENS",
        }
    }
}

/// Query for `GET /v2/trades` (market, event or global feed). `None` fields are not sent. The
/// `user` filter is not exposed (wallet feeds are out of scope); `start`/`end` are honoured only
/// on the user shape, so they are not exposed either.
#[derive(Debug, Clone, Default)]
pub struct DataTradesParams {
    /// First-page size, 0..=1000. Ignored when `cursor` is set.
    pub limit: Option<u32>,
    /// `pagination.next_cursor` of the previous page.
    pub cursor: Option<String>,
    /// Exchange default `true` (each fill once, taker side); `false` adds maker rows.
    pub taker_only: Option<bool>,
    pub filter_type: Option<DataTradeFilterType>,
    /// Minimum trade size in `filter_type` units (exchange default 0.01).
    pub filter_amount: Option<f64>,
    /// Condition ids, at most 20; sent comma-joined.
    pub condition: Option<Vec<String>>,
    /// Gamma event ids, at most 20; sent comma-joined.
    pub event_id: Option<Vec<u64>>,
    pub side: Option<DataSide>,
}

impl DataTradesParams {
    pub(crate) fn query(&self) -> Query {
        let mut q = Query::default();
        q.opt("limit", &self.limit)
            .opt("cursor", &self.cursor)
            .opt("taker_only", &self.taker_only)
            .opt(
                "filter_type",
                &self.filter_type.map(DataTradeFilterType::as_str),
            )
            .opt("filter_amount", &self.filter_amount)
            .joined("condition", &self.condition)
            .joined("event_id", &self.event_id)
            .opt("side", &self.side.map(DataSide::as_str));
        q
    }
}

/// One `/v2/trades` row (`Trade`). Profile and enrichment strings are sent as `""` when unset.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DataTrade {
    pub proxy_wallet: String,
    /// From `proxy_wallet`'s perspective.
    pub side: DataSide,
    pub token_id: String,
    pub condition_id: String,
    /// Shares.
    pub size: f64,
    pub price: f64,
    /// Block timestamp, epoch seconds.
    pub timestamp: i64,
    pub title: String,
    pub slug: String,
    pub icon: String,
    pub event_slug: String,
    pub outcome: String,
    /// `999` means the outcome could not be labelled.
    pub outcome_index: u32,
    pub name: String,
    pub pseudonym: String,
    pub bio: String,
    pub profile_image: String,
    pub profile_image_optimized: String,
    pub transaction_hash: String,
}

/// Query for `GET /v2/holders`.
#[derive(Debug, Clone, Default)]
pub struct DataHoldersParams {
    /// Rows per outcome token (exchange default 100, max 1000; 100 with `include_pnl`).
    pub limit: Option<u32>,
    pub cursor: Option<String>,
    /// Minimum net balance in shares.
    pub min_balance: Option<f64>,
    /// Adds the PnL fields to each holder; allows exactly one condition.
    pub include_pnl: Option<bool>,
}

impl DataHoldersParams {
    pub(crate) fn query(&self, condition: &[&str]) -> Query {
        let mut q = Query::default();
        q.req("condition", &condition.join(","))
            .opt("limit", &self.limit)
            .opt("cursor", &self.cursor)
            .opt("min_balance", &self.min_balance)
            .opt("include_pnl", &self.include_pnl);
        q
    }
}

/// Holders of one outcome token (`MetaHolder`).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DataTokenHolders {
    pub token_id: String,
    /// Amount descending.
    pub holders: Vec<DataHolder>,
}

/// One holder (`Holder`). The PnL fields are sent only with `include_pnl=true`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DataHolder {
    pub proxy_wallet: String,
    pub bio: String,
    pub token_id: String,
    pub pseudonym: String,
    /// Shares: net by default, per-side gross with `include_pnl=true`.
    pub amount: f64,
    pub display_username_public: bool,
    pub outcome_index: u32,
    pub name: String,
    pub profile_image: String,
    pub profile_image_optimized: String,
    pub verified: bool,
    #[serde(default)]
    pub avg_price: Option<f64>,
    #[serde(default)]
    pub entry_cost_usdc: Option<f64>,
    #[serde(default)]
    pub current_price: Option<f64>,
    #[serde(default)]
    pub current_value: Option<f64>,
    #[serde(default)]
    pub realized_pnl: Option<f64>,
    #[serde(default)]
    pub unrealized_pnl: Option<f64>,
    #[serde(default)]
    pub total_pnl: Option<f64>,
}

/// One `/v2/oi` row. Without a condition filter the single row has `condition_id == "GLOBAL"`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DataOpenInterest {
    pub condition_id: String,
    /// Priced gross open interest, USDC.
    pub value: f64,
}

/// `/v2/live-volume` payload.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DataLiveVolume {
    pub taker_volume_total: f64,
    /// `taker_volume` descending.
    pub conditions: Vec<DataConditionVolume>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DataConditionVolume {
    pub condition_id: String,
    /// Cumulative taker volume, shares.
    pub taker_volume: f64,
}

/// Window of `GET /v2/prices-history`: exactly one form per request.
#[derive(Debug, Clone, PartialEq)]
pub enum DataPriceWindow {
    /// Epoch seconds; `start` inclusive, `end` exclusive (`None` = up to now). Capped at 15 days.
    Range { start: i64, end: Option<i64> },
    /// Relative preset, sent verbatim: `max`, `all`, `1m`, `1w`, `1d`, `6h`, `1h`.
    Interval(String),
    /// Latest observation at or before this epoch second.
    AsOf(i64),
}

/// Query for `GET /v2/prices-history`.
#[derive(Debug, Clone, Default)]
pub struct DataPricesHistoryParams {
    /// Bucket width in seconds, 60..=86400 (server picks when `None`).
    pub bucket_seconds: Option<u32>,
    /// First-page size (exchange default and max 10,000).
    pub limit: Option<u32>,
    pub cursor: Option<String>,
}

impl DataPricesHistoryParams {
    pub(crate) fn query(&self, token_id: &str, window: &DataPriceWindow) -> Query {
        let mut q = Query::default();
        q.req("token_id", &token_id);
        match window {
            DataPriceWindow::Range { start, end } => {
                q.req("start", start).opt("end", end);
            }
            DataPriceWindow::Interval(i) => {
                q.req("interval", i);
            }
            DataPriceWindow::AsOf(t) => {
                q.req("as_of", t);
            }
        }
        q.opt("bucket_seconds", &self.bucket_seconds)
            .opt("limit", &self.limit)
            .opt("cursor", &self.cursor);
        q
    }
}

/// One `/v2/prices-history` point (`PricePoint`), oldest first.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DataPricePoint {
    /// Observation time, epoch seconds.
    pub timestamp: i64,
    pub price: f64,
    /// Width of the observation window: `0` for an exact tick, else the bucket width.
    pub resolution_seconds: u64,
}

/// Selector of `GET /v2/resolutions` (exactly one per request).
#[derive(Debug, Clone, PartialEq)]
pub enum DataResolutionSelector {
    /// One UMA question id.
    QuestionId(String),
    /// Condition ids, at most 20.
    Conditions(Vec<String>),
    /// Gamma event ids, at most 20.
    EventIds(Vec<u64>),
}

/// One `/v2/resolutions` row (`Resolution`). UMA question-keyed rows carry the proposal fields;
/// condition-keyed (terminal CTF / native) rows carry `market_type`, `payouts`, `resolved_*`.
/// The spec says absent fields are omitted, so the optional ones default to `None`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DataResolution {
    #[serde(default)]
    pub question_id: Option<String>,
    /// Absent on rows fetched by `question_id`.
    #[serde(default)]
    pub condition_id: Option<String>,
    /// `initialized`, `posed`, `proposed`, `challenged`, `reproposed`, `disputed`, `resolved`.
    pub status: String,
    pub extended_review: bool,
    pub was_disputed: bool,
    pub new_version_q: bool,
    /// Numeric strings in UMA units (`1e18` = 1); `"69"` means unset.
    #[serde(default)]
    pub proposed_price: Option<String>,
    #[serde(default)]
    pub reproposed_price: Option<String>,
    #[serde(default)]
    pub price: Option<String>,
    /// Empty on condition-keyed rows without a lifecycle transaction.
    pub transaction_hash: String,
    /// Numeric string; empty when `transaction_hash` is empty.
    pub log_index: String,
    /// Epoch-seconds string on question-keyed rows, RFC 3339 on condition-keyed rows.
    pub last_update_timestamp: String,
    #[serde(default)]
    pub expected_settlement_time: Option<String>,
    #[serde(default)]
    pub settlement_time_basis: Option<String>,
    /// `BINARY`, `INCREMENTAL_NEGRISK`, `ATOMIC_NEGRISK`.
    #[serde(default)]
    pub market_type: Option<String>,
    /// Per-outcome payout in micro-USDC per share.
    #[serde(default)]
    pub payouts: Option<Vec<i64>>,
    /// `UMA_OO`, `CHAINLINK`, `EOA`.
    #[serde(default)]
    pub reporter: Option<String>,
    /// `reported` or `derived`.
    #[serde(default)]
    pub resolution_source: Option<String>,
    #[serde(default)]
    pub resolved_at: Option<String>,
    #[serde(default)]
    pub resolved_block: Option<u64>,
    #[serde(default)]
    pub was_arbitrated: Option<bool>,
}

/// Selector of `GET /v2/tokens` (exactly one per request).
#[derive(Debug, Clone, PartialEq)]
pub enum DataTokenSelector {
    /// Decimal token ids, at most 50.
    TokenIds(Vec<String>),
    /// Condition ids, at most 10.
    Conditions(Vec<String>),
}

/// One `/v2/tokens` row (`TokenRef`). Nullable keys are always sent.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DataToken {
    pub token_id: String,
    pub condition_id: String,
    #[serde(deserialize_with = "nullable")]
    pub structural_condition_id: Option<String>,
    /// Issuing contract family, e.g. `v1_ctf`.
    pub module: String,
    pub outcome_index: u32,
    /// `None` until the market is mirrored.
    #[serde(deserialize_with = "nullable")]
    pub clob_index: Option<u32>,
    #[serde(deserialize_with = "nullable")]
    pub outcome: Option<String>,
    #[serde(deserialize_with = "nullable")]
    pub opposite_token_id: Option<String>,
    pub resolved: bool,
    /// Payout rate once resolved (0 lost, 1 won).
    #[serde(deserialize_with = "nullable")]
    pub final_price: Option<f64>,
    #[serde(deserialize_with = "nullable")]
    pub title: Option<String>,
    #[serde(deserialize_with = "nullable")]
    pub market_slug: Option<String>,
    #[serde(deserialize_with = "nullable")]
    pub closed: Option<bool>,
    #[serde(deserialize_with = "nullable")]
    pub neg_risk: Option<bool>,
    #[serde(deserialize_with = "nullable")]
    pub neg_risk_market_id: Option<String>,
    #[serde(deserialize_with = "nullable")]
    pub question_index: Option<u32>,
    #[serde(deserialize_with = "nullable")]
    pub event_id: Option<u64>,
    #[serde(deserialize_with = "nullable")]
    pub event_slug: Option<String>,
}

/// `/v2/status` payload (`ServiceStatus`): data freshness.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DataStatus {
    /// RFC 3339.
    pub computed_at: String,
    pub age_seconds: i64,
    pub serving: DataServingFreshness,
    pub ingestion: DataIngestionFreshness,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DataServingFreshness {
    #[serde(deserialize_with = "nullable")]
    pub lag_seconds: Option<i64>,
    #[serde(deserialize_with = "nullable")]
    pub worst: Option<String>,
    pub mechanisms: Vec<DataServingMechanism>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DataServingMechanism {
    /// `activity_feed`, `custody_balances`, `pnl`.
    pub name: String,
    pub age_seconds: i64,
    /// Absent for mechanisms without a block clock (live: `activity_feed`).
    #[serde(default)]
    pub blocks_behind: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DataIngestionFreshness {
    pub cursors: u64,
    #[serde(deserialize_with = "nullable")]
    pub network: Option<String>,
    pub chain_id: u64,
    #[serde(deserialize_with = "nullable")]
    pub max_synced_block: Option<u64>,
    #[serde(deserialize_with = "nullable")]
    pub min_synced_block: Option<u64>,
    #[serde(deserialize_with = "nullable")]
    pub most_lagged: Option<DataCursorLag>,
    pub lagging: Vec<DataCursorLag>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DataCursorLag {
    pub source: String,
    pub block: u64,
    pub behind_max: u64,
}
