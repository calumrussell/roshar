pub mod rest;
pub mod ws;

use ws::MarketDataFeedHandle;

pub use rest::{
    ByBitCreateOrderRequest, ByBitCreateOrderResponse, ByBitOrderResult, ByBitTickerData,
    ByBitTickersResponse,
};
pub use ws::MarketEvent;
pub(crate) use ws::MarketDataFeed;

use roshar_ws_mgr::Manager;
use std::sync::Arc;
use tokio::sync::mpsc;

/// ByBit client that manages WebSocket feeds
pub struct ByBitClient {
    market_data_handle: MarketDataFeedHandle,
    #[allow(dead_code)]
    market_data_feed_handle: tokio::task::JoinHandle<()>,
}

impl ByBitClient {
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
            log::error!("Failed to send restart command to ByBit market data feed: {}", e);
        }
    }
}
