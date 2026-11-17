use crate::types::{
    VariationalApiError, VariationalErrorResponse, VariationalStats, VariationalStatsV1,
};
use roshar_http::RateLimitedClient;
use serde::de::DeserializeOwned;

pub const VARIATIONAL_API_URL: &str = "https://omni-client-api.prod.ap-northeast-1.variational.io";

type ApiResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

pub struct MarketApi {
    client: RateLimitedClient,
}

impl MarketApi {
    pub fn new(requests_per_second: u32) -> Self {
        Self {
            client: RateLimitedClient::new(requests_per_second, 1),
        }
    }

    async fn get_json<T: DeserializeOwned>(&self, path: &str, what: &str) -> ApiResult<T> {
        let url = format!("{VARIATIONAL_API_URL}{path}");
        let response = self.client.get(&url).await?;
        let status = response.status();
        let text = response.text().await?;

        if !status.is_success() {
            // 429 carries `{"error","message","retry_after_seconds"}`; other errors (e.g. 404)
            // have been seen with an empty body.
            let error = serde_json::from_str::<VariationalErrorResponse>(&text).ok();
            return Err(VariationalApiError {
                status: status.as_u16(),
                error,
                body: text,
            }
            .into());
        }

        let parsed = serde_json::from_str(&text)
            .map_err(|e| format!("Failed to parse Variational {what}: {e}"))?;
        Ok(parsed)
    }

    /// Platform-wide and per-listing statistics for all perpetual and swap markets.
    /// `GET /metadata/v2/stats`
    pub async fn get_stats(&self) -> ApiResult<VariationalStats> {
        self.get_json("/metadata/v2/stats", "stats").await
    }

    /// Legacy v1 statistics. The exchange keeps it for existing integrations but does not
    /// recommend it: tickers carry no contract-type suffix and swaps report `funding_rate` `"0"`.
    /// `GET /metadata/stats`
    pub async fn get_stats_v1(&self) -> ApiResult<VariationalStatsV1> {
        self.get_json("/metadata/stats", "v1 stats").await
    }
}
