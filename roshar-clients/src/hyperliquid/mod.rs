pub mod rest;
pub mod validator;
pub mod ws;

use ws::{
    BboFeed, BboFeedHandle, FillsFeedHandler, MarketDataFeed, MarketDataFeedHandle,
    OrdersFeedHandler,
};

pub use rest::{ExchangeMetadataHandle, ExchangeMetadataManager};
pub use validator::OrderValidator;
pub use ws::MarketEvent;

use rest::{
    ExchangeApi, ExchangeDataStatus, ExchangeResponseStatus, HyperliquidOrderType, InfoApi,
    ModifyOrderParams,
};
use roshar_types::{AssetInfo, OrderBookState, SpotMarketData, UserPerpetualsState};
use tokio::sync::mpsc;

/// Result of creating an order
#[derive(Debug, Clone)]
pub enum OrderResult {
    /// Order was placed and is resting on the order book
    Resting { order_id: String },
    /// Order filled immediately (e.g., IOC orders)
    Filled {
        order_id: String,
        filled_qty: f64,
        avg_price: f64,
    },
}

/// Configuration for Hyperliquid client
#[derive(Debug, Clone, Copy)]
pub struct HyperliquidConfig {
    pub is_mainnet: bool,
    pub metadata_update_interval_secs: u64,
    pub wallet_address: Option<ethers::types::H160>, // Required for order operations
}

/// Hyperliquid-specific client implementation
pub struct HyperliquidClient {
    api: ExchangeApi,
    wallet_address: Option<ethers::types::H160>,
    validator: OrderValidator,
    #[allow(dead_code)] // Kept to prevent metadata manager task from being dropped
    metadata_manager_handle: tokio::task::JoinHandle<()>,
    metadata_handle: ExchangeMetadataHandle,
    #[allow(dead_code)] // Kept to prevent state manager tasks from being dropped
    perp_state_manager_handle: tokio::task::JoinHandle<()>,
    #[allow(dead_code)] // Kept to prevent state manager tasks from being dropped
    spot_state_manager_handle: tokio::task::JoinHandle<()>,
    perp_state_handle: crate::state_handle::StateHandle,
    spot_state_handle: crate::state_handle::StateHandle,
    is_mainnet: bool,
    // Market data feed
    market_data_handle: MarketDataFeedHandle,
    #[allow(dead_code)] // Kept to prevent market data feed task from being dropped
    market_data_feed_handle: tokio::task::JoinHandle<()>,
    event_rx: Option<mpsc::Receiver<MarketEvent>>,
    raw_rx: Option<mpsc::Receiver<String>>,
    // BBO feed
    bbo_handle: BboFeedHandle,
    #[allow(dead_code)] // Kept to prevent BBO feed task from being dropped
    bbo_feed_handle: tokio::task::JoinHandle<()>,
}

impl HyperliquidClient {
    /// Query spot asset info from metadata manager (for validation/mapping)
    async fn query_spot_asset_info(
        &self,
    ) -> Result<std::collections::HashMap<String, rest::SpotAssetInfo>, String> {
        self.metadata_handle.get_spot_asset_info().await
    }

    /// Query spot market data from metadata manager (for prices)
    async fn query_spot_market_data(
        &self,
    ) -> Result<std::collections::HashMap<String, SpotMarketData>, String> {
        self.metadata_handle.get_spot_market_data().await
    }

    /// Query perp asset info from metadata manager
    async fn query_perp_asset_info(
        &self,
    ) -> Result<std::collections::HashMap<String, AssetInfo>, String> {
        self.metadata_handle.get_perp_asset_info().await
    }

    /// Query funding rates from metadata manager
    async fn query_funding_rates(&self) -> Result<Vec<(String, f64, f64, f64)>, String> {
        self.metadata_handle.get_funding_rates().await
    }

    pub fn new(
        config: HyperliquidConfig,
        ws_manager: std::sync::Arc<roshar_ws_mgr::Manager>,
    ) -> Self {
        let validator = OrderValidator::new();

        let api = if let Some(vault_addr) = config.wallet_address.as_ref() {
            ExchangeApi::new_with_vault(config.is_mainnet, Some(format!("{:?}", vault_addr)))
        } else {
            ExchangeApi::new(config.is_mainnet)
        };

        let (metadata_handle, metadata_manager_handle) =
            ExchangeMetadataManager::spawn(config.metadata_update_interval_secs, config.is_mainnet);

        let wallet_addr = config.wallet_address;
        let is_mainnet = config.is_mainnet;

        let perp_init = if wallet_addr.is_some() {
            Some(move || Self::fetch_perp_positions(wallet_addr, is_mainnet))
        } else {
            None
        };

        let spot_init = if wallet_addr.is_some() {
            Some(move || Self::fetch_spot_positions(wallet_addr, is_mainnet))
        } else {
            None
        };

        let (perp_state_handle, perp_state_manager_handle) =
            crate::state_manager::StateManager::spawn_with_init(perp_init);
        let (spot_state_handle, spot_state_manager_handle) =
            crate::state_manager::StateManager::spawn_with_init(spot_init);

        if let Some(wallet_addr) = config.wallet_address.as_ref() {
            let wallet_address_str = format!("{:?}", wallet_addr);

            let orders_handler = OrdersFeedHandler::new(
                wallet_address_str.clone(),
                perp_state_handle.sender(),
                spot_state_handle.sender(),
                ws_manager.clone(),
                metadata_handle.clone(),
                !config.is_mainnet,
            );
            tokio::spawn(async move {
                orders_handler.run().await;
            });

            // Spawn Fills WebSocket feed handler (routes to both perp and spot managers)
            let fills_handler = FillsFeedHandler::new(
                wallet_address_str,
                perp_state_handle.sender(),
                spot_state_handle.sender(),
                ws_manager.clone(),
                metadata_handle.clone(),
                !config.is_mainnet,
            );
            tokio::spawn(async move {
                fills_handler.run().await;
            });

            log::info!(
                "Spawned Hyperliquid WebSocket feed handlers for wallet: {:?}",
                wallet_addr
            );
        } else {
            log::info!("No wallet address provided - WebSocket feeds not started");
        }

        // Set up BBO feed
        let bbo_feed = BboFeed::new(ws_manager.clone(), !config.is_mainnet);
        let bbo_handle = bbo_feed.get_handle();
        let bbo_feed_handle = tokio::spawn(async move {
            bbo_feed.run().await;
        });

        // Set up market data feed
        let (event_tx, event_rx) = mpsc::channel(10000);
        let (raw_tx, raw_rx) = mpsc::channel(10000);
        let market_data_feed = MarketDataFeed::new(ws_manager, !config.is_mainnet, event_tx, raw_tx);
        let market_data_handle = market_data_feed.get_handle();
        let market_data_feed_handle = tokio::spawn(async move {
            market_data_feed.run().await;
        });

        Self {
            api,
            wallet_address: config.wallet_address,
            validator,
            metadata_manager_handle,
            metadata_handle,
            perp_state_manager_handle,
            spot_state_manager_handle,
            perp_state_handle,
            spot_state_handle,
            is_mainnet: config.is_mainnet,
            market_data_handle,
            market_data_feed_handle,
            event_rx: Some(event_rx),
            raw_rx: Some(raw_rx),
            bbo_handle,
            bbo_feed_handle,
        }
    }

    /// Validate and round an order request
    pub async fn validate_order(
        &self,
        request: validator::OrderRequest,
    ) -> Result<validator::ValidatedOrder, String> {
        // Determine if this is a spot or perp order
        let is_spot = request.hyperliquid_is_spot.unwrap_or(false);

        if is_spot {
            let spot_asset_info = self.query_spot_asset_info().await?;
            self.validator
                .validate_and_round_hyperliquid_spot(request, &spot_asset_info)
        } else {
            let asset_info = self.query_perp_asset_info().await?;
            self.validator
                .validate_and_round_hyperliquid_perps(request, &asset_info)
        }
    }

    /// Get spot ticker for a given perp name
    /// For example: "HYPE" -> Some("@107")
    /// Returns None if no USDC-quoted spot pair exists for the perp
    pub async fn get_usdc_ticker_from_coin(
        &self,
        perp_name: &str,
    ) -> Result<Option<String>, String> {
        let spot_assets = self.query_spot_asset_info().await?;
        const USDC_TOKEN_INDEX: u32 = 0;

        for (_ticker, info) in spot_assets.iter() {
            if info.quote_token.index == USDC_TOKEN_INDEX && info.base_token.name == perp_name {
                return Ok(Some(info.asset.name.clone()));
            }
        }
        Ok(None)
    }

    /// Create a new order
    /// Returns OrderResult on success indicating whether order is resting or filled
    pub async fn create_order(
        &self,
        ticker: &str,
        is_buy: bool,
        limit_px: f64,
        sz: f64,
        reduce_only: bool,
        order_type: HyperliquidOrderType,
    ) -> Result<OrderResult, String> {
        let res = self
            .api
            .create_order(ticker, is_buy, limit_px, sz, reduce_only, order_type)
            .await;

        match res {
            Ok(ExchangeResponseStatus::Ok(data)) => {
                if let Some(first_status) = data.data.and_then(|d| d.statuses.into_iter().next()) {
                    match first_status {
                        ExchangeDataStatus::Error(msg) => {
                            Err(format!("Exchange rejected order: {}", msg))
                        }
                        ExchangeDataStatus::Resting(order) => Ok(OrderResult::Resting {
                            order_id: order.oid.to_string(),
                        }),
                        ExchangeDataStatus::Filled(order) => {
                            // Parse filled quantities and price
                            let filled_qty = order.total_sz.parse::<f64>().unwrap_or(0.0);
                            let avg_price = order.avg_px.parse::<f64>().unwrap_or(0.0);

                            Ok(OrderResult::Filled {
                                order_id: order.oid.to_string(),
                                filled_qty,
                                avg_price,
                            })
                        }
                        ExchangeDataStatus::WaitingForFill => {
                            Err("Order waiting for fill".to_string())
                        }
                        ExchangeDataStatus::WaitingForTrigger => {
                            Err("Order waiting for trigger".to_string())
                        }
                        ExchangeDataStatus::Success => {
                            Err("Order returned success without ID".to_string())
                        }
                    }
                } else {
                    Err("No order status returned".to_string())
                }
            }
            Ok(ExchangeResponseStatus::Err(err)) => Err(format!("Exchange error: {}", err)),
            Err(e) => Err(format!("API error: {:?}", e)),
        }
    }

    /// Cancel an order by order ID
    pub async fn cancel_order(&self, asset: &str, oid: u64) -> Result<(), String> {
        match self.api.cancel_order(asset, oid).await {
            Ok(ExchangeResponseStatus::Ok(_)) => Ok(()),
            Ok(ExchangeResponseStatus::Err(err)) => {
                Err(format!("Failed to cancel order {}: {}", oid, err))
            }
            Err(e) => Err(format!("API error: {:?}", e)),
        }
    }

    /// Modify an existing order
    pub async fn modify_order(
        &self,
        oid: u64,
        asset: &str,
        is_buy: bool,
        limit_px: f64,
        sz: f64,
    ) -> Result<(), String> {
        let params = ModifyOrderParams {
            oid,
            asset: asset.to_string(),
            is_buy,
            limit_px,
            sz,
            reduce_only: false,
            order_type: HyperliquidOrderType::Gtc,
        };

        self.api
            .modify_order(params)
            .await
            .map_err(|e| format!("Failed to modify order: {:?}", e))?;

        Ok(())
    }

    /// Query current position for a ticker from StateManager
    /// Returns actual position and pending orders impact
    pub async fn get_position(
        &self,
        ticker: &str,
    ) -> Result<crate::state_manager::PositionState, String> {
        // Try perp first, then spot
        let perp_state = self.perp_state_handle.get_position(ticker).await?;

        if perp_state.actual.abs() > 1e-10 || perp_state.pending.abs() > 1e-10 {
            Ok(perp_state)
        } else {
            // Try spot
            self.spot_state_handle.get_position(ticker).await
        }
    }

    /// Query perpetual positions only from StateManager
    /// Returns HashMap of perp ticker -> quantity (positive = long, negative = short)
    pub async fn get_perp_positions(
        &self,
    ) -> Result<std::collections::HashMap<String, f64>, String> {
        self.perp_state_handle.get_positions().await
    }

    /// Query spot positions only from StateManager
    /// Returns HashMap of token name -> quantity
    pub async fn get_spot_positions(
        &self,
    ) -> Result<std::collections::HashMap<String, f64>, String> {
        self.spot_state_handle.get_positions().await
    }

    /// Query perp order status by order_id
    pub async fn get_perp_order(
        &self,
        order_id: &str,
    ) -> Result<Option<crate::state_manager::OrderStatus>, String> {
        self.perp_state_handle.get_order(order_id).await
    }

    /// Query spot order status by order_id
    pub async fn get_spot_order(
        &self,
        order_id: &str,
    ) -> Result<Option<crate::state_manager::OrderStatus>, String> {
        self.spot_state_handle.get_order(order_id).await
    }

    /// Check if a perp order is completed (fully filled)
    pub async fn is_perp_order_completed(&self, order_id: &str) -> Result<bool, String> {
        self.perp_state_handle.is_order_completed(order_id).await
    }

    /// Check if a spot order is completed (fully filled)
    pub async fn is_spot_order_completed(&self, order_id: &str) -> Result<bool, String> {
        self.spot_state_handle.is_order_completed(order_id).await
    }

    /// Get pending orders for a perp ticker
    pub async fn get_perp_pending_orders(
        &self,
        ticker: &str,
    ) -> Result<Vec<crate::state_manager::PendingOrderInfo>, String> {
        self.perp_state_handle.get_pending_orders(ticker).await
    }

    /// Get pending orders for a spot ticker
    pub async fn get_spot_pending_orders(
        &self,
        ticker: &str,
    ) -> Result<Vec<crate::state_manager::PendingOrderInfo>, String> {
        self.spot_state_handle.get_pending_orders(ticker).await
    }

    /// Fetch perp positions from exchange
    /// Internal helper function used for StateManager initialization
    async fn fetch_perp_positions(
        wallet_address: Option<ethers::types::H160>,
        is_mainnet: bool,
    ) -> Result<std::collections::HashMap<String, f64>, String> {
        let wallet_addr = wallet_address
            .ok_or_else(|| "Wallet address required for fetching perp positions".to_string())?;

        let info_api = if is_mainnet {
            InfoApi::production()
        } else {
            InfoApi::testnet()
        };

        // Fetch perp positions
        let perp_state = info_api
            .user_perpetuals_state(wallet_addr)
            .await
            .map_err(|e| format!("Failed to fetch perpetuals state: {:?}", e))?;

        let mut perp_positions = std::collections::HashMap::new();
        for position in &perp_state.asset_positions {
            let size = position
                .position
                .szi
                .parse::<f64>()
                .map_err(|e| format!("Failed to parse position size: {}", e))?;
            if size.abs() > 1e-10 {
                perp_positions.insert(position.position.coin.clone(), size);
            }
        }

        Ok(perp_positions)
    }

    /// Fetch spot positions from exchange
    /// Internal helper function used for StateManager initialization
    async fn fetch_spot_positions(
        wallet_address: Option<ethers::types::H160>,
        is_mainnet: bool,
    ) -> Result<std::collections::HashMap<String, f64>, String> {
        let wallet_addr = wallet_address
            .ok_or_else(|| "Wallet address required for fetching spot positions".to_string())?;

        let info_api = if is_mainnet {
            InfoApi::production()
        } else {
            InfoApi::testnet()
        };

        // Fetch spot clearinghouse state and parse balances
        let spot_state = info_api
            .user_spot_state(&format!("{:?}", wallet_addr))
            .await
            .map_err(|e| format!("Failed to fetch spot state: {:?}", e))?;

        let mut spot_balances = std::collections::HashMap::new();
        for balance in &spot_state.balances {
            let balance_qty = balance
                .total
                .parse::<f64>()
                .map_err(|e| format!("Failed to parse balance total: {}", e))?;
            spot_balances.insert(balance.coin.clone(), balance_qty);
        }

        Ok(spot_balances)
    }

    /// Get all funding rates with size data from metadata manager (cached)
    /// Returns Vec of (coin, funding_rate, open_interest, daily_volume)
    pub async fn get_all_funding_rates_with_size(
        &self,
    ) -> Result<Vec<(String, f64, f64, f64)>, String> {
        self.query_funding_rates().await
    }

    /// Get perp asset info from cached metadata
    /// Returns HashMap of perp ticker -> AssetInfo (includes market_data with open_interest, mark_price, day_notional_volume)
    pub async fn get_perp_asset_info(
        &self,
    ) -> Result<std::collections::HashMap<String, AssetInfo>, String> {
        self.query_perp_asset_info().await
    }

    /// Get spot market data from cached metadata
    /// Returns HashMap of spot ticker -> SpotMarketData
    pub async fn get_spot_market_data(
        &self,
    ) -> Result<std::collections::HashMap<String, SpotMarketData>, String> {
        self.query_spot_market_data().await
    }

    /// Get perp mark prices from cached metadata (no REST API call)
    /// Returns HashMap of perp ticker -> mark price
    pub async fn get_perp_prices(&self) -> Result<std::collections::HashMap<String, f64>, String> {
        let asset_info = self.query_perp_asset_info().await?;
        let mut prices = std::collections::HashMap::with_capacity(asset_info.len());

        for (ticker, info) in asset_info {
            if let Some(price_str) = &info.market_data.mark_price {
                if let Ok(price) = price_str.parse::<f64>() {
                    prices.insert(ticker, price);
                }
            }
        }

        Ok(prices)
    }

    /// Get spot mark prices from cached metadata (no REST API call)
    /// Returns HashMap of spot ticker -> mark price
    pub async fn get_spot_prices(&self) -> Result<std::collections::HashMap<String, f64>, String> {
        let spot_market_data = self.query_spot_market_data().await?;
        let mut prices = std::collections::HashMap::with_capacity(spot_market_data.len());

        for (ticker, data) in spot_market_data {
            if let Ok(price) = data.mark_price.parse::<f64>() {
                prices.insert(ticker, price);
            }
        }

        Ok(prices)
    }

    /// Get full user perpetuals state from exchange (REST API call)
    /// Returns complete UserPerpetualsState including positions, margin, and liquidation info
    pub async fn get_user_perpetuals_state(&self) -> Result<UserPerpetualsState, String> {
        let wallet_address = self
            .wallet_address
            .ok_or_else(|| "Wallet address required for get_user_perpetuals_state".to_string())?;

        // Create InfoApi to query account state
        let info_api = if self.is_mainnet {
            InfoApi::production()
        } else {
            InfoApi::testnet()
        };

        // Fetch user perpetuals state
        info_api
            .user_perpetuals_state(wallet_address)
            .await
            .map_err(|e| format!("Failed to fetch perpetuals state: {:?}", e))
    }

    /// Get maximum leverage allowed for a specific coin
    /// Returns the maxLeverage from exchange metadata
    pub async fn get_max_leverage(&self, coin: &str) -> Result<u32, String> {
        let asset_info = self.query_perp_asset_info().await?;

        asset_info
            .get(coin)
            .map(|info| info.asset.max_leverage)
            .ok_or_else(|| format!("No asset info found for {}", coin))
    }

    /// Update leverage for a specific asset
    /// Returns Ok(()) if successful, Err if failed
    pub async fn update_leverage(
        &self,
        leverage: u32,
        coin: &str,
        is_cross: bool,
    ) -> Result<(), String> {
        self.api
            .update_leverage(leverage, coin, is_cross)
            .await
            .map_err(|e| format!("Failed to update leverage: {:?}", e))
            .and_then(|status| match status {
                ExchangeResponseStatus::Ok(_) => Ok(()),
                ExchangeResponseStatus::Err(msg) => {
                    Err(format!("Exchange returned error: {}", msg))
                }
            })
    }

    /// Start BBO subscription for a ticker (idempotent)
    /// The BBO state will be maintained in the background
    pub async fn create_bbo_subscription(&self, ticker: &str) -> Result<(), String> {
        self.bbo_handle.add_subscription(ticker).await
    }

    /// Remove BBO subscription for a ticker
    pub async fn remove_bbo_subscription(&self, ticker: &str) -> Result<(), String> {
        self.bbo_handle.remove_subscription(ticker).await
    }

    /// Get the latest BBO (best bid/offer) for a ticker
    /// Returns None if not subscribed or no data received yet
    pub async fn get_latest_bbo(&self, ticker: &str) -> Result<Option<(f64, f64)>, String> {
        self.bbo_handle.get_latest_bbo(ticker).await
    }

    /// Start candle subscription for a coin (idempotent)
    /// Candle updates will be delivered via the MarketEvent receiver
    pub async fn add_candles(&self, coin: &str) -> Result<(), String> {
        self.market_data_handle.add_candles(coin).await
    }

    /// Remove candle subscription for a coin
    pub async fn remove_candles(&self, coin: &str) -> Result<(), String> {
        self.market_data_handle.remove_candles(coin).await
    }

    /// Get historical funding rates for a coin
    /// Returns funding rate history from start_time to end_time (or now if None)
    pub async fn get_historical_funding_rates(
        &self,
        coin: &str,
        start_time: u64,
        end_time: Option<u64>,
    ) -> Result<Vec<roshar_types::HistoricalFundingRate>, String> {
        let info_api = if self.is_mainnet {
            InfoApi::production()
        } else {
            InfoApi::testnet()
        };

        info_api
            .get_historical_funding_rates(coin, start_time, end_time)
            .await
            .map_err(|e| format!("Failed to fetch historical funding rates: {:?}", e))
    }

    /// Get all pending perp orders across all tickers
    pub async fn get_all_perp_pending_orders(
        &self,
    ) -> Result<Vec<crate::state_manager::PendingOrderInfo>, String> {
        self.perp_state_handle.get_all_pending_orders().await
    }

    /// Get all pending spot orders across all tickers
    pub async fn get_all_spot_pending_orders(
        &self,
    ) -> Result<Vec<crate::state_manager::PendingOrderInfo>, String> {
        self.spot_state_handle.get_all_pending_orders().await
    }

    /// Start depth subscription for a coin (idempotent)
    pub async fn add_depth(&self, coin: &str) -> Result<(), String> {
        self.market_data_handle.add_depth(coin).await
    }

    /// Remove depth subscription for a coin
    pub async fn remove_depth(&self, coin: &str) -> Result<(), String> {
        self.market_data_handle.remove_depth(coin).await
    }

    /// Get the current order book state for a coin
    /// Returns None if not subscribed or no data received yet
    pub async fn get_latest_depth(&self, coin: &str) -> Result<Option<OrderBookState>, String> {
        self.market_data_handle.get_latest_depth(coin).await
    }

    /// Start trades subscription for a coin (idempotent)
    pub async fn add_trades(&self, coin: &str) -> Result<(), String> {
        self.market_data_handle.add_trades(coin).await
    }

    /// Remove trades subscription for a coin
    pub async fn remove_trades(&self, coin: &str) -> Result<(), String> {
        self.market_data_handle.remove_trades(coin).await
    }

    /// Take the event receiver for reactive market data consumption
    /// Can only be called once - returns None on subsequent calls
    pub fn take_event_receiver(&mut self) -> Option<mpsc::Receiver<MarketEvent>> {
        self.event_rx.take()
    }

    /// Take the raw receiver for raw JSON message consumption
    /// Can only be called once - returns None on subsequent calls
    /// This enables raw mode - no parsing will occur, only raw JSON forwarding
    /// This is useful for stream writers that need to forward messages to Redis/etc
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
}
