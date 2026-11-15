pub mod market;
pub mod trading;
pub use market::MarketApi;
pub use trading::TradingApi;

pub(crate) const MAINNET_BASE_URL: &str = "https://api.pacifica.fi/api/v1";
pub(crate) const TESTNET_BASE_URL: &str = "https://test-api.pacifica.fi/api/v1";

pub(crate) fn base_url(is_mainnet: bool) -> &'static str {
    if is_mainnet {
        MAINNET_BASE_URL
    } else {
        TESTNET_BASE_URL
    }
}
