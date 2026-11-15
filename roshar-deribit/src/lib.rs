/// Wire and message types for this exchange (websocket messages, REST payloads).
pub mod types;

// Endpoints
pub const DERIBIT_WSS_URL: &str = "wss://www.deribit.com/ws/api/v2";
pub const DERIBIT_TESTNET_WSS_URL: &str = "wss://test.deribit.com/ws/api/v2";

use crate::types::DeribitWssMessage;
use anyhow::Result;
use roshar_ws_mgr::{Config, Manager, Message};
use std::sync::Arc;

pub struct DeribitClient {
    ws_config: Config,
}

impl DeribitClient {
    const WS_CONN_NAME: &str = "deribit";

    pub fn new() -> Self {
        Self {
            ws_config: Config {
                name: Self::WS_CONN_NAME.to_string(),
                url: DERIBIT_WSS_URL.to_string(),
                ping_duration: 30,
                ping_message: DeribitWssMessage::test(1).to_json(),
                ping_timeout: 10,
                reconnect_timeout: 90,
                use_text_ping: Some(true),
                read_buffer_size: Some(33554432),
                write_buffer_size: Some(2097152),
                max_message_size: Some(41943040),
                max_frame_size: Some(20971520),
                tcp_recv_buffer_size: Some(16777216),
                tcp_send_buffer_size: Some(4194304),
                tcp_nodelay: Some(true),
                broadcast_channel_size: Some(131072),
            },
        }
    }

    pub fn ws_config(&self) -> roshar_ws_mgr::Config {
        self.ws_config.clone()
    }

    pub fn ws_config_mut(&mut self) -> &mut roshar_ws_mgr::Config {
        &mut self.ws_config
    }

    pub fn subscribe_orderbook(
        &self,
        manager: &Arc<Manager>,
        instruments: &[&str],
        interval: &str,
    ) -> Result<()> {
        let channels: Vec<String> = instruments
            .iter()
            .map(|i| format!("book.{}.{}", i, interval))
            .collect();
        let msg = DeribitWssMessage::subscribe(&channels, 1).to_json();
        manager.write(
            &self.ws_config.name,
            Message::TextMessage(self.ws_config.name.clone(), msg),
        )?;
        Ok(())
    }

    pub fn subscribe_trades(
        &self,
        manager: &Arc<Manager>,
        instruments: &[&str],
        interval: &str,
    ) -> Result<()> {
        let channels: Vec<String> = instruments
            .iter()
            .map(|i| format!("trades.{}.{}", i, interval))
            .collect();
        let msg = DeribitWssMessage::subscribe(&channels, 1).to_json();
        manager.write(
            &self.ws_config.name,
            Message::TextMessage(self.ws_config.name.clone(), msg),
        )?;
        Ok(())
    }

    pub fn subscribe_ticker(
        &self,
        manager: &Arc<Manager>,
        instruments: &[&str],
        interval: &str,
    ) -> Result<()> {
        let channels: Vec<String> = instruments
            .iter()
            .map(|i| format!("ticker.{}.{}", i, interval))
            .collect();
        let msg = DeribitWssMessage::subscribe(&channels, 1).to_json();
        manager.write(
            &self.ws_config.name,
            Message::TextMessage(self.ws_config.name.clone(), msg),
        )?;
        Ok(())
    }

    pub fn unsubscribe_orderbook(
        &self,
        manager: &Arc<Manager>,
        instruments: &[&str],
        interval: &str,
    ) -> Result<()> {
        let channels: Vec<String> = instruments
            .iter()
            .map(|i| format!("book.{}.{}", i, interval))
            .collect();
        let msg = DeribitWssMessage::unsubscribe(&channels, 1).to_json();
        manager.write(
            &self.ws_config.name,
            Message::TextMessage(self.ws_config.name.clone(), msg),
        )?;
        Ok(())
    }

    pub fn unsubscribe_trades(
        &self,
        manager: &Arc<Manager>,
        instruments: &[&str],
        interval: &str,
    ) -> Result<()> {
        let channels: Vec<String> = instruments
            .iter()
            .map(|i| format!("trades.{}.{}", i, interval))
            .collect();
        let msg = DeribitWssMessage::unsubscribe(&channels, 1).to_json();
        manager.write(
            &self.ws_config.name,
            Message::TextMessage(self.ws_config.name.clone(), msg),
        )?;
        Ok(())
    }

    pub fn unsubscribe_ticker(
        &self,
        manager: &Arc<Manager>,
        instruments: &[&str],
        interval: &str,
    ) -> Result<()> {
        let channels: Vec<String> = instruments
            .iter()
            .map(|i| format!("ticker.{}.{}", i, interval))
            .collect();
        let msg = DeribitWssMessage::unsubscribe(&channels, 1).to_json();
        manager.write(
            &self.ws_config.name,
            Message::TextMessage(self.ws_config.name.clone(), msg),
        )?;
        Ok(())
    }

    pub fn set_heartbeat(&self, manager: &Arc<Manager>, interval: u32) -> Result<()> {
        let msg = DeribitWssMessage::set_heartbeat(interval, 1).to_json();
        manager.write(
            &self.ws_config.name,
            Message::TextMessage(self.ws_config.name.clone(), msg),
        )?;
        Ok(())
    }
}

impl Default for DeribitClient {
    fn default() -> Self {
        Self::new()
    }
}
