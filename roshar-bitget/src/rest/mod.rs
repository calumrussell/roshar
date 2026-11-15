pub mod market;

pub use crate::types::{BitgetContract, BitgetHistoricalFundingRate, BitgetTickerData};
pub use market::MarketApi;

use crate::BITGET_REST_URL;
