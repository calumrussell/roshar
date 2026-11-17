//! Jupiter Perpetuals (Solana) through the Jupiter Perps API.
//!
//! Market data and wallet reads are plain REST. Trading is transaction-based: the API returns an
//! unsigned Solana transaction, the wallet signs it ([`signer::Keypair`]) and
//! [`TradingApi::execute`] submits it. The API has no websocket.

/// Signing of the transactions the API returns.
pub mod signer;
/// Request and response bodies.
pub mod types;

pub(crate) mod rest;

pub use rest::{MarketApi, TradingApi, JUPITER_PERPS_API_URL};

use crate::types::{JlpInfo, MarketStats, PoolInfo, TopTraders, TraderStats};
use async_trait::async_trait;
use roshar_http::RateLimitedClient;

type ApiResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

pub struct JupiterPerpsClient {
    market_api: MarketApi,
    trading_api: TradingApi,
}

impl JupiterPerpsClient {
    /// perps-api.jup.ag publishes no limit of its own. The only documented Jupiter limit for
    /// unauthenticated use is the keyless tier: 0.5 req/s, 30 per 60 s sliding window
    /// (https://developers.jup.ag/docs/portal/rate-limits). Default: 1 request every 2 s.
    const DEFAULT_REFILL: u32 = 1;
    const DEFAULT_INTERVAL_SECS: u64 = 2;

    pub fn new() -> Self {
        Self::new_with_rate_limit(Self::DEFAULT_REFILL, Self::DEFAULT_INTERVAL_SECS)
    }

    /// `refill` requests every `interval_secs`, shared by market data and trading calls.
    pub fn new_with_rate_limit(refill: u32, interval_secs: u64) -> Self {
        let client = RateLimitedClient::new(refill, interval_secs);
        Self {
            market_api: MarketApi::new(client.clone(), JUPITER_PERPS_API_URL),
            trading_api: TradingApi::new(client, JUPITER_PERPS_API_URL),
        }
    }

    pub fn market(&self) -> &MarketApi {
        &self.market_api
    }

    /// Wallet reads, transaction building and `POST /transaction/execute`.
    pub fn trading(&self) -> &TradingApi {
        &self.trading_api
    }
}

impl Default for JupiterPerpsClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Public market data.
#[async_trait]
pub trait JupiterPerpsApi {
    /// `GET /market-stats?mint=`.
    async fn get_market_stats(&self, mint: &str) -> ApiResult<MarketStats>;
    /// `GET /pool-info?mint=`: liquidity, utilisation, hourly borrow rates (Jupiter Perps has no
    /// funding rate) and fees.
    async fn get_pool_info(&self, mint: &str) -> ApiResult<PoolInfo>;
    /// `GET /jlp-info`: pool and per-custody state.
    async fn get_jlp_info(&self) -> ApiResult<JlpInfo>;
    /// `GET /top-traders`. `week` is `1`..`53` or `current`.
    async fn get_top_traders(
        &self,
        market_mint: &str,
        year: &str,
        week: &str,
    ) -> ApiResult<TopTraders>;
    /// `GET /trader-stats`. `week` is `1`..`53` or `current`.
    async fn get_trader_stats(
        &self,
        wallet_address: &str,
        market_mint: &str,
        year: &str,
        week: &str,
    ) -> ApiResult<TraderStats>;
}

#[async_trait]
impl JupiterPerpsApi for JupiterPerpsClient {
    async fn get_market_stats(&self, mint: &str) -> ApiResult<MarketStats> {
        self.market_api.market_stats(mint).await
    }

    async fn get_pool_info(&self, mint: &str) -> ApiResult<PoolInfo> {
        self.market_api.pool_info(mint).await
    }

    async fn get_jlp_info(&self) -> ApiResult<JlpInfo> {
        self.market_api.jlp_info().await
    }

    async fn get_top_traders(
        &self,
        market_mint: &str,
        year: &str,
        week: &str,
    ) -> ApiResult<TopTraders> {
        self.market_api.top_traders(market_mint, year, week).await
    }

    async fn get_trader_stats(
        &self,
        wallet_address: &str,
        market_mint: &str,
        year: &str,
        week: &str,
    ) -> ApiResult<TraderStats> {
        self.market_api
            .trader_stats(wallet_address, market_mint, year, week)
            .await
    }
}
