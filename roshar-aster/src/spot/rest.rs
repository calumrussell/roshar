use super::types::{
    AsterSpotAggTrade, AsterSpotApiError, AsterSpotBookTicker, AsterSpotExchangeInfo,
    AsterSpotKline, AsterSpotOrderBook, AsterSpotPing, AsterSpotPriceTicker, AsterSpotServerTime,
    AsterSpotTicker24hr, AsterSpotTrade,
};
use crate::types::AsterKlineInterval;
use roshar_http::RateLimitedClient;
use serde::de::DeserializeOwned;

const BASE_URL: &str = "https://sapi.asterdex.com";

type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// Appends `&key=value` when `value` is `Some`.
fn push_opt<T: std::fmt::Display>(url: &mut String, key: &str, value: Option<T>) {
    if let Some(v) = value {
        url.push_str(&format!("&{key}={v}"));
    }
}

pub struct MarketApi {
    client: RateLimitedClient,
}

impl MarketApi {
    pub fn new(requests_per_second: u32) -> Self {
        Self {
            client: RateLimitedClient::new(requests_per_second, 1),
        }
    }

    /// GET `{BASE_URL}{path}` and parse the body; on non-2xx, surfaces the
    /// exchange `{code,msg}` error when the body has that shape.
    async fn get<T: DeserializeOwned>(&self, path: &str, what: &str) -> Result<T, BoxError> {
        let url = format!("{BASE_URL}{path}");
        let response = self.client.get(&url).await?;
        let status = response.status();
        let text = response.text().await?;

        if !status.is_success() {
            return Err(match serde_json::from_str::<AsterSpotApiError>(&text) {
                Ok(e) => format!(
                    "Aster spot {what} failed ({status}): code {} msg {}",
                    e.code, e.msg
                ),
                Err(_) => format!("Aster spot {what} failed ({status}): {text}"),
            }
            .into());
        }

        serde_json::from_str(&text)
            .map_err(|e| format!("Failed to parse Aster spot {what}: {e}").into())
    }

    /// `GET /api/v3/ping`
    pub async fn ping(&self) -> Result<AsterSpotPing, BoxError> {
        self.get("/api/v3/ping", "ping").await
    }

    /// `GET /api/v3/time`
    pub async fn get_server_time(&self) -> Result<AsterSpotServerTime, BoxError> {
        self.get("/api/v3/time", "time").await
    }

    /// `GET /api/v3/exchangeInfo`
    pub async fn get_exchange_info(&self) -> Result<AsterSpotExchangeInfo, BoxError> {
        self.get("/api/v3/exchangeInfo", "exchangeInfo").await
    }

    /// `GET /api/v3/depth`. Valid limits: 5, 10, 20, 50, 100, 500, 1000; `None`
    /// omits it (exchange default 100).
    pub async fn get_orderbook(
        &self,
        symbol: &str,
        limit: Option<u32>,
    ) -> Result<AsterSpotOrderBook, BoxError> {
        let mut path = format!("/api/v3/depth?symbol={symbol}");
        push_opt(&mut path, "limit", limit);
        self.get(&path, "depth").await
    }

    /// `GET /api/v3/trades`. `None` limit omits it (default 500, max 1000).
    pub async fn get_trades(
        &self,
        symbol: &str,
        limit: Option<u32>,
    ) -> Result<Vec<AsterSpotTrade>, BoxError> {
        let mut path = format!("/api/v3/trades?symbol={symbol}");
        push_opt(&mut path, "limit", limit);
        self.get(&path, "trades").await
    }

    /// `GET /api/v3/historicalTrades`. `None` params are omitted.
    pub async fn get_historical_trades(
        &self,
        symbol: &str,
        limit: Option<u32>,
        from_id: Option<u64>,
    ) -> Result<Vec<AsterSpotTrade>, BoxError> {
        let mut path = format!("/api/v3/historicalTrades?symbol={symbol}");
        push_opt(&mut path, "limit", limit);
        push_opt(&mut path, "fromId", from_id);
        self.get(&path, "historicalTrades").await
    }

    /// `GET /api/v3/aggTrades`. `None` params are omitted; the exchange requires
    /// `endTime - startTime` < 1 hour when both are sent.
    pub async fn get_agg_trades(
        &self,
        symbol: &str,
        from_id: Option<u64>,
        start_time: Option<i64>,
        end_time: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<AsterSpotAggTrade>, BoxError> {
        let mut path = format!("/api/v3/aggTrades?symbol={symbol}");
        push_opt(&mut path, "fromId", from_id);
        push_opt(&mut path, "startTime", start_time);
        push_opt(&mut path, "endTime", end_time);
        push_opt(&mut path, "limit", limit);
        self.get(&path, "aggTrades").await
    }

    /// `GET /api/v3/klines`. `None` params are omitted (limit default 500, max 1500).
    pub async fn get_klines(
        &self,
        symbol: &str,
        interval: AsterKlineInterval,
        start_time: Option<i64>,
        end_time: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<AsterSpotKline>, BoxError> {
        let mut path = format!(
            "/api/v3/klines?symbol={symbol}&interval={}",
            interval.as_str()
        );
        push_opt(&mut path, "startTime", start_time);
        push_opt(&mut path, "endTime", end_time);
        push_opt(&mut path, "limit", limit);
        self.get(&path, "klines").await
    }

    /// `GET /api/v3/ticker/24hr?symbol=...`
    pub async fn get_ticker_24hr(&self, symbol: &str) -> Result<AsterSpotTicker24hr, BoxError> {
        self.get(
            &format!("/api/v3/ticker/24hr?symbol={symbol}"),
            "ticker/24hr",
        )
        .await
    }

    /// `GET /api/v3/ticker/24hr` (all symbols, weight 40).
    pub async fn get_tickers_24hr(&self) -> Result<Vec<AsterSpotTicker24hr>, BoxError> {
        self.get("/api/v3/ticker/24hr", "ticker/24hr").await
    }

    /// `GET /api/v3/ticker/price?symbol=...`
    pub async fn get_price_ticker(&self, symbol: &str) -> Result<AsterSpotPriceTicker, BoxError> {
        self.get(
            &format!("/api/v3/ticker/price?symbol={symbol}"),
            "ticker/price",
        )
        .await
    }

    /// `GET /api/v3/ticker/price` (all symbols).
    pub async fn get_price_tickers(&self) -> Result<Vec<AsterSpotPriceTicker>, BoxError> {
        self.get("/api/v3/ticker/price", "ticker/price").await
    }

    /// `GET /api/v3/ticker/bookTicker?symbol=...`
    pub async fn get_book_ticker(&self, symbol: &str) -> Result<AsterSpotBookTicker, BoxError> {
        self.get(
            &format!("/api/v3/ticker/bookTicker?symbol={symbol}"),
            "ticker/bookTicker",
        )
        .await
    }

    /// `GET /api/v3/ticker/bookTicker` (all symbols).
    pub async fn get_book_tickers(&self) -> Result<Vec<AsterSpotBookTicker>, BoxError> {
        self.get("/api/v3/ticker/bookTicker", "ticker/bookTicker")
            .await
    }
}
