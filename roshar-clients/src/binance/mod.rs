pub mod rest;
pub mod ws;

use rest::BinanceRestClient;
use ws::MarketDataFeedHandle;

pub use ws::MarketEvent;
pub(crate) use ws::MarketDataFeed;

use roshar_ws_mgr::Manager;
use std::sync::Arc;
use tokio::sync::mpsc;

/// Binance client that manages WebSocket feeds
pub struct BinanceClient {
    market_data_handle: MarketDataFeedHandle,
    #[allow(dead_code)]
    market_data_feed_handle: tokio::task::JoinHandle<()>,
}

impl BinanceClient {
    pub fn new(ws_manager: Arc<Manager>, channel_size: usize) -> Self {
        let market_data_feed = MarketDataFeed::new(ws_manager, channel_size);
        let market_data_handle = market_data_feed.get_handle();
        let market_data_feed_handle = tokio::spawn(async move {
            market_data_feed.run().await;
        });

        Self {
            market_data_handle,
            market_data_feed_handle,
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
            log::error!("Failed to send restart command to Binance market data feed: {}", e);
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
        BinanceRestClient::new()
            .get_24hr_ticker(symbol)
            .await
            .map_err(|e| format!("Failed to get 24hr ticker: {}", e))
    }
}
