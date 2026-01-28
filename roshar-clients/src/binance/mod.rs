pub(crate) mod rest;
pub(crate) mod ws;

use rest::BinanceRestClient;
use ws::MarketDataFeedHandle;

pub(crate) use ws::MarketDataFeed;
pub use ws::MarketEvent;

use roshar_types::BinancePremiumIndex;
use roshar_ws_mgr::Manager;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;

/// Binance client that manages WebSocket feeds and REST API calls
pub struct BinanceClient {
    market_data_handle: MarketDataFeedHandle,
    #[allow(dead_code)]
    market_data_feed_handle: tokio::task::JoinHandle<()>,
    rest_client: BinanceRestClient,
}

impl BinanceClient {
    /// Create a new Binance client.
    ///
    /// # Arguments
    /// * `ws_manager` - WebSocket manager
    /// * `channel_size` - Channel size for market data
    /// * `requests_per_second` - Maximum REST API requests per second
    pub fn new(ws_manager: Arc<Manager>, channel_size: usize, requests_per_second: u32) -> Self {
        let market_data_feed = MarketDataFeed::new(ws_manager, channel_size);
        let market_data_handle = market_data_feed.get_handle();
        let market_data_feed_handle = tokio::spawn(async move {
            market_data_feed.run().await;
        });

        Self {
            market_data_handle,
            market_data_feed_handle,
            rest_client: BinanceRestClient::new(requests_per_second),
        }
    }

    /// Get the event receiver for reactive market data consumption
    /// Can only be called once - subsequent calls will return an error
    /// Automatically disables raw mode
    pub async fn take_event_receiver(&self) -> Result<mpsc::Receiver<MarketEvent>, String> {
        self.market_data_handle.get_event_channel().await
    }

    /// Get the raw receiver for raw JSON message consumption
    /// Can only be called once - subsequent calls will return an error
    /// Automatically enables raw mode - no parsing will occur, only raw JSON forwarding
    pub async fn take_raw_receiver(&self) -> Result<mpsc::Receiver<String>, String> {
        self.market_data_handle.get_raw_channel().await
    }

    /// Trigger restart of market data feed
    pub async fn restart_market_data(&self) {
        if let Err(e) = self.market_data_handle.restart_feed().await {
            log::error!(
                "Failed to send restart command to Binance market data feed: {}",
                e
            );
        }
    }

    /// Subscribe to depth updates for symbols (batch subscription)
    pub async fn add_depth(&self, symbols: &[&str]) -> Result<(), String> {
        self.market_data_handle.add_depth(symbols).await
    }

    /// Unsubscribe from depth updates for symbols
    pub async fn remove_depth(&self, symbols: &[&str]) -> Result<(), String> {
        self.market_data_handle.remove_depth(symbols).await
    }

    /// Subscribe to trade updates for symbols (batch subscription)
    pub async fn add_trades(&self, symbols: &[&str]) -> Result<(), String> {
        self.market_data_handle.add_trades(symbols).await
    }

    /// Unsubscribe from trade updates for symbols
    pub async fn remove_trades(&self, symbols: &[&str]) -> Result<(), String> {
        self.market_data_handle.remove_trades(symbols).await
    }

    /// Subscribe to candle updates for symbols (batch subscription)
    pub async fn add_candles(&self, symbols: &[&str]) -> Result<(), String> {
        self.market_data_handle.add_candles(symbols).await
    }

    /// Unsubscribe from candle updates for symbols
    pub async fn remove_candles(&self, symbols: &[&str]) -> Result<(), String> {
        self.market_data_handle.remove_candles(symbols).await
    }

    /// Get the latest depth for a symbol
    /// Returns None if not subscribed or no data received yet
    pub async fn get_latest_depth(
        &self,
        symbol: &str,
    ) -> Result<Option<roshar_types::OrderBookState>, String> {
        self.market_data_handle.get_latest_depth(symbol).await
    }

    /// Get 24hr ticker data for all symbols or a specific symbol
    pub async fn get_24hr_ticker(
        &self,
        symbol: Option<&str>,
    ) -> Result<Vec<roshar_types::TickerData>, String> {
        self.rest_client
            .get_24hr_ticker(symbol)
            .await
            .map_err(|e| format!("Failed to get 24hr ticker: {}", e))
    }

    /// Get historical funding rates for a symbol
    ///
    /// Handles pagination internally - returns all funding rates in the time range.
    /// If rate limiting is configured (via `with_rate_limit`), each page request
    /// respects the rate limit.
    pub async fn get_historical_funding_rates(
        &self,
        symbol: &str,
        start_time: i64,
        end_time: i64,
    ) -> Result<Vec<roshar_types::BinanceHistoricalFundingRate>, String> {
        self.rest_client
            .get_historical_funding_rates(symbol, start_time, end_time)
            .await
            .map_err(|e| format!("Failed to get historical funding rates: {}", e))
    }

    /// Get exchange info including all available symbols
    pub async fn get_exchange_info(&self) -> Result<roshar_types::ExchangeInfo, String> {
        self.rest_client
            .get_exchange_info()
            .await
            .map_err(|e| format!("Failed to get exchange info: {}", e))
    }

    /// Get klines (candlestick) data for a symbol
    pub async fn get_klines(
        &self,
        symbol: &str,
        interval: &str,
        start_time: Option<i64>,
        end_time: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<roshar_types::BinanceKline>, String> {
        self.rest_client
            .get_klines(symbol, interval, start_time, end_time, limit)
            .await
            .map_err(|e| format!("Failed to get klines: {}", e))
    /// Get real-time funding rates for all perpetual contracts
    ///
    /// Returns a HashMap keyed by symbol for easy lookup of funding rate data.
    /// Each entry contains mark price, index price, current funding rate, and next funding time.
    pub async fn get_realtime_funding_rates(
        &self,
    ) -> Result<HashMap<String, BinancePremiumIndex>, String> {
        let premium_index_list = self
            .rest_client
            .get_premium_index()
            .await
            .map_err(|e| format!("Failed to get premium index: {}", e))?;

        let mut funding_rates = HashMap::new();
        for premium_index in premium_index_list {
            funding_rates.insert(premium_index.symbol.clone(), premium_index);
        }

        Ok(funding_rates)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_realtime_funding_rates() {
        let ws_manager: Arc<Manager> = Manager::new();
        let client = BinanceClient::new(ws_manager, 100, 10);

        let result = client.get_realtime_funding_rates().await;

        assert!(
            result.is_ok(),
            "Failed to fetch realtime funding rates: {:?}",
            result.err()
        );

        let funding_rates = result.unwrap();

        // Binance has many perpetual contracts, should have data
        assert!(
            !funding_rates.is_empty(),
            "Expected funding rates data, got empty HashMap"
        );

        // Check that BTCUSDT exists (it's always available)
        assert!(
            funding_rates.contains_key("BTCUSDT"),
            "Expected BTCUSDT in funding rates"
        );

        // Verify the structure of a funding rate entry
        if let Some(btc_data) = funding_rates.get("BTCUSDT") {
            assert_eq!(btc_data.symbol, "BTCUSDT");
            // Mark price should be parseable as a float
            btc_data
                .mark_price
                .parse::<f64>()
                .expect("mark_price should be a valid number");
            // Funding rate should be parseable as a float
            btc_data
                .last_funding_rate
                .parse::<f64>()
                .expect("last_funding_rate should be a valid number");
            // Next funding time should be in the future (or very recent past)
            assert!(
                btc_data.next_funding_time > 0,
                "next_funding_time should be positive"
            );
        }

        println!(
            "Fetched {} funding rates (lookup working)",
            funding_rates.len()
        );
    }
}
