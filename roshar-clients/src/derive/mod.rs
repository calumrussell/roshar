pub(crate) mod rest;

use rest::MarketApi;

pub use rest::{
    DeriveFundingRate, DeriveFundingRateHistoryResponse, DeriveInstrument,
    DeriveInstrumentsResponse, DeriveOptionPricing, DerivePerpDetails, DeriveStats, DeriveTicker,
    DeriveTickersResponse,
};

use anyhow::Result;
use async_trait::async_trait;
use roshar_ws_mgr::{Config, Manager, Message};
use std::collections::HashMap;
use std::sync::Arc;

use crate::constants::DERIVE_WSS_URL;
use crate::ws::ws_config_methods;

pub struct DeriveClient {
    market_api: MarketApi,
    ws_config: Config,
}

impl DeriveClient {
    const DEFAULT_REQUESTS_PER_SECOND: u32 = 10;
    const WS_CONN_NAME: &str = "derive";

    pub fn new() -> Self {
        Self::new_with_rate_limit(Self::DEFAULT_REQUESTS_PER_SECOND)
    }

    pub fn new_with_rate_limit(requests_per_second: u32) -> Self {
        Self {
            market_api: MarketApi::new(requests_per_second),
            ws_config: Config {
                name: Self::WS_CONN_NAME.to_string(),
                url: DERIVE_WSS_URL.to_string(),
                ping_duration: 20,
                ping_message: serde_json::json!({"method": "public/heartbeat", "params": {}, "id": 0}).to_string(),
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

    fn make_subscribe_msg(channels: &[String]) -> String {
        serde_json::json!({
            "method": "subscribe",
            "params": { "channels": channels },
            "id": 1
        })
        .to_string()
    }

    fn make_unsubscribe_msg(channels: &[String]) -> String {
        serde_json::json!({
            "method": "unsubscribe",
            "params": { "channels": channels },
            "id": 1
        })
        .to_string()
    }

    pub fn subscribe_trades(&self, manager: &Arc<Manager>, instruments: &[&str]) -> Result<()> {
        let channels: Vec<String> = instruments
            .iter()
            .map(|i| format!("trades.{i}"))
            .collect();
        let msg = Self::make_subscribe_msg(&channels);
        manager.write(
            &self.ws_config.name,
            Message::TextMessage(self.ws_config.name.clone(), msg),
        )?;
        Ok(())
    }

    pub fn unsubscribe_trades(&self, manager: &Arc<Manager>, instruments: &[&str]) -> Result<()> {
        let channels: Vec<String> = instruments
            .iter()
            .map(|i| format!("trades.{i}"))
            .collect();
        let msg = Self::make_unsubscribe_msg(&channels);
        manager.write(
            &self.ws_config.name,
            Message::TextMessage(self.ws_config.name.clone(), msg),
        )?;
        Ok(())
    }

    /// Subscribe to orderbook snapshots.
    ///
    /// * `group` - Price grouping: "1", "10", "100"
    /// * `depth` - Number of levels: "1", "10", "20", "100"
    pub fn subscribe_depth(
        &self,
        manager: &Arc<Manager>,
        instruments: &[&str],
        group: &str,
        depth: &str,
    ) -> Result<()> {
        let channels: Vec<String> = instruments
            .iter()
            .map(|i| format!("orderbook.{i}.{group}.{depth}"))
            .collect();
        let msg = Self::make_subscribe_msg(&channels);
        manager.write(
            &self.ws_config.name,
            Message::TextMessage(self.ws_config.name.clone(), msg),
        )?;
        Ok(())
    }

    pub fn unsubscribe_depth(
        &self,
        manager: &Arc<Manager>,
        instruments: &[&str],
        group: &str,
        depth: &str,
    ) -> Result<()> {
        let channels: Vec<String> = instruments
            .iter()
            .map(|i| format!("orderbook.{i}.{group}.{depth}"))
            .collect();
        let msg = Self::make_unsubscribe_msg(&channels);
        manager.write(
            &self.ws_config.name,
            Message::TextMessage(self.ws_config.name.clone(), msg),
        )?;
        Ok(())
    }

    /// Subscribe to periodic ticker snapshots.
    pub fn subscribe_tickers(
        &self,
        manager: &Arc<Manager>,
        instruments: &[&str],
        interval: &str,
    ) -> Result<()> {
        let channels: Vec<String> = instruments
            .iter()
            .map(|i| format!("ticker.{i}.{interval}"))
            .collect();
        let msg = Self::make_subscribe_msg(&channels);
        manager.write(
            &self.ws_config.name,
            Message::TextMessage(self.ws_config.name.clone(), msg),
        )?;
        Ok(())
    }

    pub fn unsubscribe_tickers(
        &self,
        manager: &Arc<Manager>,
        instruments: &[&str],
        interval: &str,
    ) -> Result<()> {
        let channels: Vec<String> = instruments
            .iter()
            .map(|i| format!("ticker.{i}.{interval}"))
            .collect();
        let msg = Self::make_unsubscribe_msg(&channels);
        manager.write(
            &self.ws_config.name,
            Message::TextMessage(self.ws_config.name.clone(), msg),
        )?;
        Ok(())
    }
}

#[async_trait]
pub trait DeriveApi {
    async fn get_instruments(
        &self,
        currency: &str,
        instrument_type: &str,
        expired: bool,
    ) -> Result<Vec<DeriveInstrument>, Box<dyn std::error::Error + Send + Sync>>;

    async fn get_funding_rate_history(
        &self,
        instrument_name: &str,
        start_timestamp: Option<i64>,
        end_timestamp: Option<i64>,
        period: Option<&str>,
    ) -> Result<Vec<DeriveFundingRate>, Box<dyn std::error::Error + Send + Sync>>;

    async fn get_tickers(
        &self,
        instrument_type: &str,
        currency: Option<&str>,
    ) -> Result<HashMap<String, DeriveTicker>, Box<dyn std::error::Error + Send + Sync>>;
}

#[async_trait]
impl DeriveApi for DeriveClient {
    async fn get_instruments(
        &self,
        currency: &str,
        instrument_type: &str,
        expired: bool,
    ) -> Result<Vec<DeriveInstrument>, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api
            .get_instruments(currency, instrument_type, expired)
            .await
    }

    async fn get_funding_rate_history(
        &self,
        instrument_name: &str,
        start_timestamp: Option<i64>,
        end_timestamp: Option<i64>,
        period: Option<&str>,
    ) -> Result<Vec<DeriveFundingRate>, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api
            .get_funding_rate_history(instrument_name, start_timestamp, end_timestamp, period)
            .await
    }

    async fn get_tickers(
        &self,
        instrument_type: &str,
        currency: Option<&str>,
    ) -> Result<HashMap<String, DeriveTicker>, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api
            .get_tickers(instrument_type, currency)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_derive_client_instruments() {
        let client = DeriveClient::new();
        let result = client.get_instruments("ETH", "perp", false).await;

        assert!(
            result.is_ok(),
            "Failed to fetch Derive instruments: {:?}",
            result.err()
        );

        let instruments = result.unwrap();
        assert!(
            !instruments.is_empty(),
            "Expected some instruments, got none"
        );

        println!(
            "Derive client fetched {} ETH perp instruments",
            instruments.len()
        );
    }
}
