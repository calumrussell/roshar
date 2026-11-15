mod ws;
pub use ws::*;

use serde::{Deserialize, Serialize};

/// Generic Bitget API response envelope.
/// Success is indicated by code "00000".
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BitgetResponse<T> {
    pub code: String,
    pub msg: String,
    pub data: T,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BitgetContract {
    pub symbol: String,
    #[serde(rename = "baseCoin", default)]
    pub base_coin: String,
    #[serde(rename = "quoteCoin", default)]
    pub quote_coin: String,
    #[serde(rename = "buyLimitPriceRatio", default)]
    pub buy_limit_price_ratio: String,
    #[serde(rename = "sellLimitPriceRatio", default)]
    pub sell_limit_price_ratio: String,
    #[serde(rename = "makerFeeRate", default)]
    pub maker_fee_rate: String,
    #[serde(rename = "takerFeeRate", default)]
    pub taker_fee_rate: String,
    #[serde(rename = "minTradeNum", default)]
    pub min_trade_num: String,
    #[serde(rename = "priceEndStep", default)]
    pub price_end_step: String,
    #[serde(rename = "volumePlace", default)]
    pub volume_place: String,
    #[serde(rename = "pricePlace", default)]
    pub price_place: String,
    #[serde(rename = "sizeMultiplier", default)]
    pub size_multiplier: String,
    /// "perpetual" or "delivery"
    #[serde(rename = "symbolType", default)]
    pub symbol_type: String,
    #[serde(rename = "minLever", default)]
    pub min_lever: String,
    #[serde(rename = "maxLever", default)]
    pub max_lever: String,
    /// "normal", "halt", etc.
    #[serde(rename = "symbolStatus", default)]
    pub symbol_status: String,
    /// Funding interval in hours
    #[serde(rename = "fundInterval", default)]
    pub fund_interval: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BitgetTickerData {
    pub symbol: String,
    /// Last traded price
    #[serde(rename = "lastPr", default)]
    pub last_pr: String,
    #[serde(rename = "askPr", default)]
    pub ask_pr: String,
    #[serde(rename = "bidPr", default)]
    pub bid_pr: String,
    #[serde(rename = "bidSz", default)]
    pub bid_sz: String,
    #[serde(rename = "askSz", default)]
    pub ask_sz: String,
    #[serde(rename = "high24h", default)]
    pub high_24h: String,
    #[serde(rename = "low24h", default)]
    pub low_24h: String,
    /// Timestamp in milliseconds
    #[serde(default)]
    pub ts: String,
    #[serde(rename = "change24h", default)]
    pub change_24h: String,
    #[serde(rename = "baseVolume", default)]
    pub base_volume: String,
    #[serde(rename = "quoteVolume", default)]
    pub quote_volume: String,
    #[serde(rename = "usdtVolume", default)]
    pub usdt_volume: String,
    #[serde(rename = "fundingRate", default)]
    pub funding_rate: String,
    #[serde(rename = "markPrice", default)]
    pub mark_price: String,
    #[serde(rename = "indexPrice", default)]
    pub index_price: String,
    #[serde(rename = "holdingAmount", default)]
    pub holding_amount: String,
    #[serde(rename = "open24h", default)]
    pub open_24h: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BitgetHistoricalFundingRate {
    pub symbol: String,
    #[serde(rename = "fundingRate")]
    pub funding_rate: String,
    /// Settlement time in milliseconds
    #[serde(rename = "settleTime")]
    pub settle_time: String,
}
