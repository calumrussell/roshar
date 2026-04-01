pub(crate) mod rest;

use rest::MarketApi;

pub use rest::MarketApi as LighterMarketApi;

use anyhow::Result;
use async_trait::async_trait;
use roshar_types::lighter::{
    LighterFundingRate, LighterMarket, LighterMarketDetail, LighterOrderBook, LighterTrade,
};
use roshar_ws_mgr::{Config, Manager, Message};
use std::sync::Arc;

use crate::ws::ws_config_methods;
use crate::LIGHTER_WSS_URL;

pub struct LighterClient {
    market_api: MarketApi,
    ws_config: Config,
}

impl LighterClient {
    const DEFAULT_REQUESTS_PER_SECOND: u32 = 10;
    const WS_CONN_NAME: &str = "lighter";

    pub fn new() -> Self {
        Self::new_with_rate_limit(Self::DEFAULT_REQUESTS_PER_SECOND)
    }

    pub fn new_with_rate_limit(requests_per_second: u32) -> Self {
        Self {
            market_api: MarketApi::new(requests_per_second),
            ws_config: Config {
                name: Self::WS_CONN_NAME.to_string(),
                url: LIGHTER_WSS_URL.to_string(),
                // Lighter disconnects if no frame is received within 2 minutes.
                ping_duration: 90,
                ping_message: roshar_types::lighter::LighterWssMessage::ping().to_json(),
                ping_timeout: 10,
                reconnect_timeout: 5000,
                use_text_ping: Some(true),
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

    /// Subscribe to order book updates for a market by its integer index.
    pub fn subscribe_depth(&self, manager: &Arc<Manager>, market_indices: &[u32]) -> Result<()> {
        for &idx in market_indices {
            let msg = roshar_types::lighter::LighterWssMessage::depth(idx).to_json();
            manager.write(
                &self.ws_config.name,
                Message::TextMessage(self.ws_config.name.clone(), msg),
            )?;
        }
        Ok(())
    }

    pub fn unsubscribe_depth(&self, manager: &Arc<Manager>, market_indices: &[u32]) -> Result<()> {
        for &idx in market_indices {
            let msg = roshar_types::lighter::LighterWssMessage::depth_unsub(idx).to_json();
            manager.write(
                &self.ws_config.name,
                Message::TextMessage(self.ws_config.name.clone(), msg),
            )?;
        }
        Ok(())
    }

    /// Subscribe to trade execution stream for a market.
    pub fn subscribe_trades(&self, manager: &Arc<Manager>, market_indices: &[u32]) -> Result<()> {
        for &idx in market_indices {
            let msg = roshar_types::lighter::LighterWssMessage::trades(idx).to_json();
            manager.write(
                &self.ws_config.name,
                Message::TextMessage(self.ws_config.name.clone(), msg),
            )?;
        }
        Ok(())
    }

    pub fn unsubscribe_trades(
        &self,
        manager: &Arc<Manager>,
        market_indices: &[u32],
    ) -> Result<()> {
        for &idx in market_indices {
            let msg = roshar_types::lighter::LighterWssMessage::trades_unsub(idx).to_json();
            manager.write(
                &self.ws_config.name,
                Message::TextMessage(self.ws_config.name.clone(), msg),
            )?;
        }
        Ok(())
    }

    /// Subscribe to market stats (mark price, funding rate, 24h volume) for a market.
    pub fn subscribe_market_stats(
        &self,
        manager: &Arc<Manager>,
        market_indices: &[u32],
    ) -> Result<()> {
        for &idx in market_indices {
            let msg = roshar_types::lighter::LighterWssMessage::market_stats(idx).to_json();
            manager.write(
                &self.ws_config.name,
                Message::TextMessage(self.ws_config.name.clone(), msg),
            )?;
        }
        Ok(())
    }

    /// Subscribe to market stats for all markets in a single message.
    pub fn subscribe_all_market_stats(&self, manager: &Arc<Manager>) -> Result<()> {
        let msg = roshar_types::lighter::LighterWssMessage::all_market_stats().to_json();
        manager.write(
            &self.ws_config.name,
            Message::TextMessage(self.ws_config.name.clone(), msg),
        )?;
        Ok(())
    }

    /// Subscribe to best-bid/offer ticker for a market.
    pub fn subscribe_ticker(&self, manager: &Arc<Manager>, market_indices: &[u32]) -> Result<()> {
        for &idx in market_indices {
            let msg = roshar_types::lighter::LighterWssMessage::ticker(idx).to_json();
            manager.write(
                &self.ws_config.name,
                Message::TextMessage(self.ws_config.name.clone(), msg),
            )?;
        }
        Ok(())
    }
}

#[async_trait]
pub trait LighterApi {
    async fn get_markets(
        &self,
    ) -> Result<Vec<LighterMarket>, Box<dyn std::error::Error + Send + Sync>>;

    async fn get_market_details(
        &self,
    ) -> Result<Vec<LighterMarketDetail>, Box<dyn std::error::Error + Send + Sync>>;

    async fn get_orderbook(
        &self,
        market_id: u32,
        limit: Option<u32>,
    ) -> Result<LighterOrderBook, Box<dyn std::error::Error + Send + Sync>>;

    async fn get_recent_trades(
        &self,
        market_id: u32,
        limit: Option<u32>,
    ) -> Result<Vec<LighterTrade>, Box<dyn std::error::Error + Send + Sync>>;

    async fn get_funding_rates(
        &self,
    ) -> Result<Vec<LighterFundingRate>, Box<dyn std::error::Error + Send + Sync>>;
}

#[async_trait]
impl LighterApi for LighterClient {
    async fn get_markets(
        &self,
    ) -> Result<Vec<LighterMarket>, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api.get_markets().await
    }

    async fn get_market_details(
        &self,
    ) -> Result<Vec<LighterMarketDetail>, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api.get_market_details().await
    }

    async fn get_orderbook(
        &self,
        market_id: u32,
        limit: Option<u32>,
    ) -> Result<LighterOrderBook, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api.get_orderbook(market_id, limit).await
    }

    async fn get_recent_trades(
        &self,
        market_id: u32,
        limit: Option<u32>,
    ) -> Result<Vec<LighterTrade>, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api.get_recent_trades(market_id, limit).await
    }

    async fn get_funding_rates(
        &self,
    ) -> Result<Vec<LighterFundingRate>, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api.get_funding_rates().await
    }
}
