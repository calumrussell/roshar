pub(crate) mod rest;

pub use rest::MarketApi as KuCoinMarketApi;
pub use rest::{KuCoinContract, KuCoinFundingRate};

use anyhow::Result;
use async_trait::async_trait;
use roshar_types::KuCoinWssMessage;
use roshar_ws_mgr::{Config, Manager, Message};
use std::collections::HashMap;
use std::sync::Arc;

use crate::ws::ws_config_methods;
use crate::KUCOIN_FUTURES_WSS_URL;

pub struct KuCoinClient {
    market_api: rest::MarketApi,
    ws_config: Config,
}

impl KuCoinClient {
    /// Conservative default: KuCoin public endpoints have weight-based limits.
    /// 10 req/sec is safe for most use cases.
    const DEFAULT_REQUESTS_PER_SECOND: u32 = 10;
    const WS_CONN_NAME: &str = "kucoin";

    pub fn new() -> Self {
        Self::new_with_rate_limit(Self::DEFAULT_REQUESTS_PER_SECOND)
    }

    pub fn new_with_rate_limit(requests_per_second: u32) -> Self {
        Self {
            market_api: rest::MarketApi::new(requests_per_second),
            ws_config: Config {
                name: Self::WS_CONN_NAME.to_string(),
                // Placeholder — call get_ws_url() and set_ws_url() before connecting
                url: KUCOIN_FUTURES_WSS_URL.to_string(),
                // KuCoin recommends a ping every 18 seconds (pingInterval: 18000ms)
                ping_duration: 18,
                ping_message: KuCoinWssMessage::ping().to_json(),
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

    ws_config_methods!();

    /// Obtain a fresh WebSocket token and return the full WSS URL.
    ///
    /// KuCoin requires a short-lived token for WebSocket connections. This
    /// method calls the bullet-public REST endpoint and returns the complete
    /// connection URL (endpoint + token). Call this before `new_conn` and
    /// update the client with `set_ws_url()`.
    pub async fn get_ws_url(&self) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api.get_ws_url().await
    }

    pub fn subscribe_depth(&self, manager: &Arc<Manager>, symbols: &[&str]) -> Result<()> {
        for symbol in symbols {
            let msg = KuCoinWssMessage::depth(symbol).to_json();
            manager.write(
                &self.ws_config.name,
                Message::TextMessage(self.ws_config.name.clone(), msg),
            )?;
        }
        Ok(())
    }

    pub fn subscribe_trades(&self, manager: &Arc<Manager>, symbols: &[&str]) -> Result<()> {
        for symbol in symbols {
            let msg = KuCoinWssMessage::trades(symbol).to_json();
            manager.write(
                &self.ws_config.name,
                Message::TextMessage(self.ws_config.name.clone(), msg),
            )?;
        }
        Ok(())
    }

    pub fn unsubscribe_depth(&self, manager: &Arc<Manager>, symbols: &[&str]) -> Result<()> {
        for symbol in symbols {
            let msg = KuCoinWssMessage::depth_unsub(symbol).to_json();
            manager.write(
                &self.ws_config.name,
                Message::TextMessage(self.ws_config.name.clone(), msg),
            )?;
        }
        Ok(())
    }

    pub fn unsubscribe_trades(&self, manager: &Arc<Manager>, symbols: &[&str]) -> Result<()> {
        for symbol in symbols {
            let msg = KuCoinWssMessage::trades_unsub(symbol).to_json();
            manager.write(
                &self.ws_config.name,
                Message::TextMessage(self.ws_config.name.clone(), msg),
            )?;
        }
        Ok(())
    }
}

impl Default for KuCoinClient {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
pub trait KuCoinApi {
    async fn get_contracts(
        &self,
    ) -> Result<HashMap<String, KuCoinContract>, Box<dyn std::error::Error + Send + Sync>>;

    async fn get_historical_funding_rates(
        &self,
        symbol: &str,
        start_time: i64,
        end_time: i64,
    ) -> Result<Vec<KuCoinFundingRate>, Box<dyn std::error::Error + Send + Sync>>;
}

#[async_trait]
impl KuCoinApi for KuCoinClient {
    async fn get_contracts(
        &self,
    ) -> Result<HashMap<String, KuCoinContract>, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api.get_contracts().await
    }

    async fn get_historical_funding_rates(
        &self,
        symbol: &str,
        start_time: i64,
        end_time: i64,
    ) -> Result<Vec<KuCoinFundingRate>, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api
            .get_historical_funding_rates(symbol, start_time, end_time)
            .await
    }
}
