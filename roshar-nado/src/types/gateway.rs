//! Gateway (`/v1/query`, `/v1/execute`) reply types.
//!
//! x18 values are fixed-point integers scaled by 1e18 (`I128`). Amounts are normalised to 18
//! decimals regardless of the token's on-chain decimals.

use super::num::{I128, U128, U64};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// `status` of a gateway reply.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NadoReplyStatus {
    Success,
    Failure,
}

/// Envelope of every gateway query reply:
/// `{"status", "data"?, "error"?, "error_code"?, "request_type"}`.
#[derive(Debug, Clone, Deserialize)]
pub struct NadoQueryReply<T> {
    pub status: NadoReplyStatus,
    #[serde(default = "Option::default")]
    pub data: Option<T>,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub error_code: Option<i64>,
    pub request_type: String,
}

/// Envelope of every execute reply:
/// `{"status", "signature", "data"?, "error"?, "error_code"?, "request_type", "id"?}`.
#[derive(Debug, Clone, Deserialize)]
pub struct NadoExecuteReply<T> {
    pub status: NadoReplyStatus,
    /// Echo of the request's signature.
    pub signature: String,
    #[serde(default = "Option::default")]
    pub data: Option<T>,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub error_code: Option<i64>,
    pub request_type: String,
    /// Client id, echoed when the request carried one.
    #[serde(default)]
    pub id: Option<u64>,
}

/// `contracts` query: chain id and endpoint contract used as the EIP-712 verifying contract for
/// every execute except `place_order`.
#[derive(Debug, Clone, Deserialize)]
pub struct NadoContracts {
    pub chain_id: U64,
    pub endpoint_addr: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NadoProductType {
    Spot,
    Perp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NadoTradingStatus {
    Live,
    PostOnly,
    ReduceOnly,
    SoftReduceOnly,
    NotTradable,
}

/// One entry of the `symbols` query.
#[derive(Debug, Clone, Deserialize)]
pub struct NadoSymbol {
    #[serde(rename = "type")]
    pub product_type: NadoProductType,
    pub product_id: u32,
    pub symbol: String,
    pub price_increment_x18: I128,
    /// Base units, x18.
    pub size_increment: I128,
    /// Minimum order notional in USDT0, x18.
    pub min_size: I128,
    pub maker_fee_rate_x18: I128,
    pub taker_fee_rate_x18: I128,
    pub long_weight_initial_x18: I128,
    pub long_weight_maintenance_x18: I128,
    /// `null` means no cap.
    pub max_open_interest_x18: Option<I128>,
    pub trading_status: NadoTradingStatus,
    pub isolated_only: bool,
}

/// `symbols` query data, keyed by symbol.
#[derive(Debug, Clone, Deserialize)]
pub struct NadoSymbols {
    pub symbols: HashMap<String, NadoSymbol>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NadoRisk {
    pub long_weight_initial_x18: I128,
    pub short_weight_initial_x18: I128,
    pub long_weight_maintenance_x18: I128,
    pub short_weight_maintenance_x18: I128,
    pub price_x18: I128,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NadoSpotConfig {
    pub token: String,
    pub interest_inflection_util_x18: I128,
    pub interest_floor_x18: I128,
    pub interest_small_cap_x18: I128,
    pub interest_large_cap_x18: I128,
    pub withdraw_fee_x18: I128,
    pub min_deposit_rate_x18: I128,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NadoSpotState {
    pub cumulative_deposits_multiplier_x18: I128,
    pub cumulative_borrows_multiplier_x18: I128,
    pub total_deposits_normalized: I128,
    pub total_borrows_normalized: I128,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NadoPerpState {
    pub cumulative_funding_long_x18: I128,
    pub cumulative_funding_short_x18: I128,
    pub available_settle: I128,
    pub open_interest: I128,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NadoBookInfo {
    pub size_increment: I128,
    pub price_increment_x18: I128,
    /// Minimum order notional in USDT0, x18.
    pub min_size: I128,
    pub collected_fees: I128,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NadoSpotProduct {
    pub product_id: u32,
    pub oracle_price_x18: I128,
    pub risk: NadoRisk,
    pub config: NadoSpotConfig,
    pub state: NadoSpotState,
    pub book_info: NadoBookInfo,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NadoPerpProduct {
    pub product_id: u32,
    pub oracle_price_x18: I128,
    pub risk: NadoRisk,
    pub state: NadoPerpState,
    pub book_info: NadoBookInfo,
}

/// `all_products` query data.
#[derive(Debug, Clone, Deserialize)]
pub struct NadoAllProducts {
    pub spot_products: Vec<NadoSpotProduct>,
    pub perp_products: Vec<NadoPerpProduct>,
}

/// `market_liquidity` query data. Levels are `[price_x18, size_x18]`; bids best first, asks best
/// first.
#[derive(Debug, Clone, Deserialize)]
pub struct NadoMarketLiquidity {
    pub bids: Vec<[I128; 2]>,
    pub asks: Vec<[I128; 2]>,
    /// Nanoseconds.
    pub timestamp: U64,
    pub product_id: u32,
}

/// `market_price` query data (top of book).
#[derive(Debug, Clone, Deserialize)]
pub struct NadoMarketPrice {
    pub product_id: u32,
    pub bid_x18: I128,
    pub ask_x18: I128,
}

/// `market_prices` query data.
#[derive(Debug, Clone, Deserialize)]
pub struct NadoMarketPrices {
    pub market_prices: Vec<NadoMarketPrice>,
}

/// `nonces` query data.
#[derive(Debug, Clone, Deserialize)]
pub struct NadoNonces {
    /// Next sequential nonce for non-order executes (e.g. `link_signer`).
    pub tx_nonce: U64,
    /// Suggested order nonce: `(now_ms + 100_000) << 20`.
    pub order_nonce: U64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NadoOrderType {
    Default,
    Ioc,
    Fok,
    PostOnly,
}

/// An open order (`order`, `subaccount_orders`, `orders` queries; cancel replies).
#[derive(Debug, Clone, Deserialize)]
pub struct NadoOrder {
    pub product_id: u32,
    pub sender: String,
    pub price_x18: I128,
    /// Original size, x18. Positive buys, negative sells.
    pub amount: I128,
    /// Unix seconds.
    pub expiration: U64,
    pub order_type: NadoOrderType,
    pub nonce: U64,
    pub appendix: U128,
    /// Remaining size, x18, signed like `amount`.
    pub unfilled_amount: I128,
    pub digest: String,
    /// Unix seconds.
    pub placed_at: u64,
    /// Client id given at placement; absent when none was given.
    #[serde(default)]
    pub id: Option<u64>,
}

/// `subaccount_orders` query data.
#[derive(Debug, Clone, Deserialize)]
pub struct NadoSubaccountOrders {
    pub sender: String,
    pub product_id: u32,
    pub orders: Vec<NadoOrder>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NadoProductOrders {
    pub product_id: u32,
    pub orders: Vec<NadoOrder>,
}

/// `orders` (multi-product) query data.
#[derive(Debug, Clone, Deserialize)]
pub struct NadoMultiProductOrders {
    pub sender: String,
    pub product_orders: Vec<NadoProductOrders>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NadoHealth {
    pub assets: I128,
    pub liabilities: I128,
    pub health: I128,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NadoSpotBalance {
    /// Positive deposit, negative borrow.
    pub amount: I128,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NadoPerpBalance {
    /// Position size, x18. Positive long, negative short.
    pub amount: I128,
    pub v_quote_balance: I128,
    pub last_cumulative_funding_x18: I128,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NadoSpotBalanceEntry {
    pub product_id: u32,
    pub balance: NadoSpotBalance,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NadoPerpBalanceEntry {
    pub product_id: u32,
    pub balance: NadoPerpBalance,
}

/// `subaccount_info` query data: balances, positions and health.
#[derive(Debug, Clone, Deserialize)]
pub struct NadoSubaccountInfo {
    pub subaccount: String,
    pub exists: bool,
    /// `[initial, maintenance, unweighted]`.
    pub healths: Vec<NadoHealth>,
    /// Indexed by product id; each entry is `[initial, maintenance, unweighted]`.
    pub health_contributions: Vec<[I128; 3]>,
    pub spot_count: u32,
    pub perp_count: u32,
    pub spot_balances: Vec<NadoSpotBalanceEntry>,
    pub perp_balances: Vec<NadoPerpBalanceEntry>,
    pub spot_products: Vec<NadoSpotProduct>,
    pub perp_products: Vec<NadoPerpProduct>,
}

/// `fee_rates` query data. Arrays are indexed by product id.
#[derive(Debug, Clone, Deserialize)]
pub struct NadoFeeRates {
    pub taker_fee_rates_x18: Vec<I128>,
    pub maker_fee_rates_x18: Vec<I128>,
    pub liquidation_sequencer_fee: I128,
    pub health_check_sequencer_fee: I128,
    pub taker_sequencer_fee: I128,
    pub withdraw_sequencer_fees: Vec<I128>,
    /// Not in the docs; present in live replies.
    pub fee_tier: u32,
}

/// `linked_signer` query data. The zero address when none is linked.
#[derive(Debug, Clone, Deserialize)]
pub struct NadoLinkedSigner {
    pub linked_signer: String,
}

/// `max_order_size` query data.
#[derive(Debug, Clone, Deserialize)]
pub struct NadoMaxOrderSize {
    /// Base units, x18.
    pub max_order_size: I128,
}

/// `place_order` / `cancel_and_place` execute data.
#[derive(Debug, Clone, Deserialize)]
pub struct NadoPlacedOrder {
    pub digest: String,
}

/// One result of a `place_orders` batch: `digest` on success, `error` on failure.
#[derive(Debug, Clone, Deserialize)]
pub struct NadoPlaceOrdersItem {
    #[serde(default)]
    pub digest: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
}

/// `place_orders` execute data.
#[derive(Debug, Clone, Deserialize)]
pub struct NadoPlacedOrders {
    pub place_orders: Vec<NadoPlaceOrdersItem>,
}

/// `cancel_orders` / `cancel_product_orders` execute data.
#[derive(Debug, Clone, Deserialize)]
pub struct NadoCancelledOrders {
    pub cancelled_orders: Vec<NadoOrder>,
}

/// `edge_all_products` query data (and `cached_edge_all_products`): `all_products` per chain id
/// (as a string).
#[derive(Debug, Clone, Deserialize)]
pub struct NadoEdgeAllProducts {
    pub edge_all_products: HashMap<String, NadoAllProducts>,
}

/// `health_groups` query data. Each pair is `[spot_product_id, perp_product_id]`; `0` stands in
/// for a missing side.
#[derive(Debug, Clone, Deserialize)]
pub struct NadoHealthGroups {
    pub health_groups: Vec<[u32; 2]>,
}

/// `insurance` query data: insurance fund value in USDT0, x18.
#[derive(Debug, Clone, Deserialize)]
pub struct NadoInsurance {
    pub insurance: I128,
}

/// Edge `cached_bbo_history` request. Every field is optional; the server defaults are
/// `interval_ms` 500 (≥ 500, rounded up to a multiple of 500) and `limit` 500 (max 3600).
#[derive(Debug, Clone, Default, Serialize)]
pub struct NadoBboHistoryQuery {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product_ids: Option<Vec<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interval_ms: Option<u64>,
    /// Epoch milliseconds; only samples at or before this time.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_time_ms: Option<u64>,
    /// Samples per product.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NadoBboSample {
    pub product_id: u32,
    /// Epoch milliseconds.
    pub timestamp: U64,
    pub bid: I128,
    pub ask: I128,
}

/// Edge `cached_bbo_history` data, ascending by `(timestamp, product_id)`.
#[derive(Debug, Clone, Deserialize)]
pub struct NadoBboHistory {
    /// Effective sampling stride.
    pub interval_ms: u64,
    /// Effective per-product cap.
    pub limit: u32,
    pub history: Vec<NadoBboSample>,
}

/// Flat reply of the edge `ping` / `time` control messages:
/// `{"status","method","id"?,"server_time","client_time"?}`.
#[derive(Debug, Clone, Deserialize)]
pub struct NadoEdgeTime {
    /// `pong` or `time`.
    pub method: String,
    #[serde(default)]
    pub id: Option<u64>,
    /// Epoch milliseconds.
    pub server_time: U64,
    /// Echo of the `ping` request's `client_time`.
    #[serde(default)]
    pub client_time: Option<U64>,
}
