use serde::{Deserialize, Serialize};

/// A trading product (pair) on the Coinbase Exchange.
///
/// Returned by `GET https://api.exchange.coinbase.com/products`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoinbaseProduct {
    pub id: String,
    pub base_currency: String,
    pub quote_currency: String,
    pub quote_increment: String,
    pub base_increment: String,
    pub display_name: String,
    #[serde(default)]
    pub min_market_funds: Option<String>,
    #[serde(default)]
    pub margin_enabled: bool,
    #[serde(default)]
    pub post_only: bool,
    #[serde(default)]
    pub limit_only: bool,
    #[serde(default)]
    pub cancel_only: bool,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub status_message: Option<String>,
    #[serde(default)]
    pub trading_disabled: bool,
    #[serde(default)]
    pub fx_stablecoin: bool,
    #[serde(default)]
    pub auction_mode: bool,
}
