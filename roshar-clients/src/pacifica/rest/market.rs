use crate::http::RateLimitedClient;
use roshar_types::pacifica::{
    PacificaFundingRate, PacificaKline, PacificaMarket, PacificaOrderBook, PacificaTicker,
    PacificaTrade,
};
use std::collections::HashMap;

const BASE_URL: &str = "https://api.pacifica.fi/api/v1";

pub struct MarketApi {
    client: RateLimitedClient,
}

impl MarketApi {
    pub fn new(requests_per_second: u32) -> Self {
        Self {
            client: RateLimitedClient::new(requests_per_second, 1),
        }
    }

    /// Fetch all available perpetual markets.
    /// `GET /api/v1/markets`
    pub async fn get_markets(
        &self,
    ) -> Result<Vec<PacificaMarket>, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{BASE_URL}/markets");
        let response = self.client.get(&url).await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("Pacifica markets failed ({status}): {body}").into());
        }

        let text = response.text().await?;
        let markets: Vec<PacificaMarket> = serde_json::from_str(&text)
            .map_err(|e| format!("Failed to parse Pacifica markets: {e}"))?;
        Ok(markets)
    }

    /// Fetch ticker snapshots for all symbols, keyed by symbol.
    /// `GET /api/v1/ticker`
    pub async fn get_tickers(
        &self,
    ) -> Result<HashMap<String, PacificaTicker>, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{BASE_URL}/ticker");
        let response = self.client.get(&url).await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("Pacifica ticker failed ({status}): {body}").into());
        }

        let text = response.text().await?;
        let tickers: Vec<PacificaTicker> = serde_json::from_str(&text)
            .map_err(|e| format!("Failed to parse Pacifica tickers: {e}"))?;
        Ok(tickers.into_iter().map(|t| (t.symbol.clone(), t)).collect())
    }

    /// Fetch the current funding rate for a symbol (or all symbols if empty).
    /// `GET /api/v1/funding-rate?symbol=...`
    pub async fn get_funding_rates(
        &self,
        symbol: Option<&str>,
    ) -> Result<Vec<PacificaFundingRate>, Box<dyn std::error::Error + Send + Sync>> {
        let mut url = format!("{BASE_URL}/funding-rate");
        if let Some(sym) = symbol {
            url.push_str(&format!("?symbol={sym}"));
        }

        let response = self.client.get(&url).await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("Pacifica funding-rate failed ({status}): {body}").into());
        }

        let text = response.text().await?;
        let rates: Vec<PacificaFundingRate> = serde_json::from_str(&text)
            .map_err(|e| format!("Failed to parse Pacifica funding rates: {e}"))?;
        Ok(rates)
    }

    /// Fetch order book for a symbol.
    /// `GET /api/v1/orderbook?symbol=...&limit=...`
    /// Defaults to 50 levels per side when not specified.
    pub async fn get_orderbook(
        &self,
        symbol: &str,
        limit: Option<u32>,
    ) -> Result<PacificaOrderBook, Box<dyn std::error::Error + Send + Sync>> {
        let depth = limit.unwrap_or(50);
        let url = format!("{BASE_URL}/orderbook?symbol={symbol}&limit={depth}");

        let response = self.client.get(&url).await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("Pacifica orderbook failed ({status}): {body}").into());
        }

        let text = response.text().await?;
        let book: PacificaOrderBook = serde_json::from_str(&text)
            .map_err(|e| format!("Failed to parse Pacifica orderbook: {e}"))?;
        Ok(book)
    }

    /// Fetch recent trades for a symbol.
    /// `GET /api/v1/trades?symbol=...&limit=...`
    pub async fn get_recent_trades(
        &self,
        symbol: &str,
        limit: Option<u32>,
    ) -> Result<Vec<PacificaTrade>, Box<dyn std::error::Error + Send + Sync>> {
        let mut url = format!("{BASE_URL}/trades?symbol={symbol}");
        if let Some(lim) = limit {
            url.push_str(&format!("&limit={lim}"));
        }

        let response = self.client.get(&url).await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("Pacifica trades failed ({status}): {body}").into());
        }

        let text = response.text().await?;
        let trades: Vec<PacificaTrade> = serde_json::from_str(&text)
            .map_err(|e| format!("Failed to parse Pacifica trades: {e}"))?;
        Ok(trades)
    }

    /// Fetch OHLCV klines for a symbol.
    /// `GET /api/v1/klines?symbol=...&interval=...`
    pub async fn get_klines(
        &self,
        symbol: &str,
        interval: &str,
        start_time: Option<i64>,
        end_time: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<PacificaKline>, Box<dyn std::error::Error + Send + Sync>> {
        let mut url = format!("{BASE_URL}/klines?symbol={symbol}&interval={interval}");

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
            return Err(format!("Pacifica klines failed ({status}): {body}").into());
        }

        let text = response.text().await?;
        let klines: Vec<PacificaKline> = serde_json::from_str(&text)
            .map_err(|e| format!("Failed to parse Pacifica klines: {e}"))?;
        Ok(klines)
    }
}
