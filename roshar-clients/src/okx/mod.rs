pub(crate) mod rest;

use rest::MarketApi;
use std::collections::HashMap;

pub use rest::{OkxInstrumentsResponse, OkxTickerData, OkxTickersResponse};
pub use rest::MarketApi as OkxMarketApi;

/// OKX client for REST API access (MVP - no WebSocket support yet).
///
/// Focuses on perpetual swap (SWAP) instruments.
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_okx_client_instruments() {
        let client = OkxClient::new();
        let result = client.get_instruments_info().await;

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
