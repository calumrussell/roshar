use crate::http::RateLimitedClient;
use roshar_types::polymarket::{ListEventsParams, PolymarketEvent};

/// Polymarket Gamma API client for market data
pub struct GammaApi {
    client: RateLimitedClient,
}

impl GammaApi {
    const BASE_URL: &'static str = "https://gamma-api.polymarket.com";

    pub fn new(requests_per_second: u32) -> Self {
        Self {
            client: RateLimitedClient::new(requests_per_second),
        }
    }

    /// List events with pagination and filters
    ///
    /// # Arguments
    /// * `params` - Query parameters for filtering and pagination
    ///
    /// # Returns
    /// * `Result<Vec<PolymarketEvent>, Box<dyn std::error::Error + Send + Sync>>` - List of events or error
    ///
    /// # Example
    /// ```no_run
    /// use roshar_clients::polymarket::GammaApi;
    /// use roshar_types::polymarket::ListEventsParams;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    ///     let api = GammaApi::new(10);
    ///     let params = ListEventsParams::new(10, 0);
    ///     let events = api.list_events(params).await?;
    ///     println!("Retrieved {} events", events.len());
    ///     Ok(())
    /// }
    /// ```
    pub async fn list_events(
        &self,
        params: ListEventsParams,
    ) -> Result<Vec<PolymarketEvent>, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/events", Self::BASE_URL);

        let request = self
            .client
            .request(reqwest::Method::GET, &url)
            .await
            .query(&params);

        let response = request.send().await?;

        if !response.status().is_success() {
            return Err(format!("API request failed with status: {}", response.status()).into());
        }

        let events: Vec<PolymarketEvent> = response.json().await?;
        Ok(events)
    }

    /// List events with simple pagination (convenience method)
    ///
    /// # Arguments
    /// * `limit` - Number of results to return
    /// * `offset` - Offset for pagination
    ///
    /// # Returns
    /// * `Result<Vec<PolymarketEvent>, Box<dyn std::error::Error + Send + Sync>>` - List of events or error
    ///
    /// # Example
    /// ```no_run
    /// use roshar_clients::polymarket::GammaApi;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    ///     let api = GammaApi::new(10);
    ///     let events = api.list_events_simple(10, 0).await?;
    ///     println!("Retrieved {} events", events.len());
    ///     Ok(())
    /// }
    /// ```
    pub async fn list_events_simple(
        &self,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<PolymarketEvent>, Box<dyn std::error::Error + Send + Sync>> {
        let params = ListEventsParams::new(limit, offset);
        self.list_events(params).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Ignore by default to avoid hitting API during normal test runs
    async fn test_list_events() {
        let api = GammaApi::new(10);
        let params = ListEventsParams::new(5, 0);
        let result = api.list_events(params).await;
        assert!(result.is_ok());
        let events = result.unwrap();
        assert!(!events.is_empty());
    }

    #[tokio::test]
    #[ignore] // Ignore by default to avoid hitting API during normal test runs
    async fn test_list_events_simple() {
        let api = GammaApi::new(10);
        let result = api.list_events_simple(5, 0).await;
        assert!(result.is_ok());
        let events = result.unwrap();
        assert!(!events.is_empty());
    }
}
