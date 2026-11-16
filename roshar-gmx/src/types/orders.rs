//! Order transaction workflow of the GMX API (`/v2/orders/txns/*`): prepare, sign, submit, status.
//!
//! The API builds every order: `prepare` returns EIP-712 typed data (express mode, relayed by
//! GMX Relay) or an `ExchangeRouter.multicall` transaction (classic mode). Express typed data is
//! signed with [`crate::signer`] and sent back with `submit`, echoing the prepared `batchParams`
//! and `relayParams` unchanged.

use super::market::{GmxDirection, GmxTradingCapacity};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// `TransactionMode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum GmxTxMode {
    /// Signed EIP-712 intent, submitted on-chain by GMX Relay. Fees are paid in tokens.
    Express,
    /// Raw `ExchangeRouter` transaction the account sends itself.
    Classic,
}

/// `OrderKind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum GmxOrderKind {
    Increase,
    Decrease,
    Swap,
}

/// `SimpleOrderType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum GmxOrderType {
    Market,
    Limit,
    StopMarket,
    TakeProfit,
    StopLoss,
    Twap,
}

/// `{token, amount}`: `token` is a symbol (e.g. `"USDC"`), `amount` in token decimals.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GmxTokenAmount {
    pub token: String,
    pub amount: String,
}

/// TWAP split of an order. `parts` 2..=30, `duration` seconds.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GmxTwapConfig {
    pub duration: u64,
    pub parts: u32,
    /// Published in the request type; the production API derives the schedule from
    /// `duration` and `parts`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frequency: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum GmxTpslType {
    TakeProfit,
    StopLoss,
}

/// Take-profit / stop-loss sidecar order of an increase (`tpsl[]`).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GmxTpsl {
    #[serde(rename = "type")]
    pub kind: GmxTpslType,
    /// 30-decimal USD.
    pub trigger_price: String,
    /// 30-decimal USD; `None` covers the whole position.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<String>,
}

/// Body of `POST /v2/orders/txns/prepare` (`PrepareRequest`).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GmxPrepareOrderRequest {
    pub kind: GmxOrderKind,
    /// Market symbol, e.g. `"ETH/USD [WETH-USDC]"`.
    pub symbol: String,
    /// Required for increase and decrease.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direction: Option<GmxDirection>,
    pub order_type: GmxOrderType,
    /// 30-decimal USD for positions; token decimals for swaps.
    pub size: String,
    /// 30-decimal USD. Required for limit and conditional orders.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trigger_price: Option<String>,
    /// Acceptable price deviation in basis points.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slippage: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acceptable_price_impact_bps: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution_fee_buffer_bps: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skip_execution_fee_priority_fee_allowance: Option<bool>,
    /// Collateral token symbol of the position.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub collateral_token: Option<String>,
    /// Required for increase and swap.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub collateral_to_pay: Option<GmxTokenAmount>,
    /// Token symbol received by a decrease or swap.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub receive_token: Option<String>,
    /// Decrease only: withdraw collateral proportionally.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keep_leverage: Option<bool>,
    /// Market addresses overriding the automatic swap path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manual_swap_path: Option<Vec<String>>,
    /// Token symbol paying the relay fee (express).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gas_payment_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub twap_config: Option<GmxTwapConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tpsl: Option<Vec<GmxTpsl>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_cancel: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ui_fee_receiver: Option<String>,
    /// Registered code or encoded bytes32.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub referral_code: Option<String>,
    pub mode: GmxTxMode,
    /// Account that owns the order (and sends the transaction in classic mode).
    pub from: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subaccount_address: Option<String>,
    /// Express subaccount approval payload; required with `subaccount_address`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subaccount_approval: Option<Value>,
}

impl GmxPrepareOrderRequest {
    /// Request with only the always-required fields set; fill in the rest as needed.
    pub fn new(
        kind: GmxOrderKind,
        symbol: impl Into<String>,
        order_type: GmxOrderType,
        size: impl Into<String>,
        mode: GmxTxMode,
        from: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            symbol: symbol.into(),
            direction: None,
            order_type,
            size: size.into(),
            trigger_price: None,
            slippage: None,
            acceptable_price_impact_bps: None,
            execution_fee_buffer_bps: None,
            skip_execution_fee_priority_fee_allowance: None,
            collateral_token: None,
            collateral_to_pay: None,
            receive_token: None,
            keep_leverage: None,
            manual_swap_path: None,
            gas_payment_token: None,
            twap_config: None,
            tpsl: None,
            auto_cancel: None,
            ui_fee_receiver: None,
            referral_code: None,
            mode,
            from: from.into(),
            subaccount_address: None,
            subaccount_approval: None,
        }
    }
}

/// Body of `POST /v2/orders/txns/edit/prepare` (`EditPrepareRequest`).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GmxEditOrderRequest {
    /// Order keys (bytes32).
    pub order_ids: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_size: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_trigger_price: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_acceptable_price: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_auto_cancel: Option<bool>,
    /// Native-token wei added to the execution fee; needed to unfreeze a frozen order.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution_fee_top_up: Option<String>,
    pub mode: GmxTxMode,
    pub from: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subaccount_address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subaccount_approval: Option<Value>,
}

/// Body of `POST /v2/orders/txns/cancel/prepare` (`CancelPrepareRequest`).
/// Set `order_ids`, or `all: Some(true)` to cancel every open order of `from`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GmxCancelOrderRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order_ids: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub all: Option<bool>,
    pub mode: GmxTxMode,
    pub from: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subaccount_address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subaccount_approval: Option<Value>,
}

/// `OrderEstimates` of a prepare response. 30-decimal USD except `execution_fee_amount`
/// (native-token wei). `size_delta_usd` is the effective size after full-close adjustment.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GmxOrderEstimates {
    pub position_price_impact_delta_usd: String,
    pub swap_price_impact_delta_usd: String,
    pub execution_fee_amount: String,
    pub acceptable_price: String,
    pub size_delta_usd: String,
    pub position_fee_usd: String,
    pub borrowing_fee_usd: String,
    pub funding_fee_usd: String,
    #[serde(default)]
    pub trading_capacity: Option<GmxTradingCapacity>,
}

/// Non-blocking structured warning, e.g. `INSUFFICIENT_LIQUIDITY` or
/// `TRADING_CAPACITY_UNAVAILABLE`. `details` depends on `code`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GmxValidationWarning {
    pub code: String,
    pub message: String,
    #[serde(default)]
    pub details: Option<Value>,
}

/// One field of an EIP-712 struct type.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct GmxTypedField {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
}

/// EIP-712 domain of GMX relay routers: name `GmxBaseGelatoRelayRouter`, version `1`.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GmxEip712Domain {
    pub name: String,
    pub version: String,
    pub chain_id: u64,
    pub verifying_contract: String,
}

/// EIP-712 typed data as returned by `prepare` (`payload.typedData`). There is no
/// `primaryType`: the primary type is the one struct no other struct references.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GmxTypedData {
    pub domain: GmxEip712Domain,
    pub types: BTreeMap<String, Vec<GmxTypedField>>,
    pub message: Value,
}

/// Express payload: typed data to sign, plus relay data echoed back on submit.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GmxExpressPayload {
    pub typed_data: GmxTypedData,
    /// Opaque; send back unchanged in `eip712Data.relayParams`. Its hash is
    /// `typed_data.message.relayParams`.
    pub relay_params: Value,
    /// Opaque; send back unchanged in `eip712Data.batchParams`.
    pub batch_params: Value,
    /// Gas payment and relay fee token details (informational).
    pub gas_payment_params: Value,
    /// The relay router the typed data is bound to (`domain.verifyingContract`).
    pub relay_router_address: String,
    pub gas_limit: String,
    pub gas_price: String,
    /// Native-token wei.
    pub execution_fee_amount: String,
    /// Relay fee, in native-token wei.
    pub relayer_fee_amount: String,
}

/// Classic payload: an `ExchangeRouter.multicall` transaction for `from` to sign and send.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GmxClassicPayload {
    pub to: String,
    pub data: String,
    /// Native-token wei to attach (execution fee); absent means zero.
    #[serde(default)]
    pub value: Option<String>,
    pub gas_limit: String,
}

/// `payload` of a prepare response, selected by `payloadType`.
#[derive(Debug, Clone)]
pub enum GmxPreparedPayload {
    /// `payloadType: "typed-data"`.
    TypedData(GmxExpressPayload),
    /// `payloadType: "transaction"`.
    Transaction(GmxClassicPayload),
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawPrepared {
    request_id: String,
    mode: GmxTxMode,
    payload_type: String,
    payload: Value,
    #[serde(default)]
    estimates: Option<GmxOrderEstimates>,
    #[serde(default)]
    expires_at: Option<u64>,
    #[serde(default)]
    warnings: Option<Vec<String>>,
    #[serde(default)]
    validation_warnings: Option<Vec<GmxValidationWarning>>,
    #[serde(default)]
    idempotency_key: Option<String>,
    #[serde(default)]
    trace_id: Option<String>,
}

/// Response of every `.../prepare` route (`PrepareResponse`).
#[derive(Debug, Clone, Deserialize)]
#[serde(try_from = "RawPrepared")]
pub struct GmxPreparedOrder {
    /// Persist it: submit and status use it, and only the host that served prepare knows it.
    pub request_id: String,
    pub mode: GmxTxMode,
    pub payload: GmxPreparedPayload,
    /// Absent on cancel prepares.
    pub estimates: Option<GmxOrderEstimates>,
    /// Unix seconds after which the prepared payload is rejected.
    pub expires_at: Option<u64>,
    pub warnings: Option<Vec<String>>,
    pub validation_warnings: Option<Vec<GmxValidationWarning>>,
    pub idempotency_key: Option<String>,
    pub trace_id: Option<String>,
}

impl TryFrom<RawPrepared> for GmxPreparedOrder {
    type Error = String;

    fn try_from(raw: RawPrepared) -> Result<Self, Self::Error> {
        let payload = match raw.payload_type.as_str() {
            "typed-data" => GmxPreparedPayload::TypedData(
                serde_json::from_value(raw.payload)
                    .map_err(|e| format!("typed-data payload: {e}"))?,
            ),
            "transaction" => GmxPreparedPayload::Transaction(
                serde_json::from_value(raw.payload)
                    .map_err(|e| format!("transaction payload: {e}"))?,
            ),
            other => return Err(format!("unknown payloadType {other:?}")),
        };
        Ok(Self {
            request_id: raw.request_id,
            mode: raw.mode,
            payload,
            estimates: raw.estimates,
            expires_at: raw.expires_at,
            warnings: raw.warnings,
            validation_warnings: raw.validation_warnings,
            idempotency_key: raw.idempotency_key,
            trace_id: raw.trace_id,
        })
    }
}

impl GmxPreparedOrder {
    /// The express payload, or `None` for a classic transaction.
    pub fn express(&self) -> Option<&GmxExpressPayload> {
        match &self.payload {
            GmxPreparedPayload::TypedData(p) => Some(p),
            GmxPreparedPayload::Transaction(_) => None,
        }
    }

    /// The classic transaction, or `None` for an express payload.
    pub fn classic(&self) -> Option<&GmxClassicPayload> {
        match &self.payload {
            GmxPreparedPayload::Transaction(p) => Some(p),
            GmxPreparedPayload::TypedData(_) => None,
        }
    }
}

/// `eip712Data` of a submit request.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GmxEip712Data {
    pub batch_params: Value,
    pub relay_params: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subaccount_approval: Option<Value>,
}

/// Body of `POST /v2/orders/txns/submit` (`SubmitRequest`).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GmxSubmitOrderRequest {
    pub mode: GmxTxMode,
    pub request_id: String,
    /// 65-byte `r || s || v` hex signature of the prepared typed data.
    pub signature: String,
    /// The account that owns the order (the main account for subaccount orders).
    pub from: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
    pub eip712_data: GmxEip712Data,
}

impl GmxSubmitOrderRequest {
    /// Submit body for an express prepare response, echoing its relay data as the SDK does.
    /// `subaccount_approval` must be the approval passed to prepare, if any.
    /// Returns `None` for a classic prepare response.
    pub fn express(
        prepared: &GmxPreparedOrder,
        signature: String,
        from: String,
        subaccount_approval: Option<Value>,
    ) -> Option<Self> {
        let payload = prepared.express()?;
        Some(Self {
            mode: prepared.mode,
            request_id: prepared.request_id.clone(),
            signature,
            from,
            idempotency_key: prepared.idempotency_key.clone(),
            eip712_data: GmxEip712Data {
                batch_params: payload.batch_params.clone(),
                relay_params: payload.relay_params.clone(),
                subaccount_approval,
            },
        })
    }
}

/// `OrderTransactionStatus`. Terminal: `Executed`, `Cancelled`, `RelayFailed`, `RelayReverted`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GmxOrderTxStatus {
    Prepared,
    RelayAccepted,
    RelayPending,
    RelaySubmitted,
    RelayUnknown,
    Created,
    Executed,
    Cancelled,
    RelayFailed,
    RelayReverted,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GmxTxError {
    pub code: String,
    pub message: String,
}

/// Response of `POST /v2/orders/txns/submit` (`SubmitResponse`).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GmxSubmitResponse {
    pub request_id: String,
    pub status: GmxOrderTxStatus,
    #[serde(default)]
    pub tx_hash: Option<String>,
    #[serde(default)]
    pub task_id: Option<String>,
    #[serde(default)]
    pub error: Option<GmxTxError>,
    #[serde(default)]
    pub trace_id: Option<String>,
}

/// Response of `POST /v2/orders/txns/status` (`StatusResponse`).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GmxOrderStatus {
    pub request_id: String,
    pub status: GmxOrderTxStatus,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub tx_hash: Option<String>,
    #[serde(default)]
    pub created_txn_hash: Option<String>,
    #[serde(default)]
    pub execution_txn_hash: Option<String>,
    #[serde(default)]
    pub order_keys: Option<Vec<String>>,
    #[serde(default)]
    pub cancellation_reason: Option<String>,
    #[serde(default)]
    pub task_id: Option<String>,
    #[serde(default)]
    pub error: Option<GmxTxError>,
    #[serde(default)]
    pub trace_id: Option<String>,
}
