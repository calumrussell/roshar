use crate::http::RateLimitedClient;
use roshar_types::{OkxCandle, OkxInstrumentInfo};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

// OKX generic response wrapper
#[derive(Debug, Deserialize, Serialize)]
pub struct OkxResponse<T> {
    pub code: String,
    pub msg: String,
    pub data: T,
}

// OKX Instruments Response
pub type OkxInstrumentsResponse = OkxResponse<Vec<OkxInstrumentInfo>>;

// OKX Tickers API Response
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct OkxTickerData {
    /// Instrument ID (e.g. "BTC-USDT-SWAP")
    #[serde(rename = "instType")]
    pub inst_type: String,
    /// Instrument ID
    #[serde(rename = "instId")]
    pub inst_id: String,
    /// Last traded price
    pub last: String,
    /// Last traded size
    #[serde(rename = "lastSz")]
    pub last_sz: String,
    /// Best ask price
    #[serde(rename = "askPx")]
    pub ask_px: String,
    /// Best ask size
    #[serde(rename = "askSz")]
    pub ask_sz: String,
    /// Best bid price
    #[serde(rename = "bidPx")]
    pub bid_px: String,
    /// Best bid size
    #[serde(rename = "bidSz")]
    pub bid_sz: String,
    /// Open price in the past 24 hours
    #[serde(rename = "open24h")]
    pub open_24h: String,
    /// Highest price in the past 24 hours
    #[serde(rename = "high24h")]
    pub high_24h: String,
    /// Lowest price in the past 24 hours
    #[serde(rename = "low24h")]
    pub low_24h: String,
    /// 24h trading volume in contracts
    #[serde(rename = "volCcy24h")]
    pub vol_ccy_24h: String,
    /// 24h trading volume in base currency
    #[serde(rename = "vol24h")]
    pub vol_24h: String,
    /// Timestamp (ms)
    pub ts: String,
}

pub type OkxTickersResponse = OkxResponse<Vec<OkxTickerData>>;

pub type OkxCandlesResponse = OkxResponse<Vec<OkxCandle>>;

/// OKX Market API for public data endpoints
pub struct MarketApi {
    client: Arc<RateLimitedClient>,
}

impl MarketApi {
    pub fn new(requests_per_second: u32) -> Self {
        Self {
            client: Arc::new(RateLimitedClient::new(requests_per_second, 1)),
        }
    }

    /// Get all SWAP (perpetual) instruments info.
    /// Returns a map of instId -> instrument info, filtered to live linear perpetuals.
    pub async fn get_instruments_info(
        &self,
    ) -> Result<HashMap<String, OkxInstrumentInfo>, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!(
            "{}/api/v5/public/instruments?instType=SWAP",
            super::OKX_REST_URL
        );

        let response = self.client.get(&url).await?;

        if !response.status().is_success() {
            return Err(format!(
                "OKX API request failed with status: {}",
                response.status()
            )
            .into());
        }

        let response_text = response.text().await?;
        let instruments_response: OkxInstrumentsResponse = serde_json::from_str(&response_text)
            .map_err(|e| format!("Failed to parse OKX instruments response: {e}"))?;

        if instruments_response.code != "0" {
            return Err(format!("OKX API error: {}", instruments_response.msg).into());
        }

        let mut instruments_map = HashMap::new();
        for instrument in instruments_response.data {
            // Only include live perpetuals
            if instrument.state == "live" {
                instruments_map.insert(instrument.inst_id.clone(), instrument);
            }
        }

        Ok(instruments_map)
    }

    /// Get tickers for all SWAP (perpetual) instruments.
    /// Returns a map of instId -> ticker data.
    pub async fn get_tickers(
        &self,
    ) -> Result<HashMap<String, OkxTickerData>, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!(
            "{}/api/v5/market/tickers?instType=SWAP",
            super::OKX_REST_URL
        );

        let response = self.client.get(&url).await?;

        if !response.status().is_success() {
            return Err(format!(
                "OKX API request failed with status: {}",
                response.status()
            )
            .into());
        }

        let response_text = response.text().await?;
        let tickers_response: OkxTickersResponse = serde_json::from_str(&response_text)
            .map_err(|e| format!("Failed to parse OKX tickers response: {e}"))?;

        if tickers_response.code != "0" {
            return Err(format!("OKX API error: {}", tickers_response.msg).into());
        }

        let mut tickers_map = HashMap::new();
        for ticker in tickers_response.data {
            tickers_map.insert(ticker.inst_id.clone(), ticker);
        }

        Ok(tickers_map)
    }

    /// Fetch candlestick data for an instrument.
    ///
    /// Uses GET /api/v5/market/candles (public, no auth).
    /// Rate limit: 40 requests per 2 seconds (IP-based).
    ///
    /// # Arguments
    /// * `inst_id` - Instrument ID (e.g. "BTC-USDT-SWAP")
    /// * `bar` - Bar size (e.g. "1m", "5m", "1H", "1D"). Default "1m"
    /// * `after` - Pagination: return records older than this timestamp (ms)
    /// * `before` - Pagination: return records newer than this timestamp (ms)
    /// * `limit` - Number of results per request (max 100, default 100)
    pub async fn get_candles(
        &self,
        inst_id: &str,
        bar: &str,
        after: Option<i64>,
        before: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<OkxCandle>, Box<dyn std::error::Error + Send + Sync>> {
        let mut url = format!(
            "{}/api/v5/market/candles?instId={}&bar={}",
            super::OKX_REST_URL,
            inst_id,
            bar
        );

        if let Some(after) = after {
            url.push_str(&format!("&after={}", after));
        }
        if let Some(before) = before {
            url.push_str(&format!("&before={}", before));
        }
        if let Some(limit) = limit {
            url.push_str(&format!("&limit={}", limit));
        }

        let response = self.client.get(&url).await?;

        if !response.status().is_success() {
            return Err(format!(
                "OKX API request failed with status: {}",
                response.status()
            )
            .into());
        }

        let response_text = response.text().await?;
        let candles_response: OkxCandlesResponse = serde_json::from_str(&response_text)
            .map_err(|e| format!("Failed to parse OKX candles response: {e}"))?;

        if candles_response.code != "0" {
            return Err(format!("OKX API error: {}", candles_response.msg).into());
        }

        Ok(candles_response.data)
    }

    /// Fetch historical candlestick data for an instrument.
    ///
    /// Uses GET /api/v5/market/history-candles (public, no auth).
    /// Rate limit: 40 requests per 2 seconds (IP-based).
    ///
    /// # Arguments
    /// * `inst_id` - Instrument ID (e.g. "BTC-USDT-SWAP")
    /// * `bar` - Bar size (e.g. "1m", "5m", "1H", "1D"). Default "1m"
    /// * `after` - Pagination: return records older than this timestamp (ms)
    /// * `before` - Pagination: return records newer than this timestamp (ms)
    /// * `limit` - Number of results per request (max 100, default 100)
    pub async fn get_candles_history(
        &self,
        inst_id: &str,
        bar: &str,
        after: Option<i64>,
        before: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<OkxCandle>, Box<dyn std::error::Error + Send + Sync>> {
        let mut url = format!(
            "{}/api/v5/market/history-candles?instId={}&bar={}",
            super::OKX_REST_URL,
            inst_id,
            bar
        );

        if let Some(after) = after {
            url.push_str(&format!("&after={}", after));
        }
        if let Some(before) = before {
            url.push_str(&format!("&before={}", before));
        }
        if let Some(limit) = limit {
            url.push_str(&format!("&limit={}", limit));
        }

        let response = self.client.get(&url).await?;

        if !response.status().is_success() {
            return Err(format!(
                "OKX API request failed with status: {}",
                response.status()
            )
            .into());
        }

        let response_text = response.text().await?;
        let candles_response: OkxCandlesResponse = serde_json::from_str(&response_text)
            .map_err(|e| format!("Failed to parse OKX candles history response: {e}"))?;

        if candles_response.code != "0" {
            return Err(format!("OKX API error: {}", candles_response.msg).into());
        }

        Ok(candles_response.data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_instruments_info() {
        let api = MarketApi::new(10);
        let result = api.get_instruments_info().await;

        assert!(
            result.is_ok(),
            "Failed to fetch instruments info: {:?}",
            result.err()
        );

        let instruments = result.unwrap();

        // Should have some instruments
        assert!(
            !instruments.is_empty(),
            "Expected some instruments, got none"
        );

        // Check that BTC-USDT-SWAP is present (a known perpetual)
        assert!(
            instruments.contains_key("BTC-USDT-SWAP"),
            "Expected BTC-USDT-SWAP to be present"
        );

        let btc = instruments.get("BTC-USDT-SWAP").unwrap();
        assert_eq!(btc.inst_type, "SWAP");
        assert_eq!(btc.state, "live");

        println!(
            "Fetched {} OKX SWAP instruments. BTC-USDT-SWAP ctVal: {}, tickSz: {}, lotSz: {}",
            instruments.len(),
            btc.ct_val,
            btc.tick_sz,
            btc.lot_sz
        );
    }

    #[tokio::test]
    async fn test_get_candles() {
        let api = MarketApi::new(10);
        let result = api
            .get_candles("BTC-USDT-SWAP", "1m", None, None, Some(5))
            .await;

        assert!(
            result.is_ok(),
            "Failed to fetch candles: {:?}",
            result.err()
        );

        let candles = result.unwrap();

        assert!(
            !candles.is_empty(),
            "Expected some candles, got none"
        );

        assert!(candles.len() <= 5, "Expected at most 5 candles");

        let first = &candles[0];
        assert!(first.ts > 0, "Expected positive timestamp");
        assert!(first.open > 0.0, "Expected positive open price");
        assert!(first.high >= first.low, "High should be >= low");
        assert!(first.confirm <= 1, "Confirm should be 0 or 1");

        println!(
            "Fetched {} OKX candles. First: ts={}, o={}, h={}, l={}, c={}, vol={}",
            candles.len(),
            first.ts,
            first.open,
            first.high,
            first.low,
            first.close,
            first.vol
        );
    }

    #[tokio::test]
    async fn test_get_tickers() {
        let api = MarketApi::new(10);
        let result = api.get_tickers().await;

        assert!(
            result.is_ok(),
            "Failed to fetch tickers: {:?}",
            result.err()
        );

        let tickers = result.unwrap();

        assert!(!tickers.is_empty(), "Expected some tickers, got none");

        assert!(
            tickers.contains_key("BTC-USDT-SWAP"),
            "Expected BTC-USDT-SWAP ticker to be present"
        );

        let btc_ticker = tickers.get("BTC-USDT-SWAP").unwrap();
        assert!(!btc_ticker.last.is_empty(), "Expected last price to be present");

        println!(
            "Fetched {} OKX SWAP tickers. BTC-USDT-SWAP last: {}, bid: {}, ask: {}",
            tickers.len(),
            btc_ticker.last,
            btc_ticker.bid_px,
            btc_ticker.ask_px
        );
    }
}
