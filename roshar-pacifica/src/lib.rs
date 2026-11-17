/// Wire and message types for this exchange (websocket messages, REST payloads).
pub mod types;

// Endpoints
pub const PACIFICA_WSS_URL: &str = "wss://ws.pacifica.fi/ws";
pub const PACIFICA_TESTNET_WSS_URL: &str = "wss://test-ws.pacifica.fi/ws";

mod deflate;
pub(crate) mod rest;
mod wire;

pub use deflate::PacificaInflater;
pub use wire::float_to_wire;

use rest::MarketApi;
use rest::TradingApi;

pub use rest::MarketApi as PacificaMarketApi;
pub use rest::TradingApi as PacificaTradingApiImpl;

use crate::types::{
    PacificaFundingRatePage, PacificaKline, PacificaMarket, PacificaOpenOrder, PacificaOrderBook,
    PacificaPosition, PacificaSpotAsset, PacificaTicker, PacificaTrade,
};
use anyhow::Result;
use async_trait::async_trait;
use roshar_http::RateLimitedClient;
use roshar_ws_mgr::{Config, Manager, Message};
use std::collections::HashMap;
use std::sync::Arc;

type BoxError = Box<dyn std::error::Error + Send + Sync>;

pub struct PacificaClient {
    market_api: MarketApi,
    ws_config: Config,
    trading_api: Option<TradingApi>,
}

impl PacificaClient {
    /// 5 requests per 3 s = 100 per 60 s, the documented base quota for a caller without an API
    /// config key (<https://docs.pacifica.fi/api-documentation/api/rate-limits>). Heavy GETs cost
    /// more than one credit; callers with a larger quota use `new_with_rate_limit`.
    const DEFAULT_RATE_REFILL: u32 = 5;
    const DEFAULT_RATE_INTERVAL_SECS: u64 = 3;
    const WS_CONN_NAME: &str = "pacifica";

    fn default_http_client() -> RateLimitedClient {
        RateLimitedClient::new(Self::DEFAULT_RATE_REFILL, Self::DEFAULT_RATE_INTERVAL_SECS)
    }

    /// Market data only. `is_mainnet` selects the REST and websocket endpoints.
    pub fn new(is_mainnet: bool) -> Self {
        Self::with_http_client(Self::default_http_client(), is_mainnet)
    }

    /// Market data only, allowing `requests` REST calls every `interval_secs` seconds.
    pub fn new_with_rate_limit(requests: u32, interval_secs: u64, is_mainnet: bool) -> Self {
        Self::with_http_client(RateLimitedClient::new(requests, interval_secs), is_mainnet)
    }

    fn with_http_client(client: RateLimitedClient, is_mainnet: bool) -> Self {
        Self {
            market_api: MarketApi::new(client, is_mainnet),
            ws_config: Self::default_ws_config(is_mainnet),
            trading_api: None,
        }
    }

    /// Market data and trading, all on the network `is_mainnet` selects, sharing one rate limit.
    /// `private_key_b58` is the account's own keypair or an API agent keypair bound to `account`;
    /// `agent_wallet` is sent only for an agent key. `expiry_window_ms` is the signature lifetime
    /// sent with every signed request.
    pub fn new_with_credentials(
        private_key_b58: &str,
        account: String,
        expiry_window_ms: u64,
        is_mainnet: bool,
    ) -> Result<Self> {
        let client = Self::default_http_client();
        let trading_api = TradingApi::new(
            client.clone(),
            private_key_b58,
            account,
            expiry_window_ms,
            is_mainnet,
        )?;
        Ok(Self {
            market_api: MarketApi::new(client, is_mainnet),
            ws_config: Self::default_ws_config(is_mainnet),
            trading_api: Some(trading_api),
        })
    }

    fn default_ws_config(is_mainnet: bool) -> Config {
        Config {
            name: Self::WS_CONN_NAME.to_string(),
            url: if is_mainnet {
                PACIFICA_WSS_URL
            } else {
                PACIFICA_TESTNET_WSS_URL
            }
            .to_string(),
            // The server closes a connection after 60 s without a client message.
            ping_duration: 30,
            ping_message: crate::types::PacificaWssMessage::ping().to_json(),
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
            headers: vec![],
        }
    }

    fn trading(&self) -> Result<&TradingApi, BoxError> {
        self.trading_api
            .as_ref()
            .ok_or_else(|| "PacificaClient was not created with credentials".into())
    }

    pub fn ws_config(&self) -> roshar_ws_mgr::Config {
        self.ws_config.clone()
    }

    pub fn ws_config_mut(&mut self) -> &mut roshar_ws_mgr::Config {
        &mut self.ws_config
    }

    /// Opens the websocket with `?compress=deflate`: subscription data then arrives as
    /// `Message::BinaryMessage` frames of raw DEFLATE, decoded with one [`PacificaInflater`] per
    /// connection (reset it on `Message::SuccessfulHandshake`). Acks, `pong` and errors stay text.
    pub fn use_ws_deflate(&mut self) {
        if !self.ws_config.url.contains("compress=deflate") {
            self.ws_config.url.push_str("?compress=deflate");
        }
    }

    fn send(&self, manager: &Arc<Manager>, msg: crate::types::PacificaWssMessage) -> Result<()> {
        manager.write(
            &self.ws_config.name,
            Message::TextMessage(self.ws_config.name.clone(), msg.to_json()),
        )?;
        Ok(())
    }

    /// `book` channel. `agg_level` is one of 1, 2, 5, 10, 100, 1000.
    pub fn subscribe_depth(
        &self,
        manager: &Arc<Manager>,
        symbols: &[&str],
        agg_level: u32,
    ) -> Result<()> {
        for symbol in symbols {
            self.send(
                manager,
                crate::types::PacificaWssMessage::depth(symbol, agg_level),
            )?;
        }
        Ok(())
    }

    /// `agg_level` must be the value used to subscribe.
    pub fn unsubscribe_depth(
        &self,
        manager: &Arc<Manager>,
        symbols: &[&str],
        agg_level: u32,
    ) -> Result<()> {
        for symbol in symbols {
            self.send(
                manager,
                crate::types::PacificaWssMessage::depth_unsub(symbol, agg_level),
            )?;
        }
        Ok(())
    }

    pub fn subscribe_trades(&self, manager: &Arc<Manager>, symbols: &[&str]) -> Result<()> {
        for symbol in symbols {
            self.send(manager, crate::types::PacificaWssMessage::trades(symbol))?;
        }
        Ok(())
    }

    pub fn unsubscribe_trades(&self, manager: &Arc<Manager>, symbols: &[&str]) -> Result<()> {
        for symbol in symbols {
            self.send(
                manager,
                crate::types::PacificaWssMessage::trades_unsub(symbol),
            )?;
        }
        Ok(())
    }

    /// `interval` is sent as given (e.g. `"1m"`).
    pub fn subscribe_candles(
        &self,
        manager: &Arc<Manager>,
        symbols: &[&str],
        interval: &str,
    ) -> Result<()> {
        for symbol in symbols {
            self.send(
                manager,
                crate::types::PacificaWssMessage::candles(symbol, interval),
            )?;
        }
        Ok(())
    }

    /// `interval` must be the value used to subscribe.
    pub fn unsubscribe_candles(
        &self,
        manager: &Arc<Manager>,
        symbols: &[&str],
        interval: &str,
    ) -> Result<()> {
        for symbol in symbols {
            self.send(
                manager,
                crate::types::PacificaWssMessage::candles_unsub(symbol, interval),
            )?;
        }
        Ok(())
    }

    /// `mark_price_candle` channel. `interval` is sent as given (e.g. `"1m"`).
    pub fn subscribe_mark_price_candles(
        &self,
        manager: &Arc<Manager>,
        symbols: &[&str],
        interval: &str,
    ) -> Result<()> {
        for symbol in symbols {
            self.send(
                manager,
                crate::types::PacificaWssMessage::mark_price_candles(symbol, interval),
            )?;
        }
        Ok(())
    }

    /// `interval` must be the value used to subscribe.
    pub fn unsubscribe_mark_price_candles(
        &self,
        manager: &Arc<Manager>,
        symbols: &[&str],
        interval: &str,
    ) -> Result<()> {
        for symbol in symbols {
            self.send(
                manager,
                crate::types::PacificaWssMessage::mark_price_candles_unsub(symbol, interval),
            )?;
        }
        Ok(())
    }

    /// `bbo` channel: top of book, sent whenever it changes.
    pub fn subscribe_bbo(&self, manager: &Arc<Manager>, symbols: &[&str]) -> Result<()> {
        for symbol in symbols {
            self.send(manager, crate::types::PacificaWssMessage::bbo(symbol))?;
        }
        Ok(())
    }

    pub fn unsubscribe_bbo(&self, manager: &Arc<Manager>, symbols: &[&str]) -> Result<()> {
        for symbol in symbols {
            self.send(manager, crate::types::PacificaWssMessage::bbo_unsub(symbol))?;
        }
        Ok(())
    }

    /// `prices` channel: mark, mid, oracle and funding for every symbol in one stream.
    pub fn subscribe_prices(&self, manager: &Arc<Manager>) -> Result<()> {
        self.send(manager, crate::types::PacificaWssMessage::prices())
    }

    pub fn unsubscribe_prices(&self, manager: &Arc<Manager>) -> Result<()> {
        self.send(manager, crate::types::PacificaWssMessage::prices_unsub())
    }
}

#[async_trait]
pub trait PacificaApi {
    async fn get_markets(&self) -> Result<Vec<PacificaMarket>, BoxError>;

    async fn get_tickers(&self) -> Result<HashMap<String, PacificaTicker>, BoxError>;

    async fn get_funding_rate_history(
        &self,
        symbol: &str,
        limit: Option<u32>,
        cursor: Option<&str>,
    ) -> Result<PacificaFundingRatePage, BoxError>;

    async fn get_orderbook(
        &self,
        symbol: &str,
        agg_level: Option<u32>,
    ) -> Result<PacificaOrderBook, BoxError>;

    /// `None` returns trades across all markets.
    async fn get_recent_trades(&self, symbol: Option<&str>)
        -> Result<Vec<PacificaTrade>, BoxError>;

    async fn get_klines(
        &self,
        symbol: &str,
        interval: &str,
        start_time: i64,
        end_time: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<PacificaKline>, BoxError>;

    async fn get_mark_price_klines(
        &self,
        symbol: &str,
        interval: &str,
        start_time: i64,
        end_time: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<PacificaKline>, BoxError>;

    async fn get_spot_assets(
        &self,
        include_inactive: Option<bool>,
        collateral_enabled_only: Option<bool>,
    ) -> Result<Vec<PacificaSpotAsset>, BoxError>;
}

#[async_trait]
pub trait PacificaTradingApi {
    fn account(&self) -> Result<&str, BoxError>;

    async fn create_limit_order(
        &self,
        symbol: &str,
        price: f64,
        amount: f64,
        is_buy: bool,
        tif: &str,
        reduce_only: bool,
        client_order_id: Option<String>,
    ) -> Result<i64, BoxError>;

    async fn cancel_order(&self, symbol: &str, order_id: i64) -> Result<(), BoxError>;

    async fn edit_order(
        &self,
        symbol: &str,
        order_id: i64,
        price: f64,
        amount: f64,
    ) -> Result<i64, BoxError>;

    async fn get_open_orders(&self) -> Result<Vec<PacificaOpenOrder>, BoxError>;

    async fn get_positions(&self) -> Result<Vec<PacificaPosition>, BoxError>;
}

#[async_trait]
impl PacificaApi for PacificaClient {
    async fn get_markets(&self) -> Result<Vec<PacificaMarket>, BoxError> {
        self.market_api.get_markets().await
    }

    async fn get_tickers(&self) -> Result<HashMap<String, PacificaTicker>, BoxError> {
        self.market_api.get_tickers().await
    }

    async fn get_funding_rate_history(
        &self,
        symbol: &str,
        limit: Option<u32>,
        cursor: Option<&str>,
    ) -> Result<PacificaFundingRatePage, BoxError> {
        self.market_api
            .get_funding_rate_history(symbol, limit, cursor)
            .await
    }

    async fn get_orderbook(
        &self,
        symbol: &str,
        agg_level: Option<u32>,
    ) -> Result<PacificaOrderBook, BoxError> {
        self.market_api.get_orderbook(symbol, agg_level).await
    }

    async fn get_recent_trades(
        &self,
        symbol: Option<&str>,
    ) -> Result<Vec<PacificaTrade>, BoxError> {
        self.market_api.get_recent_trades(symbol).await
    }

    async fn get_klines(
        &self,
        symbol: &str,
        interval: &str,
        start_time: i64,
        end_time: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<PacificaKline>, BoxError> {
        self.market_api
            .get_klines(symbol, interval, start_time, end_time, limit)
            .await
    }

    async fn get_mark_price_klines(
        &self,
        symbol: &str,
        interval: &str,
        start_time: i64,
        end_time: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<PacificaKline>, BoxError> {
        self.market_api
            .get_mark_price_klines(symbol, interval, start_time, end_time, limit)
            .await
    }

    async fn get_spot_assets(
        &self,
        include_inactive: Option<bool>,
        collateral_enabled_only: Option<bool>,
    ) -> Result<Vec<PacificaSpotAsset>, BoxError> {
        self.market_api
            .get_spot_assets(include_inactive, collateral_enabled_only)
            .await
    }
}

#[async_trait]
impl PacificaTradingApi for PacificaClient {
    fn account(&self) -> Result<&str, BoxError> {
        Ok(self.trading()?.account())
    }

    async fn create_limit_order(
        &self,
        symbol: &str,
        price: f64,
        amount: f64,
        is_buy: bool,
        tif: &str,
        reduce_only: bool,
        client_order_id: Option<String>,
    ) -> Result<i64, BoxError> {
        self.trading()?
            .create_limit_order(
                symbol,
                price,
                amount,
                is_buy,
                tif,
                reduce_only,
                client_order_id,
            )
            .await
    }

    async fn cancel_order(&self, symbol: &str, order_id: i64) -> Result<(), BoxError> {
        self.trading()?.cancel_order(symbol, order_id).await
    }

    async fn edit_order(
        &self,
        symbol: &str,
        order_id: i64,
        price: f64,
        amount: f64,
    ) -> Result<i64, BoxError> {
        self.trading()?
            .edit_order(symbol, order_id, price, amount)
            .await
    }

    async fn get_open_orders(&self) -> Result<Vec<PacificaOpenOrder>, BoxError> {
        self.trading()?.get_open_orders().await
    }

    async fn get_positions(&self) -> Result<Vec<PacificaPosition>, BoxError> {
        self.trading()?.get_positions().await
    }
}
