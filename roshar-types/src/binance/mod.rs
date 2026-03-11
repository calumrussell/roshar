mod kline;
mod ws;

pub use ws::{
    BinanceCandleMessage, BinanceDepthDiffMessage, BinanceKlineData,
    BinanceOrderBookSnapshot, BinanceTradeMessage, BinanceWssMessage, ExchangeInfo,
    OpenInterestData, SymbolInfo, TickerData,
};

pub use self::kline::BinanceKline;

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

/// Aggregate trade data from Binance API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BinanceAggTrade {
    #[serde(rename = "a")]
    pub agg_trade_id: i64,
    #[serde(rename = "p")]
    pub price: String,
    #[serde(rename = "q")]
    pub quantity: String,
    #[serde(rename = "f")]
    pub first_trade_id: i64,
    #[serde(rename = "l")]
    pub last_trade_id: i64,
    #[serde(rename = "T")]
    pub timestamp: i64,
    #[serde(rename = "m")]
    pub is_buyer_maker: bool,
}

/// Real-time premium index data from Binance Futures API
/// Contains mark price, index price, and funding rate information for perpetual contracts
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BinancePremiumIndex {
    pub symbol: String,
    pub mark_price: String,
    pub index_price: String,
    pub last_funding_rate: String,
    pub next_funding_time: i64,
}
