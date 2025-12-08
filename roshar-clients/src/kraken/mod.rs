pub mod rest;
pub mod ws;

use ws::{MarketDataFeed, MarketDataFeedHandle};

use rest::{ChartsApi, MarketApi};
pub use rest::{
    KrakenGetLeverageResponse, KrakenLeveragePreference, KrakenLeverageSettingResponse,
    KrakenModifyResponse, KrakenOpenOrdersResponse, KrakenOrder, KrakenOrderResponse,
    KrakenOrderStatusResponse, KrakenRestCandleData, KrakenRestCandleResponse, KrakenTickerData,
    MultiCollateralApi, OrderManagementApi,
};
pub use ws::MarketEvent;

use roshar_types::Candle;
use roshar_ws_mgr::Manager;
use std::sync::Arc;
use tokio::sync::mpsc;

/// Kraken client that manages WebSocket feeds and REST API calls
pub struct KrakenClient {
    // Market data feed (WebSocket)
    market_data_handle: MarketDataFeedHandle,
    #[allow(dead_code)]
    market_data_feed_handle: tokio::task::JoinHandle<()>,
    // Event receiver
    event_rx: Option<mpsc::Receiver<MarketEvent>>,
    raw_rx: Option<mpsc::Receiver<String>>,
}

impl KrakenClient {
    pub fn new(ws_manager: Arc<Manager>) -> Self {
        // Create event channel
        let (event_tx, event_rx) = mpsc::channel(10000);
        let (raw_tx, raw_rx) = mpsc::channel(10000);

        // Set up WebSocket market data feed
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

    /// Trigger restart of market data feed
    pub async fn restart_market_data(&self) {
        self.market_data_handle.restart_feed().await;
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

    /// Fetch candles for a symbol directly from the exchange
    /// Returns the most recent completed 1-minute candle
    pub async fn fetch_candles(&self, symbol: &str) -> Result<Vec<Candle>, String> {
        ChartsApi::fetch_candle(symbol)
            .await
            .map_err(|e| format!("Failed to fetch candles: {}", e))
    }

    /// Get the latest depth for a symbol
    /// Returns None if not subscribed or no data received yet
    pub async fn get_latest_depth(
        &self,
        symbol: &str,
    ) -> Result<Option<roshar_types::OrderBookState>, String> {
        self.market_data_handle.get_latest_depth(symbol).await
    }

    /// Get ticker data for all Kraken futures
    pub async fn get_tickers(
        &self,
    ) -> Result<std::collections::HashMap<String, KrakenTickerData>, String> {
        MarketApi::get_tickers()
            .await
            .map_err(|e| format!("Failed to get tickers: {}", e))
    }

    /// Get all funding rates with size data
    /// Returns Vec of (symbol, funding_rate, open_interest_usd, volume_usd)
    pub async fn get_all_funding_rates_with_size(
        &self,
    ) -> Result<Vec<(String, f64, f64, f64)>, String> {
        MarketApi::get_all_funding_rates_with_size()
            .await
            .map_err(|e| format!("Failed to get funding rates: {}", e))
    }
}
