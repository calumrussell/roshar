use crate::http::RateLimitedClient;
use roshar_types::{CoinbaseProduct, CoinbaseProductsResponse};
use std::collections::HashMap;
use std::sync::Arc;

/// Coinbase Advanced Trade public market data REST client.
///
/// These endpoints are unauthenticated (public) and return spot market data.
/// Coinbase Advanced Trade does not offer perpetual futures with funding rates.
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

    /// Get all tradable products.
    /// Returns a map of product_id -> product info.
    ///
    /// Endpoint: GET /api/v3/brokerage/market/products
    /// Rate limit: 10/sec (IP, public endpoints)
    pub async fn get_products(
        &self,
    ) -> Result<HashMap<String, CoinbaseProduct>, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!(
            "{}/api/v3/brokerage/market/products",
            super::COINBASE_REST_URL
        );

        let response = self.client.get(&url).await?;

        if !response.status().is_success() {
            return Err(format!(
                "Coinbase API request failed with status: {}",
                response.status()
            )
            .into());
        }

        let text = response.text().await?;
        let parsed: CoinbaseProductsResponse = serde_json::from_str(&text)
            .map_err(|e| format!("Failed to parse Coinbase products response: {e}"))?;

        let map = parsed
            .products
            .into_iter()
            .filter(|p| p.status == "online" && !p.trading_disabled)
            .map(|p| (p.product_id.clone(), p))
            .collect();

        Ok(map)
    }

    /// Get a single product by its product ID (e.g. "BTC-USD").
    ///
    /// Endpoint: GET /api/v3/brokerage/market/products/{product_id}
    /// Rate limit: 10/sec (IP, public endpoints)
    pub async fn get_product(
        &self,
        product_id: &str,
    ) -> Result<CoinbaseProduct, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!(
            "{}/api/v3/brokerage/market/products/{}",
            super::COINBASE_REST_URL,
            product_id
        );

        let response = self.client.get(&url).await?;

        if !response.status().is_success() {
            return Err(format!(
                "Coinbase API request failed with status: {}",
                response.status()
            )
            .into());
        }

        let text = response.text().await?;
        let product: CoinbaseProduct = serde_json::from_str(&text)
            .map_err(|e| format!("Failed to parse Coinbase product response: {e}"))?;

        Ok(product)
    }
}
