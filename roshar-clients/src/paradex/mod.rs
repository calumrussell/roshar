pub(crate) mod rest;

use rest::MarketApi;

pub use rest::MarketApi as ParadexMarketApi;

use anyhow::Result;
use async_trait::async_trait;
use roshar_types::paradex::{
    ParadexFundingEntry, ParadexKline, ParadexMarket, ParadexMarketSummary, ParadexOrderBook,
    ParadexTrade,
};
use roshar_ws_mgr::{Config, Manager, Message};
use std::collections::HashMap;
use std::sync::Arc;

use crate::ws::ws_config_methods;
use crate::PARADEX_WSS_URL;

pub struct ParadexClient {
    market_api: MarketApi,
    ws_config: Config,
}

impl ParadexClient {
    const DEFAULT_REQUESTS_PER_SECOND: u32 = 10;
    const WS_CONN_NAME: &str = "paradex";

    pub fn new() -> Self {
        Self::new_with_rate_limit(Self::DEFAULT_REQUESTS_PER_SECOND)
    }

    pub fn new_with_rate_limit(requests_per_second: u32) -> Self {
        Self {
            market_api: MarketApi::new(requests_per_second),
            ws_config: Config {
                name: Self::WS_CONN_NAME.to_string(),
                url: PARADEX_WSS_URL.to_string(),
                // Paradex server pings every 55s; client must pong within 5s.
                // The ws manager handles standard WebSocket ping/pong frames.
                ping_duration: 30,
                ping_message: roshar_types::paradex::ParadexWssMessage::ping().to_json(),
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

    pub fn subscribe_depth(&self, manager: &Arc<Manager>, markets: &[&str]) -> Result<()> {
        for market in markets {
            let msg = roshar_types::paradex::ParadexWssMessage::depth(market).to_json();
            manager.write(
                &self.ws_config.name,
                Message::TextMessage(self.ws_config.name.clone(), msg),
            )?;
        }
        Ok(())
    }

    pub fn unsubscribe_depth(&self, manager: &Arc<Manager>, markets: &[&str]) -> Result<()> {
        for market in markets {
            let msg = roshar_types::paradex::ParadexWssMessage::depth_unsub(market).to_json();
            manager.write(
                &self.ws_config.name,
                Message::TextMessage(self.ws_config.name.clone(), msg),
            )?;
        }
        Ok(())
    }

    pub fn subscribe_trades(&self, manager: &Arc<Manager>, markets: &[&str]) -> Result<()> {
        for market in markets {
            let msg = roshar_types::paradex::ParadexWssMessage::trades(market).to_json();
            manager.write(
                &self.ws_config.name,
                Message::TextMessage(self.ws_config.name.clone(), msg),
            )?;
        }
        Ok(())
    }

    pub fn unsubscribe_trades(&self, manager: &Arc<Manager>, markets: &[&str]) -> Result<()> {
        for market in markets {
            let msg = roshar_types::paradex::ParadexWssMessage::trades_unsub(market).to_json();
            manager.write(
                &self.ws_config.name,
                Message::TextMessage(self.ws_config.name.clone(), msg),
            )?;
        }
        Ok(())
    }

    pub fn subscribe_bbo(&self, manager: &Arc<Manager>, markets: &[&str]) -> Result<()> {
        for market in markets {
            let msg = roshar_types::paradex::ParadexWssMessage::bbo(market).to_json();
            manager.write(
                &self.ws_config.name,
                Message::TextMessage(self.ws_config.name.clone(), msg),
            )?;
        }
        Ok(())
    }

    pub fn subscribe_funding(&self, manager: &Arc<Manager>, markets: &[&str]) -> Result<()> {
        for market in markets {
            let msg = roshar_types::paradex::ParadexWssMessage::funding(market).to_json();
            manager.write(
                &self.ws_config.name,
                Message::TextMessage(self.ws_config.name.clone(), msg),
            )?;
        }
        Ok(())
    }

    /// Subscribe to the global markets summary ticker stream.
    pub fn subscribe_markets_summary(&self, manager: &Arc<Manager>) -> Result<()> {
        let msg = roshar_types::paradex::ParadexWssMessage::markets_summary().to_json();
        manager.write(
            &self.ws_config.name,
            Message::TextMessage(self.ws_config.name.clone(), msg),
        )?;
        Ok(())
    }
}

#[async_trait]
pub trait ParadexApi {
    async fn get_markets(
        &self,
    ) -> Result<Vec<ParadexMarket>, Box<dyn std::error::Error + Send + Sync>>;

    async fn get_markets_summary(
        &self,
        market: Option<&str>,
    ) -> Result<HashMap<String, ParadexMarketSummary>, Box<dyn std::error::Error + Send + Sync>>;

    async fn get_orderbook(
        &self,
        market: &str,
        depth: Option<u32>,
    ) -> Result<ParadexOrderBook, Box<dyn std::error::Error + Send + Sync>>;

    async fn get_trades(
        &self,
        market: &str,
        limit: Option<u32>,
    ) -> Result<Vec<ParadexTrade>, Box<dyn std::error::Error + Send + Sync>>;

    async fn get_klines(
        &self,
        market: &str,
        resolution: u32,
        start_at: Option<i64>,
        end_at: Option<i64>,
    ) -> Result<Vec<ParadexKline>, Box<dyn std::error::Error + Send + Sync>>;

    async fn get_funding_history(
        &self,
        market: &str,
        start_at: Option<i64>,
        end_at: Option<i64>,
    ) -> Result<Vec<ParadexFundingEntry>, Box<dyn std::error::Error + Send + Sync>>;
}

#[async_trait]
impl ParadexApi for ParadexClient {
    async fn get_markets(
        &self,
    ) -> Result<Vec<ParadexMarket>, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api.get_markets().await
    }

    async fn get_markets_summary(
        &self,
        market: Option<&str>,
    ) -> Result<HashMap<String, ParadexMarketSummary>, Box<dyn std::error::Error + Send + Sync>>
    {
        self.market_api.get_markets_summary(market).await
    }

    async fn get_orderbook(
        &self,
        market: &str,
        depth: Option<u32>,
    ) -> Result<ParadexOrderBook, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api.get_orderbook(market, depth).await
    }

    async fn get_trades(
        &self,
        market: &str,
        limit: Option<u32>,
    ) -> Result<Vec<ParadexTrade>, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api.get_trades(market, limit).await
    }

    async fn get_klines(
        &self,
        market: &str,
        resolution: u32,
        start_at: Option<i64>,
        end_at: Option<i64>,
    ) -> Result<Vec<ParadexKline>, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api
            .get_klines(market, resolution, start_at, end_at)
            .await
    }

    async fn get_funding_history(
        &self,
        market: &str,
        start_at: Option<i64>,
        end_at: Option<i64>,
    ) -> Result<Vec<ParadexFundingEntry>, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api
            .get_funding_history(market, start_at, end_at)
            .await
    }
}
