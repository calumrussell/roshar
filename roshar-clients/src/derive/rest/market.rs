use crate::http::RateLimitedClient;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

// JSON-RPC request/response wrappers

#[derive(Debug, Serialize)]
struct JsonRpcRequest<T: Serialize> {
    method: String,
    params: T,
    id: u64,
}

#[derive(Debug, Deserialize)]
struct JsonRpcResponse<T> {
    #[allow(dead_code)]
    id: u64,
    result: T,
}

// get_instruments types

#[derive(Debug, Serialize)]
struct GetInstrumentsParams {
    currency: String,
    instrument_type: String,
    expired: bool,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeriveInstrument {
    pub instrument_name: String,
    pub instrument_type: String,
    pub base_currency: String,
    pub is_active: bool,
    pub tick_size: String,
    pub amount_step: String,
    pub minimum_amount: String,
    pub maximum_amount: String,
    // Option-specific fields
    #[serde(default)]
    pub option_type: Option<String>,
    #[serde(default)]
    pub strike: Option<String>,
    #[serde(default)]
    pub expiry: Option<i64>,
    // Perp-specific fields
    #[serde(default)]
    pub funding_rate: Option<String>,
    #[serde(default)]
    pub aggregate_funding: Option<String>,
    #[serde(default)]
    pub index: Option<String>,
}

pub type DeriveInstrumentsResponse = Vec<DeriveInstrument>;

// get_funding_rate_history types

#[derive(Debug, Serialize)]
struct GetFundingRateHistoryParams {
    instrument_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    start_timestamp: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    end_timestamp: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    period: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeriveFundingRate {
    pub funding_rate: String,
    pub timestamp: i64,
}

#[derive(Debug, Deserialize)]
pub struct DeriveFundingRateHistoryResponse {
    pub funding_rate_history: Vec<DeriveFundingRate>,
}

// get_tickers types

#[derive(Debug, Serialize)]
struct GetTickersParams {
    instrument_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    currency: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeriveStats {
    #[serde(default)]
    pub volume: Option<String>,
    #[serde(default)]
    pub high: Option<String>,
    #[serde(default)]
    pub low: Option<String>,
    #[serde(default)]
    pub trade_count: Option<i64>,
    #[serde(default)]
    pub open_interest: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeriveOptionPricing {
    #[serde(default)]
    pub delta: Option<String>,
    #[serde(default)]
    pub gamma: Option<String>,
    #[serde(default)]
    pub theta: Option<String>,
    #[serde(default)]
    pub vega: Option<String>,
    #[serde(default)]
    pub iv: Option<String>,
    #[serde(default)]
    pub mark_price: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DerivePerpDetails {
    #[serde(default)]
    pub funding_rate: Option<String>,
    #[serde(default)]
    pub interest_rate: Option<String>,
    #[serde(default)]
    pub aggregate_funding: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeriveTicker {
    pub instrument_name: String,
    #[serde(default)]
    pub best_bid_price: Option<String>,
    #[serde(default)]
    pub best_ask_price: Option<String>,
    #[serde(default)]
    pub best_bid_amount: Option<String>,
    #[serde(default)]
    pub best_ask_amount: Option<String>,
    #[serde(default)]
    pub index_price: Option<String>,
    #[serde(default)]
    pub mark_price: Option<String>,
    #[serde(default)]
    pub stats: Option<DeriveStats>,
    #[serde(default)]
    pub timestamp: Option<i64>,
    // Option-specific
    #[serde(default)]
    pub option_pricing: Option<DeriveOptionPricing>,
    #[serde(default)]
    pub option_details: Option<serde_json::Value>,
    // Perp-specific
    #[serde(default)]
    pub perp_details: Option<DerivePerpDetails>,
}

#[derive(Debug, Deserialize)]
pub struct DeriveTickersResult {
    pub tickers: HashMap<String, DeriveTicker>,
}

pub type DeriveTickersResponse = HashMap<String, DeriveTicker>;

/// Derive (formerly Lyra) Market API using JSON-RPC over HTTP
pub struct MarketApi {
    client: Arc<RateLimitedClient>,
}

impl MarketApi {
    pub fn new(requests_per_second: u32) -> Self {
        Self {
            client: Arc::new(RateLimitedClient::new(requests_per_second, 1)),
        }
    }

    /// Get instruments for a given currency and type.
    pub async fn get_instruments(
        &self,
        currency: &str,
        instrument_type: &str,
        expired: bool,
    ) -> Result<Vec<DeriveInstrument>, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/public/get_instruments", super::DERIVE_REST_URL);

        let request = JsonRpcRequest {
            method: "public/get_instruments".to_string(),
            params: GetInstrumentsParams {
                currency: currency.to_string(),
                instrument_type: instrument_type.to_string(),
                expired,
            },
            id: 1,
        };

        let response = self
            .client
            .post(&url)
            .await
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(
                format!("Derive API request failed with status: {}", response.status()).into(),
            );
        }

        let response_text = response.text().await?;
        let rpc_response: JsonRpcResponse<DeriveInstrumentsResponse> =
            serde_json::from_str(&response_text)
                .map_err(|e| format!("Failed to parse Derive instruments response: {e}"))?;

        Ok(rpc_response.result)
    }

    /// Get funding rate history for a perpetual instrument.
    ///
    /// API limits lookback to 30 days maximum.
    pub async fn get_funding_rate_history(
        &self,
        instrument_name: &str,
        start_timestamp: Option<i64>,
        end_timestamp: Option<i64>,
        period: Option<&str>,
    ) -> Result<Vec<DeriveFundingRate>, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!(
            "{}/public/get_funding_rate_history",
            super::DERIVE_REST_URL
        );

        let request = JsonRpcRequest {
            method: "public/get_funding_rate_history".to_string(),
            params: GetFundingRateHistoryParams {
                instrument_name: instrument_name.to_string(),
                start_timestamp,
                end_timestamp,
                period: period.map(|p| p.to_string()),
            },
            id: 1,
        };

        let response = self
            .client
            .post(&url)
            .await
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(format!(
                "Derive API request failed with status: {}",
                response.status()
            )
            .into());
        }

        let response_text = response.text().await?;
        let rpc_response: JsonRpcResponse<DeriveFundingRateHistoryResponse> =
            serde_json::from_str(&response_text).map_err(|e| {
                format!("Failed to parse Derive funding rate history response: {e}")
            })?;

        Ok(rpc_response.result.funding_rate_history)
    }

    /// Get tickers for a given instrument type.
    ///
    /// `currency` is optional but required for options.
    pub async fn get_tickers(
        &self,
        instrument_type: &str,
        currency: Option<&str>,
    ) -> Result<HashMap<String, DeriveTicker>, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/public/get_tickers", super::DERIVE_REST_URL);

        let request = JsonRpcRequest {
            method: "public/get_tickers".to_string(),
            params: GetTickersParams {
                instrument_type: instrument_type.to_string(),
                currency: currency.map(|c| c.to_string()),
            },
            id: 1,
        };

        let response = self
            .client
            .post(&url)
            .await
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(
                format!("Derive API request failed with status: {}", response.status()).into(),
            );
        }

        let response_text = response.text().await?;
        let rpc_response: JsonRpcResponse<DeriveTickersResult> =
            serde_json::from_str(&response_text)
                .map_err(|e| format!("Failed to parse Derive tickers response: {e}"))?;

        Ok(rpc_response.result.tickers)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_instruments_perp() {
        let api = MarketApi::new(10);
        let result = api.get_instruments("ETH", "perp", false).await;

        assert!(
            result.is_ok(),
            "Failed to fetch Derive instruments: {:?}",
            result.err()
        );

        let instruments = result.unwrap();
        assert!(
            !instruments.is_empty(),
            "Expected some perp instruments, got none"
        );

        println!(
            "Fetched {} Derive ETH perp instruments. First: {}",
            instruments.len(),
            instruments[0].instrument_name
        );
    }

    #[tokio::test]
    async fn test_get_funding_rate_history() {
        let api = MarketApi::new(10);
        let result = api
            .get_funding_rate_history("ETH-PERP", None, None, None)
            .await;

        assert!(
            result.is_ok(),
            "Failed to fetch Derive funding rate history: {:?}",
            result.err()
        );

        let rates = result.unwrap();
        assert!(
            !rates.is_empty(),
            "Expected some funding rate history, got none"
        );

        println!(
            "Fetched {} Derive funding rate entries. First rate: {} at {}",
            rates.len(),
            rates[0].funding_rate,
            rates[0].timestamp
        );
    }

    #[tokio::test]
    async fn test_get_tickers_perp() {
        let api = MarketApi::new(10);
        let result = api.get_tickers("perp", None).await;

        assert!(
            result.is_ok(),
            "Failed to fetch Derive tickers: {:?}",
            result.err()
        );

        let tickers = result.unwrap();
        assert!(
            !tickers.is_empty(),
            "Expected some perp tickers, got none"
        );

        println!("Fetched {} Derive perp tickers", tickers.len());
    }
}
