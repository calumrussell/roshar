//! Unified WebSocket (`wss://uws.kraken.com/v2/public`) messages for Derivatives market data.
//!
//! Requests use the v2 `method` / `params` format. Every market request carries
//! `asset_class: "derivatives"`, because this crate models Kraken Futures.

use serde::{Deserialize, Serialize};

const ASSET_CLASS: &str = "derivatives";

/// For fields that are always sent but may be `null`: a missing key is an error.
fn nullable<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}

/// Book depths the `book` channel accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KrakenBookDepth {
    D10,
    D25,
    D100,
    D500,
    D1000,
}

impl KrakenBookDepth {
    pub fn as_u32(&self) -> u32 {
        match self {
            Self::D10 => 10,
            Self::D25 => 25,
            Self::D100 => 100,
            Self::D500 => 500,
            Self::D1000 => 1000,
        }
    }
}

/// Candle intervals the `ohlc` channel accepts, in minutes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KrakenOhlcInterval {
    M1,
    M5,
    M15,
    M30,
    H1,
    H4,
    D1,
    W1,
    D15,
}

impl KrakenOhlcInterval {
    pub fn as_minutes(&self) -> u32 {
        match self {
            Self::M1 => 1,
            Self::M5 => 5,
            Self::M15 => 15,
            Self::M30 => 30,
            Self::H1 => 60,
            Self::H4 => 240,
            Self::D1 => 1440,
            Self::W1 => 10080,
            Self::D15 => 21600,
        }
    }
}

/// Candle price series of the `ohlc` channel. Kraken defaults to `trade`. The documented `spot`
/// is rejected live (`tick_type 'index' is not supported`, 2026-11-16), so it is not offered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KrakenOhlcTickType {
    Trade,
    Mark,
}

impl KrakenOhlcTickType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Trade => "trade",
            Self::Mark => "mark",
        }
    }
}

/// One price level. `qty` 0 in an update removes the level.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KrakenBookLevel {
    pub price: f64,
    pub qty: f64,
}

/// One entry of `data` in a `book` message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KrakenBookData {
    pub symbol: String,
    pub bids: Vec<KrakenBookLevel>,
    pub asks: Vec<KrakenBookLevel>,
    /// CRC32 of the top 10 bids and asks.
    pub checksum: u32,
    /// RFC 3339.
    pub timestamp: String,
}

/// `book` channel message. `kind` is `snapshot` or `update`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KrakenBookMessage {
    pub channel: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub data: Vec<KrakenBookData>,
}

/// One trade of a `trade` message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KrakenTrade {
    pub symbol: String,
    /// Side of the taker order.
    pub side: String,
    pub price: f64,
    pub qty: f64,
    /// Order type of the taker order: `limit` or `market`.
    pub ord_type: String,
    /// Sequence number, unique per book.
    pub trade_id: u64,
    /// RFC 3339.
    pub timestamp: String,
}

/// `trade` channel message. `kind` is `snapshot` or `update`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KrakenTradeMessage {
    pub channel: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub data: Vec<KrakenTrade>,
}

/// One market of a `ticker` message. Derivatives tickers carry more than the documented
/// (spot-oriented) schema; required fields are those sent for every market live (2026-11-16).
/// Top of book, last trade and 24h stats are absent for untraded markets (e.g. options);
/// funding fields are sent for perpetuals only.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KrakenTicker {
    pub symbol: String,
    /// RFC 3339.
    pub timestamp: String,
    pub mark: f64,
    pub index: f64,
    pub premium: f64,
    /// 24h volume in base units.
    pub volume: f64,
    pub volume_quote: f64,
    pub open_interest: f64,
    /// 24h trade count.
    pub trades: u64,
    pub suspended: bool,
    pub bid: Option<f64>,
    pub bid_qty: Option<f64>,
    pub ask: Option<f64>,
    pub ask_qty: Option<f64>,
    pub last: Option<f64>,
    pub open: Option<f64>,
    pub high: Option<f64>,
    pub low: Option<f64>,
    pub vwap: Option<f64>,
    /// 24h change in quote currency.
    pub change: Option<f64>,
    /// 24h change in percent.
    pub change_pct: Option<f64>,
    pub funding_rate: Option<f64>,
    pub funding_rate_prediction: Option<f64>,
    pub relative_funding_rate: Option<f64>,
    pub relative_funding_rate_prediction: Option<f64>,
    /// RFC 3339.
    pub next_funding_rate_time: Option<String>,
}

/// `ticker` channel message. `kind` is `snapshot` or `update`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KrakenTickerMessage {
    pub channel: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub data: Vec<KrakenTicker>,
}

/// One candle of an `ohlc` message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KrakenOhlc {
    pub symbol: String,
    /// `trade` or `mark`. Sent live, not in the docs.
    pub tick_type: String,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub vwap: f64,
    pub trades: u64,
    pub volume: f64,
    /// RFC 3339 start of the interval.
    pub interval_begin: String,
    /// Minutes.
    pub interval: u32,
    /// Deprecated by Kraken (end of the interval); still sent.
    pub timestamp: String,
}

/// `ohlc` channel message. `kind` is `snapshot` or `update`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KrakenOhlcMessage {
    pub channel: String,
    #[serde(rename = "type")]
    pub kind: String,
    /// RFC 3339.
    pub timestamp: String,
    pub data: Vec<KrakenOhlc>,
}

/// One asset of an `instrument` message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KrakenWsAsset {
    pub id: String,
    pub status: String,
    pub precision: u32,
    pub precision_display: u32,
    pub borrowable: bool,
    pub collateral_value: f64,
    pub margin_rate: f64,
    /// Documented; not sent for derivatives.
    pub multiplier: Option<f64>,
    /// Documented; not sent for derivatives.
    pub class: Option<String>,
}

/// One market of an `instrument` message. The `Option` fields are documented (mostly
/// spot/margin) but not sent for derivatives.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KrakenWsPair {
    pub symbol: String,
    pub base: String,
    pub quote: String,
    pub status: String,
    pub price_precision: u32,
    pub qty_precision: u32,
    pub qty_min: f64,
    pub qty_increment: f64,
    pub price_increment: f64,
    pub cost_min: Option<f64>,
    pub cost_precision: Option<u32>,
    pub has_index: Option<bool>,
    pub margin_initial: Option<f64>,
    pub marginable: Option<bool>,
    pub position_limit_long: Option<i64>,
    pub position_limit_short: Option<i64>,
    pub ws_display_price_precision: Option<u32>,
    /// Deprecated: use `price_increment`.
    pub tick_size: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KrakenInstrumentData {
    pub assets: Vec<KrakenWsAsset>,
    pub pairs: Vec<KrakenWsPair>,
}

/// `instrument` channel message. `kind` is `snapshot` or `update`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KrakenInstrumentMessage {
    pub channel: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub data: KrakenInstrumentData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KrakenRfqLeg {
    pub symbol: String,
    pub size: f64,
}

/// One open (or just closed) options RFQ.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KrakenRfq {
    pub rfq_id: String,
    pub legs: Vec<KrakenRfqLeg>,
    /// RFC 3339.
    pub expiry: String,
    /// `open`, `expired`, `cancelled`, `filled_bid_side` or `filled_ask_side`.
    pub status: String,
}

/// `rfqs` channel message. `error` is `snapshot_too_large` when the snapshot was dropped.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KrakenRfqsMessage {
    pub channel: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub data: Vec<KrakenRfq>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KrakenUpcomingMaintenance {
    pub event_id: u64,
    pub title: String,
    pub expected_start_utc: String,
    #[serde(deserialize_with = "nullable")]
    pub expected_end_utc: Option<String>,
    pub time_to_start_s: i64,
    pub phase: String,
    pub affected_services: Vec<String>,
    pub order_submission: String,
    pub recommended_action: String,
    #[serde(deserialize_with = "nullable")]
    pub cancel_before_utc: Option<String>,
    pub source_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KrakenEmergencyStep {
    pub applies_to: Vec<String>,
    #[serde(rename = "type")]
    pub kind: String,
    pub expected_at_utc: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KrakenEmergency {
    pub event_id: u64,
    pub title: String,
    pub incident_status: String,
    pub impact: String,
    pub affected_services: Vec<String>,
    pub started_at_utc: String,
    pub next_steps: Vec<KrakenEmergencyStep>,
    pub source_url: String,
}

/// One entry of a `status` message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KrakenStatus {
    pub api_version: String,
    pub version: String,
    /// Sent on the first status message of a connection only.
    pub connection_id: Option<u64>,
    /// Documented engine status; not sent live (2026-11-16), which sends the two below.
    pub system: Option<String>,
    /// `online`, `cancel_only`, `maintenance` or `post_only`. Sent live, not documented.
    pub futures_exchange: Option<String>,
    /// Sent live, not documented.
    pub spot_exchange: Option<String>,
    /// Present together with `emergency` where Kraken enables it.
    pub upcoming_maintenance: Option<Vec<KrakenUpcomingMaintenance>>,
    pub emergency: Option<Vec<KrakenEmergency>>,
}

/// `status` channel message, sent on connect and on engine status changes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KrakenStatusMessage {
    pub channel: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub data: Vec<KrakenStatus>,
}

/// `result` of a subscribe/unsubscribe acknowledgement; fields echo the request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KrakenWssAckResult {
    pub channel: String,
    /// One ack per symbol; `*` for `instrument`; absent for `rfqs`.
    pub symbol: Option<String>,
    pub snapshot: Option<bool>,
    pub depth: Option<u32>,
    pub interval: Option<u32>,
    pub tick_type: Option<String>,
    pub asset_class: Option<String>,
    pub warnings: Option<Vec<String>>,
}

/// Reply to a `subscribe`/`unsubscribe` (or unknown) request. When `success` is false, `error`
/// carries Kraken's message and `result` may be absent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KrakenWssAck {
    pub method: String,
    pub success: bool,
    pub error: Option<String>,
    pub result: Option<KrakenWssAckResult>,
    /// Sent on some failures instead of `result`.
    pub symbol: Option<String>,
    pub req_id: Option<u64>,
    pub time_in: String,
    pub time_out: String,
}

/// Reply to an application-level `ping`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KrakenWssPong {
    pub method: String,
    pub req_id: Option<u64>,
    pub time_in: String,
    pub time_out: String,
}

/// Any message from the public endpoint.
#[derive(Debug, Clone)]
pub enum KrakenWssEvent {
    Book(KrakenBookMessage),
    Trade(KrakenTradeMessage),
    Ticker(KrakenTickerMessage),
    Ohlc(KrakenOhlcMessage),
    Instrument(KrakenInstrumentMessage),
    Rfqs(KrakenRfqsMessage),
    Status(KrakenStatusMessage),
    Heartbeat,
    Pong(KrakenWssPong),
    Ack(KrakenWssAck),
}

impl KrakenWssEvent {
    /// Parses one text frame. Unknown channels and malformed messages are errors.
    pub fn parse(text: &str) -> Result<Self, String> {
        let value: serde_json::Value = serde_json::from_str(text)
            .map_err(|e| format!("Kraken message is not JSON ({e}): {text}"))?;
        fn de<T: serde::de::DeserializeOwned>(
            value: serde_json::Value,
            text: &str,
        ) -> Result<T, String> {
            serde_json::from_value(value)
                .map_err(|e| format!("Failed to parse Kraken message ({e}): {text}"))
        }
        if let Some(method) = value.get("method").and_then(|m| m.as_str()) {
            return if method == "pong" {
                de(value, text).map(Self::Pong)
            } else {
                de(value, text).map(Self::Ack)
            };
        }
        let channel = value
            .get("channel")
            .and_then(|c| c.as_str())
            .ok_or_else(|| format!("Kraken message has no channel or method: {text}"))?
            .to_string();
        match channel.as_str() {
            "book" => de(value, text).map(Self::Book),
            "trade" => de(value, text).map(Self::Trade),
            "ticker" => de(value, text).map(Self::Ticker),
            "ohlc" => de(value, text).map(Self::Ohlc),
            "instrument" => de(value, text).map(Self::Instrument),
            "rfqs" => de(value, text).map(Self::Rfqs),
            "status" => de(value, text).map(Self::Status),
            "heartbeat" => Ok(Self::Heartbeat),
            other => Err(format!("Unknown Kraken channel {other}: {text}")),
        }
    }
}

/// `params` of a subscribe or unsubscribe request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KrakenWssParams {
    pub channel: String,
    pub asset_class: String,
    /// Absent for `instrument` and `rfqs`, which cover every market.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depth: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interval: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tick_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<bool>,
}

/// Outgoing request: `ping`, `subscribe` or `unsubscribe`.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct KrakenWssMessage {
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<KrakenWssParams>,
}

impl KrakenWssMessage {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("failed to serialize KrakenWssMessage")
    }

    pub fn ping() -> Self {
        Self {
            method: "ping".to_string(),
            params: None,
        }
    }

    fn request(method: &str, params: KrakenWssParams) -> Self {
        Self {
            method: method.to_string(),
            params: Some(params),
        }
    }

    fn params(channel: &str, symbols: Option<&[&str]>) -> KrakenWssParams {
        KrakenWssParams {
            channel: channel.to_string(),
            asset_class: ASSET_CLASS.to_string(),
            symbol: symbols.map(|s| s.iter().map(|s| s.to_string()).collect()),
            depth: None,
            interval: None,
            tick_type: None,
            snapshot: None,
        }
    }

    /// `book` subscription.
    pub fn depth(symbols: &[&str], depth: KrakenBookDepth, snapshot: bool) -> Self {
        Self::request(
            "subscribe",
            KrakenWssParams {
                depth: Some(depth.as_u32()),
                snapshot: Some(snapshot),
                ..Self::params("book", Some(symbols))
            },
        )
    }

    /// `book` unsubscription. `depth` must match the subscription.
    pub fn depth_unsub(symbols: &[&str], depth: KrakenBookDepth) -> Self {
        Self::request(
            "unsubscribe",
            KrakenWssParams {
                depth: Some(depth.as_u32()),
                ..Self::params("book", Some(symbols))
            },
        )
    }

    /// `trade` subscription. A snapshot holds the last 50 trades.
    pub fn trades(symbols: &[&str], snapshot: bool) -> Self {
        Self::request(
            "subscribe",
            KrakenWssParams {
                snapshot: Some(snapshot),
                ..Self::params("trade", Some(symbols))
            },
        )
    }

    pub fn trades_unsub(symbols: &[&str]) -> Self {
        Self::request("unsubscribe", Self::params("trade", Some(symbols)))
    }

    /// `ticker` subscription.
    pub fn ticker(symbols: &[&str], snapshot: bool) -> Self {
        Self::request(
            "subscribe",
            KrakenWssParams {
                snapshot: Some(snapshot),
                ..Self::params("ticker", Some(symbols))
            },
        )
    }

    pub fn ticker_unsub(symbols: &[&str]) -> Self {
        Self::request("unsubscribe", Self::params("ticker", Some(symbols)))
    }

    /// `ohlc` subscription. `tick_type: None` leaves Kraken's default (`trade`).
    pub fn ohlc(
        symbols: &[&str],
        interval: KrakenOhlcInterval,
        tick_type: Option<KrakenOhlcTickType>,
        snapshot: bool,
    ) -> Self {
        Self::request(
            "subscribe",
            KrakenWssParams {
                interval: Some(interval.as_minutes()),
                tick_type: tick_type.map(|t| t.as_str().to_string()),
                snapshot: Some(snapshot),
                ..Self::params("ohlc", Some(symbols))
            },
        )
    }

    /// `ohlc` unsubscription. `interval` and `tick_type` must match the subscription (a `mark`
    /// subscription is not removed without `tick_type: mark`).
    pub fn ohlc_unsub(
        symbols: &[&str],
        interval: KrakenOhlcInterval,
        tick_type: Option<KrakenOhlcTickType>,
    ) -> Self {
        Self::request(
            "unsubscribe",
            KrakenWssParams {
                interval: Some(interval.as_minutes()),
                tick_type: tick_type.map(|t| t.as_str().to_string()),
                ..Self::params("ohlc", Some(symbols))
            },
        )
    }

    /// `instrument` subscription (every derivatives market and asset).
    pub fn instruments(snapshot: bool) -> Self {
        Self::request(
            "subscribe",
            KrakenWssParams {
                snapshot: Some(snapshot),
                ..Self::params("instrument", None)
            },
        )
    }

    pub fn instruments_unsub() -> Self {
        Self::request("unsubscribe", Self::params("instrument", None))
    }

    /// `rfqs` subscription (every open options RFQ).
    pub fn rfqs(snapshot: bool) -> Self {
        Self::request(
            "subscribe",
            KrakenWssParams {
                snapshot: Some(snapshot),
                ..Self::params("rfqs", None)
            },
        )
    }

    pub fn rfqs_unsub() -> Self {
        Self::request("unsubscribe", Self::params("rfqs", None))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frames(name: &str) -> Vec<String> {
        std::fs::read_to_string(format!(
            "{}/tests/fixtures/{name}",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap()
        .lines()
        .map(str::to_string)
        .collect()
    }

    #[test]
    fn ping_message() {
        assert_eq!(KrakenWssMessage::ping().to_json(), r#"{"method":"ping"}"#);
    }

    #[test]
    fn book_subscribe_message() {
        let json = KrakenWssMessage::depth(&["PF_XBTUSD", "PF_ETHUSD"], KrakenBookDepth::D25, true)
            .to_json();
        assert_eq!(
            json,
            r#"{"method":"subscribe","params":{"channel":"book","asset_class":"derivatives","symbol":["PF_XBTUSD","PF_ETHUSD"],"depth":25,"snapshot":true}}"#
        );
    }

    #[test]
    fn trade_unsubscribe_message() {
        let json = KrakenWssMessage::trades_unsub(&["PF_XBTUSD"]).to_json();
        assert_eq!(
            json,
            r#"{"method":"unsubscribe","params":{"channel":"trade","asset_class":"derivatives","symbol":["PF_XBTUSD"]}}"#
        );
    }

    #[test]
    fn new_channel_messages() {
        assert_eq!(
            KrakenWssMessage::ticker(&["PF_XBTUSD"], false).to_json(),
            r#"{"method":"subscribe","params":{"channel":"ticker","asset_class":"derivatives","symbol":["PF_XBTUSD"],"snapshot":false}}"#
        );
        assert_eq!(
            KrakenWssMessage::ohlc(
                &["PF_XBTUSD"],
                KrakenOhlcInterval::M1,
                Some(KrakenOhlcTickType::Mark),
                true
            )
            .to_json(),
            r#"{"method":"subscribe","params":{"channel":"ohlc","asset_class":"derivatives","symbol":["PF_XBTUSD"],"interval":1,"tick_type":"mark","snapshot":true}}"#
        );
        assert_eq!(
            KrakenWssMessage::ohlc_unsub(&["PF_ETHUSD"], KrakenOhlcInterval::M5, None).to_json(),
            r#"{"method":"unsubscribe","params":{"channel":"ohlc","asset_class":"derivatives","symbol":["PF_ETHUSD"],"interval":5}}"#
        );
        assert_eq!(
            KrakenWssMessage::instruments(true).to_json(),
            r#"{"method":"subscribe","params":{"channel":"instrument","asset_class":"derivatives","snapshot":true}}"#
        );
        assert_eq!(
            KrakenWssMessage::rfqs_unsub().to_json(),
            r#"{"method":"unsubscribe","params":{"channel":"rfqs","asset_class":"derivatives"}}"#
        );
    }

    #[test]
    fn parses_book_update() {
        // Captured from wss://uws.kraken.com/v2/public on 2026-11-15.
        let text = r#"{"channel":"book","type":"update","data":[{"symbol":"PF_XBTUSD","checksum":2585451826,"timestamp":"2026-11-15T21:51:03.285388329Z","bids":[],"asks":[{"price":65133,"qty":0.4976}]}]}"#;
        let m: KrakenBookMessage = serde_json::from_str(text).unwrap();
        assert_eq!(m.kind, "update");
        assert_eq!(m.data[0].checksum, 2585451826);
        assert_eq!(m.data[0].asks[0].price, 65133.0);
    }

    #[test]
    fn parses_trade_update() {
        // Captured from wss://uws.kraken.com/v2/public on 2026-11-15.
        let text = r#"{"channel":"trade","type":"update","data":[{"symbol":"PF_XBTUSD","side":"sell","price":65123,"qty":0.0076,"ord_type":"limit","trade_id":109865009,"timestamp":"2026-11-15T21:51:03.335337149Z"}]}"#;
        let m: KrakenTradeMessage = serde_json::from_str(text).unwrap();
        assert_eq!(m.data[0].trade_id, 109865009);
        assert_eq!(m.data[0].ord_type, "limit");
        assert!(matches!(
            KrakenWssEvent::parse(text).unwrap(),
            KrakenWssEvent::Trade(_)
        ));
    }

    #[test]
    fn parses_every_live_frame() {
        // Captured from wss://uws.kraken.com/v2/public on 2026-11-16.
        for line in frames("ws_frames.jsonl") {
            KrakenWssEvent::parse(&line).unwrap_or_else(|e| panic!("{e}"));
        }
        let lines = frames("ws_frames.jsonl");
        let KrakenWssEvent::Status(s) = KrakenWssEvent::parse(&lines[0]).unwrap() else {
            panic!("status");
        };
        assert_eq!(s.data[0].connection_id, Some(1794827321933960924));
        assert_eq!(s.data[0].futures_exchange.as_deref(), Some("online"));
        assert!(!s.data[0].upcoming_maintenance.as_ref().unwrap().is_empty());
        assert!(matches!(
            KrakenWssEvent::parse(&lines[2]).unwrap(),
            KrakenWssEvent::Pong(KrakenWssPong {
                req_id: Some(7),
                ..
            })
        ));
        let KrakenWssEvent::Ack(a) = KrakenWssEvent::parse(&lines[6]).unwrap() else {
            panic!("ack");
        };
        assert!(a.success);
        assert_eq!(a.result.unwrap().tick_type.as_deref(), Some("mark"));
        let KrakenWssEvent::Ack(a) = KrakenWssEvent::parse(&lines[11]).unwrap() else {
            panic!("error ack");
        };
        assert!(!a.success);
        assert!(a.error.unwrap().contains("NOPE_X"));
        assert_eq!(a.symbol.as_deref(), Some("NOPE_X"));
        let KrakenWssEvent::Ticker(t) = KrakenWssEvent::parse(&lines[19]).unwrap() else {
            panic!("ticker");
        };
        assert!(t.data[0].funding_rate.is_some());
        let KrakenWssEvent::Ticker(t) = KrakenWssEvent::parse(&lines[20]).unwrap() else {
            panic!("option ticker");
        };
        assert!(t.data[0].bid.is_none());
        let KrakenWssEvent::Ohlc(o) = KrakenWssEvent::parse(&lines[23]).unwrap() else {
            panic!("ohlc");
        };
        assert_eq!(o.data[0].tick_type, "mark");
        assert!(matches!(
            KrakenWssEvent::parse(&lines[27]).unwrap(),
            KrakenWssEvent::Heartbeat
        ));
    }

    #[test]
    fn parses_instrument_snapshot() {
        // Captured 2026-11-16, trimmed to a few assets and pairs.
        let text = frames("ws_instrument_snapshot.json").join("\n");
        let KrakenWssEvent::Instrument(m) = KrakenWssEvent::parse(&text).unwrap() else {
            panic!("instrument");
        };
        assert_eq!(m.kind, "snapshot");
        assert!(m.data.pairs.iter().any(|p| p.symbol == "PF_XBTUSD"));
        assert_eq!(m.data.assets.len(), 3);
    }

    #[test]
    fn parses_documented_rfqs_and_status() {
        // Examples from the rfqs and status channel docs (no live RFQs or incidents on
        // 2026-11-16).
        let lines = frames("ws_docs_examples.jsonl");
        let KrakenWssEvent::Rfqs(r) = KrakenWssEvent::parse(&lines[0]).unwrap() else {
            panic!("rfqs");
        };
        assert_eq!(r.data[0].legs.len(), 2);
        let KrakenWssEvent::Rfqs(r) = KrakenWssEvent::parse(&lines[1]).unwrap() else {
            panic!("rfqs error");
        };
        assert_eq!(r.error.as_deref(), Some("snapshot_too_large"));
        let KrakenWssEvent::Status(s) = KrakenWssEvent::parse(&lines[2]).unwrap() else {
            panic!("status");
        };
        assert_eq!(s.data[0].system.as_deref(), Some("online"));
        assert_eq!(s.data[0].emergency.as_ref().unwrap()[0].next_steps.len(), 1);
    }

    #[test]
    fn missing_required_fields_are_errors() {
        let lines = frames("ws_frames.jsonl");
        for (i, field) in [
            (19, "\"mark\":"),
            (23, "\"interval_begin\":"),
            (2, "\"time_in\":"),
            (3, "\"success\":"),
            (0, "\"api_version\":"),
        ] {
            let broken = lines[i].replacen(field, "\"x\":", 1);
            assert!(KrakenWssEvent::parse(&broken).is_err(), "{field}");
        }
        let text =
            frames("ws_instrument_snapshot.json")
                .join("\n")
                .replacen("\"qty_min\":", "\"x\":", 1);
        assert!(KrakenWssEvent::parse(&text).is_err());
        let text = frames("ws_docs_examples.jsonl")[0].replacen("\"expiry\":", "\"x\":", 1);
        assert!(KrakenWssEvent::parse(&text).is_err());
        assert!(KrakenWssEvent::parse(r#"{"channel":"bogus"}"#).is_err());
    }
}
