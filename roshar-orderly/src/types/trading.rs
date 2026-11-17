//! Private REST responses, typed from the OpenAPI schemas in Orderly's docs (no live account was
//! available to capture them). Fields the schema marks required are required; fields it marks
//! nullable, or that only apply to some products/modes, are `Option`.

use super::OrderlySymbolMap;
use serde::{Deserialize, Serialize};

/// `data` of `POST /v1/order` and each row of `POST /v1/batch-order`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyOrderAck {
    pub order_id: u64,
    pub client_order_id: Option<String>,
    pub order_type: String,
    pub order_price: Option<f64>,
    pub order_quantity: Option<f64>,
    pub order_amount: Option<f64>,
    pub error_message: Option<String>,
}

/// `data` of `POST /v1/batch-order`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyBatchOrderAck {
    pub rows: Vec<OrderlyOrderAck>,
}

/// `data` of the cancel endpoints and `PUT /v1/order`: e.g. `CANCEL_SENT`, `CANCEL_ALL_SENT`,
/// `EDIT_SENT`. The outcome arrives on the `executionreport` stream.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyStatusAck {
    pub status: String,
}

/// `data` of `POST /v1/order/cancel_all_after`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyCancelAllAfterAck {
    /// Unix ms; absent when the timer is cleared.
    pub expected_trigger_time: Option<u64>,
}

/// An order from `GET /v1/orders`, `/v1/order/{id}`, `/v1/client/order/{cid}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyOrder {
    pub order_id: u64,
    pub user_id: u64,
    pub symbol: String,
    pub side: String,
    #[serde(rename = "type")]
    pub order_type: String,
    /// `NEW`, `PARTIAL_FILLED`, `FILLED`, `CANCELLED`, `REJECTED`.
    pub status: String,
    pub price: f64,
    pub quantity: f64,
    /// Quote amount of amount-sized orders; `null` otherwise.
    pub amount: Option<f64>,
    pub executed_quantity: Option<f64>,
    pub total_executed_quantity: f64,
    pub visible_quantity: f64,
    pub average_executed_price: f64,
    pub total_fee: f64,
    pub fee_asset: String,
    pub client_order_id: Option<String>,
    pub realized_pnl: f64,
    pub created_time: u64,
    pub updated_time: u64,
    pub reduce_only: Option<bool>,
    pub order_tag: Option<String>,
    pub margin_mode: Option<String>,
}

/// A fill from `GET /v1/trades`, `/v1/trade/{id}`, `/v1/order/{id}/trades`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyFill {
    pub id: u64,
    pub symbol: String,
    pub order_id: u64,
    pub side: String,
    pub executed_price: f64,
    pub executed_quantity: f64,
    pub executed_timestamp: u64,
    /// Total fee, including `order_enum_fee`. Base units when buying, quote when selling.
    pub fee: f64,
    pub fee_asset: String,
    /// `1` maker, `0` taker.
    pub is_maker: u8,
    pub realized_pnl: Option<f64>,
    pub match_id: Option<u64>,
    pub margin_token: Option<String>,
    pub margin_change_qty: Option<f64>,
    pub margin_mode: Option<String>,
    pub order_enum_fee: Option<f64>,
    pub order_enum_fee_rate: Option<f64>,
}

/// `data` of `GET /v1/order/{id}/trades`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyFillRows {
    pub rows: Vec<OrderlyFill>,
}

/// One position from `GET /v1/positions` or `GET /v1/position/{symbol}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyPosition {
    pub symbol: String,
    /// Signed: negative is short.
    pub position_qty: f64,
    pub cost_position: f64,
    pub last_sum_unitary_funding: f64,
    pub pending_long_qty: f64,
    pub pending_short_qty: f64,
    pub settle_price: f64,
    pub average_open_price: f64,
    pub unsettled_pnl: f64,
    pub mark_price: f64,
    pub est_liq_price: f64,
    pub imr: f64,
    pub mmr: f64,
    #[serde(rename = "IMR_withdraw_orders")]
    pub imr_withdraw_orders: f64,
    #[serde(rename = "MMR_with_orders")]
    pub mmr_with_orders: f64,
    pub pnl_24_h: f64,
    pub fee_24_h: f64,
    /// Position creation time.
    pub timestamp: u64,
    pub updated_time: Option<u64>,
    pub seq: Option<u64>,
    pub leverage: Option<u32>,
    pub margin_mode: Option<String>,
    /// Isolated positions.
    pub margin: Option<f64>,
    pub accrued_funding_fee: Option<f64>,
}

/// `data` of `GET /v1/positions`: account margin state plus every position.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyPositions {
    pub current_margin_ratio_with_orders: f64,
    pub free_collateral: f64,
    pub initial_margin_ratio: f64,
    pub initial_margin_ratio_with_orders: f64,
    pub maintenance_margin_ratio: f64,
    pub maintenance_margin_ratio_with_orders: f64,
    pub margin_ratio: f64,
    pub open_margin_ratio: f64,
    pub total_collateral_value: f64,
    pub total_pnl_24_h: f64,
    pub account_value: Option<f64>,
    pub rows: Vec<OrderlyPosition>,
}

/// One token from `GET /v1/client/holding`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyHolding {
    pub token: String,
    pub holding: f64,
    pub frozen: f64,
    pub pending_short: f64,
    pub updated_time: u64,
    pub isolated_margin: Option<f64>,
    pub isolated_order_frozen: Option<f64>,
}

/// `data` of `GET /v1/client/holding`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyHoldings {
    pub holding: Vec<OrderlyHolding>,
}

/// `GET /v1/client/info`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyClientInfo {
    pub account_id: String,
    pub email: String,
    /// e.g. `FUTURES`.
    pub account_mode: String,
    pub max_leverage: f64,
    pub taker_fee_rate: f64,
    pub maker_fee_rate: f64,
    pub futures_taker_fee_rate: f64,
    pub futures_maker_fee_rate: f64,
    /// Predict markets, bps.
    pub binary_taker_fee_rate: f64,
    /// Predict markets, bps.
    pub binary_maker_fee_rate: f64,
    pub rwa_taker_fee_rate: Option<f64>,
    pub rwa_maker_fee_rate: Option<f64>,
    pub maintenance_cancel_orders: bool,
    pub imr_factor: OrderlySymbolMap,
    pub max_notional: Option<OrderlySymbolMap>,
}

/// `GET /v1/account_info`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyAccountOverview {
    pub current_margin_ratio_with_orders: f64,
    pub free_collateral: f64,
    pub initial_margin_ratio: f64,
    pub initial_margin_ratio_with_orders: f64,
    pub maintenance_margin_ratio: f64,
    pub maintenance_margin_ratio_with_orders: f64,
    pub margin_ratio: f64,
    pub open_margin_ratio: f64,
    pub total_collateral_value: f64,
    pub account_value: f64,
    pub total_pnl_24h: f64,
}

/// Row of `GET /v1/funding_fee/history`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyFundingFee {
    pub symbol: String,
    pub funding_rate: f64,
    pub mark_price: f64,
    pub funding_fee: f64,
    /// `Pay` / `Receive`.
    pub payment_type: String,
    /// `Accrued` / `Settled`.
    pub status: String,
    pub created_time: u64,
    pub updated_time: u64,
    pub margin_mode: Option<String>,
}

/// Row of `GET /v1/client/key_info`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyKeyInfo {
    pub orderly_key: String,
    /// `ACTIVE`, `REMOVING`, `REMOVED`.
    pub key_status: String,
    pub scope: String,
    pub expiration: u64,
    pub ip_restriction_list: Option<Vec<String>>,
    /// `ALLOW_RESTRICTION_LIST`, `ALLOW_ALL_IPS`, `DISALLOW_ALL_IPS`.
    pub ip_restriction_status: Option<String>,
    pub tag: Option<String>,
}

/// `data` of `GET /v1/client/key_info`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyKeyInfoRows {
    pub rows: Vec<OrderlyKeyInfo>,
}

// ── Private websocket pushes ─────────────────────────────────────────────────
//
// Typed from the documented example messages (`tests/fixtures/docs_ws_*.json`); no live account
// was available to capture them.

/// `executionreport` push: one order state change, or one fill (`trade_id != 0`,
/// `executed_quantity > 0`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyWsExecutionReport {
    pub symbol: String,
    /// `""` when the order has none.
    #[serde(rename = "clientOrderId")]
    pub client_order_id: String,
    #[serde(rename = "orderId")]
    pub order_id: u64,
    #[serde(rename = "type")]
    pub order_type: String,
    pub side: String,
    pub quantity: f64,
    pub price: f64,
    /// `0` for non-fill updates.
    #[serde(rename = "tradeId")]
    pub trade_id: u64,
    /// This fill's price; `0` for non-fill updates.
    #[serde(rename = "executedPrice")]
    pub executed_price: f64,
    /// This fill's quantity; `0` for non-fill updates.
    #[serde(rename = "executedQuantity")]
    pub executed_quantity: f64,
    pub fee: f64,
    #[serde(rename = "feeAsset")]
    pub fee_asset: String,
    #[serde(rename = "totalExecutedQuantity")]
    pub total_executed_quantity: f64,
    #[serde(rename = "avgExecutedPrice")]
    pub avg_executed_price: f64,
    /// `NEW`, `PARTIAL_FILLED`, `FILLED`, `CANCELLED`, `REJECTED`, ...
    pub status: String,
    pub reason: String,
    #[serde(rename = "totalFee")]
    pub total_fee: f64,
    #[serde(rename = "visibleQuantity")]
    pub visible_quantity: f64,
    pub timestamp: u64,
    #[serde(rename = "marginMode")]
    pub margin_mode: Option<String>,
    pub maker: bool,
    /// Decimal string.
    pub match_id: Option<String>,
    pub seq: Option<u64>,
}

/// One entry of the `position` push.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyWsPosition {
    pub symbol: String,
    #[serde(rename = "positionQty")]
    pub position_qty: f64,
    #[serde(rename = "costPosition")]
    pub cost_position: f64,
    #[serde(rename = "lastSumUnitaryFunding")]
    pub last_sum_unitary_funding: f64,
    #[serde(rename = "pendingLongQty")]
    pub pending_long_qty: f64,
    #[serde(rename = "pendingShortQty")]
    pub pending_short_qty: f64,
    #[serde(rename = "settlePrice")]
    pub settle_price: f64,
    #[serde(rename = "averageOpenPrice")]
    pub average_open_price: f64,
    #[serde(rename = "unsettledPnl")]
    pub unsettled_pnl: f64,
    #[serde(rename = "markPrice")]
    pub mark_price: f64,
    #[serde(rename = "estLiqPrice")]
    pub est_liq_price: f64,
    pub imr: f64,
    pub mmr: f64,
    pub timestamp: u64,
    #[serde(rename = "pnl24H")]
    pub pnl_24h: Option<f64>,
    #[serde(rename = "fee24H")]
    pub fee_24h: Option<f64>,
    pub version: Option<u64>,
    pub leverage: Option<u32>,
    #[serde(rename = "marginMode")]
    pub margin_mode: Option<String>,
    pub margin: Option<f64>,
    #[serde(rename = "accruedFundingFee")]
    pub accrued_funding_fee: Option<f64>,
}

/// `data` of the `position` push.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyWsPositions {
    pub positions: Vec<OrderlyWsPosition>,
}

/// One token of the `balance` push.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyWsBalance {
    pub holding: f64,
    pub frozen: f64,
    #[serde(rename = "pendingShortQty")]
    pub pending_short_qty: f64,
    #[serde(rename = "pendingLongQty")]
    pub pending_long_qty: Option<f64>,
    pub version: u64,
    #[serde(rename = "isolatedMargin")]
    pub isolated_margin: Option<f64>,
    #[serde(rename = "isolatedOrderFrozen")]
    pub isolated_order_frozen: Option<f64>,
}

/// `data` of the `balance` push, keyed by token.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderlyWsBalances {
    pub balances: std::collections::HashMap<String, OrderlyWsBalance>,
}
