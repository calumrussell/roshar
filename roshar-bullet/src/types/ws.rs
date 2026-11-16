//! Websocket topics, request builders and server messages (`wss://<host>/ws`).
//!
//! Timestamps are unix microseconds. Messages are parsed with [`BulletWsMessage::parse`], which
//! dispatches on the `e` field (order results carry the call type, e.g. `User/PlaceOrders`, and
//! order errors carry no `e`).

use anyhow::{anyhow, Result};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::{BulletLevel, BulletTxStatus};

/// Order book depth of `SYMBOL@depth*`. `Default` (`@depth`) is 10 levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BulletDepthLevels {
    Default,
    L5,
    L10,
    L20,
    L100,
}

/// Kline interval of `SYMBOL@kline_INTERVAL` (documented as coming soon; the server accepts
/// the subscription but publishes nothing yet).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BulletKlineInterval {
    M1,
    M5,
    M15,
    M30,
    H1,
    H4,
    D1,
}

impl BulletKlineInterval {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::M1 => "1m",
            Self::M5 => "5m",
            Self::M15 => "15m",
            Self::M30 => "30m",
            Self::H1 => "1h",
            Self::H4 => "4h",
            Self::D1 => "1d",
        }
    }
}

/// A subscription topic. Symbols are Bullet symbols (`BTC-USD`); user topics take the main
/// account address, not a delegate's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BulletTopic<'a> {
    /// `SYMBOL@depth[N]`.
    Depth(&'a str, BulletDepthLevels),
    /// `SYMBOL@aggTrade`.
    AggTrade(&'a str),
    /// `SYMBOL@bookTicker`.
    BookTicker(&'a str),
    /// `bookTickers` (every market).
    AllBookTickers,
    /// `SYMBOL@markPrice`: mark, index and funding rate.
    MarkPrice(&'a str),
    /// `markPrices` (every market).
    AllMarkPrices,
    /// `SYMBOL@ticker` (coming soon).
    Ticker(&'a str),
    /// `tickers` (coming soon).
    AllTickers,
    /// `SYMBOL@kline_INTERVAL` (coming soon).
    Kline(&'a str, BulletKlineInterval),
    /// `SYMBOL@liquidations`.
    Liquidations(&'a str),
    /// `liquidations` (every market).
    AllLiquidations,
    /// `marketStatus`.
    MarketStatus,
    /// `ADDRESS@user.orders`: order and fill updates.
    UserOrders(&'a str),
}

impl BulletTopic<'_> {
    pub fn topic(&self) -> String {
        match self {
            Self::Depth(s, levels) => {
                let n = match levels {
                    BulletDepthLevels::Default => "",
                    BulletDepthLevels::L5 => "5",
                    BulletDepthLevels::L10 => "10",
                    BulletDepthLevels::L20 => "20",
                    BulletDepthLevels::L100 => "100",
                };
                format!("{s}@depth{n}")
            }
            Self::AggTrade(s) => format!("{s}@aggTrade"),
            Self::BookTicker(s) => format!("{s}@bookTicker"),
            Self::AllBookTickers => "bookTickers".to_string(),
            Self::MarkPrice(s) => format!("{s}@markPrice"),
            Self::AllMarkPrices => "markPrices".to_string(),
            Self::Ticker(s) => format!("{s}@ticker"),
            Self::AllTickers => "tickers".to_string(),
            Self::Kline(s, i) => format!("{s}@kline_{}", i.as_str()),
            Self::Liquidations(s) => format!("{s}@liquidations"),
            Self::AllLiquidations => "liquidations".to_string(),
            Self::MarketStatus => "marketStatus".to_string(),
            Self::UserOrders(a) => format!("{a}@user.orders"),
        }
    }
}

fn request(id: Option<u64>, method: &str, params: Option<Value>) -> String {
    let mut msg = json!({ "method": method });
    if let Some(id) = id {
        msg["id"] = json!(id);
    }
    if let Some(params) = params {
        msg["params"] = params;
    }
    msg.to_string()
}

fn topics(list: &[BulletTopic<'_>]) -> Value {
    Value::Array(list.iter().map(|t| Value::String(t.topic())).collect())
}

/// `{"method":"subscribe","params":[...],"id":..}`. At most 1000 topics per request and per
/// connection; one invalid topic fails the whole request.
pub fn subscribe(id: Option<u64>, list: &[BulletTopic<'_>]) -> String {
    request(id, "subscribe", Some(topics(list)))
}

/// `{"method":"unsubscribe","params":[...],"id":..}`.
pub fn unsubscribe(id: Option<u64>, list: &[BulletTopic<'_>]) -> String {
    request(id, "unsubscribe", Some(topics(list)))
}

/// `{"method":"list_subscriptions","id":..}`.
pub fn list_subscriptions(id: Option<u64>) -> String {
    request(id, "list_subscriptions", None)
}

/// Application-level `{"method":"ping","id":..}`; answered with `pong`.
pub fn ping(id: Option<u64>) -> String {
    request(id, "ping", None)
}

// ── Server messages ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum BulletWsMessage {
    Status(BulletWsStatus),
    Pong(BulletWsPong),
    Subscribe(BulletWsAck),
    Unsubscribe(BulletWsAck),
    ListSubscriptions(BulletWsSubscriptions),
    Error(BulletWsError),
    Depth(BulletWsDepth),
    AggTrade(BulletWsAggTrade),
    BookTicker(BulletWsBookTicker),
    MarkPrice(BulletWsMarkPrice),
    Liquidation(BulletWsLiquidation),
    MarketStatus(BulletWsMarketStatus),
    OrderUpdate(BulletWsOrderUpdate),
    /// Result of a websocket transaction request; `e` is the call type (`User/PlaceOrders`).
    OrderResult(BulletWsOrderResult),
    /// Failure of a websocket transaction request (no `e` field).
    OrderError(BulletWsError),
}

fn from<T: DeserializeOwned>(v: Value) -> Result<T> {
    serde_json::from_value(v).map_err(|e| anyhow!("failed to parse Bullet ws message: {e}"))
}

impl BulletWsMessage {
    pub fn parse(text: &str) -> Result<Self> {
        let v: Value =
            serde_json::from_str(text).map_err(|e| anyhow!("invalid Bullet ws JSON: {e}"))?;
        let event = match v.get("e") {
            Some(Value::String(e)) => Some(e.clone()),
            Some(other) => return Err(anyhow!("Bullet ws `e` is not a string: {other}")),
            None => None,
        };
        let event = match event {
            Some(e) => e,
            None if v.get("error").is_some() => return Ok(Self::OrderError(from(v)?)),
            None => return Err(anyhow!("Bullet ws message without `e`: {text}")),
        };
        Ok(match event.as_str() {
            "status" => Self::Status(from(v)?),
            "pong" => Self::Pong(from(v)?),
            "subscribe" => Self::Subscribe(from(v)?),
            "unsubscribe" => Self::Unsubscribe(from(v)?),
            "list_subscriptions" => Self::ListSubscriptions(from(v)?),
            "error" => Self::Error(from(v)?),
            "depthUpdate" => Self::Depth(from(v)?),
            "aggTrade" => Self::AggTrade(from(v)?),
            "bookTicker" => Self::BookTicker(from(v)?),
            "markPriceUpdate" => Self::MarkPrice(from(v)?),
            "liquidation" => Self::Liquidation(from(v)?),
            "marketStatusUpdate" => Self::MarketStatus(from(v)?),
            "orderTradeUpdate" => Self::OrderUpdate(from(v)?),
            e if e.starts_with("User/") || e.starts_with("order.") => Self::OrderResult(from(v)?),
            e => return Err(anyhow!("unknown Bullet ws event `{e}`")),
        })
    }
}

/// `status`: `connected`, or `disconnecting` with a reason (`idle_timeout`, `pong_timeout`,
/// `max_duration`, `server_shutdown`).
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct BulletWsStatus {
    #[serde(rename = "E")]
    pub event_time: i64,
    pub status: String,
    #[serde(rename = "clientId")]
    pub client_id: String,
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct BulletWsPong {
    #[serde(default)]
    pub id: Option<u64>,
    #[serde(rename = "E")]
    pub event_time: i64,
}

/// `subscribe` / `unsubscribe` acknowledgement (`result: "success"`).
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct BulletWsAck {
    #[serde(default)]
    pub id: Option<u64>,
    #[serde(rename = "E")]
    pub event_time: i64,
    pub result: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct BulletWsSubscriptions {
    #[serde(default)]
    pub id: Option<u64>,
    #[serde(rename = "E")]
    pub event_time: i64,
    pub result: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct BulletWsError {
    #[serde(default)]
    pub id: Option<u64>,
    #[serde(rename = "E")]
    pub event_time: i64,
    pub error: BulletWsErrorBody,
}

/// `code` follows Binance's numbering (-1004 bad topic, -1008 validation, -1016 service
/// unavailable, -2010 new order rejected, ...).
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct BulletWsErrorBody {
    pub code: i32,
    pub msg: String,
    #[serde(default)]
    pub param: Option<String>,
}

/// `depthUpdate`. `mt` is `s` (snapshot: replace local state) or `u` (update of the top N; a
/// level leaving the top N arrives with qty `"0"`). Check `pu` against the previous `u`.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct BulletWsDepth {
    #[serde(rename = "E")]
    pub event_time: i64,
    /// Chain time of the newest book state reflected.
    #[serde(rename = "T")]
    pub transaction_time: i64,
    #[serde(rename = "s")]
    pub symbol: String,
    #[serde(rename = "U")]
    pub first_update_id: u64,
    #[serde(rename = "u")]
    pub last_update_id: u64,
    #[serde(rename = "pu")]
    pub prev_update_id: u64,
    #[serde(rename = "b")]
    pub bids: Vec<BulletLevel>,
    #[serde(rename = "a")]
    pub asks: Vec<BulletLevel>,
    #[serde(rename = "mt")]
    pub message_type: String,
}

/// `aggTrade`. The deprecated placeholder fields (`ua`, `oi`, `mk`, `ff`, `lq`, `fe`, `nf`, `fa`,
/// `co`, `sd`, `ft`, `z`, `Z`, `rs`) carry no data and are not modelled.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct BulletWsAggTrade {
    #[serde(rename = "E")]
    pub event_time: i64,
    #[serde(rename = "s")]
    pub symbol: String,
    #[serde(rename = "a")]
    pub agg_trade_id: u64,
    #[serde(rename = "p")]
    pub price: String,
    #[serde(rename = "q")]
    pub qty: String,
    #[serde(rename = "f")]
    pub first_trade_id: u64,
    #[serde(rename = "l")]
    pub last_trade_id: u64,
    #[serde(rename = "T")]
    pub trade_time: i64,
    #[serde(rename = "m")]
    pub is_buyer_maker: bool,
    #[serde(rename = "th")]
    pub tx_hash: String,
}

/// `bookTicker`.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct BulletWsBookTicker {
    #[serde(rename = "u")]
    pub update_id: u64,
    #[serde(rename = "E")]
    pub event_time: i64,
    #[serde(rename = "T")]
    pub transaction_time: i64,
    #[serde(rename = "s")]
    pub symbol: String,
    #[serde(rename = "b")]
    pub bid_price: String,
    #[serde(rename = "B")]
    pub bid_qty: String,
    #[serde(rename = "a")]
    pub ask_price: String,
    #[serde(rename = "A")]
    pub ask_qty: String,
    #[serde(rename = "mt")]
    pub message_type: String,
}

/// `markPriceUpdate`: mark, index and funding rate (8h basis).
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct BulletWsMarkPrice {
    /// Publication time.
    #[serde(rename = "E")]
    pub event_time: i64,
    #[serde(rename = "s")]
    pub symbol: String,
    #[serde(rename = "p")]
    pub mark_price: String,
    #[serde(rename = "i")]
    pub index_price: String,
    #[serde(rename = "P", default)]
    pub estimated_settle_price: Option<String>,
    #[serde(rename = "r")]
    pub funding_rate: String,
    #[serde(rename = "T", default)]
    pub next_funding_time: Option<i64>,
    #[serde(rename = "th", default)]
    pub tx_hash: Option<String>,
    #[serde(default)]
    pub mark_execution_timestamp_us: Option<i64>,
    /// Decimal `u64` string.
    #[serde(default)]
    pub mark_event_number: Option<String>,
}

/// `liquidation`.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct BulletWsLiquidation {
    #[serde(rename = "E")]
    pub event_time: i64,
    #[serde(rename = "o")]
    pub order: BulletWsLiquidationOrder,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct BulletWsLiquidationOrder {
    #[serde(rename = "s")]
    pub symbol: String,
    #[serde(rename = "S")]
    pub side: String,
    #[serde(rename = "o")]
    pub order_type: String,
    #[serde(rename = "f")]
    pub time_in_force: String,
    #[serde(rename = "q", default)]
    pub orig_qty: Option<String>,
    #[serde(rename = "z", default)]
    pub filled_qty: Option<String>,
    #[serde(rename = "p")]
    pub price: String,
    #[serde(rename = "ap")]
    pub avg_price: String,
    #[serde(rename = "X")]
    pub status: String,
    #[serde(rename = "l")]
    pub last_filled_qty: String,
    #[serde(rename = "T")]
    pub trade_time: i64,
    #[serde(rename = "th")]
    pub tx_hash: String,
    #[serde(rename = "ua")]
    pub user_address: String,
    #[serde(rename = "oi")]
    pub order_id: u64,
    #[serde(rename = "ti")]
    pub trade_id: u64,
}

/// `marketStatusUpdate`: `mt` `s` lists every market, `u` only the changes.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct BulletWsMarketStatus {
    #[serde(rename = "E")]
    pub event_time: i64,
    #[serde(rename = "mt")]
    pub message_type: String,
    #[serde(rename = "m")]
    pub markets: Vec<BulletWsMarketStatusEntry>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct BulletWsMarketStatusEntry {
    #[serde(rename = "s")]
    pub symbol: String,
    #[serde(rename = "i")]
    pub market_id: u16,
    /// `TRADING`, `POST_ONLY`, `CANCEL_ONLY`, `HALT`, `CLOSE`.
    #[serde(rename = "st")]
    pub status: String,
    /// Previous status; only on `ch: "status"`.
    #[serde(rename = "ps", default)]
    pub previous_status: Option<String>,
    /// `listed`, `status`, `delisted`; absent in a snapshot.
    #[serde(rename = "ch", default)]
    pub change: Option<String>,
}

/// `orderTradeUpdate` (`ADDRESS@user.orders`).
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct BulletWsOrderUpdate {
    #[serde(rename = "E")]
    pub event_time: i64,
    #[serde(rename = "o")]
    pub order: BulletWsOrder,
}

/// Fields present on every execution type come first; `NEW` adds side, type, time in force,
/// price and qty; `TRADE` adds the fill fields. Other fields are `None` where they do not apply.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct BulletWsOrder {
    #[serde(rename = "s")]
    pub symbol: String,
    #[serde(rename = "i")]
    pub order_id: u64,
    /// Client order id.
    #[serde(rename = "co", default)]
    pub client_order_id: Option<u64>,
    /// Order status (`NEW`, `PARTIALLY_FILLED`, `FILLED`, `CANCELED`, ...).
    #[serde(rename = "X")]
    pub status: String,
    /// Execution type (`NEW`, `TRADE`, `CANCELED`).
    #[serde(rename = "x")]
    pub execution_type: String,
    #[serde(rename = "T")]
    pub transaction_time: i64,
    #[serde(rename = "th")]
    pub tx_hash: String,
    #[serde(rename = "S", default)]
    pub side: Option<String>,
    #[serde(rename = "o", default)]
    pub order_type: Option<String>,
    #[serde(rename = "f", default)]
    pub time_in_force: Option<String>,
    #[serde(rename = "p", default)]
    pub price: Option<String>,
    #[serde(rename = "q", default)]
    pub qty: Option<String>,
    #[serde(rename = "ap", default)]
    pub avg_price: Option<String>,
    #[serde(rename = "l", default)]
    pub last_filled_qty: Option<String>,
    #[serde(rename = "L", default)]
    pub last_filled_price: Option<String>,
    #[serde(rename = "n", default)]
    pub commission: Option<String>,
    #[serde(rename = "N", default)]
    pub commission_asset: Option<String>,
    #[serde(rename = "m", default)]
    pub is_maker: Option<bool>,
    #[serde(rename = "isDegen", default)]
    pub is_degen: Option<bool>,
    #[serde(rename = "t", default)]
    pub trade_id: Option<u64>,
    #[serde(rename = "rp", default)]
    pub realized_pnl: Option<String>,
    /// Fill type: `o` orderbook, `l` liquidation, `b` backstop, `a` ADL.
    #[serde(rename = "ft", default)]
    pub fill_type: Option<String>,
    #[serde(rename = "z", default)]
    pub cumulative_filled_qty: Option<String>,
    #[serde(rename = "Z", default)]
    pub cumulative_quote: Option<String>,
    #[serde(rename = "rs", default)]
    pub remaining_size: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct BulletWsOrderResult {
    #[serde(rename = "e")]
    pub event: String,
    #[serde(default)]
    pub id: Option<u64>,
    #[serde(rename = "E")]
    pub event_time: i64,
    pub results: BulletWsOrderResults,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct BulletWsOrderResults {
    pub tx_id: String,
    pub status: BulletTxStatus,
    pub order_ids: Vec<u64>,
    pub client_order_ids: Vec<u64>,
}
