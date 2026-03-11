pub(crate) mod rest;

use crate::http::RateLimitedClient;
use rest::{ChartsApi, MarketApi};
pub use rest::{
    KrakenGetLeverageResponse, KrakenLeveragePreference, KrakenLeverageSettingResponse,
    KrakenModifyResponse, KrakenOpenOrdersResponse, KrakenOrder, KrakenOrderResponse,
    KrakenOrderStatusResponse, KrakenRestCandleData, KrakenRestCandleResponse, KrakenTickerData,
    MultiCollateralApi, OrderManagementApi,
};

pub use rest::charts::Candle;
use std::sync::Arc;

/// Kraken client for REST API calls
pub struct KrakenClient {
    charts_api: ChartsApi,
    market_api: MarketApi,
}

impl KrakenClient {
    pub fn new(requests_per_second: u32) -> Self {
        // Create a single shared rate-limited HTTP client for ALL Kraken REST API calls
        let http_client = Arc::new(RateLimitedClient::new(requests_per_second, 1));

        Self {
            charts_api: ChartsApi::new_with_client(http_client.clone()),
            market_api: MarketApi::new_with_client(http_client),
        }
    }

    /// Fetch candles for a symbol directly from the exchange
    /// Returns the most recent completed 1-minute candle
    pub async fn fetch_candles(&self, symbol: &str) -> Result<Vec<Candle>, String> {
        self.charts_api
            .fetch_candle(symbol)
            .await
            .map_err(|e| format!("Failed to fetch candles: {}", e))
    }

    /// Get ticker data for all Kraken futures
    pub async fn get_tickers(
        &self,
    ) -> Result<std::collections::HashMap<String, KrakenTickerData>, String> {
        self.market_api
            .get_tickers()
            .await
            .map_err(|e| format!("Failed to get tickers: {}", e))
    }

    /// Get all funding rates with size data
    /// Returns Vec of (symbol, funding_rate, open_interest_usd, volume_usd)
    pub async fn get_all_funding_rates_with_size(
        &self,
    ) -> Result<Vec<(String, f64, f64, f64)>, String> {
        self.market_api
            .get_all_funding_rates_with_size()
            .await
            .map_err(|e| format!("Failed to get funding rates: {}", e))
    }
}
