/// Wire and message types for this exchange (websocket messages, REST payloads).
pub mod types;

// Endpoints
pub const BYBIT_WSS_URL: &str = "wss://stream.bybit.com/v5/public/linear";
pub const BYBIT_SPOT_WSS_URL: &str = "wss://stream.bybit.com/v5/public/spot";

pub(crate) mod rest;

use rest::MarketApi;

pub use crate::types::{ByBitHistoricalFundingRate, ByBitKlineInterval, ByBitOrderbookDepth};
pub use rest::{
    ByBitInstrumentInfo, ByBitLeverageFilter, ByBitLotSizeFilter, ByBitPriceFilter,
    ByBitTickerData, ByBitTickersResponse, MarketApi as ByBitMarketApi,
};

use crate::types::ByBitWssMessage;
use anyhow::Result;
use async_trait::async_trait;
use roshar_ws_mgr::{Config, Manager, Message};
use std::collections::HashMap;
use std::sync::Arc;

pub struct ByBitClient {
    market_api: MarketApi,
    ws_config: Config,
}

impl ByBitClient {
    const DEFAULT_REQUESTS_PER_SECOND: u32 = 10;
    const WS_CONN_NAME: &str = "bybit";

    pub fn new() -> Self {
        Self::new_with_rate_limit(Self::DEFAULT_REQUESTS_PER_SECOND)
    }

    pub fn new_with_rate_limit(requests_per_second: u32) -> Self {
        Self {
            market_api: MarketApi::new(requests_per_second),
            ws_config: Config {
                name: Self::WS_CONN_NAME.to_string(),
                url: BYBIT_WSS_URL.to_string(),
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
            },
        }
    }

    pub fn ws_config(&self) -> roshar_ws_mgr::Config {
        self.ws_config.clone()
    }

    pub fn ws_config_mut(&mut self) -> &mut roshar_ws_mgr::Config {
        &mut self.ws_config
    }

    pub fn subscribe_depth(
        &self,
        manager: &Arc<Manager>,
        symbols: &[&str],
        depth: ByBitOrderbookDepth,
    ) -> Result<()> {
        for symbol in symbols {
            let msg = ByBitWssMessage::depth(symbol, depth).to_json();
            manager.write(
                &self.ws_config.name,
                Message::TextMessage(self.ws_config.name.clone(), msg),
            )?;
        }
        Ok(())
    }

    pub fn subscribe_trades(&self, manager: &Arc<Manager>, symbols: &[&str]) -> Result<()> {
        for symbol in symbols {
            let msg = ByBitWssMessage::trades(symbol).to_json();
            manager.write(
                &self.ws_config.name,
                Message::TextMessage(self.ws_config.name.clone(), msg),
            )?;
        }
        Ok(())
    }

    pub fn subscribe_candles(
        &self,
        manager: &Arc<Manager>,
        symbols: &[&str],
        interval: ByBitKlineInterval,
    ) -> Result<()> {
        for symbol in symbols {
            let msg = ByBitWssMessage::candle(symbol, interval).to_json();
            manager.write(
                &self.ws_config.name,
                Message::TextMessage(self.ws_config.name.clone(), msg),
            )?;
        }
        Ok(())
    }

    pub fn unsubscribe_depth(
        &self,
        manager: &Arc<Manager>,
        symbols: &[&str],
        depth: ByBitOrderbookDepth,
    ) -> Result<()> {
        for symbol in symbols {
            let msg = ByBitWssMessage::depth_unsub(symbol, depth).to_json();
            manager.write(
                &self.ws_config.name,
                Message::TextMessage(self.ws_config.name.clone(), msg),
            )?;
        }
        Ok(())
    }

    pub fn unsubscribe_trades(&self, manager: &Arc<Manager>, symbols: &[&str]) -> Result<()> {
        for symbol in symbols {
            let msg = ByBitWssMessage::trades_unsub(symbol).to_json();
            manager.write(
                &self.ws_config.name,
                Message::TextMessage(self.ws_config.name.clone(), msg),
            )?;
        }
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
    async fn get_realtime_funding_rates(
        &self,
    ) -> Result<HashMap<String, ByBitTickerData>, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api.get_tickers().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_realtime_funding_rates() {
        let market_api = MarketApi::new(10);
        let result = market_api.get_tickers().await;

        assert!(
            result.is_ok(),
            "Failed to fetch realtime funding rates: {:?}",
            result.err()
        );

        let tickers = result.unwrap();

        assert!(!tickers.is_empty(), "Expected some tickers, got none");

        assert!(
            tickers.contains_key("BTCUSDT"),
            "Expected BTCUSDT ticker to be present"
        );

        let btc_ticker = tickers.get("BTCUSDT").unwrap();
        assert!(
            !btc_ticker.funding_rate.is_empty(),
            "Expected funding_rate to be present"
        );
        assert!(
            !btc_ticker.next_funding_time.is_empty(),
            "Expected next_funding_time to be present"
        );

        println!(
            "Fetched {} tickers with funding rates. BTCUSDT funding_rate: {}, next_funding_time: {}",
            tickers.len(),
            btc_ticker.funding_rate,
            btc_ticker.next_funding_time
        );
    }
}
