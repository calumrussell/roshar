//! USDⓈ-M futures websocket market streams: stream names, control messages and payloads.

use super::rest::BinanceContractType;
use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};

/// Kline intervals. `S1` is accepted by spot streams and futures continuous klines only;
/// `GET /fapi/v1/klines` rejects it with `-1120 Invalid interval`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinanceKlineInterval {
    S1,
    M1,
    M3,
    M5,
    M15,
    M30,
    H1,
    H2,
    H4,
    H6,
    H8,
    H12,
    D1,
    D3,
    W1,
    /// One month (`1M`).
    Mo1,
}

impl BinanceKlineInterval {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::S1 => "1s",
            Self::M1 => "1m",
            Self::M3 => "3m",
            Self::M5 => "5m",
            Self::M15 => "15m",
            Self::M30 => "30m",
            Self::H1 => "1h",
            Self::H2 => "2h",
            Self::H4 => "4h",
            Self::H6 => "6h",
            Self::H8 => "8h",
            Self::H12 => "12h",
            Self::D1 => "1d",
            Self::D3 => "3d",
            Self::W1 => "1w",
            Self::Mo1 => "1M",
        }
    }
}

/// Update speed of the diff and partial depth streams.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinanceDepthSpeed {
    /// No suffix: 250ms on futures, 1000ms on spot.
    Default,
    /// `@500ms`: futures only.
    Ms500,
    /// `@100ms`.
    Ms100,
}

impl BinanceDepthSpeed {
    fn suffix(&self) -> &'static str {
        match self {
            Self::Default => "",
            Self::Ms500 => "@500ms",
            Self::Ms100 => "@100ms",
        }
    }
}

/// Levels of the partial depth stream `<symbol>@depth<levels>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinanceDepthLevels {
    L5,
    L10,
    L20,
}

impl BinanceDepthLevels {
    fn as_str(&self) -> &'static str {
        match self {
            Self::L5 => "5",
            Self::L10 => "10",
            Self::L20 => "20",
        }
    }
}

/// Update speed of the mark price streams.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinanceMarkPriceSpeed {
    /// No suffix: every 3s.
    S3,
    /// `@1s`.
    S1,
}

impl BinanceMarkPriceSpeed {
    fn suffix(&self) -> &'static str {
        match self {
            Self::S3 => "",
            Self::S1 => "@1s",
        }
    }
}

/// Which USDⓈ-M websocket route serves a stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinanceWsRoute {
    /// `wss://fstream.binance.com/public`: book tickers, depth, trades.
    Public,
    /// `wss://fstream.binance.com/market`: everything else.
    Market,
}

/// A USDⓈ-M market stream. Symbols are lowercased when the name is built.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BinanceStream {
    /// `<symbol>@depth[@500ms|@100ms]`.
    Depth {
        symbol: String,
        speed: BinanceDepthSpeed,
    },
    /// `<symbol>@depth<levels>[@500ms|@100ms]`.
    PartialDepth {
        symbol: String,
        levels: BinanceDepthLevels,
        speed: BinanceDepthSpeed,
    },
    /// `<symbol>@rpiDepth@500ms`: diff depth including RPI orders.
    RpiDepth { symbol: String },
    /// `<symbol>@bookTicker`.
    BookTicker { symbol: String },
    /// `!bookTicker`.
    AllBookTickers,
    /// `<symbol>@trade`. Not in the USDⓈ-M stream docs but served on `/public`.
    Trade { symbol: String },
    /// `<symbol>@aggTrade`.
    AggTrade { symbol: String },
    /// `<symbol>@markPrice[@1s]`.
    MarkPrice {
        symbol: String,
        speed: BinanceMarkPriceSpeed,
    },
    /// `!markPrice@arr[@1s]`.
    AllMarkPrices { speed: BinanceMarkPriceSpeed },
    /// `<symbol>@kline_<interval>`.
    Kline {
        symbol: String,
        interval: BinanceKlineInterval,
    },
    /// `<pair>_<contractType>@continuousKline_<interval>`.
    ContinuousKline {
        pair: String,
        contract_type: BinanceContractType,
        interval: BinanceKlineInterval,
    },
    /// `<symbol>@miniTicker`.
    MiniTicker { symbol: String },
    /// `!miniTicker@arr`.
    AllMiniTickers,
    /// `<symbol>@ticker`.
    Ticker { symbol: String },
    /// `!ticker@arr`.
    AllTickers,
    /// `<symbol>@forceOrder`.
    ForceOrder { symbol: String },
    /// `!forceOrder@arr`.
    AllForceOrders,
    /// `<symbol>@compositeIndex`.
    CompositeIndex { symbol: String },
    /// `!contractInfo`.
    ContractInfo,
    /// `<assetSymbol>@assetIndex`, e.g. `btcusd`.
    AssetIndex { asset_symbol: String },
    /// `!assetIndex@arr`.
    AllAssetIndexes,
    /// `tradingSession`: TradFi market session changes.
    TradingSession,
}

impl BinanceStream {
    /// Stream name as sent in `SUBSCRIBE` params.
    pub fn name(&self) -> String {
        match self {
            Self::Depth { symbol, speed } => {
                format!("{}@depth{}", symbol.to_lowercase(), speed.suffix())
            }
            Self::PartialDepth {
                symbol,
                levels,
                speed,
            } => format!(
                "{}@depth{}{}",
                symbol.to_lowercase(),
                levels.as_str(),
                speed.suffix()
            ),
            Self::RpiDepth { symbol } => format!("{}@rpiDepth@500ms", symbol.to_lowercase()),
            Self::BookTicker { symbol } => format!("{}@bookTicker", symbol.to_lowercase()),
            Self::AllBookTickers => "!bookTicker".to_string(),
            Self::Trade { symbol } => format!("{}@trade", symbol.to_lowercase()),
            Self::AggTrade { symbol } => format!("{}@aggTrade", symbol.to_lowercase()),
            Self::MarkPrice { symbol, speed } => {
                format!("{}@markPrice{}", symbol.to_lowercase(), speed.suffix())
            }
            Self::AllMarkPrices { speed } => format!("!markPrice@arr{}", speed.suffix()),
            Self::Kline { symbol, interval } => {
                format!("{}@kline_{}", symbol.to_lowercase(), interval.as_str())
            }
            Self::ContinuousKline {
                pair,
                contract_type,
                interval,
            } => format!(
                "{}_{}@continuousKline_{}",
                pair.to_lowercase(),
                contract_type.as_str().to_lowercase(),
                interval.as_str()
            ),
            Self::MiniTicker { symbol } => format!("{}@miniTicker", symbol.to_lowercase()),
            Self::AllMiniTickers => "!miniTicker@arr".to_string(),
            Self::Ticker { symbol } => format!("{}@ticker", symbol.to_lowercase()),
            Self::AllTickers => "!ticker@arr".to_string(),
            Self::ForceOrder { symbol } => format!("{}@forceOrder", symbol.to_lowercase()),
            Self::AllForceOrders => "!forceOrder@arr".to_string(),
            Self::CompositeIndex { symbol } => {
                format!("{}@compositeIndex", symbol.to_lowercase())
            }
            Self::ContractInfo => "!contractInfo".to_string(),
            Self::AssetIndex { asset_symbol } => {
                format!("{}@assetIndex", asset_symbol.to_lowercase())
            }
            Self::AllAssetIndexes => "!assetIndex@arr".to_string(),
            Self::TradingSession => "tradingSession".to_string(),
        }
    }

    /// Route that serves this stream. Market streams get no data on `/public`.
    pub fn route(&self) -> BinanceWsRoute {
        match self {
            Self::Depth { .. }
            | Self::PartialDepth { .. }
            | Self::RpiDepth { .. }
            | Self::BookTicker { .. }
            | Self::AllBookTickers
            | Self::Trade { .. } => BinanceWsRoute::Public,
            _ => BinanceWsRoute::Market,
        }
    }
}

/// Control message (`SUBSCRIBE`, `UNSUBSCRIBE`, `LIST_SUBSCRIPTIONS`, `ping`).
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct BinanceWssMessage {
    pub id: u32,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Vec<String>>,
}

impl BinanceWssMessage {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("failed to serialize BinanceWssMessage")
    }

    pub fn ping() -> Self {
        Self {
            id: 1,
            method: "ping".to_string(),
            params: None,
        }
    }

    /// One `SUBSCRIBE` for all `streams` (a connection accepts 10 messages per second).
    pub fn subscribe(streams: &[BinanceStream]) -> Self {
        Self {
            id: 1,
            method: "SUBSCRIBE".to_string(),
            params: Some(streams.iter().map(BinanceStream::name).collect()),
        }
    }

    pub fn unsubscribe(streams: &[BinanceStream]) -> Self {
        Self {
            id: 1,
            method: "UNSUBSCRIBE".to_string(),
            params: Some(streams.iter().map(BinanceStream::name).collect()),
        }
    }

    pub fn list_subscriptions() -> Self {
        Self {
            id: 1,
            method: "LIST_SUBSCRIPTIONS".to_string(),
            params: None,
        }
    }
}

/// Reply to a control message. `result` is `null` for `SUBSCRIBE`/`UNSUBSCRIBE` (also for unknown
/// stream names, checked live), the stream list for `LIST_SUBSCRIPTIONS`.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct BinanceWsResponse {
    pub result: serde_json::Value,
    pub id: Option<u64>,
}

/// Error reply, e.g. `{"error":{"code":2,"msg":"Invalid request: streams must be an array"},"id":1}`.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct BinanceWsError {
    pub error: BinanceWsErrorBody,
    pub id: Option<u64>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct BinanceWsErrorBody {
    pub code: i64,
    pub msg: String,
}

/// Depth update from `<symbol>@depth*`, `<symbol>@depth<levels>*` (partial book, same shape) and
/// `<symbol>@rpiDepth@500ms`. Futures-only: the spot diff-depth payload has no `pu`.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct BinanceDepthDiffMessage {
    #[serde(rename = "e")]
    pub event_type: String, // "depthUpdate"
    #[serde(rename = "E")]
    pub event_time: u64,
    #[serde(rename = "T")]
    pub transaction_time: u64,
    #[serde(rename = "s")]
    pub symbol: String,
    #[serde(rename = "ps")]
    pub pair: String,
    #[serde(rename = "U")]
    pub first_update_id: u64,
    #[serde(rename = "u")]
    pub final_update_id: u64,
    #[serde(rename = "pu")]
    pub previous_final_update_id: u64,
    #[serde(rename = "b")]
    pub bids: Vec<[String; 2]>, // [price, quantity]
    #[serde(rename = "a")]
    pub asks: Vec<[String; 2]>, // [price, quantity]
    /// Symbol type: `1` USDⓈ-M, `2` COIN-M.
    #[serde(rename = "st")]
    pub symbol_type: u8,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct BinanceKlineData {
    #[serde(rename = "t")]
    pub open_time: u64,
    #[serde(rename = "T")]
    pub close_time: u64,
    #[serde(rename = "s")]
    pub symbol: String,
    #[serde(rename = "i")]
    pub interval: String,
    /// First trade ID (-1 when no trades).
    #[serde(rename = "f")]
    pub first_trade_id: i64,
    #[serde(rename = "L")]
    pub last_trade_id: i64,
    #[serde(rename = "o")]
    pub open: String,
    #[serde(rename = "c")]
    pub close: String,
    #[serde(rename = "h")]
    pub high: String,
    #[serde(rename = "l")]
    pub low: String,
    #[serde(rename = "v")]
    pub volume: String,
    #[serde(rename = "n")]
    pub trades: u64,
    #[serde(rename = "x")]
    pub closed: bool,
    #[serde(rename = "q")]
    pub quote_volume: String,
    #[serde(rename = "V")]
    pub taker_buy_volume: String,
    #[serde(rename = "Q")]
    pub taker_buy_quote_volume: String,
    #[serde(rename = "B")]
    pub ignore: String,
}

/// `<symbol>@kline_<interval>`.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct BinanceCandleMessage {
    #[serde(rename = "e")]
    pub event_type: String, // "kline"
    #[serde(rename = "E")]
    pub event_time: u64,
    #[serde(rename = "s")]
    pub symbol: String,
    #[serde(rename = "k")]
    pub kline: BinanceKlineData,
}

/// Kline of the continuous-kline stream: no `s`; `f`/`L` are update IDs, not trade IDs.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct BinanceContinuousKlineData {
    #[serde(rename = "t")]
    pub open_time: u64,
    #[serde(rename = "T")]
    pub close_time: u64,
    #[serde(rename = "i")]
    pub interval: String,
    #[serde(rename = "f")]
    pub first_update_id: i64,
    #[serde(rename = "L")]
    pub last_update_id: i64,
    #[serde(rename = "o")]
    pub open: String,
    #[serde(rename = "c")]
    pub close: String,
    #[serde(rename = "h")]
    pub high: String,
    #[serde(rename = "l")]
    pub low: String,
    #[serde(rename = "v")]
    pub volume: String,
    #[serde(rename = "n")]
    pub trades: u64,
    #[serde(rename = "x")]
    pub closed: bool,
    #[serde(rename = "q")]
    pub quote_volume: String,
    #[serde(rename = "V")]
    pub taker_buy_volume: String,
    #[serde(rename = "Q")]
    pub taker_buy_quote_volume: String,
    #[serde(rename = "B")]
    pub ignore: String,
}

/// `<pair>_<contractType>@continuousKline_<interval>` (event `continuous_kline`).
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct BinanceContinuousKlineMessage {
    #[serde(rename = "e")]
    pub event_type: String,
    #[serde(rename = "E")]
    pub event_time: u64,
    #[serde(rename = "ps")]
    pub pair: String,
    #[serde(rename = "ct")]
    pub contract_type: String,
    #[serde(rename = "k")]
    pub kline: BinanceContinuousKlineData,
}

/// `<symbol>@trade`. Also parses the spot `@trade` stream.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct BinanceTradeMessage {
    #[serde(rename = "e")]
    pub event_type: String, // "trade"
    #[serde(rename = "E")]
    pub event_time: u64,
    #[serde(rename = "s")]
    pub symbol: String,
    #[serde(rename = "t")]
    pub trade_id: u64,
    #[serde(rename = "p")]
    pub price: String,
    #[serde(rename = "q")]
    pub qty: String,
    #[serde(rename = "T")]
    pub trade_time: u64,
    /// Trade type (`MARKET`, ...). Sent by USDⓈ-M futures; absent from the spot stream.
    #[serde(rename = "X", default, skip_serializing_if = "Option::is_none")]
    pub order_type: Option<String>,
    #[serde(rename = "m")]
    pub is_buyer_maker: bool,
    /// Symbol type (`1` USDⓈ-M, `2` COIN-M). Futures only.
    #[serde(rename = "st", default, skip_serializing_if = "Option::is_none")]
    pub symbol_type: Option<u8>,
}

/// `<symbol>@bookTicker` and `!bookTicker`. Also parses the spot stream, which only has
/// `u s b B a A`; the other fields are futures-only.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct BinanceBookTickerMessage {
    #[serde(rename = "e", default, skip_serializing_if = "Option::is_none")]
    pub event_type: Option<String>,
    #[serde(rename = "u")]
    pub update_id: u64,
    #[serde(rename = "E", default, skip_serializing_if = "Option::is_none")]
    pub event_time: Option<u64>,
    #[serde(rename = "T", default, skip_serializing_if = "Option::is_none")]
    pub transaction_time: Option<u64>,
    #[serde(rename = "s")]
    pub symbol: String,
    #[serde(rename = "ps", default, skip_serializing_if = "Option::is_none")]
    pub pair: Option<String>,
    #[serde(rename = "b")]
    pub best_bid_price: String,
    #[serde(rename = "B")]
    pub best_bid_qty: String,
    #[serde(rename = "a")]
    pub best_ask_price: String,
    #[serde(rename = "A")]
    pub best_ask_qty: String,
    #[serde(rename = "st", default, skip_serializing_if = "Option::is_none")]
    pub symbol_type: Option<u8>,
}

/// `<symbol>@aggTrade`.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct BinanceAggTradeMessage {
    #[serde(rename = "e")]
    pub event_type: String,
    #[serde(rename = "E")]
    pub event_time: u64,
    #[serde(rename = "s")]
    pub symbol: String,
    #[serde(rename = "a")]
    pub agg_trade_id: u64,
    #[serde(rename = "p")]
    pub price: String,
    #[serde(rename = "q")]
    pub quantity: String,
    /// Quantity without RPI orders.
    #[serde(rename = "nq")]
    pub normal_quantity: String,
    #[serde(rename = "f")]
    pub first_trade_id: u64,
    #[serde(rename = "l")]
    pub last_trade_id: u64,
    #[serde(rename = "T")]
    pub trade_time: u64,
    #[serde(rename = "m")]
    pub is_buyer_maker: bool,
    #[serde(rename = "st")]
    pub symbol_type: u8,
}

/// `<symbol>@markPrice[@1s]` and elements of `!markPrice@arr[@1s]` (event `markPriceUpdate`).
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct BinanceMarkPriceMessage {
    #[serde(rename = "e")]
    pub event_type: String,
    #[serde(rename = "E")]
    pub event_time: u64,
    #[serde(rename = "s")]
    pub symbol: String,
    #[serde(rename = "p")]
    pub mark_price: String,
    /// `ap`. Not described by the stream docs; equal to `p` in every capture.
    #[serde(rename = "ap")]
    pub ap: String,
    #[serde(rename = "i")]
    pub index_price: String,
    /// Only useful in the last hour before settlement.
    #[serde(rename = "P")]
    pub estimated_settle_price: String,
    #[serde(rename = "r")]
    pub funding_rate: String,
    #[serde(rename = "T")]
    pub next_funding_time: u64,
    #[serde(rename = "st")]
    pub symbol_type: u8,
}

/// `<symbol>@miniTicker` and elements of `!miniTicker@arr` (event `24hrMiniTicker`).
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct BinanceMiniTickerMessage {
    #[serde(rename = "e")]
    pub event_type: String,
    #[serde(rename = "E")]
    pub event_time: u64,
    #[serde(rename = "s")]
    pub symbol: String,
    #[serde(rename = "ps")]
    pub pair: String,
    #[serde(rename = "c")]
    pub close_price: String,
    #[serde(rename = "o")]
    pub open_price: String,
    #[serde(rename = "h")]
    pub high_price: String,
    #[serde(rename = "l")]
    pub low_price: String,
    #[serde(rename = "v")]
    pub base_volume: String,
    #[serde(rename = "q")]
    pub quote_volume: String,
    #[serde(rename = "st")]
    pub symbol_type: u8,
}

/// `<symbol>@ticker` and elements of `!ticker@arr` (event `24hrTicker`).
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct BinanceTickerMessage {
    #[serde(rename = "e")]
    pub event_type: String,
    #[serde(rename = "E")]
    pub event_time: u64,
    #[serde(rename = "s")]
    pub symbol: String,
    #[serde(rename = "ps")]
    pub pair: String,
    #[serde(rename = "p")]
    pub price_change: String,
    #[serde(rename = "P")]
    pub price_change_percent: String,
    #[serde(rename = "w")]
    pub weighted_avg_price: String,
    #[serde(rename = "c")]
    pub last_price: String,
    #[serde(rename = "Q")]
    pub last_qty: String,
    #[serde(rename = "o")]
    pub open_price: String,
    #[serde(rename = "h")]
    pub high_price: String,
    #[serde(rename = "l")]
    pub low_price: String,
    #[serde(rename = "v")]
    pub base_volume: String,
    #[serde(rename = "q")]
    pub quote_volume: String,
    #[serde(rename = "O")]
    pub open_time: u64,
    #[serde(rename = "C")]
    pub close_time: u64,
    #[serde(rename = "F")]
    pub first_trade_id: i64,
    #[serde(rename = "L")]
    pub last_trade_id: i64,
    #[serde(rename = "n")]
    pub trades: u64,
    #[serde(rename = "st")]
    pub symbol_type: u8,
}

/// `<symbol>@forceOrder` and `!forceOrder@arr` (event `forceOrder`). The all-market stream
/// pushes one object per event, not an array (checked live).
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct BinanceForceOrderMessage {
    #[serde(rename = "e")]
    pub event_type: String,
    #[serde(rename = "E")]
    pub event_time: u64,
    #[serde(rename = "o")]
    pub order: BinanceForceOrder,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct BinanceForceOrder {
    #[serde(rename = "s")]
    pub symbol: String,
    #[serde(rename = "ps")]
    pub pair: String,
    #[serde(rename = "S")]
    pub side: String,
    #[serde(rename = "o")]
    pub order_type: String,
    #[serde(rename = "f")]
    pub time_in_force: String,
    #[serde(rename = "q")]
    pub orig_qty: String,
    #[serde(rename = "p")]
    pub price: String,
    #[serde(rename = "ap")]
    pub avg_price: String,
    #[serde(rename = "X")]
    pub status: String,
    #[serde(rename = "l")]
    pub last_filled_qty: String,
    #[serde(rename = "z")]
    pub filled_accumulated_qty: String,
    #[serde(rename = "T")]
    pub trade_time: u64,
    #[serde(rename = "st")]
    pub symbol_type: u8,
}

/// `<symbol>@compositeIndex` (event `compositeIndex`).
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct BinanceCompositeIndexMessage {
    #[serde(rename = "e")]
    pub event_type: String,
    #[serde(rename = "E")]
    pub event_time: u64,
    #[serde(rename = "s")]
    pub symbol: String,
    #[serde(rename = "p")]
    pub price: String,
    /// `baseAsset` or `quoteAsset`.
    #[serde(rename = "C")]
    pub component: String,
    #[serde(rename = "c")]
    pub composition: Vec<BinanceCompositeIndexEntry>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct BinanceCompositeIndexEntry {
    #[serde(rename = "b")]
    pub base_asset: String,
    #[serde(rename = "q")]
    pub quote_asset: String,
    #[serde(rename = "w")]
    pub weight_in_quantity: String,
    #[serde(rename = "W")]
    pub weight_in_percentage: String,
    #[serde(rename = "i")]
    pub index_price: String,
}

/// `!contractInfo` (event `contractInfo`), pushed when a contract is listed, settled or its
/// brackets change. No push was seen in a 200s capture; fields follow the docs, where `ps`,
/// `bks` and `st` are not always sent.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct BinanceContractInfoMessage {
    #[serde(rename = "e")]
    pub event_type: String,
    #[serde(rename = "E")]
    pub event_time: u64,
    #[serde(rename = "s")]
    pub symbol: String,
    #[serde(rename = "ps", default, skip_serializing_if = "Option::is_none")]
    pub pair: Option<String>,
    #[serde(rename = "ct")]
    pub contract_type: String,
    #[serde(rename = "dt")]
    pub delivery_time: u64,
    #[serde(rename = "ot")]
    pub onboard_time: u64,
    #[serde(rename = "cs")]
    pub contract_status: String,
    /// Notional brackets; only sent when they change.
    #[serde(rename = "bks", default, skip_serializing_if = "Option::is_none")]
    pub brackets: Option<Vec<BinanceContractBracket>>,
    #[serde(rename = "st", default, skip_serializing_if = "Option::is_none")]
    pub symbol_type: Option<u8>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct BinanceContractBracket {
    #[serde(rename = "bs")]
    pub bracket: u32,
    #[serde(rename = "bnf")]
    pub notional_floor: f64,
    #[serde(rename = "bnc")]
    pub notional_cap: f64,
    #[serde(rename = "mmr")]
    pub maint_margin_ratio: f64,
    #[serde(rename = "cf")]
    pub cum: f64,
    #[serde(rename = "mi")]
    pub min_leverage: u32,
    #[serde(rename = "ma")]
    pub max_leverage: u32,
}

/// `<assetSymbol>@assetIndex` and elements of `!assetIndex@arr` (event `assetIndexUpdate`).
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct BinanceAssetIndexMessage {
    #[serde(rename = "e")]
    pub event_type: String,
    #[serde(rename = "E")]
    pub event_time: u64,
    #[serde(rename = "s")]
    pub symbol: String,
    #[serde(rename = "i")]
    pub index: String,
    #[serde(rename = "b")]
    pub bid_buffer: String,
    #[serde(rename = "a")]
    pub ask_buffer: String,
    #[serde(rename = "B")]
    pub bid_rate: String,
    #[serde(rename = "A")]
    pub ask_rate: String,
    #[serde(rename = "q")]
    pub auto_exchange_bid_buffer: String,
    #[serde(rename = "g")]
    pub auto_exchange_ask_buffer: String,
    #[serde(rename = "Q")]
    pub auto_exchange_bid_rate: String,
    #[serde(rename = "G")]
    pub auto_exchange_ask_rate: String,
}

/// `tradingSession`: session change of a TradFi market. The event name is `<market>Update`,
/// e.g. `FXUpdate`, `EQUITYUpdate`.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct BinanceTradingSessionMessage {
    #[serde(rename = "e")]
    pub event_type: String,
    #[serde(rename = "E")]
    pub event_time: u64,
    #[serde(rename = "t")]
    pub start_time: u64,
    #[serde(rename = "T")]
    pub end_time: u64,
    /// `REGULAR`, `PRE_MARKET`, `AFTER_MARKET`, `OVERNIGHT`, `NO_TRADING`.
    #[serde(rename = "S")]
    pub session_type: String,
}

/// Any frame of a USDⓈ-M market connection.
#[derive(Debug, Clone)]
pub enum BinanceWsEvent {
    Response(BinanceWsResponse),
    Error(BinanceWsError),
    Depth(BinanceDepthDiffMessage),
    BookTicker(BinanceBookTickerMessage),
    Trade(BinanceTradeMessage),
    AggTrade(BinanceAggTradeMessage),
    MarkPrice(BinanceMarkPriceMessage),
    MarkPrices(Vec<BinanceMarkPriceMessage>),
    Kline(BinanceCandleMessage),
    ContinuousKline(BinanceContinuousKlineMessage),
    MiniTicker(BinanceMiniTickerMessage),
    MiniTickers(Vec<BinanceMiniTickerMessage>),
    Ticker(BinanceTickerMessage),
    Tickers(Vec<BinanceTickerMessage>),
    ForceOrder(BinanceForceOrderMessage),
    CompositeIndex(BinanceCompositeIndexMessage),
    ContractInfo(BinanceContractInfoMessage),
    AssetIndex(BinanceAssetIndexMessage),
    AssetIndexes(Vec<BinanceAssetIndexMessage>),
    TradingSession(BinanceTradingSessionMessage),
}

fn from_value<T: serde::de::DeserializeOwned>(v: serde_json::Value, what: &str) -> Result<T> {
    serde_json::from_value(v).map_err(|e| anyhow!("failed to parse Binance {what}: {e}"))
}

impl BinanceWsEvent {
    /// Parses a raw (`/ws`) or combined (`/stream`, `{"stream":..,"data":..}`) frame.
    /// Unknown event types are an error.
    pub fn parse(text: &str) -> Result<Self> {
        let value: serde_json::Value = serde_json::from_str(text)
            .map_err(|e| anyhow!("failed to parse Binance ws frame: {e}"))?;
        Self::from_json(value)
    }

    fn from_json(mut value: serde_json::Value) -> Result<Self> {
        if let Some(obj) = value.as_object_mut() {
            if obj.contains_key("stream") {
                let data = obj
                    .remove("data")
                    .ok_or_else(|| anyhow!("combined Binance frame without data"))?;
                return Self::from_json(data);
            }
        }
        if let serde_json::Value::Array(items) = value {
            let event = items
                .first()
                .and_then(|v| v.get("e"))
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("Binance array frame without an event type"))?
                .to_string();
            let array = serde_json::Value::Array(items);
            return Ok(match event.as_str() {
                "markPriceUpdate" => Self::MarkPrices(from_value(array, "mark price array")?),
                "24hrMiniTicker" => Self::MiniTickers(from_value(array, "mini ticker array")?),
                "24hrTicker" => Self::Tickers(from_value(array, "ticker array")?),
                "assetIndexUpdate" => Self::AssetIndexes(from_value(array, "asset index array")?),
                other => bail!("unknown Binance array event type {other}"),
            });
        }
        if value.get("error").is_some() {
            return Ok(Self::Error(from_value(value, "ws error")?));
        }
        if value.get("result").is_some() {
            return Ok(Self::Response(from_value(value, "ws response")?));
        }
        let event = value
            .get("e")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Binance ws frame without an event type: {value}"))?
            .to_string();
        Ok(match event.as_str() {
            "depthUpdate" => Self::Depth(from_value(value, "depth update")?),
            "bookTicker" => Self::BookTicker(from_value(value, "book ticker")?),
            "trade" => Self::Trade(from_value(value, "trade")?),
            "aggTrade" => Self::AggTrade(from_value(value, "aggregate trade")?),
            "markPriceUpdate" => Self::MarkPrice(from_value(value, "mark price")?),
            "kline" => Self::Kline(from_value(value, "kline")?),
            "continuous_kline" => Self::ContinuousKline(from_value(value, "continuous kline")?),
            "24hrMiniTicker" => Self::MiniTicker(from_value(value, "mini ticker")?),
            "24hrTicker" => Self::Ticker(from_value(value, "ticker")?),
            "forceOrder" => Self::ForceOrder(from_value(value, "force order")?),
            "compositeIndex" => Self::CompositeIndex(from_value(value, "composite index")?),
            "contractInfo" => Self::ContractInfo(from_value(value, "contract info")?),
            "assetIndexUpdate" => Self::AssetIndex(from_value(value, "asset index")?),
            other if other.ends_with("Update") => {
                Self::TradingSession(from_value(value, "trading session")?)
            }
            other => bail!("unknown Binance event type {other}"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(symbol: &str) -> String {
        symbol.to_string()
    }

    #[test]
    fn test_wss_message_ping() {
        let parsed: serde_json::Value =
            serde_json::from_str(&BinanceWssMessage::ping().to_json()).unwrap();
        assert_eq!(parsed["method"], "ping");
        assert_eq!(parsed["id"], 1);
    }

    #[test]
    fn test_stream_names() {
        let cases = [
            (
                BinanceStream::Depth {
                    symbol: s("BTCUSDT"),
                    speed: BinanceDepthSpeed::Default,
                },
                "btcusdt@depth",
            ),
            (
                BinanceStream::Depth {
                    symbol: s("BTCUSDT"),
                    speed: BinanceDepthSpeed::Ms100,
                },
                "btcusdt@depth@100ms",
            ),
            (
                BinanceStream::PartialDepth {
                    symbol: s("BTCUSDT"),
                    levels: BinanceDepthLevels::L10,
                    speed: BinanceDepthSpeed::Ms500,
                },
                "btcusdt@depth10@500ms",
            ),
            (
                BinanceStream::RpiDepth {
                    symbol: s("BTCUSDT"),
                },
                "btcusdt@rpiDepth@500ms",
            ),
            (BinanceStream::AllBookTickers, "!bookTicker"),
            (
                BinanceStream::MarkPrice {
                    symbol: s("BTCUSDT"),
                    speed: BinanceMarkPriceSpeed::S1,
                },
                "btcusdt@markPrice@1s",
            ),
            (
                BinanceStream::AllMarkPrices {
                    speed: BinanceMarkPriceSpeed::S3,
                },
                "!markPrice@arr",
            ),
            (
                BinanceStream::Kline {
                    symbol: s("BTCUSDT"),
                    interval: BinanceKlineInterval::Mo1,
                },
                "btcusdt@kline_1M",
            ),
            (
                BinanceStream::ContinuousKline {
                    pair: s("BTCUSDT"),
                    contract_type: BinanceContractType::CurrentQuarter,
                    interval: BinanceKlineInterval::M1,
                },
                "btcusdt_current_quarter@continuousKline_1m",
            ),
            (BinanceStream::AllMiniTickers, "!miniTicker@arr"),
            (BinanceStream::AllTickers, "!ticker@arr"),
            (BinanceStream::AllForceOrders, "!forceOrder@arr"),
            (BinanceStream::ContractInfo, "!contractInfo"),
            (
                BinanceStream::AssetIndex {
                    asset_symbol: s("BTCUSD"),
                },
                "btcusd@assetIndex",
            ),
            (BinanceStream::AllAssetIndexes, "!assetIndex@arr"),
            (BinanceStream::TradingSession, "tradingSession"),
        ];
        for (stream, name) in cases {
            assert_eq!(stream.name(), name);
        }
        assert_eq!(
            BinanceStream::Trade { symbol: s("X") }.route(),
            BinanceWsRoute::Public
        );
        assert_eq!(
            BinanceStream::AggTrade { symbol: s("X") }.route(),
            BinanceWsRoute::Market
        );
    }

    #[test]
    fn test_subscribe_batches_one_message() {
        let msg = BinanceWssMessage::unsubscribe(&[
            BinanceStream::Trade {
                symbol: s("BTCUSDT"),
            },
            BinanceStream::Trade {
                symbol: s("ETHUSDT"),
            },
        ]);
        assert_eq!(msg.method, "UNSUBSCRIBE");
        assert_eq!(
            msg.params.unwrap(),
            vec!["btcusdt@trade".to_string(), "ethusdt@trade".to_string()]
        );
        assert_eq!(
            BinanceWssMessage::list_subscriptions().to_json(),
            r#"{"id":1,"method":"LIST_SUBSCRIPTIONS"}"#
        );
    }

    #[test]
    fn test_spot_payloads() {
        let book: BinanceBookTickerMessage = serde_json::from_str(
            r#"{"u":400900217,"s":"BNBUSDT","b":"25.35190000","B":"31.21000000","a":"25.36520000","A":"40.66000000"}"#,
        )
        .unwrap();
        assert!(book.event_type.is_none());
        assert!(book.symbol_type.is_none());
    }
}
