pub(crate) mod rest;

use rest::MarketApi;

pub use rest::MarketApi as AsterMarketApi;

use anyhow::Result;
use async_trait::async_trait;
use roshar_types::aster::{
    AsterFundingRate, AsterKline, AsterMarkPrice, AsterOrderBook, AsterTicker24hr,
};
use roshar_ws_mgr::{Config, Manager, Message};
use std::collections::HashMap;
use std::sync::Arc;

use crate::ws::ws_config_methods;
use crate::{ASTER_WSS_URL};

pub struct AsterClient {
    market_api: MarketApi,
    ws_config: Config,
}

impl AsterClient {
    const DEFAULT_REQUESTS_PER_SECOND: u32 = 10;
    const WS_CONN_NAME: &str = "aster";

    pub fn new() -> Self {
        Self::new_with_rate_limit(Self::DEFAULT_REQUESTS_PER_SECOND)
    }

    pub fn new_with_rate_limit(requests_per_second: u32) -> Self {
        Self {
            market_api: MarketApi::new(requests_per_second),
            ws_config: Config {
                name: Self::WS_CONN_NAME.to_string(),
                url: ASTER_WSS_URL.to_string(),
                ping_duration: 10,
                ping_message: roshar_types::aster::AsterWssMessage::ping().to_json(),
                ping_timeout: 10,
                reconnect_timeout: 5000,
                use_text_ping: Some(false),
                read_buffer_size: None,
                write_buffer_size: None,
                max_message_size: None,
                max_frame_size: None,
                tcp_recv_buffer_size: None,
                tcp_send_buffer_size: None,
                tcp_nodelay: Some(true),
                broadcast_channel_size: None,
            },
        }
    }

    ws_config_methods!();

    pub fn subscribe_depth(&self, manager: &Arc<Manager>, symbols: &[&str]) -> Result<()> {
        for symbol in symbols {
            let msg = roshar_types::aster::AsterWssMessage::depth(symbol).to_json();
            manager.write(
                &self.ws_config.name,
                Message::TextMessage(self.ws_config.name.clone(), msg),
            )?;
        }
        Ok(())
    }

    pub fn unsubscribe_depth(&self, manager: &Arc<Manager>, symbols: &[&str]) -> Result<()> {
        for symbol in symbols {
            let msg = roshar_types::aster::AsterWssMessage::depth_unsub(symbol).to_json();
            manager.write(
                &self.ws_config.name,
                Message::TextMessage(self.ws_config.name.clone(), msg),
            )?;
        }
        Ok(())
    }

    pub fn subscribe_trades(&self, manager: &Arc<Manager>, symbols: &[&str]) -> Result<()> {
        for symbol in symbols {
            let msg = roshar_types::aster::AsterWssMessage::trades(symbol).to_json();
            manager.write(
                &self.ws_config.name,
                Message::TextMessage(self.ws_config.name.clone(), msg),
            )?;
        }
        Ok(())
    }

    pub fn unsubscribe_trades(&self, manager: &Arc<Manager>, symbols: &[&str]) -> Result<()> {
        for symbol in symbols {
            let msg = roshar_types::aster::AsterWssMessage::trades_unsub(symbol).to_json();
            manager.write(
                &self.ws_config.name,
                Message::TextMessage(self.ws_config.name.clone(), msg),
            )?;
        }
        Ok(())
    }

    pub fn subscribe_mark_price(&self, manager: &Arc<Manager>, symbols: &[&str]) -> Result<()> {
        for symbol in symbols {
            let msg = roshar_types::aster::AsterWssMessage::mark_price(symbol).to_json();
            manager.write(
                &self.ws_config.name,
                Message::TextMessage(self.ws_config.name.clone(), msg),
            )?;
        }
        Ok(())
    }

    pub fn subscribe_klines(&self, manager: &Arc<Manager>, symbols: &[&str]) -> Result<()> {
        for symbol in symbols {
            let msg = roshar_types::aster::AsterWssMessage::klines(symbol).to_json();
            manager.write(
                &self.ws_config.name,
                Message::TextMessage(self.ws_config.name.clone(), msg),
            )?;
        }
        Ok(())
    }
}

#[async_trait]
pub trait AsterApi {
    /// Fetch order book snapshot. Default 100 levels; the WS diff stream needs
    /// an initial snapshot from this endpoint to initialise local book state.
    async fn get_orderbook(
        &self,
        symbol: &str,
        limit: Option<u32>,
    ) -> Result<AsterOrderBook, Box<dyn std::error::Error + Send + Sync>>;

    async fn get_mark_prices(
        &self,
    ) -> Result<Vec<AsterMarkPrice>, Box<dyn std::error::Error + Send + Sync>>;

    async fn get_tickers(
        &self,
    ) -> Result<HashMap<String, AsterTicker24hr>, Box<dyn std::error::Error + Send + Sync>>;

    async fn get_historical_funding_rates(
        &self,
        symbol: &str,
        start_time: i64,
        end_time: i64,
    ) -> Result<Vec<AsterFundingRate>, Box<dyn std::error::Error + Send + Sync>>;

    async fn get_klines(
        &self,
        symbol: &str,
        interval: &str,
        start_time: Option<i64>,
        end_time: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<AsterKline>, Box<dyn std::error::Error + Send + Sync>>;
}

#[async_trait]
impl AsterApi for AsterClient {
    async fn get_orderbook(
        &self,
        symbol: &str,
        limit: Option<u32>,
    ) -> Result<AsterOrderBook, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api.get_orderbook(symbol, limit).await
    }

    async fn get_mark_prices(
        &self,
    ) -> Result<Vec<AsterMarkPrice>, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api.get_mark_prices().await
    }

    async fn get_tickers(
        &self,
    ) -> Result<HashMap<String, AsterTicker24hr>, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api.get_tickers().await
    }

    async fn get_historical_funding_rates(
        &self,
        symbol: &str,
        start_time: i64,
        end_time: i64,
    ) -> Result<Vec<AsterFundingRate>, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api
            .get_historical_funding_rates(symbol, start_time, end_time)
            .await
    }

    async fn get_klines(
        &self,
        symbol: &str,
        interval: &str,
        start_time: Option<i64>,
        end_time: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<AsterKline>, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api
            .get_klines(symbol, interval, start_time, end_time, limit)
            .await
    }
}
