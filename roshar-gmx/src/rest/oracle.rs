use super::http::{get_json, ApiResult, Query};
use crate::types::{
    GmxCandlePeriod, GmxOracleCandles, GmxOracleMarkets, GmxOracleMarketsInfo,
    GmxOracleMarketsState, GmxOraclePing, GmxOracleSignedPrices, GmxOracleTicker, GmxOracleTokens,
};
use roshar_http::RateLimitedClient;

/// Oracle API (`https://{chain}-api.gmxinfra.io`) reads. Served for Arbitrum and Avalanche only;
/// errors are `{"error": "..."}` with HTTP 400.
pub struct OracleApi {
    client: RateLimitedClient,
    base_url: String,
}

impl OracleApi {
    pub fn new(base_url: &str, requests_per_second: u32) -> Self {
        Self {
            client: RateLimitedClient::new(requests_per_second, 1),
            base_url: base_url.trim_end_matches('/').to_string(),
        }
    }

    /// `GET /prices/tickers`: latest oracle min/max price of every token.
    pub async fn get_tickers(&self) -> ApiResult<Vec<GmxOracleTicker>> {
        let url = Query::new(&self.base_url, "/prices/tickers").url;
        get_json(&self.client, &url, "oracle tickers").await
    }

    /// `GET /prices/candles?tokenSymbol=..&period=..[&limit=..]`, newest first.
    /// `limit` is 1..=10000 (exchange default 1000).
    pub async fn get_candles(
        &self,
        token_symbol: &str,
        period: GmxCandlePeriod,
        limit: Option<u32>,
    ) -> ApiResult<GmxOracleCandles> {
        let url = Query::new(&self.base_url, "/prices/candles")
            .param("tokenSymbol", token_symbol)
            .param("period", period.as_str())
            .opt("limit", limit)
            .url;
        get_json(&self.client, &url, "oracle candles").await
    }

    fn url(&self, path: &str) -> String {
        Query::new(&self.base_url, path).url
    }

    /// `GET /ping`: health check (database connectivity).
    pub async fn ping(&self) -> ApiResult<GmxOraclePing> {
        get_json(&self.client, &self.url("/ping"), "oracle ping").await
    }

    /// `GET /signed_prices/latest`: latest signed oracle report of every token (1 s cache).
    pub async fn get_signed_prices(&self) -> ApiResult<GmxOracleSignedPrices> {
        get_json(
            &self.client,
            &self.url("/signed_prices/latest"),
            "oracle signed prices",
        )
        .await
    }

    /// `GET /tokens`: tokens configured on the network.
    pub async fn get_tokens(&self) -> ApiResult<GmxOracleTokens> {
        get_json(&self.client, &self.url("/tokens"), "oracle tokens").await
    }

    /// `GET /markets`: market catalog (10 s cache).
    pub async fn get_markets(&self) -> ApiResult<GmxOracleMarkets> {
        get_json(&self.client, &self.url("/markets"), "oracle markets").await
    }

    /// `GET /markets/info`: liquidity, open interest, pool amounts and rates (10 s cache).
    pub async fn get_markets_info(&self) -> ApiResult<GmxOracleMarketsInfo> {
        get_json(
            &self.client,
            &self.url("/markets/info"),
            "oracle markets info",
        )
        .await
    }

    /// `GET /markets/state`: open / closed status of every market (5 s cache).
    pub async fn get_markets_state(&self) -> ApiResult<GmxOracleMarketsState> {
        get_json(
            &self.client,
            &self.url("/markets/state"),
            "oracle markets state",
        )
        .await
    }
}
