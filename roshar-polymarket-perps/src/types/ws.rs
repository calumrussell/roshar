//! WebSocket frames for `wss://ws.perpetuals.polymarket.com/v1/ws`.
//!
//! Requests are `{"id"?, "req": "sub"|"unsub"|"post", ...}`. Updates are
//! `{"ch", "ts", "ets", "sq", "data"}`; responses to requests are `{"id"?, "ts"?, "data"}`.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{
    Balance, BookLevel, Builder, Kline, KlineInterval, LiquidationDetails, OrderUpdate, Side,
};

/// Private channel: order state updates (requires `auth`).
pub const CHANNEL_ORDERS: &str = "orders";
/// Private channel: own fills (requires `auth`).
pub const CHANNEL_FILLS: &str = "fills";
/// Private channel: portfolio snapshots (positions, margin) (requires `auth`).
pub const CHANNEL_PORTFOLIO: &str = "portfolio";
/// Private channel: collateral balances (requires `auth`).
pub const CHANNEL_BALANCES: &str = "balances";
/// Private channel: funding payments (requires `auth`).
pub const CHANNEL_FUNDING: &str = "funding";

/// One instrument or every active instrument (`::all`), for channels that support both.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InstrumentScope {
    One(u32),
    All,
}

impl InstrumentScope {
    fn suffix(self) -> String {
        match self {
            InstrumentScope::One(iid) => iid.to_string(),
            InstrumentScope::All => "all".to_string(),
        }
    }
}

/// `bbo::{iid}`.
pub fn bbo_channel(instrument_id: u32) -> String {
    format!("bbo::{instrument_id}")
}

/// `book::{iid}` (top 20 levels per side) or `book::{iid}::{depth}`; the exchange accepts depth
/// 20 (alias of the plain channel, updates arrive on `book::{iid}`) and 50. Full snapshot every
/// 100 ms, not deltas.
pub fn book_channel(instrument_id: u32, depth: Option<u32>) -> String {
    match depth {
        Some(d) => format!("book::{instrument_id}::{d}"),
        None => format!("book::{instrument_id}"),
    }
}

/// `trades::{iid}`.
pub fn trades_channel(instrument_id: u32) -> String {
    format!("trades::{instrument_id}")
}

/// `tickers::{iid|all}`: index, mark, last, mid, open interest and funding.
pub fn tickers_channel(scope: InstrumentScope) -> String {
    format!("tickers::{}", scope.suffix())
}

/// `statistics::{iid|all}`: rolling 24 h statistics.
pub fn statistics_channel(scope: InstrumentScope) -> String {
    format!("statistics::{}", scope.suffix())
}

/// `klines::{iid}::{interval}`.
pub fn klines_channel(instrument_id: u32, interval: KlineInterval) -> String {
    format!("klines::{instrument_id}::{}", interval.as_str())
}

#[derive(Serialize)]
struct SubRequest<'a, S: AsRef<str>> {
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<u64>,
    req: &'static str,
    chs: &'a [S],
}

#[derive(Serialize)]
struct PostRequest<A: Serialize> {
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<u64>,
    req: &'static str,
    op: PostOp<A>,
}

#[derive(Serialize)]
struct PostOp<A: Serialize> {
    #[serde(rename = "type")]
    ty: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    args: Option<A>,
}

#[derive(Serialize)]
struct AuthArgs<'a> {
    proxy: &'a str,
    secret: &'a str,
}

/// `{"id"?, "req": "sub", "chs": [...]}`.
pub fn subscribe<S: AsRef<str> + Serialize>(id: Option<u64>, channels: &[S]) -> String {
    serde_json::to_string(&SubRequest {
        id,
        req: "sub",
        chs: channels,
    })
    .expect("subscribe serializes")
}

/// `{"id"?, "req": "unsub", "chs": [...]}`; use the subscribed channel names.
pub fn unsubscribe<S: AsRef<str> + Serialize>(id: Option<u64>, channels: &[S]) -> String {
    serde_json::to_string(&SubRequest {
        id,
        req: "unsub",
        chs: channels,
    })
    .expect("unsubscribe serializes")
}

/// Application-level keepalive: `{"id"?, "req": "post", "op": {"type": "ping"}}`.
pub fn ping(id: Option<u64>) -> String {
    serde_json::to_string(&PostRequest::<()> {
        id,
        req: "post",
        op: PostOp {
            ty: "ping",
            args: None,
        },
    })
    .expect("ping serializes")
}

/// Authenticates the connection for private channels with the proxy address and proxy secret.
pub fn auth(id: Option<u64>, proxy: &str, secret: &str) -> String {
    serde_json::to_string(&PostRequest {
        id,
        req: "post",
        op: PostOp {
            ty: "auth",
            args: Some(AuthArgs { proxy, secret }),
        },
    })
    .expect("auth serializes")
}

/// Any inbound frame: a channel update or a response to a request.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum WsFrame {
    Update(WsUpdate<Value>),
    Response(WsResponse<Value>),
}

/// Channel update envelope.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct WsUpdate<T> {
    pub ch: String,
    /// Server send time.
    pub ts: i64,
    /// Newest reflected event time; `0` means unknown.
    pub ets: i64,
    pub sq: u64,
    pub data: T,
}

/// Response to a `sub`/`unsub`/`post` request.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct WsResponse<T> {
    pub id: Option<u64>,
    pub ts: Option<i64>,
    pub data: T,
}

/// `data` of a `ping` response.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Pong {
    pub status: String,
    pub ts: i64,
    pub sq: u64,
}

/// `data` of an `auth` response.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "status", rename_all = "lowercase")]
pub enum AuthResult {
    /// `pod`: serving gateway pod, correlation only; older pods omit it.
    Ok {
        pod: Option<String>,
    },
    Err {
        error: String,
    },
}

/// `bbo::{iid}` data.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct WsBbo {
    pub iid: u32,
    pub bp: String,
    pub bq: String,
    pub ap: String,
    pub aq: String,
}

/// `book::{iid}[::50]` data: full snapshot.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct WsBook {
    pub b: Vec<BookLevel>,
    pub a: Vec<BookLevel>,
}

/// One element of `trades::{iid}` data.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct WsTrade {
    pub tid: u64,
    pub iid: u32,
    pub side: Side,
    pub p: String,
    pub qty: String,
    pub ts: i64,
    pub hash: String,
    pub settlement: bool,
}

/// `tickers::{iid|all}` data.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct WsTicker {
    pub iid: u32,
    pub idx: String,
    pub mark: String,
    pub last: String,
    pub mid: String,
    pub oi: String,
    /// Funding rate.
    pub fr: String,
    /// Next funding time.
    pub nxf: i64,
}

/// `statistics::{iid|all}` data.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct WsStatistics {
    pub iid: u32,
    pub vol: String,
    pub open: String,
    pub klines: Vec<Kline>,
}

/// One element of the private `fills` data.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct WsFill {
    pub tid: u64,
    pub oid: u64,
    pub iid: u32,
    pub side: Side,
    pub p: String,
    pub qty: String,
    pub taker: bool,
    pub fee: String,
    pub builder_fee: String,
    pub total_fee: String,
    pub builder: Option<Builder>,
    /// Fee asset.
    pub fea: String,
    /// Position size before the fill.
    pub psz: String,
    /// Entry price before the fill.
    pub pep: String,
    pub pnl: String,
    pub liq: bool,
    pub adl: bool,
    pub ts: i64,
    pub settlement: bool,
    pub coid: Option<String>,
    pub liquidation_details: Option<LiquidationDetails>,
}

/// Private `funding` data.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct WsFunding {
    pub id: u64,
    pub iid: u32,
    pub sz: String,
    pub fr: String,
    pub fund: String,
    pub fua: String,
    pub ts: i64,
}

/// `trades::{iid}` update.
pub type WsTradesUpdate = WsUpdate<Vec<WsTrade>>;
/// `klines::{iid}::{interval}` update.
pub type WsKlinesUpdate = WsUpdate<Vec<Kline>>;
/// `statistics::{iid|all}` update.
pub type WsStatisticsUpdate = WsUpdate<WsStatistics>;
/// Private `orders` update.
pub type WsOrdersUpdate = WsUpdate<OrderUpdate>;
/// Private `fills` update.
pub type WsFillsUpdate = WsUpdate<Vec<WsFill>>;
/// Private `balances` update.
pub type WsBalancesUpdate = WsUpdate<Balance>;
