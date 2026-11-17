//! WebSocket request builders and typed inbound messages for `/ws/v5/public` and
//! `/ws/v5/business`.

use super::market::*;
use super::{OkxCandle, OkxInstrumentInfo};
use crate::rest::OkxTickerData;
use anyhow::{anyhow, bail, Result};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

// --- Subscribe/Unsubscribe message types ---

/// Channel argument, as sent in requests and echoed in acks and pushes. Which of the optional
/// fields a channel takes is per OKX's docs; unused ones are omitted from the JSON.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OkxWssArg {
    pub channel: String,
    #[serde(rename = "instId", default, skip_serializing_if = "Option::is_none")]
    pub inst_id: Option<String>,
    #[serde(rename = "instType", default, skip_serializing_if = "Option::is_none")]
    pub inst_type: Option<String>,
    #[serde(
        rename = "instFamily",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub inst_family: Option<String>,
    #[serde(rename = "seriesId", default, skip_serializing_if = "Option::is_none")]
    pub series_id: Option<String>,
}

impl OkxWssArg {
    /// Bare channel argument; add filters with the `with_*` methods.
    pub fn new(channel: impl Into<String>) -> Self {
        Self {
            channel: channel.into(),
            inst_id: None,
            inst_type: None,
            inst_family: None,
            series_id: None,
        }
    }

    pub fn with_inst_id(mut self, inst_id: impl Into<String>) -> Self {
        self.inst_id = Some(inst_id.into());
        self
    }

    pub fn with_inst_type(mut self, inst_type: impl Into<String>) -> Self {
        self.inst_type = Some(inst_type.into());
        self
    }

    pub fn with_inst_family(mut self, inst_family: impl Into<String>) -> Self {
        self.inst_family = Some(inst_family.into());
        self
    }

    pub fn with_series_id(mut self, series_id: impl Into<String>) -> Self {
        self.series_id = Some(series_id.into());
        self
    }

    fn opt(mut self, inst_family: Option<&str>, inst_id: Option<&str>) -> Self {
        self.inst_family = inst_family.map(str::to_string);
        self.inst_id = inst_id.map(str::to_string);
        self
    }

    // --- /ws/v5/public ---

    /// `books`: 400-level snapshot then incremental updates (100 ms).
    pub fn books(inst_id: &str) -> Self {
        Self::new("books").with_inst_id(inst_id)
    }

    /// `books5`: 5-level snapshots (100 ms).
    pub fn books5(inst_id: &str) -> Self {
        Self::new("books5").with_inst_id(inst_id)
    }

    /// `bbo-tbt`: top of book, tick by tick.
    pub fn bbo_tbt(inst_id: &str) -> Self {
        Self::new("bbo-tbt").with_inst_id(inst_id)
    }

    /// `books-l2-tbt`: 400 levels tick by tick. OKX requires login and VIP4+ (error 60011
    /// "Please log in" otherwise); this crate does not log in.
    pub fn books_l2_tbt(inst_id: &str) -> Self {
        Self::new("books-l2-tbt").with_inst_id(inst_id)
    }

    /// `books50-l2-tbt`: 50 levels tick by tick. OKX requires login and VIP4+ (error 60011
    /// otherwise); this crate does not log in.
    pub fn books50_l2_tbt(inst_id: &str) -> Self {
        Self::new("books50-l2-tbt").with_inst_id(inst_id)
    }

    /// `books-rpi`: book including Retail Price Improvement orders (level index 2 = size
    /// excluding RPI).
    pub fn books_rpi(inst_id: &str) -> Self {
        Self::new("books-rpi").with_inst_id(inst_id)
    }

    /// `bbo-rpi-tbt`: top of book including RPI orders, tick by tick.
    pub fn bbo_rpi_tbt(inst_id: &str) -> Self {
        Self::new("bbo-rpi-tbt").with_inst_id(inst_id)
    }

    /// `trades`: trades aggregated per taker order.
    pub fn trades(inst_id: &str) -> Self {
        Self::new("trades").with_inst_id(inst_id)
    }

    pub fn tickers(inst_id: &str) -> Self {
        Self::new("tickers").with_inst_id(inst_id)
    }

    /// `index-tickers`: `inst_id` is an index, e.g. "BTC-USDT".
    pub fn index_tickers(inst_id: &str) -> Self {
        Self::new("index-tickers").with_inst_id(inst_id)
    }

    pub fn mark_price(inst_id: &str) -> Self {
        Self::new("mark-price").with_inst_id(inst_id)
    }

    pub fn funding_rate(inst_id: &str) -> Self {
        Self::new("funding-rate").with_inst_id(inst_id)
    }

    pub fn open_interest(inst_id: &str) -> Self {
        Self::new("open-interest").with_inst_id(inst_id)
    }

    pub fn price_limit(inst_id: &str) -> Self {
        Self::new("price-limit").with_inst_id(inst_id)
    }

    pub fn call_auction_details(inst_id: &str) -> Self {
        Self::new("call-auction-details").with_inst_id(inst_id)
    }

    /// `estimated-price`: `inst_type` FUTURES, OPTION or EVENTS; OKX requires `inst_family` or
    /// `inst_id`.
    pub fn estimated_price(
        inst_type: &str,
        inst_family: Option<&str>,
        inst_id: Option<&str>,
    ) -> Self {
        Self::new("estimated-price")
            .with_inst_type(inst_type)
            .opt(inst_family, inst_id)
    }

    /// `liquidation-orders`: all liquidations of `inst_type` (SWAP, FUTURES, MARGIN, OPTION).
    pub fn liquidation_orders(inst_type: &str) -> Self {
        Self::new("liquidation-orders").with_inst_type(inst_type)
    }

    /// `instruments`: full list on subscribe, then changes.
    pub fn instruments(inst_type: &str) -> Self {
        Self::new("instruments").with_inst_type(inst_type)
    }

    /// `opt-summary`: option greeks for an instrument family.
    pub fn opt_summary(inst_family: &str) -> Self {
        Self::new("opt-summary").with_inst_family(inst_family)
    }

    /// `option-trades`: `inst_type` is OPTION; OKX requires `inst_family` or `inst_id`.
    pub fn option_trades(
        inst_type: &str,
        inst_family: Option<&str>,
        inst_id: Option<&str>,
    ) -> Self {
        Self::new("option-trades")
            .with_inst_type(inst_type)
            .opt(inst_family, inst_id)
    }

    /// `status`: system maintenance notices.
    pub fn status() -> Self {
        Self::new("status")
    }

    /// `event-contract-markets`: event-contract market status changes and strike generation
    /// (`inst_type` is EVENTS). No snapshot on subscribe.
    pub fn event_contract_markets(inst_type: &str) -> Self {
        Self::new("event-contract-markets").with_inst_type(inst_type)
    }

    // --- /ws/v5/business ---

    /// `candle{bar}` (e.g. bar "1m", "1H", "1Dutc"). Business socket.
    pub fn candle(bar: &str, inst_id: &str) -> Self {
        Self::new(format!("candle{bar}")).with_inst_id(inst_id)
    }

    /// `mark-price-candle{bar}`. Business socket.
    pub fn mark_price_candle(bar: &str, inst_id: &str) -> Self {
        Self::new(format!("mark-price-candle{bar}")).with_inst_id(inst_id)
    }

    /// `index-candle{bar}`; `inst_id` is an index. Business socket.
    pub fn index_candle(bar: &str, inst_id: &str) -> Self {
        Self::new(format!("index-candle{bar}")).with_inst_id(inst_id)
    }

    /// `trades-all`: every fill, unaggregated. Business socket.
    pub fn trades_all(inst_id: &str) -> Self {
        Self::new("trades-all").with_inst_id(inst_id)
    }

    /// True when OKX serves this channel on `/ws/v5/business` rather than `/ws/v5/public`
    /// (subscribing on the wrong path fails with error 60018).
    pub fn is_business(&self) -> bool {
        let c = self.channel.as_str();
        c == "trades-all"
            || c.starts_with("candle")
            || c.starts_with("mark-price-candle")
            || c.starts_with("index-candle")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OkxWssMessage {
    pub op: String,
    pub args: Vec<OkxWssArg>,
}

impl OkxWssMessage {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("failed to serialize OkxWssMessage")
    }

    /// OKX uses text-frame pings: send the literal string "ping"
    pub fn ping() -> String {
        "ping".to_string()
    }

    pub fn subscribe(args: Vec<OkxWssArg>) -> Self {
        Self {
            op: "subscribe".to_string(),
            args,
        }
    }

    pub fn unsubscribe(args: Vec<OkxWssArg>) -> Self {
        Self {
            op: "unsubscribe".to_string(),
            args,
        }
    }
}

// --- Inbound messages ---

/// Control frame: `subscribe`/`unsubscribe` acks, `error`, `notice` and other `event` frames.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OkxWsEvent {
    /// "subscribe", "unsubscribe", "error", "notice", ...
    pub event: String,
    /// Echo of the request `id`, when one was sent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Present on subscribe/unsubscribe acks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arg: Option<OkxWssArg>,
    /// Present on `error`/`notice`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub msg: Option<String>,
    #[serde(rename = "connId")]
    pub conn_id: String,
}

/// Data push without `action`: `{"arg": {...}, "data": [...]}`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OkxPush<T> {
    pub arg: OkxWssArg,
    pub data: Vec<T>,
}

/// `books`, `books-rpi`, `books-l2-tbt` and `books50-l2-tbt` pushes.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OkxDepthMessage {
    pub arg: OkxWssArg,
    /// "snapshot" for the initial full book, "update" for incremental deltas
    pub action: String,
    pub data: Vec<OkxDepthBookData>,
}

impl OkxDepthMessage {
    /// The subscribed instId.
    pub fn inst_id(&self) -> Option<&str> {
        self.arg.inst_id.as_deref()
    }

    /// Returns true if this message is a full snapshot (not a delta update)
    pub fn is_snapshot(&self) -> bool {
        self.action == "snapshot"
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OkxDepthBookData {
    /// `[price, size, x, order count]`; `x` is a deprecated "0" on `books`/`*-l2-tbt` and the
    /// size excluding RPI orders on `books-rpi`.
    pub asks: Vec<[String; 4]>,
    pub bids: Vec<[String; 4]>,
    pub ts: String,
    /// Sequence ID of this message. OKX's deprecated `checksum` (always 0, `books` only) is not
    /// modelled; continuity is checked with `prev_seq_id == previous seq_id`.
    #[serde(rename = "seqId")]
    pub seq_id: i64,
    /// Sequence ID of the previous message; -1 on snapshots.
    #[serde(rename = "prevSeqId")]
    pub prev_seq_id: i64,
}

/// `trades` push entry (aggregated per taker order).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OkxTradeData {
    #[serde(rename = "instId")]
    pub inst_id: String,
    /// Last trade ID in the aggregation
    #[serde(rename = "tradeId")]
    pub trade_id: String,
    pub px: String,
    pub sz: String,
    /// Taker side: "buy" or "sell"
    pub side: String,
    pub ts: String,
    /// Number of trades aggregated into this message
    pub count: String,
    /// Order source: "0" normal, "1" Enhanced Liquidity Program
    pub source: String,
    #[serde(rename = "seqId")]
    pub seq_id: i64,
}

/// Any frame received on `/ws/v5/public` or `/ws/v5/business` for the channels this crate
/// supports. Use [`OkxWsMessage::parse`].
#[derive(Debug, Clone)]
pub enum OkxWsMessage {
    /// Reply to the text `ping`.
    Pong,
    Event(OkxWsEvent),
    /// `books`, `books-rpi`, `books-l2-tbt`, `books50-l2-tbt` (see `arg.channel`).
    Books(OkxDepthMessage),
    /// `books5`
    Books5(OkxPush<OkxOrderBook>),
    /// `bbo-tbt` or `bbo-rpi-tbt` (see `arg.channel`).
    Bbo(OkxPush<OkxOrderBook>),
    Trades(OkxPush<OkxTradeData>),
    TradesAll(OkxPush<OkxTrade>),
    Tickers(OkxPush<OkxTickerData>),
    IndexTickers(OkxPush<OkxIndexTicker>),
    MarkPrice(OkxPush<OkxMarkPrice>),
    FundingRate(OkxPush<OkxFundingRate>),
    OpenInterest(OkxPush<OkxOpenInterest>),
    PriceLimit(OkxPush<OkxPriceLimit>),
    EstimatedPrice(OkxPush<OkxEstimatedPrice>),
    LiquidationOrders(OkxPush<OkxLiquidation>),
    Instruments(OkxPush<OkxInstrumentInfo>),
    OptSummary(OkxPush<OkxOptionSummary>),
    OptionTrades(OkxPush<OkxOptionTrade>),
    CallAuctionDetails(OkxPush<OkxCallAuctionDetails>),
    Status(OkxPush<OkxSystemStatus>),
    EventContractMarkets(OkxPush<OkxEventMarket>),
    /// `candle{bar}` (business socket)
    Candle(OkxPush<OkxCandle>),
    /// `mark-price-candle{bar}` (business socket)
    MarkPriceCandle(OkxPush<OkxPriceCandle>),
    /// `index-candle{bar}` (business socket)
    IndexCandle(OkxPush<OkxPriceCandle>),
}

#[derive(Deserialize)]
struct Probe {
    #[serde(default)]
    event: Option<String>,
    #[serde(default)]
    arg: Option<ProbeArg>,
}

#[derive(Deserialize)]
struct ProbeArg {
    channel: String,
}

fn typed<T: DeserializeOwned>(text: &str, channel: &str) -> Result<T> {
    serde_json::from_str(text).map_err(|e| anyhow!("failed to parse OKX {channel} message: {e}"))
}

impl OkxWsMessage {
    /// Parse one text frame. Unknown channels and malformed payloads are errors.
    pub fn parse(text: &str) -> Result<Self> {
        if text == "pong" {
            return Ok(Self::Pong);
        }
        let probe: Probe =
            serde_json::from_str(text).map_err(|e| anyhow!("invalid OKX ws frame: {e}"))?;
        if probe.event.is_some() {
            return Ok(Self::Event(typed(text, "event")?));
        }
        let channel = probe
            .arg
            .ok_or_else(|| anyhow!("OKX ws frame has neither event nor arg: {text}"))?
            .channel;
        let c = channel.as_str();
        Ok(match c {
            "books" | "books-rpi" | "books-l2-tbt" | "books50-l2-tbt" => {
                Self::Books(typed(text, c)?)
            }
            "books5" => Self::Books5(typed(text, c)?),
            "bbo-tbt" | "bbo-rpi-tbt" => Self::Bbo(typed(text, c)?),
            "trades" => Self::Trades(typed(text, c)?),
            "trades-all" => Self::TradesAll(typed(text, c)?),
            "tickers" => Self::Tickers(typed(text, c)?),
            "index-tickers" => Self::IndexTickers(typed(text, c)?),
            "mark-price" => Self::MarkPrice(typed(text, c)?),
            "funding-rate" => Self::FundingRate(typed(text, c)?),
            "open-interest" => Self::OpenInterest(typed(text, c)?),
            "price-limit" => Self::PriceLimit(typed(text, c)?),
            "estimated-price" => Self::EstimatedPrice(typed(text, c)?),
            "liquidation-orders" => Self::LiquidationOrders(typed(text, c)?),
            "instruments" => Self::Instruments(typed(text, c)?),
            "opt-summary" => Self::OptSummary(typed(text, c)?),
            "option-trades" => Self::OptionTrades(typed(text, c)?),
            "call-auction-details" => Self::CallAuctionDetails(typed(text, c)?),
            "status" => Self::Status(typed(text, c)?),
            "event-contract-markets" => Self::EventContractMarkets(typed(text, c)?),
            c if c.starts_with("mark-price-candle") => Self::MarkPriceCandle(typed(text, c)?),
            c if c.starts_with("index-candle") => Self::IndexCandle(typed(text, c)?),
            c if c.starts_with("candle") => Self::Candle(typed(text, c)?),
            other => bail!("unsupported OKX ws channel: {other}"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    macro_rules! fixture {
        ($name:literal) => {
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/ws/",
                $name,
                ".json"
            ))
        };
    }

    fn without(text: &str, field: &str) -> String {
        let mut v: Value = serde_json::from_str(text).unwrap();
        let removed = v["data"][0].as_object_mut().unwrap().remove(field);
        assert!(removed.is_some(), "{field} not in fixture");
        v.to_string()
    }

    fn short_row(text: &str) -> String {
        let mut v: Value = serde_json::from_str(text).unwrap();
        v["data"][0].as_array_mut().unwrap().pop();
        v.to_string()
    }

    #[test]
    fn parses_bbo_rpi_tbt() {
        let msg = OkxWsMessage::parse(fixture!("bbo_rpi_tbt")).unwrap();
        assert!(
            matches!(msg, OkxWsMessage::Bbo(ref p) if !p.data.is_empty()),
            "{msg:?}"
        );
        assert!(OkxWsMessage::parse(&without(fixture!("bbo_rpi_tbt"), "seqId")).is_err());
    }

    #[test]
    fn parses_bbo_tbt() {
        let msg = OkxWsMessage::parse(fixture!("bbo_tbt")).unwrap();
        assert!(
            matches!(msg, OkxWsMessage::Bbo(ref p) if !p.data.is_empty()),
            "{msg:?}"
        );
        assert!(OkxWsMessage::parse(&without(fixture!("bbo_tbt"), "ts")).is_err());
    }

    #[test]
    fn parses_books5() {
        let msg = OkxWsMessage::parse(fixture!("books5")).unwrap();
        assert!(
            matches!(msg, OkxWsMessage::Books5(ref p) if !p.data.is_empty()),
            "{msg:?}"
        );
        assert!(OkxWsMessage::parse(&without(fixture!("books5"), "asks")).is_err());
    }

    #[test]
    fn parses_books_rpi_snapshot() {
        let msg = OkxWsMessage::parse(fixture!("books_rpi_snapshot")).unwrap();
        assert!(
            matches!(msg, OkxWsMessage::Books(ref p) if !p.data.is_empty()),
            "{msg:?}"
        );
        assert!(
            OkxWsMessage::parse(&without(fixture!("books_rpi_snapshot"), "prevSeqId")).is_err()
        );
    }

    #[test]
    fn parses_books_rpi_update() {
        let msg = OkxWsMessage::parse(fixture!("books_rpi_update")).unwrap();
        assert!(
            matches!(msg, OkxWsMessage::Books(ref p) if !p.data.is_empty()),
            "{msg:?}"
        );
        assert!(OkxWsMessage::parse(&without(fixture!("books_rpi_update"), "seqId")).is_err());
    }

    #[test]
    fn parses_books_snapshot() {
        let msg = OkxWsMessage::parse(fixture!("books_snapshot")).unwrap();
        assert!(
            matches!(msg, OkxWsMessage::Books(ref p) if !p.data.is_empty()),
            "{msg:?}"
        );
        assert!(OkxWsMessage::parse(&without(fixture!("books_snapshot"), "prevSeqId")).is_err());
    }

    #[test]
    fn parses_books_update() {
        let msg = OkxWsMessage::parse(fixture!("books_update")).unwrap();
        assert!(
            matches!(msg, OkxWsMessage::Books(ref p) if !p.data.is_empty()),
            "{msg:?}"
        );
        assert!(OkxWsMessage::parse(&without(fixture!("books_update"), "bids")).is_err());
    }

    #[test]
    fn parses_funding_rate() {
        let msg = OkxWsMessage::parse(fixture!("funding_rate")).unwrap();
        assert!(
            matches!(msg, OkxWsMessage::FundingRate(ref p) if !p.data.is_empty()),
            "{msg:?}"
        );
        assert!(OkxWsMessage::parse(&without(fixture!("funding_rate"), "fundingRate")).is_err());
    }

    #[test]
    fn parses_index_tickers() {
        let msg = OkxWsMessage::parse(fixture!("index_tickers")).unwrap();
        assert!(
            matches!(msg, OkxWsMessage::IndexTickers(ref p) if !p.data.is_empty()),
            "{msg:?}"
        );
        assert!(OkxWsMessage::parse(&without(fixture!("index_tickers"), "idxPx")).is_err());
    }

    #[test]
    fn parses_instruments_events() {
        let msg = OkxWsMessage::parse(fixture!("instruments_events")).unwrap();
        assert!(
            matches!(msg, OkxWsMessage::Instruments(ref p) if !p.data.is_empty()),
            "{msg:?}"
        );
        assert!(OkxWsMessage::parse(&without(fixture!("instruments_events"), "instId")).is_err());
    }

    #[test]
    fn parses_liquidation_orders() {
        let msg = OkxWsMessage::parse(fixture!("liquidation_orders")).unwrap();
        assert!(
            matches!(msg, OkxWsMessage::LiquidationOrders(ref p) if !p.data.is_empty()),
            "{msg:?}"
        );
        assert!(OkxWsMessage::parse(&without(fixture!("liquidation_orders"), "details")).is_err());
    }

    #[test]
    fn parses_mark_price() {
        let msg = OkxWsMessage::parse(fixture!("mark_price")).unwrap();
        assert!(
            matches!(msg, OkxWsMessage::MarkPrice(ref p) if !p.data.is_empty()),
            "{msg:?}"
        );
        assert!(OkxWsMessage::parse(&without(fixture!("mark_price"), "markPx")).is_err());
    }

    #[test]
    fn parses_open_interest() {
        let msg = OkxWsMessage::parse(fixture!("open_interest")).unwrap();
        assert!(
            matches!(msg, OkxWsMessage::OpenInterest(ref p) if !p.data.is_empty()),
            "{msg:?}"
        );
        assert!(OkxWsMessage::parse(&without(fixture!("open_interest"), "oiUsd")).is_err());
    }

    #[test]
    fn parses_opt_summary() {
        let msg = OkxWsMessage::parse(fixture!("opt_summary")).unwrap();
        assert!(
            matches!(msg, OkxWsMessage::OptSummary(ref p) if !p.data.is_empty()),
            "{msg:?}"
        );
        assert!(OkxWsMessage::parse(&without(fixture!("opt_summary"), "delta")).is_err());
    }

    #[test]
    fn parses_option_trades() {
        let msg = OkxWsMessage::parse(fixture!("option_trades")).unwrap();
        assert!(
            matches!(msg, OkxWsMessage::OptionTrades(ref p) if !p.data.is_empty()),
            "{msg:?}"
        );
        assert!(OkxWsMessage::parse(&without(fixture!("option_trades"), "fillVol")).is_err());
    }

    #[test]
    fn parses_price_limit() {
        let msg = OkxWsMessage::parse(fixture!("price_limit")).unwrap();
        assert!(
            matches!(msg, OkxWsMessage::PriceLimit(ref p) if !p.data.is_empty()),
            "{msg:?}"
        );
        assert!(OkxWsMessage::parse(&without(fixture!("price_limit"), "buyLmt")).is_err());
    }

    #[test]
    fn parses_status() {
        let msg = OkxWsMessage::parse(fixture!("status")).unwrap();
        assert!(
            matches!(msg, OkxWsMessage::Status(ref p) if !p.data.is_empty()),
            "{msg:?}"
        );
        assert!(OkxWsMessage::parse(&without(fixture!("status"), "state")).is_err());
    }

    #[test]
    fn parses_tickers() {
        let msg = OkxWsMessage::parse(fixture!("tickers")).unwrap();
        assert!(
            matches!(msg, OkxWsMessage::Tickers(ref p) if !p.data.is_empty()),
            "{msg:?}"
        );
        assert!(OkxWsMessage::parse(&without(fixture!("tickers"), "last")).is_err());
    }

    #[test]
    fn parses_trades() {
        let msg = OkxWsMessage::parse(fixture!("trades")).unwrap();
        assert!(
            matches!(msg, OkxWsMessage::Trades(ref p) if !p.data.is_empty()),
            "{msg:?}"
        );
        assert!(OkxWsMessage::parse(&without(fixture!("trades"), "count")).is_err());
    }

    #[test]
    fn parses_trades_all() {
        let msg = OkxWsMessage::parse(fixture!("trades_all")).unwrap();
        assert!(
            matches!(msg, OkxWsMessage::TradesAll(ref p) if !p.data.is_empty()),
            "{msg:?}"
        );
        assert!(OkxWsMessage::parse(&without(fixture!("trades_all"), "tradeId")).is_err());
    }

    #[test]
    fn parses_event_contract_markets() {
        let msg = OkxWsMessage::parse(fixture!("event_contract_markets")).unwrap();
        assert!(
            matches!(msg, OkxWsMessage::EventContractMarkets(ref p) if p.data[0].floor_strike == "64731.7"),
            "{msg:?}"
        );
        assert!(
            OkxWsMessage::parse(&without(fixture!("event_contract_markets"), "disputed")).is_err()
        );
    }

    #[test]
    fn parses_candles() {
        match OkxWsMessage::parse(fixture!("candle1m")).unwrap() {
            OkxWsMessage::Candle(p) => {
                assert_eq!(p.arg, OkxWssArg::candle("1m", "BTC-USDT-SWAP"));
                assert_eq!(p.data[0].confirm, 0);
            }
            other => panic!("{other:?}"),
        }
        match OkxWsMessage::parse(fixture!("mark_price_candle1m")).unwrap() {
            OkxWsMessage::MarkPriceCandle(p) => assert_eq!(p.data[0].ts, 1794827040000),
            other => panic!("{other:?}"),
        }
        match OkxWsMessage::parse(fixture!("index_candle1m")).unwrap() {
            OkxWsMessage::IndexCandle(p) => assert_eq!(p.arg.inst_id.as_deref(), Some("BTC-USDT")),
            other => panic!("{other:?}"),
        }
        for name in [
            fixture!("candle1m"),
            fixture!("mark_price_candle1m"),
            fixture!("index_candle1m"),
        ] {
            assert!(OkxWsMessage::parse(&short_row(name)).is_err());
        }
    }

    #[test]
    fn parses_book_details() {
        match OkxWsMessage::parse(fixture!("books_snapshot")).unwrap() {
            OkxWsMessage::Books(m) => {
                assert!(m.is_snapshot());
                assert_eq!(m.inst_id(), Some("BTC-USDT-SWAP"));
                assert_eq!(m.data[0].prev_seq_id, -1);
            }
            other => panic!("{other:?}"),
        }
        match OkxWsMessage::parse(fixture!("books5")).unwrap() {
            OkxWsMessage::Books5(m) => {
                assert_eq!(m.data[0].inst_id.as_deref(), Some("BTC-USDT-SWAP"))
            }
            other => panic!("{other:?}"),
        }
        match OkxWsMessage::parse(fixture!("bbo_rpi_tbt")).unwrap() {
            OkxWsMessage::Bbo(m) => assert_eq!(m.arg.channel, "bbo-rpi-tbt"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn parses_events() {
        let OkxWsMessage::Event(e) = OkxWsMessage::parse(fixture!("event_subscribe")).unwrap()
        else {
            panic!("not an event")
        };
        assert_eq!(e.event, "subscribe");
        assert_eq!(e.arg, Some(OkxWssArg::books("BTC-USDT-SWAP")));
        let OkxWsMessage::Event(e) = OkxWsMessage::parse(fixture!("event_unsubscribe")).unwrap()
        else {
            panic!("not an event")
        };
        assert_eq!(e.event, "unsubscribe");
        let OkxWsMessage::Event(e) = OkxWsMessage::parse(fixture!("event_error_login")).unwrap()
        else {
            panic!("not an event")
        };
        assert_eq!(
            (e.event.as_str(), e.code.as_deref()),
            ("error", Some("60011"))
        );
        let OkxWsMessage::Event(e) =
            OkxWsMessage::parse(fixture!("event_error_wrong_url")).unwrap()
        else {
            panic!("not an event")
        };
        assert_eq!(e.code.as_deref(), Some("60018"));
        assert!(OkxWsMessage::parse(&{
            let mut v: Value = serde_json::from_str(fixture!("event_subscribe")).unwrap();
            v.as_object_mut().unwrap().remove("connId");
            v.to_string()
        })
        .is_err());
    }

    #[test]
    fn pong_and_unknown() {
        assert!(matches!(
            OkxWsMessage::parse("pong").unwrap(),
            OkxWsMessage::Pong
        ));
        assert!(OkxWsMessage::parse(r#"{"arg":{"channel":"nope"},"data":[]}"#).is_err());
        assert!(OkxWsMessage::parse("not json").is_err());
    }

    #[test]
    fn subscribe_json() {
        let msg = OkxWssMessage::subscribe(vec![
            OkxWssArg::books("BTC-USDT-SWAP"),
            OkxWssArg::estimated_price("FUTURES", Some("BTC-USD"), None),
            OkxWssArg::status(),
        ]);
        assert_eq!(
            msg.to_json(),
            r#"{"op":"subscribe","args":[{"channel":"books","instId":"BTC-USDT-SWAP"},{"channel":"estimated-price","instType":"FUTURES","instFamily":"BTC-USD"},{"channel":"status"}]}"#
        );
        let unsub = OkxWssMessage::unsubscribe(vec![OkxWssArg::trades("BTC-USDT-SWAP")]);
        assert_eq!(
            unsub.to_json(),
            r#"{"op":"unsubscribe","args":[{"channel":"trades","instId":"BTC-USDT-SWAP"}]}"#
        );
        assert_eq!(OkxWssMessage::ping(), "ping");
    }

    #[test]
    fn business_routing() {
        assert!(OkxWssArg::candle("1H", "BTC-USDT").is_business());
        assert!(OkxWssArg::mark_price_candle("1m", "BTC-USDT-SWAP").is_business());
        assert!(OkxWssArg::index_candle("1m", "BTC-USDT").is_business());
        assert!(OkxWssArg::trades_all("BTC-USDT").is_business());
        assert!(!OkxWssArg::event_contract_markets("EVENTS").is_business());
        assert!(!OkxWssArg::mark_price("BTC-USDT-SWAP").is_business());
        assert!(!OkxWssArg::books("BTC-USDT-SWAP").is_business());
        assert!(!OkxWssArg::index_tickers("BTC-USDT").is_business());
    }
}
