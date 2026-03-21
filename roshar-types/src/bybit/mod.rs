mod ws;

use serde::{Deserialize, Serialize};

pub use ws::{
    ByBitCandleData, ByBitCandleMessage, ByBitDepthBookData, ByBitDepthMessage, ByBitMessage,
    ByBitTradesData, ByBitTradesMessage, ByBitWssMessage,
};

/// Historical funding rate data from ByBit
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ByBitHistoricalFundingRate {
    pub symbol: String,
    #[serde(rename = "fundingRate")]
    pub funding_rate: String,
    #[serde(rename = "fundingRateTimestamp")]
    pub funding_rate_timestamp: String,
}
