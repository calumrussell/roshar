use anyhow::Result;
use roshar_types::DeribitWssMessage;
use roshar_ws_mgr::{Config, Manager, Message};
use std::sync::Arc;

use crate::ws::ws_config_methods;
use crate::DERIBIT_WSS_URL;

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

    ws_config_methods!();

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
