pub(crate) mod rest;

pub use rest::MarketApi as CoinbaseMarketApi;
pub use rest::{CoinbaseProduct, CoinbaseProductsResponse};

use anyhow::Result;
use async_trait::async_trait;
use roshar_types::CoinbaseWssMessage;
use roshar_ws_mgr::{Config, Manager, Message};
use std::collections::HashMap;
use std::sync::Arc;

use crate::ws::ws_config_methods;
use crate::COINBASE_WSS_URL;

pub struct CoinbaseClient {
    market_api: rest::MarketApi,
    ws_config: Config,
}

impl CoinbaseClient {
    /// Coinbase public endpoints are throttled at 10 requests/sec by IP.
    const DEFAULT_REQUESTS_PER_SECOND: u32 = 10;
    const WS_CONN_NAME: &str = "coinbase";

    pub fn new() -> Self {
        Self::new_with_rate_limit(Self::DEFAULT_REQUESTS_PER_SECOND)
    }

    pub fn new_with_rate_limit(requests_per_second: u32) -> Self {
        Self {
            market_api: rest::MarketApi::new(requests_per_second),
            ws_config: Config {
                name: Self::WS_CONN_NAME.to_string(),
                url: COINBASE_WSS_URL.to_string(),
                // Coinbase uses standard WebSocket binary ping/pong frames.
                // use_text_ping is false (None), so the ws-mgr sends binary pings.
                ping_duration: 30,
                ping_message: String::new(),
                ping_timeout: 15,
                reconnect_timeout: 5000,
                use_text_ping: None,
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

    ws_config_methods!();

    /// Subscribe to the level2 orderbook channel for the given product IDs.
    ///
    /// Coinbase requires a subscribe message within 5 seconds of connecting.
    pub fn subscribe_depth(&self, manager: &Arc<Manager>, product_ids: &[&str]) -> Result<()> {
        for product_id in product_ids {
            let msg = CoinbaseWssMessage::depth(product_id).to_json();
            manager.write(
                &self.ws_config.name,
                Message::TextMessage(self.ws_config.name.clone(), msg),
            )?;
        }
        Ok(())
    }

    /// Subscribe to the market_trades channel for the given product IDs.
    pub fn subscribe_trades(&self, manager: &Arc<Manager>, product_ids: &[&str]) -> Result<()> {
        for product_id in product_ids {
            let msg = CoinbaseWssMessage::trades(product_id).to_json();
            manager.write(
                &self.ws_config.name,
                Message::TextMessage(self.ws_config.name.clone(), msg),
            )?;
        }
        Ok(())
    }

    pub fn unsubscribe_depth(&self, manager: &Arc<Manager>, product_ids: &[&str]) -> Result<()> {
        for product_id in product_ids {
            let msg = CoinbaseWssMessage::depth_unsub(product_id).to_json();
            manager.write(
                &self.ws_config.name,
                Message::TextMessage(self.ws_config.name.clone(), msg),
            )?;
        }
        Ok(())
    }

    pub fn unsubscribe_trades(&self, manager: &Arc<Manager>, product_ids: &[&str]) -> Result<()> {
        for product_id in product_ids {
            let msg = CoinbaseWssMessage::trades_unsub(product_id).to_json();
            manager.write(
                &self.ws_config.name,
                Message::TextMessage(self.ws_config.name.clone(), msg),
            )?;
        }
        Ok(())
    }
}

impl Default for CoinbaseClient {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
pub trait CoinbaseApi {
    async fn get_products(
        &self,
    ) -> Result<HashMap<String, CoinbaseProduct>, Box<dyn std::error::Error + Send + Sync>>;

    async fn get_product(
        &self,
        product_id: &str,
    ) -> Result<CoinbaseProduct, Box<dyn std::error::Error + Send + Sync>>;
}

#[async_trait]
impl CoinbaseApi for CoinbaseClient {
    async fn get_products(
        &self,
    ) -> Result<HashMap<String, CoinbaseProduct>, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api.get_products().await
    }

    async fn get_product(
        &self,
        product_id: &str,
    ) -> Result<CoinbaseProduct, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api.get_product(product_id).await
    }
}
