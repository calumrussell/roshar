pub(crate) mod rest;

use rest::BinanceRestClient;

use roshar_types::BinancePremiumIndex;
use std::collections::HashMap;

/// Binance client for REST API calls
pub struct BinanceClient {
    rest_client: BinanceRestClient,
}

impl BinanceClient {
    /// Create a new Binance client.
    ///
    /// # Arguments
    /// * `requests_per_second` - Maximum REST API requests per second
    pub fn new(requests_per_second: u32) -> Self {
        Self {
            rest_client: BinanceRestClient::new(requests_per_second),
        }
    }

    /// Get 24hr ticker data for all symbols or a specific symbol
    pub async fn get_24hr_ticker(
        &self,
        symbol: Option<&str>,
    ) -> Result<Vec<roshar_types::TickerData>, String> {
        self.rest_client
            .get_24hr_ticker(symbol)
            .await
            .map_err(|e| format!("Failed to get 24hr ticker: {}", e))
    }

    /// Get historical funding rates for a symbol
    ///
    /// Handles pagination internally - returns all funding rates in the time range.
    /// If rate limiting is configured (via `with_rate_limit`), each page request
    /// respects the rate limit.
    pub async fn get_historical_funding_rates(
        &self,
        symbol: &str,
        start_time: i64,
        end_time: i64,
    ) -> Result<Vec<roshar_types::BinanceHistoricalFundingRate>, String> {
        self.rest_client
            .get_historical_funding_rates(symbol, start_time, end_time)
            .await
            .map_err(|e| format!("Failed to get historical funding rates: {}", e))
    }

    /// Get exchange info including all available symbols
    pub async fn get_exchange_info(&self) -> Result<roshar_types::ExchangeInfo, String> {
        self.rest_client
            .get_exchange_info()
            .await
            .map_err(|e| format!("Failed to get exchange info: {}", e))
    }

    /// Get aggregate trades for a symbol
    ///
    /// # Arguments
    /// * `symbol` - Trading pair symbol (e.g., "BTCUSDT")
    /// * `start_time` - Optional start time in milliseconds
    /// * `end_time` - Optional end time in milliseconds
    /// * `from_id` - Optional aggregate trade ID to fetch from
    /// * `limit` - Optional number of results (max 1000)
    pub async fn get_agg_trades(
        &self,
        symbol: &str,
        start_time: Option<i64>,
        end_time: Option<i64>,
        from_id: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<roshar_types::BinanceAggTrade>, String> {
        self.rest_client
            .get_agg_trades(symbol, start_time, end_time, from_id, limit)
            .await
            .map_err(|e| format!("Failed to get aggregate trades: {}", e))
    }

    /// Get klines (candlestick) data for a symbol
    pub async fn get_klines(
        &self,
        symbol: &str,
        interval: &str,
        start_time: Option<i64>,
        end_time: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<roshar_types::BinanceKline>, String> {
        self.rest_client
            .get_klines(symbol, interval, start_time, end_time, limit)
            .await
            .map_err(|e| format!("Failed to get klines: {}", e))
    }

    /// Get real-time funding rates for all perpetual contracts
    ///
    /// Returns a HashMap keyed by symbol for easy lookup of funding rate data.
    /// Each entry contains mark price, index price, current funding rate, and next funding time.
    pub async fn get_realtime_funding_rates(
        &self,
    ) -> Result<HashMap<String, BinancePremiumIndex>, String> {
        let premium_index_list = self
            .rest_client
            .get_premium_index()
            .await
            .map_err(|e| format!("Failed to get premium index: {}", e))?;

        let mut funding_rates = HashMap::new();
        for premium_index in premium_index_list {
            funding_rates.insert(premium_index.symbol.clone(), premium_index);
        }

        Ok(funding_rates)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_realtime_funding_rates() {
        let client = BinanceClient::new(10);

        let result = client.get_realtime_funding_rates().await;

        assert!(
            result.is_ok(),
            "Failed to fetch realtime funding rates: {:?}",
            result.err()
        );

        let funding_rates = result.unwrap();

        // Binance has many perpetual contracts, should have data
        assert!(
            !funding_rates.is_empty(),
            "Expected funding rates data, got empty HashMap"
        );

        // Check that BTCUSDT exists (it's always available)
        assert!(
            funding_rates.contains_key("BTCUSDT"),
            "Expected BTCUSDT in funding rates"
        );

        // Verify the structure of a funding rate entry
        if let Some(btc_data) = funding_rates.get("BTCUSDT") {
            assert_eq!(btc_data.symbol, "BTCUSDT");
            // Mark price should be parseable as a float
            btc_data
                .mark_price
                .parse::<f64>()
                .expect("mark_price should be a valid number");
            // Funding rate should be parseable as a float
            btc_data
                .last_funding_rate
                .parse::<f64>()
                .expect("last_funding_rate should be a valid number");
            // Next funding time should be in the future (or very recent past)
            assert!(
                btc_data.next_funding_time > 0,
                "next_funding_time should be positive"
            );
        }

        println!(
            "Fetched {} funding rates (lookup working)",
            funding_rates.len()
        );
    }
}
