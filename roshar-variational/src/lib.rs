//! Variational Omni read-only API client.
//!
//! Variational documents one public REST resource (market statistics, v2 and legacy v1) and no
//! websocket. The trading API is "still in development, and is not yet available to any users"
//! (https://docs.variational.io/technical-documentation/api, checked 2026-11-15).

/// Wire types for the REST payloads.
pub mod types;

pub(crate) mod rest;

use rest::MarketApi;

pub use rest::market::VARIATIONAL_API_URL;
pub use rest::MarketApi as VariationalMarketApi;

use crate::types::{VariationalStats, VariationalStatsV1};
use async_trait::async_trait;

type ApiResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

pub struct VariationalClient {
    market_api: MarketApi,
}

impl VariationalClient {
    /// Documented limit: 10 requests per 10 seconds per IP (shared by v1 and v2), and 1000
    /// requests per minute globally.
    const DEFAULT_REQUESTS_PER_SECOND: u32 = 1;

    pub fn new() -> Self {
        Self::new_with_rate_limit(Self::DEFAULT_REQUESTS_PER_SECOND)
    }

    pub fn new_with_rate_limit(requests_per_second: u32) -> Self {
        Self {
            market_api: MarketApi::new(requests_per_second),
        }
    }
}

impl Default for VariationalClient {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
pub trait VariationalApi {
    /// `GET /metadata/v2/stats`: platform metrics plus every listing's mark price, volume,
    /// open interest, funding and indicative quotes.
    async fn get_stats(&self) -> ApiResult<VariationalStats>;

    /// `GET /metadata/stats`: legacy v1 shape (not recommended by the exchange).
    async fn get_stats_v1(&self) -> ApiResult<VariationalStatsV1>;
}

#[async_trait]
impl VariationalApi for VariationalClient {
    async fn get_stats(&self) -> ApiResult<VariationalStats> {
        self.market_api.get_stats().await
    }

    async fn get_stats_v1(&self) -> ApiResult<VariationalStatsV1> {
        self.market_api.get_stats_v1().await
    }
}
