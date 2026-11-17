use serde::{Deserialize, Serialize};
use std::fmt;

// ---------------------------------------------------------------------------------------------
// Streams, topics and requests
// ---------------------------------------------------------------------------------------------

/// Public websocket stream. Each is a separate connection with its own topic set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ByBitWsStream {
    Spot,
    /// USDT/USDC perpetuals and USDT/USDC futures.
    Linear,
    Inverse,
    /// USDT/USDC options.
    Option,
    /// Spread trading combinations.
    Spread,
    /// `system.status` maintenance notices.
    SystemStatus,
}

impl ByBitWsStream {
    pub fn url(&self) -> &'static str {
        match self {
            Self::Spot => "wss://stream.bybit.com/v5/public/spot",
            Self::Linear => "wss://stream.bybit.com/v5/public/linear",
            Self::Inverse => "wss://stream.bybit.com/v5/public/inverse",
            Self::Option => "wss://stream.bybit.com/v5/public/option",
            Self::Spread => "wss://stream.bybit.com/v5/public/spread",
            Self::SystemStatus => "wss://stream.bybit.com/v5/public/misc/status",
        }
    }

    /// roshar-ws-mgr connection name used by `ByBitClient`.
    pub fn connection_name(&self) -> &'static str {
        match self {
            Self::Spot => "bybit-spot",
            Self::Linear => "bybit-linear",
            Self::Inverse => "bybit-inverse",
            Self::Option => "bybit-option",
            Self::Spread => "bybit-spread",
            Self::SystemStatus => "bybit-system-status",
        }
    }
}

/// Depth of the `orderbook.{depth}.{symbol}` stream. Spot/linear/inverse: 1, 50, 200, 1000.
/// Option: 25, 100. Spread: 25.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ByBitOrderbookDepth {
    L1,
    L25,
    L50,
    L100,
    L200,
    L1000,
}

impl ByBitOrderbookDepth {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::L1 => "1",
            Self::L25 => "25",
            Self::L50 => "50",
            Self::L100 => "100",
            Self::L200 => "200",
            Self::L1000 => "1000",
        }
    }
}

/// Kline interval (REST `interval` and the `kline.{interval}.{symbol}` stream).
/// Numeric variants are minutes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ByBitKlineInterval {
    M1,
    M3,
    M5,
    M15,
    M30,
    M60,
    M120,
    M240,
    M360,
    M720,
    /// One day (`D`).
    Day,
    /// One week (`W`).
    Week,
    /// One month (`M`).
    Month,
}

impl ByBitKlineInterval {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::M1 => "1",
            Self::M3 => "3",
            Self::M5 => "5",
            Self::M15 => "15",
            Self::M30 => "30",
            Self::M60 => "60",
            Self::M120 => "120",
            Self::M240 => "240",
            Self::M360 => "360",
            Self::M720 => "720",
            Self::Day => "D",
            Self::Week => "W",
            Self::Month => "M",
        }
    }
}

/// A public topic. `Display` renders the wire name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ByBitTopic {
    /// `orderbook.{depth}.{symbol}` (spot, linear, inverse, option, spread).
    Orderbook {
        depth: ByBitOrderbookDepth,
        symbol: String,
    },
    /// `orderbook.rpi.{symbol}`: 50 levels incl. RPI size (spot, linear, inverse).
    OrderbookRpi { symbol: String },
    /// `orderbook.full.{symbol}`: delta-only full depth (spot, linear, inverse); seed it from
    /// `MarketApi::get_full_orderbook`.
    OrderbookFull { symbol: String },
    /// `publicTrade.{symbol}`; option streams take the base coin (e.g. `BTC`).
    PublicTrade { symbol: String },
    /// `tickers.{symbol}`.
    Ticker { symbol: String },
    /// `kline.{interval}.{symbol}` (spot, linear, inverse).
    Kline {
        interval: ByBitKlineInterval,
        symbol: String,
    },
    /// `allLiquidation.{symbol}` (linear, inverse).
    AllLiquidation { symbol: String },
    /// `insurance.{pool}`: `USDT`, `USDC` (linear stream) or `inverse` (inverse stream).
    InsurancePool { pool: String },
    /// `adlAlert.{coin}`: `USDT`, `USDC` (linear stream) or `inverse` (inverse stream).
    AdlAlert { coin: String },
    /// `priceLimit.{symbol}` (spot, linear, inverse).
    PriceLimit { symbol: String },
    /// `system.status` (system-status stream).
    SystemStatus,
}

impl fmt::Display for ByBitTopic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Orderbook { depth, symbol } => write!(f, "orderbook.{}.{symbol}", depth.as_str()),
            Self::OrderbookRpi { symbol } => write!(f, "orderbook.rpi.{symbol}"),
            Self::OrderbookFull { symbol } => write!(f, "orderbook.full.{symbol}"),
            Self::PublicTrade { symbol } => write!(f, "publicTrade.{symbol}"),
            Self::Ticker { symbol } => write!(f, "tickers.{symbol}"),
            Self::Kline { interval, symbol } => write!(f, "kline.{}.{symbol}", interval.as_str()),
            Self::AllLiquidation { symbol } => write!(f, "allLiquidation.{symbol}"),
            Self::InsurancePool { pool } => write!(f, "insurance.{pool}"),
            Self::AdlAlert { coin } => write!(f, "adlAlert.{coin}"),
            Self::PriceLimit { symbol } => write!(f, "priceLimit.{symbol}"),
            Self::SystemStatus => f.write_str("system.status"),
        }
    }
}

/// Outgoing request (`subscribe`, `unsubscribe`, `ping`).
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ByBitWssMessage {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub req_id: Option<String>,
    pub op: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub args: Option<Vec<String>>,
}

impl ByBitWssMessage {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("failed to serialize ByBitWssMessage")
    }

    pub fn ping() -> Self {
        Self {
            req_id: Some("100001".to_string()),
            op: "ping".to_string(),
            args: None,
        }
    }

    pub fn subscribe(topics: &[ByBitTopic]) -> Self {
        Self::with_topics("subscribe", topics)
    }

    pub fn unsubscribe(topics: &[ByBitTopic]) -> Self {
        Self::with_topics("unsubscribe", topics)
    }

    fn with_topics(op: &str, topics: &[ByBitTopic]) -> Self {
        Self {
            req_id: None,
            op: op.to_string(),
            args: Some(topics.iter().map(ToString::to_string).collect()),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Control replies
// ---------------------------------------------------------------------------------------------

/// Reply to `subscribe` / `unsubscribe` / `ping` on the spot, linear and inverse streams.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ByBitMessage {
    /// Echo of the request's optional `req_id`. Spot omits it when none was sent;
    /// linear returns "".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub req_id: Option<String>,
    pub op: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub args: Option<Vec<String>>,
    pub success: bool,
    /// Error text on failure, e.g. `error:handler not found,topic:orderbook.500.ETHUSDT`.
    pub ret_msg: String,
    pub conn_id: String,
}

/// Reply to `subscribe` / `unsubscribe` on the option, spread and system-status streams.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ByBitCommandResponse {
    pub success: bool,
    pub conn_id: String,
    pub data: ByBitCommandResponseData,
    /// `COMMAND_RESP`.
    #[serde(rename = "type")]
    pub msg_type: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ByBitCommandResponseData {
    pub fail_topics: Vec<String>,
    pub success_topics: Vec<String>,
}

/// Reply to `ping` on the option, spread and system-status streams: `{"args":["<ms>"],"op":"pong"}`.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ByBitPong {
    pub op: String,
    pub args: Vec<String>,
}

// ---------------------------------------------------------------------------------------------
// Order books
// ---------------------------------------------------------------------------------------------

/// `orderbook.{depth}.{symbol}` and `orderbook.full.{symbol}` (spread too).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ByBitDepthMessage {
    pub topic: String,
    /// `snapshot` or `delta` (`orderbook.full` is always `delta`).
    #[serde(rename = "type")]
    pub snapshot_type: String,
    pub ts: u64,
    /// Option stream only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub data: ByBitDepthBookData,
    pub cts: u64,
}

impl ByBitDepthMessage {
    pub fn is_full_update(&self) -> bool {
        self.snapshot_type == "snapshot"
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ByBitDepthBookData {
    pub s: String,
    /// `[price, size]`; size "0" deletes the level.
    pub b: Vec<[String; 2]>,
    pub a: Vec<[String; 2]>,
    pub u: u64,
    pub seq: u64,
}

/// `orderbook.rpi.{symbol}`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ByBitRpiDepthMessage {
    pub topic: String,
    #[serde(rename = "type")]
    pub snapshot_type: String,
    pub ts: u64,
    pub data: ByBitRpiDepthBookData,
    pub cts: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ByBitRpiDepthBookData {
    pub s: String,
    /// `[price, non-RPI size, RPI size]`.
    pub b: Vec<[String; 3]>,
    pub a: Vec<[String; 3]>,
    pub u: u64,
    pub seq: u64,
}

// ---------------------------------------------------------------------------------------------
// Trades
// ---------------------------------------------------------------------------------------------

/// `publicTrade.{symbol}` on every stream.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ByBitTradesMessage {
    pub topic: String,
    #[serde(rename = "type")]
    pub snapshot_type: String,
    pub ts: u64,
    /// Option stream only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub data: Vec<ByBitTradesData>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ByBitTradesData {
    #[serde(rename = "T")]
    pub trade_time: u64,
    pub s: String,
    /// Taker side, `Buy` / `Sell`.
    #[serde(rename = "S")]
    pub side: String,
    pub v: String,
    pub p: String,
    /// Tick direction; perps, futures and spread only.
    #[serde(rename = "L", default, skip_serializing_if = "Option::is_none")]
    pub tick_direction: Option<String>,
    pub i: String,
    /// Absent on the spread stream.
    #[serde(rename = "BT", default, skip_serializing_if = "Option::is_none")]
    pub is_block_trade: Option<bool>,
    /// Absent on the option and spread streams.
    #[serde(rename = "RPI", default, skip_serializing_if = "Option::is_none")]
    pub is_rpi_trade: Option<bool>,
    /// Cross sequence; one `seq` may span several messages (max 1024 trades each).
    pub seq: u64,
    /// Option only: mark price.
    #[serde(rename = "mP", default, skip_serializing_if = "Option::is_none")]
    pub mark_price: Option<String>,
    /// Option only: index price.
    #[serde(rename = "iP", default, skip_serializing_if = "Option::is_none")]
    pub index_price: Option<String>,
    /// Option only: mark IV.
    #[serde(rename = "mIv", default, skip_serializing_if = "Option::is_none")]
    pub mark_iv: Option<String>,
    /// Option only: trade IV.
    #[serde(rename = "iv", default, skip_serializing_if = "Option::is_none")]
    pub iv: Option<String>,
}

// ---------------------------------------------------------------------------------------------
// Tickers
// ---------------------------------------------------------------------------------------------

/// `tickers.{symbol}` on the linear / inverse streams. A `delta` carries only changed fields,
/// so everything but `symbol` is optional.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ByBitContractTickerMessage {
    pub topic: String,
    #[serde(rename = "type")]
    pub snapshot_type: String,
    pub data: ByBitContractTickerData,
    /// Cross sequence.
    pub cs: u64,
    pub ts: u64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ByBitContractTickerData {
    pub symbol: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tick_direction: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub price_24h_pcnt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_price: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prev_price_24h: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub high_price_24h: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub low_price_24h: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prev_price_1h: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mark_price: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index_price: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub open_interest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub open_interest_value: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub single_open_interest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub single_open_interest_value: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turnover_24h: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub volume_24h: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_funding_time: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub funding_rate: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bid1_price: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bid1_size: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ask1_price: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ask1_size: Option<String>,
    /// Futures only; ISO-8601 on this stream (e.g. `2026-12-25T08:00:00Z`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delivery_time: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub basis_rate: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delivery_fee_rate: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub predicted_delivery_price: Option<String>,
    /// Futures only (live; not in the docs table).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub basis: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub basis_rate_year: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pre_open_price: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pre_qty: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cur_pre_listing_phase: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub funding_interval_hour: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub funding_cap: Option<String>,
}

/// `tickers.{symbol}` on the spot stream (snapshot only).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ByBitSpotTickerMessage {
    pub topic: String,
    #[serde(rename = "type")]
    pub snapshot_type: String,
    pub ts: u64,
    pub cs: u64,
    pub data: ByBitSpotTickerData,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ByBitSpotTickerData {
    pub symbol: String,
    pub last_price: String,
    pub high_price_24h: String,
    pub low_price_24h: String,
    pub prev_price_24h: String,
    pub volume_24h: String,
    pub turnover_24h: String,
    pub price_24h_pcnt: String,
    /// Absent for pairs without a USD index (as on REST tickers).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usd_index_price: Option<String>,
}

/// `tickers.{symbol}` on the option stream (snapshot only).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ByBitOptionTickerMessage {
    pub topic: String,
    #[serde(rename = "type")]
    pub snapshot_type: String,
    pub ts: u64,
    pub id: String,
    pub data: ByBitOptionTickerData,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ByBitOptionTickerData {
    pub symbol: String,
    pub bid_price: String,
    pub bid_size: String,
    pub bid_iv: String,
    pub ask_price: String,
    pub ask_size: String,
    pub ask_iv: String,
    pub last_price: String,
    pub high_price_24h: String,
    pub low_price_24h: String,
    pub mark_price: String,
    pub index_price: String,
    pub mark_price_iv: String,
    pub underlying_price: String,
    pub open_interest: String,
    pub turnover_24h: String,
    pub volume_24h: String,
    pub total_volume: String,
    pub total_turnover: String,
    pub delta: String,
    pub gamma: String,
    pub vega: String,
    pub theta: String,
    pub predicted_delivery_price: String,
    pub change_24h: String,
}

/// `tickers.{symbol}` on the spread stream. Book fields are "" when that side is empty.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ByBitSpreadTickerMessage {
    pub topic: String,
    #[serde(rename = "type")]
    pub snapshot_type: String,
    pub ts: u64,
    pub data: ByBitSpreadTickerData,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ByBitSpreadTickerData {
    pub symbol: String,
    pub bid_price: String,
    pub bid_size: String,
    pub ask_price: String,
    pub ask_size: String,
    pub last_price: String,
    pub high_price_24h: String,
    pub low_price_24h: String,
    pub prev_price_24h: String,
    pub volume_24h: String,
}

// ---------------------------------------------------------------------------------------------
// Klines
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ByBitCandleMessage {
    pub topic: String,
    #[serde(rename = "type")]
    pub snapshot_type: String,
    pub ts: u64,
    pub data: Vec<ByBitCandleData>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ByBitCandleData {
    #[serde(rename = "start")]
    pub start_time: u64,
    #[serde(rename = "end")]
    pub end_time: u64,
    pub interval: String,
    pub open: String,
    pub high: String,
    pub low: String,
    pub close: String,
    pub volume: String,
    pub turnover: String,
    /// Whether the candle is closed.
    pub confirm: bool,
    /// Time of the last matched order in the candle.
    pub timestamp: u64,
}

// ---------------------------------------------------------------------------------------------
// Liquidations, insurance, ADL, price limits, system status
// ---------------------------------------------------------------------------------------------

/// `allLiquidation.{symbol}`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ByBitLiquidationMessage {
    pub topic: String,
    #[serde(rename = "type")]
    pub snapshot_type: String,
    pub ts: u64,
    pub data: Vec<ByBitLiquidationData>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ByBitLiquidationData {
    #[serde(rename = "T")]
    pub updated_time: u64,
    pub s: String,
    /// Position side: `Buy` means a long was liquidated.
    #[serde(rename = "S")]
    pub side: String,
    pub v: String,
    /// Bankruptcy price.
    pub p: String,
}

/// `insurance.{USDT|USDC|inverse}`. Shared pools are not pushed.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ByBitInsurancePoolMessage {
    pub topic: String,
    #[serde(rename = "type")]
    pub snapshot_type: String,
    pub ts: u64,
    pub data: Vec<ByBitInsurancePoolData>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ByBitInsurancePoolData {
    pub coin: String,
    pub symbols: String,
    pub balance: String,
    pub update_time: String,
}

/// `adlAlert.{USDT|USDC|inverse}`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ByBitAdlAlertMessage {
    pub topic: String,
    #[serde(rename = "type")]
    pub snapshot_type: String,
    pub ts: u64,
    pub data: Vec<ByBitAdlAlertData>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ByBitAdlAlertData {
    /// Insurance pool coin.
    pub c: String,
    pub s: String,
    /// Insurance pool balance.
    pub b: String,
    /// Deprecated, always "".
    pub mb: String,
    /// PnL drawdown ratio that triggers ADL.
    pub i_pr: String,
    /// Symbol's 8h PnL drawdown ratio.
    pub pr: String,
    /// Balance above which drawdown ADL can trigger.
    pub adl_tt: String,
    /// Drawdown ratio at which ADL stops.
    pub adl_sr: String,
}

/// `priceLimit.{symbol}` (no `type` field).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ByBitPriceLimitMessage {
    pub topic: String,
    pub ts: u64,
    pub data: ByBitPriceLimitData,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ByBitPriceLimitData {
    pub symbol: String,
    pub buy_lmt: String,
    pub sell_lmt: String,
}

/// `system.status`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ByBitSystemStatusMessage {
    pub topic: String,
    pub ts: u64,
    pub data: Vec<crate::rest::ByBitSystemStatus>,
}

// ---------------------------------------------------------------------------------------------
// Dispatch
// ---------------------------------------------------------------------------------------------

/// Any frame received on a public stream.
#[derive(Clone, Debug)]
pub enum ByBitWsMessage {
    /// `subscribe` / `unsubscribe` / `ping` reply on spot, linear, inverse.
    Response(ByBitMessage),
    /// `subscribe` / `unsubscribe` reply on option, spread, system-status.
    CommandResponse(ByBitCommandResponse),
    /// `ping` reply on option, spread, system-status.
    Pong(ByBitPong),
    Orderbook(ByBitDepthMessage),
    OrderbookRpi(ByBitRpiDepthMessage),
    Trades(ByBitTradesMessage),
    ContractTicker(ByBitContractTickerMessage),
    SpotTicker(ByBitSpotTickerMessage),
    OptionTicker(ByBitOptionTickerMessage),
    SpreadTicker(ByBitSpreadTickerMessage),
    Kline(ByBitCandleMessage),
    Liquidation(ByBitLiquidationMessage),
    InsurancePool(ByBitInsurancePoolMessage),
    AdlAlert(ByBitAdlAlertMessage),
    PriceLimit(ByBitPriceLimitMessage),
    SystemStatus(ByBitSystemStatusMessage),
}

impl ByBitWsMessage {
    /// Parses a text frame from `stream`. Ticker payloads differ per stream, hence the argument.
    /// Unknown topics and malformed payloads are errors.
    pub fn parse(stream: ByBitWsStream, text: &str) -> Result<Self, String> {
        let value: serde_json::Value =
            serde_json::from_str(text).map_err(|e| format!("invalid ByBit ws json: {e}"))?;
        let topic = match value.get("topic").and_then(|t| t.as_str()) {
            Some(t) => t.to_string(),
            None => return Self::parse_control(value, text),
        };
        let prefix = topic.split('.').next().unwrap_or_default();
        let second = topic.split('.').nth(1).unwrap_or_default();
        let err = |e: serde_json::Error| format!("failed to parse ByBit {topic}: {e}");
        Ok(match prefix {
            "orderbook" if second == "rpi" => {
                Self::OrderbookRpi(serde_json::from_value(value).map_err(err)?)
            }
            "orderbook" => Self::Orderbook(serde_json::from_value(value).map_err(err)?),
            "publicTrade" => Self::Trades(serde_json::from_value(value).map_err(err)?),
            "tickers" => match stream {
                ByBitWsStream::Linear | ByBitWsStream::Inverse => {
                    Self::ContractTicker(serde_json::from_value(value).map_err(err)?)
                }
                ByBitWsStream::Spot => {
                    Self::SpotTicker(serde_json::from_value(value).map_err(err)?)
                }
                ByBitWsStream::Option => {
                    Self::OptionTicker(serde_json::from_value(value).map_err(err)?)
                }
                ByBitWsStream::Spread => {
                    Self::SpreadTicker(serde_json::from_value(value).map_err(err)?)
                }
                ByBitWsStream::SystemStatus => {
                    return Err(format!(
                        "unexpected ByBit topic {topic} on system-status stream"
                    ))
                }
            },
            "kline" => Self::Kline(serde_json::from_value(value).map_err(err)?),
            "allLiquidation" => Self::Liquidation(serde_json::from_value(value).map_err(err)?),
            "insurance" => Self::InsurancePool(serde_json::from_value(value).map_err(err)?),
            "adlAlert" => Self::AdlAlert(serde_json::from_value(value).map_err(err)?),
            "priceLimit" => Self::PriceLimit(serde_json::from_value(value).map_err(err)?),
            "system" if second == "status" => {
                Self::SystemStatus(serde_json::from_value(value).map_err(err)?)
            }
            _ => return Err(format!("unknown ByBit topic {topic}")),
        })
    }

    fn parse_control(value: serde_json::Value, text: &str) -> Result<Self, String> {
        let err = |e: serde_json::Error| format!("failed to parse ByBit control frame {text}: {e}");
        if value.get("type").and_then(|t| t.as_str()) == Some("COMMAND_RESP") {
            return Ok(Self::CommandResponse(
                serde_json::from_value(value).map_err(err)?,
            ));
        }
        if value.get("op").and_then(|o| o.as_str()) == Some("pong") {
            return Ok(Self::Pong(serde_json::from_value(value).map_err(err)?));
        }
        if value.get("success").is_some() {
            return Ok(Self::Response(serde_json::from_value(value).map_err(err)?));
        }
        Err(format!("unrecognised ByBit ws frame: {text}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn topic_names() {
        let s = |symbol: &str| symbol.to_string();
        let cases = [
            (
                ByBitTopic::Orderbook {
                    depth: ByBitOrderbookDepth::L50,
                    symbol: s("BTCUSDT"),
                },
                "orderbook.50.BTCUSDT",
            ),
            (
                ByBitTopic::OrderbookRpi {
                    symbol: s("BTCUSDT"),
                },
                "orderbook.rpi.BTCUSDT",
            ),
            (
                ByBitTopic::OrderbookFull {
                    symbol: s("BTCUSDT"),
                },
                "orderbook.full.BTCUSDT",
            ),
            (
                ByBitTopic::PublicTrade { symbol: s("BTC") },
                "publicTrade.BTC",
            ),
            (
                ByBitTopic::Ticker {
                    symbol: s("BTCUSDT"),
                },
                "tickers.BTCUSDT",
            ),
            (
                ByBitTopic::Kline {
                    interval: ByBitKlineInterval::Day,
                    symbol: s("BTCUSDT"),
                },
                "kline.D.BTCUSDT",
            ),
            (
                ByBitTopic::AllLiquidation {
                    symbol: s("BTCUSDT"),
                },
                "allLiquidation.BTCUSDT",
            ),
            (
                ByBitTopic::InsurancePool { pool: s("inverse") },
                "insurance.inverse",
            ),
            (ByBitTopic::AdlAlert { coin: s("USDT") }, "adlAlert.USDT"),
            (
                ByBitTopic::PriceLimit {
                    symbol: s("BTCUSDT"),
                },
                "priceLimit.BTCUSDT",
            ),
            (ByBitTopic::SystemStatus, "system.status"),
        ];
        for (topic, name) in cases {
            assert_eq!(topic.to_string(), name);
        }
    }

    #[test]
    fn requests() {
        let ping: serde_json::Value =
            serde_json::from_str(&ByBitWssMessage::ping().to_json()).unwrap();
        assert_eq!(ping, serde_json::json!({"req_id": "100001", "op": "ping"}));

        let sub = ByBitWssMessage::subscribe(&[ByBitTopic::Ticker {
            symbol: "BTCUSDT".to_string(),
        }]);
        assert_eq!(
            sub.to_json(),
            r#"{"op":"subscribe","args":["tickers.BTCUSDT"]}"#
        );
        let unsub = ByBitWssMessage::unsubscribe(&[ByBitTopic::SystemStatus]);
        assert_eq!(
            unsub.to_json(),
            r#"{"op":"unsubscribe","args":["system.status"]}"#
        );
    }

    #[test]
    fn unknown_topic_is_error() {
        let err = ByBitWsMessage::parse(
            ByBitWsStream::Linear,
            r#"{"topic":"nope.BTCUSDT","ts":1,"data":{}}"#,
        )
        .unwrap_err();
        assert!(err.contains("unknown ByBit topic"), "{err}");
    }
}
