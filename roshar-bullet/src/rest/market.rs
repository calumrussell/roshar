use super::{Http, RestResult};
use crate::types::{
    BulletBorrowLendPool, BulletCmcContract, BulletCoinGeckoContract, BulletExchangeInfo,
    BulletFundingInfo, BulletFundingRate, BulletInsuranceBalance, BulletLeverageBracket,
    BulletListingOrderBook, BulletOpenInterest, BulletOraclePrice, BulletOrderBook,
    BulletPremiumIndex, BulletPriceTicker, BulletRiskEngineConfig, BulletServerTime,
    BulletSymbolConfig, BulletTicker24hr, BulletTrade,
};
use serde::Deserialize;

/// `GET /fapi/v1/ping` returns `{}`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Empty {}

/// Public market data. Bullet has no REST candle endpoint (`SYMBOL@kline_*` is "coming soon").
#[derive(Clone)]
pub struct MarketApi {
    http: Http,
}

impl MarketApi {
    pub(crate) fn new(http: Http) -> Self {
        Self { http }
    }

    /// `GET /fapi/v1/ping`.
    pub async fn ping(&self) -> RestResult<()> {
        let url = self.http.query("/fapi/v1/ping").url;
        let _: Empty = self.http.get(&url, "ping").await?;
        Ok(())
    }

    /// `GET /fapi/v1/time` (µs).
    pub async fn get_server_time(&self) -> RestResult<BulletServerTime> {
        let url = self.http.query("/fapi/v1/time").url;
        self.http.get(&url, "time").await
    }

    /// `GET /fapi/v1/exchangeInfo`: markets, assets and the signing `chainHash`.
    pub async fn get_exchange_info(&self) -> RestResult<BulletExchangeInfo> {
        let url = self.http.query("/fapi/v1/exchangeInfo").url;
        self.http.get(&url, "exchangeInfo").await
    }

    /// `GET /fapi/v1/depth?symbol=..[&limit=..]`. `limit`: default 20, max 1000 (larger values
    /// are clamped). Returns 503 while the server resynchronises the book.
    pub async fn get_depth(&self, symbol: &str, limit: Option<u32>) -> RestResult<BulletOrderBook> {
        let url = self
            .http
            .query("/fapi/v1/depth")
            .param("symbol", symbol)
            .opt("limit", limit)
            .url;
        self.http.get(&url, "depth").await
    }

    /// `GET /fapi/v1/trades?symbol=..[&limit=..]`, newest first. `limit`: default 100, max 1000.
    pub async fn get_trades(
        &self,
        symbol: &str,
        limit: Option<u32>,
    ) -> RestResult<Vec<BulletTrade>> {
        let url = self
            .http
            .query("/fapi/v1/trades")
            .param("symbol", symbol)
            .opt("limit", limit)
            .url;
        self.http.get(&url, "trades").await
    }

    /// `GET /fapi/v1/ticker/24hr` (every market; no symbol filter).
    pub async fn get_ticker_24hr(&self) -> RestResult<Vec<BulletTicker24hr>> {
        let url = self.http.query("/fapi/v1/ticker/24hr").url;
        self.http.get(&url, "ticker/24hr").await
    }

    /// `GET /fapi/v1/ticker/price[?symbol=..]`.
    pub async fn get_ticker_price(
        &self,
        symbol: Option<&str>,
    ) -> RestResult<Vec<BulletPriceTicker>> {
        let url = self
            .http
            .query("/fapi/v1/ticker/price")
            .opt("symbol", symbol)
            .url;
        self.http.get(&url, "ticker/price").await
    }

    /// `GET /fapi/v1/premiumIndex[?symbol=..]`: mark, index, last and estimated funding rate.
    pub async fn get_premium_index(
        &self,
        symbol: Option<&str>,
    ) -> RestResult<Vec<BulletPremiumIndex>> {
        let url = self
            .http
            .query("/fapi/v1/premiumIndex")
            .opt("symbol", symbol)
            .url;
        self.http.get(&url, "premiumIndex").await
    }

    /// `GET /fapi/v1/fundingRate`: settled hourly rates, oldest first. Times are µs, inclusive;
    /// `limit`: default 100, max 1000. Without `start_time` the most recent rows are returned.
    pub async fn get_funding_rate_history(
        &self,
        symbol: Option<&str>,
        start_time: Option<i64>,
        end_time: Option<i64>,
        limit: Option<u32>,
    ) -> RestResult<Vec<BulletFundingRate>> {
        let url = self
            .http
            .query("/fapi/v1/fundingRate")
            .opt("symbol", symbol)
            .opt("startTime", start_time)
            .opt("endTime", end_time)
            .opt("limit", limit)
            .url;
        self.http.get(&url, "fundingRate").await
    }

    /// `GET /fapi/v1/fundingInfo[?symbol=..]`: caps, interval, multiplier, spread.
    pub async fn get_funding_info(
        &self,
        symbol: Option<&str>,
    ) -> RestResult<Vec<BulletFundingInfo>> {
        let url = self
            .http
            .query("/fapi/v1/fundingInfo")
            .opt("symbol", symbol)
            .url;
        self.http.get(&url, "fundingInfo").await
    }

    /// `GET /fapi/v1/openInterest` (every market).
    pub async fn get_open_interest(&self) -> RestResult<Vec<BulletOpenInterest>> {
        let url = self.http.query("/fapi/v1/openInterest").url;
        self.http.get(&url, "openInterest").await
    }

    /// `GET /fapi/v1/symbolConfig[?symbol=..]`.
    pub async fn get_symbol_config(
        &self,
        symbol: Option<&str>,
    ) -> RestResult<Vec<BulletSymbolConfig>> {
        let url = self
            .http
            .query("/fapi/v1/symbolConfig")
            .opt("symbol", symbol)
            .url;
        self.http.get(&url, "symbolConfig").await
    }

    /// `GET /fapi/v1/leverageBracket[?symbol=..]`.
    pub async fn get_leverage_brackets(
        &self,
        symbol: Option<&str>,
    ) -> RestResult<Vec<BulletLeverageBracket>> {
        let url = self
            .http
            .query("/fapi/v1/leverageBracket")
            .opt("symbol", symbol)
            .url;
        self.http.get(&url, "leverageBracket").await
    }

    /// `GET /api/v1/oraclePrices[?assetId=..]`: every registered asset, or one (404 if unknown).
    pub async fn get_oracle_prices(
        &self,
        asset_id: Option<u16>,
    ) -> RestResult<Vec<BulletOraclePrice>> {
        let url = self
            .http
            .query("/api/v1/oraclePrices")
            .opt("assetId", asset_id)
            .url;
        self.http.get(&url, "oraclePrices").await
    }

    /// `GET /fapi/v1/insuranceBalance`.
    pub async fn get_insurance_balance(&self) -> RestResult<BulletInsuranceBalance> {
        let url = self.http.query("/fapi/v1/insuranceBalance").url;
        self.http.get(&url, "insuranceBalance").await
    }

    /// `GET /fapi/v1/borrowLendPools[?symbol=..]`. `symbol` is an asset name (e.g. `USDC`); an
    /// unknown one returns an empty list.
    pub async fn get_borrow_lend_pools(
        &self,
        symbol: Option<&str>,
    ) -> RestResult<Vec<BulletBorrowLendPool>> {
        let url = self
            .http
            .query("/fapi/v1/borrowLendPools")
            .opt("symbol", symbol)
            .url;
        self.http.get(&url, "borrowLendPools").await
    }

    /// `GET /api/v1/riskEngineConfig`: margin tables, liquidation and pricing-engine state.
    pub async fn get_risk_engine_config(&self) -> RestResult<BulletRiskEngineConfig> {
        let url = self.http.query("/api/v1/riskEngineConfig").url;
        self.http.get(&url, "riskEngineConfig").await
    }

    /// `GET /api/v1/cmc/contracts` (CoinMarketCap listing feed).
    pub async fn get_cmc_contracts(&self) -> RestResult<Vec<BulletCmcContract>> {
        let url = self.http.query("/api/v1/cmc/contracts").url;
        self.http.get(&url, "cmc/contracts").await
    }

    /// `GET /api/v1/cmc/orderbook/{ticker_id}[?depth=..]`. `ticker_id` is `BASE-PERP`; `depth`
    /// is total levels across both sides, `0` or `None` for the full book.
    pub async fn get_cmc_orderbook(
        &self,
        ticker_id: &str,
        depth: Option<u32>,
    ) -> RestResult<BulletListingOrderBook> {
        let url = self
            .http
            .query(&format!("/api/v1/cmc/orderbook/{ticker_id}"))
            .opt("depth", depth)
            .url;
        self.http.get(&url, "cmc/orderbook").await
    }

    /// `GET /api/v1/coingecko/contracts` (CoinGecko listing feed).
    pub async fn get_coingecko_contracts(&self) -> RestResult<Vec<BulletCoinGeckoContract>> {
        let url = self.http.query("/api/v1/coingecko/contracts").url;
        self.http.get(&url, "coingecko/contracts").await
    }

    /// `GET /api/v1/coingecko/orderbook?ticker_id=..[&depth=..]`; `depth` as for CMC.
    pub async fn get_coingecko_orderbook(
        &self,
        ticker_id: &str,
        depth: Option<u32>,
    ) -> RestResult<BulletListingOrderBook> {
        let url = self
            .http
            .query("/api/v1/coingecko/orderbook")
            .param("ticker_id", ticker_id)
            .opt("depth", depth)
            .url;
        self.http.get(&url, "coingecko/orderbook").await
    }
}
