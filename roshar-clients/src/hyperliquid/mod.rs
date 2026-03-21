pub(crate) mod rest;
pub mod validator;

pub use rest::{ExchangeMetadataHandle, ExchangeMetadataManager};
pub use validator::OrderValidator;

use crate::http::RateLimitedClient;
use rest::{
    ExchangeApi, ExchangeDataStatus, ExchangeResponseStatus, HyperliquidOrderType, InfoApi,
    ModifyOrderParams,
};

use anyhow::Result;
use async_trait::async_trait;
use roshar_types::{
    AssetInfo, HyperliquidWssMessage, SpotMarketData, UserOrder, UserPerpetualsState,
};
use roshar_ws_mgr::{Manager, Message};
use std::sync::Arc;

use crate::{HL_TESTNET_WSS_URL, HL_WSS_URL};

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
    /// Order partially filled on placement, remainder is resting on the book
    PartialFill {
        order_id: String,
        filled_qty: f64,
        avg_price: f64,
        remaining_qty: f64,
    },
}

/// Configuration for Hyperliquid client
#[derive(Debug, Clone, Copy)]
pub struct HyperliquidConfig {
    pub is_mainnet: bool,
    pub metadata_update_interval_secs: u64,
    pub wallet_address: Option<ethers::types::H160>, // Required for order operations
    pub rate_limit_refill: u32,                      // Number of tokens to add per interval
    pub rate_limit_interval_secs: u64,               // Interval in seconds between token refills
}

/// Hyperliquid-specific client implementation
pub struct HyperliquidClient {
    api: ExchangeApi,
    wallet_address: Option<ethers::types::H160>,
    validator: OrderValidator,
    #[allow(dead_code)] // Kept to prevent metadata manager task from being dropped
    metadata_manager_handle: tokio::task::JoinHandle<()>,
    metadata_handle: ExchangeMetadataHandle,
    info_api: InfoApi,
    ws_config: roshar_ws_mgr::Config,
}

impl HyperliquidClient {
    const WS_CONN_NAME: &str = "hyperliquid";

    crate::ws::ws_config_methods!();

    pub fn subscribe_depth(&self, manager: &Arc<Manager>, coins: &[&str]) -> Result<()> {
        for coin in coins {
            let msg = HyperliquidWssMessage::l2_book(coin).to_json();
            manager.write(
                &self.ws_config.name,
                Message::TextMessage(self.ws_config.name.clone(), msg),
            )?;
        }
        Ok(())
    }

    pub fn subscribe_trades(&self, manager: &Arc<Manager>, coins: &[&str]) -> Result<()> {
        for coin in coins {
            let msg = HyperliquidWssMessage::trades(coin).to_json();
            manager.write(
                &self.ws_config.name,
                Message::TextMessage(self.ws_config.name.clone(), msg),
            )?;
        }
        Ok(())
    }

    pub fn subscribe_candles(&self, manager: &Arc<Manager>, coins: &[&str]) -> Result<()> {
        for coin in coins {
            let msg = HyperliquidWssMessage::candle(coin).to_json();
            manager.write(
                &self.ws_config.name,
                Message::TextMessage(self.ws_config.name.clone(), msg),
            )?;
        }
        Ok(())
    }

    pub fn subscribe_bbo(&self, manager: &Arc<Manager>, coins: &[&str]) -> Result<()> {
        for coin in coins {
            let msg = HyperliquidWssMessage::bbo(coin).to_json();
            manager.write(
                &self.ws_config.name,
                Message::TextMessage(self.ws_config.name.clone(), msg),
            )?;
        }
        Ok(())
    }

    pub fn subscribe_user_fills(&self, manager: &Arc<Manager>, user_address: &str) -> Result<()> {
        let msg = HyperliquidWssMessage::user_fills(user_address).to_json();
        manager.write(
            &self.ws_config.name,
            Message::TextMessage(self.ws_config.name.clone(), msg),
        )?;
        Ok(())
    }

    pub fn subscribe_order_updates(
        &self,
        manager: &Arc<Manager>,
        user_address: &str,
    ) -> Result<()> {
        let msg = HyperliquidWssMessage::order_updates(user_address).to_json();
        manager.write(
            &self.ws_config.name,
            Message::TextMessage(self.ws_config.name.clone(), msg),
        )?;
        Ok(())
    }

    pub fn unsubscribe_depth(&self, manager: &Arc<Manager>, coins: &[&str]) -> Result<()> {
        for coin in coins {
            let msg = HyperliquidWssMessage::l2_book_unsub(coin).to_json();
            manager.write(
                &self.ws_config.name,
                Message::TextMessage(self.ws_config.name.clone(), msg),
            )?;
        }
        Ok(())
    }

    pub fn unsubscribe_trades(&self, manager: &Arc<Manager>, coins: &[&str]) -> Result<()> {
        for coin in coins {
            let msg = HyperliquidWssMessage::trades_unsub(coin).to_json();
            manager.write(
                &self.ws_config.name,
                Message::TextMessage(self.ws_config.name.clone(), msg),
            )?;
        }
        Ok(())
    }

    pub fn unsubscribe_candles(&self, manager: &Arc<Manager>, coins: &[&str]) -> Result<()> {
        for coin in coins {
            let msg = HyperliquidWssMessage::candle_unsub(coin).to_json();
            manager.write(
                &self.ws_config.name,
                Message::TextMessage(self.ws_config.name.clone(), msg),
            )?;
        }
        Ok(())
    }

    pub fn unsubscribe_bbo(&self, manager: &Arc<Manager>, coins: &[&str]) -> Result<()> {
        for coin in coins {
            let msg = HyperliquidWssMessage::bbo_unsub(coin).to_json();
            manager.write(
                &self.ws_config.name,
                Message::TextMessage(self.ws_config.name.clone(), msg),
            )?;
        }
        Ok(())
    }

    pub fn unsubscribe_user_fills(&self, manager: &Arc<Manager>, user_address: &str) -> Result<()> {
        let msg = HyperliquidWssMessage::user_fills_unsub(user_address).to_json();
        manager.write(
            &self.ws_config.name,
            Message::TextMessage(self.ws_config.name.clone(), msg),
        )?;
        Ok(())
    }

    pub fn unsubscribe_order_updates(
        &self,
        manager: &Arc<Manager>,
        user_address: &str,
    ) -> Result<()> {
        let msg = HyperliquidWssMessage::order_updates_unsub(user_address).to_json();
        manager.write(
            &self.ws_config.name,
            Message::TextMessage(self.ws_config.name.clone(), msg),
        )?;
        Ok(())
    }

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

    pub fn new(config: HyperliquidConfig) -> Self {
        let validator = OrderValidator::new();

        let api = if let Some(vault_addr) = config.wallet_address.as_ref() {
            ExchangeApi::new_with_vault(config.is_mainnet, Some(format!("{:?}", vault_addr)))
        } else {
            ExchangeApi::new(config.is_mainnet)
        };

        // Create a single shared rate-limited HTTP client for ALL Hyperliquid REST API calls.
        let http_client = Arc::new(RateLimitedClient::new(
            config.rate_limit_refill,
            config.rate_limit_interval_secs,
        ));

        let (metadata_handle, metadata_manager_handle) = ExchangeMetadataManager::spawn(
            config.metadata_update_interval_secs,
            config.is_mainnet,
            http_client.clone(),
        );

        // Create InfoApi using the shared rate-limited client
        let info_api = if config.is_mainnet {
            InfoApi::production_with_client(http_client)
        } else {
            InfoApi::testnet_with_client(http_client)
        };

        let ws_url = if config.is_mainnet {
            HL_WSS_URL
        } else {
            HL_TESTNET_WSS_URL
        };

        Self {
            api,
            wallet_address: config.wallet_address,
            validator,
            metadata_manager_handle,
            metadata_handle,
            info_api,
            ws_config: roshar_ws_mgr::Config {
                name: Self::WS_CONN_NAME.to_string(),
                url: ws_url.to_string(),
                ping_duration: 20,
                ping_message: HyperliquidWssMessage::ping().to_json(),
                ping_timeout: 10,
                reconnect_timeout: 5000,
                use_text_ping: Some(true),
                read_buffer_size: None,
                write_buffer_size: None,
                max_message_size: None,
                max_frame_size: None,
                tcp_recv_buffer_size: None,
                tcp_send_buffer_size: None,
                tcp_nodelay: None,
                broadcast_channel_size: None,
            },
        }
    }
}

#[async_trait]
pub trait HyperliquidApi {
    async fn validate_order(
        &self,
        request: validator::OrderRequest,
    ) -> Result<validator::ValidatedOrder, String>;
    async fn get_usdc_ticker_from_coin(&self, perp_name: &str) -> Result<Option<String>, String>;
    async fn create_order(
        &self,
        ticker: &str,
        is_buy: bool,
        limit_px: f64,
        sz: f64,
        reduce_only: bool,
        order_type: HyperliquidOrderType,
    ) -> Result<OrderResult, String>;
    async fn cancel_order(&self, asset: &str, oid: u64) -> Result<(), String>;
    async fn modify_order(
        &self,
        oid: u64,
        asset: &str,
        is_buy: bool,
        limit_px: f64,
        sz: f64,
    ) -> Result<(), String>;
    async fn get_all_funding_rates_with_size(&self)
        -> Result<Vec<(String, f64, f64, f64)>, String>;
    async fn get_perp_asset_info(
        &self,
    ) -> Result<std::collections::HashMap<String, AssetInfo>, String>;
    async fn get_spot_market_data(
        &self,
    ) -> Result<std::collections::HashMap<String, SpotMarketData>, String>;
    async fn get_perp_prices(&self) -> Result<std::collections::HashMap<String, f64>, String>;
    async fn get_spot_prices(&self) -> Result<std::collections::HashMap<String, f64>, String>;
    async fn get_user_perpetuals_state(&self) -> Result<UserPerpetualsState, String>;
    async fn get_max_leverage(&self, coin: &str) -> Result<u32, String>;
    async fn update_leverage(
        &self,
        leverage: u32,
        coin: &str,
        is_cross: bool,
    ) -> Result<(), String>;
    async fn get_candle_snapshot(
        &self,
        coin: &str,
        interval: &str,
        start_time: u64,
        end_time: u64,
    ) -> Result<Vec<roshar_types::HyperliquidCandleData>, String>;
    async fn get_historical_funding_rates(
        &self,
        coin: &str,
        start_time: u64,
        end_time: Option<u64>,
    ) -> Result<Vec<roshar_types::HistoricalFundingRate>, String>;
    async fn get_open_orders(&self) -> Result<Vec<UserOrder>, String>;
}

#[async_trait]
impl HyperliquidApi for HyperliquidClient {
    /// Validate and round an order request
    async fn validate_order(
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
    async fn get_usdc_ticker_from_coin(&self, perp_name: &str) -> Result<Option<String>, String> {
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
    async fn create_order(
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

                            let remaining_qty = sz - filled_qty;
                            if remaining_qty > 1e-10 {
                                // GTC order partially filled, remainder resting on book
                                Ok(OrderResult::PartialFill {
                                    order_id: order.oid.to_string(),
                                    filled_qty,
                                    avg_price,
                                    remaining_qty,
                                })
                            } else {
                                Ok(OrderResult::Filled {
                                    order_id: order.oid.to_string(),
                                    filled_qty,
                                    avg_price,
                                })
                            }
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
    async fn cancel_order(&self, asset: &str, oid: u64) -> Result<(), String> {
        match self.api.cancel_order(asset, oid).await {
            Ok(ExchangeResponseStatus::Ok(_)) => Ok(()),
            Ok(ExchangeResponseStatus::Err(err)) => {
                Err(format!("Failed to cancel order {}: {}", oid, err))
            }
            Err(e) => Err(format!("API error: {:?}", e)),
        }
    }

    /// Modify an existing order
    async fn modify_order(
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

    /// Get all funding rates with size data from metadata manager (cached)
    /// Returns Vec of (coin, funding_rate, open_interest, daily_volume)
    async fn get_all_funding_rates_with_size(
        &self,
    ) -> Result<Vec<(String, f64, f64, f64)>, String> {
        self.query_funding_rates().await
    }

    /// Get perp asset info from cached metadata
    /// Returns HashMap of perp ticker -> AssetInfo (includes market_data with open_interest, mark_price, day_notional_volume)
    async fn get_perp_asset_info(
        &self,
    ) -> Result<std::collections::HashMap<String, AssetInfo>, String> {
        self.query_perp_asset_info().await
    }

    /// Get spot market data from cached metadata
    /// Returns HashMap of spot ticker -> SpotMarketData
    async fn get_spot_market_data(
        &self,
    ) -> Result<std::collections::HashMap<String, SpotMarketData>, String> {
        self.query_spot_market_data().await
    }

    /// Get perp mark prices from cached metadata (no REST API call)
    /// Returns HashMap of perp ticker -> mark price
    async fn get_perp_prices(&self) -> Result<std::collections::HashMap<String, f64>, String> {
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
    async fn get_spot_prices(&self) -> Result<std::collections::HashMap<String, f64>, String> {
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
    async fn get_user_perpetuals_state(&self) -> Result<UserPerpetualsState, String> {
        let wallet_address = self
            .wallet_address
            .ok_or_else(|| "Wallet address required for get_user_perpetuals_state".to_string())?;

        // Fetch user perpetuals state
        self.info_api
            .user_perpetuals_state(wallet_address)
            .await
            .map_err(|e| format!("Failed to fetch perpetuals state: {:?}", e))
    }

    /// Get maximum leverage allowed for a specific coin
    /// Returns the maxLeverage from exchange metadata
    async fn get_max_leverage(&self, coin: &str) -> Result<u32, String> {
        let asset_info = self.query_perp_asset_info().await?;

        asset_info
            .get(coin)
            .map(|info| info.asset.max_leverage)
            .ok_or_else(|| format!("No asset info found for {}", coin))
    }

    /// Update leverage for a specific asset
    /// Returns Ok(()) if successful, Err if failed
    async fn update_leverage(
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

    /// Get candle snapshot for a coin
    async fn get_candle_snapshot(
        &self,
        coin: &str,
        interval: &str,
        start_time: u64,
        end_time: u64,
    ) -> Result<Vec<roshar_types::HyperliquidCandleData>, String> {
        self.info_api
            .get_candle_snapshot(coin, interval, start_time, end_time)
            .await
            .map_err(|e| format!("Failed to fetch candle snapshot: {:?}", e))
    }

    /// Get historical funding rates for a coin
    /// Returns funding rate history from start_time to end_time (or now if None)
    async fn get_historical_funding_rates(
        &self,
        coin: &str,
        start_time: u64,
        end_time: Option<u64>,
    ) -> Result<Vec<roshar_types::HistoricalFundingRate>, String> {
        self.info_api
            .get_historical_funding_rates(coin, start_time, end_time)
            .await
            .map_err(|e| format!("Failed to fetch historical funding rates: {:?}", e))
    }

    async fn get_open_orders(&self) -> Result<Vec<UserOrder>, String> {
        let wallet_address = self
            .wallet_address
            .ok_or_else(|| "Wallet address required for get_open_orders".to_string())?;

        self.info_api
            .open_orders(wallet_address)
            .await
            .map_err(|e| format!("Failed to fetch open orders: {:?}", e))
    }
}
