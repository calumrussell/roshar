use crate::handlers::MessageHandler;
use async_trait::async_trait;
use log::{info, warn};
use roshar_types::WebsocketSupportedExchanges;
use crate::{Manager, Message};
use std::collections::HashMap;
use std::sync::Arc;

/// Handler that automatically subscribes to depth and trade feeds for configured coins
/// on successful WebSocket handshake.
pub struct ExchangeSubscriptionManager {
    manager: Arc<Manager>,
    connection_name: String,
    exchange: WebsocketSupportedExchanges,
    channels: HashMap<String, Vec<String>>,
}

impl ExchangeSubscriptionManager {
    /// Create a new exchange subscription manager
    ///
    /// # Arguments
    /// * `manager` - The Manager instance
    /// * `connection_name` - Name of the connection (must match Manager connection name)
    /// * `exchange` - Exchange type
    /// * `channels` - HashMap of channel types to coin lists (e.g., {"depth": ["BTCUSDT"], "trades": ["ETHUSDT"]})
    pub fn new(
        manager: Arc<Manager>,
        connection_name: String,
        exchange: WebsocketSupportedExchanges,
        channels: HashMap<String, Vec<String>>,
    ) -> Self {
        Self {
            manager,
            connection_name,
            exchange,
            channels,
        }
    }

    /// Subscribe to all configured channels for this exchange
    fn subscribe_to_channels(&self) -> anyhow::Result<()> {
        info!("[{}] → Subscribing to channels", self.connection_name);

        match self.exchange {
            WebsocketSupportedExchanges::Binance => {
                self.subscribe_binance()?;
            }
            WebsocketSupportedExchanges::ByBit => {
                self.subscribe_individual()?;
            }
            WebsocketSupportedExchanges::Hyperliquid => {
                self.subscribe_individual()?;
            }
            WebsocketSupportedExchanges::Kraken => {
                self.subscribe_individual()?;
            }
        }

        Ok(())
    }

    /// Subscribe to Binance channels (supports batching)
    fn subscribe_binance(&self) -> anyhow::Result<()> {
        let mut depth_symbols = Vec::new();
        let mut trade_symbols = Vec::new();

        for (channel_type, symbols) in &self.channels {
            if channel_type == "depth" {
                depth_symbols.extend(symbols.clone());
            } else if channel_type == "trades" {
                trade_symbols.extend(symbols.clone());
            }
        }

        if !depth_symbols.is_empty() {
            if let Some(msg) = self.exchange.batch_depth(&depth_symbols) {
                self.manager.write(
                    &self.connection_name,
                    Message::TextMessage(self.connection_name.clone(), msg),
                )?;
                info!(
                    "[{}] ✓ Subscribed to depth for {} symbols",
                    self.connection_name,
                    depth_symbols.len()
                );
            }
        }

        if !trade_symbols.is_empty() {
            if let Some(msg) = self.exchange.batch_trades(&trade_symbols) {
                self.manager.write(
                    &self.connection_name,
                    Message::TextMessage(self.connection_name.clone(), msg),
                )?;
                info!(
                    "[{}] ✓ Subscribed to trades for {} symbols",
                    self.connection_name,
                    trade_symbols.len()
                );
            }
        }

        Ok(())
    }

    /// Subscribe to channels individually (for exchanges that don't support batching)
    fn subscribe_individual(&self) -> anyhow::Result<()> {
        for (channel_type, symbols) in &self.channels {
            for symbol in symbols {
                let msg = match channel_type.as_str() {
                    "depth" => self.exchange.depth(symbol),
                    "trades" => self.exchange.trades(symbol),
                    _ => continue,
                };

                self.manager.write(
                    &self.connection_name,
                    Message::TextMessage(self.connection_name.clone(), msg),
                )?;
            }
        }

        info!(
            "[{}] ✓ Subscribed to all configured channels",
            self.connection_name
        );
        Ok(())
    }
}

#[async_trait]
impl MessageHandler for ExchangeSubscriptionManager {
    async fn on_message(&self, message: &Message) {
        if let Message::SuccessfulHandshake(name) = message {
            if name == &self.connection_name {
                if let Err(e) = self.subscribe_to_channels() {
                    warn!(
                        "[{}] ⚠ Subscription manager error: {}",
                        self.connection_name, e
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_manager() {
        let manager = Manager::new();
        let channels = HashMap::new();
        let mgr = ExchangeSubscriptionManager::new(
            manager,
            "test".to_string(),
            WebsocketSupportedExchanges::Binance,
            channels,
        );
        assert_eq!(mgr.connection_name, "test");
    }

    #[tokio::test]
    async fn test_handles_successful_handshake() {
        let manager = Manager::new();
        let channels = HashMap::new();
        let mgr = ExchangeSubscriptionManager::new(
            manager,
            "test".to_string(),
            WebsocketSupportedExchanges::Binance,
            channels,
        );

        // Should handle SuccessfulHandshake for matching name
        let msg = Message::SuccessfulHandshake("test".to_string());
        mgr.on_message(&msg).await;
    }

    #[tokio::test]
    async fn test_ignores_other_messages() {
        let manager = Manager::new();
        let channels = HashMap::new();
        let mgr = ExchangeSubscriptionManager::new(
            manager,
            "test".to_string(),
            WebsocketSupportedExchanges::Binance,
            channels,
        );

        // Should ignore messages for other connections
        let msg = Message::SuccessfulHandshake("other".to_string());
        mgr.on_message(&msg).await;

        // Should ignore other message types
        let msg = Message::PongMessage("test".to_string(), "pong".to_string());
        mgr.on_message(&msg).await;
    }
}
