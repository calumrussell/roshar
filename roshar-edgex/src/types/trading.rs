//! Private REST request bodies, queries and replies.
//!
//! The reply records follow the V2 docs' examples; no live private replies were captured (no
//! account). Fields that every documented example of a record carries are required; fields that
//! some examples omit are `Option`.

use super::{EdgexOrderStatus, EdgexOrderType, EdgexSide, EdgexTimeInForce};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// `POST /api/v2/private/order/createOrder` body, field for field as the SDK sends it. Build it
/// with `L2Signer::create_order_body`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateOrderBody {
    pub account_id: String,
    pub contract_id: String,
    pub price: String,
    pub size: String,
    #[serde(rename = "type")]
    pub order_type: EdgexOrderType,
    pub side: EdgexSide,
    pub time_in_force: EdgexTimeInForce,
    pub client_order_id: String,
    pub expire_time: String,
    pub l2_nonce: String,
    pub l2_signature: String,
    pub l2_expire_time: String,
    pub l2_value: String,
    pub l2_size: String,
    pub l2_limit_fee: String,
    pub reduce_only: bool,
    /// Empty unless conditional.
    pub trigger_price: String,
    /// Empty unless conditional.
    pub trigger_price_type: String,
    pub source_key: String,
    pub is_position_tpsl: bool,
    pub open_tpsl_parent_order_id: String,
    pub is_set_open_tp: bool,
    pub open_tp: Option<Value>,
    pub is_set_open_sl: bool,
    pub open_sl: Option<Value>,
    pub extra_type: String,
    pub extra_data_json: String,
}

/// `POST /api/v2/private/account/setMarginMode` body. Build it with
/// `L2Signer::set_margin_mode_body`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetMarginModeBody {
    pub account_id: String,
    pub contract_id: String,
    pub margin_mode: String,
    pub l2_nonce: String,
    pub l2_expire_time: String,
    pub signer: String,
    pub l2_signature: String,
}

/// `POST /api/v2/private/order/cancelAllOrder` filters. `None` omits the field; an empty list is
/// sent as `[]`. All `None` cancels every active order.
#[derive(Debug, Clone, Default)]
pub struct CancelAllFilter {
    pub coin_ids: Option<Vec<String>>,
    pub contract_ids: Option<Vec<String>>,
    pub order_types: Option<Vec<EdgexOrderType>>,
    pub order_statuses: Option<Vec<EdgexOrderStatus>>,
    pub is_position_tpsl: Option<Vec<bool>>,
}

/// Filters shared by `getActiveOrderPage` (size ≤ 200) and `getHistoryOrderPage` (size ≤ 100).
/// `order_ids` applies to history only.
#[derive(Debug, Clone, Default)]
pub struct OrderPageQuery {
    pub size: Option<u32>,
    pub offset_data: Option<String>,
    pub coin_ids: Vec<String>,
    pub contract_ids: Vec<String>,
    pub types: Vec<EdgexOrderType>,
    pub statuses: Vec<EdgexOrderStatus>,
    pub is_liquidate: Option<bool>,
    pub is_deleverage: Option<bool>,
    pub is_position_tpsl: Option<bool>,
    pub start_created_time_inclusive: Option<u64>,
    pub end_created_time_exclusive: Option<u64>,
    pub order_ids: Vec<String>,
}

/// `getHistoryOrderFillTransactionPage` filters (size ≤ 100).
#[derive(Debug, Clone, Default)]
pub struct FillPageQuery {
    pub size: Option<u32>,
    pub offset_data: Option<String>,
    pub coin_ids: Vec<String>,
    pub contract_ids: Vec<String>,
    pub order_ids: Vec<String>,
    pub is_liquidate: Option<bool>,
    pub is_deleverage: Option<bool>,
    pub is_position_tpsl: Option<bool>,
    pub start_created_time_inclusive: Option<u64>,
    pub end_created_time_exclusive: Option<u64>,
}

/// `createOrder` reply data.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EdgexCreateOrderResult {
    pub order_id: String,
}

/// `cancelOrderById` / `cancelOrderByClientOrderId` / `cancelAllOrder` reply data: id → result
/// (`"SUCCESS"` or a reason).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EdgexCancelResult {
    pub cancel_result_map: HashMap<String, String>,
}

/// `getMaxCreateOrderSize` reply data.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EdgexMaxOrderSize {
    pub max_buy_size: String,
    pub max_sell_size: String,
    pub ask1_price: String,
    pub bid1_price: String,
}

/// An order record (`getOrderById`, `getActiveOrderPage`, `getHistoryOrderPage`, WS `order`).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EdgexOrder {
    pub id: String,
    pub user_id: Option<String>,
    pub account_id: Option<String>,
    pub coin_id: Option<String>,
    pub contract_id: String,
    pub side: String,
    pub price: Option<String>,
    pub size: Option<String>,
    pub client_order_id: Option<String>,
    #[serde(rename = "type")]
    pub order_type: String,
    pub time_in_force: Option<String>,
    pub reduce_only: Option<bool>,
    pub trigger_price: Option<String>,
    pub trigger_price_type: Option<String>,
    pub expire_time: Option<String>,
    pub status: String,
    pub cum_fill_size: Option<String>,
    pub cum_fill_value: Option<String>,
    pub cum_fill_fee: Option<String>,
    /// Websocket order records carry `cumMatch*` instead of `cumFill*`.
    pub cum_match_size: Option<String>,
    pub cum_match_value: Option<String>,
    pub cum_match_fee: Option<String>,
    pub created_time: Option<String>,
    pub updated_time: String,
}

/// A fill (`getHistoryOrderFillTransactionPage`).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EdgexOrderFill {
    pub id: String,
    pub user_id: String,
    pub account_id: String,
    pub coin_id: String,
    pub contract_id: String,
    pub order_id: String,
    pub order_side: String,
    pub fill_size: String,
    pub fill_value: String,
    pub fill_fee: String,
    pub fill_price: String,
    pub liquidate_fee: String,
    pub realize_pnl: String,
    /// `MAKER` or `TAKER`.
    pub direction: String,
    pub match_time: String,
    pub created_time: String,
    pub updated_time: String,
}

/// Account record (`getAccountById`, `getAccountAsset().account`, WS `account`).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EdgexAccount {
    pub id: String,
    pub user_id: String,
    pub client_account_id: String,
    pub status: String,
    pub is_liquidating: Option<bool>,
    pub created_time: Option<String>,
    pub updated_time: String,
}

/// Collateral balance (`getAccountAsset().collateralList`, `getCollateralByCoinId`, WS
/// `collateral`).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EdgexCollateral {
    pub user_id: Option<String>,
    pub account_id: Option<String>,
    pub coin_id: String,
    pub amount: String,
    pub cum_deposit_amount: Option<String>,
    pub cum_withdraw_amount: Option<String>,
    pub cum_transfer_in_amount: Option<String>,
    pub cum_transfer_out_amount: Option<String>,
    pub cum_position_buy_amount: Option<String>,
    pub cum_position_sell_amount: Option<String>,
    pub cum_fill_fee_amount: Option<String>,
    pub cum_funding_fee_amount: Option<String>,
    pub created_time: Option<String>,
    pub updated_time: String,
}

/// Position (`getAccountAsset().positionList`, `getPositionByContractId`, WS `position`).
/// `openSize` is signed: negative is short.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EdgexPosition {
    pub user_id: Option<String>,
    pub account_id: Option<String>,
    pub coin_id: Option<String>,
    pub contract_id: String,
    pub open_size: String,
    pub open_value: String,
    pub open_fee: Option<String>,
    pub funding_fee: String,
    pub created_time: Option<String>,
    pub updated_time: String,
}

/// `getAccountAsset().positionAssetList[]`: per-position risk.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EdgexPositionAsset {
    pub user_id: String,
    pub account_id: String,
    pub coin_id: String,
    pub contract_id: String,
    pub position_value: String,
    pub max_leverage: String,
    pub initial_margin_requirement: String,
    pub risk_rate: String,
    pub risk_value: String,
    pub avg_entry_price: String,
    pub liquidate_price: String,
    pub bankrupt_price: String,
    pub worst_close_price: String,
    pub unrealize_pnl: String,
    pub term_realize_pnl: String,
    pub total_realize_pnl: String,
}

/// `getAccountAsset().collateralAssetModelList[]`: equity and available balance.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EdgexCollateralAsset {
    pub user_id: String,
    pub account_id: String,
    pub coin_id: String,
    pub total_equity: String,
    pub total_position_value_abs: String,
    pub initial_margin_requirement: String,
    pub risk_value: String,
    pub pending_withdraw_amount: String,
    pub pending_transfer_out_amount: String,
    pub order_frozen_amount: String,
    pub available_amount: String,
}

/// `GET /api/v2/private/account/getAccountAsset`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EdgexAccountAsset {
    pub account: EdgexAccount,
    pub collateral_list: Vec<EdgexCollateral>,
    pub position_list: Vec<EdgexPosition>,
    pub version: String,
    pub position_asset_list: Vec<EdgexPositionAsset>,
    pub collateral_asset_model_list: Vec<EdgexCollateralAsset>,
    pub oracle_price_list: Vec<Value>,
}
