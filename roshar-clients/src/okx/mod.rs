pub(crate) mod rest;

use rest::MarketApi;

pub use rest::MarketApi as OkxMarketApi;
pub use rest::{OkxInstrumentsResponse, OkxTickerData, OkxTickersResponse};

use anyhow::Result;
use async_trait::async_trait;
use roshar_types::OkxWssMessage;
use roshar_ws_mgr::{Config, Manager, Message};
use std::collections::HashMap;
use std::sync::Arc;

use crate::ws::ws_config_methods;
use crate::OKX_WSS_URL;

pub struct OkxClient {
    market_api: MarketApi,
    ws_config: Config,
}

impl OkxClient {
    const DEFAULT_REQUESTS_PER_SECOND: u32 = 10;
    const WS_CONN_NAME: &str = "okx";

    pub fn new() -> Self {
        Self::new_with_rate_limit(Self::DEFAULT_REQUESTS_PER_SECOND)
    }

    pub fn new_with_rate_limit(requests_per_second: u32) -> Self {
        Self {
            market_api: MarketApi::new(requests_per_second),
            ws_config: Config {
                name: Self::WS_CONN_NAME.to_string(),
                url: OKX_WSS_URL.to_string(),
                ping_duration: 20,
                ping_message: OkxWssMessage::ping(),
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

    pub fn subscribe_depth(&self, manager: &Arc<Manager>, symbols: &[&str]) -> Result<()> {
        for symbol in symbols {
            let msg = OkxWssMessage::depth(symbol).to_json();
            manager.write(
                &self.ws_config.name,
                Message::TextMessage(self.ws_config.name.clone(), msg),
            )?;
        }
        Ok(())
    }

    pub fn subscribe_trades(&self, manager: &Arc<Manager>, symbols: &[&str]) -> Result<()> {
        for symbol in symbols {
            let msg = OkxWssMessage::trades(symbol).to_json();
            manager.write(
                &self.ws_config.name,
                Message::TextMessage(self.ws_config.name.clone(), msg),
            )?;
        }
        Ok(())
    }

    pub fn unsubscribe_depth(&self, manager: &Arc<Manager>, symbols: &[&str]) -> Result<()> {
        for symbol in symbols {
            let msg = OkxWssMessage::depth_unsub(symbol).to_json();
            manager.write(
                &self.ws_config.name,
                Message::TextMessage(self.ws_config.name.clone(), msg),
            )?;
        }
        Ok(())
    }

    pub fn unsubscribe_trades(&self, manager: &Arc<Manager>, symbols: &[&str]) -> Result<()> {
        for symbol in symbols {
            let msg = OkxWssMessage::trades_unsub(symbol).to_json();
            manager.write(
                &self.ws_config.name,
                Message::TextMessage(self.ws_config.name.clone(), msg),
            )?;
        }
        Ok(())
    }
}

#[async_trait]
pub trait OkxApi {
    async fn get_instruments_info(
        &self,
    ) -> Result<
        HashMap<String, roshar_types::OkxInstrumentInfo>,
        Box<dyn std::error::Error + Send + Sync>,
    >;
    async fn get_tickers(
        &self,
    ) -> Result<HashMap<String, OkxTickerData>, Box<dyn std::error::Error + Send + Sync>>;
    async fn get_candles(
        &self,
        inst_id: &str,
        bar: &str,
        after: Option<i64>,
        before: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<roshar_types::OkxCandle>, Box<dyn std::error::Error + Send + Sync>>;
}

#[async_trait]
impl OkxApi for OkxClient {
    async fn get_instruments_info(
        &self,
    ) -> Result<
        HashMap<String, roshar_types::OkxInstrumentInfo>,
        Box<dyn std::error::Error + Send + Sync>,
    > {
        self.market_api.get_instruments_info().await
    }

    async fn get_tickers(
        &self,
    ) -> Result<HashMap<String, OkxTickerData>, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api.get_tickers().await
    }

    async fn get_candles(
        &self,
        inst_id: &str,
        bar: &str,
        after: Option<i64>,
        before: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<roshar_types::OkxCandle>, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api
            .get_candles(inst_id, bar, after, before, limit)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_okx_client_instruments() {
        let market_api = MarketApi::new(10);
        let result = market_api.get_instruments_info().await;

        assert!(
            result.is_ok(),
            "Failed to fetch OKX instruments: {:?}",
            result.err()
        );

        let instruments = result.unwrap();
        assert!(
            !instruments.is_empty(),
            "Expected some instruments, got none"
        );

        println!("OKX client fetched {} SWAP instruments", instruments.len());
    }
}
