use super::{read_json, ApiResult, Query};
use crate::types::{
    Bbo, Book, CollateralAsset, ExchangeInfo, ExchangeStatistics, FeesInfo, FundingHistory,
    IndexPrice, Instrument, KlineInterval, Klines, LimitTier, MarkHistory, PingResponse,
    Registration, ServerTime, Statistic, Ticker, Trades,
};
use roshar_http::RateLimitedClient;
use serde::de::DeserializeOwned;

/// Public `/v1/info/*` endpoints. No authentication.
pub struct MarketApi {
    client: RateLimitedClient,
    base_url: String,
}

impl MarketApi {
    pub fn new(client: RateLimitedClient, base_url: &str) -> Self {
        Self {
            client,
            base_url: base_url.to_string(),
        }
    }

    async fn get<T: DeserializeOwned>(&self, url: String, what: &str) -> ApiResult<T> {
        let response = self.client.get(&url).await?;
        read_json(response, what).await
    }

    fn query(&self, path: &str) -> Query {
        Query::new(&self.base_url, path)
    }

    /// `GET /v1/info/ping`.
    pub async fn ping(&self) -> ApiResult<PingResponse> {
        self.get(self.query("/v1/info/ping").url(), "ping").await
    }

    /// `GET /v1/info/time`.
    pub async fn get_time(&self) -> ApiResult<ServerTime> {
        self.get(self.query("/v1/info/time").url(), "time").await
    }

    /// `GET /v1/info/exchange`: EIP-712 domain fields and cancel-only flag.
    pub async fn get_exchange(&self) -> ApiResult<ExchangeInfo> {
        self.get(self.query("/v1/info/exchange").url(), "exchange")
            .await
    }

    /// `GET /v1/info/assets`.
    pub async fn get_assets(&self) -> ApiResult<Vec<CollateralAsset>> {
        self.get(self.query("/v1/info/assets").url(), "assets")
            .await
    }

    /// `GET /v1/info/instruments[?instrument_id=&instrument_type=&category=]`. Retired
    /// instruments are only returned when filtered by `instrument_id`.
    pub async fn get_instruments(
        &self,
        instrument_id: Option<u32>,
        instrument_type: Option<&str>,
        category: Option<&str>,
    ) -> ApiResult<Vec<Instrument>> {
        let url = self
            .query("/v1/info/instruments")
            .opt("instrument_id", instrument_id)
            .opt("instrument_type", instrument_type)
            .opt("category", category)
            .url();
        self.get(url, "instruments").await
    }

    /// `GET /v1/info/tickers[?instrument_id=]`: prices, open interest and current funding.
    pub async fn get_tickers(&self, instrument_id: Option<u32>) -> ApiResult<Vec<Ticker>> {
        let url = self
            .query("/v1/info/tickers")
            .opt("instrument_id", instrument_id)
            .url();
        self.get(url, "tickers").await
    }

    /// `GET /v1/info/statistics[?instrument_id=]`: rolling 24 h statistics.
    pub async fn get_statistics(&self, instrument_id: Option<u32>) -> ApiResult<Vec<Statistic>> {
        let url = self
            .query("/v1/info/statistics")
            .opt("instrument_id", instrument_id)
            .url();
        self.get(url, "statistics").await
    }

    /// `GET /v1/info/exchange-stats?start_timestamp=&end_timestamp=` (half-open, at most 31 days).
    pub async fn get_exchange_stats(
        &self,
        start_timestamp: i64,
        end_timestamp: i64,
    ) -> ApiResult<ExchangeStatistics> {
        let url = self
            .query("/v1/info/exchange-stats")
            .param("start_timestamp", start_timestamp)
            .param("end_timestamp", end_timestamp)
            .url();
        self.get(url, "exchange stats").await
    }

    /// `GET /v1/info/klines`. Unset `end_timestamp` means now; at most 1000 candles.
    pub async fn get_klines(
        &self,
        instrument_id: u32,
        interval: KlineInterval,
        start_timestamp: i64,
        end_timestamp: Option<i64>,
    ) -> ApiResult<Klines> {
        let url = self
            .query("/v1/info/klines")
            .param("instrument_id", instrument_id)
            .param("interval", interval.as_str())
            .param("start_timestamp", start_timestamp)
            .opt("end_timestamp", end_timestamp)
            .url();
        self.get(url, "klines").await
    }

    /// `GET /v1/info/mark-history`. Unset `end_timestamp` means now; at most 1000 points.
    pub async fn get_mark_history(
        &self,
        instrument_id: u32,
        interval: KlineInterval,
        start_timestamp: i64,
        end_timestamp: Option<i64>,
    ) -> ApiResult<MarkHistory> {
        let url = self
            .query("/v1/info/mark-history")
            .param("instrument_id", instrument_id)
            .param("interval", interval.as_str())
            .param("start_timestamp", start_timestamp)
            .opt("end_timestamp", end_timestamp)
            .url();
        self.get(url, "mark history").await
    }

    /// `GET /v1/info/bbo[?instrument_id=]`.
    pub async fn get_bbo(&self, instrument_id: Option<u32>) -> ApiResult<Vec<Bbo>> {
        let url = self
            .query("/v1/info/bbo")
            .opt("instrument_id", instrument_id)
            .url();
        self.get(url, "bbo").await
    }

    /// `GET /v1/info/book?instrument_id=[&depth=]`. The exchange accepts depth 10, 100, 500 or
    /// 1000 (default 100); deeper books weigh more against the IP budget.
    pub async fn get_book(&self, instrument_id: u32, depth: Option<u32>) -> ApiResult<Book> {
        let url = self
            .query("/v1/info/book")
            .param("instrument_id", instrument_id)
            .opt("depth", depth)
            .url();
        self.get(url, "book").await
    }

    /// `GET /v1/info/index?asset=`.
    pub async fn get_index(&self, asset: &str) -> ApiResult<IndexPrice> {
        let url = self.query("/v1/info/index").param("asset", asset).url();
        self.get(url, "index").await
    }

    /// `GET /v1/info/trades?instrument_id=[&start_timestamp=&end_timestamp=]`, newest first, at
    /// most 100.
    pub async fn get_trades(
        &self,
        instrument_id: u32,
        start_timestamp: Option<i64>,
        end_timestamp: Option<i64>,
    ) -> ApiResult<Trades> {
        let url = self
            .query("/v1/info/trades")
            .param("instrument_id", instrument_id)
            .opt("start_timestamp", start_timestamp)
            .opt("end_timestamp", end_timestamp)
            .url();
        self.get(url, "trades").await
    }

    /// `GET /v1/info/funding?instrument_id=[&start_timestamp=&end_timestamp=]`: funding rate
    /// history, newest first, at most 100.
    pub async fn get_funding_history(
        &self,
        instrument_id: u32,
        start_timestamp: Option<i64>,
        end_timestamp: Option<i64>,
    ) -> ApiResult<FundingHistory> {
        let url = self
            .query("/v1/info/funding")
            .param("instrument_id", instrument_id)
            .opt("start_timestamp", start_timestamp)
            .opt("end_timestamp", end_timestamp)
            .url();
        self.get(url, "funding history").await
    }

    /// `GET /v1/info/fees`.
    pub async fn get_fees(&self) -> ApiResult<FeesInfo> {
        self.get(self.query("/v1/info/fees").url(), "fees").await
    }

    /// `GET /v1/info/limit-tiers`.
    pub async fn get_limit_tiers(&self) -> ApiResult<Vec<LimitTier>> {
        self.get(self.query("/v1/info/limit-tiers").url(), "limit tiers")
            .await
    }

    /// `GET /v1/info/registered?address=`: whether an owner address has a Perps account.
    pub async fn get_registered(&self, address: &str) -> ApiResult<Registration> {
        let url = self
            .query("/v1/info/registered")
            .param("address", address)
            .url();
        self.get(url, "registration").await
    }
}
