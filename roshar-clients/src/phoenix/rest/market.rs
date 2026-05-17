use crate::http::RateLimitedClient;
use roshar_types::phoenix::{
    PhoenixCandle, PhoenixCollateralEvent, PhoenixExchange, PhoenixFundingHistory,
    PhoenixMarketConfig, PhoenixOrderBook, PhoenixOrderHistory, PhoenixPaginatedResponse,
    PhoenixPnlPoint, PhoenixTradeHistory, PhoenixTraderState,
};

const BASE_URL: &str = "https://perp-api.phoenix.trade";

pub struct MarketApi {
    client: RateLimitedClient,
}

impl MarketApi {
    pub fn new(requests_per_second: u32) -> Self {
        Self {
            client: RateLimitedClient::new(requests_per_second, 1),
        }
    }

    /// Full exchange state including on-chain keys and all market configs.
    /// `GET /exchange`
    pub async fn get_exchange(
        &self,
    ) -> Result<PhoenixExchange, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{BASE_URL}/exchange");
        let response = self.client.get(&url).await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("Phoenix /exchange failed ({status}): {body}").into());
        }

        let text = response.text().await?;
        let exchange: PhoenixExchange = serde_json::from_str(&text)
            .map_err(|e| format!("Failed to parse Phoenix exchange: {e}"))?;
        Ok(exchange)
    }

    /// All market configurations.
    /// `GET /exchange/markets`
    pub async fn get_markets(
        &self,
    ) -> Result<Vec<PhoenixMarketConfig>, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{BASE_URL}/exchange/markets");
        let response = self.client.get(&url).await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("Phoenix /exchange/markets failed ({status}): {body}").into());
        }

        let text = response.text().await?;
        let markets: Vec<PhoenixMarketConfig> = serde_json::from_str(&text)
            .map_err(|e| format!("Failed to parse Phoenix markets: {e}"))?;
        Ok(markets)
    }

    /// Single market configuration by symbol (case-insensitive, uppercased server-side).
    /// `GET /exchange/market/{symbol}`
    pub async fn get_market(
        &self,
        symbol: &str,
    ) -> Result<PhoenixMarketConfig, Box<dyn std::error::Error + Send + Sync>> {
        let symbol = symbol.to_uppercase();
        let url = format!("{BASE_URL}/exchange/market/{symbol}");
        let response = self.client.get(&url).await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(
                format!("Phoenix /exchange/market/{symbol} failed ({status}): {body}").into(),
            );
        }

        let text = response.text().await?;
        let market: PhoenixMarketConfig = serde_json::from_str(&text)
            .map_err(|e| format!("Failed to parse Phoenix market config: {e}"))?;
        Ok(market)
    }

    /// L2 orderbook snapshot for a market.
    /// `GET /orderbook?symbol={symbol}`
    pub async fn get_orderbook(
        &self,
        symbol: &str,
    ) -> Result<PhoenixOrderBook, Box<dyn std::error::Error + Send + Sync>> {
        let symbol = symbol.to_uppercase();
        let url = format!("{BASE_URL}/orderbook?symbol={symbol}");
        let response = self.client.get(&url).await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("Phoenix /orderbook failed ({status}): {body}").into());
        }

        let text = response.text().await?;
        let book: PhoenixOrderBook = serde_json::from_str(&text)
            .map_err(|e| format!("Failed to parse Phoenix orderbook: {e}"))?;
        Ok(book)
    }

    /// OHLCV candles for a market and timeframe.
    /// Valid timeframes: "1s","5s","1m","5m","15m","30m","1h","4h","1d".
    /// `GET /candles?symbol={symbol}&timeframe={timeframe}&start_time=...&end_time=...&limit=...`
    pub async fn get_candles(
        &self,
        symbol: &str,
        timeframe: &str,
        start_time: Option<i64>,
        end_time: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<PhoenixCandle>, Box<dyn std::error::Error + Send + Sync>> {
        let symbol = symbol.to_uppercase();
        let mut url = format!("{BASE_URL}/candles?symbol={symbol}&timeframe={timeframe}");
        if let Some(st) = start_time {
            url.push_str(&format!("&start_time={st}"));
        }
        if let Some(et) = end_time {
            url.push_str(&format!("&end_time={et}"));
        }
        if let Some(lim) = limit {
            url.push_str(&format!("&limit={lim}"));
        }

        let response = self.client.get(&url).await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("Phoenix /candles failed ({status}): {body}").into());
        }

        let text = response.text().await?;
        let candles: Vec<PhoenixCandle> = serde_json::from_str(&text)
            .map_err(|e| format!("Failed to parse Phoenix candles: {e}"))?;
        Ok(candles)
    }

    /// Trader account state (positions, orders, collateral).
    /// `GET /trader/{authority}/state?pda_index={pda_index}`
    pub async fn get_trader_state(
        &self,
        authority: &str,
        pda_index: Option<u8>,
    ) -> Result<Vec<PhoenixTraderState>, Box<dyn std::error::Error + Send + Sync>> {
        let mut url = format!("{BASE_URL}/trader/{authority}/state");
        if let Some(idx) = pda_index {
            url.push_str(&format!("?pda_index={idx}"));
        }

        let response = self.client.get(&url).await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("Phoenix /trader/state failed ({status}): {body}").into());
        }

        let text = response.text().await?;
        let state: Vec<PhoenixTraderState> = serde_json::from_str(&text)
            .map_err(|e| format!("Failed to parse Phoenix trader state: {e}"))?;
        Ok(state)
    }

    /// PnL time-series for a trader.
    /// `GET /trader/{authority}/pnl?resolution={resolution}&start_time=...&end_time=...&limit=...`
    /// Valid resolutions: "Minute1","Minute5","Minute15","Hour1","Hour4","Day1","Week1","Month1".
    pub async fn get_trader_pnl(
        &self,
        authority: &str,
        resolution: &str,
        start_time: Option<i64>,
        end_time: Option<i64>,
        limit: Option<i64>,
    ) -> Result<Vec<PhoenixPnlPoint>, Box<dyn std::error::Error + Send + Sync>> {
        let mut url = format!("{BASE_URL}/trader/{authority}/pnl?resolution={resolution}");
        if let Some(st) = start_time {
            url.push_str(&format!("&start_time={st}"));
        }
        if let Some(et) = end_time {
            url.push_str(&format!("&end_time={et}"));
        }
        if let Some(lim) = limit {
            url.push_str(&format!("&limit={lim}"));
        }

        let response = self.client.get(&url).await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("Phoenix /trader/pnl failed ({status}): {body}").into());
        }

        let text = response.text().await?;
        let pnl: Vec<PhoenixPnlPoint> = serde_json::from_str(&text)
            .map_err(|e| format!("Failed to parse Phoenix trader PnL: {e}"))?;
        Ok(pnl)
    }

    /// Paginated order history for a trader.
    /// `GET /trader/{authority}/order-history?limit={limit}&market_symbol=...&cursor=...`
    pub async fn get_order_history(
        &self,
        authority: &str,
        market_symbol: Option<&str>,
        limit: Option<i64>,
        cursor: Option<&str>,
    ) -> Result<PhoenixPaginatedResponse<PhoenixOrderHistory>, Box<dyn std::error::Error + Send + Sync>>
    {
        let lim = limit.unwrap_or(50);
        let mut url =
            format!("{BASE_URL}/trader/{authority}/order-history?limit={lim}");
        if let Some(sym) = market_symbol {
            url.push_str(&format!("&market_symbol={sym}"));
        }
        if let Some(c) = cursor {
            url.push_str(&format!("&cursor={c}"));
        }

        let response = self.client.get(&url).await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("Phoenix /trader/order-history failed ({status}): {body}").into());
        }

        let text = response.text().await?;
        let result: PhoenixPaginatedResponse<PhoenixOrderHistory> =
            serde_json::from_str(&text)
                .map_err(|e| format!("Failed to parse Phoenix order history: {e}"))?;
        Ok(result)
    }

    /// Paginated trade history for a trader.
    /// `GET /trader/{authority}/trades-history?pda_index={pda_index}&market_symbol=...&limit=...&cursor=...`
    pub async fn get_trades_history(
        &self,
        authority: &str,
        pda_index: u8,
        market_symbol: Option<&str>,
        limit: Option<i64>,
        cursor: Option<&str>,
    ) -> Result<PhoenixPaginatedResponse<PhoenixTradeHistory>, Box<dyn std::error::Error + Send + Sync>>
    {
        let mut url =
            format!("{BASE_URL}/trader/{authority}/trades-history?pda_index={pda_index}");
        if let Some(sym) = market_symbol {
            url.push_str(&format!("&market_symbol={sym}"));
        }
        if let Some(lim) = limit {
            url.push_str(&format!("&limit={lim}"));
        }
        if let Some(c) = cursor {
            url.push_str(&format!("&cursor={c}"));
        }

        let response = self.client.get(&url).await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(
                format!("Phoenix /trader/trades-history failed ({status}): {body}").into(),
            );
        }

        let text = response.text().await?;
        let result: PhoenixPaginatedResponse<PhoenixTradeHistory> =
            serde_json::from_str(&text)
                .map_err(|e| format!("Failed to parse Phoenix trades history: {e}"))?;
        Ok(result)
    }

    /// Funding payment history for a trader.
    /// `GET /trader/{authority}/funding-history?pda_index={pda_index}&symbol=...&start_time=...&end_time=...&limit=...`
    pub async fn get_funding_history(
        &self,
        authority: &str,
        pda_index: u8,
        symbol: Option<&str>,
        start_time: Option<i64>,
        end_time: Option<i64>,
        limit: Option<i64>,
    ) -> Result<Vec<PhoenixFundingHistory>, Box<dyn std::error::Error + Send + Sync>> {
        let mut url =
            format!("{BASE_URL}/trader/{authority}/funding-history?pda_index={pda_index}");
        if let Some(sym) = symbol {
            url.push_str(&format!("&symbol={sym}"));
        }
        if let Some(st) = start_time {
            url.push_str(&format!("&start_time={st}"));
        }
        if let Some(et) = end_time {
            url.push_str(&format!("&end_time={et}"));
        }
        if let Some(lim) = limit {
            url.push_str(&format!("&limit={lim}"));
        }

        let response = self.client.get(&url).await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(
                format!("Phoenix /trader/funding-history failed ({status}): {body}").into(),
            );
        }

        let text = response.text().await?;
        // Response may be wrapped: {"items": [...]} or a bare array
        let wrapper: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| format!("Failed to parse Phoenix funding history response: {e}"))?;
        let items = if wrapper.is_array() {
            serde_json::from_value(wrapper)
        } else {
            serde_json::from_value(wrapper["items"].clone())
        }
        .map_err(|e| format!("Failed to parse Phoenix funding history items: {e}"))?;
        Ok(items)
    }

    /// Collateral deposit/withdrawal history for a trader.
    /// `GET /trader/{authority}/collateral-history?pdaIndex={pda_index}&limit={limit}&cursor=...`
    pub async fn get_collateral_history(
        &self,
        authority: &str,
        pda_index: u8,
        limit: Option<i64>,
        cursor: Option<&str>,
    ) -> Result<Vec<PhoenixCollateralEvent>, Box<dyn std::error::Error + Send + Sync>> {
        let lim = limit.unwrap_or(50);
        let mut url = format!(
            "{BASE_URL}/trader/{authority}/collateral-history?pdaIndex={pda_index}&limit={lim}"
        );
        if let Some(c) = cursor {
            url.push_str(&format!("&cursor={c}"));
        }

        let response = self.client.get(&url).await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(
                format!("Phoenix /trader/collateral-history failed ({status}): {body}").into(),
            );
        }

        let text = response.text().await?;
        let wrapper: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| format!("Failed to parse Phoenix collateral history response: {e}"))?;
        let items = if wrapper.is_array() {
            serde_json::from_value(wrapper)
        } else {
            serde_json::from_value(wrapper["items"].clone())
        }
        .map_err(|e| format!("Failed to parse Phoenix collateral history items: {e}"))?;
        Ok(items)
    }
}
