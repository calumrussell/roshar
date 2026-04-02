use crate::http::RateLimitedClient;
use roshar_types::{BitgetContract, BitgetHistoricalFundingRate, BitgetResponse, BitgetTickerData};
use std::collections::HashMap;
use std::sync::Arc;

/// Bitget USDT-margined perpetuals public market data REST client.
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

    /// Get all USDT-margined perpetual contract configs.
    /// Returns a map of symbol -> contract info.
    ///
    /// Endpoint: GET /api/v2/mix/market/contracts?productType=USDT-FUTURES
    /// Rate limit: 20/sec (IP)
    pub async fn get_contracts(
        &self,
    ) -> Result<HashMap<String, BitgetContract>, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!(
            "{}/api/v2/mix/market/contracts?productType=USDT-FUTURES",
            super::BITGET_REST_URL
        );

        let response = self.client.get(&url).await?;

        if !response.status().is_success() {
            return Err(format!(
                "Bitget API request failed with status: {}",
                response.status()
            )
            .into());
        }

        let text = response.text().await?;
        let parsed: BitgetResponse<Vec<BitgetContract>> = serde_json::from_str(&text)
            .map_err(|e| format!("Failed to parse Bitget contracts response: {e}"))?;

        if parsed.code != "00000" {
            return Err(format!("Bitget API error {}: {}", parsed.code, parsed.msg).into());
        }

        let map = parsed
            .data
            .into_iter()
            .filter(|c| c.symbol_status == "normal")
            .map(|c| (c.symbol.clone(), c))
            .collect();

        Ok(map)
    }

    /// Get current tickers for all USDT-margined perpetuals.
    /// Returns a map of symbol -> ticker data (includes current funding rate).
    ///
    /// Endpoint: GET /api/v2/mix/market/tickers?productType=USDT-FUTURES
    /// Rate limit: 20/sec (IP)
    pub async fn get_tickers(
        &self,
    ) -> Result<HashMap<String, BitgetTickerData>, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!(
            "{}/api/v2/mix/market/tickers?productType=USDT-FUTURES",
            super::BITGET_REST_URL
        );

        let response = self.client.get(&url).await?;

        if !response.status().is_success() {
            return Err(format!(
                "Bitget API request failed with status: {}",
                response.status()
            )
            .into());
        }

        let text = response.text().await?;
        let parsed: BitgetResponse<Vec<BitgetTickerData>> = serde_json::from_str(&text)
            .map_err(|e| format!("Failed to parse Bitget tickers response: {e}"))?;

        if parsed.code != "00000" {
            return Err(format!("Bitget API error {}: {}", parsed.code, parsed.msg).into());
        }

        let map = parsed
            .data
            .into_iter()
            .map(|t| (t.symbol.clone(), t))
            .collect();

        Ok(map)
    }

    /// Get historical funding rates for a symbol.
    ///
    /// Endpoint: GET /api/v2/mix/market/history-fund-rate
    /// Rate limit: 20/sec (IP)
    ///
    /// Results are returned in ascending order by settle time.
    /// Uses page-based pagination internally (pageSize=100).
    pub async fn get_historical_funding_rates(
        &self,
        symbol: &str,
        page_size: u32,
        page_no: u32,
    ) -> Result<Vec<BitgetHistoricalFundingRate>, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!(
            "{}/api/v2/mix/market/history-fund-rate?symbol={}&productType=usdt-futures&pageSize={}&pageNo={}",
            super::BITGET_REST_URL,
            symbol,
            page_size,
            page_no,
        );

        let response = self.client.get(&url).await?;

        if !response.status().is_success() {
            return Err(format!(
                "Bitget API request failed with status: {}",
                response.status()
            )
            .into());
        }

        let text = response.text().await?;
        let parsed: BitgetResponse<Vec<BitgetHistoricalFundingRate>> =
            serde_json::from_str(&text)
                .map_err(|e| format!("Failed to parse Bitget funding rates response: {e}"))?;

        if parsed.code != "00000" {
            return Err(format!("Bitget API error {}: {}", parsed.code, parsed.msg).into());
        }

        let mut rates = parsed.data;
        rates.sort_by(|a, b| {
            let ta: i64 = a.settle_time.parse().unwrap_or(0);
            let tb: i64 = b.settle_time.parse().unwrap_or(0);
            ta.cmp(&tb)
        });

        Ok(rates)
    }
}
