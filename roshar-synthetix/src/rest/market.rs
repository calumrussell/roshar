use super::{read_envelope, read_plain, ApiResult, SYNTHETIX_API_URL, SYNTHETIX_INFO_URL};
use crate::types::{
    SynthetixAggregatorOrderBook, SynthetixCandles, SynthetixCollateralConfig, SynthetixContract,
    SynthetixContractSpec, SynthetixExchangeStatus, SynthetixFundingRate,
    SynthetixFundingRateHistory, SynthetixLastTrades, SynthetixMarket, SynthetixMarketPrice,
    SynthetixOpenInterest, SynthetixOrderBook, SynthetixStatus,
    SynthetixSubAccountIdsWithDelegations, SynthetixTier,
};
use roshar_http::RateLimitedClient;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};
use std::collections::HashMap;

/// `{"params":{"action":…, …}}` body for `POST /v1/info`; unset parameters are left out. The
/// info-websocket `post` frame carries the same `params` object.
pub(crate) struct InfoParams(Map<String, Value>);

impl InfoParams {
    pub(crate) fn new(action: &str) -> Self {
        let mut m = Map::new();
        m.insert("action".into(), Value::from(action));
        InfoParams(m)
    }

    pub(crate) fn param(mut self, key: &str, value: impl Into<Value>) -> Self {
        self.0.insert(key.into(), value.into());
        self
    }

    pub(crate) fn opt(self, key: &str, value: Option<impl Into<Value>>) -> Self {
        match value {
            Some(v) => self.param(key, v),
            None => self,
        }
    }

    pub(crate) fn into_map(self) -> Map<String, Value> {
        self.0
    }

    fn body(self) -> String {
        let mut outer = Map::new();
        outer.insert("params".into(), Value::Object(self.0));
        Value::Object(outer).to_string()
    }
}

/// Public market data. Every request carries the caller's `User-Agent`: the exchange answers
/// HTTP 403 without one.
pub struct MarketApi {
    client: RateLimitedClient,
    user_agent: String,
}

impl MarketApi {
    pub fn new(user_agent: &str, requests_per_second: u32) -> Self {
        Self {
            client: RateLimitedClient::new(requests_per_second, 1),
            user_agent: user_agent.to_string(),
        }
    }

    async fn info<T: DeserializeOwned>(&self, params: InfoParams, what: &str) -> ApiResult<T> {
        let response = self
            .client
            .post(SYNTHETIX_INFO_URL)
            .await
            .header("User-Agent", &self.user_agent)
            .header("Content-Type", "application/json")
            .body(params.body())
            .send()
            .await?;
        read_envelope(response, what).await
    }

    async fn get<T: DeserializeOwned>(
        &self,
        path: &str,
        what: &str,
        enveloped: bool,
    ) -> ApiResult<T> {
        self.client.acquire().await;
        let response = self
            .client
            .inner()
            .get(format!("{SYNTHETIX_API_URL}{path}"))
            .header("User-Agent", &self.user_agent)
            .send()
            .await?;
        if enveloped {
            read_envelope(response, what).await
        } else {
            read_plain(response, what).await
        }
    }

    /// `getMarkets`: market configuration. `active_only: Some(true)` returns open markets only.
    pub async fn get_markets(&self, active_only: Option<bool>) -> ApiResult<Vec<SynthetixMarket>> {
        let p = InfoParams::new("getMarkets").opt("activeOnly", active_only);
        self.info(p, "markets").await
    }

    /// `getMarketPrices`: bid/ask, mark, index, last, funding, OI and period stats, keyed by symbol.
    pub async fn get_market_prices(&self) -> ApiResult<HashMap<String, SynthetixMarketPrice>> {
        self.info(InfoParams::new("getMarketPrices"), "market prices")
            .await
    }

    /// `getOrderbook`. `limit` (levels per side): 5, 10, 20, 50, 100, 500 or 1000; unset = 500.
    pub async fn get_orderbook(
        &self,
        symbol: &str,
        limit: Option<u32>,
    ) -> ApiResult<SynthetixOrderBook> {
        let p = InfoParams::new("getOrderbook")
            .param("symbol", symbol)
            .opt("limit", limit);
        self.info(p, "orderbook").await
    }

    /// `getLastTrades`: recent public trades. `limit` 1..=100; unset = 50.
    pub async fn get_last_trades(
        &self,
        symbol: &str,
        limit: Option<u32>,
    ) -> ApiResult<SynthetixLastTrades> {
        let p = InfoParams::new("getLastTrades")
            .param("symbol", symbol)
            .opt("limit", limit);
        self.info(p, "last trades").await
    }

    /// `getCandles`. `interval`: 1m, 5m, 15m, 30m, 1h, 4h, 8h, 12h, 1d, 3d, 1w, 1M, 3M.
    /// Times are Unix ms; unset start = 24 h ago, unset end = now. `limit` 0 = no limit.
    pub async fn get_candles(
        &self,
        symbol: &str,
        interval: &str,
        start_time: Option<u64>,
        end_time: Option<u64>,
        limit: Option<u32>,
    ) -> ApiResult<SynthetixCandles> {
        let p = InfoParams::new("getCandles")
            .param("symbol", symbol)
            .param("interval", interval)
            .opt("startTime", start_time)
            .opt("endTime", end_time)
            .opt("limit", limit);
        self.info(p, "candles").await
    }

    /// `getFundingRate`: estimated next rate and last settlement.
    pub async fn get_funding_rate(&self, symbol: &str) -> ApiResult<SynthetixFundingRate> {
        let p = InfoParams::new("getFundingRate").param("symbol", symbol);
        self.info(p, "funding rate").await
    }

    /// `getFundingRateHistory`. Unix ms; `start_time` at most 30 days back. `limit` capped at 2160.
    pub async fn get_funding_rate_history(
        &self,
        symbol: &str,
        start_time: u64,
        end_time: u64,
        limit: Option<u32>,
    ) -> ApiResult<SynthetixFundingRateHistory> {
        let p = InfoParams::new("getFundingRateHistory")
            .param("symbol", symbol)
            .param("startTime", start_time)
            .param("endTime", end_time)
            .opt("limit", limit);
        self.info(p, "funding rate history").await
    }

    /// `getOpenInterest`: one entry per market.
    pub async fn get_open_interest(&self) -> ApiResult<Vec<SynthetixOpenInterest>> {
        self.info(InfoParams::new("getOpenInterest"), "open interest")
            .await
    }

    /// `getMids`: mid price keyed by symbol.
    pub async fn get_mids(&self) -> ApiResult<HashMap<String, String>> {
        self.info(InfoParams::new("getMids"), "mids").await
    }

    /// `getCollaterals`: collateral assets, caps, LTVs, haircut tiers, withdrawal fees.
    pub async fn get_collaterals(&self) -> ApiResult<Vec<SynthetixCollateralConfig>> {
        self.info(InfoParams::new("getCollaterals"), "collaterals")
            .await
    }

    /// `getTiers`: fee tiers and order caps.
    pub async fn get_tiers(&self) -> ApiResult<Vec<SynthetixTier>> {
        self.info(InfoParams::new("getTiers"), "tiers").await
    }

    /// `getIsWhitelisted`.
    pub async fn get_is_whitelisted(&self, wallet_address: &str) -> ApiResult<bool> {
        let p = InfoParams::new("getIsWhitelisted").param("walletAddress", wallet_address);
        self.info(p, "whitelist").await
    }

    /// `getSubAccountIds`: subaccounts owned by `wallet_address` (EIP-55 checksummed, else 400).
    pub async fn get_sub_account_ids(&self, wallet_address: &str) -> ApiResult<Vec<String>> {
        let p = InfoParams::new("getSubAccountIds").param("walletAddress", wallet_address);
        self.info(p, "subaccount ids").await
    }

    /// `getSubAccountIds` with `includeDelegations: true`: owned, managed and delegated ids.
    pub async fn get_sub_account_ids_with_delegations(
        &self,
        wallet_address: &str,
    ) -> ApiResult<SynthetixSubAccountIdsWithDelegations> {
        let p = InfoParams::new("getSubAccountIds")
            .param("walletAddress", wallet_address)
            .param("includeDelegations", true);
        self.info(p, "subaccount ids").await
    }

    /// `GET /v1/info/contracts`: open perpetuals with price, funding and OI (aggregator format).
    pub async fn get_contracts(&self) -> ApiResult<Vec<SynthetixContract>> {
        self.get("/v1/info/contracts", "contracts", false).await
    }

    /// `GET /v1/info/contract_specs`.
    pub async fn get_contract_specs(&self) -> ApiResult<Vec<SynthetixContractSpec>> {
        self.get("/v1/info/contract_specs", "contract specs", false)
            .await
    }

    /// `GET /v1/info/orderbook?ticker_id=..[&depth=..]`. `depth`: 5, 10, 20, 50, 100, 500 or
    /// 1000; unset = 100.
    pub async fn get_aggregator_orderbook(
        &self,
        ticker_id: &str,
        depth: Option<u32>,
    ) -> ApiResult<SynthetixAggregatorOrderBook> {
        let path = match depth {
            Some(d) => format!("/v1/info/orderbook?ticker_id={ticker_id}&depth={d}"),
            None => format!("/v1/info/orderbook?ticker_id={ticker_id}"),
        };
        self.get(&path, "aggregator orderbook", false).await
    }

    /// `GET /v1/status`: maintenance flag (what the SDK polls; served during maintenance).
    pub async fn get_status(&self) -> ApiResult<SynthetixStatus> {
        self.get("/v1/status", "status", false).await
    }

    /// `GET /v1/exchange/status`: the documented status route.
    pub async fn get_exchange_status(&self) -> ApiResult<SynthetixExchangeStatus> {
        self.get("/v1/exchange/status", "exchange status", true)
            .await
    }

    /// `GET /v1/ws/exchange/status`: the same status served from the websocket service (plain
    /// HTTP, no upgrade).
    pub async fn get_ws_exchange_status(&self) -> ApiResult<SynthetixExchangeStatus> {
        self.get("/v1/ws/exchange/status", "websocket exchange status", true)
            .await
    }
}
