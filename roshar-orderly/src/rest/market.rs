use super::{parse_bare, parse_data, parse_query, ApiResult, Rows};
use crate::types::{
    OrderlyBinaryRound, OrderlyBinaryRoundDetail, OrderlyChainInfo, OrderlyFundingRate,
    OrderlyFundingRateHistory, OrderlyFundingSummary, OrderlyFuture, OrderlyHistoryChart,
    OrderlyIndexPricePoint, OrderlyIndexPriceSource, OrderlyInsuranceFund, OrderlyLeverageConfig,
    OrderlyLiquidatedPosition, OrderlyLiquidation, OrderlyMarketTrade, OrderlyMaxLeverage,
    OrderlyPage, OrderlyPriceChanges, OrderlyQueryCandle, OrderlyQueryFundingComparison,
    OrderlyQueryFundingRate, OrderlyQueryLiquidation, OrderlyQueryMarketDetail,
    OrderlyQueryMarketSummary, OrderlyQueryOrderbook, OrderlyQueryPage, OrderlyQueryTrade,
    OrderlySymbol, OrderlySystemInfo, OrderlyToken, OrderlyTradersOpenInterest, OrderlyTvBars,
    OrderlyTvConfig, OrderlyTvSymbolInfo,
};
use crate::wire::{path_segment, PathQuery};
use roshar_http::RateLimitedClient;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};

/// `marketDetail` request of the Public Info API. Unset fields are omitted (exchange defaults).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OrderlyMarketDetailQuery<'a> {
    pub symbol: &'a str,
    /// Subset of `market_info`, `orderbook`, `recent_trades`, `funding_history`, `candles`.
    pub include: Option<&'a [&'a str]>,
    pub orderbook_levels: Option<u32>,
    pub recent_trades_limit: Option<u32>,
    pub funding_history_limit: Option<u32>,
    pub candles_interval: Option<&'a str>,
    pub candles_limit: Option<u32>,
}

impl<'a> OrderlyMarketDetailQuery<'a> {
    pub fn new(symbol: &'a str) -> Self {
        Self {
            symbol,
            include: None,
            orderbook_levels: None,
            recent_trades_limit: None,
            funding_history_limit: None,
            candles_interval: None,
            candles_limit: None,
        }
    }
}

/// JSON body of a Public Info API request; `None` values are omitted.
struct QueryBody(Map<String, Value>);

impl QueryBody {
    fn new(kind: &str) -> Self {
        let mut m = Map::new();
        m.insert("type".to_string(), Value::from(kind));
        Self(m)
    }

    fn opt(mut self, key: &str, value: Option<impl Into<Value>>) -> Self {
        if let Some(v) = value {
            self.0.insert(key.to_string(), v.into());
        }
        self
    }

    fn finish(self) -> String {
        Value::Object(self.0).to_string()
    }
}

/// Public (unsigned) market data.
pub struct MarketApi {
    client: RateLimitedClient,
    base_url: String,
}

impl MarketApi {
    pub fn new(client: RateLimitedClient, base_url: &str) -> Self {
        Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
        }
    }

    async fn fetch(&self, path: &str) -> ApiResult<(u16, String)> {
        let url = format!("{}{path}", self.base_url);
        let response = self.client.get(&url).await?;
        let status = response.status().as_u16();
        let body = response.text().await?;
        Ok((status, body))
    }

    async fn get_data<T: DeserializeOwned>(&self, path: &str, what: &str) -> ApiResult<T> {
        let (status, body) = self.fetch(path).await?;
        Ok(parse_data(status, &body, what)?)
    }

    async fn get_rows<T: DeserializeOwned>(&self, path: &str, what: &str) -> ApiResult<Vec<T>> {
        let rows: Rows<T> = self.get_data(path, what).await?;
        Ok(rows.rows)
    }

    /// `GET /v1/public/info` — every symbol with its order rules.
    pub async fn get_symbols(&self) -> ApiResult<Vec<OrderlySymbol>> {
        self.get_rows("/v1/public/info", "symbols").await
    }

    /// `GET /v1/public/info/{symbol}`
    pub async fn get_symbol(&self, symbol: &str) -> ApiResult<OrderlySymbol> {
        path_segment("symbol", symbol)?;
        self.get_data(&format!("/v1/public/info/{symbol}"), "symbol")
            .await
    }

    /// `GET /v1/public/futures` — prices, funding, open interest and 24 h stats for every symbol.
    pub async fn get_futures(&self) -> ApiResult<Vec<OrderlyFuture>> {
        self.get_rows("/v1/public/futures", "futures").await
    }

    /// `GET /v1/public/futures/{symbol}`
    pub async fn get_future(&self, symbol: &str) -> ApiResult<OrderlyFuture> {
        path_segment("symbol", symbol)?;
        self.get_data(&format!("/v1/public/futures/{symbol}"), "future")
            .await
    }

    /// `GET /v1/public/market_trades?symbol=..[&limit=..]` — latest trades; unset `limit` is the
    /// exchange default (10).
    pub async fn get_market_trades(
        &self,
        symbol: &str,
        limit: Option<u32>,
    ) -> ApiResult<Vec<OrderlyMarketTrade>> {
        let path = PathQuery::new("/v1/public/market_trades")
            .param("symbol", symbol)?
            .opt("limit", limit)?
            .finish();
        self.get_rows(&path, "market trades").await
    }

    /// `GET /v1/public/funding_rates` — predicted funding for every symbol.
    pub async fn get_funding_rates(&self) -> ApiResult<Vec<OrderlyFundingRate>> {
        self.get_rows("/v1/public/funding_rates", "funding rates")
            .await
    }

    /// `GET /v1/public/funding_rate/{symbol}`
    pub async fn get_funding_rate(&self, symbol: &str) -> ApiResult<OrderlyFundingRate> {
        path_segment("symbol", symbol)?;
        self.get_data(&format!("/v1/public/funding_rate/{symbol}"), "funding rate")
            .await
    }

    /// `GET /v1/public/funding_rate_history?symbol=..[&start_t&end_t&page&size]` — newest first.
    pub async fn get_funding_rate_history(
        &self,
        symbol: &str,
        start_t: Option<u64>,
        end_t: Option<u64>,
        page: Option<u32>,
        size: Option<u32>,
    ) -> ApiResult<OrderlyPage<OrderlyFundingRateHistory>> {
        let path = PathQuery::new("/v1/public/funding_rate_history")
            .param("symbol", symbol)?
            .opt("start_t", start_t)?
            .opt("end_t", end_t)?
            .opt("page", page)?
            .opt("size", size)?
            .finish();
        self.get_data(&path, "funding rate history").await
    }

    /// `GET /v1/public/market_info/funding_history` — summed funding over fixed windows, every
    /// symbol.
    pub async fn get_funding_summary(&self) -> ApiResult<Vec<OrderlyFundingSummary>> {
        self.get_rows("/v1/public/market_info/funding_history", "funding summary")
            .await
    }

    /// `GET /v1/public/market_info/price_changes`
    pub async fn get_price_changes(&self) -> ApiResult<Vec<OrderlyPriceChanges>> {
        self.get_rows("/v1/public/market_info/price_changes", "price changes")
            .await
    }

    /// `GET /v1/public/market_info/traders_open_interests` — long/short OI, market makers excluded.
    pub async fn get_traders_open_interests(&self) -> ApiResult<Vec<OrderlyTradersOpenInterest>> {
        self.get_rows(
            "/v1/public/market_info/traders_open_interests",
            "traders open interests",
        )
        .await
    }

    /// `GET /v1/tv/kline_history?symbol=..&resolution=..&from=..&to=..[&limit=..]` — candles
    /// without an Orderly key. `resolution` is `1m`, `5m`, `15m`, `30m`, `1h`, `4h`, `12h`, `1d`,
    /// `1w` or `1mon`; `from`/`to` are unix seconds; at most 1000 bars, counted back from `to`.
    /// The docs mark `from`/`to` optional, but the exchange rejects requests without them (-1005).
    pub async fn get_kline_history(
        &self,
        symbol: &str,
        resolution: &str,
        from: i64,
        to: i64,
        limit: Option<u32>,
    ) -> ApiResult<OrderlyTvBars> {
        let path = PathQuery::new("/v1/tv/kline_history")
            .param("symbol", symbol)?
            .param("resolution", resolution)?
            .param("from", from)?
            .param("to", to)?
            .opt("limit", limit)?
            .finish();
        let (status, body) = self.fetch(&path).await?;
        Ok(parse_bare(status, &body, "kline history")?)
    }

    /// `GET /v1/tv/history?symbol=..&resolution=..&from=..&to=..` — TradingView bars;
    /// `resolution` in TradingView form (`1`, `5`, `60`, `1D`, ...); `from`/`to` unix seconds.
    pub async fn get_tv_history(
        &self,
        symbol: &str,
        resolution: &str,
        from: i64,
        to: i64,
    ) -> ApiResult<OrderlyTvBars> {
        let path = PathQuery::new("/v1/tv/history")
            .param("symbol", symbol)?
            .param("resolution", resolution)?
            .param("from", from)?
            .param("to", to)?
            .finish();
        let (status, body) = self.fetch(&path).await?;
        Ok(parse_bare(status, &body, "tv history")?)
    }

    /// `GET /v1/public/system_info` — maintenance status.
    pub async fn get_system_info(&self) -> ApiResult<OrderlySystemInfo> {
        self.get_data("/v1/public/system_info", "system info").await
    }

    /// `GET /v1/public/market_info/history_charts?time_interval=..` — close-price series for
    /// every non-Predict symbol, newest first. The docs mark `time_interval` optional; the
    /// exchange rejects requests without it (-1005). Live: `1h`, `4h`, `1d`, `1w` answered;
    /// `1m`, `5m`, `15m` returned -1000.
    pub async fn get_history_charts(
        &self,
        time_interval: &str,
    ) -> ApiResult<Vec<OrderlyHistoryChart>> {
        let path = PathQuery::new("/v1/public/market_info/history_charts")
            .param("time_interval", time_interval)?
            .finish();
        self.get_rows(&path, "history charts").await
    }

    /// `GET /v1/public/index_price?symbol=..&start_t=..&end_t=..[&limit=..]` — 1 s index prices,
    /// ascending. `start_t` inclusive, `end_t` exclusive (ms); the window must lie in the last
    /// 24 h and span 1 s to 10 min. At most `min(limit, 1000)` rows.
    pub async fn get_index_price_history(
        &self,
        symbol: &str,
        start_t: u64,
        end_t: u64,
        limit: Option<u32>,
    ) -> ApiResult<Vec<OrderlyIndexPricePoint>> {
        let path = PathQuery::new("/v1/public/index_price")
            .param("symbol", symbol)?
            .param("start_t", start_t)?
            .param("end_t", end_t)?
            .opt("limit", limit)?
            .finish();
        self.get_rows(&path, "index price history").await
    }

    /// `GET /v1/public/index_price_source` — venues behind each index.
    pub async fn get_index_price_sources(&self) -> ApiResult<Vec<OrderlyIndexPriceSource>> {
        self.get_rows("/v1/public/index_price_source", "index price sources")
            .await
    }

    /// `GET /v1/public/insurancefund`
    pub async fn get_insurance_fund(&self) -> ApiResult<OrderlyInsuranceFund> {
        self.get_data("/v1/public/insurancefund", "insurance fund")
            .await
    }

    /// `GET /v1/public/config` — selectable leverage values.
    pub async fn get_leverage_config(&self) -> ApiResult<OrderlyLeverageConfig> {
        self.get_data("/v1/public/config", "leverage config").await
    }

    /// `GET /v1/public/leverage` — platform maximum leverage.
    pub async fn get_max_leverage(&self) -> ApiResult<OrderlyMaxLeverage> {
        self.get_data("/v1/public/leverage", "max leverage").await
    }

    /// `GET /v1/public/liquidated_positions[?symbol&start_t&end_t&page&size]` — completed
    /// liquidations, newest first.
    pub async fn get_liquidated_positions(
        &self,
        symbol: Option<&str>,
        start_t: Option<u64>,
        end_t: Option<u64>,
        page: Option<u32>,
        size: Option<u32>,
    ) -> ApiResult<OrderlyPage<OrderlyLiquidatedPosition>> {
        let path = PathQuery::new("/v1/public/liquidated_positions")
            .opt("symbol", symbol)?
            .opt("start_t", start_t)?
            .opt("end_t", end_t)?
            .opt("page", page)?
            .opt("size", size)?
            .finish();
        self.get_data(&path, "liquidated positions").await
    }

    /// `GET /v1/public/liquidation[?start_t&end_t&page&size]` — positions currently up for
    /// liquidation (claimable by liquidators).
    pub async fn get_liquidations(
        &self,
        start_t: Option<u64>,
        end_t: Option<u64>,
        page: Option<u32>,
        size: Option<u32>,
    ) -> ApiResult<OrderlyPage<OrderlyLiquidation>> {
        let path = PathQuery::new("/v1/public/liquidation")
            .opt("start_t", start_t)?
            .opt("end_t", end_t)?
            .opt("page", page)?
            .opt("size", size)?
            .finish();
        self.get_data(&path, "liquidations").await
    }

    /// `GET /v1/public/token[?chain_id=..]` — collateral tokens, weights and per-chain details.
    pub async fn get_tokens(&self, chain_id: Option<&str>) -> ApiResult<Vec<OrderlyToken>> {
        let path = PathQuery::new("/v1/public/token")
            .opt("chain_id", chain_id)?
            .finish();
        self.get_rows(&path, "tokens").await
    }

    /// `GET /v1/public/chain_info[?broker_id=..]` — supported chains and their builders.
    pub async fn get_chain_info(
        &self,
        broker_id: Option<&str>,
    ) -> ApiResult<Vec<OrderlyChainInfo>> {
        let path = PathQuery::new("/v1/public/chain_info")
            .opt("broker_id", broker_id)?
            .finish();
        self.get_rows(&path, "chain info").await
    }

    /// `GET /v1/public/binary/info` — Orderly Predict rounds (most recent 300).
    pub async fn get_binary_rounds(&self) -> ApiResult<Vec<OrderlyBinaryRound>> {
        self.get_rows("/v1/public/binary/info", "binary rounds")
            .await
    }

    /// `GET /v1/public/binary/info/{symbol}` — one Predict round with settlement evidence.
    pub async fn get_binary_round(&self, symbol: &str) -> ApiResult<OrderlyBinaryRoundDetail> {
        path_segment("symbol", symbol)?;
        self.get_data(&format!("/v1/public/binary/info/{symbol}"), "binary round")
            .await
    }

    /// `GET /v1/tv/symbol_info?group=..` — TradingView symbol metadata (the server returns every
    /// symbol whatever `group` is).
    pub async fn get_tv_symbol_info(&self, group: &str) -> ApiResult<OrderlyTvSymbolInfo> {
        let path = PathQuery::new("/v1/tv/symbol_info")
            .param("group", group)?
            .finish();
        let (status, body) = self.fetch(&path).await?;
        Ok(parse_bare(status, &body, "tv symbol info")?)
    }

    /// `GET /v1/tv/config?locale=..` — TradingView datafeed config.
    pub async fn get_tv_config(&self, locale: &str) -> ApiResult<OrderlyTvConfig> {
        let path = PathQuery::new("/v1/tv/config")
            .param("locale", locale)?
            .finish();
        let (status, body) = self.fetch(&path).await?;
        Ok(parse_bare(status, &body, "tv config")?)
    }

    // ── Public Info API: `POST /v1/public/query` (own per-IP weight pool) ──

    async fn query<T: DeserializeOwned>(&self, body: QueryBody, what: &str) -> ApiResult<T> {
        let url = format!("{}/v1/public/query", self.base_url);
        let response = self
            .client
            .post(&url)
            .await
            .header("Content-Type", "application/json")
            .body(body.finish())
            .send()
            .await?;
        let status = response.status().as_u16();
        let text = response.text().await?;
        Ok(parse_query(status, &text, what)?)
    }

    /// `marketSummary`; `None` (or empty) is every active symbol.
    pub async fn query_market_summary(
        &self,
        symbols: Option<&[&str]>,
    ) -> ApiResult<OrderlyQueryMarketSummary> {
        let body = QueryBody::new("marketSummary").opt("symbols", symbols.map(|s| s.to_vec()));
        self.query(body, "market summary").await
    }

    /// `orderbook`; `max_level` 1..1000 (exchange default 100).
    pub async fn query_orderbook(
        &self,
        symbol: &str,
        max_level: Option<u32>,
    ) -> ApiResult<OrderlyQueryOrderbook> {
        let body = QueryBody::new("orderbook")
            .opt("symbol", Some(symbol))
            .opt("max_level", max_level);
        self.query(body, "query orderbook").await
    }

    /// `candles`: `interval` `1m`, `5m`, `15m`, `30m`, `1h`, `4h`, `1d`, `1w`; times filter on
    /// open time (ms); newest first; pass `next_cursor` back as `cursor`.
    pub async fn query_candles(
        &self,
        symbol: &str,
        interval: &str,
        start_time: Option<u64>,
        end_time: Option<u64>,
        limit: Option<u32>,
        cursor: Option<&str>,
    ) -> ApiResult<OrderlyQueryPage<OrderlyQueryCandle>> {
        let body = QueryBody::new("candles")
            .opt("symbol", Some(symbol))
            .opt("interval", Some(interval))
            .opt("start_time", start_time)
            .opt("end_time", end_time)
            .opt("limit", limit)
            .opt("cursor", cursor);
        self.query(body, "query candles").await
    }

    /// `marketTrades` (taker side, with address/account/broker).
    pub async fn query_market_trades(
        &self,
        symbol: &str,
        limit: Option<u32>,
    ) -> ApiResult<Vec<OrderlyQueryTrade>> {
        let body = QueryBody::new("marketTrades")
            .opt("symbol", Some(symbol))
            .opt("limit", limit);
        let rows: Rows<OrderlyQueryTrade> = self.query(body, "query market trades").await?;
        Ok(rows.rows)
    }

    /// `liquidations`; `None` symbol is every symbol.
    pub async fn query_liquidations(
        &self,
        symbol: Option<&str>,
        limit: Option<u32>,
        cursor: Option<&str>,
    ) -> ApiResult<OrderlyQueryPage<OrderlyQueryLiquidation>> {
        let body = QueryBody::new("liquidations")
            .opt("symbol", symbol)
            .opt("limit", limit)
            .opt("cursor", cursor);
        self.query(body, "query liquidations").await
    }

    /// `fundingRateHistory` (settled rates with the mark price at settlement).
    pub async fn query_funding_rate_history(
        &self,
        symbol: &str,
        start_time: Option<u64>,
        end_time: Option<u64>,
        limit: Option<u32>,
        cursor: Option<&str>,
    ) -> ApiResult<OrderlyQueryPage<OrderlyQueryFundingRate>> {
        let body = QueryBody::new("fundingRateHistory")
            .opt("symbol", Some(symbol))
            .opt("start_time", start_time)
            .opt("end_time", end_time)
            .opt("limit", limit)
            .opt("cursor", cursor);
        self.query(body, "query funding rate history").await
    }

    /// `fundingComparison` (Orderly vs other venues); `None` symbol is every symbol.
    pub async fn query_funding_comparison(
        &self,
        symbol: Option<&str>,
    ) -> ApiResult<Vec<OrderlyQueryFundingComparison>> {
        let body = QueryBody::new("fundingComparison").opt("symbol", symbol);
        let rows: Rows<OrderlyQueryFundingComparison> =
            self.query(body, "query funding comparison").await?;
        Ok(rows.rows)
    }

    /// `marketDetail`.
    pub async fn query_market_detail(
        &self,
        q: &OrderlyMarketDetailQuery<'_>,
    ) -> ApiResult<OrderlyQueryMarketDetail> {
        let body = QueryBody::new("marketDetail")
            .opt("symbol", Some(q.symbol))
            .opt("include", q.include.map(|s| s.to_vec()))
            .opt("orderbook_levels", q.orderbook_levels)
            .opt("recent_trades_limit", q.recent_trades_limit)
            .opt("funding_history_limit", q.funding_history_limit)
            .opt("candles_interval", q.candles_interval)
            .opt("candles_limit", q.candles_limit);
        self.query(body, "query market detail").await
    }
}
