use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};

/// Request ids must be unique: the server echoes the id in its `ack`/`pong`.
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

fn next_id() -> String {
    NEXT_ID.fetch_add(1, Ordering::Relaxed).to_string()
}

/// Public Classic Futures topics. Every topic takes a comma-separated symbol list (the
/// exchange accepts up to 100 per request); the builders join `symbols` with `,` and send
/// one request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KuCoinChannel {
    /// `/contractMarket/tickerV2:{symbols}`, BBO, real-time.
    TickerV2,
    /// `/contractMarket/ticker:{symbols}`, last trade + BBO. Documented as no longer
    /// recommended in favour of `TickerV2`.
    Ticker,
    /// `/contractMarket/level2:{symbols}`, order-book increments.
    Level2,
    /// `/contractMarket/level2Depth5:{symbols}`, top-5 snapshot every 100 ms.
    Level2Depth5,
    /// `/contractMarket/level2Depth50:{symbols}`, top-50 snapshot every 100 ms.
    Level2Depth50,
    /// `/contractMarket/execution:{symbols}`, trades.
    Execution,
    /// `/contract/instrument:{symbols}`, mark/index price (1 s) and funding rate (1 min).
    Instrument,
    /// `/contractMarket/snapshot:{symbols}`, 24 h statistics every 5 s.
    Snapshot,
}

impl KuCoinChannel {
    pub fn prefix(&self) -> &'static str {
        match self {
            Self::TickerV2 => "/contractMarket/tickerV2",
            Self::Ticker => "/contractMarket/ticker",
            Self::Level2 => "/contractMarket/level2",
            Self::Level2Depth5 => "/contractMarket/level2Depth5",
            Self::Level2Depth50 => "/contractMarket/level2Depth50",
            Self::Execution => "/contractMarket/execution",
            Self::Instrument => "/contract/instrument",
            Self::Snapshot => "/contractMarket/snapshot",
        }
    }

    pub fn topic(&self, symbols: &[&str]) -> String {
        format!("{}:{}", self.prefix(), symbols.join(","))
    }
}

/// `/contractMarket/limitCandle:{symbol}_{type},...`. `kline_type` is the exchange value:
/// `1min, 3min, 5min, 15min, 30min, 1hour, 2hour, 4hour, 8hour, 12hour, 1day, 1week, 1month`.
pub fn kucoin_candle_topic(pairs: &[(&str, &str)]) -> String {
    let list: Vec<String> = pairs
        .iter()
        .map(|(symbol, kline_type)| format!("{symbol}_{kline_type}"))
        .collect();
    format!("/contractMarket/limitCandle:{}", list.join(","))
}

/// `/contract/announcement` (funding settlement). `None` subscribes the bare topic, which
/// the server acknowledges and which pushes every symbol; `Some(symbols)` adds the symbol
/// list. Both forms are documented.
pub fn kucoin_announcement_topic(symbols: Option<&[&str]>) -> String {
    match symbols {
        None => "/contract/announcement".to_string(),
        Some(symbols) => format!("/contract/announcement:{}", symbols.join(",")),
    }
}

/// Client-to-server message on the Classic Futures WebSocket.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KuCoinWssMessage {
    pub id: String,
    #[serde(rename = "type")]
    pub msg_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topic: Option<String>,
    #[serde(rename = "privateChannel", skip_serializing_if = "Option::is_none")]
    pub private_channel: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response: Option<bool>,
}

impl KuCoinWssMessage {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("failed to serialize KuCoinWssMessage")
    }

    pub fn ping() -> Self {
        Self {
            id: next_id(),
            msg_type: "ping".to_string(),
            topic: None,
            private_channel: None,
            response: None,
        }
    }

    fn public(msg_type: &str, topic: String) -> Self {
        Self {
            id: next_id(),
            msg_type: msg_type.to_string(),
            topic: Some(topic),
            private_channel: Some(false),
            response: Some(true),
        }
    }

    /// Subscribe to any public topic string (see `KuCoinChannel::topic`,
    /// `kucoin_candle_topic`, `kucoin_announcement_topic`).
    pub fn subscribe(topic: String) -> Self {
        Self::public("subscribe", topic)
    }

    pub fn unsubscribe(topic: String) -> Self {
        Self::public("unsubscribe", topic)
    }
}

// ---------------------------------------------------------------------------
// Server-to-client messages
// ---------------------------------------------------------------------------

/// `/contractMarket/level2` data. `change` is `"price,side,size"`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KuCoinWsLevel2Change {
    pub sequence: i64,
    pub change: String,
    /// Milliseconds.
    pub timestamp: i64,
}

impl KuCoinWsLevel2Change {
    /// Split `change` into `(price, side, size)`. Size `0` removes the level.
    pub fn parts(&self) -> Result<(f64, &str, i64), String> {
        let mut it = self.change.split(',');
        let (Some(p), Some(side), Some(s), None) = (it.next(), it.next(), it.next(), it.next())
        else {
            return Err(format!("malformed level2 change {:?}", self.change));
        };
        let price = p
            .parse()
            .map_err(|e| format!("bad level2 price {p:?}: {e}"))?;
        let size = s
            .parse()
            .map_err(|e| format!("bad level2 size {s:?}: {e}"))?;
        Ok((price, side, size))
    }
}

/// `/contractMarket/level2Depth5` and `level2Depth50` data. Levels are
/// `[price (string), size in lots]`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KuCoinWsDepth {
    pub sequence: i64,
    pub asks: Vec<(String, i64)>,
    pub bids: Vec<(String, i64)>,
    /// Milliseconds.
    pub timestamp: i64,
    /// Milliseconds.
    pub ts: i64,
}

/// `/contractMarket/tickerV2` data.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KuCoinWsTickerV2 {
    pub symbol: String,
    pub sequence: i64,
    pub best_bid_size: i64,
    pub best_bid_price: String,
    pub best_ask_price: String,
    pub best_ask_size: i64,
    /// Nanoseconds.
    pub ts: i64,
}

/// `/contractMarket/ticker` (V1) data.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KuCoinWsTicker {
    pub symbol: String,
    pub sequence: i64,
    pub side: String,
    pub size: i64,
    pub price: String,
    pub best_bid_size: i64,
    pub best_bid_price: String,
    pub best_ask_price: String,
    pub trade_id: String,
    pub best_ask_size: i64,
    /// Nanoseconds.
    pub ts: i64,
}

/// `/contractMarket/execution` data (subject `match`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KuCoinWsTrade {
    pub symbol: String,
    pub sequence: i64,
    /// Taker side.
    pub side: String,
    pub size: i64,
    pub price: String,
    pub taker_order_id: String,
    pub maker_order_id: String,
    pub trade_id: String,
    /// Nanoseconds.
    pub ts: i64,
}

/// `/contract/instrument` subject `mark.index.price`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KuCoinWsMarkIndexPrice {
    pub mark_price: f64,
    pub index_price: f64,
    pub granularity: i64,
    /// Milliseconds.
    pub timestamp: i64,
}

/// `/contract/instrument` subject `funding.rate`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KuCoinWsFundingRate {
    /// Sent live; not in the documented example.
    pub period: Option<i64>,
    pub granularity: i64,
    pub funding_rate: f64,
    /// Milliseconds.
    pub timestamp: i64,
}

/// `/contract/announcement` subjects `funding.begin` / `funding.end`. Built from the
/// documented example: settlement pushes only arrive at funding time and were not captured.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KuCoinWsFundingSettlement {
    pub symbol: String,
    /// Milliseconds.
    pub funding_time: i64,
    pub funding_rate: f64,
    /// Milliseconds.
    pub timestamp: i64,
}

/// `/contractMarket/snapshot` subject `snapshot.24h`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KuCoinWsSnapshot {
    /// Sent live; not in the documented example (delivery contracts have no funding).
    pub funding_rate: Option<f64>,
    pub high_price: f64,
    pub last_price: f64,
    pub low_price: f64,
    #[serde(rename = "price24HoursBefore")]
    pub price_24_hours_before: f64,
    pub price_chg: f64,
    pub price_chg_pct: f64,
    pub symbol: String,
    /// Nanoseconds.
    pub ts: i64,
    pub turnover: f64,
    pub volume: f64,
}

/// `/contractMarket/limitCandle` subject `candle.stick`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KuCoinWsCandle {
    pub symbol: String,
    /// `[start time (seconds), open, close, high, low, volume, turnover]` as strings.
    /// The exchange documents the volume element as incorrect.
    pub candles: [String; 7],
    /// Milliseconds.
    pub time: i64,
}

/// A `type: "message"` push: its topic (with symbols/kline type) and typed data.
#[derive(Debug, Clone)]
pub struct KuCoinWsPush<T> {
    pub topic: String,
    /// `sn`, sent on level2, depth, ticker and execution pushes.
    pub sn: Option<i64>,
    pub data: T,
}

/// Every server-to-client frame on the public Classic Futures WebSocket.
#[derive(Debug, Clone)]
pub enum KuCoinWsEvent {
    Welcome {
        id: String,
    },
    Ack {
        id: String,
    },
    /// `timestamp` is microseconds.
    Pong {
        id: String,
        timestamp: i64,
    },
    /// Subscribe/request error, e.g. `code 404, data "topic does not exist"`.
    Error {
        id: String,
        code: i64,
        data: String,
    },
    Level2(KuCoinWsPush<KuCoinWsLevel2Change>),
    Level2Depth5(KuCoinWsPush<KuCoinWsDepth>),
    Level2Depth50(KuCoinWsPush<KuCoinWsDepth>),
    TickerV2(KuCoinWsPush<KuCoinWsTickerV2>),
    Ticker(KuCoinWsPush<KuCoinWsTicker>),
    Trade(KuCoinWsPush<KuCoinWsTrade>),
    MarkIndexPrice(KuCoinWsPush<KuCoinWsMarkIndexPrice>),
    FundingRate(KuCoinWsPush<KuCoinWsFundingRate>),
    FundingBegin(KuCoinWsPush<KuCoinWsFundingSettlement>),
    FundingEnd(KuCoinWsPush<KuCoinWsFundingSettlement>),
    Snapshot(KuCoinWsPush<KuCoinWsSnapshot>),
    Candle(KuCoinWsPush<KuCoinWsCandle>),
}

#[derive(Deserialize)]
struct RawFrame {
    #[serde(rename = "type")]
    msg_type: String,
    id: Option<String>,
    topic: Option<String>,
    subject: Option<String>,
    sn: Option<i64>,
    code: Option<i64>,
    timestamp: Option<i64>,
    data: Option<serde_json::Value>,
}

fn req<T>(v: Option<T>, field: &str, kind: &str) -> Result<T, String> {
    v.ok_or_else(|| format!("KuCoin ws {kind} frame missing `{field}`"))
}

impl KuCoinWsEvent {
    /// Parse one text frame. Unknown `type`, topic or subject, or a missing/mistyped field,
    /// is an error.
    pub fn parse(text: &str) -> Result<Self, String> {
        let raw: RawFrame =
            serde_json::from_str(text).map_err(|e| format!("KuCoin ws frame: {e}"))?;
        match raw.msg_type.as_str() {
            "welcome" => Ok(Self::Welcome {
                id: req(raw.id, "id", "welcome")?,
            }),
            "ack" => Ok(Self::Ack {
                id: req(raw.id, "id", "ack")?,
            }),
            "pong" => Ok(Self::Pong {
                id: req(raw.id, "id", "pong")?,
                timestamp: req(raw.timestamp, "timestamp", "pong")?,
            }),
            "error" => {
                let data = req(raw.data, "data", "error")?;
                Ok(Self::Error {
                    id: req(raw.id, "id", "error")?,
                    code: req(raw.code, "code", "error")?,
                    data: match data {
                        serde_json::Value::String(s) => s,
                        other => other.to_string(),
                    },
                })
            }
            "message" => Self::parse_push(raw),
            other => Err(format!("KuCoin ws unknown frame type {other:?}")),
        }
    }

    fn parse_push(raw: RawFrame) -> Result<Self, String> {
        let topic = req(raw.topic, "topic", "message")?;
        let subject = req(raw.subject, "subject", "message")?;
        let data = req(raw.data, "data", "message")?;
        let sn = raw.sn;
        let prefix = topic.split(':').next().unwrap_or_default().to_string();

        fn push<T: DeserializeOwned>(
            topic: String,
            sn: Option<i64>,
            data: serde_json::Value,
        ) -> Result<KuCoinWsPush<T>, String> {
            let data =
                serde_json::from_value(data).map_err(|e| format!("KuCoin ws {topic} data: {e}"))?;
            Ok(KuCoinWsPush { topic, sn, data })
        }

        match (prefix.as_str(), subject.as_str()) {
            ("/contractMarket/level2", "level2") => Ok(Self::Level2(push(topic, sn, data)?)),
            ("/contractMarket/level2Depth5", "level2") => {
                Ok(Self::Level2Depth5(push(topic, sn, data)?))
            }
            ("/contractMarket/level2Depth50", "level2") => {
                Ok(Self::Level2Depth50(push(topic, sn, data)?))
            }
            ("/contractMarket/tickerV2", "tickerV2") => Ok(Self::TickerV2(push(topic, sn, data)?)),
            ("/contractMarket/ticker", "ticker") => Ok(Self::Ticker(push(topic, sn, data)?)),
            ("/contractMarket/execution", "match") => Ok(Self::Trade(push(topic, sn, data)?)),
            ("/contract/instrument", "mark.index.price") => {
                Ok(Self::MarkIndexPrice(push(topic, sn, data)?))
            }
            ("/contract/instrument", "funding.rate") => {
                Ok(Self::FundingRate(push(topic, sn, data)?))
            }
            ("/contract/announcement", "funding.begin") => {
                Ok(Self::FundingBegin(push(topic, sn, data)?))
            }
            ("/contract/announcement", "funding.end") => {
                Ok(Self::FundingEnd(push(topic, sn, data)?))
            }
            ("/contractMarket/snapshot", "snapshot.24h") => {
                Ok(Self::Snapshot(push(topic, sn, data)?))
            }
            ("/contractMarket/limitCandle", "candle.stick") => {
                Ok(Self::Candle(push(topic, sn, data)?))
            }
            _ => Err(format!(
                "KuCoin ws unknown push topic {topic:?} subject {subject:?}"
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> String {
        std::fs::read_to_string(format!(
            "{}/tests/fixtures/{name}",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap()
    }

    fn without(name: &str, field: &str) -> String {
        let mut v: serde_json::Value = serde_json::from_str(&fixture(name)).unwrap();
        assert!(v["data"].as_object_mut().unwrap().remove(field).is_some());
        v.to_string()
    }

    #[test]
    fn test_ping_serializes_correctly() {
        let msg = KuCoinWssMessage::ping();
        let json = msg.to_json();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["type"], "ping");
        assert!(!parsed["id"].as_str().unwrap().is_empty());
        assert!(parsed.get("topic").is_none());
    }

    #[test]
    fn test_ids_are_unique() {
        let a = KuCoinWssMessage::ping();
        let b = KuCoinWssMessage::subscribe(KuCoinChannel::Level2.topic(&["XBTUSDTM"]));
        assert_ne!(a.id, b.id);
    }

    #[test]
    fn test_subscribe_multi_symbol() {
        let msg =
            KuCoinWssMessage::subscribe(KuCoinChannel::Execution.topic(&["XBTUSDTM", "ETHUSDTM"]));
        let parsed: serde_json::Value = serde_json::from_str(&msg.to_json()).unwrap();
        assert_eq!(parsed["type"], "subscribe");
        assert_eq!(
            parsed["topic"],
            "/contractMarket/execution:XBTUSDTM,ETHUSDTM"
        );
        assert_eq!(parsed["privateChannel"], false);
        assert_eq!(parsed["response"], true);
        let un = KuCoinWssMessage::unsubscribe(KuCoinChannel::Level2Depth50.topic(&["XBTUSDTM"]));
        assert_eq!(un.msg_type, "unsubscribe");
        assert_eq!(
            un.topic.as_deref(),
            Some("/contractMarket/level2Depth50:XBTUSDTM")
        );
    }

    #[test]
    fn test_candle_and_announcement_topics() {
        assert_eq!(
            kucoin_candle_topic(&[("XBTUSDTM", "1min"), ("ETHUSDTM", "5min")]),
            "/contractMarket/limitCandle:XBTUSDTM_1min,ETHUSDTM_5min"
        );
        assert_eq!(kucoin_announcement_topic(None), "/contract/announcement");
        assert_eq!(
            kucoin_announcement_topic(Some(&["XBTUSDTM"])),
            "/contract/announcement:XBTUSDTM"
        );
    }

    #[test]
    fn control_frames() {
        assert!(matches!(
            KuCoinWsEvent::parse(&fixture("ws_welcome.json")).unwrap(),
            KuCoinWsEvent::Welcome { .. }
        ));
        assert!(matches!(
            KuCoinWsEvent::parse(&fixture("ws_ack.json")).unwrap(),
            KuCoinWsEvent::Ack { id } if id == "100"
        ));
        assert!(matches!(
            KuCoinWsEvent::parse(&fixture("ws_pong.json")).unwrap(),
            KuCoinWsEvent::Pong { .. }
        ));
        match KuCoinWsEvent::parse(&fixture("ws_error.json")).unwrap() {
            KuCoinWsEvent::Error { code, data, .. } => {
                assert_eq!(code, 404);
                assert_eq!(data, "topic does not exist");
            }
            other => panic!("{other:?}"),
        }
        assert!(KuCoinWsEvent::parse(r#"{"type":"pong","id":"1"}"#).is_err());
        assert!(KuCoinWsEvent::parse(r#"{"type":"nope","id":"1"}"#).is_err());
    }

    #[test]
    fn level2() {
        match KuCoinWsEvent::parse(&fixture("ws_level2.json")).unwrap() {
            KuCoinWsEvent::Level2(p) => {
                assert!(p.sn.is_some());
                let (price, side, size) = p.data.parts().unwrap();
                assert!(price > 0.0);
                assert_eq!(side, "buy");
                assert_eq!(size, 8);
            }
            other => panic!("{other:?}"),
        }
        assert!(KuCoinWsEvent::parse(&without("ws_level2.json", "change")).is_err());
    }

    #[test]
    fn depth() {
        match KuCoinWsEvent::parse(&fixture("ws_level2_depth5.json")).unwrap() {
            KuCoinWsEvent::Level2Depth5(p) => assert_eq!(p.data.asks.len(), 5),
            other => panic!("{other:?}"),
        }
        match KuCoinWsEvent::parse(&fixture("ws_level2_depth50.json")).unwrap() {
            KuCoinWsEvent::Level2Depth50(p) => {
                assert!(!p.data.bids.is_empty() && p.data.bids.len() <= 50)
            }
            other => panic!("{other:?}"),
        }
        assert!(KuCoinWsEvent::parse(&without("ws_level2_depth5.json", "bids")).is_err());
    }

    #[test]
    fn tickers_and_trades() {
        assert!(matches!(
            KuCoinWsEvent::parse(&fixture("ws_ticker_v2.json")).unwrap(),
            KuCoinWsEvent::TickerV2(_)
        ));
        assert!(matches!(
            KuCoinWsEvent::parse(&fixture("ws_ticker.json")).unwrap(),
            KuCoinWsEvent::Ticker(_)
        ));
        match KuCoinWsEvent::parse(&fixture("ws_execution.json")).unwrap() {
            KuCoinWsEvent::Trade(p) => assert_eq!(p.topic, "/contractMarket/execution:ETHUSDTM"),
            other => panic!("{other:?}"),
        }
        assert!(KuCoinWsEvent::parse(&without("ws_ticker_v2.json", "bestAskPrice")).is_err());
        assert!(KuCoinWsEvent::parse(&without("ws_execution.json", "tradeId")).is_err());
    }

    #[test]
    fn instrument_snapshot_candle() {
        assert!(matches!(
            KuCoinWsEvent::parse(&fixture("ws_mark_index_price.json")).unwrap(),
            KuCoinWsEvent::MarkIndexPrice(_)
        ));
        match KuCoinWsEvent::parse(&fixture("ws_funding_rate.json")).unwrap() {
            KuCoinWsEvent::FundingRate(p) => assert_eq!(p.data.period, Some(1)),
            other => panic!("{other:?}"),
        }
        match KuCoinWsEvent::parse(&fixture("ws_snapshot.json")).unwrap() {
            KuCoinWsEvent::Snapshot(p) => assert_eq!(p.data.symbol, "XBTUSDTM"),
            other => panic!("{other:?}"),
        }
        match KuCoinWsEvent::parse(&fixture("ws_candle.json")).unwrap() {
            KuCoinWsEvent::Candle(p) => assert_eq!(p.data.candles[0], "1794826800"),
            other => panic!("{other:?}"),
        }
        assert!(KuCoinWsEvent::parse(&without("ws_mark_index_price.json", "markPrice")).is_err());
        assert!(KuCoinWsEvent::parse(&without("ws_snapshot.json", "lastPrice")).is_err());
        assert!(KuCoinWsEvent::parse(&without("ws_candle.json", "candles")).is_err());
    }

    #[test]
    fn funding_settlement_from_docs() {
        let j = r#"{"type":"message","topic":"/contract/announcement","subject":"funding.end",
            "data":{"symbol":"XBTUSDTM","fundingTime":1551770400000,"fundingRate":-0.002966,
            "timestamp":1551770410000}}"#;
        assert!(matches!(
            KuCoinWsEvent::parse(j).unwrap(),
            KuCoinWsEvent::FundingEnd(_)
        ));
    }
}
