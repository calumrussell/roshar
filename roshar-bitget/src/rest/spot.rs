use super::{get_data, BoxError, Query};
use crate::types::{
    BitgetFillsHistoryParams, BitgetMarketTrade, BitgetMergeDepth, BitgetSpotAuction,
    BitgetSpotCandle, BitgetSpotOrderBook, BitgetSpotSymbol, BitgetSpotTicker,
};
use roshar_http::RateLimitedClient;
use std::sync::Arc;

/// Bitget classic (v2) spot public market data REST client.
pub struct SpotMarketApi {
    client: Arc<RateLimitedClient>,
}

impl SpotMarketApi {
    pub fn new(requests_per_second: u32) -> Self {
        Self {
            client: Arc::new(RateLimitedClient::new(requests_per_second, 1)),
        }
    }

    pub fn new_with_client(client: Arc<RateLimitedClient>) -> Self {
        Self { client }
    }

    /// All spot symbols, or one `symbol`.
    ///
    /// Endpoint: GET /api/v2/spot/public/symbols
    /// Rate limit: 20/sec (IP)
    pub async fn get_symbols(
        &self,
        symbol: Option<&str>,
    ) -> Result<Vec<BitgetSpotSymbol>, BoxError> {
        let url = Query::new("/api/v2/spot/public/symbols")
            .opt("symbol", symbol)
            .build();
        get_data(&self.client, &url, "spot symbols").await
    }

    /// All spot tickers, or one `symbol`.
    ///
    /// Endpoint: GET /api/v2/spot/market/tickers
    /// Rate limit: 20/sec (IP)
    pub async fn get_tickers(
        &self,
        symbol: Option<&str>,
    ) -> Result<Vec<BitgetSpotTicker>, BoxError> {
        let url = Query::new("/api/v2/spot/market/tickers")
            .opt("symbol", symbol)
            .build();
        get_data(&self.client, &url, "spot tickers").await
    }

    /// `step`: `step0`..`step5`; `limit` at most 150.
    ///
    /// Endpoint: GET /api/v2/spot/market/orderbook
    /// Rate limit: 20/sec (IP)
    pub async fn get_orderbook(
        &self,
        symbol: &str,
        step: Option<&str>,
        limit: Option<u32>,
    ) -> Result<BitgetSpotOrderBook, BoxError> {
        let url = Query::new("/api/v2/spot/market/orderbook")
            .param("symbol", symbol)
            .opt("type", step)
            .opt("limit", limit)
            .build();
        get_data(&self.client, &url, "spot orderbook").await
    }

    /// `precision`: `scale0`..`scale3`; `limit`: `1`, `5`, `15`, `50` or `max`.
    ///
    /// Endpoint: GET /api/v2/spot/market/merge-depth
    /// Rate limit: 20/sec (IP)
    pub async fn get_merge_depth(
        &self,
        symbol: &str,
        precision: Option<&str>,
        limit: Option<&str>,
    ) -> Result<BitgetMergeDepth, BoxError> {
        let url = Query::new("/api/v2/spot/market/merge-depth")
            .param("symbol", symbol)
            .opt("precision", precision)
            .opt("limit", limit)
            .build();
        get_data(&self.client, &url, "spot merge depth").await
    }

    /// Endpoint: GET /api/v2/spot/market/fills
    /// Rate limit: 10/sec (IP)
    pub async fn get_recent_trades(
        &self,
        symbol: &str,
        limit: Option<u32>,
    ) -> Result<Vec<BitgetMarketTrade>, BoxError> {
        let url = Query::new("/api/v2/spot/market/fills")
            .param("symbol", symbol)
            .opt("limit", limit)
            .build();
        get_data(&self.client, &url, "spot recent trades").await
    }

    /// Trades of the last 90 days, at most 7 days per request (newest first).
    ///
    /// Endpoint: GET /api/v2/spot/market/fills-history
    /// Rate limit: 10/sec (IP)
    pub async fn get_trades_history(
        &self,
        symbol: &str,
        params: &BitgetFillsHistoryParams,
    ) -> Result<Vec<BitgetMarketTrade>, BoxError> {
        let url = Query::new("/api/v2/spot/market/fills-history")
            .param("symbol", symbol)
            .opt("limit", params.limit)
            .opt("idLessThan", params.id_less_than.as_deref())
            .opt("startTime", params.start_time)
            .opt("endTime", params.end_time)
            .build();
        get_data(&self.client, &url, "spot trades history").await
    }

    /// `granularity` is Bitget's spot value (`1min`, `1h`, `1day`, `1Dutc`, ...).
    ///
    /// Endpoint: GET /api/v2/spot/market/candles
    /// Rate limit: 20/sec (IP)
    pub async fn get_candles(
        &self,
        symbol: &str,
        granularity: &str,
        start_time: Option<i64>,
        end_time: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<BitgetSpotCandle>, BoxError> {
        let url = Query::new("/api/v2/spot/market/candles")
            .param("symbol", symbol)
            .param("granularity", granularity)
            .opt("startTime", start_time)
            .opt("endTime", end_time)
            .opt("limit", limit)
            .build();
        get_data(&self.client, &url, "spot candles").await
    }

    /// Candles before `end_time` (required by Bitget).
    ///
    /// Endpoint: GET /api/v2/spot/market/history-candles
    /// Rate limit: 20/sec (IP)
    pub async fn get_history_candles(
        &self,
        symbol: &str,
        granularity: &str,
        end_time: i64,
        limit: Option<u32>,
    ) -> Result<Vec<BitgetSpotCandle>, BoxError> {
        let url = Query::new("/api/v2/spot/market/history-candles")
            .param("symbol", symbol)
            .param("granularity", granularity)
            .param("endTime", end_time)
            .opt("limit", limit)
            .build();
        get_data(&self.client, &url, "spot history candles").await
    }

    /// Endpoint: GET /api/v2/spot/market/auction
    /// Rate limit: 20/sec (IP)
    pub async fn get_auction(&self, symbol: &str) -> Result<BitgetSpotAuction, BoxError> {
        let url = Query::new("/api/v2/spot/market/auction")
            .param("symbol", symbol)
            .build();
        get_data(&self.client, &url, "spot auction").await
    }
}
