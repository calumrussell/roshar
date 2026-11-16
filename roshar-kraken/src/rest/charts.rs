use roshar_http::RateLimitedClient;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::sync::Arc;

const CHARTS_URL: &str = "https://futures.kraken.com/api/charts/v1";

type ChartsError = Box<dyn std::error::Error + Send + Sync>;

/// Price series of the charts endpoint (`GET /api/charts/v1/` lists them).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KrakenTickType {
    Trade,
    Mark,
    Spot,
}

impl KrakenTickType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Trade => "trade",
            Self::Mark => "mark",
            Self::Spot => "spot",
        }
    }
}

/// Candle resolutions of the charts endpoint (`GET /api/charts/v1/{tick_type}/{symbol}` lists
/// them). The analytics endpoints take the same set as an `interval` in seconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KrakenResolution {
    M1,
    M5,
    M15,
    M30,
    H1,
    H4,
    H12,
    D1,
    W1,
}

impl KrakenResolution {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::M1 => "1m",
            Self::M5 => "5m",
            Self::M15 => "15m",
            Self::M30 => "30m",
            Self::H1 => "1h",
            Self::H4 => "4h",
            Self::H12 => "12h",
            Self::D1 => "1d",
            Self::W1 => "1w",
        }
    }

    pub fn as_secs(&self) -> u32 {
        match self {
            Self::M1 => 60,
            Self::M5 => 300,
            Self::M15 => 900,
            Self::M30 => 1800,
            Self::H1 => 3600,
            Self::H4 => 14400,
            Self::H12 => 43200,
            Self::D1 => 86400,
            Self::W1 => 604800,
        }
    }
}

/// One candle as sent by the charts endpoint. `time` is the open time in milliseconds.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct KrakenRestCandleData {
    pub time: i64,
    pub open: String,
    pub high: String,
    pub low: String,
    pub close: String,
    pub volume: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct KrakenRestCandleResponse {
    pub candles: Vec<KrakenRestCandleData>,
    /// True when the range held more candles than one response returns.
    pub more_candles: bool,
}

/// `analytics_type` of `GET /api/charts/v1/analytics/{symbol}/{analytics_type}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KrakenAnalyticsType {
    OpenInterest,
    AggressorDifferential,
    TradeVolume,
    TradeCount,
    LiquidationVolume,
    RollingVolatility,
    LongShortRatio,
    LongShortInfo,
    Cvd,
    TopTraders,
    Orderbook,
    Spreads,
    Liquidity,
    Slippage,
    FutureBasis,
    Funding,
}

impl KrakenAnalyticsType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::OpenInterest => "open-interest",
            Self::AggressorDifferential => "aggressor-differential",
            Self::TradeVolume => "trade-volume",
            Self::TradeCount => "trade-count",
            Self::LiquidationVolume => "liquidation-volume",
            Self::RollingVolatility => "rolling-volatility",
            Self::LongShortRatio => "long-short-ratio",
            Self::LongShortInfo => "long-short-info",
            Self::Cvd => "cvd",
            Self::TopTraders => "top-traders",
            Self::Orderbook => "orderbook",
            Self::Spreads => "spreads",
            Self::Liquidity => "liquidity",
            Self::Slippage => "slippage",
            Self::FutureBasis => "future-basis",
            Self::Funding => "funding",
        }
    }
}

/// `long-short-info`: account counts and shares per bucket.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct KrakenAnalyticsLongShort {
    #[serde(rename = "longCount")]
    pub long_count: Vec<u64>,
    #[serde(rename = "shortCount")]
    pub short_count: Vec<u64>,
    #[serde(rename = "longPercent")]
    pub long_percent: Vec<String>,
    #[serde(rename = "shortPercent")]
    pub short_percent: Vec<String>,
    pub ratio: Vec<String>,
}

/// `cvd`. Sent in snake_case (the spec says camelCase).
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct KrakenAnalyticsCvd {
    pub buy_volume: Vec<String>,
    pub sell_volume: Vec<String>,
    pub cvd: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct KrakenAnalyticsTopTradersLevel {
    #[serde(rename = "openInterest")]
    pub open_interest: Vec<String>,
    #[serde(rename = "longCount")]
    pub long_count: Vec<u64>,
    #[serde(rename = "shortCount")]
    pub short_count: Vec<u64>,
    #[serde(rename = "longPercent")]
    pub long_percent: Vec<String>,
    #[serde(rename = "shortPercent")]
    pub short_percent: Vec<String>,
    pub ratio: Vec<String>,
}

/// `top-traders`.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct KrakenAnalyticsTopTraders {
    #[serde(rename = "top20Percent")]
    pub top_20_percent: KrakenAnalyticsTopTradersLevel,
}

/// Bid and ask series of the order book analytics.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct KrakenAnalyticsSides<T> {
    pub bid: T,
    pub ask: T,
}

/// `orderbook` side: every book metric. Entries are `null` where Kraken has no value.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct KrakenAnalyticsOrderbookSide {
    #[serde(rename = "bestPrice")]
    pub best_price: Vec<Option<String>>,
    pub liquidity005: Vec<Option<String>>,
    pub liquidity01: Vec<Option<String>>,
    pub liquidity025: Vec<Option<String>>,
    pub liquidity05: Vec<Option<String>>,
    pub liquidity10: Vec<Option<String>>,
    pub liquidity100: Vec<Option<String>>,
    #[serde(rename = "slippage1k")]
    pub slippage_1k: Vec<Option<String>>,
    #[serde(rename = "slippage10k")]
    pub slippage_10k: Vec<Option<String>>,
    #[serde(rename = "slippage100k")]
    pub slippage_100k: Vec<Option<String>>,
    #[serde(rename = "slippage1m")]
    pub slippage_1m: Vec<Option<String>>,
}

/// `spreads` side.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct KrakenAnalyticsSpreadsSide {
    pub best_price: Vec<Option<String>>,
}

/// `liquidity` side: resting size within 0.05 %, 0.1 %, 0.25 %, 0.5 %, 1 % and 10 % of the
/// best price.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct KrakenAnalyticsLiquiditySide {
    pub liquidity_005: Vec<Option<String>>,
    pub liquidity_01: Vec<Option<String>>,
    pub liquidity_025: Vec<Option<String>>,
    pub liquidity_05: Vec<Option<String>>,
    pub liquidity_10: Vec<Option<String>>,
    pub liquidity_100: Vec<Option<String>>,
}

/// `slippage` side: average fill price for 1k, 10k, 100k and 1m USD.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct KrakenAnalyticsSlippageSide {
    pub slippage_1k: Vec<Option<String>>,
    pub slippage_10k: Vec<Option<String>>,
    pub slippage_100k: Vec<Option<String>>,
    pub slippage_1m: Vec<Option<String>>,
}

/// `future-basis`.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct KrakenAnalyticsBasis {
    pub basis: Vec<String>,
}

/// `funding`: per-bucket `[open, high, low, close]` of the absolute and relative rate. The spec
/// says numbers; sent as strings.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct KrakenAnalyticsFunding {
    pub rate: Vec<[String; 4]>,
    #[serde(rename = "relativeRate")]
    pub relative_rate: Vec<[String; 4]>,
}

/// `analytics/liquidity-pool`.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct KrakenAnalyticsLiquidityPool {
    #[serde(rename = "usdValue")]
    pub usd_value: Vec<String>,
}

/// `data` of an analytics reply; the shape depends on the analytics type requested.
#[derive(Debug, Serialize, Clone)]
pub enum KrakenAnalyticsData {
    /// `open-interest`: per-bucket `[open, high, low, close]`.
    Ohlc(Vec<[String; 4]>),
    /// `aggressor-differential`, `trade-volume`, `liquidation-volume`, `rolling-volatility`,
    /// `long-short-ratio`.
    Values(Vec<String>),
    /// `trade-count`.
    Counts(Vec<u64>),
    LongShortInfo(KrakenAnalyticsLongShort),
    Cvd(KrakenAnalyticsCvd),
    TopTraders(KrakenAnalyticsTopTraders),
    Orderbook(KrakenAnalyticsSides<KrakenAnalyticsOrderbookSide>),
    Spreads(KrakenAnalyticsSides<KrakenAnalyticsSpreadsSide>),
    Liquidity(KrakenAnalyticsSides<KrakenAnalyticsLiquiditySide>),
    Slippage(KrakenAnalyticsSides<KrakenAnalyticsSlippageSide>),
    FutureBasis(KrakenAnalyticsBasis),
    Funding(KrakenAnalyticsFunding),
    LiquidityPool(KrakenAnalyticsLiquidityPool),
}

/// An analytics reply. `timestamp` holds the bucket starts as sent: seconds for every type
/// except `funding`, which sends milliseconds.
#[derive(Debug, Serialize, Clone)]
pub struct KrakenAnalytics {
    pub timestamp: Vec<i64>,
    /// True when the range held more buckets than one response returns.
    pub more: bool,
    pub data: KrakenAnalyticsData,
}

/// One entry of `errors` in an analytics reply.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct KrakenAnalyticsError {
    pub severity: String,
    pub error_class: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub msg: Option<String>,
    pub value: Option<String>,
    pub field: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawAnalyticsResult {
    timestamp: Vec<i64>,
    more: bool,
    data: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct RawAnalyticsResponse {
    result: Option<RawAnalyticsResult>,
    errors: Vec<KrakenAnalyticsError>,
}

/// Which `data` shape to expect: an analytics type, or the liquidity pool endpoint.
#[derive(Debug, Clone, Copy)]
enum AnalyticsShape {
    Market(KrakenAnalyticsType),
    LiquidityPool,
}

fn parse_analytics(shape: AnalyticsShape, text: &str) -> Result<KrakenAnalytics, ChartsError> {
    let raw: RawAnalyticsResponse = serde_json::from_str(text)
        .map_err(|e| format!("Failed to parse Kraken analytics response ({e}): {text}"))?;
    let result = match raw.result {
        Some(r) if raw.errors.is_empty() => r,
        _ => return Err(format!("Kraken analytics error: {text}").into()),
    };
    fn de<T: DeserializeOwned>(v: serde_json::Value) -> Result<T, serde_json::Error> {
        serde_json::from_value(v)
    }
    use KrakenAnalyticsType as A;
    let d = result.data;
    let data = match shape {
        AnalyticsShape::LiquidityPool => de(d).map(KrakenAnalyticsData::LiquidityPool),
        AnalyticsShape::Market(kind) => match kind {
            A::OpenInterest => de(d).map(KrakenAnalyticsData::Ohlc),
            A::AggressorDifferential
            | A::TradeVolume
            | A::LiquidationVolume
            | A::RollingVolatility
            | A::LongShortRatio => de(d).map(KrakenAnalyticsData::Values),
            A::TradeCount => de(d).map(KrakenAnalyticsData::Counts),
            A::LongShortInfo => de(d).map(KrakenAnalyticsData::LongShortInfo),
            A::Cvd => de(d).map(KrakenAnalyticsData::Cvd),
            A::TopTraders => de(d).map(KrakenAnalyticsData::TopTraders),
            A::Orderbook => de(d).map(KrakenAnalyticsData::Orderbook),
            A::Spreads => de(d).map(KrakenAnalyticsData::Spreads),
            A::Liquidity => de(d).map(KrakenAnalyticsData::Liquidity),
            A::Slippage => de(d).map(KrakenAnalyticsData::Slippage),
            A::FutureBasis => de(d).map(KrakenAnalyticsData::FutureBasis),
            A::Funding => de(d).map(KrakenAnalyticsData::Funding),
        },
    }
    .map_err(|e| format!("Failed to parse Kraken analytics data ({e}): {text}"))?;
    Ok(KrakenAnalytics {
        timestamp: result.timestamp,
        more: result.more,
        data,
    })
}

fn analytics_query(since: i64, interval: KrakenResolution, to: Option<i64>) -> String {
    let mut q = format!("since={since}&interval={}", interval.as_secs());
    if let Some(to) = to {
        q.push_str(&format!("&to={to}"));
    }
    q
}

pub struct ChartsApi {
    client: Arc<RateLimitedClient>,
}

impl ChartsApi {
    pub fn new_with_client(client: Arc<RateLimitedClient>) -> Self {
        Self { client }
    }

    /// GETs `url`; a non-2xx reply (plain-text for the candle endpoints) becomes an error.
    async fn get_text(&self, url: &str) -> Result<(reqwest::StatusCode, String), ChartsError> {
        let response = self.client.get(url).await?;
        let status = response.status();
        let text = response.text().await?;
        Ok((status, text))
    }

    async fn get_json<R: DeserializeOwned>(&self, url: &str) -> Result<R, ChartsError> {
        let (status, text) = self.get_text(url).await?;
        if !status.is_success() {
            return Err(format!("Kraken charts request failed ({status}): {text}").into());
        }
        let r = serde_json::from_str(&text)
            .map_err(|e| format!("Failed to parse Kraken charts response ({e}): {text}"))?;
        Ok(r)
    }

    /// `GET /api/charts/v1/{tick_type}/{symbol}/{resolution}?from=&to=&count=`.
    ///
    /// `from` and `to` are in seconds; `count` is the number of candles. `None` leaves each to
    /// the exchange. The candles are returned as sent, including the one still forming.
    pub async fn get_candles(
        &self,
        tick_type: KrakenTickType,
        symbol: &str,
        resolution: KrakenResolution,
        from: Option<i64>,
        to: Option<i64>,
        count: Option<u32>,
    ) -> Result<KrakenRestCandleResponse, ChartsError> {
        let mut url = format!(
            "{CHARTS_URL}/{}/{symbol}/{}",
            tick_type.as_str(),
            resolution.as_str()
        );
        let mut sep = '?';
        if let Some(from) = from {
            url.push_str(&format!("{sep}from={from}"));
            sep = '&';
        }
        if let Some(to) = to {
            url.push_str(&format!("{sep}to={to}"));
            sep = '&';
        }
        if let Some(count) = count {
            url.push_str(&format!("{sep}count={count}"));
        }
        self.get_json(&url).await
    }

    /// `GET /api/charts/v1/`: tick types (`mark`, `spot`, `trade`).
    pub async fn get_tick_types(&self) -> Result<Vec<String>, ChartsError> {
        self.get_json(&format!("{CHARTS_URL}/")).await
    }

    /// `GET /api/charts/v1/{tick_type}`: symbols with candles of that type.
    pub async fn get_markets(&self, tick_type: KrakenTickType) -> Result<Vec<String>, ChartsError> {
        self.get_json(&format!("{CHARTS_URL}/{}", tick_type.as_str()))
            .await
    }

    /// `GET /api/charts/v1/{tick_type}/{symbol}`: resolutions available, as sent (e.g. `1h`).
    pub async fn get_resolutions(
        &self,
        tick_type: KrakenTickType,
        symbol: &str,
    ) -> Result<Vec<String>, ChartsError> {
        self.get_json(&format!("{CHARTS_URL}/{}/{symbol}", tick_type.as_str()))
            .await
    }

    /// `GET /api/charts/v1/analytics/{symbol}/{analytics_type}?since=&interval=&to=`.
    /// `since`/`to` are in seconds; `to` defaults to now.
    pub async fn get_analytics(
        &self,
        symbol: &str,
        analytics_type: KrakenAnalyticsType,
        since: i64,
        interval: KrakenResolution,
        to: Option<i64>,
    ) -> Result<KrakenAnalytics, ChartsError> {
        let url = format!(
            "{CHARTS_URL}/analytics/{symbol}/{}?{}",
            analytics_type.as_str(),
            analytics_query(since, interval, to)
        );
        // Errors come as JSON with HTTP 400/404, parsed for the message.
        let (_, text) = self.get_text(&url).await?;
        parse_analytics(AnalyticsShape::Market(analytics_type), &text)
    }

    /// `GET /api/charts/v1/analytics/liquidity-pool?since=&interval=&to=`.
    pub async fn get_liquidity_pool(
        &self,
        since: i64,
        interval: KrakenResolution,
        to: Option<i64>,
    ) -> Result<KrakenAnalytics, ChartsError> {
        let url = format!(
            "{CHARTS_URL}/analytics/liquidity-pool?{}",
            analytics_query(since, interval, to)
        );
        let (_, text) = self.get_text(&url).await?;
        parse_analytics(AnalyticsShape::LiquidityPool, &text)
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

    #[test]
    fn parses_wire_response() {
        let text = r#"{"candles":[{"time":1794747060000,"open":"65385","high":"65410","low":"65380","close":"65403","volume":"0.10440000"}],"more_candles":false}"#;
        let r: KrakenRestCandleResponse = serde_json::from_str(text).unwrap();
        assert_eq!(r.candles.len(), 1);
        assert_eq!(r.candles[0].time, 1794747060000);
        assert_eq!(r.candles[0].volume, "0.10440000");
        assert!(!r.more_candles);
    }

    #[test]
    fn parses_candles_with_count_and_lists() {
        let r: KrakenRestCandleResponse =
            serde_json::from_str(&fixture("charts_candles_count.json")).unwrap();
        assert_eq!(r.candles.len(), 2);
        let t: Vec<String> = serde_json::from_str(&fixture("charts_tick_types.json")).unwrap();
        assert_eq!(t, ["mark", "spot", "trade"]);
        let m: Vec<String> = serde_json::from_str(&fixture("charts_markets.json")).unwrap();
        assert_eq!(m.len(), 5);
        let r: Vec<String> = serde_json::from_str(&fixture("charts_resolutions.json")).unwrap();
        assert!(r.contains(&"1h".to_string()));
    }

    #[test]
    fn parses_analytics_fixtures() {
        use KrakenAnalyticsType as T;
        let cases = [
            ("open_interest", T::OpenInterest),
            ("trade_volume", T::TradeVolume),
            ("trade_count", T::TradeCount),
            ("long_short_info", T::LongShortInfo),
            ("cvd", T::Cvd),
            ("top_traders", T::TopTraders),
            ("orderbook", T::Orderbook),
            ("spreads", T::Spreads),
            ("liquidity", T::Liquidity),
            ("slippage", T::Slippage),
            ("future_basis", T::FutureBasis),
            ("funding", T::Funding),
        ];
        for (file, kind) in cases {
            let text = fixture(&format!("analytics_{file}.json"));
            let a = parse_analytics(AnalyticsShape::Market(kind), &text)
                .unwrap_or_else(|e| panic!("{file}: {e}"));
            assert_eq!(a.timestamp.len(), 2, "{file}");
        }
        let a = parse_analytics(
            AnalyticsShape::LiquidityPool,
            &fixture("analytics_liquidity_pool.json"),
        )
        .unwrap();
        assert!(matches!(a.data, KrakenAnalyticsData::LiquidityPool(_)));

        let a = parse_analytics(
            AnalyticsShape::Market(T::Funding),
            &fixture("analytics_funding.json"),
        )
        .unwrap();
        // Funding buckets are in milliseconds.
        assert!(a.timestamp[0] > 1_000_000_000_000);
    }

    #[test]
    fn analytics_shape_mismatch_and_missing_field_are_errors() {
        // A value series parsed as open interest OHLC.
        let text = fixture("analytics_trade_volume.json");
        assert!(parse_analytics(
            AnalyticsShape::Market(KrakenAnalyticsType::OpenInterest),
            &text
        )
        .is_err());
        let text = fixture("analytics_cvd.json").replace("\"sell_volume\"", "\"x\"");
        assert!(parse_analytics(AnalyticsShape::Market(KrakenAnalyticsType::Cvd), &text).is_err());
        let text = fixture("analytics_funding.json").replace("\"more\":", "\"x\":");
        assert!(
            parse_analytics(AnalyticsShape::Market(KrakenAnalyticsType::Funding), &text).is_err()
        );
    }

    #[test]
    fn analytics_error_reply_is_error() {
        // analytics/PF_XBTUSD/open-interest?interval=7, 2026-11-16 (HTTP 400).
        let text = r#"{"result":null,"errors":[{"severity":"E","error_class":"General","type":"Invalid arguments","msg":null,"value":null,"field":"interval"}]}"#;
        let err = parse_analytics(
            AnalyticsShape::Market(KrakenAnalyticsType::OpenInterest),
            text,
        )
        .unwrap_err();
        assert!(err.to_string().contains("Invalid arguments"));
    }

    fn now() -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
    }

    #[tokio::test]
    async fn test_get_candles() {
        let api = ChartsApi::new_with_client(Arc::new(RateLimitedClient::new(10, 1)));
        let to = now();
        let r = api
            .get_candles(
                KrakenTickType::Trade,
                "PF_XBTUSD",
                KrakenResolution::H1,
                Some(to - 6 * 3600),
                Some(to),
                None,
            )
            .await
            .expect("candles");
        assert!(!r.candles.is_empty());
        let r = api
            .get_candles(
                KrakenTickType::Mark,
                "PF_XBTUSD",
                KrakenResolution::H1,
                None,
                None,
                Some(2),
            )
            .await
            .expect("candles count");
        assert_eq!(r.candles.len(), 2);
    }

    #[tokio::test]
    async fn test_chart_lists_and_analytics() {
        let api = ChartsApi::new_with_client(Arc::new(RateLimitedClient::new(10, 1)));
        assert!(!api.get_tick_types().await.expect("tick types").is_empty());
        assert!(!api
            .get_markets(KrakenTickType::Trade)
            .await
            .expect("markets")
            .is_empty());
        api.get_resolutions(KrakenTickType::Trade, "PF_XBTUSD")
            .await
            .expect("resolutions");
        let since = now() - 4 * 3600;
        for kind in [
            KrakenAnalyticsType::OpenInterest,
            KrakenAnalyticsType::AggressorDifferential,
            KrakenAnalyticsType::TradeVolume,
            KrakenAnalyticsType::TradeCount,
            KrakenAnalyticsType::LiquidationVolume,
            KrakenAnalyticsType::RollingVolatility,
            KrakenAnalyticsType::LongShortRatio,
            KrakenAnalyticsType::LongShortInfo,
            KrakenAnalyticsType::Cvd,
            KrakenAnalyticsType::TopTraders,
            KrakenAnalyticsType::Orderbook,
            KrakenAnalyticsType::Spreads,
            KrakenAnalyticsType::Liquidity,
            KrakenAnalyticsType::Slippage,
            KrakenAnalyticsType::FutureBasis,
            KrakenAnalyticsType::Funding,
        ] {
            api.get_analytics("PF_XBTUSD", kind, since, KrakenResolution::H1, None)
                .await
                .unwrap_or_else(|e| panic!("{kind:?}: {e}"));
        }
        api.get_liquidity_pool(since, KrakenResolution::H1, None)
            .await
            .expect("liquidity pool");
    }
}
