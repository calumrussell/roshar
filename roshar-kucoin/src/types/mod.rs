mod ws;
pub use ws::*;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KuCoinResponse<T> {
    pub code: String,
    #[serde(default)]
    pub msg: String,
    pub data: T,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KuCoinContract {
    pub symbol: String,
    #[serde(rename = "rootSymbol", default)]
    pub root_symbol: String,
    #[serde(rename = "baseCurrency", default)]
    pub base_currency: String,
    #[serde(rename = "quoteCurrency", default)]
    pub quote_currency: String,
    #[serde(rename = "settleCurrency", default)]
    pub settle_currency: String,
    /// Current funding fee rate
    #[serde(rename = "fundingFeeRate", default)]
    pub funding_fee_rate: f64,
    /// Predicted next funding fee rate
    #[serde(rename = "predictedFundingFeeRate", default)]
    pub predicted_funding_fee_rate: f64,
    #[serde(rename = "markPrice", default)]
    pub mark_price: f64,
    #[serde(rename = "indexPrice", default)]
    pub index_price: f64,
    #[serde(rename = "lastTradePrice", default)]
    pub last_trade_price: f64,
    #[serde(rename = "maxLeverage", default)]
    pub max_leverage: i32,
    #[serde(rename = "tickSize", default)]
    pub tick_size: f64,
    #[serde(rename = "lotSize", default)]
    pub lot_size: f64,
    /// Contract status: "Open", "BeingSettled", "Settled", etc.
    #[serde(default)]
    pub status: String,
    /// Contract type: "FFWCSX" (perpetual), "FFICSX" (quarterly), etc.
    #[serde(rename = "type", default)]
    pub contract_type: String,
    #[serde(rename = "multiplier", default)]
    pub multiplier: f64,
    #[serde(rename = "makerFeeRate", default)]
    pub maker_fee_rate: f64,
    #[serde(rename = "takerFeeRate", default)]
    pub taker_fee_rate: f64,
    /// Funding interval in hours (8 for 8-hour funding)
    #[serde(rename = "fundingRateSymbol", default)]
    pub funding_rate_symbol: String,
    #[serde(rename = "openInterest", default)]
    pub open_interest: String,
    #[serde(rename = "turnoverOf24h", default)]
    pub turnover_of_24h: f64,
    #[serde(rename = "volumeOf24h", default)]
    pub volume_of_24h: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KuCoinFundingRate {
    pub symbol: String,
    #[serde(rename = "fundingRate")]
    pub funding_rate: f64,
    /// Settlement timestamp in milliseconds
    pub timepoint: i64,
}

/// Response from GET /api/v1/contract/funding-rates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KuCoinFundingRateListData {
    #[serde(rename = "dataList")]
    pub data_list: Vec<KuCoinFundingRate>,
    #[serde(rename = "hasMore")]
    pub has_more: bool,
}

/// Token and connection info returned by POST /api/v1/bullet-public
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KuCoinWsInstanceServer {
    pub endpoint: String,
    pub encrypt: bool,
    pub protocol: String,
    #[serde(rename = "pingInterval")]
    pub ping_interval: u64,
    #[serde(rename = "pingTimeout")]
    pub ping_timeout: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KuCoinWsTokenData {
    pub token: String,
    #[serde(rename = "instanceServers")]
    pub instance_servers: Vec<KuCoinWsInstanceServer>,
}
