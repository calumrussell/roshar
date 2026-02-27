use crate::http::RateLimitedClient;
use roshar_types::CoinbaseProduct;

const BASE_URL: &str = "https://api.exchange.coinbase.com";

/// Coinbase Exchange REST API client
pub struct CoinbaseRestClient {
    client: RateLimitedClient,
}

impl CoinbaseRestClient {
    pub fn new(requests_per_second: u32) -> Self {
        Self {
            client: RateLimitedClient::new(requests_per_second, 1),
        }
    }

    async fn get(&self, url: &str) -> Result<reqwest::Response, reqwest::Error> {
        self.client.get(url).await
    }

    /// Get all known trading pairs (products) from Coinbase Exchange.
    ///
    /// This is a public endpoint that requires no authentication.
    /// Returns a list of all available products with their trading parameters.
    pub async fn get_products(
        &self,
    ) -> Result<Vec<CoinbaseProduct>, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/products", BASE_URL);
        let response = self.get(&url).await?;

        if !response.status().is_success() {
            return Err(format!("Coinbase API error: HTTP {}", response.status()).into());
        }

        let response_text = response.text().await?;
        let products: Vec<CoinbaseProduct> = serde_json::from_str(&response_text)
            .map_err(|e| format!("Failed to parse Coinbase products response: {e}"))?;

        Ok(products)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_products() {
        let client = CoinbaseRestClient::new(10);
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

        // BTC-USD should always be available
        let btc_usd = products.iter().find(|p| p.id == "BTC-USD");
        assert!(btc_usd.is_some(), "Expected BTC-USD in products list");

        let btc = btc_usd.unwrap();
        assert_eq!(btc.base_currency, "BTC");
        assert_eq!(btc.quote_currency, "USD");

        println!("Fetched {} Coinbase products", products.len());
    }
}
