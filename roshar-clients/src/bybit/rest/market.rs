use crate::http::RateLimitedClient;
use roshar_types::ByBitHistoricalFundingRate;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

// ByBit Instruments Info Response Structures
#[derive(Debug, Deserialize, Serialize)]
struct ByBitInstrumentsResponse {
    #[serde(rename = "retCode")]
    ret_code: i32,
    #[serde(rename = "retMsg")]
    ret_msg: String,
    result: ByBitInstrumentsResult,
}

#[derive(Debug, Deserialize, Serialize)]
struct ByBitInstrumentsResult {
    category: String,
    list: Vec<ByBitInstrumentInfo>,
    #[serde(rename = "nextPageCursor")]
    next_page_cursor: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ByBitInstrumentInfo {
    pub symbol: String,
    #[serde(rename = "contractType")]
    pub contract_type: String,
    pub status: String,
    #[serde(rename = "baseCoin")]
    pub base_coin: String,
    #[serde(rename = "quoteCoin")]
    pub quote_coin: String,
    #[serde(rename = "launchTime")]
    pub launch_time: String,
    #[serde(rename = "priceScale")]
    pub price_scale: String,
    #[serde(rename = "leverageFilter")]
    pub leverage_filter: ByBitLeverageFilter,
    #[serde(rename = "priceFilter")]
    pub price_filter: ByBitPriceFilter,
    #[serde(rename = "lotSizeFilter")]
    pub lot_size_filter: ByBitLotSizeFilter,
    #[serde(rename = "fundingInterval")]
    pub funding_interval: i32,
    #[serde(rename = "settleCoin")]
    pub settle_coin: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ByBitLeverageFilter {
    #[serde(rename = "minLeverage")]
    pub min_leverage: String,
    #[serde(rename = "maxLeverage")]
    pub max_leverage: String,
    #[serde(rename = "leverageStep")]
    pub leverage_step: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ByBitPriceFilter {
    #[serde(rename = "minPrice")]
    pub min_price: String,
    #[serde(rename = "maxPrice")]
    pub max_price: String,
    #[serde(rename = "tickSize")]
    pub tick_size: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ByBitLotSizeFilter {
    #[serde(rename = "maxOrderQty")]
    pub max_order_qty: String,
    #[serde(rename = "minOrderQty")]
    pub min_order_qty: String,
    #[serde(rename = "qtyStep")]
    pub qty_step: String,
    #[serde(rename = "minNotionalValue")]
    pub min_notional_value: String,
}

// ByBit Funding Rate History Response Structures
#[derive(Debug, Deserialize, Serialize)]
struct ByBitFundingHistoryResponse {
    #[serde(rename = "retCode")]
    ret_code: i32,
    #[serde(rename = "retMsg")]
    ret_msg: String,
    result: ByBitFundingHistoryResult,
}

#[derive(Debug, Deserialize, Serialize)]
struct ByBitFundingHistoryResult {
    list: Vec<ByBitHistoricalFundingRate>,
}

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

/// ByBit Market API
pub struct MarketApi {
    client: Arc<RateLimitedClient>,
}

impl MarketApi {
    pub fn new(requests_per_second: u32) -> Self {
        Self {
            client: Arc::new(RateLimitedClient::new(requests_per_second, 1)),
        }
    }

    pub fn new_with_client(client: Arc<RateLimitedClient>) -> Self {
        Self { client }
    }

    /// Get all linear perpetual instruments info
    /// Returns a map of symbol -> instrument info
    pub async fn get_instruments_info(
        &self,
    ) -> Result<HashMap<String, ByBitInstrumentInfo>, Box<dyn std::error::Error + Send + Sync>>
    {
        let url = format!(
            "{}/v5/market/instruments-info?category=linear&limit=1000",
            super::BYBIT_REST_URL
        );

        let response = self.client.get(&url).await?;

        if !response.status().is_success() {
            return Err(format!(
                "ByBit API request failed with status: {}",
                response.status()
            )
            .into());
        }

        let response_text = response.text().await?;
        let instruments_response: ByBitInstrumentsResponse =
            serde_json::from_str(&response_text)
                .map_err(|e| format!("Failed to parse ByBit instruments response: {e}"))?;

        if instruments_response.ret_code != 0 {
            return Err(format!("ByBit API error: {}", instruments_response.ret_msg).into());
        }

        let mut instruments_map = HashMap::new();
        for instrument in instruments_response.result.list {
            // Only include actively trading perpetuals
            if instrument.status == "Trading" && instrument.contract_type == "LinearPerpetual" {
                instruments_map.insert(instrument.symbol.clone(), instrument);
            }
        }

        Ok(instruments_map)
    }

    pub async fn get_tickers(
        &self,
    ) -> Result<HashMap<String, ByBitTickerData>, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!(
            "{}/v5/market/tickers?category=linear",
            super::BYBIT_REST_URL
        );

        let response = self.client.get(&url).await?;

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

    /// Get all funding rates with size data for perpetual contracts
    ///
    /// Returns a vector of (symbol, funding_rate, open_interest_usd, volume_24h_usd)
    /// Uses the existing tickers API which contains all required data.
    pub async fn get_all_funding_rates_with_size(
        &self,
    ) -> Result<Vec<(String, f64, f64, f64)>, Box<dyn std::error::Error + Send + Sync>> {
        let tickers = self.get_tickers().await?;

        let mut funding_rates = Vec::new();

        for (symbol, ticker) in tickers {
            // Parse funding rate from string to f64
            let funding_rate: f64 = ticker.funding_rate.parse().unwrap_or(0.0);

            // Parse open interest value (already in USD)
            let open_interest_usd: f64 = ticker.open_interest_value.parse().unwrap_or(0.0);

            // Parse turnover_24h (already in USD for linear contracts)
            let volume_24h_usd: f64 = ticker.turnover_24h.parse().unwrap_or(0.0);

            // Only include if we have a valid funding rate or open interest
            if funding_rate != 0.0 || open_interest_usd > 0.0 {
                funding_rates.push((symbol, funding_rate, open_interest_usd, volume_24h_usd));
            }
        }

        Ok(funding_rates)
    }

    /// Fetch historical funding rates for a symbol
    ///
    /// Handles pagination internally - returns all funding rates in the time range.
    /// Results are returned in chronological order (oldest first).
    ///
    /// # Arguments
    /// * `symbol` - Trading pair symbol (e.g., "BTCUSDT")
    /// * `start_time` - Start time in milliseconds
    /// * `end_time` - End time in milliseconds
    pub async fn get_historical_funding_rates(
        &self,
        symbol: &str,
        start_time: i64,
        end_time: i64,
    ) -> Result<Vec<ByBitHistoricalFundingRate>, Box<dyn std::error::Error + Send + Sync>> {
        let mut all_rates = Vec::new();
        let mut current_end = end_time;
        const LIMIT: u32 = 200;

        loop {
            // ByBit requires both startTime and endTime, or just endTime
            let url = format!(
                "{}/v5/market/funding/history?category=linear&symbol={}&startTime={}&endTime={}&limit={}",
                super::BYBIT_REST_URL, symbol, start_time, current_end, LIMIT
            );

            let response = self.client.get(&url).await?;

            if !response.status().is_success() {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                return Err(format!(
                    "ByBit historical funding rates endpoint failed (status: {status}): {body}"
                )
                .into());
            }

            let response_text = response.text().await?;
            let funding_response: ByBitFundingHistoryResponse =
                serde_json::from_str(&response_text)
                    .map_err(|e| format!("Failed to parse ByBit funding rates response: {e}"))?;

            if funding_response.ret_code != 0 {
                return Err(format!("ByBit API error: {}", funding_response.ret_msg).into());
            }

            let rates = funding_response.result.list;
            if rates.is_empty() {
                break;
            }

            let batch_size = rates.len();

            // Find the earliest timestamp in this batch for pagination
            let earliest_time: i64 = rates
                .iter()
                .filter_map(|r| r.funding_rate_timestamp.parse().ok())
                .min()
                .unwrap_or(current_end);

            // Prepend rates (since we're paginating backwards)
            all_rates.splice(0..0, rates);

            // If we got less than limit, we've reached the beginning
            if batch_size < LIMIT as usize || earliest_time <= start_time {
                break;
            }

            // Move end time back for next iteration (subtract 1ms to avoid duplicates)
            current_end = earliest_time - 1;
        }

        // Sort by timestamp ascending (oldest first)
        all_rates.sort_by(|a, b| {
            let t1: i64 = a.funding_rate_timestamp.parse().unwrap_or(0);
            let t2: i64 = b.funding_rate_timestamp.parse().unwrap_or(0);
            t1.cmp(&t2)
        });

        Ok(all_rates)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_historical_funding_rates() {
        // Fetch 30 days of funding rates for BTCUSDT
        let end_time = chrono::Utc::now().timestamp_millis();
        let start_time = end_time - (30 * 24 * 60 * 60 * 1000); // 30 days ago

        let api = MarketApi::new(10);
        let result = api
            .get_historical_funding_rates("BTCUSDT", start_time, end_time)
            .await;

        assert!(
            result.is_ok(),
            "Failed to fetch funding rates: {:?}",
            result.err()
        );

        let rates = result.unwrap();

        // ByBit has 8-hour funding, so ~90 rates in 30 days
        assert!(!rates.is_empty(), "Expected some funding rates, got none");

        // Verify chronological order (oldest first)
        for window in rates.windows(2) {
            let t1: i64 = window[0].funding_rate_timestamp.parse().unwrap();
            let t2: i64 = window[1].funding_rate_timestamp.parse().unwrap();
            assert!(
                t1 <= t2,
                "Rates not in chronological order: {} > {}",
                t1,
                t2
            );
        }

        println!("Fetched {} funding rates for BTCUSDT", rates.len());
    }

    #[tokio::test]
    async fn test_get_historical_funding_rates_pagination() {
        // Fetch 2 years of funding rates to test pagination (> 200 results)
        let end_time = chrono::Utc::now().timestamp_millis();
        let start_time = end_time - (730 * 24 * 60 * 60 * 1000); // 2 years ago

        let api = MarketApi::new(10);
        let result = api
            .get_historical_funding_rates("BTCUSDT", start_time, end_time)
            .await;

        assert!(
            result.is_ok(),
            "Failed to fetch funding rates: {:?}",
            result.err()
        );

        let rates = result.unwrap();

        // 2 years * 365 days * 3 funding periods per day = ~2190 rates
        // This proves pagination worked (limit is 200 per request)
        assert!(
            rates.len() > 200,
            "Expected more than 200 rates (pagination), got {}",
            rates.len()
        );

        println!("Fetched {} funding rates (pagination working)", rates.len());
    }
}
