use crate::types::{
    PhoenixCandle, PhoenixCollateralEvent, PhoenixCommodityCalendar, PhoenixExchange,
    PhoenixExchangeKeys, PhoenixExchangeSnapshot, PhoenixExchangeStatus, PhoenixFill,
    PhoenixFundingHistoryResponse, PhoenixFundingOverview, PhoenixFundingRateHistory,
    PhoenixMarkPrice, PhoenixMarketCalendarRecord, PhoenixMarketCalendarResponse,
    PhoenixMarketConfig, PhoenixMarketStatsHistory, PhoenixNextCommodityMarketTransition,
    PhoenixNextMarketCalendarTransition, PhoenixOrderBook, PhoenixOrderHistory,
    PhoenixPaginatedResponse, PhoenixPnlPoint, PhoenixTradeHistory, PhoenixTraderState,
};
use crate::PHOENIX_REST_URL;
use roshar_http::RateLimitedClient;
use serde::de::DeserializeOwned;

type BoxError = Box<dyn std::error::Error + Send + Sync>;

pub struct MarketApi {
    client: RateLimitedClient,
}

impl MarketApi {
    pub fn new(requests_per_second: u32) -> Self {
        Self {
            client: RateLimitedClient::new(requests_per_second, 1),
        }
    }

    /// Rate-limited GET of `PHOENIX_REST_URL + path` with URL-encoded query pairs.
    async fn get_json<T: DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, String)],
    ) -> Result<T, BoxError> {
        let url = format!("{PHOENIX_REST_URL}{path}");
        self.client.acquire().await;
        let response = self.client.inner().get(&url).query(query).send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("Phoenix GET {path} failed ({status}): {body}").into());
        }

        let text = response.text().await?;
        serde_json::from_str(&text)
            .map_err(|e| format!("Failed to parse Phoenix GET {path} response: {e}").into())
    }

    /// Exchange keys and all market configs.
    /// `GET /v1/view/exchange`
    pub async fn get_exchange(&self) -> Result<PhoenixExchange, BoxError> {
        self.get_json("/v1/view/exchange", &[]).await
    }

    /// All market configurations.
    /// `GET /v1/view/exchange/markets`
    pub async fn get_markets(&self) -> Result<Vec<PhoenixMarketConfig>, BoxError> {
        self.get_json("/v1/view/exchange/markets", &[]).await
    }

    /// Single market configuration. The server matches `symbol` case-insensitively.
    /// `GET /v1/view/exchange/market/{symbol}`
    pub async fn get_market(&self, symbol: &str) -> Result<PhoenixMarketConfig, BoxError> {
        self.get_json(&format!("/v1/view/exchange/market/{symbol}"), &[])
            .await
    }

    /// Exchange account and authority keys.
    /// `GET /v1/view/exchange/keys`
    pub async fn get_exchange_keys(&self) -> Result<PhoenixExchangeKeys, BoxError> {
        self.get_json("/v1/view/exchange/keys", &[]).await
    }

    /// Exchange running/gated/withdrawal status.
    /// `GET /v1/view/exchange/status`
    pub async fn get_exchange_status(&self) -> Result<PhoenixExchangeStatus, BoxError> {
        self.get_json("/v1/view/exchange/status", &[]).await
    }

    /// Exchange state and full market parameters (large: ~0.5 MB).
    /// `GET /v1/exchange/snapshot`
    pub async fn get_exchange_snapshot(&self) -> Result<PhoenixExchangeSnapshot, BoxError> {
        self.get_json("/v1/exchange/snapshot", &[]).await
    }

    /// L2 orderbook snapshot for a market.
    /// `include_splines`: also return spline liquidity. `bypass_execution_band`: full
    /// book during commodities after-hours. The server only honours the camelCase
    /// query names (`includeSplines`, `bypassExecutionBand`); the snake_case names in
    /// the OpenAPI document are silently ignored.
    /// `GET /v1/view/orderbook/{symbol}?includeSplines=&bypassExecutionBand=`
    pub async fn get_orderbook(
        &self,
        symbol: &str,
        include_splines: Option<bool>,
        bypass_execution_band: Option<bool>,
    ) -> Result<PhoenixOrderBook, BoxError> {
        let mut query = Vec::new();
        if let Some(v) = include_splines {
            query.push(("includeSplines", v.to_string()));
        }
        if let Some(v) = bypass_execution_band {
            query.push(("bypassExecutionBand", v.to_string()));
        }
        self.get_json(&format!("/v1/view/orderbook/{symbol}"), &query)
            .await
    }

    /// Current mark price.
    /// `GET /v1/market/{symbol}/mark-price`
    pub async fn get_mark_price(&self, symbol: &str) -> Result<PhoenixMarkPrice, BoxError> {
        self.get_json(&format!("/v1/market/{symbol}/mark-price"), &[])
            .await
    }

    /// Market stats history (OI, mark/spot price, cumulative fees). Times are RFC 3339
    /// strings. Server defaults: last hour, limit 1000 (max 10000), raw points.
    /// `GET /v1/market/{symbol}/stats?start_time=&end_time=&limit=&timeframe=`
    pub async fn get_market_stats(
        &self,
        symbol: &str,
        start_time: Option<&str>,
        end_time: Option<&str>,
        limit: Option<u32>,
        timeframe: Option<&str>,
    ) -> Result<PhoenixMarketStatsHistory, BoxError> {
        let mut query = Vec::new();
        if let Some(v) = start_time {
            query.push(("start_time", v.to_string()));
        }
        if let Some(v) = end_time {
            query.push(("end_time", v.to_string()));
        }
        if let Some(v) = limit {
            query.push(("limit", v.to_string()));
        }
        if let Some(v) = timeframe {
            query.push(("timeframe", v.to_string()));
        }
        self.get_json(&format!("/v1/market/{symbol}/stats"), &query)
            .await
    }

    /// Calendar for a market (404 `market_calendar_not_configured` if it has none).
    /// `GET /v1/market/{symbol}/market-calendar`
    pub async fn get_market_calendar(
        &self,
        symbol: &str,
    ) -> Result<PhoenixMarketCalendarResponse, BoxError> {
        self.get_json(&format!("/v1/market/{symbol}/market-calendar"), &[])
            .await
    }

    /// Calendar by id (e.g. `cme_commodities`, `us_equities_extended`).
    /// `GET /v1/market-calendar/{market_calendar_id}`
    pub async fn get_market_calendar_by_id(
        &self,
        market_calendar_id: &str,
    ) -> Result<PhoenixMarketCalendarRecord, BoxError> {
        self.get_json(&format!("/v1/market-calendar/{market_calendar_id}"), &[])
            .await
    }

    /// `GET /v1/market/commodity-calendar`
    pub async fn get_commodity_calendar(&self) -> Result<PhoenixCommodityCalendar, BoxError> {
        self.get_json("/v1/market/commodity-calendar", &[]).await
    }

    /// `GET /v1/market/{symbol}/next-market-calendar-transition`
    pub async fn get_next_market_calendar_transition(
        &self,
        symbol: &str,
    ) -> Result<PhoenixNextMarketCalendarTransition, BoxError> {
        self.get_json(
            &format!("/v1/market/{symbol}/next-market-calendar-transition"),
            &[],
        )
        .await
    }

    /// `GET /v1/market/next-commodity-market-transition`
    pub async fn get_next_commodity_market_transition(
        &self,
    ) -> Result<PhoenixNextCommodityMarketTransition, BoxError> {
        self.get_json("/v1/market/next-commodity-market-transition", &[])
            .await
    }

    /// Funding rate history. Times are unix milliseconds. Server defaults: last 7 days,
    /// limit 1000 (max 10000).
    /// `GET /v1/funding/{symbol}/rates?startTime=&endTime=&limit=`
    pub async fn get_funding_rates(
        &self,
        symbol: &str,
        start_time: Option<i64>,
        end_time: Option<i64>,
        limit: Option<u32>,
    ) -> Result<PhoenixFundingRateHistory, BoxError> {
        let mut query = Vec::new();
        if let Some(v) = start_time {
            query.push(("startTime", v.to_string()));
        }
        if let Some(v) = end_time {
            query.push(("endTime", v.to_string()));
        }
        if let Some(v) = limit {
            query.push(("limit", v.to_string()));
        }
        self.get_json(&format!("/v1/funding/{symbol}/rates"), &query)
            .await
    }

    /// Funding series for all markets. Times are unix milliseconds. Server defaults:
    /// last 7 days, 500 points per market (max 5000).
    /// `GET /v1/funding/overview?startTime=&endTime=&perMarketLimit=`
    pub async fn get_funding_overview(
        &self,
        start_time: Option<i64>,
        end_time: Option<i64>,
        per_market_limit: Option<u32>,
    ) -> Result<PhoenixFundingOverview, BoxError> {
        let mut query = Vec::new();
        if let Some(v) = start_time {
            query.push(("startTime", v.to_string()));
        }
        if let Some(v) = end_time {
            query.push(("endTime", v.to_string()));
        }
        if let Some(v) = per_market_limit {
            query.push(("perMarketLimit", v.to_string()));
        }
        self.get_json("/v1/funding/overview", &query).await
    }

    /// Public market fills, newest first. Times are unix milliseconds (`end_time`
    /// exclusive). Server default limit 100 (max 10000). Page with `next_cursor`.
    /// `GET /v1/trades/{symbol}/fills?limit=&cursor=&startTime=&endTime=`
    pub async fn get_market_fills(
        &self,
        symbol: &str,
        limit: Option<u32>,
        cursor: Option<&str>,
        start_time: Option<i64>,
        end_time: Option<i64>,
    ) -> Result<PhoenixPaginatedResponse<PhoenixFill>, BoxError> {
        let mut query = Vec::new();
        if let Some(v) = limit {
            query.push(("limit", v.to_string()));
        }
        if let Some(v) = cursor {
            query.push(("cursor", v.to_string()));
        }
        if let Some(v) = start_time {
            query.push(("startTime", v.to_string()));
        }
        if let Some(v) = end_time {
            query.push(("endTime", v.to_string()));
        }
        self.get_json(&format!("/v1/trades/{symbol}/fills"), &query)
            .await
    }

    /// OHLCV candles. Times are unix milliseconds.
    /// Documented timeframes: "1s","5s","1m","5m","15m","30m","1h","4h","1d".
    /// `GET /v1/candles/{symbol}?timeframe=&startTime=&endTime=&limit=`
    pub async fn get_candles(
        &self,
        symbol: &str,
        timeframe: &str,
        start_time: Option<i64>,
        end_time: Option<i64>,
        limit: Option<u32>,
        enable_external_source: Option<bool>,
    ) -> Result<Vec<PhoenixCandle>, BoxError> {
        let mut query = vec![("timeframe", timeframe.to_string())];
        if let Some(st) = start_time {
            query.push(("startTime", st.to_string()));
        }
        if let Some(et) = end_time {
            query.push(("endTime", et.to_string()));
        }
        if let Some(lim) = limit {
            query.push(("limit", lim.to_string()));
        }
        if let Some(v) = enable_external_source {
            query.push(("enableExternalSource", v.to_string()));
        }
        self.get_json(&format!("/v1/candles/{symbol}"), &query)
            .await
    }

    /// Trader state snapshot (all subaccounts) for one trader PDA under an authority.
    /// The server uses PDA index 0 when `trader_pda_index` is `None`.
    /// `GET /v1/trader/state/{authority}?traderPdaIndex=`
    pub async fn get_trader_state(
        &self,
        authority: &str,
        trader_pda_index: Option<u8>,
    ) -> Result<PhoenixTraderState, BoxError> {
        let mut query = Vec::new();
        if let Some(idx) = trader_pda_index {
            query.push(("traderPdaIndex", idx.to_string()));
        }
        self.get_json(&format!("/v1/trader/state/{authority}"), &query)
            .await
    }

    /// PnL time series for an authority (all trader PDAs).
    /// Resolutions: "1m","5m","15m","1h","4h","1d","1w","1M".
    /// `GET /v1/users/{authority}/pnl?resolution=&startTime=&endTime=&limit=`
    pub async fn get_trader_pnl(
        &self,
        authority: &str,
        resolution: &str,
        start_time: Option<i64>,
        end_time: Option<i64>,
        limit: Option<i64>,
    ) -> Result<Vec<PhoenixPnlPoint>, BoxError> {
        let mut query = vec![("resolution", resolution.to_string())];
        if let Some(st) = start_time {
            query.push(("startTime", st.to_string()));
        }
        if let Some(et) = end_time {
            query.push(("endTime", et.to_string()));
        }
        if let Some(lim) = limit {
            query.push(("limit", lim.to_string()));
        }
        self.get_json(&format!("/v1/users/{authority}/pnl"), &query)
            .await
    }

    /// Paginated order history. `limit` is required by the API.
    /// `GET /v1/trader/{authority}/order-history?limit=&traderPdaIndex=&marketSymbol=&cursor=`
    pub async fn get_order_history(
        &self,
        authority: &str,
        trader_pda_index: Option<u8>,
        market_symbol: Option<&str>,
        limit: i64,
        cursor: Option<&str>,
    ) -> Result<PhoenixPaginatedResponse<PhoenixOrderHistory>, BoxError> {
        let mut query = vec![("limit", limit.to_string())];
        if let Some(idx) = trader_pda_index {
            query.push(("traderPdaIndex", idx.to_string()));
        }
        if let Some(sym) = market_symbol {
            query.push(("marketSymbol", sym.to_string()));
        }
        if let Some(c) = cursor {
            query.push(("cursor", c.to_string()));
        }
        self.get_json(&format!("/v1/trader/{authority}/order-history"), &query)
            .await
    }

    /// Paginated trade (fill) history.
    /// `GET /v1/trader/{authority}/trades-history?pdaIndex=&marketSymbol=&limit=&cursor=`
    pub async fn get_trades_history(
        &self,
        authority: &str,
        pda_index: u8,
        market_symbol: Option<&str>,
        limit: Option<i64>,
        cursor: Option<&str>,
    ) -> Result<PhoenixPaginatedResponse<PhoenixTradeHistory>, BoxError> {
        let mut query = vec![("pdaIndex", pda_index.to_string())];
        if let Some(sym) = market_symbol {
            query.push(("marketSymbol", sym.to_string()));
        }
        if let Some(lim) = limit {
            query.push(("limit", lim.to_string()));
        }
        if let Some(c) = cursor {
            query.push(("cursor", c.to_string()));
        }
        self.get_json(&format!("/v1/trader/{authority}/trades-history"), &query)
            .await
    }

    /// Funding payment history. Times are unix milliseconds; `cursor` and
    /// `start_time` are mutually exclusive.
    /// `GET /v1/trader/{authority}/funding-history?traderPdaIndex=&symbol=&startTime=&endTime=&limit=&cursor=`
    #[allow(clippy::too_many_arguments)]
    pub async fn get_funding_history(
        &self,
        authority: &str,
        pda_index: u8,
        symbol: Option<&str>,
        start_time: Option<i64>,
        end_time: Option<i64>,
        limit: Option<i64>,
        cursor: Option<&str>,
    ) -> Result<PhoenixFundingHistoryResponse, BoxError> {
        let mut query = vec![("traderPdaIndex", pda_index.to_string())];
        if let Some(sym) = symbol {
            query.push(("symbol", sym.to_string()));
        }
        if let Some(st) = start_time {
            query.push(("startTime", st.to_string()));
        }
        if let Some(et) = end_time {
            query.push(("endTime", et.to_string()));
        }
        if let Some(lim) = limit {
            query.push(("limit", lim.to_string()));
        }
        if let Some(c) = cursor {
            query.push(("cursor", c.to_string()));
        }
        self.get_json(&format!("/v1/trader/{authority}/funding-history"), &query)
            .await
    }

    /// Paginated collateral deposit/withdrawal/transfer history. `limit` is required
    /// by the API; `next_cursor` fetches older events.
    /// `GET /v1/trader/{authority}/collateral-history?pdaIndex=&limit=&nextCursor=`
    pub async fn get_collateral_history(
        &self,
        authority: &str,
        pda_index: u8,
        limit: i64,
        next_cursor: Option<&str>,
    ) -> Result<PhoenixPaginatedResponse<PhoenixCollateralEvent>, BoxError> {
        let mut query = vec![
            ("pdaIndex", pda_index.to_string()),
            ("limit", limit.to_string()),
        ];
        if let Some(c) = next_cursor {
            query.push(("nextCursor", c.to_string()));
        }
        self.get_json(
            &format!("/v1/trader/{authority}/collateral-history"),
            &query,
        )
        .await
    }
}
