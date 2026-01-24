use crate::http::RateLimitedClient;
use roshar_types::{
    BinanceHistoricalFundingRate, BinanceOrderBookSnapshot, ExchangeInfo, OpenInterestData,
    TickerData,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Premium index data from Binance /fapi/v1/premiumIndex endpoint
/// Contains mark price and funding rate information for perpetual futures
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BinancePremiumIndexData {
    pub symbol: String,
    pub mark_price: String,
    pub last_funding_rate: String,
    pub next_funding_time: i64,
    pub interest_rate: String,
}

const BASE_URL: &str = "https://fapi.binance.com";

/// Binance Futures REST API client
pub struct BinanceRestClient {
    client: RateLimitedClient,
}

impl BinanceRestClient {
    /// Create a new client with rate limiting.
    ///
    /// # Arguments
    /// * `requests_per_second` - Maximum requests per second
    pub fn new(requests_per_second: u32) -> Self {
        Self {
            client: RateLimitedClient::new(requests_per_second, 1),
        }
    }

    /// Make a GET request, respecting rate limits.
    async fn get(&self, url: &str) -> Result<reqwest::Response, reqwest::Error> {
        self.client.get(url).await
    }

    pub async fn get_exchange_info(
        &self,
    ) -> Result<ExchangeInfo, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/fapi/v1/exchangeInfo", BASE_URL);
        let response = self.get(&url).await?;

        if !response.status().is_success() {
            return Err(format!("Binance API error: HTTP {}", response.status()).into());
        }

        let response_text = response.text().await?;
        let exchange_info: ExchangeInfo = serde_json::from_str(&response_text)
            .map_err(|e| format!("Failed to parse Binance exchange info response: {e}"))?;

        Ok(exchange_info)
    }

    pub async fn get_open_interest(
        &self,
        symbol: &str,
    ) -> Result<OpenInterestData, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/fapi/v1/openInterest?symbol={}", BASE_URL, symbol);
        let response = self.get(&url).await?;

        if !response.status().is_success() {
            return Err(format!("Binance API error: HTTP {}", response.status()).into());
        }

        let response_text = response.text().await?;
        let open_interest_data: OpenInterestData = serde_json::from_str(&response_text)
            .map_err(|e| format!("Failed to parse Binance open interest response: {e}"))?;

        Ok(open_interest_data)
    }

    pub async fn get_24hr_ticker(
        &self,
        symbol: Option<&str>,
    ) -> Result<Vec<TickerData>, Box<dyn std::error::Error + Send + Sync>> {
        let mut url = format!("{}/fapi/v1/ticker/24hr", BASE_URL);

        if let Some(symbol) = symbol {
            url.push_str(&format!("?symbol={symbol}"));
        }

        let response = self.get(&url).await?;

        if !response.status().is_success() {
            return Err(format!("Binance API error: HTTP {}", response.status()).into());
        }

        let response_text = response.text().await?;

        if symbol.is_some() {
            // Single symbol response
            let ticker_data: TickerData = serde_json::from_str(&response_text)
                .map_err(|e| format!("Failed to parse Binance 24hr ticker response: {e}"))?;
            Ok(vec![ticker_data])
        } else {
            // Multiple symbols response
            let ticker_data: Vec<TickerData> = serde_json::from_str(&response_text)
                .map_err(|e| format!("Failed to parse Binance 24hr ticker response: {e}"))?;
            Ok(ticker_data)
        }
    }

    pub async fn get_depth_snapshot(
        &self,
        symbol: &str,
        limit: Option<u32>,
    ) -> Result<BinanceOrderBookSnapshot, Box<dyn std::error::Error + Send + Sync>> {
        let limit = limit.unwrap_or(1000);
        let url = format!(
            "{}/fapi/v1/depth?symbol={}&limit={}",
            BASE_URL, symbol, limit
        );
        let response = self.get(&url).await?;

        if !response.status().is_success() {
            return Err(format!("Binance API error: HTTP {}", response.status()).into());
        }

        let response_text = response.text().await?;
        let snapshot: BinanceOrderBookSnapshot = serde_json::from_str(&response_text)
            .map_err(|e| format!("Failed to parse Binance depth snapshot response: {e}"))?;

        Ok(snapshot)
    }

    /// Fetch premium index data for all symbols
    ///
    /// Returns mark price and funding rate information for all perpetual futures contracts.
    /// Uses the /fapi/v1/premiumIndex endpoint.
    pub async fn get_all_funding_rates(
        &self,
    ) -> Result<Vec<BinancePremiumIndexData>, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/fapi/v1/premiumIndex", BASE_URL);
        let response = self.get(&url).await?;

        if !response.status().is_success() {
            return Err(format!("Binance API error: HTTP {}", response.status()).into());
        }

        let response_text = response.text().await?;
        let premium_index_data: Vec<BinancePremiumIndexData> = serde_json::from_str(&response_text)
            .map_err(|e| format!("Failed to parse Binance premium index response: {e}"))?;

        Ok(premium_index_data)
    }

    /// Fetch funding rates combined with size information (open interest and volume)
    ///
    /// Returns a vector of tuples: (symbol, funding_rate, open_interest_usd, volume_usd)
    /// Combines data from /fapi/v1/premiumIndex and /fapi/v1/ticker/24hr endpoints.
    pub async fn get_all_funding_rates_with_size(
        &self,
    ) -> Result<Vec<(String, f64, f64, f64)>, Box<dyn std::error::Error + Send + Sync>> {
        // Fetch both datasets concurrently
        let premium_index_data = self.get_all_funding_rates().await?;
        let ticker_data = self.get_24hr_ticker(None).await?;

        // Create a map of ticker data for quick lookup
        let ticker_map: HashMap<String, &TickerData> = ticker_data
            .iter()
            .map(|t| (t.symbol.clone(), t))
            .collect();

        let mut results = Vec::new();

        for premium in premium_index_data {
            let funding_rate: f64 = premium.last_funding_rate.parse().unwrap_or(0.0);

            // Get volume from ticker data if available
            let (open_interest, volume) = if let Some(ticker) = ticker_map.get(&premium.symbol) {
                let quote_volume: f64 = ticker.quote_volume.parse().unwrap_or(0.0);

                // For Binance Futures, we need to fetch open interest separately or estimate
                // The ticker doesn't include OI, so we'll use quote_volume as volume in USD
                // Open interest would require additional API call per symbol, which is expensive
                // For now, we set OI to 0 and volume to quote_volume (which is already in USDT)
                (0.0, quote_volume)
            } else {
                (0.0, 0.0)
            };

            // Only include symbols with non-zero funding rate or volume
            if funding_rate != 0.0 || volume > 0.0 {
                results.push((premium.symbol, funding_rate, open_interest, volume));
            }
        }

        Ok(results)
    }

    /// Fetch historical funding rates for a symbol
    ///
    /// Handles pagination internally - returns all funding rates in the time range.
    /// If rate limiting is configured, each page request respects the rate limit.
    ///
    /// # Arguments
    /// * `symbol` - Trading pair symbol (e.g., "BTCUSDT")
    /// * `start_time` - Start time in milliseconds
    /// * `end_time` - End time in milliseconds
    pub async fn get_historical_funding_rates(
        &self,
        symbol: &str,
        start_time: i64,
        end_time: i64,
    ) -> Result<Vec<BinanceHistoricalFundingRate>, Box<dyn std::error::Error + Send + Sync>> {
        let mut all_rates = Vec::new();
        let mut current_start = start_time;
        const LIMIT: u32 = 1000;

        loop {
            let url = format!(
                "{}/fapi/v1/fundingRate?symbol={}&startTime={}&endTime={}&limit={}",
                BASE_URL, symbol, current_start, end_time, LIMIT
            );

            let response = self.get(&url).await?;

            if !response.status().is_success() {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                return Err(format!(
                    "Historical funding rates endpoint failed (status: {status}): {body}"
                )
                .into());
            }

            let response_text = response.text().await?;
            let rates: Vec<BinanceHistoricalFundingRate> = serde_json::from_str(&response_text)
                .map_err(|e| format!("Failed to parse Binance funding rates response: {e}"))?;

            if rates.is_empty() {
                break;
            }

            let last_time = rates.last().unwrap().funding_time;
            let batch_size = rates.len();
            all_rates.extend(rates);

            // If we got less than limit, we've reached the end
            if batch_size < LIMIT as usize || last_time >= end_time {
                break;
            }

            current_start = last_time + 1;
        }

        Ok(all_rates)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_all_funding_rates() {
        let client = BinanceRestClient::new(10);
        let result = client.get_all_funding_rates().await;

        assert!(
            result.is_ok(),
            "Failed to fetch premium index data: {:?}",
            result.err()
        );

        let rates = result.unwrap();

        // Should have multiple symbols
        assert!(!rates.is_empty(), "Expected at least some symbols");

        // Verify data structure
        for rate in rates.iter().take(5) {
            // Symbol should not be empty
            assert!(!rate.symbol.is_empty(), "Symbol should not be empty");

            // Mark price should be parseable as f64
            let mark_price: Result<f64, _> = rate.mark_price.parse();
            assert!(
                mark_price.is_ok(),
                "Mark price should be parseable: {}",
                rate.mark_price
            );

            // Funding rate should be parseable as f64
            let funding_rate: Result<f64, _> = rate.last_funding_rate.parse();
            assert!(
                funding_rate.is_ok(),
                "Funding rate should be parseable: {}",
                rate.last_funding_rate
            );
        }

        println!("Fetched {} premium index entries", rates.len());
    }

    #[tokio::test]
    async fn test_get_all_funding_rates_with_size() {
        let client = BinanceRestClient::new(10);
        let result = client.get_all_funding_rates_with_size().await;

        assert!(
            result.is_ok(),
            "Failed to fetch funding rates with size: {:?}",
            result.err()
        );

        let rates = result.unwrap();

        // Should have multiple symbols
        assert!(!rates.is_empty(), "Expected at least some symbols");

        // Verify data structure - each tuple should be (symbol, funding_rate, oi, volume)
        for (symbol, funding_rate, open_interest, volume) in rates.iter().take(5) {
            // Symbol should not be empty
            assert!(!symbol.is_empty(), "Symbol should not be empty");

            // Funding rate should be a valid number
            assert!(
                funding_rate.is_finite(),
                "Funding rate should be finite: {}",
                funding_rate
            );

            // Open interest and volume should be non-negative
            assert!(
                *open_interest >= 0.0,
                "Open interest should be non-negative"
            );
            assert!(*volume >= 0.0, "Volume should be non-negative");
        }

        // Find BTCUSDT and verify it has volume
        let btc_entry = rates.iter().find(|(s, _, _, _)| s == "BTCUSDT");
        assert!(
            btc_entry.is_some(),
            "BTCUSDT should be present in the results"
        );
        if let Some((_symbol, funding_rate, _oi, volume)) = btc_entry {
            println!(
                "BTCUSDT: funding_rate={}, volume={}",
                funding_rate, volume
            );
            assert!(*volume > 0.0, "BTCUSDT should have positive volume");
        }

        println!("Fetched {} funding rates with size data", rates.len());
    }

    #[tokio::test]
    async fn test_get_historical_funding_rates_pagination() {
        // Binance funding rates are every 8 hours, so 3 per day
        // 1000 limit / 3 per day = ~333 days in one page
        // Fetch 2 years of data to ensure we need multiple pages
        let end_time = chrono::Utc::now().timestamp_millis();
        let start_time = end_time - (730 * 24 * 60 * 60 * 1000); // 2 years ago

        let client = BinanceRestClient::new(10);
        let result = client
            .get_historical_funding_rates("BTCUSDT", start_time, end_time)
            .await;

        assert!(
            result.is_ok(),
            "Failed to fetch funding rates: {:?}",
            result.err()
        );

        let rates = result.unwrap();

        // Should have more than 1000 results (proving pagination worked)
        // 2 years * 365 days * 3 per day = ~2190 funding rates
        assert!(
            rates.len() > 1000,
            "Expected more than 1000 rates (pagination), got {}",
            rates.len()
        );

        // Verify chronological order
        for window in rates.windows(2) {
            assert!(
                window[0].funding_time <= window[1].funding_time,
                "Rates not in chronological order"
            );
        }

        println!("Fetched {} funding rates (pagination working)", rates.len());
    }
}
