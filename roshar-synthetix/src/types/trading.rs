//! Account and trading types for `POST /v1/trade`.
//!
//! Query parameter structs (`Get*`) serialize into the signed request's `params`, after
//! `action`, `subaccountId` and `walletAddress`; unset fields are left out. Response structs are
//! built from the docs (developers.synthetix.io, 2026-11-15): there is no live capture without a
//! funded account. Deprecated duplicate fields (`orderId`, `id`, `*OrderId(s)`, `timestamp` /
//! `fundingTimestamp` on funding payments) are not modelled; use the canonical `order` objects.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A read-only `SubAccountAction` query. `ACTION` is `params.action`; `Response` is the type of
/// the envelope's `response`.
pub trait SubAccountQuery: Serialize {
    const ACTION: &'static str;
    type Response: DeserializeOwned;
}

// ── Queries ─────────────────────────────────────────────────────────────────

/// `getOpenOrders`.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetOpenOrders<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_order_id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<u32>,
}

impl SubAccountQuery for GetOpenOrders<'_> {
    const ACTION: &'static str = "getOpenOrders";
    type Response = Vec<OpenOrder>;
}

/// `getOrderHistory`. Time range at most 7 days; `limit` at most 1000.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetOrderHistory<'a> {
    /// e.g. `["filled", "cancelled"]`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<&'a [&'a str]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub side: Option<&'a str>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub order_type: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_time: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_time: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}

impl SubAccountQuery for GetOrderHistory<'_> {
    const ACTION: &'static str = "getOrderHistory";
    type Response = Vec<HistoricalOrder>;
}

/// `getPositions`.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetPositions<'a> {
    /// `open`, `close` or `update`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_time: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_time: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_by: Option<&'a str>,
    /// `asc` or `desc`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_order: Option<&'a str>,
}

impl SubAccountQuery for GetPositions<'_> {
    const ACTION: &'static str = "getPositions";
    type Response = Vec<Position>;
}

/// `getSubAccount`: balances, margin summary, positions, leverage settings, fee rates.
#[derive(Debug, Clone, Default, Serialize)]
pub struct GetSubAccount {}

impl SubAccountQuery for GetSubAccount {
    const ACTION: &'static str = "getSubAccount";
    type Response = SubAccount;
}

/// `getTrades` (fills). With both times set the range is at most 7 days; `limit` at most 1000.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetTrades<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol: Option<&'a str>,
    /// Venue order id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order_id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_time: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_time: Option<u64>,
}

impl SubAccountQuery for GetTrades<'_> {
    const ACTION: &'static str = "getTrades";
    type Response = AccountTrades;
}

/// `getFundingPayments`. Window at most 30 days back; `limit` at most 1000.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetFundingPayments<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_time: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_time: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}

impl SubAccountQuery for GetFundingPayments<'_> {
    const ACTION: &'static str = "getFundingPayments";
    type Response = FundingPayments;
}

/// `getRateLimits`. The docs list `user` (wallet address) as required; the SDK sends none.
#[derive(Debug, Clone, Default, Serialize)]
pub struct GetRateLimits<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<&'a str>,
}

impl SubAccountQuery for GetRateLimits<'_> {
    const ACTION: &'static str = "getRateLimits";
    type Response = RateLimits;
}

/// `getDelegatedSigners`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct GetDelegatedSigners {}

impl SubAccountQuery for GetDelegatedSigners {
    const ACTION: &'static str = "getDelegatedSigners";
    type Response = DelegatedSigners;
}

// ── Shared ──────────────────────────────────────────────────────────────────

/// Canonical order identifier.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderRef {
    pub venue_id: String,
    /// Docs: optional client order id.
    pub client_id: Option<String>,
}

/// Order identifier on a rejected `placeOrders` item: `venueId` is `null`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RejectedOrderRef {
    pub venue_id: Option<String>,
    pub client_id: Option<String>,
}

// ── placeOrders / cancelOrders ──────────────────────────────────────────────

/// `placeOrders` and `cancelOrders` response: one status per submitted item, in request order.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct OrderStatuses {
    pub statuses: Vec<OrderStatus>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum OrderStatus {
    Resting {
        resting: RestingOrder,
    },
    Filled {
        filled: FilledOrder,
    },
    Canceled {
        canceled: CanceledOrder,
    },
    Error {
        error: String,
        #[serde(rename = "errorCode")]
        error_code: String,
        /// Present on `placeOrders` rejections, absent on `cancelOrders` ones.
        order: Option<RejectedOrderRef>,
    },
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestingOrder {
    pub order: OrderRef,
    /// Unix ms; `limitGtd` only.
    pub expires_at: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FilledOrder {
    pub order: OrderRef,
    pub avg_price: String,
    pub total_size: String,
    pub expires_at: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct CanceledOrder {
    pub order: OrderRef,
}

/// `cancelAllOrders` item. `message` is empty on success and holds the failure reason otherwise.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct CancelAllResult {
    pub order: OrderRef,
    pub message: String,
    pub symbol: String,
}

/// `modifyOrder`. `status` is `"modified"` or `"rejected"` (then `error` / `error_code` are set).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModifyOrderResult {
    pub order: OrderRef,
    pub status: String,
    pub price: Option<String>,
    pub quantity: Option<String>,
    pub trigger_price: Option<String>,
    pub cum_qty: Option<String>,
    pub avg_price: Option<String>,
    pub error: Option<String>,
    pub error_code: Option<String>,
    pub timestamp: u64,
}

/// `scheduleCancel`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleCancelResult {
    pub is_active: bool,
    pub message: String,
    pub timeout_seconds: u64,
    /// Absent / `null` when disarmed.
    pub trigger_time: Option<u64>,
}

/// `updateLeverage`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateLeverageResult {
    pub symbol: String,
    pub previous_leverage: String,
    pub new_leverage: String,
}

// ── Orders ──────────────────────────────────────────────────────────────────

/// `getOpenOrders` item.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenOrder {
    pub order: OrderRef,
    pub symbol: String,
    pub side: String,
    /// e.g. `LIMIT`, `MARKET`, `TWAP`.
    #[serde(rename = "type")]
    pub order_type: String,
    pub quantity: String,
    pub price: String,
    /// Docs: omitted for non-conditional orders.
    pub trigger_price: Option<String>,
    pub trigger_price_type: String,
    pub time_in_force: String,
    pub reduce_only: bool,
    pub post_only: bool,
    pub close_position: bool,
    pub created_time: u64,
    pub updated_time: u64,
    pub filled_quantity: String,
    /// Entry orders placed with TP/SL grouping only.
    pub take_profit_order: Option<OrderRef>,
    pub stop_loss_order: Option<OrderRef>,
    /// Unix ms; only for orders with an explicit expiry.
    pub expires_at: Option<u64>,
    /// TWAP parents only.
    pub twap_details: Option<TwapDetails>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TwapDetails {
    pub average_price: String,
    pub interval_ms: u64,
    /// Omitted when zero.
    pub interval_variance_bps: Option<u64>,
    /// Omitted when zero.
    pub size_variance_bps: Option<u64>,
    pub total_trades: u64,
    pub trades_filled: u64,
    pub total_fees: String,
    pub started_at_ms: u64,
    pub total_duration_ms: u64,
}

/// `getOrderHistory` item.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoricalOrder {
    pub order: OrderRef,
    pub symbol: String,
    pub side: String,
    #[serde(rename = "type")]
    pub order_type: String,
    pub status: String,
    pub quantity: String,
    pub price: String,
    pub time_in_force: String,
    pub reduce_only: bool,
    pub post_only: bool,
    pub created_time: u64,
    /// The docs' example uses `updateTime`, the order object table `updatedTime`; both accepted.
    #[serde(alias = "updatedTime")]
    pub update_time: u64,
    pub filled_quantity: String,
    pub filled_price: String,
    pub triggered_by_liquidation: bool,
    /// Documented as optional or absent from the docs' examples:
    pub close_position: Option<bool>,
    pub trigger_price: Option<String>,
    pub trigger_price_type: Option<String>,
    pub expires_at: Option<u64>,
    pub cancel_reason: Option<String>,
    pub cancelled_at: Option<u64>,
    pub tp_order: Option<OrderRef>,
    pub sl_order: Option<OrderRef>,
    #[serde(rename = "cumulative_filled_quantity")]
    pub cumulative_filled_quantity: Option<String>,
    #[serde(rename = "cumulative_filled_value")]
    pub cumulative_filled_value: Option<String>,
    #[serde(rename = "position_id")]
    pub position_id: Option<String>,
    /// Flip trades only.
    pub legs: Option<Vec<PositionTradeLeg>>,
}

/// One leg of a flip trade (snake_case on the wire).
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct PositionTradeLeg {
    pub id: String,
    pub position_id: String,
    /// `close` or `open`.
    pub leg_type: String,
    pub direction: String,
    pub quantity: String,
    pub fee: String,
    /// `close` legs only.
    pub closed_pnl: Option<String>,
    pub entry_price: String,
}

// ── Positions / account ─────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Position {
    /// ADL priority bucket, 1 (lowest) to 5.
    pub adl_bucket: u8,
    pub position_id: String,
    pub sub_account_id: String,
    pub symbol: String,
    /// `long` or `short`.
    pub side: String,
    pub entry_price: String,
    pub quantity: String,
    pub realized_pnl: String,
    pub unrealized_pnl: String,
    pub used_margin: String,
    pub maintenance_margin: String,
    pub liquidation_price: String,
    /// `open`, `close` or `update`.
    pub status: String,
    pub net_funding: String,
    pub take_profit_orders: Vec<OrderRef>,
    pub stop_loss_orders: Vec<OrderRef>,
    pub updated_at: u64,
    pub created_at: u64,
}

/// `getSubAccount`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubAccount {
    pub sub_account_id: String,
    /// `null` for a master account.
    pub master_account_id: Option<String>,
    pub sub_account_name: String,
    pub collaterals: Vec<Collateral>,
    pub cross_margin_summary: CrossMarginSummary,
    pub positions: Vec<Position>,
    pub market_preferences: MarketPreferences,
    pub fee_rates: FeeRates,
    pub account_limits: AccountLimits,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Collateral {
    pub symbol: String,
    pub quantity: String,
    pub withdrawable: String,
    pub pending_withdraw: String,
    pub adjusted_collateral_value: String,
    pub collateral_value: String,
    pub haircut_rate: String,
    pub haircut_adjustment: String,
    pub price: String,
    /// `0` for USDT.
    pub calculated_at: u64,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CrossMarginSummary {
    pub account_value: String,
    pub available_margin: String,
    pub total_unrealized_pnl: String,
    pub maintenance_margin: String,
    pub initial_margin: String,
    pub withdrawable: String,
    pub adjusted_account_value: String,
    pub debt: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct MarketPreferences {
    /// Symbol → leverage multiplier.
    pub leverages: HashMap<String, u32>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeeRates {
    pub maker_fee_rate: String,
    pub taker_fee_rate: String,
    pub tier_name: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountLimits {
    pub max_borrow_capacity: String,
    pub max_orders_per_market: u32,
    pub max_sub_accounts: u32,
    pub max_total_orders: u32,
}

// ── Fills / funding ─────────────────────────────────────────────────────────

/// `getTrades`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountTrades {
    pub trades: Vec<AccountTrade>,
    pub has_more: bool,
    pub total: u64,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountTrade {
    pub trade_id: String,
    pub order: OrderRef,
    pub symbol: String,
    pub side: String,
    /// e.g. `open long`, `close short`.
    pub direction: String,
    /// `limit`, `market`, `stopMarket`, `takeProfitMarket`, `stopLimit`, `takeProfitLimit`.
    pub order_type: String,
    pub price: String,
    pub quantity: String,
    pub realized_pnl: String,
    pub fee: String,
    pub fee_rate: String,
    pub mark_price: String,
    pub entry_price: String,
    pub timestamp: u64,
    pub maker: bool,
    pub reduce_only: bool,
    pub triggered_by_liquidation: bool,
    pub post_only: bool,
}

/// `getFundingPayments`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FundingPayments {
    pub summary: FundingSummary,
    pub funding_history: Vec<FundingPayment>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FundingSummary {
    pub total_funding_received: String,
    pub total_funding_paid: String,
    pub net_funding: String,
    pub total_payments: String,
    pub average_payment_size: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FundingPayment {
    pub payment_id: String,
    pub symbol: String,
    /// Signed.
    pub position_size: String,
    pub funding_rate: String,
    /// Negative = paid.
    pub payment: String,
    pub payment_time: u64,
    pub funding_time: u64,
}

/// `getRateLimits`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RateLimits {
    pub requests_used: u64,
    pub requests_cap: u64,
}

// ── Delegation ──────────────────────────────────────────────────────────────

/// `getDelegatedSigners`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DelegatedSigners {
    pub delegated_signers: Vec<DelegatedSigner>,
}

/// `getDelegatedSigners` item and `addDelegatedSigner` response.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DelegatedSigner {
    pub sub_account_id: String,
    pub wallet_address: String,
    /// `["session"]` or `["delegate"]`.
    pub permissions: Vec<String>,
    /// Unix ms; `null` = no expiry.
    pub expires_at: Option<u64>,
    /// Docs: omitted for pre-migration records.
    pub added_by: Option<String>,
}

/// `removeDelegatedSigner`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemovedDelegatedSigner {
    pub sub_account_id: String,
    pub wallet_address: String,
    /// Docs: omitted when empty.
    pub cascade_removed_signers: Option<Vec<String>>,
}

/// `removeAllDelegatedSigners`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemovedAllDelegatedSigners {
    pub sub_account_id: String,
    pub removed_signers: Vec<String>,
}

// ── Trade websocket ─────────────────────────────────────────────────────────

/// `subAccountUpdates` push: `{"channel":"subAccountUpdate","data":{"eventType":…},…}`. `data` is
/// left as JSON: the docs list 15 event types and ask clients to tolerate new ones.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubAccountUpdate {
    pub channel: String,
    pub data: serde_json::Value,
    pub timestamp: u64,
    /// Monotonic per subaccount stream (docs: optional provenance field).
    pub seq: Option<u64>,
    pub meseq: Option<u64>,
    pub met: Option<String>,
    pub provenance_missing: Option<bool>,
}

impl SubAccountUpdate {
    /// `data.eventType`, e.g. `orderPlaced`, `trade`, `marginUpdate`.
    pub fn event_type(&self) -> Option<&str> {
        self.data.get("eventType").and_then(|v| v.as_str())
    }
}
