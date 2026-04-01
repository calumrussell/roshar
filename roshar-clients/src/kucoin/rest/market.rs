use crate::http::RateLimitedClient;
use roshar_types::{KuCoinContract, KuCoinFundingRate, KuCoinFundingRateListData, KuCoinResponse};
use std::collections::HashMap;
use std::sync::Arc;

const BULLET_PUBLIC_URL: &str =
    "https://api-futures.kucoin.com/api/v1/bullet-public";

/// KuCoin Futures public market data REST client.
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

    /// Fetch a short-lived WebSocket token and the server endpoint URL.
    /// Returns the full WSS URL with the token appended as a query parameter.
    ///
    /// Call this before creating a WebSocket connection and update the client's
    /// ws_config URL via `set_ws_url()`. Tokens expire after a short period.
    ///
    /// Rate limit weight: 10
    pub async fn get_ws_url(&self) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let response = self.client.post(BULLET_PUBLIC_URL).await.send().await?;

        if !response.status().is_success() {
            return Err(format!(
                "KuCoin bullet-public request failed with status: {}",
                response.status()
            )
            .into());
        }

        let text = response.text().await?;
        let parsed: KuCoinResponse<roshar_types::KuCoinWsTokenData> =
            serde_json::from_str(&text)
                .map_err(|e| format!("Failed to parse KuCoin bullet-public response: {e}"))?;

        if parsed.code != "200000" {
            return Err(format!("KuCoin API error {}: {}", parsed.code, parsed.msg).into());
        }

        let server = parsed
            .data
            .instance_servers
            .into_iter()
            .next()
            .ok_or("KuCoin bullet-public returned no instance servers")?;

        Ok(format!("{}?token={}", server.endpoint, parsed.data.token))
    }

    /// Get all active perpetual contracts.
    /// Returns a map of symbol -> contract info.
    ///
    /// Endpoint: GET /api/v1/contracts/active
    /// Rate limit weight: 3
    pub async fn get_contracts(
        &self,
    ) -> Result<HashMap<String, KuCoinContract>, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/api/v1/contracts/active", super::KUCOIN_FUTURES_REST_URL);
        let response = self.client.get(&url).await?;

        if !response.status().is_success() {
            return Err(format!(
                "KuCoin API request failed with status: {}",
                response.status()
            )
            .into());
        }

        let text = response.text().await?;
        let parsed: KuCoinResponse<Vec<KuCoinContract>> = serde_json::from_str(&text)
            .map_err(|e| format!("Failed to parse KuCoin contracts response: {e}"))?;

        if parsed.code != "200000" {
            return Err(format!("KuCoin API error {}: {}", parsed.code, parsed.msg).into());
        }

        let map = parsed
            .data
            .into_iter()
            .filter(|c| c.status == "Open")
            .map(|c| (c.symbol.clone(), c))
            .collect();

        Ok(map)
    }

    /// Get historical funding rates for a symbol within a time range.
    ///
    /// Endpoint: GET /api/v1/contract/funding-rates
    /// Rate limit weight: 5
    ///
    /// Results are returned in ascending order by timepoint. The API returns
    /// up to 1000 records per request; this method paginates automatically.
    pub async fn get_historical_funding_rates(
        &self,
        symbol: &str,
        start_time: i64,
        end_time: i64,
    ) -> Result<Vec<KuCoinFundingRate>, Box<dyn std::error::Error + Send + Sync>> {
        let mut all_rates: Vec<KuCoinFundingRate> = Vec::new();
        let mut current_from = start_time;

        loop {
            let url = format!(
                "{}/api/v1/contract/funding-rates?symbol={}&from={}&to={}",
                super::KUCOIN_FUTURES_REST_URL,
                symbol,
                current_from,
                end_time,
            );

            let response = self.client.get(&url).await?;

            if !response.status().is_success() {
                return Err(format!(
                    "KuCoin API request failed with status: {}",
                    response.status()
                )
                .into());
            }

            let text = response.text().await?;
            let parsed: KuCoinResponse<KuCoinFundingRateListData> =
                serde_json::from_str(&text)
                    .map_err(|e| format!("Failed to parse KuCoin funding rates response: {e}"))?;

            if parsed.code != "200000" {
                return Err(
                    format!("KuCoin API error {}: {}", parsed.code, parsed.msg).into(),
                );
            }

            let batch = parsed.data.data_list;
            let has_more = parsed.data.has_more;

            if batch.is_empty() {
                break;
            }

            let latest_timepoint = batch.iter().map(|r| r.timepoint).max().unwrap_or(0);
            all_rates.extend(batch);

            if !has_more || latest_timepoint >= end_time {
                break;
            }

            // Advance past the last record we received
            current_from = latest_timepoint + 1;
        }

        all_rates.sort_by_key(|r| r.timepoint);
        Ok(all_rates)
    }
}
