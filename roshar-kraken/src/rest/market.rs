use roshar_http::RateLimitedClient;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct KrakenTickerData {
    pub symbol: String,
    #[serde(rename = "indexPrice", default)]
    pub index_price: f64,
    #[serde(rename = "markPrice", default)]
    pub mark_price: f64,
    #[serde(default)]
    pub bid: f64,
    #[serde(default)]
    pub ask: f64,
    #[serde(rename = "fundingRate", default)]
    pub funding_rate: f64,
    #[serde(rename = "fundingRatePrediction", default)]
    pub funding_rate_prediction: f64,
    #[serde(
        rename = "nextFundingRateTime",
        alias = "nextFundingTime",
        alias = "next_funding_time",
        default
    )]
    pub next_funding_rate_time: String,
    #[serde(rename = "openInterest", default)]
    pub open_interest: f64,
    #[serde(rename = "vol24h", default)]
    pub vol_24h: f64,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct KrakenTickerResponse {
    pub result: String,
    pub tickers: Vec<KrakenTickerData>,
}

pub struct MarketApi {
    client: Arc<RateLimitedClient>,
}

impl MarketApi {
    pub fn new_with_client(client: Arc<RateLimitedClient>) -> Self {
        Self { client }
    }

    pub async fn get_tickers(
        &self,
    ) -> Result<std::collections::HashMap<String, KrakenTickerData>, Box<dyn std::error::Error>>
    {
        let url = "https://futures.kraken.com/derivatives/api/v3/tickers";

        let response = self.client.get(url).await?;
        let ticker_response: KrakenTickerResponse = response.json().await?;

        let mut tickers = std::collections::HashMap::new();

        for ticker in ticker_response.tickers {
            tickers.insert(ticker.symbol.clone(), ticker);
        }

        Ok(tickers)
    }
}
