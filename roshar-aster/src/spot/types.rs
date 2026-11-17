//! Wire types for Aster spot public market data (`/api/v3/*` REST and the
//! `wss://sstream.asterdex.com` market streams).

use crate::types::AsterKlineInterval;
use serde::{Deserialize, Deserializer, Serialize};

/// Nullable but always-sent field: the key must be present (no serde default).
fn nullable<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}

// ── REST ─────────────────────────────────────────────────────────────────────

/// `GET /api/v3/ping` returns `{}`.
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AsterSpotPing {}

/// `GET /api/v3/time`.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AsterSpotServerTime {
    #[serde(rename = "serverTime")]
    pub server_time: i64,
}

/// `GET /api/v3/exchangeInfo`.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AsterSpotExchangeInfo {
    pub timezone: String,
    #[serde(rename = "serverTime")]
    pub server_time: i64,
    #[serde(rename = "rateLimits")]
    pub rate_limits: Vec<AsterSpotRateLimit>,
    /// Always `[]` in the docs and live; element shape is undocumented.
    #[serde(rename = "exchangeFilters")]
    pub exchange_filters: Vec<serde_json::Value>,
    pub assets: Vec<AsterSpotAsset>,
    pub symbols: Vec<AsterSpotSymbol>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AsterSpotRateLimit {
    #[serde(rename = "rateLimitType")]
    pub rate_limit_type: String,
    pub interval: String,
    #[serde(rename = "intervalNum")]
    pub interval_num: u32,
    pub limit: u32,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AsterSpotAsset {
    pub asset: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AsterSpotSymbol {
    pub symbol: String,
    pub status: String,
    #[serde(rename = "baseAsset")]
    pub base_asset: String,
    #[serde(rename = "quoteAsset")]
    pub quote_asset: String,
    #[serde(rename = "pricePrecision")]
    pub price_precision: u32,
    #[serde(rename = "quantityPrecision")]
    pub quantity_precision: u32,
    #[serde(rename = "baseAssetPrecision")]
    pub base_asset_precision: u32,
    #[serde(rename = "quotePrecision")]
    pub quote_precision: u32,
    /// Not in the docs example; present on every live symbol.
    #[serde(rename = "listingTime")]
    pub listing_time: i64,
    /// Not in the docs example; present on every live symbol.
    #[serde(rename = "symbolType")]
    pub symbol_type: i64,
    /// Token contract address; `null` live for some symbols (e.g. `BNBUSDT`).
    #[serde(rename = "baseAssetAddress", deserialize_with = "nullable")]
    pub base_asset_address: Option<String>,
    pub filters: Vec<AsterSpotSymbolFilter>,
    #[serde(rename = "orderTypes")]
    pub order_types: Vec<String>,
    #[serde(rename = "timeInForce")]
    pub time_in_force: Vec<String>,
    #[serde(rename = "ocoAllowed")]
    pub oco_allowed: bool,
}

/// Symbol filters, tagged by `filterType`. An unknown filter type is a parse error.
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(tag = "filterType")]
pub enum AsterSpotSymbolFilter {
    #[serde(rename = "PRICE_FILTER")]
    PriceFilter {
        #[serde(rename = "minPrice")]
        min_price: String,
        #[serde(rename = "maxPrice")]
        max_price: String,
        #[serde(rename = "tickSize")]
        tick_size: String,
    },
    #[serde(rename = "LOT_SIZE")]
    LotSize {
        #[serde(rename = "minQty")]
        min_qty: String,
        #[serde(rename = "maxQty")]
        max_qty: String,
        #[serde(rename = "stepSize")]
        step_size: String,
    },
    #[serde(rename = "MARKET_LOT_SIZE")]
    MarketLotSize {
        #[serde(rename = "minQty")]
        min_qty: String,
        #[serde(rename = "maxQty")]
        max_qty: String,
        #[serde(rename = "stepSize")]
        step_size: String,
    },
    #[serde(rename = "MAX_NUM_ORDERS")]
    MaxNumOrders { limit: u32 },
    #[serde(rename = "MIN_NOTIONAL")]
    MinNotional {
        #[serde(rename = "minNotional")]
        min_notional: String,
    },
    #[serde(rename = "MAX_NOTIONAL")]
    MaxNotional {
        #[serde(rename = "maxNotional")]
        max_notional: String,
    },
    #[serde(rename = "NOTIONAL")]
    Notional {
        #[serde(rename = "minNotional")]
        min_notional: String,
        /// Absent live on symbols without a `MAX_NOTIONAL` filter (e.g. `TESTUSDT`).
        #[serde(rename = "maxNotional")]
        max_notional: Option<String>,
        #[serde(rename = "avgPriceMins")]
        avg_price_mins: u32,
        #[serde(rename = "applyMinToMarket")]
        apply_min_to_market: bool,
        #[serde(rename = "applyMaxToMarket")]
        apply_max_to_market: bool,
    },
    #[serde(rename = "PERCENT_PRICE")]
    PercentPrice {
        #[serde(rename = "multiplierUp")]
        multiplier_up: String,
        #[serde(rename = "multiplierDown")]
        multiplier_down: String,
        #[serde(rename = "multiplierDecimal")]
        multiplier_decimal: String,
    },
    #[serde(rename = "PERCENT_PRICE_BY_SIDE")]
    PercentPriceBySide {
        #[serde(rename = "bidMultiplierUp")]
        bid_multiplier_up: String,
        #[serde(rename = "bidMultiplierDown")]
        bid_multiplier_down: String,
        #[serde(rename = "askMultiplierUp")]
        ask_multiplier_up: String,
        #[serde(rename = "askMultiplierDown")]
        ask_multiplier_down: String,
        #[serde(rename = "avgPriceMins")]
        avg_price_mins: u32,
        #[serde(rename = "multiplierDecimal")]
        multiplier_decimal: String,
    },
}

/// `GET /api/v3/depth`. `E`, `T` and `symbol` are not in the docs example but always present live.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AsterSpotOrderBook {
    #[serde(rename = "lastUpdateId")]
    pub last_update_id: u64,
    #[serde(rename = "E")]
    pub event_time: i64,
    #[serde(rename = "T")]
    pub transaction_time: i64,
    pub symbol: String,
    /// `[price, qty]`.
    pub bids: Vec<[String; 2]>,
    /// `[price, qty]`.
    pub asks: Vec<[String; 2]>,
}

/// Element of `GET /api/v3/trades` and `GET /api/v3/historicalTrades`.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AsterSpotTrade {
    pub id: u64,
    pub price: String,
    pub qty: String,
    #[serde(rename = "baseQty")]
    pub base_qty: String,
    pub time: i64,
    #[serde(rename = "isBuyerMaker")]
    pub is_buyer_maker: bool,
}

/// Element of `GET /api/v3/aggTrades`.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AsterSpotAggTrade {
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
    pub time: i64,
    #[serde(rename = "m")]
    pub is_buyer_maker: bool,
}

/// Element of `GET /api/v3/klines`: a 12-element JSON array (live carries a
/// trailing ignore field the docs example omits). Deserialised positionally.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AsterSpotKline {
    pub open_time: i64,
    pub open: String,
    pub high: String,
    pub low: String,
    pub close: String,
    pub volume: String,
    pub close_time: i64,
    pub quote_volume: String,
    pub trade_count: u64,
    pub taker_buy_base_volume: String,
    pub taker_buy_quote_volume: String,
    pub ignore: String,
}

/// `GET /api/v3/ticker/24hr`.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AsterSpotTicker24hr {
    pub symbol: String,
    #[serde(rename = "priceChange")]
    pub price_change: String,
    #[serde(rename = "priceChangePercent")]
    pub price_change_percent: String,
    #[serde(rename = "weightedAvgPrice")]
    pub weighted_avg_price: String,
    #[serde(rename = "lastPrice")]
    pub last_price: String,
    #[serde(rename = "lastQty")]
    pub last_qty: String,
    #[serde(rename = "openPrice")]
    pub open_price: String,
    #[serde(rename = "highPrice")]
    pub high_price: String,
    #[serde(rename = "lowPrice")]
    pub low_price: String,
    pub volume: String,
    #[serde(rename = "quoteVolume")]
    pub quote_volume: String,
    #[serde(rename = "openTime")]
    pub open_time: i64,
    #[serde(rename = "closeTime")]
    pub close_time: i64,
    #[serde(rename = "firstId")]
    pub first_id: i64,
    #[serde(rename = "lastId")]
    pub last_id: i64,
    pub count: u64,
    /// The all-symbols response includes symbols not listed in `exchangeInfo`
    /// (e.g. `EVENT2_IRAQ_NUSDT`); those omit the six fields below.
    #[serde(rename = "baseAsset")]
    pub base_asset: Option<String>,
    #[serde(rename = "quoteAsset")]
    pub quote_asset: Option<String>,
    #[serde(rename = "bidPrice")]
    pub bid_price: Option<String>,
    #[serde(rename = "bidQty")]
    pub bid_qty: Option<String>,
    #[serde(rename = "askPrice")]
    pub ask_price: Option<String>,
    #[serde(rename = "askQty")]
    pub ask_qty: Option<String>,
}

/// `GET /api/v3/ticker/price`.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AsterSpotPriceTicker {
    pub symbol: String,
    pub price: String,
    pub time: i64,
    /// Present live only on some symbols (value seen: `listing_price`).
    #[serde(rename = "type")]
    pub price_type: Option<String>,
}

/// `GET /api/v3/ticker/bookTicker`.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AsterSpotBookTicker {
    pub symbol: String,
    #[serde(rename = "bidPrice")]
    pub bid_price: String,
    #[serde(rename = "bidQty")]
    pub bid_qty: String,
    #[serde(rename = "askPrice")]
    pub ask_price: String,
    #[serde(rename = "askQty")]
    pub ask_qty: String,
    pub time: i64,
}

/// REST error body `{"code":-1121,"msg":"Invalid symbol."}`.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AsterSpotApiError {
    pub code: i64,
    pub msg: String,
}

// ── WebSocket requests ───────────────────────────────────────────────────────

/// Levels of the partial book depth stream `<symbol>@depth<levels>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsterSpotDepthLevels {
    L5,
    L10,
    L20,
}

impl AsterSpotDepthLevels {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::L5 => "5",
            Self::L10 => "10",
            Self::L20 => "20",
        }
    }
}

/// Update speed of the depth streams.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsterSpotDepthSpeed {
    /// No suffix (1000ms).
    Ms1000,
    /// `@100ms`.
    Ms100,
}

impl AsterSpotDepthSpeed {
    fn suffix(&self) -> &'static str {
        match self {
            Self::Ms1000 => "",
            Self::Ms100 => "@100ms",
        }
    }
}

/// Stream name builders (symbols lowercased as the server requires).
pub mod streams {
    use super::{AsterKlineInterval, AsterSpotDepthLevels, AsterSpotDepthSpeed};

    pub fn agg_trade(symbol: &str) -> String {
        format!("{}@aggTrade", symbol.to_lowercase())
    }
    pub fn trade(symbol: &str) -> String {
        format!("{}@trade", symbol.to_lowercase())
    }
    pub fn kline(symbol: &str, interval: AsterKlineInterval) -> String {
        format!("{}@kline_{}", symbol.to_lowercase(), interval.as_str())
    }
    pub fn mini_ticker(symbol: &str) -> String {
        format!("{}@miniTicker", symbol.to_lowercase())
    }
    pub const ALL_MINI_TICKERS: &str = "!miniTicker@arr";
    pub fn ticker(symbol: &str) -> String {
        format!("{}@ticker", symbol.to_lowercase())
    }
    pub const ALL_TICKERS: &str = "!ticker@arr";
    pub fn book_ticker(symbol: &str) -> String {
        format!("{}@bookTicker", symbol.to_lowercase())
    }
    pub const ALL_BOOK_TICKERS: &str = "!bookTicker";
    pub fn partial_depth(
        symbol: &str,
        levels: AsterSpotDepthLevels,
        speed: AsterSpotDepthSpeed,
    ) -> String {
        format!(
            "{}@depth{}{}",
            symbol.to_lowercase(),
            levels.as_str(),
            speed.suffix()
        )
    }
    pub fn diff_depth(symbol: &str, speed: AsterSpotDepthSpeed) -> String {
        format!("{}@depth{}", symbol.to_lowercase(), speed.suffix())
    }
}

/// `SUBSCRIBE` / `UNSUBSCRIBE` request carrying every stream in `params`.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AsterSpotWsRequest {
    pub id: u32,
    pub method: String,
    pub params: Vec<String>,
}

impl AsterSpotWsRequest {
    pub fn subscribe(params: Vec<String>) -> Self {
        Self {
            id: 1,
            method: "SUBSCRIBE".to_string(),
            params,
        }
    }

    pub fn unsubscribe(params: Vec<String>) -> Self {
        Self {
            id: 1,
            method: "UNSUBSCRIBE".to_string(),
            params,
        }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("Failed to serialize AsterSpotWsRequest")
    }
}

// ── WebSocket payloads ───────────────────────────────────────────────────────

/// `<symbol>@aggTrade`. The docs' `M` field is absent live.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AsterSpotWsAggTrade {
    #[serde(rename = "e")]
    pub event_type: String,
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
}

/// `<symbol>@trade`.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AsterSpotWsTrade {
    #[serde(rename = "e")]
    pub event_type: String,
    #[serde(rename = "E")]
    pub event_time: i64,
    #[serde(rename = "s")]
    pub symbol: String,
    #[serde(rename = "t")]
    pub trade_id: u64,
    #[serde(rename = "p")]
    pub price: String,
    #[serde(rename = "q")]
    pub qty: String,
    #[serde(rename = "T")]
    pub trade_time: i64,
    #[serde(rename = "m")]
    pub is_buyer_maker: bool,
}

/// `<symbol>@kline_<interval>`.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AsterSpotWsKline {
    #[serde(rename = "e")]
    pub event_type: String,
    #[serde(rename = "E")]
    pub event_time: i64,
    #[serde(rename = "s")]
    pub symbol: String,
    #[serde(rename = "k")]
    pub kline: AsterSpotWsKlineData,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AsterSpotWsKlineData {
    #[serde(rename = "t")]
    pub start_time: i64,
    #[serde(rename = "T")]
    pub close_time: i64,
    #[serde(rename = "s")]
    pub symbol: String,
    #[serde(rename = "i")]
    pub interval: String,
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
    pub trade_count: u64,
    #[serde(rename = "x")]
    pub is_closed: bool,
    #[serde(rename = "q")]
    pub quote_volume: String,
    #[serde(rename = "V")]
    pub taker_buy_base_volume: String,
    #[serde(rename = "Q")]
    pub taker_buy_quote_volume: String,
    #[serde(rename = "B")]
    pub ignore: String,
}

/// `<symbol>@miniTicker` and elements of `!miniTicker@arr`.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AsterSpotWsMiniTicker {
    #[serde(rename = "e")]
    pub event_type: String,
    #[serde(rename = "E")]
    pub event_time: i64,
    #[serde(rename = "s")]
    pub symbol: String,
    #[serde(rename = "c")]
    pub close: String,
    #[serde(rename = "o")]
    pub open: String,
    #[serde(rename = "h")]
    pub high: String,
    #[serde(rename = "l")]
    pub low: String,
    #[serde(rename = "v")]
    pub volume: String,
    #[serde(rename = "q")]
    pub quote_volume: String,
}

/// `<symbol>@ticker` and elements of `!ticker@arr`.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AsterSpotWsTicker {
    #[serde(rename = "e")]
    pub event_type: String,
    #[serde(rename = "E")]
    pub event_time: i64,
    #[serde(rename = "s")]
    pub symbol: String,
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
    pub open: String,
    #[serde(rename = "h")]
    pub high: String,
    #[serde(rename = "l")]
    pub low: String,
    #[serde(rename = "v")]
    pub volume: String,
    #[serde(rename = "q")]
    pub quote_volume: String,
    #[serde(rename = "O")]
    pub open_time: i64,
    #[serde(rename = "C")]
    pub close_time: i64,
    #[serde(rename = "F")]
    pub first_trade_id: i64,
    #[serde(rename = "L")]
    pub last_trade_id: i64,
    #[serde(rename = "n")]
    pub trade_count: u64,
}

/// `<symbol>@bookTicker` and `!bookTicker`. `e`, `E`, `T` are not in the docs
/// example but present on every live frame.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AsterSpotWsBookTicker {
    #[serde(rename = "e")]
    pub event_type: String,
    #[serde(rename = "E")]
    pub event_time: i64,
    #[serde(rename = "T")]
    pub transaction_time: i64,
    #[serde(rename = "u")]
    pub update_id: u64,
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
}

/// `<symbol>@depth<levels>[@100ms]` and `<symbol>@depth[@100ms]`. Live, the
/// partial-depth stream uses `b`/`a` (not the docs' `bids`/`asks`), so both
/// streams share this shape.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AsterSpotWsDepthUpdate {
    #[serde(rename = "e")]
    pub event_type: String,
    #[serde(rename = "E")]
    pub event_time: i64,
    #[serde(rename = "T")]
    pub transaction_time: i64,
    #[serde(rename = "s")]
    pub symbol: String,
    #[serde(rename = "U")]
    pub first_update_id: u64,
    #[serde(rename = "u")]
    pub final_update_id: u64,
    #[serde(rename = "pu")]
    pub previous_final_update_id: u64,
    /// `[price, qty]`.
    #[serde(rename = "b")]
    pub bids: Vec<[String; 2]>,
    /// `[price, qty]`.
    #[serde(rename = "a")]
    pub asks: Vec<[String; 2]>,
}

/// `result` of a request response.
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(untagged)]
pub enum AsterSpotWsResult {
    /// `SUBSCRIBE` / `UNSUBSCRIBE` / `SET_PROPERTY` success (`null`).
    Null(()),
    /// `GET_PROPERTY` value.
    Bool(bool),
    /// `LIST_SUBSCRIPTIONS` stream names.
    Streams(Vec<String>),
}

/// Request error. Live shape: `{"id":5,"error":{"code":2,"msg":"..."}}`
/// (`id` is `null` for unparseable requests); the docs' flat
/// `{"code":..,"msg":..[,"id":..]}` form is accepted too.
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
pub struct AsterSpotWsError {
    pub id: Option<u64>,
    pub code: i64,
    pub msg: String,
}

/// Any frame on a raw (`/ws`) connection, or the `data` of a combined frame.
#[derive(Debug, Clone)]
pub enum AsterSpotWsMessage {
    Response { id: u64, result: AsterSpotWsResult },
    Error(AsterSpotWsError),
    AggTrade(AsterSpotWsAggTrade),
    Trade(AsterSpotWsTrade),
    Kline(AsterSpotWsKline),
    MiniTicker(AsterSpotWsMiniTicker),
    MiniTickers(Vec<AsterSpotWsMiniTicker>),
    Ticker(AsterSpotWsTicker),
    Tickers(Vec<AsterSpotWsTicker>),
    BookTicker(AsterSpotWsBookTicker),
    DepthUpdate(AsterSpotWsDepthUpdate),
}

/// Frame on a combined (`/stream?streams=`) connection: market data arrives as
/// `{"stream":..,"data":..}`; request responses/errors arrive unwrapped
/// (`stream` is `None`).
#[derive(Debug, Clone)]
pub struct AsterSpotWsCombinedMessage {
    pub stream: Option<String>,
    pub message: AsterSpotWsMessage,
}

#[derive(Deserialize)]
struct ResponseFrame {
    id: u64,
    result: AsterSpotWsResult,
}

#[derive(Deserialize)]
struct NestedError {
    code: i64,
    msg: String,
}

#[derive(Deserialize)]
struct NestedErrorFrame {
    #[serde(deserialize_with = "nullable")]
    id: Option<u64>,
    error: NestedError,
}

#[derive(Deserialize)]
struct FlatErrorFrame {
    #[serde(default)]
    id: Option<u64>,
    code: i64,
    msg: String,
}

impl AsterSpotWsMessage {
    pub fn parse(text: &str) -> Result<Self, serde_json::Error> {
        let value: serde_json::Value = serde_json::from_str(text)?;
        Self::from_value(value)
    }

    pub fn from_value(value: serde_json::Value) -> Result<Self, serde_json::Error> {
        use serde::de::Error;
        use serde_json::from_value;

        match &value {
            serde_json::Value::Array(items) => {
                let event = items
                    .first()
                    .and_then(|v| v.get("e"))
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        serde_json::Error::custom("Aster spot ws: array frame without event type")
                    })?;
                match event {
                    "24hrMiniTicker" => Ok(Self::MiniTickers(from_value(value)?)),
                    "24hrTicker" => Ok(Self::Tickers(from_value(value)?)),
                    other => Err(serde_json::Error::custom(format!(
                        "Aster spot ws: unknown array event type {other}"
                    ))),
                }
            }
            serde_json::Value::Object(obj) => {
                if obj.contains_key("result") {
                    let r: ResponseFrame = from_value(value)?;
                    return Ok(Self::Response {
                        id: r.id,
                        result: r.result,
                    });
                }
                if obj.contains_key("error") {
                    let e: NestedErrorFrame = from_value(value)?;
                    return Ok(Self::Error(AsterSpotWsError {
                        id: e.id,
                        code: e.error.code,
                        msg: e.error.msg,
                    }));
                }
                if obj.contains_key("code") {
                    let e: FlatErrorFrame = from_value(value)?;
                    return Ok(Self::Error(AsterSpotWsError {
                        id: e.id,
                        code: e.code,
                        msg: e.msg,
                    }));
                }
                let event = obj.get("e").and_then(|v| v.as_str()).ok_or_else(|| {
                    serde_json::Error::custom("Aster spot ws: object frame without event type")
                })?;
                match event {
                    "aggTrade" => Ok(Self::AggTrade(from_value(value)?)),
                    "trade" => Ok(Self::Trade(from_value(value)?)),
                    "kline" => Ok(Self::Kline(from_value(value)?)),
                    "24hrMiniTicker" => Ok(Self::MiniTicker(from_value(value)?)),
                    "24hrTicker" => Ok(Self::Ticker(from_value(value)?)),
                    "bookTicker" => Ok(Self::BookTicker(from_value(value)?)),
                    "depthUpdate" => Ok(Self::DepthUpdate(from_value(value)?)),
                    other => Err(serde_json::Error::custom(format!(
                        "Aster spot ws: unknown event type {other}"
                    ))),
                }
            }
            _ => Err(serde_json::Error::custom(
                "Aster spot ws: frame is neither object nor array",
            )),
        }
    }
}

impl AsterSpotWsCombinedMessage {
    pub fn parse(text: &str) -> Result<Self, serde_json::Error> {
        use serde::de::Error;

        let mut value: serde_json::Value = serde_json::from_str(text)?;
        if let Some(obj) = value.as_object_mut() {
            if obj.contains_key("stream") {
                let stream = match obj.remove("stream") {
                    Some(serde_json::Value::String(s)) => s,
                    _ => {
                        return Err(serde_json::Error::custom(
                            "Aster spot ws: combined `stream` is not a string",
                        ))
                    }
                };
                let data = obj.remove("data").ok_or_else(|| {
                    serde_json::Error::custom("Aster spot ws: combined frame missing `data`")
                })?;
                return Ok(Self {
                    stream: Some(stream),
                    message: AsterSpotWsMessage::from_value(data)?,
                });
            }
        }
        Ok(Self {
            stream: None,
            message: AsterSpotWsMessage::from_value(value)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! fixture {
        ($name:literal) => {
            include_str!(concat!("../../tests/fixtures/spot/", $name))
        };
    }

    fn remove_field(text: &str, pointer_parent: &str, field: &str) -> String {
        let mut v: serde_json::Value = serde_json::from_str(text).unwrap();
        v.pointer_mut(pointer_parent)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .remove(field)
            .expect("field present in fixture");
        v.to_string()
    }

    // ── REST ──

    #[test]
    fn rest_ping_and_time() {
        let _: AsterSpotPing = serde_json::from_str(fixture!("rest_ping.json")).unwrap();
        let t: AsterSpotServerTime = serde_json::from_str(fixture!("rest_time.json")).unwrap();
        assert!(t.server_time > 0);
        assert!(serde_json::from_str::<AsterSpotServerTime>("{}").is_err());
    }

    #[test]
    fn rest_exchange_info() {
        let info: AsterSpotExchangeInfo =
            serde_json::from_str(fixture!("rest_exchange_info.json")).unwrap();
        assert_eq!(info.symbols.len(), 2);
        assert!(info.symbols[0].base_asset_address.is_none());
        let aster = info
            .symbols
            .iter()
            .find(|s| s.symbol == "ASTERUSDT")
            .unwrap();
        assert!(aster.base_asset_address.is_some());
        assert!(aster
            .filters
            .iter()
            .any(|f| matches!(f, AsterSpotSymbolFilter::MaxNotional { .. })));
        let no_max = info
            .symbols
            .iter()
            .find(|s| s.symbol == "TESTUSDT")
            .unwrap();
        assert!(no_max.filters.iter().any(|f| matches!(
            f,
            AsterSpotSymbolFilter::Notional {
                max_notional: None,
                ..
            }
        )));
    }

    #[test]
    fn rest_exchange_info_strict() {
        let text = fixture!("rest_exchange_info.json");
        let missing = remove_field(text, "/symbols/0", "pricePrecision");
        assert!(serde_json::from_str::<AsterSpotExchangeInfo>(&missing).is_err());
        let missing = remove_field(text, "/symbols/0/filters/0", "tickSize");
        assert!(serde_json::from_str::<AsterSpotExchangeInfo>(&missing).is_err());
        let missing = remove_field(text, "/symbols/0", "baseAssetAddress");
        assert!(serde_json::from_str::<AsterSpotExchangeInfo>(&missing).is_err());
        let unknown = text.replacen("\"PRICE_FILTER\"", "\"NEW_FILTER\"", 1);
        assert!(serde_json::from_str::<AsterSpotExchangeInfo>(&unknown).is_err());
    }

    #[test]
    fn rest_depth() {
        let text = fixture!("rest_depth.json");
        let book: AsterSpotOrderBook = serde_json::from_str(text).unwrap();
        assert_eq!(book.symbol, "BTCUSDT");
        assert_eq!(book.bids.len(), 5);
        let missing = remove_field(text, "", "lastUpdateId");
        assert!(serde_json::from_str::<AsterSpotOrderBook>(&missing).is_err());
    }

    #[test]
    fn rest_trades() {
        for text in [
            fixture!("rest_trades.json"),
            fixture!("rest_historical_trades.json"),
        ] {
            let trades: Vec<AsterSpotTrade> = serde_json::from_str(text).unwrap();
            assert_eq!(trades.len(), 3);
            let missing = remove_field(text, "/0", "baseQty");
            assert!(serde_json::from_str::<Vec<AsterSpotTrade>>(&missing).is_err());
        }
    }

    #[test]
    fn rest_agg_trades() {
        let text = fixture!("rest_agg_trades.json");
        let trades: Vec<AsterSpotAggTrade> = serde_json::from_str(text).unwrap();
        assert_eq!(trades.len(), 3);
        let missing = remove_field(text, "/0", "f");
        assert!(serde_json::from_str::<Vec<AsterSpotAggTrade>>(&missing).is_err());
    }

    #[test]
    fn rest_klines() {
        let text = fixture!("rest_klines.json");
        let klines: Vec<AsterSpotKline> = serde_json::from_str(text).unwrap();
        assert_eq!(klines.len(), 3);
        assert!(klines[0].close_time > klines[0].open_time);
        let mut v: serde_json::Value = serde_json::from_str(text).unwrap();
        v[0].as_array_mut().unwrap().pop();
        v[0].as_array_mut().unwrap().pop();
        assert!(serde_json::from_value::<Vec<AsterSpotKline>>(v).is_err());
    }

    #[test]
    fn rest_ticker_24hr() {
        let single: AsterSpotTicker24hr =
            serde_json::from_str(fixture!("rest_ticker_24hr_single.json")).unwrap();
        assert!(single.bid_price.is_some());
        let text = fixture!("rest_ticker_24hr_all.json");
        let all: Vec<AsterSpotTicker24hr> = serde_json::from_str(text).unwrap();
        assert!(all[0].base_asset.is_some());
        assert!(all[1].base_asset.is_none());
        let missing = remove_field(text, "/1", "lastPrice");
        assert!(serde_json::from_str::<Vec<AsterSpotTicker24hr>>(&missing).is_err());
    }

    #[test]
    fn rest_price_ticker() {
        let single: AsterSpotPriceTicker =
            serde_json::from_str(fixture!("rest_ticker_price_single.json")).unwrap();
        assert_eq!(single.symbol, "BTCUSDT");
        let text = fixture!("rest_ticker_price_all.json");
        let all: Vec<AsterSpotPriceTicker> = serde_json::from_str(text).unwrap();
        assert_eq!(all[0].price_type.as_deref(), Some("listing_price"));
        assert!(all[1].price_type.is_none());
        let missing = remove_field(text, "/0", "price");
        assert!(serde_json::from_str::<Vec<AsterSpotPriceTicker>>(&missing).is_err());
    }

    #[test]
    fn rest_book_ticker() {
        let single: AsterSpotBookTicker =
            serde_json::from_str(fixture!("rest_book_ticker_single.json")).unwrap();
        assert_eq!(single.symbol, "BTCUSDT");
        let text = fixture!("rest_book_ticker_all.json");
        let all: Vec<AsterSpotBookTicker> = serde_json::from_str(text).unwrap();
        assert_eq!(all.len(), 2);
        let missing = remove_field(text, "/0", "askQty");
        assert!(serde_json::from_str::<Vec<AsterSpotBookTicker>>(&missing).is_err());
    }

    #[test]
    fn rest_error() {
        let e: AsterSpotApiError = serde_json::from_str(fixture!("rest_error.json")).unwrap();
        assert_eq!(e.code, -1121);
        assert_eq!(e.msg, "Invalid symbol.");
    }

    // ── WS ──

    fn parse(text: &str) -> AsterSpotWsMessage {
        AsterSpotWsMessage::parse(text).unwrap()
    }

    #[test]
    fn ws_events() {
        assert!(matches!(
            parse(fixture!("ws_agg_trade.json")),
            AsterSpotWsMessage::AggTrade(_)
        ));
        assert!(matches!(
            parse(fixture!("ws_trade.json")),
            AsterSpotWsMessage::Trade(_)
        ));
        match parse(fixture!("ws_kline.json")) {
            AsterSpotWsMessage::Kline(k) => assert_eq!(k.kline.interval, "1m"),
            other => panic!("unexpected {other:?}"),
        }
        assert!(matches!(
            parse(fixture!("ws_mini_ticker.json")),
            AsterSpotWsMessage::MiniTicker(_)
        ));
        assert!(matches!(
            parse(fixture!("ws_ticker.json")),
            AsterSpotWsMessage::Ticker(_)
        ));
        assert!(matches!(
            parse(fixture!("ws_book_ticker.json")),
            AsterSpotWsMessage::BookTicker(_)
        ));
        match parse(fixture!("ws_depth_partial.json")) {
            AsterSpotWsMessage::DepthUpdate(d) => {
                assert_eq!(d.bids.len(), 10);
                assert_eq!(d.asks.len(), 10);
            }
            other => panic!("unexpected {other:?}"),
        }
        match parse(fixture!("ws_depth_diff.json")) {
            AsterSpotWsMessage::DepthUpdate(d) => assert!(d.bids.is_empty()),
            other => panic!("unexpected {other:?}"),
        }
        match parse(fixture!("ws_mini_ticker_arr.json")) {
            AsterSpotWsMessage::MiniTickers(v) => assert!(!v.is_empty()),
            other => panic!("unexpected {other:?}"),
        }
        match parse(fixture!("ws_ticker_arr.json")) {
            AsterSpotWsMessage::Tickers(v) => assert!(!v.is_empty()),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn ws_events_strict() {
        let cases: [(&str, &str, &str); 9] = [
            (fixture!("ws_agg_trade.json"), "", "a"),
            (fixture!("ws_trade.json"), "", "t"),
            (fixture!("ws_kline.json"), "/k", "x"),
            (fixture!("ws_mini_ticker.json"), "", "c"),
            (fixture!("ws_ticker.json"), "", "P"),
            (fixture!("ws_book_ticker.json"), "", "u"),
            (fixture!("ws_depth_partial.json"), "", "pu"),
            (fixture!("ws_mini_ticker_arr.json"), "/0", "q"),
            (fixture!("ws_ticker_arr.json"), "/0", "n"),
        ];
        for (text, parent, field) in cases {
            let missing = remove_field(text, parent, field);
            assert!(
                AsterSpotWsMessage::parse(&missing).is_err(),
                "missing {field} should fail"
            );
        }
        assert!(AsterSpotWsMessage::parse(r#"{"e":"unknownEvent"}"#).is_err());
    }

    #[test]
    fn ws_responses_and_errors() {
        match parse(fixture!("ws_sub_result.json")) {
            AsterSpotWsMessage::Response { id, result } => {
                assert_eq!(id, 1);
                assert_eq!(result, AsterSpotWsResult::Null(()));
            }
            other => panic!("unexpected {other:?}"),
        }
        match parse(fixture!("ws_get_property_result.json")) {
            AsterSpotWsMessage::Response { result, .. } => {
                assert_eq!(result, AsterSpotWsResult::Bool(false))
            }
            other => panic!("unexpected {other:?}"),
        }
        match parse(fixture!("ws_list_subscriptions_result.json")) {
            AsterSpotWsMessage::Response { result, .. } => assert_eq!(
                result,
                AsterSpotWsResult::Streams(vec![
                    "asterusdt@aggTrade".to_string(),
                    "btcusdt@depth5".to_string()
                ])
            ),
            other => panic!("unexpected {other:?}"),
        }
        match parse(fixture!("ws_error_unknown_method.json")) {
            AsterSpotWsMessage::Error(e) => {
                assert_eq!(e.id, Some(5));
                assert_eq!(e.code, 2);
            }
            other => panic!("unexpected {other:?}"),
        }
        match parse(fixture!("ws_error_invalid_json.json")) {
            AsterSpotWsMessage::Error(e) => {
                assert_eq!(e.id, None);
                assert_eq!(e.code, 3);
            }
            other => panic!("unexpected {other:?}"),
        }
        match parse(r#"{"code": 0, "msg": "Unknown property"}"#) {
            AsterSpotWsMessage::Error(e) => assert_eq!(e.code, 0),
            other => panic!("unexpected {other:?}"),
        }
        let missing = remove_field(fixture!("ws_error_unknown_method.json"), "/error", "msg");
        assert!(AsterSpotWsMessage::parse(&missing).is_err());
        let missing = remove_field(fixture!("ws_sub_result.json"), "", "id");
        assert!(AsterSpotWsMessage::parse(&missing).is_err());
    }

    #[test]
    fn ws_combined() {
        let m = AsterSpotWsCombinedMessage::parse(fixture!("ws_combined_agg_trade.json")).unwrap();
        assert_eq!(m.stream.as_deref(), Some("asterusdt@aggTrade"));
        assert!(matches!(m.message, AsterSpotWsMessage::AggTrade(_)));
        let m = AsterSpotWsCombinedMessage::parse(fixture!("ws_combined_mini_ticker_arr.json"))
            .unwrap();
        assert_eq!(m.stream.as_deref(), Some("!miniTicker@arr"));
        assert!(matches!(m.message, AsterSpotWsMessage::MiniTickers(_)));
        let m = AsterSpotWsCombinedMessage::parse(fixture!("ws_combined_get_property_result.json"))
            .unwrap();
        assert!(m.stream.is_none());
        assert!(matches!(
            m.message,
            AsterSpotWsMessage::Response {
                result: AsterSpotWsResult::Bool(true),
                ..
            }
        ));
        let missing = remove_field(fixture!("ws_combined_agg_trade.json"), "", "data");
        assert!(AsterSpotWsCombinedMessage::parse(&missing).is_err());
    }

    #[test]
    fn stream_names() {
        assert_eq!(streams::agg_trade("BTCUSDT"), "btcusdt@aggTrade");
        assert_eq!(
            streams::kline("BTCUSDT", AsterKlineInterval::Mo1),
            "btcusdt@kline_1M"
        );
        assert_eq!(
            streams::partial_depth(
                "BTCUSDT",
                AsterSpotDepthLevels::L10,
                AsterSpotDepthSpeed::Ms100
            ),
            "btcusdt@depth10@100ms"
        );
        assert_eq!(
            streams::diff_depth("BTCUSDT", AsterSpotDepthSpeed::Ms1000),
            "btcusdt@depth"
        );
        let req = AsterSpotWsRequest::subscribe(vec![streams::trade("ETHUSDT")]);
        assert_eq!(
            req.to_json(),
            r#"{"id":1,"method":"SUBSCRIBE","params":["ethusdt@trade"]}"#
        );
    }
}
