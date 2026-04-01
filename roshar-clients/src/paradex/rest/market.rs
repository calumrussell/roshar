use crate::http::RateLimitedClient;
use roshar_types::paradex::{
    ParadexFundingEntry, ParadexKline, ParadexListResponse, ParadexMarket, ParadexMarketSummary,
    ParadexOrderBook, ParadexTrade,
};
use std::collections::HashMap;

const BASE_URL: &str = "https://api.prod.paradex.trade/v1";

pub struct MarketApi {
    client: RateLimitedClient,
}

impl MarketApi {
    pub fn new(requests_per_second: u32) -> Self {
        Self {
            client: RateLimitedClient::new(requests_per_second, 1),
        }
    }

    /// Fetch static market definitions (contract specs, fee config, margin params).
    /// `GET /v1/markets`
    pub async fn get_markets(
        &self,
    ) -> Result<Vec<ParadexMarket>, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{BASE_URL}/markets");
        let response = self.client.get(&url).await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("Paradex markets failed ({status}): {body}").into());
        }

        let text = response.text().await?;
        let wrapper: ParadexListResponse<ParadexMarket> = serde_json::from_str(&text)
            .map_err(|e| format!("Failed to parse Paradex markets: {e}"))?;
        Ok(wrapper.results)
    }

    /// Fetch live ticker data for all or a single market, keyed by symbol.
    /// `GET /v1/markets/summary?market=ALL`
    pub async fn get_markets_summary(
        &self,
        market: Option<&str>,
    ) -> Result<HashMap<String, ParadexMarketSummary>, Box<dyn std::error::Error + Send + Sync>>
    {
        let param = market.unwrap_or("ALL");
        let url = format!("{BASE_URL}/markets/summary?market={param}");
        let response = self.client.get(&url).await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("Paradex markets/summary failed ({status}): {body}").into());
        }

        let text = response.text().await?;
        let wrapper: ParadexListResponse<ParadexMarketSummary> = serde_json::from_str(&text)
            .map_err(|e| format!("Failed to parse Paradex markets summary: {e}"))?;
        Ok(wrapper
            .results
            .into_iter()
            .map(|s| (s.symbol.clone(), s))
            .collect())
    }

    /// Fetch order book snapshot for a market.
    /// `GET /v1/orderbook/{market}?depth=...`
    /// Paradex defaults to 20 levels when unspecified; we request 50.
    pub async fn get_orderbook(
        &self,
        market: &str,
        depth: Option<u32>,
    ) -> Result<ParadexOrderBook, Box<dyn std::error::Error + Send + Sync>> {
        let levels = depth.unwrap_or(50);
        let url = format!("{BASE_URL}/orderbook/{market}?depth={levels}");

        let response = self.client.get(&url).await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("Paradex orderbook failed ({status}): {body}").into());
        }

        let text = response.text().await?;
        let book: ParadexOrderBook = serde_json::from_str(&text)
            .map_err(|e| format!("Failed to parse Paradex order book: {e}"))?;
        Ok(book)
    }

    /// Fetch recent public trades for a market.
    /// `GET /v1/markets/{market}/trades`
    pub async fn get_trades(
        &self,
        market: &str,
        limit: Option<u32>,
    ) -> Result<Vec<ParadexTrade>, Box<dyn std::error::Error + Send + Sync>> {
        let mut url = format!("{BASE_URL}/markets/{market}/trades");
        if let Some(lim) = limit {
            url.push_str(&format!("?page_size={lim}"));
        }

        let response = self.client.get(&url).await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("Paradex trades failed ({status}): {body}").into());
        }

        let text = response.text().await?;
        let wrapper: ParadexListResponse<ParadexTrade> = serde_json::from_str(&text)
            .map_err(|e| format!("Failed to parse Paradex trades: {e}"))?;
        Ok(wrapper.results)
    }

    /// Fetch OHLCV klines for a market.
    /// Resolution is in minutes: 1, 3, 5, 15, 30, 60.
    /// `GET /v1/markets/{market}/klines?resolution=...&start_at=...&end_at=...`
    pub async fn get_klines(
        &self,
        market: &str,
        resolution: u32,
        start_at: Option<i64>,
        end_at: Option<i64>,
    ) -> Result<Vec<ParadexKline>, Box<dyn std::error::Error + Send + Sync>> {
        let mut url = format!("{BASE_URL}/markets/{market}/klines?resolution={resolution}");

        if let Some(start) = start_at {
            url.push_str(&format!("&start_at={start}"));
        }
        if let Some(end) = end_at {
            url.push_str(&format!("&end_at={end}"));
        }

        let response = self.client.get(&url).await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("Paradex klines failed ({status}): {body}").into());
        }

        let text = response.text().await?;
        let wrapper: ParadexListResponse<ParadexKline> = serde_json::from_str(&text)
            .map_err(|e| format!("Failed to parse Paradex klines: {e}"))?;
        Ok(wrapper.results)
    }

    /// Fetch historical funding rate data for a market (paginated).
    /// `GET /v1/markets/{market}/funding`
    pub async fn get_funding_history(
        &self,
        market: &str,
        start_at: Option<i64>,
        end_at: Option<i64>,
    ) -> Result<Vec<ParadexFundingEntry>, Box<dyn std::error::Error + Send + Sync>> {
        let mut all: Vec<ParadexFundingEntry> = Vec::new();
        let mut cursor: Option<String> = None;

        loop {
            let mut url = format!("{BASE_URL}/markets/{market}/funding?page_size=100");
            if let Some(start) = start_at {
                url.push_str(&format!("&start_at={start}"));
            }
            if let Some(end) = end_at {
                url.push_str(&format!("&end_at={end}"));
            }
            if let Some(ref c) = cursor {
                url.push_str(&format!("&cursor={c}"));
            }

            let response = self.client.get(&url).await?;

            if !response.status().is_success() {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                return Err(format!("Paradex funding history failed ({status}): {body}").into());
            }

            let text = response.text().await?;
            let wrapper: ParadexListResponse<ParadexFundingEntry> = serde_json::from_str(&text)
                .map_err(|e| format!("Failed to parse Paradex funding history: {e}"))?;

            let batch_size = wrapper.results.len();
            all.extend(wrapper.results);

            match wrapper.next {
                Some(next_cursor) if batch_size == 100 => {
                    cursor = Some(next_cursor);
                }
                _ => break,
            }
        }

        Ok(all)
    }
}
