mod ws;

pub use ws::{
    BinanceCandleMessage, BinanceDepthDiffMessage, BinanceKlineData, BinanceOrderBook,
    BinanceOrderBookSnapshot, BinanceTradeMessage, BinanceWssMessage, ExchangeInfo,
    OpenInterestData, SymbolInfo, TickerData,
};

use serde::{Deserialize, Serialize};

/// Historical funding rate data from Binance Futures API
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BinanceHistoricalFundingRate {
    pub symbol: String,
    pub funding_rate: String,
    pub funding_time: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mark_price: Option<String>,
}

/// Premium index data from Binance Futures API /fapi/v1/premiumIndex
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BinancePremiumIndex {
    pub symbol: String,
    pub mark_price: String,
    pub index_price: String,
    pub estimated_settle_price: String,
    pub last_funding_rate: String,
    pub next_funding_time: i64,
    pub interest_rate: String,
}
