pub(crate) mod rest;
pub(crate) mod ws;

use std::collections::HashMap;
use ws::MarketDataFeedHandle;

pub use rest::{
    ByBitCreateOrderRequest, ByBitCreateOrderResponse, ByBitInstrumentInfo, ByBitLeverageFilter,
    ByBitLotSizeFilter, ByBitOrderResult, ByBitPriceFilter, ByBitTickerData, ByBitTickersResponse,
    MarketApi as ByBitMarketApi,
};
use rest::MarketApi;
pub use roshar_types::ByBitHistoricalFundingRate;
pub(crate) use ws::MarketDataFeed;
pub use ws::MarketEvent;

use roshar_ws_mgr::Manager;
use std::sync::Arc;
use tokio::sync::mpsc;

/// Simplified funding rate information extracted from ticker data
#[derive(Debug, Clone)]
pub struct FundingRateInfo {
    pub symbol: String,
    pub funding_rate: String,
    pub next_funding_time: String,
}

/// ByBit client that manages WebSocket feeds and REST API access
pub struct ByBitClient {
    market_data_handle: MarketDataFeedHandle,
    #[allow(dead_code)]
    market_data_feed_handle: tokio::task::JoinHandle<()>,
    market_api: MarketApi,
}

impl ByBitClient {
    pub fn new(ws_manager: Arc<Manager>, channel_size: usize, market_api: MarketApi) -> Self {
        let market_data_feed = MarketDataFeed::new(ws_manager, channel_size);
        let market_data_handle = market_data_feed.get_handle();
        let market_data_feed_handle = tokio::spawn(async move {
            market_data_feed.run().await;
        });

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

    /// Get all linear perpetual tickers
    /// Returns a map of symbol -> ticker data
    pub async fn get_tickers(
        &self,
    ) -> Result<HashMap<String, ByBitTickerData>, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api.get_tickers().await
    }

    /// Get real-time funding rates for all linear perpetuals
    /// Returns a simplified map of symbol -> funding rate info
    pub async fn get_funding_rates(
        &self,
    ) -> Result<HashMap<String, FundingRateInfo>, Box<dyn std::error::Error + Send + Sync>> {
        let tickers = self.market_api.get_tickers().await?;

        let funding_rates: HashMap<String, FundingRateInfo> = tickers
            .into_iter()
            .map(|(symbol, ticker)| {
                (
                    symbol.clone(),
                    FundingRateInfo {
                        symbol,
                        funding_rate: ticker.funding_rate,
                        next_funding_time: ticker.next_funding_time,
                    },
                )
            })
            .collect();

        Ok(funding_rates)
    }
}
