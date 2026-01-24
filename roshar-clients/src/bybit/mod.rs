pub(crate) mod rest;
pub(crate) mod ws;

use rest::MarketApi;
use ws::MarketDataFeedHandle;

pub use rest::{
    ByBitCreateOrderRequest, ByBitCreateOrderResponse, ByBitInstrumentInfo, ByBitLeverageFilter,
    ByBitLotSizeFilter, ByBitOrderResult, ByBitPriceFilter, ByBitTickerData, ByBitTickersResponse,
    MarketApi as ByBitMarketApi,
};
pub use roshar_types::ByBitHistoricalFundingRate;
pub(crate) use ws::MarketDataFeed;
pub use ws::MarketEvent;

use roshar_ws_mgr::Manager;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;

/// ByBit client that manages WebSocket feeds and REST API
pub struct ByBitClient {
    market_data_handle: MarketDataFeedHandle,
    #[allow(dead_code)]
    market_data_feed_handle: tokio::task::JoinHandle<()>,
    market_api: MarketApi,
}

impl ByBitClient {
    /// Default rate limit for REST API requests (10 requests per second)
    const DEFAULT_REQUESTS_PER_SECOND: u32 = 10;

    pub fn new(ws_manager: Arc<Manager>, channel_size: usize) -> Self {
        Self::new_with_rate_limit(ws_manager, channel_size, Self::DEFAULT_REQUESTS_PER_SECOND)
    }

    pub fn new_with_rate_limit(
        ws_manager: Arc<Manager>,
        channel_size: usize,
        requests_per_second: u32,
    ) -> Self {
        let market_data_feed = MarketDataFeed::new(ws_manager, channel_size);
        let market_data_handle = market_data_feed.get_handle();
        let market_data_feed_handle = tokio::spawn(async move {
            market_data_feed.run().await;
        });
        let market_api = MarketApi::new(requests_per_second);

        Self {
            market_data_handle,
            market_data_feed_handle,
            market_api,
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

    /// Subscribe to depth updates for a symbol
    pub async fn add_depth(&self, symbol: &str) -> Result<(), String> {
        self.market_data_handle.add_depth(symbol).await
    }

    /// Unsubscribe from depth updates for a symbol
    pub async fn remove_depth(&self, symbol: &str) -> Result<(), String> {
        self.market_data_handle.remove_depth(symbol).await
    }

    /// Subscribe to trade updates for a symbol
    pub async fn add_trades(&self, symbol: &str) -> Result<(), String> {
        self.market_data_handle.add_trades(symbol).await
    }

    /// Unsubscribe from trade updates for a symbol
    pub async fn remove_trades(&self, symbol: &str) -> Result<(), String> {
        self.market_data_handle.remove_trades(symbol).await
    }

    /// Get the latest depth for a symbol
    /// Returns None if not subscribed or no data received yet
    pub async fn get_latest_depth(
        &self,
        symbol: &str,
    ) -> Result<Option<roshar_types::OrderBookState>, String> {
        self.market_data_handle.get_latest_depth(symbol).await
    }

    /// Subscribe to candle updates for a symbol
    pub async fn add_candles(&self, symbol: &str) -> Result<(), String> {
        self.market_data_handle.add_candles(symbol).await
    }

    /// Unsubscribe from candle updates for a symbol
    pub async fn remove_candles(&self, symbol: &str) -> Result<(), String> {
        self.market_data_handle.remove_candles(symbol).await
    }

    /// Trigger restart of market data feed
    pub async fn restart_market_data(&self) {
        if let Err(e) = self.market_data_handle.restart_feed().await {
            log::error!(
                "Failed to send restart command to ByBit market data feed: {}",
                e
            );
        }
    }

    /// Get real-time funding rates for all linear perpetual symbols
    ///
    /// Returns ticker data containing funding_rate and next_funding_time for each symbol.
    /// The data is fetched from ByBit's tickers endpoint which provides real-time funding rates.
    pub async fn get_realtime_funding_rates(
        &self,
    ) -> Result<HashMap<String, ByBitTickerData>, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api.get_tickers().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_realtime_funding_rates() {
        let market_api = MarketApi::new(10);
        let result = market_api.get_tickers().await;

        assert!(
            result.is_ok(),
            "Failed to fetch realtime funding rates: {:?}",
            result.err()
        );

        let tickers = result.unwrap();

        // Should have some tickers
        assert!(!tickers.is_empty(), "Expected some tickers, got none");

        // Check that BTCUSDT is present (a known perpetual)
        assert!(
            tickers.contains_key("BTCUSDT"),
            "Expected BTCUSDT ticker to be present"
        );

        // Verify funding rate data is present
        let btc_ticker = tickers.get("BTCUSDT").unwrap();
        assert!(
            !btc_ticker.funding_rate.is_empty(),
            "Expected funding_rate to be present"
        );
        assert!(
            !btc_ticker.next_funding_time.is_empty(),
            "Expected next_funding_time to be present"
        );

        println!(
            "Fetched {} tickers with funding rates. BTCUSDT funding_rate: {}, next_funding_time: {}",
            tickers.len(),
            btc_ticker.funding_rate,
            btc_ticker.next_funding_time
        );
    }
}
