pub mod market;

pub use market::MarketApi;
pub use roshar_types::{KuCoinContract, KuCoinFundingRate};

use crate::KUCOIN_FUTURES_REST_URL;
