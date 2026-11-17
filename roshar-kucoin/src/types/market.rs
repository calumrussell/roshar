//! REST market-data payloads (the `data` of each envelope).

use serde::{Deserialize, Serialize};

/// GET /api/v1/ticker and one entry of GET /api/v1/allTickers.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KuCoinTicker {
    pub sequence: i64,
    pub symbol: String,
    /// Taker side of the last trade: `buy` or `sell`.
    pub side: String,
    /// Last trade size, in lots.
    pub size: i64,
    pub trade_id: String,
    pub price: String,
    pub best_bid_price: String,
    pub best_bid_size: i64,
    pub best_ask_price: String,
    pub best_ask_size: i64,
    /// Nanoseconds.
    pub ts: i64,
}

/// GET /api/v1/level2/snapshot (full) and GET /api/v1/level2/depth{20,100} (partial).
/// Levels are `[price, size in lots]`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KuCoinOrderBook {
    /// Sequence to align `/contractMarket/level2` increments with.
    pub sequence: i64,
    pub symbol: String,
    pub bids: Vec<(f64, i64)>,
    pub asks: Vec<(f64, i64)>,
    /// Nanoseconds.
    pub ts: i64,
}

/// One entry of GET /api/v1/trade/history (the latest 100 trades).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KuCoinTrade {
    pub sequence: i64,
    pub contract_id: i64,
    pub trade_id: String,
    pub maker_order_id: String,
    pub taker_order_id: String,
    /// Nanoseconds.
    pub ts: i64,
    pub size: i64,
    pub price: String,
    /// Taker side: `buy` or `sell`.
    pub side: String,
}

/// One row of GET /api/v1/kline/query:
/// `[time ms, open, high, low, close, volume (lots), turnover]`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct KuCoinKline(
    pub i64,
    pub f64,
    pub f64,
    pub f64,
    pub f64,
    pub f64,
    pub f64,
);

impl KuCoinKline {
    pub fn time(&self) -> i64 {
        self.0
    }
    pub fn open(&self) -> f64 {
        self.1
    }
    pub fn high(&self) -> f64 {
        self.2
    }
    pub fn low(&self) -> f64 {
        self.3
    }
    pub fn close(&self) -> f64 {
        self.4
    }
    pub fn volume(&self) -> f64 {
        self.5
    }
    pub fn turnover(&self) -> f64 {
        self.6
    }
}

/// GET /api/v1/mark-price/{symbol}/current
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KuCoinMarkPrice {
    pub symbol: String,
    /// Milliseconds.
    pub granularity: i64,
    /// Milliseconds.
    pub time_point: i64,
    /// Mark price.
    pub value: f64,
    pub index_price: f64,
}

/// Query parameters shared by GET /api/v1/index/query, /api/v1/interest/query and
/// /api/v1/premium/query. Each `None` field is omitted and the exchange default applies
/// (`reverse=true`, `forward=true`, `maxCount=10`, max 100).
#[derive(Debug, Clone, Default)]
pub struct KuCoinIndexQuery {
    /// Milliseconds.
    pub start_at: Option<i64>,
    /// Milliseconds.
    pub end_at: Option<i64>,
    pub reverse: Option<bool>,
    /// Paging cursor from the previous page.
    pub offset: Option<i64>,
    pub forward: Option<bool>,
    pub max_count: Option<u32>,
}

impl KuCoinIndexQuery {
    pub(crate) fn to_query(&self) -> String {
        let mut q = String::new();
        if let Some(v) = self.start_at {
            q.push_str(&format!("&startAt={v}"));
        }
        if let Some(v) = self.end_at {
            q.push_str(&format!("&endAt={v}"));
        }
        if let Some(v) = self.reverse {
            q.push_str(&format!("&reverse={v}"));
        }
        if let Some(v) = self.offset {
            q.push_str(&format!("&offset={v}"));
        }
        if let Some(v) = self.forward {
            q.push_str(&format!("&forward={v}"));
        }
        if let Some(v) = self.max_count {
            q.push_str(&format!("&maxCount={v}"));
        }
        q
    }
}

/// Page of the index/interest/premium queries.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KuCoinIndexPage<T> {
    pub data_list: Vec<T>,
    pub has_more: bool,
}

/// One entry of GET /api/v1/interest/query and /api/v1/premium/query.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KuCoinIndexValue {
    pub symbol: String,
    /// Milliseconds.
    pub granularity: i64,
    /// Milliseconds.
    pub time_point: i64,
    pub value: f64,
}

/// One component exchange of a spot index.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KuCoinIndexComponent {
    pub exchange: String,
    pub exchange_name: String,
    pub price: f64,
    pub weight: f64,
}

/// One entry of GET /api/v1/index/query (Get Spot Index Price).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KuCoinSpotIndex {
    pub symbol: String,
    /// Milliseconds.
    pub granularity: i64,
    /// Milliseconds.
    pub time_point: i64,
    pub value: f64,
    /// Exchange spelling (`decomposionList`).
    #[serde(rename = "decomposionList")]
    pub decomposition_list: Vec<KuCoinIndexComponent>,
}

/// GET /api/v1/funding-rate/{symbol}/current. Delivery contracts answer error 415000.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KuCoinCurrentFundingRate {
    /// Funding rate index symbol, e.g. `.XBTUSDTMFPI8H`.
    pub symbol: String,
    /// Funding interval, milliseconds.
    pub granularity: i64,
    /// Milliseconds.
    pub time_point: i64,
    /// Current funding rate.
    pub value: f64,
    /// Documented as required, but not sent by the live endpoint.
    pub predicted_value: Option<f64>,
    /// Not documented; sent live.
    pub daily_interest_rate: f64,
    pub funding_rate_cap: f64,
    pub funding_rate_floor: f64,
    /// `0` or `1`, as documented.
    pub period: i64,
    /// Next settlement, milliseconds.
    pub funding_time: i64,
    /// Not documented; sent live. Rate applied at the previous settlement.
    pub last_time_funding_rate: f64,
}

/// GET /api/v1/status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KuCoinServiceStatus {
    /// `open`, `close` or `cancelonly`.
    pub status: String,
    pub msg: String,
}

/// One entry of GET https://api.kucoin.com/api/ua/v2/market/open-interest (real-time mode,
/// no `interval`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KuCoinOpenInterest {
    pub symbol: String,
    /// In lots.
    pub open_interest: String,
    /// Milliseconds.
    pub ts: i64,
}

/// One entry of the same endpoint in interval (history) mode. No `symbol` is sent.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KuCoinOpenInterestPoint {
    /// In lots.
    pub open_interest: String,
    /// Milliseconds.
    pub ts: i64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{KuCoinContract, KuCoinResponse};
    use serde::de::DeserializeOwned;

    fn data<T: DeserializeOwned>(json: &str) -> T {
        let r: KuCoinResponse<T> = serde_json::from_str(json).unwrap();
        assert_eq!(r.code, "200000");
        r.data.unwrap()
    }

    fn strip<T: DeserializeOwned + std::fmt::Debug>(json: &str, field: &str) {
        let mut v: serde_json::Value = serde_json::from_str(json).unwrap();
        let obj = v["data"].as_object_mut().unwrap();
        assert!(obj.remove(field).is_some(), "fixture lacks {field}");
        assert!(serde_json::from_value::<KuCoinResponse<T>>(v).is_err());
    }

    #[test]
    fn contract() {
        let j = include_str!("../../tests/fixtures/contract.json");
        let c: KuCoinContract = data(j);
        assert_eq!(c.symbol, "XBTUSDTM");
        strip::<KuCoinContract>(j, "markPrice");
    }

    #[test]
    fn ticker() {
        let j = include_str!("../../tests/fixtures/ticker.json");
        let t: KuCoinTicker = data(j);
        assert_eq!(t.symbol, "XBTUSDTM");
        strip::<KuCoinTicker>(j, "bestBidPrice");
        let all: Vec<KuCoinTicker> = data(include_str!("../../tests/fixtures/all_tickers.json"));
        assert_eq!(all.len(), 3);
    }

    #[test]
    fn order_books() {
        let j = include_str!("../../tests/fixtures/orderbook_full.json");
        let b: KuCoinOrderBook = data(j);
        assert_eq!(b.bids.len(), 5);
        assert!(b.bids[0].0 < b.asks[0].0);
        strip::<KuCoinOrderBook>(j, "sequence");
        let d: KuCoinOrderBook = data(include_str!("../../tests/fixtures/orderbook_depth20.json"));
        assert_eq!(d.symbol, "XBTUSDTM");
    }

    #[test]
    fn trades() {
        let j = include_str!("../../tests/fixtures/trade_history.json");
        let t: Vec<KuCoinTrade> = data(j);
        assert_eq!(t.len(), 3);
        let bad = j.replace("\"makerOrderId\"", "\"x\"");
        assert!(serde_json::from_str::<KuCoinResponse<Vec<KuCoinTrade>>>(&bad).is_err());
    }

    #[test]
    fn klines() {
        let k: Vec<KuCoinKline> = data(include_str!("../../tests/fixtures/klines.json"));
        assert_eq!(k.len(), 3);
        assert_eq!(k[0].time(), 1794780000000);
        assert!(k[0].high() >= k[0].low());
        assert!(serde_json::from_str::<KuCoinKline>("[1,2.0,3.0,4.0,5.0,6]").is_err());
    }

    #[test]
    fn mark_price() {
        let j = include_str!("../../tests/fixtures/mark_price.json");
        let m: KuCoinMarkPrice = data(j);
        assert_eq!(m.symbol, "XBTUSDTM");
        strip::<KuCoinMarkPrice>(j, "indexPrice");
    }

    #[test]
    fn index_queries() {
        let s: KuCoinIndexPage<KuCoinSpotIndex> =
            data(include_str!("../../tests/fixtures/spot_index.json"));
        assert!(!s.data_list[0].decomposition_list.is_empty());
        let j = include_str!("../../tests/fixtures/interest_rate_index.json");
        let i: KuCoinIndexPage<KuCoinIndexValue> = data(j);
        assert_eq!(i.data_list.len(), 2);
        assert!(i.has_more);
        strip::<KuCoinIndexPage<KuCoinIndexValue>>(j, "hasMore");
        let p: KuCoinIndexPage<KuCoinIndexValue> =
            data(include_str!("../../tests/fixtures/premium_index.json"));
        assert_eq!(p.data_list[0].symbol, ".XBTUSDTMPI8H");
    }

    #[test]
    fn index_query_string() {
        assert_eq!(KuCoinIndexQuery::default().to_query(), "");
        let q = KuCoinIndexQuery {
            start_at: Some(1),
            max_count: Some(5),
            reverse: Some(false),
            ..Default::default()
        };
        assert_eq!(q.to_query(), "&startAt=1&reverse=false&maxCount=5");
    }

    #[test]
    fn current_funding_rate() {
        let j = include_str!("../../tests/fixtures/current_funding_rate.json");
        let f: KuCoinCurrentFundingRate = data(j);
        assert_eq!(f.symbol, ".XBTUSDTMFPI8H");
        assert!(f.predicted_value.is_none());
        strip::<KuCoinCurrentFundingRate>(j, "fundingTime");
    }

    #[test]
    fn server_time_and_status() {
        let t: i64 = data(include_str!("../../tests/fixtures/server_time.json"));
        assert!(t > 0);
        let j = include_str!("../../tests/fixtures/service_status.json");
        let s: KuCoinServiceStatus = data(j);
        assert_eq!(s.status, "open");
        strip::<KuCoinServiceStatus>(j, "status");
    }

    #[test]
    fn open_interest() {
        let j = include_str!("../../tests/fixtures/open_interest.json");
        let o: Vec<KuCoinOpenInterest> = data(j);
        assert_eq!(o.len(), 2);
        let h: Vec<KuCoinOpenInterestPoint> = data(include_str!(
            "../../tests/fixtures/open_interest_history.json"
        ));
        assert_eq!(h.len(), 3);
        // History rows carry no symbol, so they do not parse as real-time rows.
        assert!(
            serde_json::from_str::<KuCoinResponse<Vec<KuCoinOpenInterest>>>(include_str!(
                "../../tests/fixtures/open_interest_history.json"
            ))
            .is_err()
        );
    }

    #[test]
    fn trade_statistics_needs_auth() {
        let r: KuCoinResponse<serde_json::Value> = serde_json::from_str(include_str!(
            "../../tests/fixtures/trade_statistics_error.json"
        ))
        .unwrap();
        assert_eq!(r.code, "400001");
    }
}
