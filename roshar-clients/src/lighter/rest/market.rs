use crate::http::RateLimitedClient;
use roshar_types::lighter::{
    LighterFundingRate, LighterMarket, LighterMarketDetail, LighterOrderBook, LighterTrade,
};

const BASE_URL: &str = "https://mainnet.zklighter.elliot.ai/api/v1";

pub struct MarketApi {
    client: RateLimitedClient,
}

impl MarketApi {
    pub fn new(requests_per_second: u32) -> Self {
        Self {
            client: RateLimitedClient::new(requests_per_second, 1),
        }
    }

    /// List all markets with fee and size metadata.
    /// `GET /api/v1/orderBooks`
    pub async fn get_markets(
        &self,
    ) -> Result<Vec<LighterMarket>, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{BASE_URL}/orderBooks");
        let response = self.client.get(&url).await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("Lighter orderBooks failed ({status}): {body}").into());
        }

        let text = response.text().await?;
        // Response is {"order_books": [...]}
        let wrapper: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| format!("Failed to parse Lighter markets response: {e}"))?;
        let markets: Vec<LighterMarket> = serde_json::from_value(
            wrapper["order_books"].clone(),
        )
        .map_err(|e| format!("Failed to parse Lighter markets: {e}"))?;
        Ok(markets)
    }

    /// Fetch extended market details including daily stats and mark price.
    /// `GET /api/v1/orderBookDetails`
    pub async fn get_market_details(
        &self,
    ) -> Result<Vec<LighterMarketDetail>, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{BASE_URL}/orderBookDetails");
        let response = self.client.get(&url).await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("Lighter orderBookDetails failed ({status}): {body}").into());
        }

        let text = response.text().await?;
        let wrapper: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| format!("Failed to parse Lighter market details response: {e}"))?;
        let details: Vec<LighterMarketDetail> = serde_json::from_value(
            wrapper["order_book_details"].clone(),
        )
        .map_err(|e| format!("Failed to parse Lighter market details: {e}"))?;
        Ok(details)
    }

    /// Fetch live order book for a market.
    /// `GET /api/v1/orderBookOrders?market_id=...&limit=...`
    /// Max 250 orders. Defaults to 50 levels per side when not specified.
    pub async fn get_orderbook(
        &self,
        market_id: u32,
        limit: Option<u32>,
    ) -> Result<LighterOrderBook, Box<dyn std::error::Error + Send + Sync>> {
        let depth = limit.unwrap_or(50);
        let url = format!("{BASE_URL}/orderBookOrders?market_id={market_id}&limit={depth}");

        let response = self.client.get(&url).await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("Lighter orderBookOrders failed ({status}): {body}").into());
        }

        let text = response.text().await?;
        let book: LighterOrderBook = serde_json::from_str(&text)
            .map_err(|e| format!("Failed to parse Lighter order book: {e}"))?;
        Ok(book)
    }

    /// Fetch recent trades for a market (max 100).
    /// `GET /api/v1/recentTrades?market_id=...&limit=...`
    pub async fn get_recent_trades(
        &self,
        market_id: u32,
        limit: Option<u32>,
    ) -> Result<Vec<LighterTrade>, Box<dyn std::error::Error + Send + Sync>> {
        let mut url = format!("{BASE_URL}/recentTrades?market_id={market_id}");
        if let Some(lim) = limit {
            url.push_str(&format!("&limit={lim}"));
        }

        let response = self.client.get(&url).await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("Lighter recentTrades failed ({status}): {body}").into());
        }

        let text = response.text().await?;
        let trades: Vec<LighterTrade> = serde_json::from_str(&text)
            .map_err(|e| format!("Failed to parse Lighter trades: {e}"))?;
        Ok(trades)
    }

    /// Fetch current funding rates across all markets (includes cross-exchange comparison).
    /// `GET /api/v1/funding-rates`
    pub async fn get_funding_rates(
        &self,
    ) -> Result<Vec<LighterFundingRate>, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{BASE_URL}/funding-rates");
        let response = self.client.get(&url).await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("Lighter funding-rates failed ({status}): {body}").into());
        }

        let text = response.text().await?;
        let wrapper: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| format!("Failed to parse Lighter funding rates response: {e}"))?;
        let rates: Vec<LighterFundingRate> = serde_json::from_value(
            wrapper["funding_rates"].clone(),
        )
        .map_err(|e| format!("Failed to parse Lighter funding rates: {e}"))?;
        Ok(rates)
    }
}
