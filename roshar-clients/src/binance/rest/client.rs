use crate::http::RateLimitedClient;
use roshar_types::{
    BinanceHistoricalFundingRate, BinanceOrderBookSnapshot, BinancePremiumIndex, ExchangeInfo,
    OpenInterestData, TickerData,
};

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

    /// Fetch real-time premium index data for all perpetual contracts
    ///
    /// Returns mark price, index price, and funding rate information.
    /// This endpoint provides funding rates for ALL symbols in a single call.
    pub async fn get_premium_index(
        &self,
    ) -> Result<Vec<BinancePremiumIndex>, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/fapi/v1/premiumIndex", BASE_URL);
        let response = self.get(&url).await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("Premium index endpoint failed (status: {status}): {body}").into());
        }

        let response_text = response.text().await?;
        let premium_index: Vec<BinancePremiumIndex> = serde_json::from_str(&response_text)
            .map_err(|e| format!("Failed to parse Binance premium index response: {e}"))?;

        Ok(premium_index)
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
