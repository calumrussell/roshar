pub(crate) mod rest;

use rest::MarketApi;

pub use rest::{
    ByBitCreateOrderRequest, ByBitCreateOrderResponse, ByBitInstrumentInfo, ByBitLeverageFilter,
    ByBitLotSizeFilter, ByBitOrderResult, ByBitPriceFilter, ByBitTickerData, ByBitTickersResponse,
    MarketApi as ByBitMarketApi,
};
pub use roshar_types::ByBitHistoricalFundingRate;

use std::collections::HashMap;

/// ByBit client for REST API
pub struct ByBitClient {
    market_api: MarketApi,
}

impl ByBitClient {
    /// Default rate limit for REST API requests (10 requests per second)
    const DEFAULT_REQUESTS_PER_SECOND: u32 = 10;

    pub fn new() -> Self {
        Self::new_with_rate_limit(Self::DEFAULT_REQUESTS_PER_SECOND)
    }

    pub fn new_with_rate_limit(requests_per_second: u32) -> Self {
        let market_api = MarketApi::new(requests_per_second);

        Self { market_api }
    }

    /// Get real-time funding rates for all linear perpetual symbols
    ///
    /// Returns ticker data containing funding_rate and next_funding_time for each symbol.
    /// The data is fetched from ByBit's tickers endpoint which provides real-time funding rates.
    pub async fn get_realtime_funding_rates(
        &self,
    ) -> Result<HashMap<String, ByBitTickerData>, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api.get_tickers().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_realtime_funding_rates() {
        let market_api = MarketApi::new(10);
        let result = market_api.get_tickers().await;

        assert!(
            result.is_ok(),
            "Failed to fetch realtime funding rates: {:?}",
            result.err()
        );

        let tickers = result.unwrap();

        // Should have some tickers
        assert!(!tickers.is_empty(), "Expected some tickers, got none");

        // Check that BTCUSDT is present (a known perpetual)
        assert!(
            tickers.contains_key("BTCUSDT"),
            "Expected BTCUSDT ticker to be present"
        );

        // Verify funding rate data is present
        let btc_ticker = tickers.get("BTCUSDT").unwrap();
        assert!(
            !btc_ticker.funding_rate.is_empty(),
            "Expected funding_rate to be present"
        );
        assert!(
            !btc_ticker.next_funding_time.is_empty(),
            "Expected next_funding_time to be present"
        );

        println!(
            "Fetched {} tickers with funding rates. BTCUSDT funding_rate: {}, next_funding_time: {}",
            tickers.len(),
            btc_ticker.funding_rate,
            btc_ticker.next_funding_time
        );
    }
}
