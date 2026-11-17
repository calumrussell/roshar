use serde::{Deserialize, Serialize};

#[cfg(test)]
mod fixture_tests;

// ── Stream parameters ────────────────────────────────────────────────────────

/// Kline intervals for `GET /fapi/v3/klines`, `indexPriceKlines`, `markPriceKlines`
/// and the `<symbol>@kline_<interval>` stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsterKlineInterval {
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

impl AsterKlineInterval {
    pub fn as_str(&self) -> &'static str {
        match self {
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

/// Update speed of the diff-depth and partial-depth streams.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsterDepthSpeed {
    /// No suffix (250ms).
    Default,
    /// `@500ms`.
    Ms500,
    /// `@100ms`.
    Ms100,
}

impl AsterDepthSpeed {
    fn suffix(&self) -> &'static str {
        match self {
            Self::Default => "",
            Self::Ms500 => "@500ms",
            Self::Ms100 => "@100ms",
        }
    }
}

/// Levels of the partial book depth stream `<symbol>@depth<levels>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsterDepthLevels {
    L5,
    L10,
    L20,
}

impl AsterDepthLevels {
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
pub enum AsterMarkPriceSpeed {
    /// No suffix (3 s).
    Default,
    /// `@1s`.
    S1,
}

impl AsterMarkPriceSpeed {
    fn suffix(&self) -> &'static str {
        match self {
            Self::Default => "",
            Self::S1 => "@1s",
        }
    }
}

// ── Stream names ─────────────────────────────────────────────────────────────

fn depth_stream(symbol: &str, speed: AsterDepthSpeed) -> String {
    format!("{}@depth{}", symbol.to_lowercase(), speed.suffix())
}

fn partial_depth_stream(symbol: &str, levels: AsterDepthLevels, speed: AsterDepthSpeed) -> String {
    format!(
        "{}@depth{}{}",
        symbol.to_lowercase(),
        levels.as_str(),
        speed.suffix()
    )
}

fn agg_trade_stream(symbol: &str) -> String {
    format!("{}@aggTrade", symbol.to_lowercase())
}

fn mark_price_stream(symbol: &str, speed: AsterMarkPriceSpeed) -> String {
    format!("{}@markPrice{}", symbol.to_lowercase(), speed.suffix())
}

fn kline_stream(symbol: &str, interval: AsterKlineInterval) -> String {
    format!("{}@kline_{}", symbol.to_lowercase(), interval.as_str())
}

fn symbol_stream(symbol: &str, suffix: &str) -> String {
    format!("{}@{suffix}", symbol.to_lowercase())
}

/// Subscribe / unsubscribe request for Aster market streams.
///
/// One request carries every stream in `params`: the server accepts at most 10
/// incoming messages per second and 200 streams per connection. Keep-alive is
/// WebSocket ping/pong frames; the server has no JSON ping method.
/// `params` holds the stream names, so it can also feed [`combined_stream_url`].
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AsterWssMessage {
    pub id: u32,
    pub method: String,
    pub params: Vec<String>,
}

impl AsterWssMessage {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("Failed to serialize AsterWssMessage")
    }

    fn request(method: &str, params: Vec<String>) -> Self {
        Self {
            id: 1,
            method: method.to_string(),
            params,
        }
    }

    fn sub(params: Vec<String>) -> Self {
        Self::request("SUBSCRIBE", params)
    }

    fn unsub(params: Vec<String>) -> Self {
        Self::request("UNSUBSCRIBE", params)
    }

    pub fn depth(symbols: &[&str], speed: AsterDepthSpeed) -> Self {
        Self::sub(symbols.iter().map(|s| depth_stream(s, speed)).collect())
    }

    pub fn depth_unsub(symbols: &[&str], speed: AsterDepthSpeed) -> Self {
        Self::unsub(symbols.iter().map(|s| depth_stream(s, speed)).collect())
    }

    pub fn partial_depth(
        symbols: &[&str],
        levels: AsterDepthLevels,
        speed: AsterDepthSpeed,
    ) -> Self {
        Self::sub(
            symbols
                .iter()
                .map(|s| partial_depth_stream(s, levels, speed))
                .collect(),
        )
    }

    pub fn partial_depth_unsub(
        symbols: &[&str],
        levels: AsterDepthLevels,
        speed: AsterDepthSpeed,
    ) -> Self {
        Self::unsub(
            symbols
                .iter()
                .map(|s| partial_depth_stream(s, levels, speed))
                .collect(),
        )
    }

    pub fn trades(symbols: &[&str]) -> Self {
        Self::sub(symbols.iter().map(|s| agg_trade_stream(s)).collect())
    }

    pub fn trades_unsub(symbols: &[&str]) -> Self {
        Self::unsub(symbols.iter().map(|s| agg_trade_stream(s)).collect())
    }

    pub fn mark_price(symbols: &[&str], speed: AsterMarkPriceSpeed) -> Self {
        Self::sub(
            symbols
                .iter()
                .map(|s| mark_price_stream(s, speed))
                .collect(),
        )
    }

    pub fn mark_price_unsub(symbols: &[&str], speed: AsterMarkPriceSpeed) -> Self {
        Self::unsub(
            symbols
                .iter()
                .map(|s| mark_price_stream(s, speed))
                .collect(),
        )
    }

    /// `!markPrice@arr[@1s]`.
    pub fn all_mark_prices(speed: AsterMarkPriceSpeed) -> Self {
        Self::sub(vec![format!("!markPrice@arr{}", speed.suffix())])
    }

    pub fn all_mark_prices_unsub(speed: AsterMarkPriceSpeed) -> Self {
        Self::unsub(vec![format!("!markPrice@arr{}", speed.suffix())])
    }

    pub fn klines(symbols: &[&str], interval: AsterKlineInterval) -> Self {
        Self::sub(symbols.iter().map(|s| kline_stream(s, interval)).collect())
    }

    pub fn klines_unsub(symbols: &[&str], interval: AsterKlineInterval) -> Self {
        Self::unsub(symbols.iter().map(|s| kline_stream(s, interval)).collect())
    }

    pub fn mini_tickers(symbols: &[&str]) -> Self {
        Self::sub(
            symbols
                .iter()
                .map(|s| symbol_stream(s, "miniTicker"))
                .collect(),
        )
    }

    pub fn mini_tickers_unsub(symbols: &[&str]) -> Self {
        Self::unsub(
            symbols
                .iter()
                .map(|s| symbol_stream(s, "miniTicker"))
                .collect(),
        )
    }

    /// `!miniTicker@arr`.
    pub fn all_mini_tickers() -> Self {
        Self::sub(vec!["!miniTicker@arr".to_string()])
    }

    pub fn all_mini_tickers_unsub() -> Self {
        Self::unsub(vec!["!miniTicker@arr".to_string()])
    }

    pub fn tickers(symbols: &[&str]) -> Self {
        Self::sub(symbols.iter().map(|s| symbol_stream(s, "ticker")).collect())
    }

    pub fn tickers_unsub(symbols: &[&str]) -> Self {
        Self::unsub(symbols.iter().map(|s| symbol_stream(s, "ticker")).collect())
    }

    /// `!ticker@arr`.
    pub fn all_tickers() -> Self {
        Self::sub(vec!["!ticker@arr".to_string()])
    }

    pub fn all_tickers_unsub() -> Self {
        Self::unsub(vec!["!ticker@arr".to_string()])
    }

    pub fn book_tickers(symbols: &[&str]) -> Self {
        Self::sub(
            symbols
                .iter()
                .map(|s| symbol_stream(s, "bookTicker"))
                .collect(),
        )
    }

    pub fn book_tickers_unsub(symbols: &[&str]) -> Self {
        Self::unsub(
            symbols
                .iter()
                .map(|s| symbol_stream(s, "bookTicker"))
                .collect(),
        )
    }

    /// `!bookTicker`.
    pub fn all_book_tickers() -> Self {
        Self::sub(vec!["!bookTicker".to_string()])
    }

    pub fn all_book_tickers_unsub() -> Self {
        Self::unsub(vec!["!bookTicker".to_string()])
    }

    pub fn liquidations(symbols: &[&str]) -> Self {
        Self::sub(
            symbols
                .iter()
                .map(|s| symbol_stream(s, "forceOrder"))
                .collect(),
        )
    }

    pub fn liquidations_unsub(symbols: &[&str]) -> Self {
        Self::unsub(
            symbols
                .iter()
                .map(|s| symbol_stream(s, "forceOrder"))
                .collect(),
        )
    }

    /// `!forceOrder@arr`.
    pub fn all_liquidations() -> Self {
        Self::sub(vec!["!forceOrder@arr".to_string()])
    }

    pub fn all_liquidations_unsub() -> Self {
        Self::unsub(vec!["!forceOrder@arr".to_string()])
    }
}

/// `{"method":"LIST_SUBSCRIPTIONS","id":<id>}`. The reply is a
/// [`AsterWsMessage::Response`] whose result is [`AsterWsResult::Subscriptions`].
pub fn list_subscriptions_request(id: u32) -> String {
    format!(r#"{{"id":{id},"method":"LIST_SUBSCRIPTIONS"}}"#)
}

/// `{"method":"GET_PROPERTY","params":["combined"],"id":<id>}`. The reply result is
/// [`AsterWsResult::Property`].
pub fn get_combined_property_request(id: u32) -> String {
    format!(r#"{{"id":{id},"method":"GET_PROPERTY","params":["combined"]}}"#)
}

/// `{"method":"SET_PROPERTY","params":["combined",<combined>],"id":<id>}`. When `true`,
/// payloads arrive wrapped as [`AsterWsMessage::Combined`].
pub fn set_combined_property_request(combined: bool, id: u32) -> String {
    format!(r#"{{"id":{id},"method":"SET_PROPERTY","params":["combined",{combined}]}}"#)
}

/// Combined-stream URL `wss://fstream.asterdex.com/stream?streams=<a>/<b>/...`.
/// Payloads on it are wrapped as `{"stream":...,"data":...}`. An empty `streams` gives the
/// bare `/stream` endpoint, which accepts `SUBSCRIBE` later.
pub fn combined_stream_url(streams: &[String]) -> String {
    if streams.is_empty() {
        crate::ASTER_COMBINED_WSS_URL.to_string()
    } else {
        format!(
            "{}?streams={}",
            crate::ASTER_COMBINED_WSS_URL,
            streams.join("/")
        )
    }
}

// ── REST response types ───────────────────────────────────────────────────────

/// Error body returned with non-2xx statuses, e.g. `{"code":-1121,"msg":"Invalid symbol."}`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsterApiError {
    pub code: i64,
    pub msg: String,
}

/// `GET /fapi/v3/time`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsterServerTime {
    #[serde(rename = "serverTime")]
    pub server_time: i64,
}

/// `GET /fapi/v3/exchangeInfo`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsterExchangeInfo {
    pub timezone: String,
    /// The docs say to ignore this and use `GET /fapi/v3/time`.
    #[serde(rename = "serverTime")]
    pub server_time: i64,
    #[serde(rename = "futuresType")]
    pub futures_type: String,
    #[serde(rename = "rateLimits")]
    pub rate_limits: Vec<AsterRateLimit>,
    /// Always empty in the docs and live; no element shape is documented.
    #[serde(rename = "exchangeFilters")]
    pub exchange_filters: Vec<serde_json::Value>,
    pub assets: Vec<AsterAssetInfo>,
    pub symbols: Vec<AsterSymbolInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsterRateLimit {
    #[serde(rename = "rateLimitType")]
    pub rate_limit_type: String,
    pub interval: String,
    #[serde(rename = "intervalNum")]
    pub interval_num: u32,
    pub limit: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsterAssetInfo {
    pub asset: String,
    /// Whether the asset can be used as margin in Multi-Assets mode.
    #[serde(rename = "marginAvailable")]
    pub margin_available: bool,
    /// Auto-exchange threshold in Multi-Assets mode. The docs show `null` for some assets;
    /// live values are decimal strings.
    #[serde(rename = "autoAssetExchange", deserialize_with = "nullable")]
    pub auto_asset_exchange: Option<String>,
    /// Live only (`crypto`, `stable_coin`, `stock`, `other`); not in the docs.
    #[serde(rename = "assetType")]
    pub asset_type: String,
}

/// One entry of `exchangeInfo.symbols`. Fields marked "live only" are not in the docs
/// but are present on every symbol.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsterSymbolInfo {
    pub symbol: String,
    pub pair: String,
    /// `PERPETUAL`; live data also has `""` on symbols that are not trading yet.
    #[serde(rename = "contractType")]
    pub contract_type: String,
    #[serde(rename = "deliveryDate")]
    pub delivery_date: i64,
    #[serde(rename = "onboardDate")]
    pub onboard_date: i64,
    /// Live only.
    #[serde(rename = "createTime")]
    pub create_time: i64,
    /// For example `TRADING`, `PENDING_TRADING`, `SETTLING`.
    pub status: String,
    /// The docs say to ignore this.
    #[serde(rename = "maintMarginPercent")]
    pub maint_margin_percent: String,
    /// The docs say to ignore this.
    #[serde(rename = "requiredMarginPercent")]
    pub required_margin_percent: String,
    #[serde(rename = "baseAsset")]
    pub base_asset: String,
    #[serde(rename = "quoteAsset")]
    pub quote_asset: String,
    #[serde(rename = "marginAsset")]
    pub margin_asset: String,
    /// Not the tick size; use the `PRICE_FILTER` filter.
    #[serde(rename = "pricePrecision")]
    pub price_precision: u32,
    /// Not the step size; use the `LOT_SIZE` filter.
    #[serde(rename = "quantityPrecision")]
    pub quantity_precision: u32,
    #[serde(rename = "baseAssetPrecision")]
    pub base_asset_precision: u32,
    #[serde(rename = "quotePrecision")]
    pub quote_precision: u32,
    #[serde(rename = "underlyingType")]
    pub underlying_type: String,
    #[serde(rename = "underlyingSubType")]
    pub underlying_sub_type: Vec<String>,
    /// Live only.
    #[serde(rename = "symbolType")]
    pub symbol_type: i64,
    /// Live only.
    #[serde(rename = "tradingMode")]
    pub trading_mode: i64,
    /// Live only; usually `""`, for example `Gold` on commodity symbols.
    pub name: String,
    /// Live only; for example `""`, `{}` or `nasdaq`.
    pub channel: String,
    /// Live only.
    #[serde(rename = "sequenceNo")]
    pub sequence_no: i64,
    /// Live only.
    #[serde(rename = "twapMinNotional")]
    pub twap_min_notional: String,
    /// Live only.
    pub imn: String,
    /// Live only.
    pub tags: Vec<String>,
    #[serde(rename = "settlePlan")]
    pub settle_plan: i64,
    /// Threshold for algo orders with `priceProtect`.
    #[serde(rename = "triggerProtect")]
    pub trigger_protect: String,
    #[serde(rename = "liquidationFee")]
    pub liquidation_fee: String,
    /// Max price difference rate from mark price a market order can make.
    #[serde(rename = "marketTakeBound")]
    pub market_take_bound: String,
    pub filters: Vec<AsterSymbolFilter>,
    #[serde(rename = "orderTypes")]
    pub order_types: Vec<String>,
    #[serde(rename = "timeInForce")]
    pub time_in_force: Vec<String>,
}

/// Symbol filters, tagged by `filterType`. An unknown filter type is a parse error.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "filterType")]
pub enum AsterSymbolFilter {
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
    #[serde(rename = "MAX_NUM_ALGO_ORDERS")]
    MaxNumAlgoOrders { limit: u32 },
    #[serde(rename = "MIN_NOTIONAL")]
    MinNotional { notional: String },
    #[serde(rename = "PERCENT_PRICE")]
    PercentPrice {
        #[serde(rename = "multiplierUp")]
        multiplier_up: String,
        #[serde(rename = "multiplierDown")]
        multiplier_down: String,
        /// A string live; the docs show an integer.
        #[serde(rename = "multiplierDecimal")]
        multiplier_decimal: String,
        /// Live only.
        #[serde(rename = "ltMultiplierUp")]
        lt_multiplier_up: String,
        /// Live only.
        #[serde(rename = "ltMultiplierDown")]
        lt_multiplier_down: String,
    },
}

/// Trade from `GET /fapi/v3/trades` and `GET /fapi/v3/historicalTrades`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsterTrade {
    pub id: u64,
    pub price: String,
    pub qty: String,
    #[serde(rename = "quoteQty")]
    pub quote_qty: String,
    pub time: i64,
    #[serde(rename = "isBuyerMaker")]
    pub is_buyer_maker: bool,
}

/// Aggregate trade from `GET /fapi/v3/aggTrades`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsterAggTrade {
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

/// Mark price and funding rate for a single symbol (`GET /fapi/v3/premiumIndex`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsterMarkPrice {
    pub symbol: String,
    #[serde(rename = "markPrice")]
    pub mark_price: String,
    #[serde(rename = "indexPrice")]
    pub index_price: String,
    /// Only meaningful in the last hour before settlement.
    #[serde(rename = "estimatedSettlePrice")]
    pub estimated_settle_price: String,
    #[serde(rename = "lastFundingRate")]
    pub last_funding_rate: String,
    #[serde(rename = "interestRate")]
    pub interest_rate: String,
    #[serde(rename = "nextFundingTime")]
    pub next_funding_time: i64,
    pub time: i64,
}

/// Historical funding rate entry (`GET /fapi/v3/fundingRate`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsterFundingRate {
    pub symbol: String,
    #[serde(rename = "fundingRate")]
    pub funding_rate: String,
    #[serde(rename = "fundingTime")]
    pub funding_time: i64,
}

/// Funding config for a symbol (`GET /fapi/v3/fundingInfo`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsterFundingInfo {
    pub symbol: String,
    #[serde(rename = "interestRate")]
    pub interest_rate: String,
    pub time: i64,
    #[serde(rename = "fundingIntervalHours")]
    pub funding_interval_hours: u32,
    /// JSON number on the wire (for example `0.02`).
    #[serde(rename = "fundingFeeCap")]
    pub funding_fee_cap: f64,
    /// JSON number on the wire (for example `-0.02`).
    #[serde(rename = "fundingFeeFloor")]
    pub funding_fee_floor: f64,
}

/// 24-hour ticker statistics (`GET /fapi/v3/ticker/24hr`). The docs also list
/// `prevClosePrice`, but the live API does not send it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsterTicker24hr {
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
    pub volume: String,
    #[serde(rename = "quoteVolume")]
    pub quote_volume: String,
    #[serde(rename = "openTime")]
    pub open_time: i64,
    #[serde(rename = "closeTime")]
    pub close_time: i64,
    #[serde(rename = "openPrice")]
    pub open_price: String,
    #[serde(rename = "highPrice")]
    pub high_price: String,
    #[serde(rename = "lowPrice")]
    pub low_price: String,
    #[serde(rename = "firstId")]
    pub first_id: i64,
    #[serde(rename = "lastId")]
    pub last_id: i64,
    pub count: u64,
}

/// Latest price (`GET /fapi/v3/ticker/price`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsterPriceTicker {
    pub symbol: String,
    pub price: String,
    /// Transaction time.
    pub time: i64,
}

/// Best bid/ask (`GET /fapi/v3/ticker/bookTicker`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsterBookTicker {
    pub symbol: String,
    #[serde(rename = "bidPrice")]
    pub bid_price: String,
    #[serde(rename = "bidQty")]
    pub bid_qty: String,
    #[serde(rename = "askPrice")]
    pub ask_price: String,
    #[serde(rename = "askQty")]
    pub ask_qty: String,
    /// Transaction time.
    pub time: i64,
    /// Live only; not in the docs.
    #[serde(rename = "lastUpdateId")]
    pub last_update_id: u64,
}

/// Index price components (`GET /fapi/v3/indexreferences`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsterIndexReferences {
    pub symbol: String,
    pub time: i64,
    pub references: Vec<AsterIndexReference>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsterIndexReference {
    /// Source exchange, for example `binance`.
    pub exchange: String,
    /// Symbol on the source exchange.
    pub symbol: String,
    pub weight: String,
}

/// `GET /fapi/v3/remainingOpenableNotionalValue`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsterRemainingOpenableNotional {
    /// USDT notional still openable at the leverage tier; `"-1"` means no cap.
    #[serde(rename = "remainingOpenableNotionalValue")]
    pub remaining_openable_notional_value: String,
}

/// `GET /fapi/v3/openInterest`. Live but not in the V3 docs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsterOpenInterest {
    pub symbol: String,
    #[serde(rename = "openInterest")]
    pub open_interest: String,
    pub time: i64,
}

/// For fields that are always sent but may be `null`: unlike a bare `Option`, a missing key
/// is an error.
fn nullable<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}

fn kline_row<'a, E: serde::de::Error>(
    name: &'static str,
    value: &'a serde_json::Value,
) -> Result<&'a Vec<serde_json::Value>, E> {
    value
        .as_array()
        .ok_or_else(|| E::custom(format!("{name}: expected array")))
}

fn row_int<E: serde::de::Error>(
    name: &'static str,
    a: &[serde_json::Value],
    i: usize,
) -> Result<i64, E> {
    a.get(i)
        .ok_or_else(|| E::custom(format!("{name}: missing index {i}")))?
        .as_i64()
        .ok_or_else(|| E::custom(format!("{name}: index {i} is not an integer")))
}

fn row_str<E: serde::de::Error>(
    name: &'static str,
    a: &[serde_json::Value],
    i: usize,
) -> Result<String, E> {
    a.get(i)
        .ok_or_else(|| E::custom(format!("{name}: missing index {i}")))?
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| E::custom(format!("{name}: index {i} is not a string")))
}

/// OHLCV kline entry.
/// Aster returns klines as arrays: [open_time, open, high, low, close, volume,
/// close_time, quote_volume, num_trades, taker_buy_base, taker_buy_quote, ignore]
#[derive(Debug, Clone, Serialize)]
pub struct AsterKline {
    pub open_time: i64,
    pub open: String,
    pub high: String,
    pub low: String,
    pub close: String,
    pub volume: String,
    pub close_time: i64,
    pub quote_volume: String,
    pub num_trades: i64,
}

impl<'de> Deserialize<'de> for AsterKline {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        const N: &str = "AsterKline";
        let v: serde_json::Value = Deserialize::deserialize(deserializer)?;
        let a = kline_row::<D::Error>(N, &v)?;
        Ok(Self {
            open_time: row_int::<D::Error>(N, a, 0)?,
            open: row_str::<D::Error>(N, a, 1)?,
            high: row_str::<D::Error>(N, a, 2)?,
            low: row_str::<D::Error>(N, a, 3)?,
            close: row_str::<D::Error>(N, a, 4)?,
            volume: row_str::<D::Error>(N, a, 5)?,
            close_time: row_int::<D::Error>(N, a, 6)?,
            quote_volume: row_str::<D::Error>(N, a, 7)?,
            num_trades: row_int::<D::Error>(N, a, 8)?,
        })
    }
}

/// Price-only kline from `GET /fapi/v3/indexPriceKlines`, `markPriceKlines` and
/// `premiumIndexKlines`: [open_time, open, high, low, close, ignore, close_time, ignore,
/// data_points, ignore, ignore, ignore]. The ignored columns are always `"0"`.
#[derive(Debug, Clone, Serialize)]
pub struct AsterPriceKline {
    pub open_time: i64,
    pub open: String,
    pub high: String,
    pub low: String,
    pub close: String,
    pub close_time: i64,
    /// Number of price samples in the bar.
    pub data_points: i64,
}

impl<'de> Deserialize<'de> for AsterPriceKline {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        const N: &str = "AsterPriceKline";
        let v: serde_json::Value = Deserialize::deserialize(deserializer)?;
        let a = kline_row::<D::Error>(N, &v)?;
        Ok(Self {
            open_time: row_int::<D::Error>(N, a, 0)?,
            open: row_str::<D::Error>(N, a, 1)?,
            high: row_str::<D::Error>(N, a, 2)?,
            low: row_str::<D::Error>(N, a, 3)?,
            close: row_str::<D::Error>(N, a, 4)?,
            close_time: row_int::<D::Error>(N, a, 6)?,
            data_points: row_int::<D::Error>(N, a, 8)?,
        })
    }
}

/// Order book snapshot from `GET /fapi/v3/depth`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsterOrderBook {
    #[serde(rename = "lastUpdateId")]
    pub last_update_id: u64,
    #[serde(rename = "T")]
    pub transaction_time: u64,
    #[serde(rename = "E")]
    pub event_time: u64,
    pub bids: Vec<[String; 2]>,
    pub asks: Vec<[String; 2]>,
}

// ── WebSocket stream message types ───────────────────────────────────────────

/// Diff depth (`<symbol>@depth[@500ms|@100ms]`) and partial depth
/// (`<symbol>@depth<levels>[@500ms|@100ms]`) events. Both use this shape; partial depth
/// carries the top `<levels>` of each side.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AsterDepthMessage {
    #[serde(rename = "e")]
    pub event_type: String,
    #[serde(rename = "E")]
    pub event_time: u64,
    #[serde(rename = "T")]
    pub transaction_time: u64,
    #[serde(rename = "s")]
    pub symbol: String,
    #[serde(rename = "U")]
    pub first_update_id: u64,
    #[serde(rename = "u")]
    pub final_update_id: u64,
    #[serde(rename = "b")]
    pub bids: Vec<[String; 2]>,
    #[serde(rename = "a")]
    pub asks: Vec<[String; 2]>,
    #[serde(rename = "pu")]
    pub previous_final_update_id: u64,
}

/// Aggregate trade stream (`{symbol}@aggTrade`).
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AsterAggTradeMessage {
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
    pub qty: String,
    #[serde(rename = "f")]
    pub first_trade_id: u64,
    #[serde(rename = "l")]
    pub last_trade_id: u64,
    #[serde(rename = "T")]
    pub trade_time: u64,
    #[serde(rename = "m")]
    pub is_buyer_maker: bool,
}

/// Mark price streams (`{symbol}@markPrice[@1s]`, elements of `!markPrice@arr[@1s]`).
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AsterMarkPriceMessage {
    #[serde(rename = "e")]
    pub event_type: String,
    #[serde(rename = "E")]
    pub event_time: u64,
    #[serde(rename = "s")]
    pub symbol: String,
    #[serde(rename = "p")]
    pub mark_price: String,
    #[serde(rename = "i")]
    pub index_price: String,
    /// Only meaningful in the last hour before settlement.
    #[serde(rename = "P")]
    pub estimated_settle_price: String,
    #[serde(rename = "r")]
    pub funding_rate: String,
    #[serde(rename = "T")]
    pub next_funding_time: u64,
}

/// Kline stream (`{symbol}@kline_{interval}`).
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AsterKlineMessage {
    #[serde(rename = "e")]
    pub event_type: String,
    #[serde(rename = "E")]
    pub event_time: u64,
    #[serde(rename = "s")]
    pub symbol: String,
    #[serde(rename = "k")]
    pub kline: AsterStreamKline,
}

/// The `k` object of a kline stream event.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AsterStreamKline {
    #[serde(rename = "t")]
    pub open_time: u64,
    #[serde(rename = "T")]
    pub close_time: u64,
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
    pub num_trades: u64,
    #[serde(rename = "x")]
    pub is_closed: bool,
    #[serde(rename = "q")]
    pub quote_volume: String,
    #[serde(rename = "V")]
    pub taker_buy_volume: String,
    #[serde(rename = "Q")]
    pub taker_buy_quote_volume: String,
}

/// Mini ticker (`<symbol>@miniTicker`, elements of `!miniTicker@arr`).
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AsterMiniTickerMessage {
    #[serde(rename = "e")]
    pub event_type: String,
    #[serde(rename = "E")]
    pub event_time: u64,
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

/// 24h ticker (`<symbol>@ticker`, elements of `!ticker@arr`).
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AsterTickerMessage {
    #[serde(rename = "e")]
    pub event_type: String,
    #[serde(rename = "E")]
    pub event_time: u64,
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
    pub num_trades: u64,
}

/// Best bid/ask (`<symbol>@bookTicker`, `!bookTicker`).
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AsterBookTickerMessage {
    #[serde(rename = "e")]
    pub event_type: String,
    #[serde(rename = "u")]
    pub update_id: u64,
    #[serde(rename = "E")]
    pub event_time: u64,
    #[serde(rename = "T")]
    pub transaction_time: u64,
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

/// Liquidation order snapshot (`<symbol>@forceOrder`, `!forceOrder@arr`).
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AsterForceOrderMessage {
    #[serde(rename = "e")]
    pub event_type: String,
    #[serde(rename = "E")]
    pub event_time: u64,
    #[serde(rename = "o")]
    pub order: AsterForceOrder,
}

/// The `o` object of a `forceOrder` event.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AsterForceOrder {
    #[serde(rename = "s")]
    pub symbol: String,
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
}

/// `result` of a request reply.
#[derive(Debug, Clone, PartialEq)]
pub enum AsterWsResult {
    /// `null`: `SUBSCRIBE`, `UNSUBSCRIBE`, `SET_PROPERTY` succeeded.
    Null,
    /// `LIST_SUBSCRIPTIONS`.
    Subscriptions(Vec<String>),
    /// `GET_PROPERTY`.
    Property(bool),
}

/// A parsed frame from the market WebSocket (raw `/ws` or combined `/stream`).
#[derive(Debug, Clone)]
pub enum AsterWsMessage {
    /// `{"id":..,"result":..}`.
    Response {
        id: u64,
        result: AsterWsResult,
    },
    /// `{"id":..,"error":{"code":..,"msg":..}}`. The docs show errors without `id`.
    Error {
        id: Option<u64>,
        code: i64,
        msg: String,
    },
    Depth(AsterDepthMessage),
    AggTrade(AsterAggTradeMessage),
    MarkPrice(AsterMarkPriceMessage),
    MarkPriceArr(Vec<AsterMarkPriceMessage>),
    Kline(AsterKlineMessage),
    MiniTicker(AsterMiniTickerMessage),
    MiniTickerArr(Vec<AsterMiniTickerMessage>),
    Ticker(AsterTickerMessage),
    TickerArr(Vec<AsterTickerMessage>),
    BookTicker(AsterBookTickerMessage),
    ForceOrder(AsterForceOrderMessage),
    /// `{"stream":..,"data":..}` from the combined endpoint.
    Combined {
        stream: String,
        data: Box<AsterWsMessage>,
    },
}

impl AsterWsMessage {
    pub fn parse(text: &str) -> Result<Self, String> {
        let v: serde_json::Value =
            serde_json::from_str(text).map_err(|e| format!("Aster ws: invalid JSON: {e}"))?;
        Self::from_value(v)
    }

    fn from_value(v: serde_json::Value) -> Result<Self, String> {
        fn de<T: serde::de::DeserializeOwned>(
            v: serde_json::Value,
            what: &str,
        ) -> Result<T, String> {
            serde_json::from_value(v).map_err(|e| format!("Aster ws: bad {what}: {e}"))
        }

        if let serde_json::Value::Array(items) = &v {
            let event = items
                .first()
                .and_then(|i| i.get("e"))
                .and_then(|e| e.as_str())
                .ok_or("Aster ws: array frame without event type")?
                .to_string();
            return match event.as_str() {
                "markPriceUpdate" => Ok(Self::MarkPriceArr(de(v, "markPrice array")?)),
                "24hrMiniTicker" => Ok(Self::MiniTickerArr(de(v, "miniTicker array")?)),
                "24hrTicker" => Ok(Self::TickerArr(de(v, "ticker array")?)),
                other => Err(format!("Aster ws: unknown array event {other}")),
            };
        }

        let obj = v.as_object().ok_or("Aster ws: frame is not an object")?;

        if obj.contains_key("stream") {
            let stream = obj
                .get("stream")
                .and_then(|s| s.as_str())
                .ok_or("Aster ws: combined `stream` is not a string")?
                .to_string();
            let data = obj
                .get("data")
                .cloned()
                .ok_or("Aster ws: combined frame without `data`")?;
            return Ok(Self::Combined {
                stream,
                data: Box::new(Self::from_value(data)?),
            });
        }

        if let Some(err) = obj.get("error") {
            #[derive(Deserialize)]
            struct E {
                code: i64,
                msg: String,
            }
            let e: E = de(err.clone(), "error")?;
            let id = match obj.get("id") {
                None | Some(serde_json::Value::Null) => None,
                Some(i) => Some(i.as_u64().ok_or("Aster ws: error `id` is not an integer")?),
            };
            return Ok(Self::Error {
                id,
                code: e.code,
                msg: e.msg,
            });
        }

        if let Some(result) = obj.get("result") {
            let id = obj
                .get("id")
                .and_then(|i| i.as_u64())
                .ok_or("Aster ws: response without integer `id`")?;
            let result = match result {
                serde_json::Value::Null => AsterWsResult::Null,
                serde_json::Value::Bool(b) => AsterWsResult::Property(*b),
                serde_json::Value::Array(_) => {
                    AsterWsResult::Subscriptions(de(result.clone(), "subscription list")?)
                }
                other => return Err(format!("Aster ws: unexpected result {other}")),
            };
            return Ok(Self::Response { id, result });
        }

        let event = obj
            .get("e")
            .and_then(|e| e.as_str())
            .ok_or("Aster ws: frame without event type")?
            .to_string();
        match event.as_str() {
            "depthUpdate" => Ok(Self::Depth(de(v, "depthUpdate")?)),
            "aggTrade" => Ok(Self::AggTrade(de(v, "aggTrade")?)),
            "markPriceUpdate" => Ok(Self::MarkPrice(de(v, "markPriceUpdate")?)),
            "kline" => Ok(Self::Kline(de(v, "kline")?)),
            "24hrMiniTicker" => Ok(Self::MiniTicker(de(v, "24hrMiniTicker")?)),
            "24hrTicker" => Ok(Self::Ticker(de(v, "24hrTicker")?)),
            "bookTicker" => Ok(Self::BookTicker(de(v, "bookTicker")?)),
            "forceOrder" => Ok(Self::ForceOrder(de(v, "forceOrder")?)),
            other => Err(format!("Aster ws: unknown event {other}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kline_parses_wire_row() {
        let row = r#"[1499040000000,"0.016","0.8","0.015","0.0158","148976.1",1499644799999,"2434.2",308,"1756.8","28.4","0"]"#;
        let k: AsterKline = serde_json::from_str(row).unwrap();
        assert_eq!(k.open_time, 1499040000000);
        assert_eq!(k.open, "0.016");
        assert_eq!(k.close, "0.0158");
        assert_eq!(k.close_time, 1499644799999);
        assert_eq!(k.quote_volume, "2434.2");
        assert_eq!(k.num_trades, 308);
    }

    #[test]
    fn kline_short_row_is_error() {
        let row = r#"[1499040000000,"0.016","0.8"]"#;
        let err = serde_json::from_str::<AsterKline>(row).unwrap_err();
        assert!(err.to_string().contains("missing index 3"), "{err}");
    }

    #[test]
    fn kline_wrong_type_is_error() {
        let row =
            r#"[1499040000000,0.016,"0.8","0.015","0.0158","148976.1",1499644799999,"2434.2",308]"#;
        let err = serde_json::from_str::<AsterKline>(row).unwrap_err();
        assert!(err.to_string().contains("index 1 is not a string"), "{err}");
    }

    #[test]
    fn subscription_streams() {
        assert_eq!(
            AsterWssMessage::depth(&["BTCUSDT", "ETHUSDT"], AsterDepthSpeed::Ms100).params,
            vec!["btcusdt@depth@100ms", "ethusdt@depth@100ms"]
        );
        assert_eq!(
            AsterWssMessage::depth_unsub(&["BTCUSDT"], AsterDepthSpeed::Default).params,
            vec!["btcusdt@depth"]
        );
        assert_eq!(
            AsterWssMessage::klines(&["BTCUSDT"], AsterKlineInterval::M1).params,
            vec!["btcusdt@kline_1m"]
        );
        assert_eq!(
            AsterWssMessage::klines(&["BTCUSDT"], AsterKlineInterval::Mo1).params,
            vec!["btcusdt@kline_1M"]
        );
        assert_eq!(
            AsterWssMessage::trades(&["BTCUSDT"]).to_json(),
            r#"{"id":1,"method":"SUBSCRIBE","params":["btcusdt@aggTrade"]}"#
        );
    }

    #[test]
    fn new_subscription_streams() {
        assert_eq!(
            AsterWssMessage::partial_depth(
                &["BTCUSDT"],
                AsterDepthLevels::L10,
                AsterDepthSpeed::Ms500
            )
            .params,
            vec!["btcusdt@depth10@500ms"]
        );
        assert_eq!(
            AsterWssMessage::partial_depth_unsub(
                &["BTCUSDT"],
                AsterDepthLevels::L5,
                AsterDepthSpeed::Default
            )
            .params,
            vec!["btcusdt@depth5"]
        );
        assert_eq!(
            AsterWssMessage::mark_price(&["BTCUSDT"], AsterMarkPriceSpeed::S1).params,
            vec!["btcusdt@markPrice@1s"]
        );
        assert_eq!(
            AsterWssMessage::mark_price(&["BTCUSDT"], AsterMarkPriceSpeed::Default).params,
            vec!["btcusdt@markPrice"]
        );
        assert_eq!(
            AsterWssMessage::all_mark_prices(AsterMarkPriceSpeed::S1).params,
            vec!["!markPrice@arr@1s"]
        );
        assert_eq!(
            AsterWssMessage::mini_tickers(&["BTCUSDT"]).params,
            vec!["btcusdt@miniTicker"]
        );
        assert_eq!(
            AsterWssMessage::all_mini_tickers().params,
            vec!["!miniTicker@arr"]
        );
        assert_eq!(
            AsterWssMessage::tickers(&["BTCUSDT"]).params,
            vec!["btcusdt@ticker"]
        );
        assert_eq!(AsterWssMessage::all_tickers().params, vec!["!ticker@arr"]);
        assert_eq!(
            AsterWssMessage::book_tickers(&["BTCUSDT"]).params,
            vec!["btcusdt@bookTicker"]
        );
        assert_eq!(
            AsterWssMessage::all_book_tickers().params,
            vec!["!bookTicker"]
        );
        assert_eq!(
            AsterWssMessage::liquidations_unsub(&["BTCUSDT"]).to_json(),
            r#"{"id":1,"method":"UNSUBSCRIBE","params":["btcusdt@forceOrder"]}"#
        );
        assert_eq!(
            AsterWssMessage::all_liquidations().params,
            vec!["!forceOrder@arr"]
        );
        assert_eq!(
            list_subscriptions_request(3),
            r#"{"id":3,"method":"LIST_SUBSCRIPTIONS"}"#
        );
        assert_eq!(
            set_combined_property_request(true, 5),
            r#"{"id":5,"method":"SET_PROPERTY","params":["combined",true]}"#
        );
        assert_eq!(
            get_combined_property_request(2),
            r#"{"id":2,"method":"GET_PROPERTY","params":["combined"]}"#
        );
        assert_eq!(
            combined_stream_url(&AsterWssMessage::book_tickers(&["BTCUSDT", "ETHUSDT"]).params),
            "wss://fstream.asterdex.com/stream?streams=btcusdt@bookTicker/ethusdt@bookTicker"
        );
        assert_eq!(
            combined_stream_url(&[]),
            "wss://fstream.asterdex.com/stream"
        );
    }

    // Payloads captured from wss://fstream.asterdex.com/ws on 2026-11-15.
    #[test]
    fn stream_payloads_parse() {
        let depth = r#"{"e":"depthUpdate","E":1794779363319,"T":1794779363200,"s":"BTCUSDT","U":656219401083,"u":656219402410,"pu":656219398534,"b":[["64851.6","0.000"]],"a":[["65102.6","1.766"]]}"#;
        let d: AsterDepthMessage = serde_json::from_str(depth).unwrap();
        assert_eq!(d.previous_final_update_id, 656219398534);

        let mark = r#"{"e":"markPriceUpdate","E":1794779364000,"s":"BTCUSDT","p":"65099.05215217","P":"65035.80960924","i":"65120.81869565","r":"0.00006988","T":1794787200000}"#;
        let m: AsterMarkPriceMessage = serde_json::from_str(mark).unwrap();
        assert_eq!(m.estimated_settle_price, "65035.80960924");

        let kline = r#"{"e":"kline","E":1794779364735,"s":"BTCUSDT","k":{"t":1793491200000,"T":1796083199999,"s":"BTCUSDT","i":"1M","f":149440476,"L":150045443,"o":"77525.2","c":"65097.2","h":"81186.5","l":"61952.0","v":"101826.125","n":604752,"x":false,"q":"7376898131.3453","V":"49239.913","Q":"3578708281.2790","B":"0"}}"#;
        let k: AsterKlineMessage = serde_json::from_str(kline).unwrap();
        assert_eq!(k.kline.interval, "1M");
        assert!(!k.kline.is_closed);
        assert!(matches!(
            AsterWsMessage::parse(kline).unwrap(),
            AsterWsMessage::Kline(_)
        ));

        let trade = r#"{"e":"aggTrade","E":1794779364889,"a":89415770,"s":"BTCUSDT","p":"65097.2","q":"0.076","f":150045443,"l":150045443,"T":1794779364700,"m":false}"#;
        let t: AsterAggTradeMessage = serde_json::from_str(trade).unwrap();
        assert_eq!(t.first_trade_id, 150045443);
    }

    // Response captured from GET /fapi/v3/depth on 2026-11-15.
    #[test]
    fn orderbook_parses_and_requires_times() {
        let ok = r#"{"lastUpdateId":656218934716,"E":1794779336782,"T":1794779336750,"bids":[["65085.5","0.385"]],"asks":[["65085.6","2.489"]]}"#;
        let b: AsterOrderBook = serde_json::from_str(ok).unwrap();
        assert_eq!(b.transaction_time, 1794779336750);
        let missing = r#"{"lastUpdateId":1,"bids":[],"asks":[]}"#;
        assert!(serde_json::from_str::<AsterOrderBook>(missing).is_err());
    }

    #[test]
    fn kline_not_array_is_error() {
        assert!(serde_json::from_str::<AsterKline>("{}").is_err());
    }
}
