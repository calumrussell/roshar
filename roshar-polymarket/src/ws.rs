//! CLOB market websocket client (public order books, prices, trades and market lifecycle).

use crate::types::{MarketWsOperation, MarketWsSubscription, MarketWsUpdate};
use anyhow::Result;
use roshar_ws_mgr::{Config, Manager, Message};
use std::sync::Arc;

/// Public market channel. No authentication; reads work from the UK.
pub const POLYMARKET_MARKET_WSS_URL: &str = "wss://ws-subscriptions-clob.polymarket.com/ws/market";

/// Connection config and frame senders for the market channel. Parse received text frames with
/// [`crate::types::MarketWsMessage::parse`].
///
/// The first frame sent after connecting must be [`Self::subscribe`]; later changes use
/// [`Self::add_assets`] / [`Self::remove_assets`]. A second `subscribe` on the same connection is
/// answered `INVALID OPERATION`.
pub struct PolymarketMarketWs {
    ws_config: Config,
}

impl Default for PolymarketMarketWs {
    fn default() -> Self {
        Self::new()
    }
}

impl PolymarketMarketWs {
    const WS_CONN_NAME: &'static str = "polymarket-market";

    pub fn new() -> Self {
        Self {
            ws_config: Config {
                name: Self::WS_CONN_NAME.to_string(),
                url: POLYMARKET_MARKET_WSS_URL.to_string(),
                // The docs ask for the text frame `PING` every 10 s (answered `PONG`). That text
                // must not reach the server before the subscription frame (it closes the socket
                // with 1008 `invalid subscription payload`), and ws-mgr's first ping fires on
                // connect. Live, the server answers protocol ping frames with pong frames, which
                // ws-mgr times, and a subscribed connection with no pings at all stayed open for
                // 150 s. So this sends protocol pings every 10 s instead.
                ping_duration: 10,
                ping_message: "PING".to_string(),
                ping_timeout: 30,
                reconnect_timeout: 5000,
                use_text_ping: Some(false),
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

    pub fn ws_config(&self) -> Config {
        self.ws_config.clone()
    }

    pub fn ws_config_mut(&mut self) -> &mut Config {
        &mut self.ws_config
    }

    fn send(&self, manager: &Arc<Manager>, text: String) -> Result<()> {
        manager.write(
            &self.ws_config.name,
            Message::TextMessage(self.ws_config.name.clone(), text),
        )?;
        Ok(())
    }

    /// Opening `{"type": "market", ...}` frame. Must be the first frame on a connection (send it
    /// again after every reconnect).
    pub fn subscribe(&self, manager: &Arc<Manager>, sub: &MarketWsSubscription) -> Result<()> {
        self.send(manager, sub.to_json())
    }

    /// `{"operation": "subscribe", "assets_ids": [...]}` on an open connection. The server
    /// answers with a `book` snapshot per added asset.
    pub fn add_assets(&self, manager: &Arc<Manager>, assets_ids: &[&str]) -> Result<()> {
        self.update(
            manager,
            &MarketWsUpdate::new(MarketWsOperation::Subscribe, assets_ids),
        )
    }

    /// `{"operation": "unsubscribe", "assets_ids": [...]}`. No reply is sent.
    pub fn remove_assets(&self, manager: &Arc<Manager>, assets_ids: &[&str]) -> Result<()> {
        self.update(
            manager,
            &MarketWsUpdate::new(MarketWsOperation::Unsubscribe, assets_ids),
        )
    }

    /// Any update frame (e.g. with `level` or `custom_feature_enabled`).
    pub fn update(&self, manager: &Arc<Manager>, update: &MarketWsUpdate) -> Result<()> {
        self.send(manager, update.to_json())
    }
}
