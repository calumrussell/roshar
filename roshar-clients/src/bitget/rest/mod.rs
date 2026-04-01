pub mod market;

pub use market::MarketApi;
pub use roshar_types::{BitgetContract, BitgetHistoricalFundingRate, BitgetTickerData};

use crate::BITGET_REST_URL;
