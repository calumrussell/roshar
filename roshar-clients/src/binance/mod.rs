pub mod rest;
pub mod ws;

use rest::BinanceRestClient;
use ws::{MarketDataFeed, MarketDataFeedHandle};

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
    raw_rx: Option<mpsc::Receiver<String>>,
}

impl BinanceClient {
    pub fn new(ws_manager: Arc<Manager>) -> Self {
        let (event_tx, event_rx) = mpsc::channel(10000);
        let (raw_tx, raw_rx) = mpsc::channel(10000);
        let market_data_feed = MarketDataFeed::new(ws_manager, event_tx, raw_tx);
        let market_data_handle = market_data_feed.get_handle();
        let market_data_feed_handle = tokio::spawn(async move {
            market_data_feed.run().await;
        });

        Self {
            market_data_handle,
            market_data_feed_handle,
            event_rx: Some(event_rx),
            raw_rx: Some(raw_rx),
        }
    }

    /// Take the event receiver for reactive market data consumption
    /// Can only be called once - returns None on subsequent calls
    pub fn take_event_receiver(&mut self) -> Option<mpsc::Receiver<MarketEvent>> {
        self.event_rx.take()
    }

    /// Take the raw receiver for raw JSON message consumption
    /// Can only be called once - returns None on subsequent calls
    /// This enables raw mode - no parsing will occur, only raw JSON forwarding
    pub async fn take_raw_receiver(&mut self) -> Result<mpsc::Receiver<String>, String> {
        self.market_data_handle.set_raw_mode(true).await?;
        self.raw_rx
            .take()
            .ok_or_else(|| "Raw receiver already taken".to_string())
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
        BinanceRestClient::new()
            .get_24hr_ticker(symbol)
            .await
            .map_err(|e| format!("Failed to get 24hr ticker: {}", e))
    }
}
