use crate::types::{
    CoinbaseCandlesResponse, CoinbaseErrorResponse, CoinbaseGranularity,
    CoinbaseMarketTradesResponse, CoinbaseProduct, CoinbaseProductBookResponse,
    CoinbaseProductsQuery, CoinbaseProductsResponse, CoinbaseServerTime,
};
use roshar_http::RateLimitedClient;
use serde::de::DeserializeOwned;
use std::sync::Arc;

type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// Coinbase Advanced Trade public market data REST client.
///
/// These endpoints are unauthenticated. Products include spot (venue CBE), US futures (FCM),
/// equities (CCM) and the legacy INTX perpetuals, selected by `product_type`. International
/// perpetuals moved to the Global Derivatives gateway (drb.coinbase.com) on 2026-10-01; this
/// client does not cover that gateway (see the crate README).
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

    /// GET `path` with `query`, then parse the body as `T`. A non-2xx status returns the
    /// Coinbase `error`/`message` when the body has them.
    async fn get_json<T: DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, String)],
        what: &str,
    ) -> Result<T, BoxError> {
        let url = format!("{}{}", super::COINBASE_REST_URL, path);

        self.client.acquire().await;
        let response = self.client.inner().get(&url).query(query).send().await?;
        let status = response.status();
        let text = response.text().await?;

        if !status.is_success() {
            return Err(match serde_json::from_str::<CoinbaseErrorResponse>(&text) {
                Ok(e) => format!(
                    "Coinbase {what} request failed with status {status}: {}: {}",
                    e.error, e.message
                ),
                Err(_) => format!("Coinbase {what} request failed with status {status}: {text}"),
            }
            .into());
        }

        serde_json::from_str(&text)
            .map_err(|e| format!("Failed to parse Coinbase {what} response: {e}").into())
    }

    /// List products. With `CoinbaseProductsQuery::default()` the server returns all SPOT
    /// products, whatever their status. Page with `limit` plus `offset` or
    /// `cursor = pagination.next_cursor`.
    ///
    /// Endpoint: GET /api/v3/brokerage/market/products
    pub async fn get_products(
        &self,
        query: &CoinbaseProductsQuery,
    ) -> Result<CoinbaseProductsResponse, BoxError> {
        self.get_json(
            "/api/v3/brokerage/market/products",
            &query.to_query(),
            "products",
        )
        .await
    }

    /// Get a single product by its product ID, e.g. "BTC-USD" or "BIP-20DEC30-CDE".
    ///
    /// Endpoint: GET /api/v3/brokerage/market/products/{product_id}
    pub async fn get_product(&self, product_id: &str) -> Result<CoinbaseProduct, BoxError> {
        self.get_json(
            &format!("/api/v3/brokerage/market/products/{product_id}"),
            &[],
            "product",
        )
        .await
    }

    /// Order book of one product. `limit` is the number of levels per side;
    /// `aggregation_price_increment` groups levels into price buckets (e.g. "10").
    ///
    /// Endpoint: GET /api/v3/brokerage/market/product_book
    pub async fn get_product_book(
        &self,
        product_id: &str,
        limit: Option<u32>,
        aggregation_price_increment: Option<&str>,
    ) -> Result<CoinbaseProductBookResponse, BoxError> {
        let mut query = vec![("product_id", product_id.to_string())];
        if let Some(v) = limit {
            query.push(("limit", v.to_string()));
        }
        if let Some(v) = aggregation_price_increment {
            query.push(("aggregation_price_increment", v.to_string()));
        }
        self.get_json(
            "/api/v3/brokerage/market/product_book",
            &query,
            "product book",
        )
        .await
    }

    /// Recent trades plus best bid/ask of one product, newest first. `start`/`end` are UNIX
    /// seconds.
    ///
    /// The spec marks `limit` required; the server also answers without it.
    ///
    /// Endpoint: GET /api/v3/brokerage/market/products/{product_id}/ticker
    pub async fn get_market_trades(
        &self,
        product_id: &str,
        limit: Option<u32>,
        start: Option<u64>,
        end: Option<u64>,
    ) -> Result<CoinbaseMarketTradesResponse, BoxError> {
        let mut query = Vec::new();
        if let Some(v) = limit {
            query.push(("limit", v.to_string()));
        }
        if let Some(v) = start {
            query.push(("start", v.to_string()));
        }
        if let Some(v) = end {
            query.push(("end", v.to_string()));
        }
        self.get_json(
            &format!("/api/v3/brokerage/market/products/{product_id}/ticker"),
            &query,
            "market trades",
        )
        .await
    }

    /// Candles of one product, newest first. `start`/`end` are UNIX seconds; the server returns
    /// at most 350 candles (the default `limit`).
    ///
    /// The spec marks `start` and `end` required; the server also answers without them,
    /// returning the latest candles.
    ///
    /// Endpoint: GET /api/v3/brokerage/market/products/{product_id}/candles
    pub async fn get_candles(
        &self,
        product_id: &str,
        granularity: CoinbaseGranularity,
        start: Option<u64>,
        end: Option<u64>,
        limit: Option<u32>,
    ) -> Result<CoinbaseCandlesResponse, BoxError> {
        let mut query = vec![("granularity", granularity.as_str().to_string())];
        if let Some(v) = start {
            query.push(("start", v.to_string()));
        }
        if let Some(v) = end {
            query.push(("end", v.to_string()));
        }
        if let Some(v) = limit {
            query.push(("limit", v.to_string()));
        }
        self.get_json(
            &format!("/api/v3/brokerage/market/products/{product_id}/candles"),
            &query,
            "candles",
        )
        .await
    }

    /// Server time.
    ///
    /// Endpoint: GET /api/v3/brokerage/time
    pub async fn get_server_time(&self) -> Result<CoinbaseServerTime, BoxError> {
        self.get_json("/api/v3/brokerage/time", &[], "server time")
            .await
    }
}
