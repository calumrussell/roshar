/// Wire and message types for this exchange (websocket messages, REST payloads).
pub mod types;

pub(crate) mod rest;

pub use crate::types::*;
pub use rest::types::*;
pub use rest::{
    ByBitAuctionFeeInfo, ByBitInstrumentInfo, ByBitLeverageFilter, ByBitLotSizeFilter,
    ByBitPreListingInfo, ByBitPreListingPhase, ByBitPriceFilter, ByBitRiskParameters,
    ByBitTickerData, MarketApi as ByBitMarketApi,
};

use anyhow::Result;
use async_trait::async_trait;
use rest::MarketApi;
use roshar_ws_mgr::{Config, Manager, Message};
use std::collections::HashMap;
use std::sync::Arc;

pub struct ByBitClient {
    market_api: MarketApi,
    ws_stream: ByBitWsStream,
    ws_config: Config,
}

impl Default for ByBitClient {
    fn default() -> Self {
        Self::new()
    }
}

impl ByBitClient {
    const DEFAULT_REQUESTS_PER_SECOND: u32 = 10;

    /// Client on the linear websocket stream with the default REST rate.
    pub fn new() -> Self {
        Self::new_with_rate_limit(Self::DEFAULT_REQUESTS_PER_SECOND)
    }

    /// Client on the linear websocket stream.
    pub fn new_with_rate_limit(requests_per_second: u32) -> Self {
        Self::new_for_stream(ByBitWsStream::Linear, requests_per_second)
    }

    /// Client whose websocket config targets `stream` (one connection per stream).
    pub fn new_for_stream(stream: ByBitWsStream, requests_per_second: u32) -> Self {
        Self {
            market_api: MarketApi::new(requests_per_second),
            ws_stream: stream,
            ws_config: Config {
                name: stream.connection_name().to_string(),
                url: stream.url().to_string(),
                ping_duration: 20,
                ping_message: ByBitWssMessage::ping().to_json(),
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
                headers: vec![],
            },
        }
    }

    pub fn market(&self) -> &MarketApi {
        &self.market_api
    }

    pub fn ws_stream(&self) -> ByBitWsStream {
        self.ws_stream
    }

    pub fn ws_config(&self) -> roshar_ws_mgr::Config {
        self.ws_config.clone()
    }

    pub fn ws_config_mut(&mut self) -> &mut roshar_ws_mgr::Config {
        &mut self.ws_config
    }

    /// Sends one `subscribe` request per topic (spot caps `args` at 10 per request).
    pub fn subscribe(&self, manager: &Arc<Manager>, topics: &[ByBitTopic]) -> Result<()> {
        for topic in topics {
            self.send(
                manager,
                ByBitWssMessage::subscribe(std::slice::from_ref(topic)),
            )?;
        }
        Ok(())
    }

    /// Sends one `unsubscribe` request per topic.
    pub fn unsubscribe(&self, manager: &Arc<Manager>, topics: &[ByBitTopic]) -> Result<()> {
        for topic in topics {
            self.send(
                manager,
                ByBitWssMessage::unsubscribe(std::slice::from_ref(topic)),
            )?;
        }
        Ok(())
    }

    /// Parses a text frame received on this client's stream.
    pub fn parse_ws_message(&self, text: &str) -> Result<ByBitWsMessage, String> {
        ByBitWsMessage::parse(self.ws_stream, text)
    }

    fn send(&self, manager: &Arc<Manager>, msg: ByBitWssMessage) -> Result<()> {
        manager.write(
            &self.ws_config.name,
            Message::TextMessage(self.ws_config.name.clone(), msg.to_json()),
        )?;
        Ok(())
    }
}

#[async_trait]
pub trait ByBitApi {
    async fn get_realtime_funding_rates(
        &self,
    ) -> Result<HashMap<String, ByBitTickerData>, Box<dyn std::error::Error + Send + Sync>>;
}

#[async_trait]
impl ByBitApi for ByBitClient {
    /// Linear tickers keyed by symbol (funding rate and next funding time per perpetual).
    async fn get_realtime_funding_rates(
        &self,
    ) -> Result<HashMap<String, ByBitTickerData>, Box<dyn std::error::Error + Send + Sync>> {
        let tickers = self
            .market_api
            .get_tickers(ByBitContractCategory::Linear, None)
            .await?;
        Ok(tickers
            .list
            .into_iter()
            .map(|t| (t.symbol.clone(), t))
            .collect())
    }
}
