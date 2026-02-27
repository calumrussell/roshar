pub(crate) mod rest;

use rest::CoinbaseRestClient;

use roshar_types::CoinbaseProduct;

/// Coinbase Exchange client (REST-only MVP).
///
/// Provides access to public market data from the Coinbase Exchange API.
pub struct CoinbaseClient {
    rest_client: CoinbaseRestClient,
}

impl CoinbaseClient {
    /// Create a new Coinbase client.
    ///
    /// # Arguments
    /// * `requests_per_second` - Maximum REST API requests per second
    pub fn new(requests_per_second: u32) -> Self {
        Self {
            rest_client: CoinbaseRestClient::new(requests_per_second),
        }
    }

    /// Get all known trading pairs (products) from Coinbase Exchange.
    pub async fn get_products(&self) -> Result<Vec<CoinbaseProduct>, String> {
        self.rest_client
            .get_products()
            .await
            .map_err(|e| format!("Failed to get Coinbase products: {}", e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_products() {
        let client = CoinbaseClient::new(10);
        let result = client.get_products().await;

        assert!(
            result.is_ok(),
            "Failed to fetch products: {:?}",
            result.err()
        );

        let products = result.unwrap();
        assert!(
            !products.is_empty(),
            "Expected products data, got empty list"
        );

        println!("Fetched {} Coinbase products", products.len());
    }
}
