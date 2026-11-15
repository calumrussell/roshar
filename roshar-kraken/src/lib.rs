// Endpoints
pub const KRAKEN_WSS_URL: &str = "wss://futures.kraken.com/ws/v1";
pub const KRAKEN_SPOT_WSS_URL: &str = "wss://ws.kraken.com/v2";

pub(crate) mod rest;

use rest::{ChartsApi, MarketApi};
pub use rest::{
    KrakenCredentials, KrakenGetLeverageResponse, KrakenLeveragePreference,
    KrakenLeverageSettingResponse, KrakenModifyResponse, KrakenOpenOrdersResponse, KrakenOrder,
    KrakenOrderResponse, KrakenOrderStatusResponse, KrakenResolution, KrakenRestCandleData,
    KrakenRestCandleResponse, KrakenTickType, KrakenTickerData, MultiCollateralApi,
    OrderManagementApi,
};
use roshar_http::RateLimitedClient;

use anyhow::Result;
use async_trait::async_trait;
use roshar_types::KrakenWssMessage;
use roshar_ws_mgr::{Config, Manager, Message};
use std::sync::Arc;

pub struct KrakenClient {
    charts_api: ChartsApi,
    market_api: MarketApi,
    ws_config: Config,
}

impl KrakenClient {
    const WS_CONN_NAME: &str = "kraken";

    pub fn new(requests_per_second: u32) -> Self {
        let http_client = Arc::new(RateLimitedClient::new(requests_per_second, 1));

        Self {
            charts_api: ChartsApi::new_with_client(http_client.clone()),
            market_api: MarketApi::new_with_client(http_client),
            ws_config: Config {
                name: Self::WS_CONN_NAME.to_string(),
                url: KRAKEN_WSS_URL.to_string(),
                ping_duration: 30,
                ping_message: KrakenWssMessage::ping().to_json(),
                ping_timeout: 15,
                reconnect_timeout: 10000,
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
            let msg = KrakenWssMessage::depth(symbol).to_json();
            manager.write(
                &self.ws_config.name,
                Message::TextMessage(self.ws_config.name.clone(), msg),
            )?;
        }
        Ok(())
    }

    pub fn subscribe_trades(&self, manager: &Arc<Manager>, symbols: &[&str]) -> Result<()> {
        for symbol in symbols {
            let msg = KrakenWssMessage::trades(symbol).to_json();
            manager.write(
                &self.ws_config.name,
                Message::TextMessage(self.ws_config.name.clone(), msg),
            )?;
        }
        Ok(())
    }

    pub fn unsubscribe_depth(&self, manager: &Arc<Manager>, symbols: &[&str]) -> Result<()> {
        for symbol in symbols {
            let msg = KrakenWssMessage::depth_unsub(symbol).to_json();
            manager.write(
                &self.ws_config.name,
                Message::TextMessage(self.ws_config.name.clone(), msg),
            )?;
        }
        Ok(())
    }

    pub fn unsubscribe_trades(&self, manager: &Arc<Manager>, symbols: &[&str]) -> Result<()> {
        for symbol in symbols {
            let msg = KrakenWssMessage::trades_unsub(symbol).to_json();
            manager.write(
                &self.ws_config.name,
                Message::TextMessage(self.ws_config.name.clone(), msg),
            )?;
        }
        Ok(())
    }
}

#[async_trait]
pub trait KrakenApi {
    /// Candles from the charts endpoint, as sent. `from`/`to` are in seconds.
    async fn get_candles(
        &self,
        tick_type: KrakenTickType,
        symbol: &str,
        resolution: KrakenResolution,
        from: Option<i64>,
        to: Option<i64>,
    ) -> Result<KrakenRestCandleResponse, String>;
    async fn get_tickers(
        &self,
    ) -> Result<std::collections::HashMap<String, KrakenTickerData>, String>;
}

#[async_trait]
impl KrakenApi for KrakenClient {
    async fn get_candles(
        &self,
        tick_type: KrakenTickType,
        symbol: &str,
        resolution: KrakenResolution,
        from: Option<i64>,
        to: Option<i64>,
    ) -> Result<KrakenRestCandleResponse, String> {
        self.charts_api
            .get_candles(tick_type, symbol, resolution, from, to)
            .await
            .map_err(|e| format!("Failed to get candles: {}", e))
    }

    async fn get_tickers(
        &self,
    ) -> Result<std::collections::HashMap<String, KrakenTickerData>, String> {
        self.market_api
            .get_tickers()
            .await
            .map_err(|e| format!("Failed to get tickers: {}", e))
    }
}
