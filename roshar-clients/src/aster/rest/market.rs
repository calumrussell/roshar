use crate::http::RateLimitedClient;
use roshar_types::aster::{
    AsterFundingRate, AsterKline, AsterMarkPrice, AsterOrderBook, AsterTicker24hr,
};
use std::collections::HashMap;

const BASE_URL: &str = "https://fapi.asterdex.com";

pub struct MarketApi {
    client: RateLimitedClient,
}

impl MarketApi {
    pub fn new(requests_per_second: u32) -> Self {
        Self {
            client: RateLimitedClient::new(requests_per_second, 1),
        }
    }

    /// Fetch an order book snapshot for a symbol.
    /// `GET /fapi/v1/depth?symbol=...&limit=...`
    /// Valid limits: 5, 10, 20, 50, 100, 500, 1000. Default 100.
    /// Note: the diff-depth WS stream (`{symbol}@depth@100ms`) needs an initial
    /// snapshot from this endpoint before diffs can be applied correctly.
    pub async fn get_orderbook(
        &self,
        symbol: &str,
        limit: Option<u32>,
    ) -> Result<AsterOrderBook, Box<dyn std::error::Error + Send + Sync>> {
        let depth = limit.unwrap_or(100);
        let url = format!("{BASE_URL}/fapi/v1/depth?symbol={symbol}&limit={depth}");
        let response = self.client.get(&url).await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("Aster depth failed ({status}): {body}").into());
        }

        let text = response.text().await?;
        let book: AsterOrderBook = serde_json::from_str(&text)
            .map_err(|e| format!("Failed to parse Aster order book: {e}"))?;
        Ok(book)
    }

    /// Fetch mark price and current funding rate for all symbols.
    /// `GET /fapi/v1/markPrice`
    pub async fn get_mark_prices(
        &self,
    ) -> Result<Vec<AsterMarkPrice>, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{BASE_URL}/fapi/v1/markPrice");
        let response = self.client.get(&url).await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("Aster markPrice failed ({status}): {body}").into());
        }

        let text = response.text().await?;
        let prices: Vec<AsterMarkPrice> = serde_json::from_str(&text)
            .map_err(|e| format!("Failed to parse Aster mark prices: {e}"))?;
        Ok(prices)
    }

    /// Fetch 24-hour ticker statistics for all symbols.
    /// `GET /fapi/v1/ticker/24hr`
    pub async fn get_tickers(
        &self,
    ) -> Result<HashMap<String, AsterTicker24hr>, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{BASE_URL}/fapi/v1/ticker/24hr");
        let response = self.client.get(&url).await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("Aster ticker failed ({status}): {body}").into());
        }

        let text = response.text().await?;
        let tickers: Vec<AsterTicker24hr> = serde_json::from_str(&text)
            .map_err(|e| format!("Failed to parse Aster tickers: {e}"))?;

        Ok(tickers.into_iter().map(|t| (t.symbol.clone(), t)).collect())
    }

    /// Fetch historical funding rates for a symbol with pagination.
    /// `GET /fapi/v1/fundingRate?symbol=...&startTime=...&endTime=...`
    pub async fn get_historical_funding_rates(
        &self,
        symbol: &str,
        start_time: i64,
        end_time: i64,
    ) -> Result<Vec<AsterFundingRate>, Box<dyn std::error::Error + Send + Sync>> {
        let mut all_rates: Vec<AsterFundingRate> = Vec::new();
        let mut current_start = start_time;
        const LIMIT: u32 = 1000;

        loop {
            let url = format!(
                "{BASE_URL}/fapi/v1/fundingRate?symbol={symbol}&startTime={current_start}&endTime={end_time}&limit={LIMIT}"
            );
            let response = self.client.get(&url).await?;

            if !response.status().is_success() {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                return Err(format!("Aster fundingRate failed ({status}): {body}").into());
            }

            let text = response.text().await?;
            let rates: Vec<AsterFundingRate> = serde_json::from_str(&text)
                .map_err(|e| format!("Failed to parse Aster funding rates: {e}"))?;

            if rates.is_empty() {
                break;
            }

            let last_time = rates.last().unwrap().funding_time;
            let batch_size = rates.len();
            all_rates.extend(rates);

            if batch_size < LIMIT as usize || last_time >= end_time {
                break;
            }

            current_start = last_time + 1;
        }

        Ok(all_rates)
    }

    /// Fetch OHLCV klines for a symbol.
    /// `GET /fapi/v1/klines?symbol=...&interval=...`
    pub async fn get_klines(
        &self,
        symbol: &str,
        interval: &str,
        start_time: Option<i64>,
        end_time: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<AsterKline>, Box<dyn std::error::Error + Send + Sync>> {
        let mut url = format!("{BASE_URL}/fapi/v1/klines?symbol={symbol}&interval={interval}");

        if let Some(start) = start_time {
            url.push_str(&format!("&startTime={start}"));
        }
        if let Some(end) = end_time {
            url.push_str(&format!("&endTime={end}"));
        }
        if let Some(lim) = limit {
            url.push_str(&format!("&limit={lim}"));
        }

        let response = self.client.get(&url).await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("Aster klines failed ({status}): {body}").into());
        }

        let text = response.text().await?;
        let klines: Vec<AsterKline> = serde_json::from_str(&text)
            .map_err(|e| format!("Failed to parse Aster klines: {e}"))?;
        Ok(klines)
    }
}
