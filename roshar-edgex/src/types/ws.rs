//! Websocket messages. Public: `wss://edgex-quote-prod-v2.edgex.exchange/api/v1/public/ws`, JSON
//! `subscribe`/`unsubscribe` by channel name. Private: `/api/v1/private/ws`, authenticated in the
//! handshake, no subscriptions.

use super::{EdgexAccount, EdgexCollateral, EdgexOrder, EdgexPosition};
use super::{EdgexCoin, EdgexDepthLevel, EdgexKlineType, EdgexPriceType, EdgexRiskTier};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Client → server frame: `{"type":"subscribe"|"unsubscribe","channel":...}` or
/// `{"type":"ping"|"pong","time":...}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EdgexWsRequest {
    #[serde(rename = "type")]
    pub kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time: Option<String>,
}

impl EdgexWsRequest {
    pub fn subscribe(channel: String) -> Self {
        Self {
            kind: "subscribe",
            channel: Some(channel),
            time: None,
        }
    }

    pub fn unsubscribe(channel: String) -> Self {
        Self {
            kind: "unsubscribe",
            channel: Some(channel),
            time: None,
        }
    }

    /// Reply to a server `ping`, echoing its `time`.
    pub fn pong(time: &str) -> Self {
        Self {
            kind: "pong",
            channel: None,
            time: Some(time.to_string()),
        }
    }

    /// Client-initiated `ping`; the server answers with a `pong` echoing `time`.
    pub fn ping(time: &str) -> Self {
        Self {
            kind: "ping",
            channel: None,
            time: Some(time.to_string()),
        }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("EdgexWsRequest serializes")
    }
}

/// Public channel names.
pub struct EdgexChannel;

impl EdgexChannel {
    /// `ticker.{contractId}`.
    pub fn ticker(contract_id: &str) -> String {
        format!("ticker.{contract_id}")
    }

    /// `ticker.all.1s`: every contract, batched each second.
    pub fn ticker_all() -> String {
        "ticker.all.1s".to_string()
    }

    /// `depth.{contractId}.{15|200}`.
    pub fn depth(contract_id: &str, level: EdgexDepthLevel) -> String {
        format!("depth.{contract_id}.{}", level.as_u32())
    }

    /// `trades.{contractId}`.
    pub fn trades(contract_id: &str) -> String {
        format!("trades.{contract_id}")
    }

    /// `kline.{priceType}.{contractId}.{interval}`.
    pub fn kline(
        contract_id: &str,
        price_type: EdgexPriceType,
        interval: EdgexKlineType,
    ) -> String {
        format!("kline.{price_type}.{contract_id}.{interval}")
    }

    /// `fundingRate.{contractId}`.
    pub fn funding_rate(contract_id: &str) -> String {
        format!("fundingRate.{contract_id}")
    }

    /// `fundingRate.all`.
    pub fn funding_rate_all() -> String {
        "fundingRate.all".to_string()
    }

    /// `metadata`.
    pub fn metadata() -> String {
        "metadata".to_string()
    }
}

/// Server → client frame on the public websocket.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum EdgexPublicWsMessage {
    #[serde(rename = "connected")]
    Connected { sid: String },
    #[serde(rename = "subscribed")]
    Subscribed { channel: String, request: String },
    #[serde(rename = "unsubscribed")]
    Unsubscribed { channel: String, request: String },
    /// Server heartbeat; answer with `EdgexWsRequest::pong(time)`.
    #[serde(rename = "ping")]
    Ping { time: String },
    #[serde(rename = "pong")]
    Pong { time: String },
    #[serde(rename = "error")]
    Error {
        request: String,
        content: EdgexWsError,
    },
    #[serde(rename = "quote-event")]
    QuoteEvent {
        channel: String,
        content: EdgexQuoteContent,
    },
}

/// `{"code":"GATEWAY_INVALID_CHANNEL","msg":"invalid channel : ..."}`.
#[derive(Debug, Clone, Deserialize)]
pub struct EdgexWsError {
    pub code: String,
    pub msg: String,
}

/// Live frames send `Snapshot` first and `changed` (lower case) afterwards; the docs write
/// `Changed`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum EdgexDataType {
    #[serde(rename = "Snapshot")]
    Snapshot,
    #[serde(rename = "changed", alias = "Changed")]
    Changed,
}

/// `quote-event` content. `data` is a list whose item type depends on the channel:
/// `ticker.*` → [`EdgexWsTicker`], `depth.*` → `EdgexDepth`, `trades.*` → [`EdgexWsTrade`],
/// `kline.*` → `EdgexKline`, `fundingRate.*` → [`EdgexWsFundingRate`],
/// `metadata` → [`EdgexWsMetaData`].
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EdgexQuoteContent {
    pub channel: String,
    pub data_type: EdgexDataType,
    pub data: Value,
}

impl EdgexQuoteContent {
    pub fn data<T: DeserializeOwned>(&self) -> serde_json::Result<Vec<T>> {
        Vec::<T>::deserialize(&self.data)
    }
}

/// `ticker.*` item. `bestAskPrice`/`bestBidPrice` are absent on some updates (and on markets
/// without a book side); `liquidity` appears only on some contracts.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EdgexWsTicker {
    pub contract_id: String,
    pub contract_name: String,
    pub price_change: String,
    pub price_change_percent: String,
    pub trades: String,
    pub size: String,
    pub value: String,
    pub high: String,
    pub low: String,
    pub open: String,
    pub close: String,
    pub high_time: String,
    pub low_time: String,
    pub start_time: String,
    pub end_time: String,
    pub last_price: String,
    pub index_price: String,
    pub oracle_price: String,
    pub mark_price: String,
    pub open_interest: String,
    pub funding_rate: String,
    pub funding_time: String,
    pub next_funding_time: String,
    pub market_open: bool,
    pub best_ask_price: Option<String>,
    pub best_bid_price: Option<String>,
    pub liquidity: Option<String>,
}

/// `trades.*` item.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EdgexWsTrade {
    pub contract_id: String,
    pub contract_name: String,
    pub ticket_id: String,
    pub time: String,
    pub price: String,
    pub size: String,
    pub value: String,
    pub is_buyer_maker: bool,
    pub is_best_match: bool,
    pub maker_order_id: String,
    pub taker_order_id: String,
}

/// `metadata` item. Live (2026-11-16) this is an older, V1-era shape and NOT the REST
/// `getMetaData` payload: `global.contractAddress`/`chainId` differ from REST, contracts carry
/// `oraclePriceSigner`/`oraclePriceSignedAssetId`, and there is no `campaign`. Do not derive the
/// L2 signing domain from it; use REST `getMetaData`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EdgexWsMetaData {
    pub global: EdgexWsMetaGlobal,
    /// Same shape as REST coins (`assetId`/`resolution` only on the collateral coin).
    pub coin_list: Vec<EdgexCoin>,
    pub contract_list: Vec<EdgexWsMetaContract>,
    pub multi_chain: EdgexWsMultiChain,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EdgexWsMetaGlobal {
    pub app_env: String,
    pub app_name: String,
    pub app_only_sign_on: String,
    pub booster_ratio: String,
    pub chain_id: String,
    pub collateral_coin: EdgexCoin,
    pub contract_address: String,
    pub fast_withdraw_account_id: String,
    pub fast_withdraw_account_l2_key: String,
    pub fast_withdraw_max_amount: String,
    pub fast_withdraw_registry_address: String,
    pub fee_account_id: String,
    pub funding_validity_period: i64,
    pub isolated_margin_sign_permission: String,
    pub isolated_margin_signer: String,
    pub max_funding_rate: i64,
    pub orders_tree_height: i64,
    pub pool_account_id: String,
    pub pool_account_l2_key: String,
    pub positions_tree_height: i64,
    pub price_validity_period: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EdgexWsMetaContract {
    pub base_coin_id: String,
    pub contract_id: String,
    pub contract_name: String,
    pub default_leverage: String,
    pub default_maker_fee_rate: String,
    pub default_taker_fee_rate: String,
    pub display_digit_merge: String,
    pub display_hot_icon: bool,
    pub display_max_leverage: String,
    pub display_min_leverage: String,
    pub display_new_icon: bool,
    pub enable_booster_credit: bool,
    pub enable_display: bool,
    pub enable_open_position: bool,
    pub enable_trade: bool,
    pub funding_impact_margin_notional: String,
    pub funding_interest_rate: String,
    pub funding_max_rate: String,
    pub funding_min_rate: String,
    pub funding_rate_interval_min: String,
    pub liquidate_fee_rate: String,
    pub match_server_name: String,
    pub max_order_buy_price_ratio: String,
    pub max_order_size: String,
    pub max_position_size: String,
    pub min_order_sell_price_ratio: String,
    pub min_order_size: String,
    pub oracle_price_quorum: String,
    pub oracle_price_signed_asset_id: Vec<String>,
    pub oracle_price_signer: Vec<String>,
    pub quote_coin_id: String,
    pub resolution: String,
    pub risk_tier_list: Vec<EdgexRiskTier>,
    pub step_size: String,
    pub synthetic_asset_id: String,
    pub tick_size: String,
    pub tpsl_spread_protection_threshold: String,
}

/// `metadata.multiChain`: deposit/withdraw chains.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EdgexWsMultiChain {
    pub coin_id: String,
    pub max_withdraw: String,
    pub min_deposit: String,
    pub min_withdraw: String,
    pub chain_list: Vec<EdgexWsChain>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EdgexWsChain {
    pub block_time: String,
    pub chain: String,
    pub chain_icon_url: String,
    pub chain_id: String,
    pub contract_address: String,
    pub deposit_gas_fee_less: bool,
    pub fee_less: bool,
    pub fee_rate: String,
    pub gas_less: bool,
    pub gas_token: String,
    pub min_fee: String,
    pub rpc_url: String,
    pub token_list: Vec<EdgexWsChainToken>,
    pub tx_confirm: String,
    pub web_tx_url: String,
    pub withdraw_gas_fee_less: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EdgexWsChainToken {
    pub decimals: String,
    pub fixed_rate: String,
    pub icon_url: String,
    pub pull_off: bool,
    pub token: String,
    pub token_address: String,
    pub withdraw_enable: bool,
}

/// Server → client frame on the private websocket.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum EdgexPrivateWsMessage {
    #[serde(rename = "connected")]
    Connected { sid: String, time: String },
    /// Answer with `EdgexWsRequest::pong(time)`.
    #[serde(rename = "ping")]
    Ping { time: String },
    #[serde(rename = "pong")]
    Pong { time: String },
    #[serde(rename = "trade-event")]
    TradeEvent { content: EdgexTradeEvent },
}

/// `trade-event` content. `event` is `Snapshot`, `ORDER_UPDATE`, `DEPOSIT_UPDATE`,
/// `WITHDRAW_UPDATE`, `TRANSFER_IN_UPDATE`, `TRANSFER_OUT_UPDATE` or `FUNDING_SETTLEMENT`.
/// `accountId` is documented as always `0`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EdgexTradeEvent {
    pub event: String,
    pub version: i64,
    pub time: i64,
    pub account_id: i64,
    pub data: EdgexTradeEventData,
}

/// Groups of a `trade-event`. The docs: "Not every event populates every group", so absent
/// groups are empty.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct EdgexTradeEventData {
    pub account: Vec<EdgexAccount>,
    pub collateral: Vec<EdgexCollateral>,
    pub collateral_transaction: Vec<Value>,
    pub position: Vec<EdgexPosition>,
    pub position_transaction: Vec<Value>,
    pub order: Vec<EdgexOrder>,
    pub order_fill_transaction: Vec<EdgexWsOrderFill>,
    pub deposit: Vec<Value>,
    pub withdraw: Vec<Value>,
    pub transfer_in: Vec<Value>,
    pub transfer_out: Vec<Value>,
}

/// `fundingRate.*` item. Unlike the REST record, live frames never carry `forecastFundingRate`
/// (0 of 596 items captured on 2026-11-15), and the settlement flag is `settlement`, not
/// `isSettlement`. (The docs' WS model lists `contractId`, `fundingRate`, `nextFundingTime` and
/// `timestamp`; live frames carry the fields below.)
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EdgexWsFundingRate {
    pub contract_id: String,
    pub funding_time: String,
    pub funding_timestamp: String,
    pub oracle_price: String,
    pub index_price: String,
    pub mark_price: String,
    pub funding_rate: String,
    pub settlement: bool,
    pub previous_funding_rate: String,
    pub previous_funding_timestamp: String,
    pub premium_index: String,
    pub avg_premium_index: String,
    pub premium_index_timestamp: String,
    pub impact_margin_notional: String,
    pub impact_ask_price: String,
    pub impact_bid_price: String,
    pub interest_rate: String,
    pub predicted_funding_rate: String,
    pub funding_rate_interval_min: String,
    pub funding_index: String,
}

/// `orderFillTransaction` record on the private websocket. The documented live sample carries
/// only the fields required here; the rest of the REST fill record is optional.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EdgexWsOrderFill {
    pub id: String,
    pub order_id: String,
    pub fill_size: String,
    pub fill_value: String,
    pub fill_fee: String,
    pub fill_price: String,
    /// `MAKER` or `TAKER`.
    pub direction: String,
    pub match_time: String,
    pub user_id: Option<String>,
    pub account_id: Option<String>,
    pub coin_id: Option<String>,
    pub contract_id: Option<String>,
    pub order_side: Option<String>,
    pub liquidate_fee: Option<String>,
    pub realize_pnl: Option<String>,
    pub created_time: Option<String>,
    pub updated_time: Option<String>,
}
