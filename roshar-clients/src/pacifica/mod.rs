pub(crate) mod rest;

use rest::MarketApi;
use rest::TradingApi;

pub use rest::MarketApi as PacificaMarketApi;
pub use rest::TradingApi as PacificaTradingApiImpl;

use anyhow::Result;
use async_trait::async_trait;
use roshar_types::pacifica::{
    PacificaFundingRate, PacificaKline, PacificaMarket, PacificaOpenOrder, PacificaOrderBook,
    PacificaPosition, PacificaTicker, PacificaTrade,
};
use roshar_ws_mgr::{Config, Manager, Message};
use std::collections::HashMap;
use std::sync::Arc;

use crate::ws::ws_config_methods;
use crate::PACIFICA_WSS_URL;

pub struct PacificaClient {
    market_api: MarketApi,
    ws_config: Config,
    trading_api: Option<TradingApi>,
}

impl PacificaClient {
    const DEFAULT_REQUESTS_PER_SECOND: u32 = 10;
    const WS_CONN_NAME: &str = "pacifica";

    pub fn new() -> Self {
        Self::new_with_rate_limit(Self::DEFAULT_REQUESTS_PER_SECOND)
    }

    pub fn new_with_rate_limit(requests_per_second: u32) -> Self {
        Self {
            market_api: MarketApi::new(requests_per_second),
            ws_config: Config {
                name: Self::WS_CONN_NAME.to_string(),
                url: PACIFICA_WSS_URL.to_string(),
                ping_duration: 30,
                ping_message: roshar_types::pacifica::PacificaWssMessage::ping().to_json(),
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
            trading_api: None,
        }
    }

    pub fn new_with_credentials(
        private_key_b58: &str,
        account: String,
        is_mainnet: bool,
    ) -> Result<Self> {
        let trading_api = TradingApi::new(
            Self::DEFAULT_REQUESTS_PER_SECOND,
            private_key_b58,
            account,
            is_mainnet,
        )?;
        Ok(Self {
            market_api: MarketApi::new(Self::DEFAULT_REQUESTS_PER_SECOND),
            ws_config: Config {
                name: Self::WS_CONN_NAME.to_string(),
                url: PACIFICA_WSS_URL.to_string(),
                ping_duration: 30,
                ping_message: roshar_types::pacifica::PacificaWssMessage::ping().to_json(),
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
            trading_api: Some(trading_api),
        })
    }

    fn trading(&self) -> Result<&TradingApi, Box<dyn std::error::Error + Send + Sync>> {
        self.trading_api
            .as_ref()
            .ok_or_else(|| "PacificaClient was not created with credentials".into())
    }

    ws_config_methods!();

    pub fn subscribe_depth(&self, manager: &Arc<Manager>, symbols: &[&str]) -> Result<()> {
        for symbol in symbols {
            let msg = roshar_types::pacifica::PacificaWssMessage::depth(symbol).to_json();
            manager.write(
                &self.ws_config.name,
                Message::TextMessage(self.ws_config.name.clone(), msg),
            )?;
        }
        Ok(())
    }

    pub fn unsubscribe_depth(&self, manager: &Arc<Manager>, symbols: &[&str]) -> Result<()> {
        for symbol in symbols {
            let msg = roshar_types::pacifica::PacificaWssMessage::depth_unsub(symbol).to_json();
            manager.write(
                &self.ws_config.name,
                Message::TextMessage(self.ws_config.name.clone(), msg),
            )?;
        }
        Ok(())
    }

    pub fn subscribe_trades(&self, manager: &Arc<Manager>, symbols: &[&str]) -> Result<()> {
        for symbol in symbols {
            let msg = roshar_types::pacifica::PacificaWssMessage::trades(symbol).to_json();
            manager.write(
                &self.ws_config.name,
                Message::TextMessage(self.ws_config.name.clone(), msg),
            )?;
        }
        Ok(())
    }

    pub fn unsubscribe_trades(&self, manager: &Arc<Manager>, symbols: &[&str]) -> Result<()> {
        for symbol in symbols {
            let msg = roshar_types::pacifica::PacificaWssMessage::trades_unsub(symbol).to_json();
            manager.write(
                &self.ws_config.name,
                Message::TextMessage(self.ws_config.name.clone(), msg),
            )?;
        }
        Ok(())
    }

    pub fn subscribe_candles(&self, manager: &Arc<Manager>, symbols: &[&str]) -> Result<()> {
        for symbol in symbols {
            let msg = roshar_types::pacifica::PacificaWssMessage::candles(symbol).to_json();
            manager.write(
                &self.ws_config.name,
                Message::TextMessage(self.ws_config.name.clone(), msg),
            )?;
        }
        Ok(())
    }

    pub fn subscribe_mark_price(&self, manager: &Arc<Manager>, symbols: &[&str]) -> Result<()> {
        for symbol in symbols {
            let msg = roshar_types::pacifica::PacificaWssMessage::mark_price(symbol).to_json();
            manager.write(
                &self.ws_config.name,
                Message::TextMessage(self.ws_config.name.clone(), msg),
            )?;
        }
        Ok(())
    }
}

#[async_trait]
pub trait PacificaApi {
    async fn get_markets(
        &self,
    ) -> Result<Vec<PacificaMarket>, Box<dyn std::error::Error + Send + Sync>>;

    async fn get_tickers(
        &self,
    ) -> Result<HashMap<String, PacificaTicker>, Box<dyn std::error::Error + Send + Sync>>;

    async fn get_funding_rates(
        &self,
        symbol: Option<&str>,
    ) -> Result<Vec<PacificaFundingRate>, Box<dyn std::error::Error + Send + Sync>>;

    async fn get_orderbook(
        &self,
        symbol: &str,
        limit: Option<u32>,
    ) -> Result<PacificaOrderBook, Box<dyn std::error::Error + Send + Sync>>;

    async fn get_recent_trades(
        &self,
        symbol: &str,
        limit: Option<u32>,
    ) -> Result<Vec<PacificaTrade>, Box<dyn std::error::Error + Send + Sync>>;

    async fn get_klines(
        &self,
        symbol: &str,
        interval: &str,
        start_time: Option<i64>,
        end_time: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<PacificaKline>, Box<dyn std::error::Error + Send + Sync>>;
}

#[async_trait]
pub trait PacificaTradingApi {
    fn account(&self) -> Result<&str, Box<dyn std::error::Error + Send + Sync>>;

    async fn create_limit_order(
        &self,
        symbol: &str,
        price: f64,
        amount: f64,
        is_buy: bool,
        tif: &str,
        reduce_only: bool,
        client_order_id: Option<String>,
    ) -> Result<i64, Box<dyn std::error::Error + Send + Sync>>;

    async fn cancel_order(
        &self,
        symbol: &str,
        order_id: i64,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    async fn edit_order(
        &self,
        symbol: &str,
        order_id: i64,
        price: f64,
        amount: f64,
    ) -> Result<i64, Box<dyn std::error::Error + Send + Sync>>;

    async fn get_open_orders(
        &self,
    ) -> Result<Vec<PacificaOpenOrder>, Box<dyn std::error::Error + Send + Sync>>;

    async fn get_positions(
        &self,
    ) -> Result<Vec<PacificaPosition>, Box<dyn std::error::Error + Send + Sync>>;
}

#[async_trait]
impl PacificaApi for PacificaClient {
    async fn get_markets(
        &self,
    ) -> Result<Vec<PacificaMarket>, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api.get_markets().await
    }

    async fn get_tickers(
        &self,
    ) -> Result<HashMap<String, PacificaTicker>, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api.get_tickers().await
    }

    async fn get_funding_rates(
        &self,
        symbol: Option<&str>,
    ) -> Result<Vec<PacificaFundingRate>, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api.get_funding_rates(symbol).await
    }

    async fn get_orderbook(
        &self,
        symbol: &str,
        limit: Option<u32>,
    ) -> Result<PacificaOrderBook, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api.get_orderbook(symbol, limit).await
    }

    async fn get_recent_trades(
        &self,
        symbol: &str,
        limit: Option<u32>,
    ) -> Result<Vec<PacificaTrade>, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api.get_recent_trades(symbol, limit).await
    }

    async fn get_klines(
        &self,
        symbol: &str,
        interval: &str,
        start_time: Option<i64>,
        end_time: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<PacificaKline>, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api
            .get_klines(symbol, interval, start_time, end_time, limit)
            .await
    }
}

#[async_trait]
impl PacificaTradingApi for PacificaClient {
    fn account(&self) -> Result<&str, Box<dyn std::error::Error + Send + Sync>> {
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
    ) -> Result<i64, Box<dyn std::error::Error + Send + Sync>> {
        self.trading()?
            .create_limit_order(symbol, price, amount, is_buy, tif, reduce_only, client_order_id)
            .await
    }

    async fn cancel_order(
        &self,
        symbol: &str,
        order_id: i64,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.trading()?.cancel_order(symbol, order_id).await
    }

    async fn edit_order(
        &self,
        symbol: &str,
        order_id: i64,
        price: f64,
        amount: f64,
    ) -> Result<i64, Box<dyn std::error::Error + Send + Sync>> {
        self.trading()?.edit_order(symbol, order_id, price, amount).await
    }

    async fn get_open_orders(
        &self,
    ) -> Result<Vec<PacificaOpenOrder>, Box<dyn std::error::Error + Send + Sync>> {
        self.trading()?.get_open_orders().await
    }

    async fn get_positions(
        &self,
    ) -> Result<Vec<PacificaPosition>, Box<dyn std::error::Error + Send + Sync>> {
        self.trading()?.get_positions().await
    }
}
