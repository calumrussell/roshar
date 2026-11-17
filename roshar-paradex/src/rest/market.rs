use crate::types::{
    ParadexBbo, ParadexExchangeStats, ParadexFundingEntry, ParadexImpactPrice,
    ParadexInsuranceFund, ParadexInteractiveOrderBook, ParadexKline, ParadexListResponse,
    ParadexMarginConfig, ParadexMarket, ParadexMarketEvent, ParadexMarketSummary, ParadexOrderBook,
    ParadexPageQuery, ParadexSettlementPrice, ParadexSystemConfig, ParadexSystemState,
    ParadexSystemTime, ParadexTrade,
};
use roshar_http::RateLimitedClient;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use std::collections::HashMap;
use std::fmt::Display;

const BASE_URL: &str = "https://api.prod.paradex.trade/v1";

type ApiResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// `{"results": [...]}` wrapper of the non-paginated endpoints.
#[derive(Deserialize)]
struct Results<T> {
    results: Vec<T>,
}

/// Builds `BASE_URL/{path}?k=v&...`, skipping unset parameters.
struct Query {
    url: String,
    has_params: bool,
}

impl Query {
    fn new(path: &str) -> Self {
        Self {
            url: format!("{BASE_URL}/{path}"),
            has_params: false,
        }
    }

    fn param(mut self, key: &str, value: impl Display) -> Self {
        self.url.push(if self.has_params { '&' } else { '?' });
        self.url.push_str(key);
        self.url.push('=');
        self.url.push_str(&value.to_string());
        self.has_params = true;
        self
    }

    fn opt(self, key: &str, value: Option<impl Display>) -> Self {
        match value {
            Some(v) => self.param(key, v),
            None => self,
        }
    }

    fn page(self, page: &ParadexPageQuery) -> Self {
        self.opt("start_at", page.start_at)
            .opt("end_at", page.end_at)
            .opt("page_size", page.page_size)
            .opt("cursor", page.cursor.as_deref())
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

    async fn get_json<T: DeserializeOwned>(&self, url: &str, what: &str) -> ApiResult<T> {
        let response = self.client.get(url).await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("Paradex {what} failed ({status}): {body}").into());
        }

        let text = response.text().await?;
        let parsed = serde_json::from_str(&text)
            .map_err(|e| format!("Failed to parse Paradex {what}: {e}"))?;
        Ok(parsed)
    }

    /// Fetch static market definitions (contract specs, margin params).
    /// `GET /v1/markets[?market=..|?asset_kind=..&base_asset=..]`
    ///
    /// `asset_kind` and `base_asset` are comma-separated lists and apply only without `market`.
    /// Unfiltered requests draw on a per-client budget (HTTP 429 past it).
    pub async fn get_markets(
        &self,
        market: Option<&str>,
        asset_kind: Option<&str>,
        base_asset: Option<&str>,
    ) -> ApiResult<Vec<ParadexMarket>> {
        let url = Query::new("markets")
            .opt("market", market)
            .opt("asset_kind", asset_kind)
            .opt("base_asset", base_asset)
            .url;
        let wrapper: Results<ParadexMarket> = self.get_json(&url, "markets").await?;
        Ok(wrapper.results)
    }

    /// Fetch live ticker data, keyed by symbol.
    /// `GET /v1/markets/summary?market={market|ALL}[&asset_kind=..&base_asset=..]`
    ///
    /// `market: None` requests `ALL`. `asset_kind` and `base_asset` are comma-separated lists and
    /// apply only to `ALL`. Unfiltered `ALL` requests draw on a per-client budget (HTTP 429 past it).
    pub async fn get_markets_summary(
        &self,
        market: Option<&str>,
        asset_kind: Option<&str>,
        base_asset: Option<&str>,
    ) -> ApiResult<HashMap<String, ParadexMarketSummary>> {
        let url = Query::new("markets/summary")
            .param("market", market.unwrap_or("ALL"))
            .opt("asset_kind", asset_kind)
            .opt("base_asset", base_asset)
            .url;
        let wrapper: Results<ParadexMarketSummary> = self.get_json(&url, "markets summary").await?;
        Ok(wrapper
            .results
            .into_iter()
            .map(|s| (s.symbol.clone(), s))
            .collect())
    }

    /// Fetch an order book snapshot for a market.
    /// `GET /v1/orderbook/{market}[?depth=..&price_tick=..]`
    ///
    /// Unset `depth` uses the exchange default (20 levels). `price_tick` aggregates levels.
    pub async fn get_orderbook(
        &self,
        market: &str,
        depth: Option<u32>,
        price_tick: Option<&str>,
    ) -> ApiResult<ParadexOrderBook> {
        let url = Query::new(&format!("orderbook/{market}"))
            .opt("depth", depth)
            .opt("price_tick", price_tick)
            .url;
        self.get_json(&url, "orderbook").await
    }

    /// Fetch one page of public trades for a market, newest first.
    /// `GET /v1/trades?market={market}[&start_at=..&end_at=..&page_size=..&cursor=..]`
    pub async fn get_trades(
        &self,
        market: &str,
        page: &ParadexPageQuery,
    ) -> ApiResult<ParadexListResponse<ParadexTrade>> {
        let url = Query::new("trades").param("market", market).page(page).url;
        self.get_json(&url, "trades").await
    }

    /// Fetch OHLCV klines for a market.
    /// `GET /v1/markets/klines?symbol=..&resolution=..&start_at=..&end_at=..[&price_kind=..]`
    ///
    /// `resolution` is in minutes: 1, 3, 5, 15, 30 or 60. `start_at` / `end_at` are unix ms.
    /// `price_kind` is `last`, `mark` or `underlying`; unset uses the exchange default.
    pub async fn get_klines(
        &self,
        market: &str,
        resolution: u32,
        start_at: i64,
        end_at: i64,
        price_kind: Option<&str>,
    ) -> ApiResult<Vec<ParadexKline>> {
        let url = Query::new("markets/klines")
            .param("symbol", market)
            .param("resolution", resolution)
            .param("start_at", start_at)
            .param("end_at", end_at)
            .opt("price_kind", price_kind)
            .url;
        let wrapper: Results<ParadexKline> = self.get_json(&url, "klines").await?;
        Ok(wrapper.results)
    }

    /// Fetch one page of historical funding data for a market, newest first.
    /// `GET /v1/funding/data?market={market}[&start_at=..&end_at=..&page_size=..&cursor=..]`
    pub async fn get_funding_history(
        &self,
        market: &str,
        page: &ParadexPageQuery,
    ) -> ApiResult<ParadexListResponse<ParadexFundingEntry>> {
        let url = Query::new("funding/data")
            .param("market", market)
            .page(page)
            .url;
        self.get_json(&url, "funding data").await
    }

    /// Fetch the best bid/offer (API orders, excluding RPI).
    /// `GET /v1/bbo/{market}`
    pub async fn get_bbo(&self, market: &str) -> ApiResult<ParadexBbo> {
        let url = Query::new(&format!("bbo/{market}")).url;
        self.get_json(&url, "bbo").await
    }

    /// Fetch the best bid/offer including RPI orders (as shown in the UI).
    /// `GET /v1/bbo/{market}/interactive`
    pub async fn get_bbo_interactive(&self, market: &str) -> ApiResult<ParadexBbo> {
        let url = Query::new(&format!("bbo/{market}/interactive")).url;
        self.get_json(&url, "interactive bbo").await
    }

    /// Fetch an order book snapshot including RPI orders.
    /// `GET /v1/orderbook/{market}/interactive[?depth=..&price_tick=..]`
    pub async fn get_orderbook_interactive(
        &self,
        market: &str,
        depth: Option<u32>,
        price_tick: Option<&str>,
    ) -> ApiResult<ParadexInteractiveOrderBook> {
        let url = Query::new(&format!("orderbook/{market}/interactive"))
            .opt("depth", depth)
            .opt("price_tick", price_tick)
            .url;
        self.get_json(&url, "interactive orderbook").await
    }

    /// Fetch the average fill price of a market order of `size` on `side` (`BUY` or `SELL`).
    /// `GET /v1/orderbook/{market}/impact-price?side=..&size=..`
    ///
    /// The docs list only `size`; the exchange also requires `side`. An empty book side
    /// returns HTTP 500 `filled size is zero`.
    pub async fn get_impact_price(
        &self,
        market: &str,
        side: &str,
        size: &str,
    ) -> ApiResult<ParadexImpactPrice> {
        let url = Query::new(&format!("orderbook/{market}/impact-price"))
            .param("side", side)
            .param("size", size)
            .url;
        self.get_json(&url, "impact price").await
    }

    /// Fetch market spec change events of the last 7 days, newest first.
    /// `GET /v1/markets/history[?market=..]`
    ///
    /// `market: None` (or `"ALL"`) returns every market.
    pub async fn get_markets_history(
        &self,
        market: Option<&str>,
    ) -> ApiResult<Vec<ParadexMarketEvent>> {
        let url = Query::new("markets/history").opt("market", market).url;
        let wrapper: Results<ParadexMarketEvent> = self.get_json(&url, "markets history").await?;
        Ok(wrapper.results)
    }

    /// Fetch the settlement price of a settled market.
    /// `GET /v1/markets/settlement-price?market=..` (HTTP 404 `MARKET_NOT_SETTLED` before).
    pub async fn get_settlement_price(&self, market: &str) -> ApiResult<ParadexSettlementPrice> {
        let url = Query::new("markets/settlement-price")
            .param("market", market)
            .url;
        self.get_json(&url, "settlement price").await
    }

    /// Fetch the insurance fund account. `GET /v1/insurance`
    pub async fn get_insurance_fund(&self) -> ApiResult<ParadexInsuranceFund> {
        self.get_json(&Query::new("insurance").url, "insurance fund")
            .await
    }

    /// Fetch the clearing and settlement layer config. `GET /v1/system/config`
    pub async fn get_system_config(&self) -> ApiResult<ParadexSystemConfig> {
        self.get_json(&Query::new("system/config").url, "system config")
            .await
    }

    /// Fetch the exchange status. `GET /v1/system/state`
    pub async fn get_system_state(&self) -> ApiResult<ParadexSystemState> {
        self.get_json(&Query::new("system/state").url, "system state")
            .await
    }

    /// Fetch the server time. `GET /v1/system/time`
    pub async fn get_system_time(&self) -> ApiResult<ParadexSystemTime> {
        self.get_json(&Query::new("system/time").url, "system time")
            .await
    }

    /// Fetch exchange-wide stats. `GET /v1/system/stats`
    pub async fn get_exchange_stats(&self) -> ApiResult<ParadexExchangeStats> {
        self.get_json(&Query::new("system/stats").url, "exchange stats")
            .await
    }

    /// Fetch margin configuration per base asset.
    /// `GET /v1/system/margin-config[?base_asset=..]`
    pub async fn get_margin_config(
        &self,
        base_asset: Option<&str>,
    ) -> ApiResult<Vec<ParadexMarginConfig>> {
        let url = Query::new("system/margin-config")
            .opt("base_asset", base_asset)
            .url;
        let wrapper: Results<ParadexMarginConfig> = self.get_json(&url, "margin config").await?;
        Ok(wrapper.results)
    }
}
