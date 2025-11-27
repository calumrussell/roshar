use serde::{Deserialize, Serialize};

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

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct KrakenRestCandleData {
    pub time: i64,
    pub open: String,
    pub high: String,
    pub low: String,
    pub close: String,
    #[allow(dead_code)]
    pub volume: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct KrakenRestCandleResponse {
    pub candles: Vec<KrakenRestCandleData>,
}

pub struct MarketApi;

impl MarketApi {
    pub async fn get_all_funding_rates_with_size()
    -> Result<Vec<(String, f64, f64, f64)>, Box<dyn std::error::Error>> {
        let client = crate::http::get_http_client();
        let url = "https://futures.kraken.com/derivatives/api/v3/tickers";

        let response = client.get(url).send().await?;
        let ticker_response: KrakenTickerResponse = response.json().await?;

        let mut funding_rates = Vec::new();

        for ticker in ticker_response.tickers {
            if ticker.funding_rate != 0.0 || !ticker.next_funding_rate_time.is_empty() {
                let mid_price = (ticker.bid + ticker.ask) / 2.0;

                let relative_funding_rate = if mid_price > 0.0 {
                    ticker.funding_rate / mid_price
                } else {
                    ticker.funding_rate
                };

                let open_interest = ticker.open_interest * mid_price;
                let volume = ticker.vol_24h * mid_price;

                funding_rates.push((ticker.symbol, relative_funding_rate, open_interest, volume));
            }
        }
        Ok(funding_rates)
    }

    pub async fn get_tickers()
    -> Result<std::collections::HashMap<String, KrakenTickerData>, Box<dyn std::error::Error>> {
        let client = crate::http::get_http_client();
        let url = "https://futures.kraken.com/derivatives/api/v3/tickers";

        let response = client.get(url).send().await?;
        let ticker_response: KrakenTickerResponse = response.json().await?;

        let mut tickers = std::collections::HashMap::new();

        for ticker in ticker_response.tickers {
            tickers.insert(ticker.symbol.clone(), ticker);
        }

        Ok(tickers)
    }
}
