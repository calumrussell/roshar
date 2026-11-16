mod ws;

use serde::{Deserialize, Serialize};

pub use ws::{
    ByBitAdlAlertData, ByBitAdlAlertMessage, ByBitCandleData, ByBitCandleMessage,
    ByBitCommandResponse, ByBitCommandResponseData, ByBitContractTickerData,
    ByBitContractTickerMessage, ByBitDepthBookData, ByBitDepthMessage, ByBitInsurancePoolData,
    ByBitInsurancePoolMessage, ByBitKlineInterval, ByBitLiquidationData, ByBitLiquidationMessage,
    ByBitMessage, ByBitOptionTickerData, ByBitOptionTickerMessage, ByBitOrderbookDepth, ByBitPong,
    ByBitPriceLimitData, ByBitPriceLimitMessage, ByBitRpiDepthBookData, ByBitRpiDepthMessage,
    ByBitSpotTickerData, ByBitSpotTickerMessage, ByBitSpreadTickerData, ByBitSpreadTickerMessage,
    ByBitSystemStatusMessage, ByBitTopic, ByBitTradesData, ByBitTradesMessage, ByBitWsMessage,
    ByBitWsStream, ByBitWssMessage,
};

/// Row of `GET /v5/market/funding/history`.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ByBitHistoricalFundingRate {
    pub symbol: String,
    #[serde(rename = "fundingRate")]
    pub funding_rate: String,
    #[serde(rename = "fundingRateTimestamp")]
    pub funding_rate_timestamp: String,
}
