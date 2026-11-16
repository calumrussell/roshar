use super::{get_data, BoxError, Query};
use crate::types::{
    BitgetCandle, BitgetCandleParams, BitgetContract, BitgetCurrentFundingRate,
    BitgetFillsHistoryParams, BitgetHistoricalFundingRate, BitgetKLineType, BitgetMarketTrade,
    BitgetMergeDepth, BitgetNextFundingTime, BitgetOiLimit, BitgetOpenInterest, BitgetProductType,
    BitgetServerTime, BitgetSymbolPrice, BitgetTickerData,
};
use roshar_http::RateLimitedClient;
use std::collections::HashMap;
use std::sync::Arc;

/// Bitget classic (v2) futures public market data REST client, plus the shared server time.
pub struct MarketApi {
    client: Arc<RateLimitedClient>,
}

impl MarketApi {
    pub fn new(requests_per_second: u32) -> Self {
        Self {
            client: Arc::new(RateLimitedClient::new(requests_per_second, 1)),
        }
    }

    pub fn new_with_client(client: Arc<RateLimitedClient>) -> Self {
        Self { client }
    }

    /// Endpoint: GET /api/v2/public/time
    pub async fn get_server_time(&self) -> Result<BitgetServerTime, BoxError> {
        let url = Query::new("/api/v2/public/time").build();
        get_data(&self.client, &url, "server time").await
    }

    /// Get contract configs for a product type (all, or one `symbol`), whatever their
    /// `symbolStatus`. Returns a map of symbol -> contract info. Filter on `symbol_status`
    /// (e.g. `"normal"`) to keep only tradable contracts.
    ///
    /// Endpoint: GET /api/v2/mix/market/contracts
    /// Rate limit: 20/sec (IP)
    pub async fn get_contracts(
        &self,
        product_type: BitgetProductType,
        symbol: Option<&str>,
    ) -> Result<HashMap<String, BitgetContract>, BoxError> {
        let url = Query::new("/api/v2/mix/market/contracts")
            .param("productType", product_type.as_str())
            .opt("symbol", symbol)
            .build();
        let data: Vec<BitgetContract> = get_data(&self.client, &url, "contracts").await?;
        Ok(data.into_iter().map(|c| (c.symbol.clone(), c)).collect())
    }

    /// Get current tickers for all contracts of a product type.
    /// Returns a map of symbol -> ticker data (includes current funding rate).
    ///
    /// Endpoint: GET /api/v2/mix/market/tickers
    /// Rate limit: 20/sec (IP)
    pub async fn get_tickers(
        &self,
        product_type: BitgetProductType,
    ) -> Result<HashMap<String, BitgetTickerData>, BoxError> {
        let url = Query::new("/api/v2/mix/market/tickers")
            .param("productType", product_type.as_str())
            .build();
        let data: Vec<BitgetTickerData> = get_data(&self.client, &url, "tickers").await?;
        Ok(data.into_iter().map(|t| (t.symbol.clone(), t)).collect())
    }

    /// Ticker of one contract. Bitget returns it as a list.
    ///
    /// Endpoint: GET /api/v2/mix/market/ticker
    /// Rate limit: 20/sec (IP)
    pub async fn get_ticker(
        &self,
        product_type: BitgetProductType,
        symbol: &str,
    ) -> Result<Vec<BitgetTickerData>, BoxError> {
        let url = Query::new("/api/v2/mix/market/ticker")
            .param("symbol", symbol)
            .param("productType", product_type.as_str())
            .build();
        get_data(&self.client, &url, "ticker").await
    }

    /// Order book merged to a price step.
    /// `precision`: `scale0`..`scale3`; `limit`: `1`, `5`, `15`, `50` or `max`.
    ///
    /// Endpoint: GET /api/v2/mix/market/merge-depth
    /// Rate limit: 20/sec (IP)
    pub async fn get_merge_depth(
        &self,
        product_type: BitgetProductType,
        symbol: &str,
        precision: Option<&str>,
        limit: Option<&str>,
    ) -> Result<BitgetMergeDepth, BoxError> {
        let url = Query::new("/api/v2/mix/market/merge-depth")
            .param("symbol", symbol)
            .param("productType", product_type.as_str())
            .opt("precision", precision)
            .opt("limit", limit)
            .build();
        get_data(&self.client, &url, "merge depth").await
    }

    /// Most recent trades (newest first).
    ///
    /// Endpoint: GET /api/v2/mix/market/fills
    /// Rate limit: 20/sec (IP)
    pub async fn get_recent_trades(
        &self,
        product_type: BitgetProductType,
        symbol: &str,
        limit: Option<u32>,
    ) -> Result<Vec<BitgetMarketTrade>, BoxError> {
        let url = Query::new("/api/v2/mix/market/fills")
            .param("symbol", symbol)
            .param("productType", product_type.as_str())
            .opt("limit", limit)
            .build();
        get_data(&self.client, &url, "recent trades").await
    }

    /// Trades of the last 90 days, at most 7 days per request (newest first).
    ///
    /// Endpoint: GET /api/v2/mix/market/fills-history
    /// Rate limit: 10/sec (IP)
    pub async fn get_trades_history(
        &self,
        product_type: BitgetProductType,
        symbol: &str,
        params: &BitgetFillsHistoryParams,
    ) -> Result<Vec<BitgetMarketTrade>, BoxError> {
        let url = Query::new("/api/v2/mix/market/fills-history")
            .param("symbol", symbol)
            .param("productType", product_type.as_str())
            .opt("limit", params.limit)
            .opt("idLessThan", params.id_less_than.as_deref())
            .opt("startTime", params.start_time)
            .opt("endTime", params.end_time)
            .build();
        get_data(&self.client, &url, "trades history").await
    }

    async fn candles(
        &self,
        path: &str,
        product_type: BitgetProductType,
        symbol: &str,
        granularity: &str,
        kline_type: Option<BitgetKLineType>,
        params: BitgetCandleParams,
    ) -> Result<Vec<BitgetCandle>, BoxError> {
        let url = Query::new(path)
            .param("symbol", symbol)
            .param("productType", product_type.as_str())
            .param("granularity", granularity)
            .opt("kLineType", kline_type.map(|k| k.as_str()))
            .opt("startTime", params.start_time)
            .opt("endTime", params.end_time)
            .opt("limit", params.limit)
            .build();
        get_data(&self.client, &url, "candles").await
    }

    /// Recent candles of traded, mark or index price. `granularity` is Bitget's value
    /// (`1m`, `1H`, `1Dutc`, ...).
    ///
    /// Endpoint: GET /api/v2/mix/market/candles
    /// Rate limit: 20/sec (IP)
    pub async fn get_candles(
        &self,
        product_type: BitgetProductType,
        symbol: &str,
        granularity: &str,
        kline_type: Option<BitgetKLineType>,
        params: BitgetCandleParams,
    ) -> Result<Vec<BitgetCandle>, BoxError> {
        self.candles(
            "/api/v2/mix/market/candles",
            product_type,
            symbol,
            granularity,
            kline_type,
            params,
        )
        .await
    }

    /// Endpoint: GET /api/v2/mix/market/history-candles
    /// Rate limit: 20/sec (IP)
    pub async fn get_history_candles(
        &self,
        product_type: BitgetProductType,
        symbol: &str,
        granularity: &str,
        params: BitgetCandleParams,
    ) -> Result<Vec<BitgetCandle>, BoxError> {
        self.candles(
            "/api/v2/mix/market/history-candles",
            product_type,
            symbol,
            granularity,
            None,
            params,
        )
        .await
    }

    /// Endpoint: GET /api/v2/mix/market/history-index-candles
    /// Rate limit: 20/sec (IP)
    pub async fn get_history_index_candles(
        &self,
        product_type: BitgetProductType,
        symbol: &str,
        granularity: &str,
        params: BitgetCandleParams,
    ) -> Result<Vec<BitgetCandle>, BoxError> {
        self.candles(
            "/api/v2/mix/market/history-index-candles",
            product_type,
            symbol,
            granularity,
            None,
            params,
        )
        .await
    }

    /// Endpoint: GET /api/v2/mix/market/history-mark-candles
    /// Rate limit: 20/sec (IP)
    pub async fn get_history_mark_candles(
        &self,
        product_type: BitgetProductType,
        symbol: &str,
        granularity: &str,
        params: BitgetCandleParams,
    ) -> Result<Vec<BitgetCandle>, BoxError> {
        self.candles(
            "/api/v2/mix/market/history-mark-candles",
            product_type,
            symbol,
            granularity,
            None,
            params,
        )
        .await
    }

    /// Endpoint: GET /api/v2/mix/market/open-interest
    /// Rate limit: 20/sec (IP)
    pub async fn get_open_interest(
        &self,
        product_type: BitgetProductType,
        symbol: &str,
    ) -> Result<BitgetOpenInterest, BoxError> {
        let url = Query::new("/api/v2/mix/market/open-interest")
            .param("symbol", symbol)
            .param("productType", product_type.as_str())
            .build();
        get_data(&self.client, &url, "open interest").await
    }

    /// Current funding rate of every contract, or of one `symbol`.
    ///
    /// Endpoint: GET /api/v2/mix/market/current-fund-rate
    /// Rate limit: 20/sec (IP)
    pub async fn get_current_funding_rates(
        &self,
        product_type: BitgetProductType,
        symbol: Option<&str>,
    ) -> Result<Vec<BitgetCurrentFundingRate>, BoxError> {
        let url = Query::new("/api/v2/mix/market/current-fund-rate")
            .param("productType", product_type.as_str())
            .opt("symbol", symbol)
            .build();
        get_data(&self.client, &url, "current funding rate").await
    }

    /// Get one page of historical funding rates for a symbol.
    ///
    /// Endpoint: GET /api/v2/mix/market/history-fund-rate
    /// Rate limit: 20/sec (IP)
    ///
    /// `page_size` is at most 100 (Bitget's default is 20); `page_no` starts at 1. Bitget pages
    /// newest first; the rates within the returned page are sorted ascending by `fundingTime`.
    pub async fn get_historical_funding_rates(
        &self,
        product_type: BitgetProductType,
        symbol: &str,
        page_size: u32,
        page_no: u32,
    ) -> Result<Vec<BitgetHistoricalFundingRate>, BoxError> {
        let url = Query::new("/api/v2/mix/market/history-fund-rate")
            .param("symbol", symbol)
            .param("productType", product_type.as_str())
            .param("pageSize", page_size)
            .param("pageNo", page_no)
            .build();

        let data: Vec<BitgetHistoricalFundingRate> =
            get_data(&self.client, &url, "funding rates").await?;

        let mut timed = Vec::with_capacity(data.len());
        for rate in data {
            let ts: i64 = rate
                .funding_time
                .parse()
                .map_err(|e| format!("Invalid Bitget fundingTime {:?}: {e}", rate.funding_time))?;
            timed.push((ts, rate));
        }
        timed.sort_by_key(|(ts, _)| *ts);

        Ok(timed.into_iter().map(|(_, rate)| rate).collect())
    }

    /// Endpoint: GET /api/v2/mix/market/funding-time
    /// Rate limit: 20/sec (IP)
    pub async fn get_next_funding_time(
        &self,
        product_type: BitgetProductType,
        symbol: &str,
    ) -> Result<Vec<BitgetNextFundingTime>, BoxError> {
        let url = Query::new("/api/v2/mix/market/funding-time")
            .param("symbol", symbol)
            .param("productType", product_type.as_str())
            .build();
        get_data(&self.client, &url, "funding time").await
    }

    /// Last, index and mark price.
    ///
    /// Endpoint: GET /api/v2/mix/market/symbol-price
    /// Rate limit: 20/sec
    pub async fn get_symbol_price(
        &self,
        product_type: BitgetProductType,
        symbol: &str,
    ) -> Result<Vec<BitgetSymbolPrice>, BoxError> {
        let url = Query::new("/api/v2/mix/market/symbol-price")
            .param("symbol", symbol)
            .param("productType", product_type.as_str())
            .build();
        get_data(&self.client, &url, "symbol price").await
    }

    /// Position notional limits of every contract, or of one `symbol`.
    ///
    /// Endpoint: GET /api/v2/mix/market/oi-limit
    /// Rate limit: 10/sec (IP)
    pub async fn get_oi_limits(
        &self,
        product_type: BitgetProductType,
        symbol: Option<&str>,
    ) -> Result<Vec<BitgetOiLimit>, BoxError> {
        let url = Query::new("/api/v2/mix/market/oi-limit")
            .param("productType", product_type.as_str())
            .opt("symbol", symbol)
            .build();
        get_data(&self.client, &url, "OI limit").await
    }
}
