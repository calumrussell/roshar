pub mod rest;
pub mod ws;

use ws::{CandleFeed, CandleFeedHandle, MarketDataFeed, MarketDataFeedHandle};

pub use rest::BinanceRestClient;
pub use ws::MarketEvent;

use roshar_ws_mgr::Manager;
use std::sync::Arc;
use tokio::sync::mpsc;

/// Binance client that manages WebSocket feeds
pub struct BinanceClient {
    // Market data feed
    market_data_handle: MarketDataFeedHandle,
    #[allow(dead_code)]
    market_data_feed_handle: tokio::task::JoinHandle<()>,
    event_rx: Option<mpsc::Receiver<MarketEvent>>,
    // Candle feed
    candle_handle: CandleFeedHandle,
    #[allow(dead_code)]
    candle_feed_handle: tokio::task::JoinHandle<()>,
}

impl BinanceClient {
    pub fn new(ws_manager: Arc<Manager>) -> Self {
        // Set up market data feed
        let (event_tx, event_rx) = mpsc::channel(10000);
        let market_data_feed = MarketDataFeed::new(ws_manager.clone(), event_tx);
        let market_data_handle = market_data_feed.get_handle();
        let market_data_feed_handle = tokio::spawn(async move {
            market_data_feed.run().await;
        });

        // Set up candle feed
        let candle_feed = CandleFeed::new(ws_manager);
        let candle_handle = candle_feed.get_handle();
        let candle_feed_handle = tokio::spawn(async move {
            candle_feed.run().await;
        });

        Self {
            market_data_handle,
            market_data_feed_handle,
            event_rx: Some(event_rx),
            candle_handle,
            candle_feed_handle,
        }
    }

    /// Take the event receiver for reactive market data consumption
    /// Can only be called once - returns None on subsequent calls
    pub fn take_event_receiver(&mut self) -> Option<mpsc::Receiver<MarketEvent>> {
        self.event_rx.take()
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

    /// Start candle subscription for a symbol (idempotent)
    pub async fn create_candle_subscription(&self, symbol: &str) -> Result<(), String> {
        self.candle_handle.add_subscription(symbol).await
    }

    /// Remove candle subscription for a symbol
    pub async fn remove_candle_subscription(&self, symbol: &str) -> Result<(), String> {
        self.candle_handle.remove_subscription(symbol).await
    }

    /// Get the latest candle for a symbol
    /// Returns None if not subscribed or no data received yet
    pub async fn get_latest_candle(
        &self,
        symbol: &str,
    ) -> Result<Option<roshar_types::Candle>, String> {
        self.candle_handle.get_latest_candle(symbol).await
    }
}
