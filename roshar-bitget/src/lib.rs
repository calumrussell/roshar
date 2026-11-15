/// Wire and message types for this exchange (websocket messages, REST payloads).
pub mod types;

// Endpoints
pub const BITGET_WSS_URL: &str = "wss://ws.bitget.com/v2/ws/public";
pub const BITGET_REST_URL: &str = "https://api.bitget.com";

pub(crate) mod rest;

pub use rest::MarketApi as BitgetMarketApi;
pub use rest::{BitgetContract, BitgetHistoricalFundingRate, BitgetTickerData};

use crate::types::BitgetWssMessage;
use anyhow::Result;
use async_trait::async_trait;
use roshar_ws_mgr::{Config, Manager, Message};
use std::collections::HashMap;
use std::sync::Arc;

pub struct BitgetClient {
    market_api: rest::MarketApi,
    ws_config: Config,
}

impl BitgetClient {
    /// Bitget public endpoints allow 20 requests per second per IP.
    const DEFAULT_REQUESTS_PER_SECOND: u32 = 20;
    const WS_CONN_NAME: &str = "bitget";

    pub fn new() -> Self {
        Self::new_with_rate_limit(Self::DEFAULT_REQUESTS_PER_SECOND)
    }

    pub fn new_with_rate_limit(requests_per_second: u32) -> Self {
        Self {
            market_api: rest::MarketApi::new(requests_per_second),
            ws_config: Config {
                name: Self::WS_CONN_NAME.to_string(),
                url: BITGET_WSS_URL.to_string(),
                ping_duration: 30,
                ping_message: BitgetWssMessage::ping(),
                ping_timeout: 15,
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

    pub fn ws_config(&self) -> roshar_ws_mgr::Config {
        self.ws_config.clone()
    }

    pub fn ws_config_mut(&mut self) -> &mut roshar_ws_mgr::Config {
        &mut self.ws_config
    }

    pub fn subscribe_depth(&self, manager: &Arc<Manager>, symbols: &[&str]) -> Result<()> {
        for symbol in symbols {
            let msg = BitgetWssMessage::depth(symbol).to_json();
            manager.write(
                &self.ws_config.name,
                Message::TextMessage(self.ws_config.name.clone(), msg),
            )?;
        }
        Ok(())
    }

    pub fn subscribe_trades(&self, manager: &Arc<Manager>, symbols: &[&str]) -> Result<()> {
        for symbol in symbols {
            let msg = BitgetWssMessage::trades(symbol).to_json();
            manager.write(
                &self.ws_config.name,
                Message::TextMessage(self.ws_config.name.clone(), msg),
            )?;
        }
        Ok(())
    }

    pub fn unsubscribe_depth(&self, manager: &Arc<Manager>, symbols: &[&str]) -> Result<()> {
        for symbol in symbols {
            let msg = BitgetWssMessage::depth_unsub(symbol).to_json();
            manager.write(
                &self.ws_config.name,
                Message::TextMessage(self.ws_config.name.clone(), msg),
            )?;
        }
        Ok(())
    }

    pub fn unsubscribe_trades(&self, manager: &Arc<Manager>, symbols: &[&str]) -> Result<()> {
        for symbol in symbols {
            let msg = BitgetWssMessage::trades_unsub(symbol).to_json();
            manager.write(
                &self.ws_config.name,
                Message::TextMessage(self.ws_config.name.clone(), msg),
            )?;
        }
        Ok(())
    }
}

impl Default for BitgetClient {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
pub trait BitgetApi {
    async fn get_contracts(
        &self,
    ) -> Result<HashMap<String, BitgetContract>, Box<dyn std::error::Error + Send + Sync>>;

    async fn get_tickers(
        &self,
    ) -> Result<HashMap<String, BitgetTickerData>, Box<dyn std::error::Error + Send + Sync>>;

    async fn get_historical_funding_rates(
        &self,
        symbol: &str,
        page_size: u32,
        page_no: u32,
    ) -> Result<Vec<BitgetHistoricalFundingRate>, Box<dyn std::error::Error + Send + Sync>>;
}

#[async_trait]
impl BitgetApi for BitgetClient {
    async fn get_contracts(
        &self,
    ) -> Result<HashMap<String, BitgetContract>, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api.get_contracts().await
    }

    async fn get_tickers(
        &self,
    ) -> Result<HashMap<String, BitgetTickerData>, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api.get_tickers().await
    }

    async fn get_historical_funding_rates(
        &self,
        symbol: &str,
        page_size: u32,
        page_no: u32,
    ) -> Result<Vec<BitgetHistoricalFundingRate>, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api
            .get_historical_funding_rates(symbol, page_size, page_no)
            .await
    }
}
