pub mod market;

pub use crate::types::{CoinbaseProduct, CoinbaseProductsResponse};
pub use market::MarketApi;

use crate::COINBASE_REST_URL;
