/// Wire and message types for this exchange (websocket messages, REST payloads).
pub mod types;

// Endpoints
pub const COINBASE_WSS_URL: &str = "wss://advanced-trade-ws.coinbase.com";
pub const COINBASE_REST_URL: &str = "https://api.coinbase.com";

pub(crate) mod rest;

pub use rest::MarketApi as CoinbaseMarketApi;
pub use types::{CoinbaseProduct, CoinbaseProductsResponse};

use crate::types::CoinbaseWssMessage;
use crate::types::{
    CoinbaseCandlesResponse, CoinbaseGranularity, CoinbaseMarketTradesResponse,
    CoinbaseProductBookResponse, CoinbaseProductsQuery, CoinbaseServerTime,
};
use anyhow::Result;
use async_trait::async_trait;
use roshar_ws_mgr::{Config, Manager, Message};
use std::sync::Arc;

pub struct CoinbaseClient {
    market_api: rest::MarketApi,
    ws_config: Config,
}

impl CoinbaseClient {
    /// Coinbase no longer documents a REST limit for the Advanced Trade public endpoints; 10/s per
    /// IP is the figure it last published. Override with `new_with_rate_limit`.
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
                headers: vec![],
            },
        }
    }

    pub fn ws_config(&self) -> roshar_ws_mgr::Config {
        self.ws_config.clone()
    }

    pub fn ws_config_mut(&mut self) -> &mut roshar_ws_mgr::Config {
        &mut self.ws_config
    }

    fn send(&self, manager: &Arc<Manager>, msg: CoinbaseWssMessage) -> Result<()> {
        manager.write(
            &self.ws_config.name,
            Message::TextMessage(self.ws_config.name.clone(), msg.to_json()),
        )
    }

    /// Subscribe to the level2 orderbook channel for the given product IDs, in one message.
    ///
    /// Coinbase requires a subscribe message within 5 seconds of connecting.
    pub fn subscribe_depth(&self, manager: &Arc<Manager>, product_ids: &[&str]) -> Result<()> {
        self.send(manager, CoinbaseWssMessage::depth(product_ids))
    }

    /// Subscribe to the market_trades channel for the given product IDs, in one message.
    pub fn subscribe_trades(&self, manager: &Arc<Manager>, product_ids: &[&str]) -> Result<()> {
        self.send(manager, CoinbaseWssMessage::trades(product_ids))
    }

    /// Subscribe to the heartbeats channel, which keeps quiet subscriptions open.
    pub fn subscribe_heartbeats(&self, manager: &Arc<Manager>) -> Result<()> {
        self.send(manager, CoinbaseWssMessage::heartbeats())
    }

    pub fn unsubscribe_depth(&self, manager: &Arc<Manager>, product_ids: &[&str]) -> Result<()> {
        self.send(manager, CoinbaseWssMessage::depth_unsub(product_ids))
    }

    pub fn unsubscribe_trades(&self, manager: &Arc<Manager>, product_ids: &[&str]) -> Result<()> {
        self.send(manager, CoinbaseWssMessage::trades_unsub(product_ids))
    }

    pub fn unsubscribe_heartbeats(&self, manager: &Arc<Manager>) -> Result<()> {
        self.send(manager, CoinbaseWssMessage::heartbeats_unsub())
    }

    /// Subscribe to the ticker channel for the given product IDs, in one message.
    pub fn subscribe_ticker(&self, manager: &Arc<Manager>, product_ids: &[&str]) -> Result<()> {
        self.send(manager, CoinbaseWssMessage::ticker(product_ids))
    }

    pub fn unsubscribe_ticker(&self, manager: &Arc<Manager>, product_ids: &[&str]) -> Result<()> {
        self.send(manager, CoinbaseWssMessage::ticker_unsub(product_ids))
    }

    /// Subscribe to the ticker_batch channel (5 s batches) for the given product IDs.
    pub fn subscribe_ticker_batch(
        &self,
        manager: &Arc<Manager>,
        product_ids: &[&str],
    ) -> Result<()> {
        self.send(manager, CoinbaseWssMessage::ticker_batch(product_ids))
    }

    pub fn unsubscribe_ticker_batch(
        &self,
        manager: &Arc<Manager>,
        product_ids: &[&str],
    ) -> Result<()> {
        self.send(manager, CoinbaseWssMessage::ticker_batch_unsub(product_ids))
    }

    /// Subscribe to the candles channel (five-minute candles) for the given product IDs.
    pub fn subscribe_candles(&self, manager: &Arc<Manager>, product_ids: &[&str]) -> Result<()> {
        self.send(manager, CoinbaseWssMessage::candles(product_ids))
    }

    pub fn unsubscribe_candles(&self, manager: &Arc<Manager>, product_ids: &[&str]) -> Result<()> {
        self.send(manager, CoinbaseWssMessage::candles_unsub(product_ids))
    }

    /// Subscribe to the status channel for the given product IDs.
    pub fn subscribe_status(&self, manager: &Arc<Manager>, product_ids: &[&str]) -> Result<()> {
        self.send(manager, CoinbaseWssMessage::status(product_ids))
    }

    pub fn unsubscribe_status(&self, manager: &Arc<Manager>, product_ids: &[&str]) -> Result<()> {
        self.send(manager, CoinbaseWssMessage::status_unsub(product_ids))
    }
}

impl Default for CoinbaseClient {
    fn default() -> Self {
        Self::new()
    }
}

type BoxError = Box<dyn std::error::Error + Send + Sync>;

#[async_trait]
pub trait CoinbaseApi {
    async fn get_products(
        &self,
        query: &CoinbaseProductsQuery,
    ) -> Result<CoinbaseProductsResponse, BoxError>;

    async fn get_product(&self, product_id: &str) -> Result<CoinbaseProduct, BoxError>;

    async fn get_product_book(
        &self,
        product_id: &str,
        limit: Option<u32>,
        aggregation_price_increment: Option<&str>,
    ) -> Result<CoinbaseProductBookResponse, BoxError>;

    async fn get_market_trades(
        &self,
        product_id: &str,
        limit: Option<u32>,
        start: Option<u64>,
        end: Option<u64>,
    ) -> Result<CoinbaseMarketTradesResponse, BoxError>;

    async fn get_candles(
        &self,
        product_id: &str,
        granularity: CoinbaseGranularity,
        start: Option<u64>,
        end: Option<u64>,
        limit: Option<u32>,
    ) -> Result<CoinbaseCandlesResponse, BoxError>;

    async fn get_server_time(&self) -> Result<CoinbaseServerTime, BoxError>;
}

#[async_trait]
impl CoinbaseApi for CoinbaseClient {
    async fn get_products(
        &self,
        query: &CoinbaseProductsQuery,
    ) -> Result<CoinbaseProductsResponse, BoxError> {
        self.market_api.get_products(query).await
    }

    async fn get_product(&self, product_id: &str) -> Result<CoinbaseProduct, BoxError> {
        self.market_api.get_product(product_id).await
    }

    async fn get_product_book(
        &self,
        product_id: &str,
        limit: Option<u32>,
        aggregation_price_increment: Option<&str>,
    ) -> Result<CoinbaseProductBookResponse, BoxError> {
        self.market_api
            .get_product_book(product_id, limit, aggregation_price_increment)
            .await
    }

    async fn get_market_trades(
        &self,
        product_id: &str,
        limit: Option<u32>,
        start: Option<u64>,
        end: Option<u64>,
    ) -> Result<CoinbaseMarketTradesResponse, BoxError> {
        self.market_api
            .get_market_trades(product_id, limit, start, end)
            .await
    }

    async fn get_candles(
        &self,
        product_id: &str,
        granularity: CoinbaseGranularity,
        start: Option<u64>,
        end: Option<u64>,
        limit: Option<u32>,
    ) -> Result<CoinbaseCandlesResponse, BoxError> {
        self.market_api
            .get_candles(product_id, granularity, start, end, limit)
            .await
    }

    async fn get_server_time(&self) -> Result<CoinbaseServerTime, BoxError> {
        self.market_api.get_server_time().await
    }
}
