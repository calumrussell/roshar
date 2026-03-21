pub(crate) mod rest;

use rest::MarketApi;

pub use rest::{OkxInstrumentsResponse, OkxTickerData, OkxTickersResponse};
pub use rest::MarketApi as OkxMarketApi;

use std::collections::HashMap;

/// OKX client for REST API.
pub struct OkxClient {
    market_api: MarketApi,
}

impl OkxClient {
    /// Default rate limit for REST API requests (10 requests per second)
    const DEFAULT_REQUESTS_PER_SECOND: u32 = 10;

    pub fn new() -> Self {
        Self::new_with_rate_limit(Self::DEFAULT_REQUESTS_PER_SECOND)
    }

    pub fn new_with_rate_limit(requests_per_second: u32) -> Self {
        let market_api = MarketApi::new(requests_per_second);

        Self { market_api }
    }

    /// Get all live SWAP (perpetual) instruments info.
    pub async fn get_instruments_info(
        &self,
    ) -> Result<
        HashMap<String, roshar_types::OkxInstrumentInfo>,
        Box<dyn std::error::Error + Send + Sync>,
    > {
        self.market_api.get_instruments_info().await
    }

    /// Get tickers for all SWAP (perpetual) instruments.
    pub async fn get_tickers(
        &self,
    ) -> Result<HashMap<String, OkxTickerData>, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api.get_tickers().await
    }

    /// Fetch candlestick data for an instrument.
    ///
    /// Rate limit: 40 requests per 2 seconds (IP-based).
    pub async fn get_candles(
        &self,
        inst_id: &str,
        bar: &str,
        after: Option<i64>,
        before: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<roshar_types::OkxCandle>, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api
            .get_candles(inst_id, bar, after, before, limit)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_okx_client_instruments() {
        let market_api = MarketApi::new(10);
        let result = market_api.get_instruments_info().await;

        assert!(
            result.is_ok(),
            "Failed to fetch OKX instruments: {:?}",
            result.err()
        );

        let instruments = result.unwrap();
        assert!(
            !instruments.is_empty(),
            "Expected some instruments, got none"
        );

        println!("OKX client fetched {} SWAP instruments", instruments.len());
    }
}
