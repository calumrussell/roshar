mod ws;
pub use ws::*;

use serde::{Deserialize, Serialize};

/// A single product from GET /api/v3/brokerage/market/products.
///
/// Coinbase Advanced Trade is a spot exchange; there are no perpetual
/// futures with funding rates, so this struct covers spot/SPOT products.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoinbaseProduct {
    pub product_id: String,
    #[serde(default)]
    pub price: String,
    #[serde(rename = "price_percentage_change_24h", default)]
    pub price_percentage_change_24h: String,
    #[serde(rename = "volume_24h", default)]
    pub volume_24h: String,
    #[serde(rename = "volume_percentage_change_24h", default)]
    pub volume_percentage_change_24h: String,
    #[serde(rename = "base_increment", default)]
    pub base_increment: String,
    #[serde(rename = "quote_increment", default)]
    pub quote_increment: String,
    #[serde(rename = "quote_min_size", default)]
    pub quote_min_size: String,
    #[serde(rename = "base_min_size", default)]
    pub base_min_size: String,
    #[serde(rename = "base_max_size", default)]
    pub base_max_size: String,
    #[serde(rename = "base_name", default)]
    pub base_name: String,
    #[serde(rename = "quote_name", default)]
    pub quote_name: String,
    /// "online", "offline", etc.
    #[serde(default)]
    pub status: String,
    #[serde(rename = "cancel_only", default)]
    pub cancel_only: bool,
    #[serde(rename = "limit_only", default)]
    pub limit_only: bool,
    #[serde(rename = "post_only", default)]
    pub post_only: bool,
    #[serde(rename = "trading_disabled", default)]
    pub trading_disabled: bool,
    /// "SPOT", "FUTURE", etc.
    #[serde(rename = "product_type", default)]
    pub product_type: String,
    #[serde(rename = "quote_currency_id", default)]
    pub quote_currency_id: String,
    #[serde(rename = "base_currency_id", default)]
    pub base_currency_id: String,
    #[serde(rename = "mid_market_price", default)]
    pub mid_market_price: String,
}

/// Top-level response from GET /api/v3/brokerage/market/products.
#[derive(Debug, Deserialize, Serialize)]
pub struct CoinbaseProductsResponse {
    pub products: Vec<CoinbaseProduct>,
    pub num_products: Option<i32>,
}
