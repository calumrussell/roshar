pub(crate) mod rest;
pub(crate) mod ws;

use rest::MarketApi;
use ws::MarketDataFeedHandle;

pub use rest::{OkxInstrumentsResponse, OkxTickerData, OkxTickersResponse};
pub use rest::MarketApi as OkxMarketApi;
pub(crate) use ws::MarketDataFeed;
pub use ws::MarketEvent;

use roshar_ws_mgr::Manager;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};

/// OKX client that manages WebSocket feeds and REST API.
pub struct OkxClient {
    market_data_handle: MarketDataFeedHandle,
    #[allow(dead_code)]
    market_data_feed_handle: tokio::task::JoinHandle<()>,
    market_api: MarketApi,
}

impl OkxClient {
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
    pub async fn take_event_receiver(
        &self,
    ) -> Result<broadcast::Receiver<MarketEvent>, String> {
        self.market_data_handle.get_event_channel().await
    }

    /// Get the raw receiver for raw JSON message consumption
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
    pub async fn get_latest_depth(
        &self,
        symbol: &str,
    ) -> Result<Option<roshar_types::OrderBookState>, String> {
        self.market_data_handle.get_latest_depth(symbol).await
    }

    /// Trigger restart of market data feed
    pub async fn restart_market_data(&self) {
        if let Err(e) = self.market_data_handle.restart_feed().await {
            log::error!(
                "Failed to send restart command to OKX market data feed: {}",
                e
            );
        }
    }

    /// Get all live SWAP (perpetual) instruments info.
    pub async fn get_instruments_info(
        &self,
    ) -> Result<
        HashMap<String, roshar_types::OkxInstrumentInfo>,
        Box<dyn std::error::Error + Send + Sync>,
    > {
        self.market_api.get_instruments_info().await
    }

    /// Get tickers for all SWAP (perpetual) instruments.
    pub async fn get_tickers(
        &self,
    ) -> Result<HashMap<String, OkxTickerData>, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api.get_tickers().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_okx_client_instruments() {
        let market_api = MarketApi::new(10);
        let result = market_api.get_instruments_info().await;

        assert!(
            result.is_ok(),
            "Failed to fetch OKX instruments: {:?}",
            result.err()
        );

        let instruments = result.unwrap();
        assert!(
            !instruments.is_empty(),
            "Expected some instruments, got none"
        );

        println!("OKX client fetched {} SWAP instruments", instruments.len());
    }
}
