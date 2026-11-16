//! Private REST payloads (`/v1/account/*`, `/v1/trade/*`) and the private channel payloads they
//! share with the WebSocket.

use serde::Deserialize;

use super::{Side, TimeInForce};

/// Builder fee attribution attached to an order or fill.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Builder {
    pub address: String,
    /// Decimal fraction of executed notional.
    pub fee_rate: String,
}

/// One element of a `createOrders` response (REST and WebSocket). Acceptance only: read the order
/// state for fills.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "status", rename_all = "lowercase")]
pub enum OrderAck {
    Ok {
        oid: u64,
        coid: Option<String>,
        /// WebSocket acks only.
        ts: Option<i64>,
        arts: Option<i64>,
        builder: Option<Builder>,
    },
    /// A single element without `oid`/`coid` rejects the whole request.
    Err {
        error: String,
        oid: Option<u64>,
        coid: Option<String>,
        ts: Option<i64>,
        arts: Option<i64>,
        #[serde(rename = "ref")]
        reference: Option<String>,
    },
}

/// One element of a `cancelOrders` / `cancelOrdersCOID` response.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "status", rename_all = "lowercase")]
pub enum CancelResult {
    Ok {
        oid: u64,
        coid: Option<String>,
        /// When the exchange decided the outcome.
        ts: i64,
    },
    /// Per-order rejections carry `oid` or `coid` and `ts`; a single element without either is a
    /// request-level rejection.
    Err {
        error: String,
        oid: Option<u64>,
        coid: Option<String>,
        ts: Option<i64>,
        arts: Option<i64>,
        #[serde(rename = "ref")]
        reference: Option<String>,
    },
}

/// Compact order state: modify results and the private `orders` channel.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct OrderUpdate {
    pub oid: u64,
    pub iid: u32,
    pub buy: bool,
    pub p: String,
    pub qty: String,
    pub tif: TimeInForce,
    pub po: bool,
    pub ro: bool,
    /// Resting quantity.
    pub rest: String,
    /// Filled quantity.
    pub fill: String,
    pub cts: i64,
    pub uts: i64,
    /// e.g. `open`, `filled`, `cancelled`, `ioc_no_fill`, `ioc_expired`, `stp_cancelled`.
    pub status: String,
    pub coid: Option<String>,
    pub builder: Option<Builder>,
}

/// One element of a `modifyOrders` / `modifyOrdersCOID` response.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "status", rename_all = "lowercase")]
pub enum ModifyResult {
    Ok {
        order: OrderUpdate,
    },
    Err {
        error: String,
        oid: Option<u64>,
        coid: Option<String>,
        ts: Option<i64>,
        arts: Option<i64>,
        #[serde(rename = "ref")]
        reference: Option<String>,
    },
}

/// `{"status":"ok"}` or `{"status":"err","error":..}`: cancel-all, isolated margin, delete proxy
/// and the WebSocket acks of those ops.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "status", rename_all = "lowercase")]
pub enum Ack {
    Ok,
    Err { error: String },
}

/// `PATCH /v1/trade/auto-cancel` response.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "status", rename_all = "lowercase")]
pub enum AutoCancelResult {
    /// The armed deadline (Unix ms), `0` when cleared.
    Ok {
        deadline: i64,
    },
    Err {
        error: String,
    },
}

/// `PATCH /v1/trade/leverage` response and one element of the batch response.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "status", rename_all = "lowercase")]
pub enum LeverageResult {
    Ok {
        instrument_id: u32,
        leverage: u32,
        cross: bool,
    },
    /// Batch items carry `instrument_id`; request-level rejections do not.
    Err {
        instrument_id: Option<u32>,
        error: String,
    },
}

/// `POST /v1/account/proxy` response.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "status", rename_all = "lowercase")]
pub enum CreateProxyResult {
    /// `secret` authenticates private REST reads and the WebSocket.
    Ok {
        secret: String,
    },
    Err {
        error: String,
    },
}

/// `GET /v1/account/credentials`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Credentials {
    /// Account owner address.
    pub address: String,
    pub keys: Vec<ProxyKey>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct ProxyKey {
    pub proxy: String,
    pub label: Option<String>,
    pub expiry: i64,
}

/// `GET /v1/account/orders` and `GET /v1/account/open-orders`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Order {
    pub order_id: u64,
    pub instrument_id: u32,
    pub buy: bool,
    pub price: String,
    pub quantity: String,
    pub tif: TimeInForce,
    pub post_only: bool,
    pub ro: bool,
    pub resting_quantity: String,
    pub filled_quantity: String,
    pub status: String,
    pub created_timestamp: i64,
    pub updated_timestamp: i64,
    pub client_order_id: Option<String>,
    /// Present on take-profit / stop-loss orders.
    pub tpsl: Option<TpSlFields>,
    pub builder: Option<Builder>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct TpSlFields {
    /// `tp` or `sl`.
    pub kind: String,
    /// `order` or `position`.
    pub scope: String,
    /// Trigger price.
    pub trp: String,
    pub parent_oid: Option<u64>,
    pub armed_qty: Option<String>,
    pub slip_bps: Option<u32>,
}

/// `GET /v1/account/balances` and the private `balances` channel.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Balance {
    pub asset: String,
    pub balance: String,
    /// USD value.
    pub value: String,
}

/// `GET /v1/account/portfolio` and the private `portfolio` channel.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Portfolio {
    pub positions: Vec<Position>,
    pub margin: MarginSummary,
    pub withdrawable: String,
    pub in_liquidation: bool,
    /// Index into the `tiers` of `/v1/info/fees`.
    pub fee_tier: u32,
    pub timestamp: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Position {
    pub instrument_id: u32,
    pub symbol: String,
    /// Signed contracts: positive long, negative short.
    pub size: String,
    pub entry_price: String,
    pub leverage: u32,
    pub cross: bool,
    pub initial_margin: String,
    pub maintenance_margin: String,
    pub position_value: String,
    pub liquidation_price: String,
    pub unrealized_pnl: String,
    pub return_on_equity: String,
    pub cumulative_funding: String,
    /// 0..=3, higher is more auto-deleveraging risk.
    pub adl_index: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct MarginSummary {
    pub total_account_value: String,
    pub available_order_margin: String,
    pub total_initial_margin: String,
    pub total_maintenance_margin: String,
    pub total_position_value: String,
}

/// `GET /v1/account/fills` (at most 100 per page).
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct AccountFills {
    pub data: Vec<AccountFill>,
    pub more: bool,
    /// Keyset cursor for the next page; present while `more` is true.
    pub cursor: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct AccountFill {
    pub trade_id: u64,
    pub order_id: u64,
    pub instrument_id: u32,
    pub side: Side,
    pub price: String,
    pub quantity: String,
    pub taker: bool,
    /// Signed exchange fee; negative is a rebate.
    pub fee: String,
    pub fee_asset: String,
    pub previous_size: String,
    pub previous_entry_price: String,
    pub pnl: String,
    pub liquidation: bool,
    pub adl: bool,
    pub timestamp: i64,
    pub hash: String,
    pub builder_fee: String,
    pub total_fee: String,
    pub builder: Option<Builder>,
    pub settlement: bool,
    pub liquidation_details: Option<LiquidationDetails>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct LiquidationDetails {
    pub liquidated_user: Option<String>,
    pub mark: String,
    /// `market` or `backstop`.
    pub method: String,
}

/// `GET /v1/account/funding` (at most 100 per page).
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct AccountFundingHistory {
    pub data: Vec<AccountFunding>,
    pub more: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct AccountFunding {
    pub id: u64,
    pub instrument_id: u32,
    pub size: String,
    pub funding_rate: String,
    pub funding_asset: String,
    pub funding: String,
    pub timestamp: i64,
}

/// `GET /v1/account/config`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct InstrumentConfig {
    pub instrument_id: u32,
    pub leverage: u32,
    pub cross: bool,
}

/// `GET /v1/account/auto-cancel`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct AutoCancelStatus {
    /// Armed cancel time, `0` when nothing is armed.
    pub deadline: i64,
    pub triggered: u32,
    pub daily_limit: u32,
    pub next_reset: i64,
}

/// `sort` of `/v1/account/fills`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FillsSort {
    Desc,
    Asc,
}

impl FillsSort {
    pub fn as_str(self) -> &'static str {
        match self {
            FillsSort::Desc => "desc",
            FillsSort::Asc => "asc",
        }
    }
}
