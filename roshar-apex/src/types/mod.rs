//! Wire types for ApeX Omni REST and websocket messages, named after the exchange's fields.
//!
//! Public types were built from live responses (fixtures under `tests/fixtures`). Private types
//! follow the documented examples; fields the docs mark optional, or that differ between documented
//! examples, are `Option`.

use serde::de::IgnoredAny;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ── REST envelope ─────────────────────────────────────────────────────────────

/// Every REST reply: `{"data": ..., "timeCost": n}` on success, `{"code": n, "msg": "...",
/// "timeCost": n}` on failure (HTTP 200 either way).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApexEnvelope<T> {
    pub data: Option<T>,
    pub code: Option<i64>,
    pub msg: Option<String>,
    pub time_cost: u64,
}

/// A REST error reply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApexError {
    pub code: i64,
    pub msg: String,
}

impl std::fmt::Display for ApexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ApeX error {}: {}", self.code, self.msg)
    }
}

impl std::error::Error for ApexError {}

impl<T> ApexEnvelope<T> {
    /// The payload, or the exchange's error. A reply with neither is an error too.
    pub fn into_result(self) -> Result<T, ApexError> {
        if let Some(code) = self.code {
            if code != 0 || self.data.is_none() {
                return Err(ApexError {
                    code,
                    msg: self.msg.unwrap_or_default(),
                });
            }
        }
        self.data.ok_or(ApexError {
            code: -1,
            msg: "reply has neither data nor code".to_string(),
        })
    }
}

// ── Public REST ───────────────────────────────────────────────────────────────

/// `GET /v3/time`: server time, unix ms.
#[derive(Debug, Clone, Deserialize)]
pub struct ApexServerTime {
    pub time: u64,
}

/// `GET /v3/symbols`. Only the parts needed to trade are modelled; `omniSwapConfig`,
/// `eventConfig`, `spotConfig.global`/`multiChain`/`web3TokenConfig` and `contractConfig.global`
/// are ignored.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApexSymbols {
    pub spot_config: ApexSpotConfig,
    pub contract_config: ApexContractConfig,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApexSpotConfig {
    pub assets: Vec<ApexAsset>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApexContractConfig {
    /// Settle / collateral assets. An order's `settle_decimals` is the `decimals` of the asset whose
    /// `token` equals the contract's `settleAssetId`.
    pub assets: Vec<ApexAsset>,
    pub tokens: Vec<ApexToken>,
    pub perpetual_contract: Vec<ApexContract>,
    pub prelaunch_contract: Vec<ApexContract>,
    pub prediction_contract: Vec<ApexContract>,
    pub stock_contract: Vec<ApexContract>,
    pub max_market_balance_buffer: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApexAsset {
    pub token_id: String,
    pub token: String,
    pub display_name: String,
    pub decimals: u32,
    pub show_step: String,
    pub icon_url: String,
    pub l2_withdraw_fee: String,
    pub enable_collateral: bool,
    pub enable_cross_collateral: bool,
    /// `null` for assets that are not cross collateral.
    pub cross_collateral_discount_rate: Option<Vec<ApexDiscountStep>>,
    pub is_gray: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApexDiscountStep {
    pub value_step: String,
    pub discount_rate: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApexToken {
    pub token: String,
    pub step_size: String,
    pub icon_url: String,
}

/// One contract (perpetual, prelaunch, prediction or stock).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApexContract {
    /// `BTC-USDT`: the symbol of order, account and funding-history requests.
    pub symbol: String,
    /// `BTCUSDT`: the symbol of depth, trades, klines, ticker and websocket topics.
    pub cross_symbol_name: String,
    pub symbol_display_name: String,
    pub cross_id: u64,
    /// `-1` on some live contracts (e.g. ETCUSDT, seen 2026-11-16).
    pub cross_symbol_id: i64,
    /// Signed into every order (`ApexOrderContext::l2_pair_id`).
    pub l2_pair_id: String,
    pub settle_asset_id: String,
    pub base_token_id: String,
    pub token_name: String,
    pub tick_size: String,
    pub step_size: String,
    pub min_order_size: String,
    pub max_order_size: String,
    pub max_position_size: String,
    pub max_position_value: String,
    pub max_market_price_range: String,
    pub max_market_size_buffer: String,
    pub initial_margin_rate: String,
    pub maintenance_margin_rate: String,
    pub max_maintenance_margin_rate: String,
    pub default_initial_margin_rate: String,
    pub incremental_initial_margin_rate: String,
    pub incremental_maintenance_margin_rate: String,
    pub incremental_position_value: String,
    pub baseline_position_value: String,
    pub display_max_leverage: String,
    pub display_min_leverage: String,
    pub digit_merge: String,
    pub enable_display: bool,
    pub enable_open_position: bool,
    pub enable_trade: bool,
    pub enable_funding_settlement: bool,
    pub enable_funding_mx_value: bool,
    pub funding_impact_margin_notional: String,
    pub funding_interest_rate: String,
    pub funding_max_rate: String,
    pub funding_min_rate: String,
    pub funding_max_value: String,
    pub index_price_decimals: u32,
    pub index_price_var_rate: String,
    pub open_position_oi_limit_rate: String,
    pub tag_icon_url: String,
    pub tag: String,
    pub risk_tip: bool,
    pub kline_start_time: u64,
    pub settle_time_stamp: u64,
    pub is_prelaunch: bool,
    pub risk_limit_config: ApexRiskLimitConfig,
    pub contract_type: String,
    pub prediction_contract_type: String,
    /// Absent on some contracts.
    pub category: Option<String>,
    /// Delisting time, unix seconds; only on contracts scheduled for removal.
    pub pull_off_time: Option<u64>,
    /// Unix seconds; only on contracts scheduled for removal.
    pub disable_open_position_time: Option<u64>,
    /// Unix ms; only on dated contracts.
    pub delivery_time: Option<u64>,
    /// e.g. `LIQUID_RISK`; only on some contracts.
    pub tip_tag: Option<String>,
}

impl ApexContract {
    /// `l2PairId` as the integer the order signature needs.
    pub fn l2_pair_id_u16(&self) -> Result<u16, std::num::ParseIntError> {
        self.l2_pair_id.parse()
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApexRiskLimitConfig {
    pub position_steps: Vec<String>,
    pub imr_steps: Vec<String>,
    pub mmr_steps: Vec<String>,
}

/// `[price, size]`.
pub type ApexLevel = [String; 2];

/// `GET /v3/depth`. `a`/`b` are `null` (and `s` empty) for an unknown symbol.
#[derive(Debug, Clone, Deserialize)]
pub struct ApexDepth {
    pub a: Option<Vec<ApexLevel>>,
    pub b: Option<Vec<ApexLevel>>,
    pub s: String,
    pub u: u64,
}

/// `GET /v3/trades` entry.
#[derive(Debug, Clone, Deserialize)]
pub struct ApexTrade {
    pub i: String,
    pub p: String,
    /// `Buy` / `Sell` (taker side).
    #[serde(rename = "S")]
    pub side: String,
    pub v: String,
    pub s: String,
    #[serde(rename = "T")]
    pub time: u64,
}

/// `GET /v3/klines` candle (the reply maps symbol to candles).
#[derive(Debug, Clone, Deserialize)]
pub struct ApexKline {
    pub s: String,
    /// Interval token, e.g. `1`, `60`, `D`.
    pub i: String,
    /// Start time, unix ms.
    pub t: u64,
    pub o: String,
    pub h: String,
    pub l: String,
    pub c: String,
    pub v: String,
    pub tr: String,
}

/// `GET /v3/ticker` entry. Also the current funding rate. Empty strings mean "not available"
/// (`oraclePrice`, `tradeCount` on REST).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApexTicker {
    pub symbol: String,
    pub last_price: String,
    pub mark_price: String,
    pub index_price: String,
    pub oracle_price: String,
    pub price24h_pcnt: String,
    pub high_price24h: String,
    pub low_price24h: String,
    pub turnover24h: String,
    pub volume24h: String,
    pub open_interest: String,
    pub funding_rate: String,
    pub predicted_funding_rate: String,
    /// ISO-8601, e.g. `2026-11-15T23:00:00Z`.
    pub next_funding_time: String,
    pub trade_count: String,
}

/// `GET /v3/history-funding`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApexFundingHistory {
    pub history_funds: Vec<ApexFundingEntry>,
    pub total_size: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApexFundingEntry {
    pub symbol: String,
    pub rate: String,
    pub price: String,
    pub funding_time: u64,
    pub funding_timestamp: u64,
}

/// Query of `GET /v3/history-funding`; unset fields are omitted.
#[derive(Debug, Clone, Default)]
pub struct ApexFundingHistoryQuery {
    pub limit: Option<u32>,
    /// Page number, from 0.
    pub page: Option<u32>,
    /// Unix ms.
    pub begin_time_inclusive: Option<u64>,
    /// Unix ms.
    pub end_time_exclusive: Option<u64>,
}

// ── Private REST ──────────────────────────────────────────────────────────────

/// `GET /v3/account`. `spotAccount`, `spotWallets` and `experienceMoney` are not modelled.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApexAccount {
    /// Account id, the `account_id` of order signatures.
    pub id: String,
    pub ethereum_address: String,
    pub l2_key: String,
    pub contract_account: ApexContractAccount,
    pub contract_wallets: Vec<ApexContractWallet>,
    pub positions: Vec<ApexPosition>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApexContractAccount {
    pub created_at: u64,
    pub taker_fee_rate: String,
    pub maker_fee_rate: String,
    pub min_initial_margin_rate: String,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApexContractWallet {
    pub asset: String,
    pub balance: String,
    pub pending_deposit_amount: String,
    pub pending_withdraw_amount: String,
    pub pending_transfer_out_amount: String,
    pub pending_transfer_in_amount: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApexPosition {
    pub symbol: String,
    /// `LONG` / `SHORT`.
    pub side: String,
    pub size: String,
    pub entry_price: String,
    pub exit_price: Option<String>,
    pub fee: Option<String>,
    pub funding_fee: Option<String>,
    pub created_at: Option<u64>,
    pub updated_time: Option<u64>,
    /// ADL ranking.
    pub light_numbers: Option<String>,
    pub custom_initial_margin_rate: Option<String>,
}

/// `GET /v3/account-balance`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApexAccountBalance {
    pub total_equity_value: String,
    pub available_balance: String,
    pub initial_margin: String,
    pub maintenance_margin: String,
    pub symbol_to_oracle_price: HashMap<String, ApexOraclePrice>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApexOraclePrice {
    pub oracle_price: String,
    pub created_time: u64,
}

/// An order as `POST /v3/order`, `GET /v3/open-orders`, `/v3/history-orders`, `/v3/order` and
/// `/v3/order-by-client-order-id` return it. The documented examples disagree on the client id
/// field (`clientId` vs `clientOrderId`), so both are optional.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApexOrder {
    pub id: String,
    pub symbol: String,
    pub side: String,
    pub price: String,
    pub size: String,
    #[serde(rename = "type")]
    pub order_type: String,
    /// `PENDING`, `OPEN`, `FILLED`, `CANCELED`, `EXPIRED`, `UNTRIGGERED`.
    pub status: String,
    pub time_in_force: String,
    pub reduce_only: bool,
    pub created_at: u64,
    pub client_id: Option<String>,
    pub client_order_id: Option<String>,
    pub account_id: Option<String>,
    pub limit_fee: Option<String>,
    pub fee: Option<String>,
    pub trigger_price: Option<String>,
    pub trigger_price_type: Option<String>,
    pub trailing_percent: Option<String>,
    pub remaining_size: Option<String>,
    pub updated_time: Option<u64>,
    pub expires_at: Option<u64>,
    pub post_only: Option<bool>,
    pub cancel_reason: Option<String>,
    pub latest_match_fill_price: Option<String>,
    pub cum_match_fill_size: Option<String>,
    pub cum_match_fill_value: Option<String>,
    pub cum_match_fill_fee: Option<String>,
    pub cum_success_fill_size: Option<String>,
    pub cum_success_fill_value: Option<String>,
    pub cum_success_fill_fee: Option<String>,
    pub is_position_tpsl: Option<bool>,
}

/// `GET /v3/history-orders` and `GET /v3/fills` (whose rows are order-shaped, under `orders`).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApexOrderPage {
    pub orders: Vec<ApexOrder>,
    pub total_size: u64,
}

/// Paging/time filters shared by the private history endpoints; unset fields are omitted.
#[derive(Debug, Clone, Default)]
pub struct ApexHistoryQuery<'a> {
    pub symbol: Option<&'a str>,
    /// `BUY` / `SELL`.
    pub side: Option<&'a str>,
    pub limit: Option<u32>,
    /// Page number, from 0.
    pub page: Option<u32>,
    /// Unix ms.
    pub begin_time_inclusive: Option<u64>,
    /// Unix ms.
    pub end_time_exclusive: Option<u64>,
    /// History orders: order status filter. Funding: position funding status.
    pub status: Option<&'a str>,
    /// History orders: `LIMIT`, `MARKET`, ...
    pub order_type: Option<&'a str>,
    /// History orders / fills `orderType`: `ACTIVE`, `CONDITION`, `HISTORY`.
    pub order_category: Option<&'a str>,
}

/// `GET /v3/funding`: the account's funding payments.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApexFundingPayments {
    pub funding_values: Vec<ApexFundingPayment>,
    pub total_size: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApexFundingPayment {
    pub id: String,
    pub symbol: String,
    pub funding_value: String,
    pub rate: String,
    pub position_size: String,
    pub price: String,
    pub side: String,
    pub status: String,
    pub funding_time: u64,
    pub transaction_id: String,
}

/// `GET /v3/get-worst-price`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApexWorstPrice {
    pub worst_price: String,
    pub bid_one_price: String,
    pub ask_one_price: String,
}

/// Replies whose `data` carries nothing the caller needs (`delete-open-orders`,
/// `set-initial-margin-rate`).
pub type ApexAck = IgnoredAny;

// ── WebSocket ─────────────────────────────────────────────────────────────────

/// `{"op": ..., "args": [...]}`: subscribe, unsubscribe, ping, pong.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApexWsRequest {
    pub op: String,
    pub args: Vec<String>,
}

/// The request echoed in an `ApexWsResponse`. When the server cannot decode a frame it echoes
/// `{"op":"","args":null}`; `args` is always sent.
#[derive(Debug, Clone, Deserialize)]
pub struct ApexWsEchoedRequest {
    pub op: String,
    #[serde(deserialize_with = "nullable")]
    pub args: Option<Vec<String>>,
}

/// Key required, value may be `null`.
fn nullable<'de, D, T>(d: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(d)
}

impl ApexWsRequest {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("ApexWsRequest serializes")
    }

    pub fn subscribe(topics: &[String]) -> Self {
        Self {
            op: "subscribe".to_string(),
            args: topics.to_vec(),
        }
    }

    pub fn unsubscribe(topics: &[String]) -> Self {
        Self {
            op: "unsubscribe".to_string(),
            args: topics.to_vec(),
        }
    }

    /// Client heartbeat: `{"op":"ping","args":["<unix ms>"]}`.
    pub fn ping(timestamp_ms: u64) -> Self {
        Self {
            op: "ping".to_string(),
            args: vec![timestamp_ms.to_string()],
        }
    }

    /// Reply to a server ping, echoing its args. The server closes a connection that leaves its
    /// pings unanswered for about 150 s, whether or not the client pings.
    pub fn pong(args: Vec<String>) -> Self {
        Self {
            op: "pong".to_string(),
            args,
        }
    }

    /// The pong to send back when `text` is a server ping, `None` otherwise.
    pub fn pong_reply(text: &str) -> Option<String> {
        let msg: ApexWsRequest = serde_json::from_str(text).ok()?;
        (msg.op == "ping").then(|| Self::pong(msg.args).to_json())
    }
}

/// Order book topic depth.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApexBookDepth {
    D25,
    D200,
}

/// Push frequency token: `H` (high) or `M` (middle). Checked live on 2026-11-16: the server
/// rejects `orderBook*.M.*` and `recentlyTrade.M.*` with `error:handler not found` and accepts
/// `instrumentInfo.M.*`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApexWsFrequency {
    High,
    Middle,
}

impl ApexWsFrequency {
    fn as_str(self) -> &'static str {
        match self {
            Self::High => "H",
            Self::Middle => "M",
        }
    }
}

/// Public topic names. Symbols are `crossSymbolName`s (`BTCUSDT`).
pub struct ApexTopic;

impl ApexTopic {
    /// `orderBook{25|200}.{H|M}.{symbol}`: snapshot, then deltas.
    pub fn order_book(symbol: &str, depth: ApexBookDepth, frequency: ApexWsFrequency) -> String {
        let depth = match depth {
            ApexBookDepth::D25 => 25,
            ApexBookDepth::D200 => 200,
        };
        format!("orderBook{depth}.{}.{symbol}", frequency.as_str())
    }

    /// `recentlyTrade.{H|M}.{symbol}`.
    pub fn trades(symbol: &str, frequency: ApexWsFrequency) -> String {
        format!("recentlyTrade.{}.{symbol}", frequency.as_str())
    }

    /// `instrumentInfo.{H|M}.{symbol}`: ticker, mark/index price and funding rate.
    pub fn ticker(symbol: &str, frequency: ApexWsFrequency) -> String {
        format!("instrumentInfo.{}.{symbol}", frequency.as_str())
    }

    /// `instrumentInfo.all`: compact tickers for every symbol.
    pub fn all_tickers() -> String {
        "instrumentInfo.all".to_string()
    }

    /// `candle.{interval}.{symbol}`; `interval` is `1 3 5 15 30 60 120 240 360 720 D W M`.
    pub fn candle(symbol: &str, interval: &str) -> String {
        format!("candle.{interval}.{symbol}")
    }
}

/// Private topics, subscribed in the login message or with `subscribe` after login.
pub const APEX_TOPIC_ACCOUNT: &str = "ws_zk_accounts_v3";
pub const APEX_TOPIC_NOTIFY: &str = "ws_notify_v1";

/// Reply to a request: `{"success":..,"ret_msg":..,"conn_id":..,"request":{"op":..,"args":[..]}}`.
/// A ping is answered with `ret_msg: "pong"`. Failures have `success: false`, e.g.
/// `ret_msg: "error:handler not found"` for an unknown topic (the whole batch is rejected) or
/// `"Failed to decode incoming data:..."` for a non-JSON frame.
#[derive(Debug, Clone, Deserialize)]
pub struct ApexWsResponse {
    pub success: bool,
    pub ret_msg: String,
    pub conn_id: String,
    pub request: ApexWsEchoedRequest,
}

/// Any frame of the public websocket, dispatched on `topic` (pushes), `op` (server ping) or the
/// reply shape.
#[derive(Debug, Clone)]
pub enum ApexWsPublicMessage {
    /// Server heartbeat `{"op":"ping","args":["<ms>"]}`; answer with `ApexWsRequest::pong_reply`.
    Ping(ApexWsRequest),
    /// Subscribe/unsubscribe/ping reply, success or failure.
    Response(ApexWsResponse),
    Book(ApexWsPush<ApexWsBook>),
    Trades(ApexWsPush<Vec<ApexWsTrade>>),
    Ticker(ApexWsPush<ApexWsTicker>),
    AllTickers(ApexWsPush<Vec<ApexWsCompactTicker>>),
    Candles(ApexWsPush<Vec<ApexWsCandle>>),
}

impl ApexWsPublicMessage {
    /// Parse a text frame. Unknown topics and malformed frames are errors.
    pub fn parse(text: &str) -> Result<Self, serde_json::Error> {
        use serde::de::Error as _;
        let value: serde_json::Value = serde_json::from_str(text)?;
        if let Some(topic) = value.get("topic").and_then(|t| t.as_str()) {
            let msg = if topic == "instrumentInfo.all" {
                Self::AllTickers(serde_json::from_value(value)?)
            } else if topic.starts_with("orderBook") {
                Self::Book(serde_json::from_value(value)?)
            } else if topic.starts_with("recentlyTrade.") {
                Self::Trades(serde_json::from_value(value)?)
            } else if topic.starts_with("instrumentInfo.") {
                Self::Ticker(serde_json::from_value(value)?)
            } else if topic.starts_with("candle.") {
                Self::Candles(serde_json::from_value(value)?)
            } else {
                return Err(serde_json::Error::custom(format!(
                    "unknown ApeX topic {topic}"
                )));
            };
            return Ok(msg);
        }
        if value.get("op").is_some() {
            let req: ApexWsRequest = serde_json::from_value(value)?;
            if req.op != "ping" {
                return Err(serde_json::Error::custom(format!(
                    "unexpected ApeX op {}",
                    req.op
                )));
            }
            return Ok(Self::Ping(req));
        }
        Ok(Self::Response(serde_json::from_value(value)?))
    }
}

/// A public topic push.
#[derive(Debug, Clone, Deserialize)]
pub struct ApexWsPush<T> {
    pub topic: String,
    /// `snapshot` or `delta`.
    #[serde(rename = "type")]
    pub kind: String,
    pub data: T,
    /// Absent on candle pushes.
    pub cs: Option<u64>,
    /// Microseconds on book/trade/ticker pushes, milliseconds on candle pushes.
    pub ts: u64,
}

/// `orderBook*` data. A size of `0` deletes the level.
#[derive(Debug, Clone, Deserialize)]
pub struct ApexWsBook {
    pub s: String,
    pub b: Vec<ApexLevel>,
    pub a: Vec<ApexLevel>,
    /// Update id; resets to 1 on a snapshot.
    pub u: u64,
}

/// `recentlyTrade*` entry.
#[derive(Debug, Clone, Deserialize)]
pub struct ApexWsTrade {
    #[serde(rename = "T")]
    pub time: u64,
    pub s: String,
    #[serde(rename = "S")]
    pub side: String,
    pub v: String,
    pub p: String,
    /// Tick direction: `PlusTick`, `ZeroPlusTick`, `MinusTick`, `ZeroMinusTick`.
    #[serde(rename = "L")]
    pub tick_direction: String,
    pub i: String,
}

/// `instrumentInfo.{H|M}.{symbol}` data. Deltas carry only changed fields.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApexWsTicker {
    pub symbol: String,
    pub last_price: Option<String>,
    pub mark_price: Option<String>,
    pub index_price: Option<String>,
    pub oracle_price: Option<String>,
    pub price24h_pcnt: Option<String>,
    pub high_price24h: Option<String>,
    pub low_price24h: Option<String>,
    pub turnover24h: Option<String>,
    pub volume24h: Option<String>,
    pub open_interest: Option<String>,
    pub trade_count: Option<String>,
    pub funding_rate: Option<String>,
    pub predicted_funding_rate: Option<String>,
    pub next_funding_time: Option<String>,
    pub symbol_status: Option<String>,
}

/// `instrumentInfo.all` entry (abbreviated keys). Updates carry only changed fields.
#[derive(Debug, Clone, Deserialize)]
pub struct ApexWsCompactTicker {
    pub s: String,
    /// Last price.
    pub p: Option<String>,
    /// 24h change.
    pub pr: Option<String>,
    pub h: Option<String>,
    pub l: Option<String>,
    /// Index price.
    pub xp: Option<String>,
    /// Mark price.
    pub mp: Option<String>,
    /// Oracle price.
    pub op: Option<String>,
    /// 24h turnover.
    pub to: Option<String>,
    pub v: Option<String>,
    /// Funding rate.
    pub fr: Option<String>,
    /// Open interest.
    pub o: Option<String>,
    /// Trade count.
    pub tc: Option<String>,
    /// Symbol status.
    pub ss: Option<String>,
}

/// `candle.*` entry.
#[derive(Debug, Clone, Deserialize)]
pub struct ApexWsCandle {
    pub start: u64,
    pub end: u64,
    pub interval: String,
    pub open: String,
    pub close: String,
    pub high: String,
    pub low: String,
    pub volume: String,
    pub turnover: String,
    /// `true` on the candle's final update.
    pub confirm: bool,
    pub time: u64,
}

/// A private topic push: `{"topic":..,"type":..,"timestamp":..,"contents":{..}}`. The docs state an
/// absent `type` means `snapshot`.
#[derive(Debug, Clone, Deserialize)]
pub struct ApexWsPrivatePush<T> {
    pub topic: String,
    #[serde(rename = "type")]
    pub kind: Option<String>,
    pub timestamp: Option<u64>,
    pub contents: T,
}

/// `ws_zk_accounts_v3` contents. Deltas carry only the collections that changed.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApexWsAccountContents {
    pub orders: Option<Vec<ApexWsOrder>>,
    pub fills: Option<Vec<ApexWsFill>>,
    pub positions: Option<Vec<ApexWsPosition>>,
    pub contract_wallets: Option<Vec<ApexWsContractWallet>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApexWsOrder {
    pub id: String,
    pub symbol: String,
    pub side: String,
    pub price: String,
    pub size: String,
    #[serde(rename = "type")]
    pub order_type: String,
    pub status: String,
    pub reduce_only: bool,
    pub created_at: u64,
    pub updated_at: u64,
    pub account_id: Option<String>,
    pub client_id: Option<String>,
    pub client_order_id: Option<String>,
    pub remaining_size: Option<String>,
    pub limit_fee: Option<String>,
    pub trigger_price: Option<String>,
    pub cancel_reason: Option<String>,
    pub cum_success_fill_size: Option<String>,
    pub cum_success_fill_value: Option<String>,
    pub cum_success_fill_fee: Option<String>,
    pub expires_at: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApexWsFill {
    pub id: String,
    pub order_id: String,
    pub symbol: String,
    pub side: String,
    pub price: String,
    pub size: String,
    pub fee: String,
    /// `MAKER` / `TAKER`.
    pub liquidity: String,
    pub created_at: u64,
    pub account_id: Option<String>,
    pub quote_amount: Option<String>,
    pub is_open: Option<bool>,
    pub updated_at: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApexWsPosition {
    pub symbol: String,
    pub side: String,
    pub size: String,
    pub entry_price: String,
    pub updated_at: u64,
    pub account_id: Option<String>,
    pub exit_price: Option<String>,
    pub realized_pnl: Option<String>,
    pub funding_fee: Option<String>,
    pub sum_open: Option<String>,
    pub sum_close: Option<String>,
    pub open_value: Option<String>,
    pub custom_imr: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApexWsContractWallet {
    pub token: String,
    pub balance: String,
    pub pending_deposit_amount: Option<String>,
    pub pending_withdraw_amount: Option<String>,
    pub pending_transfer_in_amount: Option<String>,
    pub pending_transfer_out_amount: Option<String>,
}

#[cfg(test)]
mod tests;
