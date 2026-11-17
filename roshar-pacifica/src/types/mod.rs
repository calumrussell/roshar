use serde::{Deserialize, Serialize};

// ── WebSocket messages ────────────────────────────────────────────────────────

/// Subscribe / unsubscribe / ping message.
/// <https://docs.pacifica.fi/api-documentation/api/websocket>
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PacificaWssMessage {
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<PacificaSubscribeParams>,
}

/// `params` of a subscription. Which fields a `source` takes is set by the exchange: `book` needs
/// `symbol` and `agg_level`, `trades` and `bbo` need `symbol`, `candle` and `mark_price_candle`
/// need `symbol` and `interval`, and `prices` takes neither. The server echoes these params back
/// as the `data` of its `subscribe` / `unsubscribe` acknowledgement.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PacificaSubscribeParams {
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interval: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agg_level: Option<u32>,
}

impl PacificaWssMessage {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("Failed to serialize PacificaWssMessage")
    }

    pub fn ping() -> Self {
        Self {
            method: "ping".to_string(),
            params: None,
        }
    }

    fn with(method: &str, params: PacificaSubscribeParams) -> Self {
        Self {
            method: method.to_string(),
            params: Some(params),
        }
    }

    fn book_params(symbol: &str, agg_level: u32) -> PacificaSubscribeParams {
        PacificaSubscribeParams {
            source: "book".to_string(),
            symbol: Some(symbol.to_string()),
            interval: None,
            agg_level: Some(agg_level),
        }
    }

    fn trades_params(symbol: &str) -> PacificaSubscribeParams {
        PacificaSubscribeParams {
            source: "trades".to_string(),
            symbol: Some(symbol.to_string()),
            interval: None,
            agg_level: None,
        }
    }

    /// `book` channel. `agg_level` is one of 1, 2, 5, 10, 100, 1000; the exchange requires it, and an
    /// unsubscribe must repeat the value used to subscribe.
    pub fn depth(symbol: &str, agg_level: u32) -> Self {
        Self::with("subscribe", Self::book_params(symbol, agg_level))
    }

    pub fn depth_unsub(symbol: &str, agg_level: u32) -> Self {
        Self::with("unsubscribe", Self::book_params(symbol, agg_level))
    }

    pub fn trades(symbol: &str) -> Self {
        Self::with("subscribe", Self::trades_params(symbol))
    }

    pub fn trades_unsub(symbol: &str) -> Self {
        Self::with("unsubscribe", Self::trades_params(symbol))
    }

    /// `candle` channel. `interval` is sent as given (e.g. `"1m"`).
    pub fn candles(symbol: &str, interval: &str) -> Self {
        Self::with("subscribe", Self::candle_params("candle", symbol, interval))
    }

    /// `interval` must be the value used to subscribe.
    pub fn candles_unsub(symbol: &str, interval: &str) -> Self {
        Self::with(
            "unsubscribe",
            Self::candle_params("candle", symbol, interval),
        )
    }

    /// `mark_price_candle` channel. `interval` is sent as given (e.g. `"1m"`).
    pub fn mark_price_candles(symbol: &str, interval: &str) -> Self {
        Self::with(
            "subscribe",
            Self::candle_params("mark_price_candle", symbol, interval),
        )
    }

    /// `interval` must be the value used to subscribe.
    pub fn mark_price_candles_unsub(symbol: &str, interval: &str) -> Self {
        Self::with(
            "unsubscribe",
            Self::candle_params("mark_price_candle", symbol, interval),
        )
    }

    /// `bbo` channel: top of book for one symbol, sent whenever it changes.
    pub fn bbo(symbol: &str) -> Self {
        Self::with("subscribe", Self::bbo_params(symbol))
    }

    pub fn bbo_unsub(symbol: &str) -> Self {
        Self::with("unsubscribe", Self::bbo_params(symbol))
    }

    /// `prices` channel: mark, mid, oracle, funding and open interest for every symbol.
    pub fn prices() -> Self {
        Self::with("subscribe", Self::prices_params())
    }

    pub fn prices_unsub() -> Self {
        Self::with("unsubscribe", Self::prices_params())
    }

    fn candle_params(source: &str, symbol: &str, interval: &str) -> PacificaSubscribeParams {
        PacificaSubscribeParams {
            source: source.to_string(),
            symbol: Some(symbol.to_string()),
            interval: Some(interval.to_string()),
            agg_level: None,
        }
    }

    fn bbo_params(symbol: &str) -> PacificaSubscribeParams {
        PacificaSubscribeParams {
            source: "bbo".to_string(),
            symbol: Some(symbol.to_string()),
            interval: None,
            agg_level: None,
        }
    }

    fn prices_params() -> PacificaSubscribeParams {
        PacificaSubscribeParams {
            source: "prices".to_string(),
            symbol: None,
            interval: None,
            agg_level: None,
        }
    }
}

// ── WebSocket stream payloads ─────────────────────────────────────────────────

/// A text frame from the websocket (or a decompressed binary frame, see
/// [`PacificaInflater`](crate::PacificaInflater)). Parse with [`PacificaWsMessage::parse`].
#[derive(Debug, Clone)]
pub enum PacificaWsMessage {
    /// A frame with a `channel`.
    Event(PacificaWsEvent),
    /// A frame without a `channel`: the server rejected a request.
    Error(PacificaWsError),
}

impl PacificaWsMessage {
    /// Frames with a `channel` must match that channel's shape; frames without one must be one of
    /// the two error shapes. Anything else is an error.
    pub fn parse(text: &str) -> Result<Self, serde_json::Error> {
        #[derive(Deserialize)]
        struct Probe {
            #[serde(default)]
            channel: Option<serde::de::IgnoredAny>,
        }
        let probe: Probe = serde_json::from_str(text)?;
        if probe.channel.is_some() {
            serde_json::from_str(text).map(Self::Event)
        } else {
            serde_json::from_str(text).map(Self::Error)
        }
    }
}

/// Frame tagged by `channel`, with its payload in `data`.
/// <https://docs.pacifica.fi/api-documentation/api/websocket/subscriptions>
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "channel", content = "data", rename_all = "snake_case")]
pub enum PacificaWsEvent {
    /// Subscription acknowledgement echoing the request's params.
    Subscribe(PacificaSubscribeParams),
    /// Unsubscription acknowledgement echoing the request's params.
    Unsubscribe(PacificaSubscribeParams),
    /// Reply to `{"method":"ping"}`.
    Pong,
    Book(PacificaWsBook),
    Bbo(PacificaBbo),
    Trades(Vec<PacificaWsTrade>),
    Candle(PacificaKline),
    MarkPriceCandle(PacificaKline),
    Prices(Vec<PacificaTicker>),
}

/// Rejected request. Live shapes: `{"error":"Symbol not found"}` (unknown symbol) and
/// `{"code":400,"err":"Invalid subscription parameters.","t":...}` (unknown source / bad params).
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum PacificaWsError {
    Coded {
        code: i64,
        err: String,
        /// Milliseconds.
        t: i64,
    },
    Message {
        error: String,
    },
}

/// `book` stream snapshot: `l[0]` bids, `l[1]` asks, up to 10 levels each, every 250 ms.
/// <https://docs.pacifica.fi/api-documentation/api/websocket/subscriptions/orderbook>
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PacificaWsBook {
    #[serde(rename = "s")]
    pub symbol: String,
    #[serde(rename = "l")]
    pub levels: [Vec<PacificaBookLevel>; 2],
    #[serde(rename = "t")]
    pub timestamp: i64,
    /// Last ID: per-symbol sequence number.
    pub li: i64,
}

impl PacificaWsBook {
    pub fn bids(&self) -> &[PacificaBookLevel] {
        &self.levels[0]
    }

    pub fn asks(&self) -> &[PacificaBookLevel] {
        &self.levels[1]
    }
}

/// `bbo` stream update.
/// <https://docs.pacifica.fi/api-documentation/api/websocket/subscriptions/best-bid-offer-bbo>
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PacificaBbo {
    #[serde(rename = "s")]
    pub symbol: String,
    /// Order id.
    #[serde(rename = "i")]
    pub order_id: i64,
    /// Last ID: per-symbol sequence number.
    pub li: i64,
    #[serde(rename = "t")]
    pub timestamp: i64,
    #[serde(rename = "b")]
    pub bid_price: String,
    #[serde(rename = "B")]
    pub bid_amount: String,
    #[serde(rename = "a")]
    pub ask_price: String,
    #[serde(rename = "A")]
    pub ask_amount: String,
}

/// `trades` stream entry (taker side).
/// <https://docs.pacifica.fi/api-documentation/api/websocket/subscriptions/trades>
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PacificaWsTrade {
    /// History ID.
    #[serde(rename = "h")]
    pub history_id: i64,
    #[serde(rename = "s")]
    pub symbol: String,
    #[serde(rename = "a")]
    pub amount: String,
    #[serde(rename = "p")]
    pub price: String,
    /// `open_long`, `open_short`, `close_long` or `close_short`.
    #[serde(rename = "d")]
    pub side: String,
    /// Trade cause, e.g. `normal`, `market_liquidation`.
    #[serde(rename = "tc")]
    pub cause: String,
    #[serde(rename = "t")]
    pub timestamp: i64,
    /// Last ID: per-symbol sequence number.
    pub li: i64,
    /// Instrument type.
    #[serde(rename = "it")]
    pub instrument_type: i64,
}

// ── REST response types ───────────────────────────────────────────────────────

/// Envelope of every REST reply: `{"success", "data", "error", "code", ...}`.
/// `data` is `null` on failure and absent on some acknowledgements (cancel). `error` and `code` are
/// absent from some successful replies (funding history). `error_id` is only sent on failures.
/// <https://docs.pacifica.fi/api-documentation/api/error-codes>
#[derive(Debug, Clone, Deserialize)]
pub struct PacificaResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub code: Option<i64>,
    #[serde(default)]
    pub error_id: Option<String>,
}

/// Market specification from `GET /api/v1/info`.
/// <https://docs.pacifica.fi/api-documentation/api/rest-api/markets/get-market-info>
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PacificaMarket {
    pub symbol: String,
    pub tick_size: String,
    pub min_tick: String,
    pub max_tick: String,
    pub lot_size: String,
    pub max_leverage: u32,
    pub isolated_only: bool,
    /// USD.
    pub min_order_size: String,
    /// USD.
    pub max_order_size: String,
    pub funding_rate: String,
    pub next_funding_rate: String,
    pub created_at: i64,
    pub instrument_type: String,
    pub base_asset: String,
}

/// Price snapshot from `GET /api/v1/info/prices` (same shape as the `prices` websocket channel).
/// <https://docs.pacifica.fi/api-documentation/api/rest-api/markets/get-prices>
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PacificaTicker {
    pub symbol: String,
    pub funding: String,
    pub next_funding: String,
    pub mark: String,
    pub mid: String,
    pub oracle: String,
    /// USD.
    pub open_interest: String,
    /// USD.
    pub volume_24h: String,
    pub yesterday_price: String,
    pub timestamp: i64,
}

/// One aggregated level of the book.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PacificaBookLevel {
    #[serde(rename = "p")]
    pub price: String,
    #[serde(rename = "a")]
    pub amount: String,
    #[serde(rename = "n")]
    pub orders: u32,
}

/// Order book from `GET /api/v1/book`: `l[0]` holds bids, `l[1]` asks, up to 10 levels each.
/// <https://docs.pacifica.fi/api-documentation/api/rest-api/markets/get-orderbook>
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PacificaOrderBook {
    #[serde(rename = "s")]
    pub symbol: String,
    #[serde(rename = "l")]
    pub levels: [Vec<PacificaBookLevel>; 2],
    #[serde(rename = "t")]
    pub timestamp: i64,
}

impl PacificaOrderBook {
    pub fn bids(&self) -> &[PacificaBookLevel] {
        &self.levels[0]
    }

    pub fn asks(&self) -> &[PacificaBookLevel] {
        &self.levels[1]
    }
}

/// Trade event from `GET /api/v1/trades`.
/// <https://docs.pacifica.fi/api-documentation/api/rest-api/markets/get-recent-trades>
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PacificaTrade {
    /// `fulfill_taker` or `fulfill_maker`.
    pub event_type: String,
    pub price: String,
    pub amount: String,
    /// `open_long`, `open_short`, `close_long` or `close_short`.
    pub side: String,
    pub cause: String,
    pub created_at: i64,
}

/// Settled funding record from `GET /api/v1/funding_rate/history`.
/// <https://docs.pacifica.fi/api-documentation/api/rest-api/markets/get-historical-funding>
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PacificaFundingRate {
    pub oracle_price: String,
    pub bid_impact_price: String,
    pub ask_impact_price: String,
    pub funding_rate: String,
    pub next_funding_rate: String,
    pub created_at: i64,
}

/// One page of funding history. `next_cursor` is absent on the last page.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PacificaFundingRatePage {
    pub rates: Vec<PacificaFundingRate>,
    pub next_cursor: Option<String>,
    pub has_more: bool,
}

/// Open order from `GET /api/v1/orders`.
/// <https://docs.pacifica.fi/api-documentation/api/rest-api/orders/get-open-orders>
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PacificaOpenOrder {
    pub order_id: i64,
    pub client_order_id: Option<String>,
    /// Order strategy that placed the order.
    pub parent_id: Option<String>,
    pub symbol: String,
    pub side: String,
    pub price: String,
    pub initial_amount: String,
    pub filled_amount: String,
    pub cancelled_amount: String,
    pub stop_price: Option<String>,
    pub order_type: String,
    pub stop_parent_order_id: Option<i64>,
    pub trigger_price_type: Option<String>,
    pub reduce_only: bool,
    pub order_expiry: Option<i64>,
    pub instrument_type: String,
    pub created_at: i64,
    pub updated_at: i64,
    /// Last ID sequence number; `null` until the engine touches the order.
    pub li: Option<i64>,
}

/// Position from `GET /api/v1/positions`.
/// <https://docs.pacifica.fi/api-documentation/api/rest-api/account/get-positions>
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PacificaPosition {
    pub id: String,
    pub symbol: String,
    /// `bid` (long) or `ask` (short).
    pub side: String,
    pub amount: String,
    pub entry_price: String,
    pub margin: String,
    pub funding: String,
    pub isolated: bool,
    pub liquidation_price: Option<String>,
    /// `null` for positions opened before 2026-10-04.
    pub realized_pnl: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub li: Option<i64>,
}

/// Candle from `GET /api/v1/kline` and `GET /api/v1/kline/mark` (same shape as the `candle` and
/// `mark_price_candle` websocket channels). Mark price candles carry mark prices in `o/h/l/c`.
/// <https://docs.pacifica.fi/api-documentation/api/rest-api/markets/get-candle-data>
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PacificaKline {
    #[serde(rename = "t")]
    pub open_time: i64,
    #[serde(rename = "T")]
    pub close_time: i64,
    #[serde(rename = "s")]
    pub symbol: String,
    #[serde(rename = "i")]
    pub interval: String,
    #[serde(rename = "o")]
    pub open: String,
    #[serde(rename = "h")]
    pub high: String,
    #[serde(rename = "l")]
    pub low: String,
    #[serde(rename = "c")]
    pub close: String,
    #[serde(rename = "v")]
    pub volume: String,
    #[serde(rename = "n")]
    pub trades: u64,
}

/// Spot asset from `GET /api/v1/spot_assets`.
/// <https://docs.pacifica.fi/api-documentation/api/rest-api/spot/get-spot-assets>
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PacificaSpotAsset {
    pub symbol: String,
    pub tick_size: String,
    pub lot_size: String,
    pub active: bool,
    pub collateral_enabled: bool,
    pub ltv_ratio: String,
    pub created_at: i64,
    pub updated_at: i64,
}
