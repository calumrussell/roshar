pub mod rest;
pub mod ws;

use ws::{MarketDataFeed, MarketDataFeedHandle};

pub use rest::BinanceRestClient;
pub use ws::MarketEvent;

use roshar_ws_mgr::Manager;
use std::sync::Arc;
use tokio::sync::mpsc;

/// Binance client that manages WebSocket feeds
pub struct BinanceClient {
    market_data_handle: MarketDataFeedHandle,
    #[allow(dead_code)]
    market_data_feed_handle: tokio::task::JoinHandle<()>,
    event_rx: Option<mpsc::Receiver<MarketEvent>>,
}

impl BinanceClient {
    pub fn new(ws_manager: Arc<Manager>) -> Self {
        let (event_tx, event_rx) = mpsc::channel(10000);
        let market_data_feed = MarketDataFeed::new(ws_manager, event_tx);
        let market_data_handle = market_data_feed.get_handle();
        let market_data_feed_handle = tokio::spawn(async move {
            market_data_feed.run().await;
        });

        Self {
            market_data_handle,
            market_data_feed_handle,
            event_rx: Some(event_rx),
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

    /// Subscribe to candle updates for a symbol
    pub async fn add_candles(&self, symbol: &str) -> Result<(), String> {
        self.market_data_handle.add_candles(symbol).await
    }

    /// Unsubscribe from candle updates for a symbol
    pub async fn remove_candles(&self, symbol: &str) -> Result<(), String> {
        self.market_data_handle.remove_candles(symbol).await
    }

    /// Get the latest depth for a symbol
    /// Returns None if not subscribed or no data received yet
    pub async fn get_latest_depth(
        &self,
        symbol: &str,
    ) -> Result<Option<roshar_types::OrderBookState>, String> {
        self.market_data_handle.get_latest_depth(symbol).await
    }
}
