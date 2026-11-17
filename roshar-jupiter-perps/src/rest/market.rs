use super::{parse, BoxError, Query};
use crate::types::{JlpInfo, MarketStats, PoolInfo, TopTraders, TraderStats};
use roshar_http::RateLimitedClient;
use serde::de::DeserializeOwned;

/// Public market data: `GET /market-stats`, `/pool-info`, `/jlp-info`, `/top-traders`,
/// `/trader-stats`. The API has no order book, candles, funding history or websocket: Jupiter
/// Perps is a pool (JLP) priced by oracles, and longs/shorts pay an hourly borrow rate
/// ([`PoolInfo`]) instead of funding.
pub struct MarketApi {
    client: RateLimitedClient,
    base_url: String,
}

impl MarketApi {
    pub fn new(client: RateLimitedClient, base_url: &str) -> Self {
        Self {
            client,
            base_url: base_url.to_string(),
        }
    }

    async fn get<T: DeserializeOwned>(&self, url: &str, what: &str) -> Result<T, BoxError> {
        parse(self.client.get(url).await?, what).await
    }

    /// `GET /market-stats?mint=`: price, 24 h change/high/low and volume of one market.
    pub async fn market_stats(&self, mint: &str) -> Result<MarketStats, BoxError> {
        let url = Query::new(&self.base_url, "market-stats")
            .param("mint", mint)
            .url;
        self.get(&url, "market-stats").await
    }

    /// `GET /pool-info?mint=`: available liquidity, utilisation, borrow rates and fees.
    pub async fn pool_info(&self, mint: &str) -> Result<PoolInfo, BoxError> {
        let url = Query::new(&self.base_url, "pool-info")
            .param("mint", mint)
            .url;
        self.get(&url, "pool-info").await
    }

    /// `GET /jlp-info`: JLP AUM, price, APR/APY and per-custody state.
    pub async fn jlp_info(&self) -> Result<JlpInfo, BoxError> {
        let url = Query::new(&self.base_url, "jlp-info").url;
        self.get(&url, "jlp-info").await
    }

    /// `GET /top-traders?marketMint=&year=&week=`. `week` is `1`..`53` or `current`.
    pub async fn top_traders(
        &self,
        market_mint: &str,
        year: &str,
        week: &str,
    ) -> Result<TopTraders, BoxError> {
        let url = Query::new(&self.base_url, "top-traders")
            .param("marketMint", market_mint)
            .param("year", year)
            .param("week", week)
            .url;
        self.get(&url, "top-traders").await
    }

    /// `GET /trader-stats?walletAddress=&marketMint=&year=&week=`. `week` is `1`..`53` or
    /// `current`. The API answers 500 for an address that is not a valid public key.
    pub async fn trader_stats(
        &self,
        wallet_address: &str,
        market_mint: &str,
        year: &str,
        week: &str,
    ) -> Result<TraderStats, BoxError> {
        let url = Query::new(&self.base_url, "trader-stats")
            .param("walletAddress", wallet_address)
            .param("marketMint", market_mint)
            .param("year", year)
            .param("week", week)
            .url;
        self.get(&url, "trader-stats").await
    }
}
