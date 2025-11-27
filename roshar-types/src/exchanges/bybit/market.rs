use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ByBit Tickers API Response Structures
#[derive(Debug, Deserialize, Serialize)]
pub struct ByBitTickersResponse {
    #[serde(rename = "retCode")]
    pub ret_code: i32,
    #[serde(rename = "retMsg")]
    pub ret_msg: String,
    pub result: ByBitTickersResult,
    #[serde(rename = "retExtInfo")]
    pub ret_ext_info: serde_json::Value,
    pub time: u64,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ByBitTickersResult {
    pub category: String,
    pub list: Vec<ByBitTickerData>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ByBitTickerData {
    pub symbol: String,
    #[serde(rename = "lastPrice")]
    pub last_price: String,
    #[serde(rename = "indexPrice")]
    pub index_price: String,
    #[serde(rename = "markPrice")]
    pub mark_price: String,
    #[serde(rename = "prevPrice24h")]
    pub prev_price_24h: String,
    #[serde(rename = "price24hPcnt")]
    pub price_24h_pcnt: String,
    #[serde(rename = "highPrice24h")]
    pub high_price_24h: String,
    #[serde(rename = "lowPrice24h")]
    pub low_price_24h: String,
    #[serde(rename = "prevPrice1h")]
    pub prev_price_1h: String,
    #[serde(rename = "openInterest")]
    pub open_interest: String,
    #[serde(rename = "openInterestValue")]
    pub open_interest_value: String,
    #[serde(rename = "turnover24h")]
    pub turnover_24h: String,
    #[serde(rename = "volume24h")]
    pub volume_24h: String,
    #[serde(rename = "fundingRate")]
    pub funding_rate: String,
    #[serde(rename = "nextFundingTime")]
    pub next_funding_time: String,
    #[serde(rename = "predictedDeliveryPrice")]
    pub predicted_delivery_price: String,
    #[serde(rename = "basisRate")]
    pub basis_rate: String,
    #[serde(rename = "deliveryFeeRate")]
    pub delivery_fee_rate: String,
    #[serde(rename = "deliveryTime")]
    pub delivery_time: String,
    #[serde(rename = "ask1Size")]
    pub ask1_size: String,
    #[serde(rename = "bid1Price")]
    pub bid1_price: String,
    #[serde(rename = "ask1Price")]
    pub ask1_price: String,
    #[serde(rename = "bid1Size")]
    pub bid1_size: String,
    #[serde(rename = "basis")]
    pub basis: String,
}

// Market API implementation
pub struct MarketApi;

impl MarketApi {
    pub async fn get_tickers(
        client: &Client,
    ) -> Result<HashMap<String, ByBitTickerData>, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!(
            "{}/v5/market/tickers?category=linear",
            crate::exchanges::bybit::BYBIT_REST_URL
        );

        let response = client
            .get(&url)
            .header("Content-Type", "application/json")
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(format!(
                "ByBit API request failed with status: {}",
                response.status()
            )
            .into());
        }

        let response_text = response.text().await?;
        let tickers_response: ByBitTickersResponse = serde_json::from_str(&response_text)
            .map_err(|e| format!("Failed to parse ByBit response: {e}"))?;

        if tickers_response.ret_code != 0 {
            return Err(format!("ByBit API error: {}", tickers_response.ret_msg).into());
        }

        let mut tickers_map = HashMap::new();
        for ticker in tickers_response.result.list {
            tickers_map.insert(ticker.symbol.clone(), ticker);
        }

        Ok(tickers_map)
    }
}
