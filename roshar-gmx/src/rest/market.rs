use super::http::{get_json, post_json, ApiResult, Query};
use crate::types::{
    GmxCandlePeriod, GmxDirection, GmxMarket, GmxMarketConfig, GmxMarketInfo, GmxMarketRates,
    GmxMarketTicker, GmxMarketValues, GmxOhlcvCandle, GmxOrder, GmxPair, GmxPeriod, GmxPosition,
    GmxRiskOracleMarket, GmxToken, GmxTokenInfo, GmxTokenPrices, GmxTradesList, GmxTradesQuery,
    GmxTradesSearchRequest, GmxTradingCapacity, GmxWalletBalance,
};
use roshar_http::RateLimitedClient;

/// Public reads of the GMX API. `base_url` is a deployment root such as
/// [`crate::GMX_ARBITRUM_API_URL`]; paths carry their `/v1` prefix.
pub struct MarketApi {
    client: RateLimitedClient,
    base_url: String,
}

impl MarketApi {
    pub fn new(base_url: &str, requests_per_second: u32) -> Self {
        Self {
            client: RateLimitedClient::new(requests_per_second, 1),
            base_url: base_url.trim_end_matches('/').to_string(),
        }
    }

    fn q(&self, path: &str) -> Query {
        Query::new(&self.base_url, path)
    }

    /// `GET /v1/markets`: listed markets with leverage tiers and size minimums.
    pub async fn get_markets(&self) -> ApiResult<Vec<GmxMarket>> {
        get_json(&self.client, &self.q("/v1/markets").url, "markets").await
    }

    /// `GET /v1/markets/info`: configuration plus near-live pool, open interest, funding and
    /// borrowing state of every market (1 s cache).
    pub async fn get_markets_info(&self) -> ApiResult<Vec<GmxMarketInfo>> {
        get_json(
            &self.client,
            &self.q("/v1/markets/info").url,
            "markets info",
        )
        .await
    }

    /// `GET /v1/markets/config`: the configuration fields of `/v1/markets/info`.
    pub async fn get_markets_config(&self) -> ApiResult<Vec<GmxMarketConfig>> {
        get_json(
            &self.client,
            &self.q("/v1/markets/config").url,
            "markets config",
        )
        .await
    }

    /// `GET /v1/markets/values`: the live pool, open interest, funding and borrowing fields of
    /// `/v1/markets/info`.
    pub async fn get_markets_values(&self) -> ApiResult<Vec<GmxMarketValues>> {
        get_json(
            &self.client,
            &self.q("/v1/markets/values").url,
            "markets values",
        )
        .await
    }

    /// `GET /v1/risk-oracle/markets`: active markets as named by the risk oracle (503 where the
    /// deployment has no risk oracle).
    pub async fn get_risk_oracle_markets(&self) -> ApiResult<Vec<GmxRiskOracleMarket>> {
        get_json(
            &self.client,
            &self.q("/v1/risk-oracle/markets").url,
            "risk oracle markets",
        )
        .await
    }

    /// `GET /v1/markets/tickers[?addresses=..][&symbols=..]`: prices, 24 h stats, open interest,
    /// current funding/borrowing rates and capacity. Empty filters return every market;
    /// unknown symbols are dropped, malformed addresses are a 400.
    pub async fn get_markets_tickers(
        &self,
        addresses: &[&str],
        symbols: &[&str],
    ) -> ApiResult<Vec<GmxMarketTicker>> {
        let url = self
            .q("/v1/markets/tickers")
            .list("addresses", addresses)
            .list("symbols", symbols)
            .url;
        get_json(&self.client, &url, "markets tickers").await
    }

    /// `GET /v1/markets/trading-capacity?symbol=..&direction=..`: JIT-aware increase capacity of
    /// one market side.
    pub async fn get_trading_capacity(
        &self,
        symbol: &str,
        direction: GmxDirection,
    ) -> ApiResult<GmxTradingCapacity> {
        let url = self
            .q("/v1/markets/trading-capacity")
            .param("symbol", symbol)
            .param("direction", direction.as_str())
            .url;
        get_json(&self.client, &url, "trading capacity").await
    }

    /// `GET /v1/tokens`: token catalog.
    pub async fn get_tokens(&self) -> ApiResult<Vec<GmxToken>> {
        get_json(&self.client, &self.q("/v1/tokens").url, "tokens").await
    }

    /// `GET /v1/tokens/info`: tokens with current min/max prices.
    pub async fn get_tokens_info(&self) -> ApiResult<Vec<GmxTokenInfo>> {
        get_json(&self.client, &self.q("/v1/tokens/info").url, "tokens info").await
    }

    /// `GET /v1/prices/tickers`: oracle min/max price of every token, keyed by address.
    pub async fn get_prices(&self) -> ApiResult<GmxTokenPrices> {
        get_json(&self.client, &self.q("/v1/prices/tickers").url, "prices").await
    }

    /// `GET /v1/prices/ohlcv?symbol=..&timeframe=..[&limit=..][&since=..]`, newest first.
    /// `symbol` is a token symbol (`ETH`) or a full market symbol (`BTC/USD [BTC-USDC]`);
    /// `since` is unix ms.
    pub async fn get_ohlcv(
        &self,
        symbol: &str,
        timeframe: GmxCandlePeriod,
        limit: Option<u32>,
        since: Option<u64>,
    ) -> ApiResult<Vec<GmxOhlcvCandle>> {
        let url = self
            .q("/v1/prices/ohlcv")
            .param("symbol", symbol)
            .param("timeframe", timeframe.as_str())
            .opt("limit", limit)
            .opt("since", since)
            .url;
        get_json(&self.client, &url, "ohlcv").await
    }

    /// `GET /v1/rates[?period=..][&averageBy=..][&address=..]`: hourly funding and borrowing rate
    /// snapshots per market (indexed history, not live; live rates are in tickers / markets info).
    pub async fn get_rates(
        &self,
        period: Option<GmxPeriod>,
        average_by: Option<GmxPeriod>,
        market_address: Option<&str>,
    ) -> ApiResult<Vec<GmxMarketRates>> {
        let url = self
            .q("/v1/rates")
            .opt("period", period.map(GmxPeriod::as_str))
            .opt("averageBy", average_by.map(GmxPeriod::as_str))
            .opt("address", market_address)
            .url;
        get_json(&self.client, &url, "rates").await
    }

    /// `GET /v1/pairs`: aggregator pair feed (perpetual and spot rows).
    pub async fn get_pairs(&self) -> ApiResult<Vec<GmxPair>> {
        get_json(&self.client, &self.q("/v1/pairs").url, "pairs").await
    }

    /// `GET /v1/pairs/solana`: GMX-on-Solana perpetual pairs in the `/v1/pairs` format. Served
    /// only by the deployment that computes protocol stats (Arbitrum; Avalanche answers 503).
    pub async fn get_solana_pairs(&self) -> ApiResult<Vec<GmxPair>> {
        get_json(
            &self.client,
            &self.q("/v1/pairs/solana").url,
            "solana pairs",
        )
        .await
    }

    /// `GET /v1/positions?address=..[&includeRelatedOrders=true]`.
    pub async fn get_positions(
        &self,
        address: &str,
        include_related_orders: bool,
    ) -> ApiResult<Vec<GmxPosition>> {
        let url = self
            .q("/v1/positions")
            .param("address", address)
            .opt(
                "includeRelatedOrders",
                include_related_orders.then_some(true),
            )
            .url;
        get_json(&self.client, &url, "positions").await
    }

    /// `GET /v1/positions/{key}[?includeRelatedOrders=true]`; `key` is the position's
    /// `contract_key` (bytes32). A closed position is a 404.
    pub async fn get_position(
        &self,
        key: &str,
        include_related_orders: bool,
    ) -> ApiResult<GmxPosition> {
        let url = self
            .q(&format!("/v1/positions/{key}"))
            .opt(
                "includeRelatedOrders",
                include_related_orders.then_some(true),
            )
            .url;
        get_json(&self.client, &url, "position").await
    }

    /// `GET /v1/orders?address=..`: open orders of an account.
    pub async fn get_orders(&self, address: &str) -> ApiResult<Vec<GmxOrder>> {
        let url = self.q("/v1/orders").param("address", address).url;
        get_json(&self.client, &url, "orders").await
    }

    /// `GET /v1/orders/{key}`: one open order by key.
    pub async fn get_order(&self, key: &str) -> ApiResult<GmxOrder> {
        get_json(
            &self.client,
            &self.q(&format!("/v1/orders/{key}")).url,
            "order",
        )
        .await
    }

    /// `GET /v1/trades?...`: account trade history (order created/updated/cancelled/executed
    /// events), newest first. `address` is required by the API.
    pub async fn get_trades(&self, query: &GmxTradesQuery) -> ApiResult<GmxTradesList> {
        let url = self
            .q("/v1/trades")
            .opt("address", query.address.as_deref())
            .opt("symbol", query.symbol.as_deref())
            .opt("marketAddress", query.market_address.as_deref())
            .opt("since", query.since)
            .opt("until", query.until)
            .list("actions", query.actions.as_slice())
            .opt("limit", query.limit)
            .opt("cursor", query.cursor.as_deref())
            .url;
        get_json(&self.client, &url, "trades").await
    }

    /// `POST /v1/trades/search`: trade history with the UI's filters. With
    /// `for_all_accounts: Some(true)` and no `address` it is the public trade tape.
    pub async fn search_trades(
        &self,
        request: &GmxTradesSearchRequest,
    ) -> ApiResult<GmxTradesList> {
        post_json(
            &self.client,
            &self.q("/v1/trades/search").url,
            request,
            "trades search",
        )
        .await
    }

    /// `GET /v1/balances/wallet?address=..`: wallet token balances.
    pub async fn get_wallet_balances(&self, address: &str) -> ApiResult<Vec<GmxWalletBalance>> {
        let url = self.q("/v1/balances/wallet").param("address", address).url;
        get_json(&self.client, &url, "wallet balances").await
    }
}
