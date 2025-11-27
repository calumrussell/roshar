use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct ExchangeInfo {
    pub timezone: String,
    #[serde(rename = "serverTime")]
    pub server_time: i64,
    pub symbols: Vec<SymbolInfo>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SymbolInfo {
    pub symbol: String,
    pub status: String,
    #[serde(rename = "contractType")]
    pub contract_type: String,
    #[serde(rename = "baseAsset")]
    pub base_asset: String,
    #[serde(rename = "quoteAsset")]
    pub quote_asset: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OpenInterestData {
    pub symbol: String,
    #[serde(rename = "openInterest")]
    pub open_interest: String,
    pub time: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TickerData {
    pub symbol: String,
    pub volume: String,
    #[serde(rename = "quoteVolume")]
    pub quote_volume: String,
    #[serde(rename = "lastPrice")]
    pub last_price: String,
    #[serde(rename = "priceChange")]
    pub price_change: String,
    #[serde(rename = "priceChangePercent")]
    pub price_change_percent: String,
    #[serde(rename = "highPrice")]
    pub high_price: String,
    #[serde(rename = "lowPrice")]
    pub low_price: String,
    #[serde(rename = "openPrice")]
    pub open_price: String,
    #[serde(rename = "weightedAvgPrice")]
    pub weighted_avg_price: String,
    #[serde(rename = "lastQty")]
    pub last_qty: String,
    #[serde(rename = "openTime")]
    pub open_time: i64,
    #[serde(rename = "closeTime")]
    pub close_time: i64,
    #[serde(rename = "firstId")]
    pub first_id: i64,
    #[serde(rename = "lastId")]
    pub last_id: i64,
    pub count: u64,
}

pub struct BinanceRestClient {
    client: Client,
    base_url: String,
}

impl BinanceRestClient {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            base_url: "https://fapi.binance.com".to_string(),
        }
    }

    pub async fn get_exchange_info(
        &self,
    ) -> Result<ExchangeInfo, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/fapi/v1/exchangeInfo", self.base_url);

        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            return Err(format!("Binance API error: HTTP {}", response.status()).into());
        }

        let response_text = response.text().await?;
        let exchange_info: ExchangeInfo = serde_json::from_str(&response_text)
            .map_err(|e| format!("Failed to parse Binance exchange info response: {e}"))?;

        Ok(exchange_info)
    }

    pub async fn get_open_interest(
        &self,
        symbol: &str,
    ) -> Result<OpenInterestData, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/fapi/v1/openInterest?symbol={}", self.base_url, symbol);

        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            return Err(format!("Binance API error: HTTP {}", response.status()).into());
        }

        let response_text = response.text().await?;
        let open_interest_data: OpenInterestData = serde_json::from_str(&response_text)
            .map_err(|e| format!("Failed to parse Binance open interest response: {e}"))?;

        Ok(open_interest_data)
    }

    pub async fn get_24hr_ticker(
        &self,
        symbol: Option<&str>,
    ) -> Result<Vec<TickerData>, Box<dyn std::error::Error + Send + Sync>> {
        let mut url = format!("{}/fapi/v1/ticker/24hr", self.base_url);

        if let Some(symbol) = symbol {
            url.push_str(&format!("?symbol={symbol}"));
        }

        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            return Err(format!("Binance API error: HTTP {}", response.status()).into());
        }

        let response_text = response.text().await?;

        if symbol.is_some() {
            // Single symbol response
            let ticker_data: TickerData = serde_json::from_str(&response_text)
                .map_err(|e| format!("Failed to parse Binance 24hr ticker response: {e}"))?;
            Ok(vec![ticker_data])
        } else {
            // Multiple symbols response
            let ticker_data: Vec<TickerData> = serde_json::from_str(&response_text)
                .map_err(|e| format!("Failed to parse Binance 24hr ticker response: {e}"))?;
            Ok(ticker_data)
        }
    }
}

impl Default for BinanceRestClient {
    fn default() -> Self {
        Self::new()
    }
}
