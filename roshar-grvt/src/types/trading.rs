//! Trading API types (`trades.grvt.io/full/v1/*`, `edge.grvt.io/auth/*`). Built from the docs:
//! no live private responses were captured (no account).

use super::{ser_opt_u64_str, GrvtKind, GrvtPageQuery, GrvtVenue};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GrvtTimeInForce {
    GoodTillTime,
    /// Block trades / RFQ maker only.
    AllOrNone,
    ImmediateOrCancel,
    FillOrKill,
    /// GTT + post-only, takeable only by UI users. Not signable with the SDK's signing table.
    RetailPriceImprovement,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GrvtOrderStatus {
    Pending,
    Open,
    Filled,
    Rejected,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GrvtPositionMarginType {
    Isolated,
    Cross,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GrvtMarginType {
    SimpleCrossMargin,
    PortfolioCrossMargin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GrvtTriggerType {
    Unspecified,
    TakeProfit,
    StopLoss,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GrvtTriggerBy {
    Unspecified,
    Index,
    Last,
    Mid,
    Mark,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GrvtCancelStatus {
    Expired,
    DroppedDuplicate,
}

// ---------------------------------------------------------------------------------------------
// Auth
// ---------------------------------------------------------------------------------------------

/// `POST /auth/api_key/login` and `/auth/wallet/login` success body.
#[derive(Debug, Clone, Deserialize)]
pub struct GrvtLoginResponse {
    pub status: String,
    pub location: String,
    pub funding_account_address: String,
    /// Present only when the API key was generated from a trading account.
    #[serde(default)]
    pub sub_account_id: Option<String>,
}

/// Login failure body, e.g. `{"status":"failure","error":"Invalid Credentials"}` (HTTP 401).
#[derive(Debug, Clone, Deserialize)]
pub struct GrvtLoginFailure {
    pub status: String,
    pub error: String,
}

/// Session returned by a login. Send `cookie` as the `Cookie` header and `account_id` as
/// `X-Grvt-Account-Id` on every trading request and on the trading websocket upgrade.
/// The caller refreshes the session before `cookie_expires`.
#[derive(Debug, Clone)]
pub struct GrvtSession {
    /// `gravity=<value>`.
    pub cookie: String,
    /// `Expires` attribute of the `gravity` cookie, as sent (e.g. `Sun, 16 Nov 2026 22:00:00 GMT`).
    pub cookie_expires: String,
    /// `X-Grvt-Account-Id` response header.
    pub account_id: String,
    pub body: GrvtLoginResponse,
}

impl GrvtSession {
    /// Headers for authenticated REST calls and the trading websocket upgrade.
    pub fn headers(&self) -> Vec<(String, String)> {
        vec![
            ("Cookie".to_string(), self.cookie.clone()),
            ("X-Grvt-Account-Id".to_string(), self.account_id.clone()),
        ]
    }
}

/// `Signature` as sent. Orders omit `chain_id` (as the SDK does); wallet login sends it.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct GrvtSignatureRequest {
    pub signer: String,
    pub r: String,
    pub s: String,
    pub v: u8,
    /// Unix nanoseconds.
    pub expiration: String,
    pub nonce: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chain_id: Option<String>,
}

/// `POST /auth/wallet/login` body; build with `Signer::sign_wallet_login`.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct GrvtWalletLoginRequest {
    pub address: String,
    pub signature: GrvtSignatureRequest,
}

// ---------------------------------------------------------------------------------------------
// Requests
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct GrvtOrderLegRequest {
    pub instrument: String,
    pub size: String,
    pub is_buying_asset: bool,
    pub limit_price: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct GrvtOrderMetadataRequest {
    pub client_order_id: String,
}

/// Signed `Order` as sent to `create_order` / `bulk_orders`; build with `Signer::sign_order`.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct GrvtOrderRequest {
    pub sub_account_id: String,
    pub time_in_force: GrvtTimeInForce,
    pub legs: Vec<GrvtOrderLegRequest>,
    pub signature: GrvtSignatureRequest,
    pub metadata: GrvtOrderMetadataRequest,
    pub is_market: bool,
    pub post_only: bool,
    pub reduce_only: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub builder: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub builder_fee: Option<String>,
}

/// `full/v1/create_order` body.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct GrvtCreateOrderRequest {
    pub order: GrvtOrderRequest,
}

/// `full/v1/cancel_order` body. Set exactly one of `order_id` / `client_order_id`.
#[derive(Debug, Clone, Serialize)]
pub struct GrvtCancelOrderRequest {
    pub sub_account_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_order_id: Option<String>,
    /// Milliseconds during which a later create with the same `client_order_id` is dropped
    /// (exchange default 100, rounded down to 100 ms, capped at 5000).
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "ser_opt_u64_str"
    )]
    pub time_to_live_ms: Option<u64>,
}

/// Kind / base / quote filter shared by several endpoints; unset means all.
#[derive(Debug, Clone, Default, Serialize)]
pub struct GrvtAssetFilter {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<Vec<GrvtKind>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quote: Option<Vec<String>>,
}

/// `full/v1/cancel_all_orders` body.
#[derive(Debug, Clone, Serialize)]
pub struct GrvtCancelAllOrdersRequest {
    pub sub_account_id: String,
    #[serde(flatten)]
    pub filter: GrvtAssetFilter,
}

/// `full/v1/order` body. Set exactly one of `order_id` / `client_order_id`.
#[derive(Debug, Clone, Serialize)]
pub struct GrvtGetOrderRequest {
    pub sub_account_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_order_id: Option<String>,
}

/// `full/v1/open_orders` and `full/v1/positions` body.
#[derive(Debug, Clone, Serialize)]
pub struct GrvtFilteredRequest {
    pub sub_account_id: String,
    #[serde(flatten)]
    pub filter: GrvtAssetFilter,
}

/// `full/v1/order_history` and `full/v1/fill_history` body.
#[derive(Debug, Clone, Serialize)]
pub struct GrvtHistoryRequest {
    pub sub_account_id: String,
    #[serde(flatten)]
    pub filter: GrvtAssetFilter,
    #[serde(flatten)]
    pub page: GrvtPageQuery,
}

/// `full/v1/funding_payment_history` body.
#[derive(Debug, Clone, Serialize)]
pub struct GrvtFundingPaymentHistoryRequest {
    pub sub_account_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instrument: Option<String>,
    #[serde(flatten)]
    pub filter: GrvtAssetFilter,
    #[serde(flatten)]
    pub page: GrvtPageQuery,
}

/// `full/v1/cancel_on_disconnect` body.
#[derive(Debug, Clone, Serialize)]
pub struct GrvtCancelOnDisconnectRequest {
    pub sub_account_id: String,
    /// Milliseconds, 1000..=300000; 0 disables the timer. Unset uses the exchange default (1000).
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "ser_opt_u64_str"
    )]
    pub countdown_time: Option<u64>,
}

/// `full/v2/bulk_orders` body: create (orders only), cancel (ids only) or cancel-replace (both).
/// `order_ids` and `client_order_ids` are mutually exclusive; up to 100 entries each.
/// Wire names are the documented `order_i_ds` / `client_order_i_ds`.
#[derive(Debug, Clone, Serialize)]
pub struct GrvtBulkOrdersRequest {
    pub sub_account_id: String,
    pub orders: Vec<GrvtOrderRequest>,
    #[serde(rename = "order_i_ds")]
    pub order_ids: Vec<String>,
    #[serde(rename = "client_order_i_ds")]
    pub client_order_ids: Vec<String>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "ser_opt_u64_str"
    )]
    pub time_to_live_ms: Option<u64>,
}

/// `{"sub_account_id"}` body (`full/v1/account_summary`).
#[derive(Debug, Clone, Serialize)]
pub struct GrvtSubAccountRequest {
    pub sub_account_id: String,
}

// ---------------------------------------------------------------------------------------------
// Responses
// ---------------------------------------------------------------------------------------------

/// `Ack`.
#[derive(Debug, Clone, Deserialize)]
pub struct GrvtAck {
    pub ack: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GrvtOrderLeg {
    pub instrument: String,
    pub size: String,
    /// Documented optional (default `0`, market orders).
    #[serde(default)]
    pub limit_price: Option<String>,
    pub is_buying_asset: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GrvtSignature {
    pub signer: String,
    pub r: String,
    pub s: String,
    pub v: u8,
    pub expiration: String,
    pub nonce: u32,
    /// Documented, but the SDK never sends it; optional until seen live.
    #[serde(default)]
    pub chain_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GrvtTpslMetadata {
    pub trigger_by: GrvtTriggerBy,
    pub trigger_price: String,
    pub close_position: bool,
    pub is_split_position: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GrvtTriggerMetadata {
    pub trigger_type: GrvtTriggerType,
    pub tpsl: GrvtTpslMetadata,
}

/// `OrderMetadata`. Optional fields carry their documented defaults.
#[derive(Debug, Clone, Deserialize)]
pub struct GrvtOrderMetadata {
    pub client_order_id: String,
    #[serde(default)]
    pub create_time: Option<String>,
    #[serde(default)]
    pub trigger: Option<GrvtTriggerMetadata>,
    /// `BrokerTag`.
    #[serde(default)]
    pub broker: Option<String>,
    #[serde(default)]
    pub is_ecn: bool,
    #[serde(default)]
    pub slippage_bps: u32,
    #[serde(default)]
    pub rfq_id: Option<String>,
    #[serde(default)]
    pub is_private: bool,
}

/// `OrderState`. Per-leg vectors follow `Order.legs`.
#[derive(Debug, Clone, Deserialize)]
pub struct GrvtOrderState {
    pub status: GrvtOrderStatus,
    /// `OrderRejectReason` (`UNSPECIFIED` unless rejected or cancelled; ~60 values).
    pub reject_reason: String,
    pub book_size: Vec<String>,
    pub traded_size: Vec<String>,
    pub update_time: String,
    /// Absent from the create-order reply recorded in the SDK (`grvt_ccxt_ws.py`).
    #[serde(default)]
    pub avg_fill_price: Vec<String>,
}

/// `Order` as returned by the exchange.
#[derive(Debug, Clone, Deserialize)]
pub struct GrvtOrder {
    /// Documented optional (filled by the backend).
    #[serde(default)]
    pub order_id: Option<String>,
    pub sub_account_id: String,
    #[serde(default)]
    pub is_market: bool,
    pub time_in_force: GrvtTimeInForce,
    #[serde(default)]
    pub post_only: bool,
    #[serde(default)]
    pub reduce_only: bool,
    pub legs: Vec<GrvtOrderLeg>,
    pub signature: GrvtSignature,
    pub metadata: GrvtOrderMetadata,
    /// Documented optional (filled by the backend).
    #[serde(default)]
    pub state: Option<GrvtOrderState>,
    /// Optional in the SDK's `Order`; set only for builder orders.
    #[serde(default)]
    pub builder: Option<String>,
    #[serde(default)]
    pub builder_fee: Option<String>,
}

/// `full/v2/bulk_orders` response.
#[derive(Debug, Clone, Deserialize)]
pub struct GrvtBulkOrdersResponse {
    pub orders: Vec<GrvtOrder>,
    pub cancel_acks: Vec<GrvtAck>,
}

/// Private `Fill`.
#[derive(Debug, Clone, Deserialize)]
pub struct GrvtFill {
    pub event_time: String,
    pub sub_account_id: String,
    pub instrument: String,
    pub is_buyer: bool,
    pub is_taker: bool,
    pub size: String,
    pub price: String,
    #[serde(default)]
    pub mark_price: Option<String>,
    pub index_price: String,
    pub interest_rate: String,
    #[serde(default)]
    pub forward_price: Option<String>,
    pub realized_pnl: String,
    pub fee: String,
    pub fee_rate: String,
    pub trade_id: String,
    pub order_id: String,
    pub venue: GrvtVenue,
    pub is_liquidation: bool,
    pub client_order_id: String,
    pub signer: String,
    /// `BrokerTag`, documented optional.
    #[serde(default)]
    pub broker: Option<String>,
    pub is_rpi: bool,
    pub builder: String,
    pub builder_fee_rate: String,
    pub builder_fee: String,
    pub fee_currency: String,
    /// `RepaymentScenario`.
    pub repayment_scenario: String,
    #[serde(default)]
    pub rfq_id: Option<String>,
    /// `RfqCounterpartyType`, RFQ fills only.
    #[serde(default)]
    pub counterparty_type: Option<String>,
}

/// `Positions`.
#[derive(Debug, Clone, Deserialize)]
pub struct GrvtPosition {
    pub event_time: String,
    pub sub_account_id: String,
    pub instrument: String,
    /// Negative for shorts.
    pub size: String,
    pub notional: String,
    pub entry_price: String,
    pub exit_price: String,
    pub mark_price: String,
    pub unrealized_pnl: String,
    pub realized_pnl: String,
    pub total_pnl: String,
    pub roi: String,
    pub quote_index_price: String,
    pub est_liquidation_price: String,
    pub leverage: String,
    pub cumulative_fee: String,
    pub cumulative_realized_funding_payment: String,
    pub margin_type: GrvtPositionMarginType,
    #[serde(default)]
    pub isolated_balance: Option<String>,
    #[serde(default)]
    pub isolated_im: Option<String>,
    #[serde(default)]
    pub isolated_mm: Option<String>,
}

/// `SpotBalance`.
#[derive(Debug, Clone, Deserialize)]
pub struct GrvtSpotBalance {
    pub currency: String,
    pub balance: String,
    pub index_price: String,
    pub entry_price: String,
    pub realized_pnl: String,
    pub unrealized_pnl: String,
    pub available_to_transfer: String,
}

/// `SubAccount` (`full/v1/account_summary`).
#[derive(Debug, Clone, Deserialize)]
pub struct GrvtSubAccount {
    pub event_time: String,
    pub sub_account_id: String,
    pub margin_type: GrvtMarginType,
    pub settle_currency: String,
    pub unrealized_pnl: String,
    pub total_equity: String,
    pub initial_margin: String,
    pub maintenance_margin: String,
    /// Deprecated by the exchange; use `spot_balances[..].available_to_transfer`.
    pub available_balance: String,
    pub spot_balances: Vec<GrvtSpotBalance>,
    pub positions: Vec<GrvtPosition>,
    pub settle_index_price: String,
    #[serde(default)]
    pub is_vault: Option<bool>,
    #[serde(default)]
    pub vault_im_additions: Option<String>,
    pub derisk_margin: String,
    pub derisk_to_maintenance_margin_ratio: String,
    pub total_cross_equity: String,
    pub cross_unrealized_pnl: String,
    /// `SubAccountMode` (`SINGLE_ASSET_MODE`).
    pub sub_account_mode: String,
    pub margin_balance: String,
}

/// `FundingPayment`. `amount` is positive when paid, negative when received.
#[derive(Debug, Clone, Deserialize)]
pub struct GrvtFundingPayment {
    pub event_time: String,
    pub sub_account_id: String,
    pub instrument: String,
    pub currency: String,
    pub amount: String,
    pub tx_id: String,
}

/// `v1.state` feed payload.
#[derive(Debug, Clone, Deserialize)]
pub struct GrvtOrderStateFeed {
    pub order_id: String,
    pub client_order_id: String,
    pub order_state: GrvtOrderState,
}

/// `v1.cancel` feed payload (TTL expiry of cancel requests).
#[derive(Debug, Clone, Deserialize)]
pub struct GrvtCancelStatusFeed {
    pub sub_account_id: String,
    pub client_order_id: String,
    pub order_id: String,
    /// `OrderRejectReason`.
    pub reason: String,
    #[serde(default)]
    pub update_time: Option<String>,
    pub cancel_status: GrvtCancelStatus,
}
