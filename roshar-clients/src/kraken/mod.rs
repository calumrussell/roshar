pub(crate) mod rest;

use crate::http::RateLimitedClient;
use rest::{ChartsApi, MarketApi};
pub use rest::{
    KrakenGetLeverageResponse, KrakenLeveragePreference, KrakenLeverageSettingResponse,
    KrakenModifyResponse, KrakenOpenOrdersResponse, KrakenOrder, KrakenOrderResponse,
    KrakenOrderStatusResponse, KrakenRestCandleData, KrakenRestCandleResponse, KrakenTickerData,
    MultiCollateralApi, OrderManagementApi,
};

pub use rest::charts::Candle;

use async_trait::async_trait;
use anyhow::Result;
use roshar_types::KrakenWssMessage;
use roshar_ws_mgr::{Config, Manager, Message};
use std::sync::Arc;

use crate::KRAKEN_WSS_URL;
use crate::ws::ws_config_methods;

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

    ws_config_methods!();

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
    async fn fetch_candles(&self, symbol: &str) -> Result<Vec<Candle>, String>;
    async fn get_tickers(
        &self,
    ) -> Result<std::collections::HashMap<String, KrakenTickerData>, String>;
    async fn get_all_funding_rates_with_size(
        &self,
    ) -> Result<Vec<(String, f64, f64, f64)>, String>;
}

#[async_trait]
impl KrakenApi for KrakenClient {
    async fn fetch_candles(&self, symbol: &str) -> Result<Vec<Candle>, String> {
        self.charts_api
            .fetch_candle(symbol)
            .await
            .map_err(|e| format!("Failed to fetch candles: {}", e))
    }

    async fn get_tickers(
        &self,
    ) -> Result<std::collections::HashMap<String, KrakenTickerData>, String> {
        self.market_api
            .get_tickers()
            .await
            .map_err(|e| format!("Failed to get tickers: {}", e))
    }

    async fn get_all_funding_rates_with_size(
        &self,
    ) -> Result<Vec<(String, f64, f64, f64)>, String> {
        self.market_api
            .get_all_funding_rates_with_size()
            .await
            .map_err(|e| format!("Failed to get funding rates: {}", e))
    }
}
