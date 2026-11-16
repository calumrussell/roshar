use super::{parse_reply, ApiResult, Query};
use crate::types::{
    ApexDepth, ApexFundingHistory, ApexFundingHistoryQuery, ApexKline, ApexServerTime, ApexSymbols,
    ApexTicker, ApexTrade,
};
use roshar_http::RateLimitedClient;
use serde::de::DeserializeOwned;
use std::collections::HashMap;

/// Public REST endpoints. Symbols: depth, trades, klines and ticker take the `crossSymbolName`
/// (`BTCUSDT`); funding history takes the `symbol` (`BTC-USDT`).
pub struct MarketApi {
    client: RateLimitedClient,
    base_url: &'static str,
}

impl MarketApi {
    pub fn new(client: RateLimitedClient, base_url: &'static str) -> Self {
        Self { client, base_url }
    }

    async fn get<T: DeserializeOwned>(&self, path: &str, what: &str) -> ApiResult<T> {
        let response = self.client.get(&format!("{}{path}", self.base_url)).await?;
        parse_reply(response, what).await
    }

    /// `GET /v3/time`.
    pub async fn get_time(&self) -> ApiResult<ApexServerTime> {
        self.get("/v3/time", "time").await
    }

    /// `GET /v3/symbols`: assets and every contract's trading parameters (including `l2PairId`).
    pub async fn get_symbols(&self) -> ApiResult<ApexSymbols> {
        self.get("/v3/symbols", "symbols").await
    }

    /// `GET /v3/depth?symbol=..[&limit=..]`. Unset `limit` uses the exchange default (100).
    pub async fn get_depth(&self, symbol: &str, limit: Option<u32>) -> ApiResult<ApexDepth> {
        let path = Query::new("/v3/depth")
            .param("symbol", symbol)
            .opt("limit", limit)
            .path;
        self.get(&path, "depth").await
    }

    /// `GET /v3/trades?symbol=..[&limit=..]`, newest last. `limit` is at most 500.
    pub async fn get_trades(&self, symbol: &str, limit: Option<u32>) -> ApiResult<Vec<ApexTrade>> {
        let path = Query::new("/v3/trades")
            .param("symbol", symbol)
            .opt("limit", limit)
            .path;
        self.get(&path, "trades").await
    }

    /// `GET /v3/klines?symbol=..&interval=..[&start=..&end=..&limit=..]`, keyed by symbol.
    ///
    /// `interval`: `1 5 15 30 60 120 240 360 720 D W M`. `start` / `end` are unix seconds; `limit`
    /// is at most 200.
    pub async fn get_klines(
        &self,
        symbol: &str,
        interval: &str,
        start: Option<u64>,
        end: Option<u64>,
        limit: Option<u32>,
    ) -> ApiResult<HashMap<String, Vec<ApexKline>>> {
        let path = Query::new("/v3/klines")
            .param("symbol", symbol)
            .param("interval", interval)
            .opt("start", start)
            .opt("end", end)
            .opt("limit", limit)
            .path;
        self.get(&path, "klines").await
    }

    /// `GET /v3/ticker?symbol=..`: last/mark/index price, 24h stats, current and predicted funding.
    /// An unknown symbol returns an empty list.
    pub async fn get_ticker(&self, symbol: &str) -> ApiResult<Vec<ApexTicker>> {
        let path = Query::new("/v3/ticker").param("symbol", symbol).path;
        self.get(&path, "ticker").await
    }

    /// `GET /v3/history-funding?symbol=..`, newest first. `symbol` is `BTC-USDT` form.
    pub async fn get_funding_history(
        &self,
        symbol: &str,
        query: &ApexFundingHistoryQuery,
    ) -> ApiResult<ApexFundingHistory> {
        let path = Query::new("/v3/history-funding")
            .param("symbol", symbol)
            .opt("limit", query.limit)
            .opt("page", query.page)
            .opt("beginTimeInclusive", query.begin_time_inclusive)
            .opt("endTimeExclusive", query.end_time_exclusive)
            .path;
        self.get(&path, "funding history").await
    }
}
