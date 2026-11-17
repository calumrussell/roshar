use serde::de::{DeserializeOwned, Error as _};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::value::RawValue;
use serde_json::{Map, Value};
use std::borrow::Cow;

use super::rest::{DeribitBlockRfq, DeribitInstrument, DeribitPriceLevel, DeribitRpcResponse};
use crate::request::DeribitRequest;

/// A field the exchange always sends but whose value may be `null`.
/// Unlike a bare `Option<T>`, a missing key is an error.
pub(crate) fn nullable<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}

/// For partial updates: missing key => `None`, `null` => `Some(None)`.
/// Use with `#[serde(default)]`.
fn present<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(Some)
}

/// An outgoing JSON-RPC request frame.
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
pub struct DeribitWssMessage {
    pub jsonrpc: String,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Map<String, Value>>,
    pub id: u64,
}

impl DeribitWssMessage {
    /// Wraps a request with a caller-chosen `id`; replies echo it. Empty params are omitted.
    pub fn new(request: DeribitRequest, id: u64) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            method: request.method.to_string(),
            params: (!request.params.is_empty()).then_some(request.params),
            id,
        }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("failed to serialize DeribitWssMessage")
    }
}

/// Any frame the server sends on a public connection.
#[derive(Debug, Clone)]
pub enum DeribitWsIncoming {
    /// `method: "subscription"`.
    Subscription(DeribitSubscriptionMessage),
    /// `method: "heartbeat"`.
    Heartbeat(DeribitHeartbeatMessage),
    /// Reply to a request; match on `id` and parse with `result_as`.
    Response(DeribitRpcResponse<Box<RawValue>>),
}

impl DeribitWsIncoming {
    pub fn parse(text: &str) -> serde_json::Result<Self> {
        #[derive(Deserialize)]
        struct Peek<'a> {
            #[serde(borrow)]
            method: Option<Cow<'a, str>>,
        }
        let peek: Peek<'_> = serde_json::from_str(text)?;
        match peek.method.as_deref() {
            Some("subscription") => serde_json::from_str(text).map(Self::Subscription),
            Some("heartbeat") => serde_json::from_str(text).map(Self::Heartbeat),
            Some(other) => Err(serde_json::Error::custom(format!(
                "unknown notification method `{other}`"
            ))),
            None => serde_json::from_str(text).map(Self::Response),
        }
    }
}

impl DeribitRpcResponse<Box<RawValue>> {
    /// Parses `result` as the type of the method whose request carried this `id`;
    /// an exchange error becomes `Err`.
    pub fn result_as<T: DeserializeOwned>(&self) -> anyhow::Result<T> {
        if let Some(error) = &self.error {
            return Err(error.clone().into());
        }
        let raw = self
            .result
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Deribit response has neither result nor error"))?;
        Ok(serde_json::from_str(raw.get())?)
    }
}

/// `{"jsonrpc":"2.0","method":"heartbeat","params":{"type":...}}`, sent after
/// `public/set_heartbeat`.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeribitHeartbeatMessage {
    pub jsonrpc: String,
    pub method: String,
    pub params: DeribitHeartbeatParams,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeribitHeartbeatParams {
    #[serde(rename = "type")]
    pub heartbeat_type: DeribitHeartbeatType,
}

#[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DeribitHeartbeatType {
    /// Server-side keepalive; no reply needed.
    Heartbeat,
    /// Reply with `public/test`.
    TestRequest,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeribitSubscriptionMessage {
    pub jsonrpc: String,
    pub method: String,
    pub params: DeribitSubscriptionParams,
}

/// `data` is parsed according to the `channel` prefix.
#[derive(Debug, Serialize, Clone)]
pub struct DeribitSubscriptionParams {
    pub channel: String,
    pub data: DeribitData,
}

impl<'de> Deserialize<'de> for DeribitSubscriptionParams {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Raw {
            channel: String,
            data: Box<RawValue>,
        }
        let raw = Raw::deserialize(deserializer)?;
        let data = DeribitData::parse(&raw.channel, raw.data.get()).map_err(D::Error::custom)?;
        Ok(Self {
            channel: raw.channel,
            data,
        })
    }
}

/// Notification payload by channel.
#[derive(Debug, Serialize, Clone)]
#[serde(untagged)]
pub enum DeribitData {
    /// `book.{instrument_name}.{interval}`.
    Book(DeribitOrderBookData),
    /// `book.{instrument_name}.{group}.{depth}.{interval}`.
    GroupedBook(DeribitGroupedBookData),
    /// `trades.{instrument_name}.{interval}` and `trades.{kind}.{currency}.{interval}`.
    Trades(Vec<DeribitTradeData>),
    /// `ticker.{instrument_name}.{interval}`.
    Ticker(DeribitTickerData),
    /// `incremental_ticker.{instrument_name}`.
    IncrementalTicker(DeribitIncrementalTickerData),
    /// `chart.trades.{instrument_name}.{resolution}`.
    ChartTrades(DeribitChartTradesData),
    /// `quote.{instrument_name}`.
    Quote(DeribitQuoteData),
    /// `deribit_price_index.{index_name}`.
    PriceIndex(DeribitPriceIndexData),
    /// `deribit_price_ranking.{index_name}`.
    PriceRanking(Vec<DeribitPriceRankingEntry>),
    /// `deribit_price_statistics.{index_name}`.
    PriceStatistics(DeribitPriceStatisticsData),
    /// `deribit_volatility_index.{index_name}`.
    VolatilityIndex(DeribitVolatilityIndexTick),
    /// `estimated_expiration_price.{index_name}`.
    EstimatedExpirationPrice(DeribitEstimatedExpirationPriceData),
    /// `markprice.options.{index_name}`.
    MarkPriceOptions(Vec<DeribitMarkPriceOption>),
    /// `perpetual.{instrument_name}.{interval}`.
    Perpetual(DeribitPerpetualData),
    /// `instrument.state.{kind}.{currency}`.
    InstrumentState(DeribitInstrumentStateData),
    /// `instrument.creation.{kind}.{currency}`.
    InstrumentCreation(Box<DeribitInstrument>),
    /// `platform_state`.
    PlatformState(DeribitPlatformStateData),
    /// `platform_state.public_methods_state`.
    PublicMethodsState(DeribitPublicMethodsStateData),
    /// `block_rfq.trades.{currency}`: one Block RFQ per notification (the AsyncAPI schema says
    /// array; live frames and the documented example are a single object).
    BlockRfqTrade(Box<DeribitBlockRfq>),
}

impl DeribitData {
    /// Parses a notification's `data` for the given channel name.
    pub fn parse(channel: &str, data: &str) -> serde_json::Result<Self> {
        fn p<T: DeserializeOwned>(data: &str) -> serde_json::Result<T> {
            serde_json::from_str(data)
        }
        let (prefix, rest) = channel.split_once('.').unwrap_or((channel, ""));
        Ok(match prefix {
            // `book.{instrument}.{interval}` vs `book.{instrument}.{group}.{depth}.{interval}`;
            // instrument names contain no dots.
            "book" if rest.matches('.').count() == 1 => Self::Book(p(data)?),
            "book" => Self::GroupedBook(p(data)?),
            "trades" => Self::Trades(p(data)?),
            "ticker" => Self::Ticker(p(data)?),
            "incremental_ticker" => Self::IncrementalTicker(p(data)?),
            "chart" if rest.starts_with("trades.") => Self::ChartTrades(p(data)?),
            "quote" => Self::Quote(p(data)?),
            "deribit_price_index" => Self::PriceIndex(p(data)?),
            "deribit_price_ranking" => Self::PriceRanking(p(data)?),
            "deribit_price_statistics" => Self::PriceStatistics(p(data)?),
            "deribit_volatility_index" => Self::VolatilityIndex(p(data)?),
            "estimated_expiration_price" => Self::EstimatedExpirationPrice(p(data)?),
            "markprice" if rest.starts_with("options.") => Self::MarkPriceOptions(p(data)?),
            "perpetual" => Self::Perpetual(p(data)?),
            "instrument" if rest.starts_with("state.") => Self::InstrumentState(p(data)?),
            "instrument" if rest.starts_with("creation.") => Self::InstrumentCreation(p(data)?),
            "platform_state" if rest.is_empty() => Self::PlatformState(p(data)?),
            "platform_state" if rest == "public_methods_state" => {
                Self::PublicMethodsState(p(data)?)
            }
            "block_rfq" if rest.starts_with("trades.") => Self::BlockRfqTrade(p(data)?),
            _ => {
                return Err(serde_json::Error::custom(format!(
                    "unsupported channel `{channel}`"
                )))
            }
        })
    }
}

#[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DeribitBookAction {
    New,
    Change,
    Delete,
}

/// `[action, price, amount]`. Amount is USD for perpetuals and inverse futures,
/// base currency for options and linear instruments.
pub type DeribitBookLevel = (DeribitBookAction, f64, f64);

#[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DeribitBookUpdateType {
    Snapshot,
    Change,
}

/// `book.{instrument_name}.{interval}`: a full snapshot, then incremental changes.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeribitOrderBookData {
    pub timestamp: u64,
    pub instrument_name: String,
    pub change_id: u64,
    /// Absent on the first (snapshot) notification.
    pub prev_change_id: Option<u64>,
    /// Documented as optional.
    #[serde(rename = "type")]
    pub update_type: Option<DeribitBookUpdateType>,
    pub bids: Vec<DeribitBookLevel>,
    pub asks: Vec<DeribitBookLevel>,
}

/// One trade from `trades.{instrument_name}.{interval}`. Delayed Block RFQ trades
/// keep their original `timestamp`/`trade_seq`, so they can arrive out of sequence.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeribitTradeData {
    pub trade_seq: u64,
    pub trade_id: String,
    pub timestamp: u64,
    pub price: f64,
    pub amount: f64,
    /// Taker side: `buy` or `sell`.
    pub direction: String,
    /// 0 plus tick, 1 zero-plus, 2 minus, 3 zero-minus.
    pub tick_direction: u8,
    pub instrument_name: String,
    pub index_price: f64,
    pub mark_price: f64,
    pub contracts: Option<f64>,
    /// Options only.
    pub iv: Option<f64>,
    /// `M`, `T` or `MT`; only on liquidation trades.
    pub liquidation: Option<String>,
    pub block_trade_id: Option<String>,
    pub block_trade_leg_count: Option<u32>,
    pub block_rfq_id: Option<u64>,
    /// Publication time (ms) of a delayed Block RFQ trade.
    pub block_delayed: Option<u64>,
    pub combo_id: Option<String>,
    pub combo_trade_id: Option<String>,
    /// Starbase-matched trades only.
    pub starbase_match_id: Option<u64>,
    /// Nanoseconds; Starbase-matched trades only.
    pub starbase_timestamp: Option<u64>,
    /// Combo trades only: the per-leg trades.
    pub legs: Option<Vec<DeribitTradeData>>,
}

/// `ticker.{instrument_name}.{interval}`.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeribitTickerData {
    pub timestamp: u64,
    pub instrument_name: String,
    /// `open`, `settlement`, `delivered`, `inactive`, `locked`, `halted` or `archivized`.
    pub state: String,
    pub stats: DeribitTickerStats,
    #[serde(deserialize_with = "nullable")]
    pub last_price: Option<f64>,
    pub index_price: f64,
    pub mark_price: f64,
    #[serde(deserialize_with = "nullable")]
    pub best_bid_price: Option<f64>,
    #[serde(deserialize_with = "nullable")]
    pub best_bid_amount: Option<f64>,
    #[serde(deserialize_with = "nullable")]
    pub best_ask_price: Option<f64>,
    #[serde(deserialize_with = "nullable")]
    pub best_ask_amount: Option<f64>,
    pub min_price: f64,
    pub max_price: f64,
    /// Futures and options only (absent for spot). Reported with an 8-hour delay.
    pub open_interest: Option<f64>,
    /// Futures and options only (absent for spot).
    pub estimated_delivery_price: Option<f64>,
    pub settlement_price: Option<f64>,
    pub delivery_price: Option<f64>,
    /// Perpetuals only.
    pub funding_8h: Option<f64>,
    pub current_funding: Option<f64>,
    pub interest_value: Option<f64>,
    // Options only.
    pub bid_iv: Option<f64>,
    pub ask_iv: Option<f64>,
    pub mark_iv: Option<f64>,
    pub underlying_index: Option<String>,
    pub underlying_price: Option<f64>,
    pub interest_rate: Option<f64>,
    pub greeks: Option<DeribitGreeks>,
    // RWA perpetuals with an active anchor price only.
    pub is_anchor_breached: Option<bool>,
    pub anchor_min_price: Option<f64>,
    pub anchor_max_price: Option<f64>,
}

/// 24-hour statistics in a ticker.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeribitTickerStats {
    pub volume: f64,
    #[serde(deserialize_with = "nullable")]
    pub high: Option<f64>,
    #[serde(deserialize_with = "nullable")]
    pub low: Option<f64>,
    /// Percent; `null` when there is no 24-hour price.
    pub price_change: Option<f64>,
    pub volume_realtime: Option<f64>,
    pub volume_delayed: Option<f64>,
    pub volume_usd: Option<f64>,
    pub volume_realtime_usd: Option<f64>,
    pub volume_delayed_usd: Option<f64>,
    pub volume_notional: Option<f64>,
    pub volume_realtime_notional: Option<f64>,
    pub volume_delayed_notional: Option<f64>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeribitGreeks {
    pub delta: f64,
    pub gamma: f64,
    pub rho: f64,
    pub theta: f64,
    pub vega: f64,
}

/// `book.{instrument_name}.{group}.{depth}.{interval}`: the whole grouped top of book in
/// every notification (no actions, no `prev_change_id`).
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeribitGroupedBookData {
    pub timestamp: u64,
    pub instrument_name: String,
    pub change_id: u64,
    pub bids: Vec<DeribitPriceLevel>,
    pub asks: Vec<DeribitPriceLevel>,
}

/// `incremental_ticker.{instrument_name}`. The first notification (`type: snapshot`) carries
/// every ticker field; later ones (`type: change`) carry only fields that changed. A field
/// that can be `null` is `Option<Option<_>>`: outer `None` = unchanged, `Some(None)` = now null.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeribitIncrementalTickerData {
    #[serde(rename = "type")]
    pub update_type: DeribitBookUpdateType,
    pub timestamp: u64,
    pub instrument_name: String,
    pub state: Option<String>,
    pub stats: Option<DeribitIncrementalTickerStats>,
    #[serde(default, deserialize_with = "present")]
    pub last_price: Option<Option<f64>>,
    #[serde(default, deserialize_with = "present")]
    pub best_bid_price: Option<Option<f64>>,
    #[serde(default, deserialize_with = "present")]
    pub best_bid_amount: Option<Option<f64>>,
    #[serde(default, deserialize_with = "present")]
    pub best_ask_price: Option<Option<f64>>,
    #[serde(default, deserialize_with = "present")]
    pub best_ask_amount: Option<Option<f64>>,
    pub index_price: Option<f64>,
    pub mark_price: Option<f64>,
    pub min_price: Option<f64>,
    pub max_price: Option<f64>,
    pub open_interest: Option<f64>,
    pub estimated_delivery_price: Option<f64>,
    pub settlement_price: Option<f64>,
    pub delivery_price: Option<f64>,
    pub funding_8h: Option<f64>,
    pub current_funding: Option<f64>,
    pub interest_value: Option<f64>,
    pub bid_iv: Option<f64>,
    pub ask_iv: Option<f64>,
    pub mark_iv: Option<f64>,
    pub underlying_index: Option<String>,
    pub underlying_price: Option<f64>,
    pub interest_rate: Option<f64>,
    pub greeks: Option<DeribitIncrementalGreeks>,
}

/// Changed 24-hour statistics in an `incremental_ticker` notification.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeribitIncrementalTickerStats {
    pub volume: Option<f64>,
    #[serde(default, deserialize_with = "present")]
    pub high: Option<Option<f64>>,
    #[serde(default, deserialize_with = "present")]
    pub low: Option<Option<f64>>,
    #[serde(default, deserialize_with = "present")]
    pub price_change: Option<Option<f64>>,
    pub volume_realtime: Option<f64>,
    pub volume_delayed: Option<f64>,
    pub volume_usd: Option<f64>,
    pub volume_realtime_usd: Option<f64>,
    pub volume_delayed_usd: Option<f64>,
    pub volume_notional: Option<f64>,
    pub volume_realtime_notional: Option<f64>,
    pub volume_delayed_notional: Option<f64>,
}

/// Changed greeks in an `incremental_ticker` notification.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeribitIncrementalGreeks {
    pub delta: Option<f64>,
    pub gamma: Option<f64>,
    pub rho: Option<f64>,
    pub theta: Option<f64>,
    pub vega: Option<f64>,
}

/// `chart.trades.{instrument_name}.{resolution}`: the current (still updating) candle.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeribitChartTradesData {
    /// Candle open time, ms.
    pub tick: u64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
    pub cost: f64,
}

/// `quote.{instrument_name}`.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeribitQuoteData {
    pub timestamp: u64,
    pub instrument_name: String,
    #[serde(deserialize_with = "nullable")]
    pub best_bid_price: Option<f64>,
    #[serde(deserialize_with = "nullable")]
    pub best_bid_amount: Option<f64>,
    #[serde(deserialize_with = "nullable")]
    pub best_ask_price: Option<f64>,
    #[serde(deserialize_with = "nullable")]
    pub best_ask_amount: Option<f64>,
}

/// `deribit_price_index.{index_name}`.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeribitPriceIndexData {
    pub timestamp: u64,
    pub index_name: String,
    pub price: f64,
}

/// One constituent exchange in `deribit_price_ranking.{index_name}`.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeribitPriceRankingEntry {
    /// Exchange id, e.g. `coinbase`.
    pub identifier: String,
    pub enabled: bool,
    pub timestamp: u64,
    /// Percent.
    pub weight: f64,
    #[serde(deserialize_with = "nullable")]
    pub price: Option<f64>,
    #[serde(deserialize_with = "nullable")]
    pub original_price: Option<f64>,
}

/// `deribit_price_statistics.{index_name}`.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeribitPriceStatisticsData {
    pub index_name: String,
    pub low24h: f64,
    pub high24h: f64,
    pub change24h: f64,
    /// Documented as optional.
    pub fast_market: Option<bool>,
}

/// `deribit_volatility_index.{index_name}` (DVOL).
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeribitVolatilityIndexTick {
    pub timestamp: u64,
    pub index_name: String,
    pub volatility: f64,
}

/// `estimated_expiration_price.{index_name}`.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeribitEstimatedExpirationPriceData {
    pub timestamp: u64,
    /// Seconds to the next expiration.
    pub seconds: i64,
    pub price: f64,
    pub is_estimated: bool,
    /// Documented as optional.
    pub left_ticks: Option<f64>,
    pub total_ticks: Option<f64>,
}

/// One option in `markprice.options.{index_name}`.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeribitMarkPriceOption {
    pub timestamp: u64,
    pub instrument_name: String,
    pub mark_price: f64,
    /// Fraction (0.4659 = 46.59%).
    pub iv: f64,
}

/// `perpetual.{instrument_name}.{interval}`.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeribitPerpetualData {
    pub timestamp: u64,
    pub index_price: f64,
    /// Current perpetual interest (funding) rate.
    pub interest: f64,
}

/// `instrument.state.{kind}.{currency}`.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeribitInstrumentStateData {
    pub timestamp: u64,
    pub instrument_name: String,
    /// `created`, `open`, `settlement`, `delivered`, `inactive`, `locked`, `halted`,
    /// `archivized`, ...
    pub state: String,
}

/// `platform_state`: lock state of one price index, or platform maintenance.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeribitPlatformStateData {
    pub price_index: Option<String>,
    pub locked: Option<bool>,
    pub maintenance: Option<bool>,
    /// Undocumented; present (and empty) in every capture.
    pub reasons: Option<Vec<String>>,
}

/// `platform_state.public_methods_state`.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeribitPublicMethodsStateData {
    pub allow_unauthenticated_public_requests: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::DeribitVersion;

    // Fixtures are frames captured from wss://www.deribit.com/ws/api/v2 on 2026-11-15/16.
    const BOOK_SNAPSHOT: &str = include_str!("../../tests/fixtures/book_snapshot.json");
    const BOOK_CHANGE: &str = include_str!("../../tests/fixtures/book_change.json");
    const TRADES: &str = include_str!("../../tests/fixtures/trades_perpetual.json");
    const TICKER_PERP: &str = include_str!("../../tests/fixtures/ticker_perpetual.json");
    const TICKER_OPTION: &str = include_str!("../../tests/fixtures/ticker_option.json");
    const TICKER_SPOT: &str = include_str!("../../tests/fixtures/ticker_spot.json");
    const HEARTBEAT: &str = include_str!("../../tests/fixtures/heartbeat_test_request.json");

    macro_rules! fixture {
        ($name:literal) => {
            include_str!(concat!("../../tests/fixtures/", $name, ".json"))
        };
    }

    fn data(json: &str) -> DeribitData {
        serde_json::from_str::<DeribitSubscriptionMessage>(json)
            .unwrap()
            .params
            .data
    }

    fn ticker(json: &str) -> DeribitTickerData {
        match data(json) {
            DeribitData::Ticker(t) => t,
            other => panic!("expected ticker, got {other:?}"),
        }
    }

    fn response(json: &str) -> DeribitRpcResponse<Box<RawValue>> {
        match DeribitWsIncoming::parse(json).unwrap() {
            DeribitWsIncoming::Response(r) => r,
            other => panic!("expected response, got {other:?}"),
        }
    }

    /// Removes `key` from the (first) notification `data` object and re-parses the frame.
    fn without(json: &str, key: &str) -> serde_json::Result<DeribitSubscriptionMessage> {
        let mut v: serde_json::Value = serde_json::from_str(json).unwrap();
        let d = &mut v["params"]["data"];
        let obj = match d {
            serde_json::Value::Array(a) => a[0].as_object_mut().unwrap(),
            other => other.as_object_mut().unwrap(),
        };
        assert!(obj.remove(key).is_some(), "fixture lacks {key}");
        serde_json::from_str(&v.to_string())
    }

    #[test]
    fn test_wss_message_subscribe() {
        let req = DeribitRequest::subscribe(vec!["book.BTC-PERPETUAL.100ms".to_string()]);
        let msg = DeribitWssMessage::new(req, 7);
        let parsed: serde_json::Value = serde_json::from_str(&msg.to_json()).unwrap();

        assert_eq!(parsed["jsonrpc"], "2.0");
        assert_eq!(parsed["method"], "public/subscribe");
        assert_eq!(parsed["params"]["channels"][0], "book.BTC-PERPETUAL.100ms");
        assert_eq!(parsed["id"], 7);
    }

    #[test]
    fn test_wss_message_set_heartbeat() {
        let msg = DeribitWssMessage::new(DeribitRequest::set_heartbeat(30), 1);
        let parsed: serde_json::Value = serde_json::from_str(&msg.to_json()).unwrap();

        assert_eq!(parsed["method"], "public/set_heartbeat");
        assert_eq!(parsed["params"]["interval"], 30);
    }

    #[test]
    fn test_wss_message_without_params_omits_them() {
        for req in [
            DeribitRequest::test(None),
            DeribitRequest::unsubscribe_all(),
            DeribitRequest::disable_heartbeat(),
        ] {
            let parsed: serde_json::Value =
                serde_json::from_str(&DeribitWssMessage::new(req, 3).to_json()).unwrap();
            assert!(parsed.get("params").is_none());
        }
    }

    #[test]
    fn test_wss_message_method_params_are_typed_json() {
        let req = DeribitRequest::get_order_book_by_instrument_id(210838, Some(5));
        let parsed: serde_json::Value =
            serde_json::from_str(&DeribitWssMessage::new(req, 4).to_json()).unwrap();
        assert_eq!(parsed["method"], "public/get_order_book_by_instrument_id");
        assert!(parsed["params"]["instrument_id"].is_u64());
        assert_eq!(parsed["params"]["depth"], 5);
    }

    #[test]
    fn test_heartbeat_test_request_parsing() {
        let msg: DeribitHeartbeatMessage = serde_json::from_str(HEARTBEAT).unwrap();
        assert_eq!(msg.method, "heartbeat");
        assert_eq!(msg.params.heartbeat_type, DeribitHeartbeatType::TestRequest);
        assert!(matches!(
            DeribitWsIncoming::parse(HEARTBEAT).unwrap(),
            DeribitWsIncoming::Heartbeat(_)
        ));
    }

    #[test]
    fn test_incoming_responses() {
        let r = response(fixture!("ws_response_hello"));
        assert_eq!(r.id, Some(100));
        assert_eq!(r.result_as::<DeribitVersion>().unwrap().version, "1.2.26");

        let r = response(fixture!("ws_response_subscribe"));
        assert_eq!(r.result_as::<Vec<String>>().unwrap().len(), 21);

        for json in [
            fixture!("ws_response_disable_heartbeat"),
            fixture!("ws_response_unsubscribe_all"),
        ] {
            assert_eq!(response(json).result_as::<String>().unwrap(), "ok");
        }

        let r = response(fixture!("ws_response_get_time"));
        assert_eq!(r.result_as::<u64>().unwrap(), 1794827335334);
    }

    #[test]
    fn test_incoming_error_response_surfaces_code() {
        let r = response(fixture!("ws_response_error_raw_unauthorized"));
        let err = r.error.clone().unwrap();
        assert_eq!(err.code, 13778);
        assert_eq!(
            err.message,
            "raw_subscriptions_not_available_for_unauthorized"
        );
        assert!(r.result_as::<Vec<String>>().is_err());
    }

    #[test]
    fn test_incoming_rejects_unknown_channel() {
        let json = r#"{"jsonrpc":"2.0","method":"subscription",
            "params":{"channel":"user.orders.any.any.raw","data":[]}}"#;
        assert!(DeribitWsIncoming::parse(json).is_err());
    }

    #[test]
    fn test_orderbook_snapshot_and_change_parsing() {
        let DeribitData::Book(book) = data(BOOK_SNAPSHOT) else {
            panic!("expected book");
        };
        assert_eq!(book.instrument_name, "BTC-PERPETUAL");
        assert_eq!(book.update_type, Some(DeribitBookUpdateType::Snapshot));
        assert!(book.prev_change_id.is_none());
        assert!(book.bids.iter().all(|l| l.0 == DeribitBookAction::New));

        let DeribitData::Book(book) = data(BOOK_CHANGE) else {
            panic!("expected book");
        };
        assert_eq!(book.update_type, Some(DeribitBookUpdateType::Change));
        assert!(book.prev_change_id.is_some());
    }

    #[test]
    fn test_orderbook_rejects_unknown_action() {
        let json = r#"{"timestamp":1,"instrument_name":"X","change_id":1,
            "bids":[["upsert",1.0,1.0]],"asks":[]}"#;
        assert!(serde_json::from_str::<DeribitOrderBookData>(json).is_err());
    }

    #[test]
    fn test_grouped_book_parsing() {
        for json in [
            fixture!("ws_book_grouped"),
            fixture!("ws_book_grouped_none_agg2"),
        ] {
            let DeribitData::GroupedBook(book) = data(json) else {
                panic!("expected grouped book");
            };
            assert_eq!(book.instrument_name, "BTC-PERPETUAL");
            assert!(book.bids.len() > 1 && !book.asks.is_empty());
            assert!(book.bids[0].0 > book.bids[1].0);
        }
        assert!(without(fixture!("ws_book_grouped"), "change_id").is_err());
    }

    #[test]
    fn test_trades_parsing() {
        let DeribitData::Trades(trades) = data(TRADES) else {
            panic!("expected trades");
        };
        assert!(!trades.is_empty());
        assert_eq!(trades[0].instrument_name, "BTC-PERPETUAL");
        assert!(trades[0].contracts.is_some());

        let DeribitData::Trades(trades) = data(fixture!("ws_trades_kind_currency")) else {
            panic!("expected trades");
        };
        assert!(trades[0].iv.is_some());
    }

    #[test]
    fn test_ticker_perpetual_parsing() {
        let t = ticker(TICKER_PERP);
        assert_eq!(t.instrument_name, "BTC-PERPETUAL");
        assert!(t.funding_8h.is_some());
        assert!(t.open_interest.is_some());
        assert!(t.stats.volume_usd.is_some());
    }

    #[test]
    fn test_ticker_option_parsing() {
        let t = ticker(TICKER_OPTION);
        assert!(t.last_price.is_none());
        assert!(t.stats.high.is_none());
        assert!(t.greeks.is_some());
        assert!(t.mark_iv.is_some());
    }

    #[test]
    fn test_ticker_spot_parsing() {
        let t = ticker(TICKER_SPOT);
        assert_eq!(t.instrument_name, "BTC_USDC");
        assert!(t.open_interest.is_none());
        assert!(t.estimated_delivery_price.is_none());
    }

    #[test]
    fn test_ticker_requires_nullable_fields_present() {
        assert!(without(TICKER_SPOT, "best_bid_price").is_err());
    }

    #[test]
    fn test_incremental_ticker_parsing() {
        let DeribitData::IncrementalTicker(t) = data(fixture!("ws_incremental_ticker_snapshot"))
        else {
            panic!("expected incremental ticker");
        };
        assert_eq!(t.update_type, DeribitBookUpdateType::Snapshot);
        assert!(t.state.is_some() && t.stats.is_some() && t.funding_8h.is_some());
        assert!(matches!(t.best_bid_price, Some(Some(_))));

        let DeribitData::IncrementalTicker(t) = data(fixture!("ws_incremental_ticker_change"))
        else {
            panic!("expected incremental ticker");
        };
        assert_eq!(t.update_type, DeribitBookUpdateType::Change);
        assert!(t.state.is_none());

        let DeribitData::IncrementalTicker(t) =
            data(fixture!("ws_incremental_ticker_option_snapshot"))
        else {
            panic!("expected incremental ticker");
        };
        assert!(t.greeks.as_ref().unwrap().gamma.is_some());

        let DeribitData::IncrementalTicker(t) =
            data(fixture!("ws_incremental_ticker_option_change"))
        else {
            panic!("expected incremental ticker");
        };
        assert!(t.greeks.as_ref().unwrap().gamma.is_none());

        let json = r#"{"type":"change","timestamp":1,"instrument_name":"X","last_price":null}"#;
        let t: DeribitIncrementalTickerData = serde_json::from_str(json).unwrap();
        assert_eq!(t.last_price, Some(None));
        assert_eq!(t.best_bid_price, None);

        assert!(without(fixture!("ws_incremental_ticker_change"), "type").is_err());
    }

    #[test]
    fn test_chart_trades_parsing() {
        let DeribitData::ChartTrades(c) = data(fixture!("ws_chart_trades")) else {
            panic!("expected chart trades");
        };
        assert!(c.high >= c.low);
        assert!(without(fixture!("ws_chart_trades"), "tick").is_err());
    }

    #[test]
    fn test_quote_parsing() {
        let DeribitData::Quote(q) = data(fixture!("ws_quote")) else {
            panic!("expected quote");
        };
        assert_eq!(q.instrument_name, "BTC-PERPETUAL");
        assert!(q.best_bid_price.is_some());
        assert!(without(fixture!("ws_quote"), "best_ask_price").is_err());
    }

    #[test]
    fn test_index_channels_parsing() {
        let DeribitData::PriceIndex(p) = data(fixture!("ws_price_index")) else {
            panic!("expected price index");
        };
        assert_eq!(p.index_name, "btc_usd");
        assert!(without(fixture!("ws_price_index"), "price").is_err());

        let DeribitData::PriceRanking(r) = data(fixture!("ws_price_ranking")) else {
            panic!("expected price ranking");
        };
        assert!(r.iter().any(|e| e.price.is_none()));
        assert!(r.iter().any(|e| e.enabled));
        assert!(without(fixture!("ws_price_ranking"), "identifier").is_err());

        let DeribitData::PriceStatistics(s) = data(fixture!("ws_price_statistics")) else {
            panic!("expected price statistics");
        };
        assert!(s.high24h >= s.low24h);
        assert!(without(fixture!("ws_price_statistics"), "change24h").is_err());

        let DeribitData::VolatilityIndex(v) = data(fixture!("ws_volatility_index")) else {
            panic!("expected volatility index");
        };
        assert!(v.volatility > 0.0);
        assert!(without(fixture!("ws_volatility_index"), "volatility").is_err());

        let DeribitData::EstimatedExpirationPrice(e) =
            data(fixture!("ws_estimated_expiration_price"))
        else {
            panic!("expected estimated expiration price");
        };
        assert!(e.seconds > 0);
        assert!(without(fixture!("ws_estimated_expiration_price"), "is_estimated").is_err());
    }

    #[test]
    fn test_markprice_options_parsing() {
        let DeribitData::MarkPriceOptions(m) = data(fixture!("ws_markprice_options")) else {
            panic!("expected markprice options");
        };
        assert!(!m.is_empty());
        assert!(without(fixture!("ws_markprice_options"), "iv").is_err());
    }

    #[test]
    fn test_perpetual_parsing() {
        let DeribitData::Perpetual(p) = data(fixture!("ws_perpetual")) else {
            panic!("expected perpetual");
        };
        assert!(p.index_price > 0.0);
        assert!(without(fixture!("ws_perpetual"), "interest").is_err());
    }

    #[test]
    fn test_platform_state_parsing() {
        let DeribitData::PlatformState(s) = data(fixture!("ws_platform_state")) else {
            panic!("expected platform state");
        };
        assert!(s.price_index.is_some());
        assert_eq!(s.locked, Some(false));

        let DeribitData::PublicMethodsState(s) = data(fixture!("ws_platform_state_public_methods"))
        else {
            panic!("expected public methods state");
        };
        assert!(s.allow_unauthenticated_public_requests);
        assert!(without(
            fixture!("ws_platform_state_public_methods"),
            "allow_unauthenticated_public_requests"
        )
        .is_err());
    }

    #[test]
    fn test_instrument_state_parsing() {
        let DeribitData::InstrumentState(s) = data(fixture!("ws_instrument_state")) else {
            panic!("expected instrument state");
        };
        assert_eq!(s.state, "inactive");
        assert!(without(fixture!("ws_instrument_state"), "state").is_err());
    }

    #[test]
    fn test_instrument_creation_parsing() {
        let DeribitData::InstrumentCreation(i) = data(fixture!("ws_instrument_creation")) else {
            panic!("expected instrument creation");
        };
        assert!(i.timestamp.is_some());
        assert!(without(fixture!("ws_instrument_creation"), "instrument_id").is_err());
    }

    #[test]
    fn test_block_rfq_trade_parsing() {
        let DeribitData::BlockRfqTrade(b) = data(fixture!("ws_block_rfq_trade")) else {
            panic!("expected block rfq trade");
        };
        assert!(b.combo_id.is_none());
        assert_eq!(b.legs.len(), 2);
        assert!(b.index_prices.is_some());
        assert!(without(fixture!("ws_block_rfq_trade"), "combo_id").is_err());
    }
}
