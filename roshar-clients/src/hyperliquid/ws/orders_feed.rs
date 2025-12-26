use roshar_types::{
    HyperliquidOrderUpdatesMessage, HyperliquidWssMessage, SupportedMessages, Venue,
};
use roshar_ws_mgr::Manager;
use serde_json;
use tokio::sync::mpsc;

use crate::hyperliquid::ExchangeMetadataHandle;
use crate::state_manager::{OrderDirection, StateMessage};
use crate::{HL_TESTNET_WSS_URL, HL_WSS_URL};

/// Manages order status tracking for Hyperliquid
/// - Uses shared WebSocket Manager
/// - Subscribes to orderUpdates
/// - Parses orderUpdates messages
/// - Sends OrderPlaced/OrderCancelled messages to StateManager
pub struct OrdersFeedHandler {
    wallet_address: String,
    perp_state_tx: mpsc::Sender<StateMessage>,
    spot_state_tx: mpsc::Sender<StateMessage>,
    ws_manager: std::sync::Arc<Manager>,
    metadata_handle: ExchangeMetadataHandle,
    is_testnet: bool,
}

impl OrdersFeedHandler {
    pub fn new(
        wallet_address: String,
        perp_state_tx: mpsc::Sender<StateMessage>,
        spot_state_tx: mpsc::Sender<StateMessage>,
        ws_manager: std::sync::Arc<Manager>,
        metadata_handle: ExchangeMetadataHandle,
        is_testnet: bool,
    ) -> Self {
        Self {
            wallet_address,
            perp_state_tx,
            spot_state_tx,
            ws_manager,
            metadata_handle,
            is_testnet,
        }
    }

    pub async fn run(self) {
        let conn_name = format!("hyperliquid-orders-{}", self.wallet_address);

        // Set up reader before establishing connection
        let mut recv = self.ws_manager.setup_reader(&conn_name, 1000);
        log::info!("WebSocket reader set up for orderUpdates: {}", conn_name);

        // Determine WebSocket URL
        let ws_url = if self.is_testnet {
            HL_TESTNET_WSS_URL
        } else {
            HL_WSS_URL
        };

        // Create WebSocket configuration
        let ws_config = roshar_ws_mgr::Config {
            name: conn_name.clone(),
            url: ws_url.to_string(),
            ping_duration: 10,
            ping_message: HyperliquidWssMessage::ping().to_json(),
            ping_timeout: 10,
            reconnect_timeout: 90,
            read_buffer_size: Some(33554432),
            write_buffer_size: Some(2097152),
            max_message_size: Some(41943040),
            max_frame_size: Some(20971520),
            tcp_recv_buffer_size: Some(16777216),
            tcp_send_buffer_size: Some(4194304),
            tcp_nodelay: Some(true),
            broadcast_channel_size: Some(131072),
            use_text_ping: Some(true),
        };

        // Establish WebSocket connection
        if let Err(e) = self.ws_manager.new_conn(&conn_name, ws_config) {
            log::error!("Failed to create orderUpdates WebSocket connection: {}", e);
            return;
        }

        // Process messages
        while let Ok(msg) = recv.recv().await {
            match msg {
                roshar_ws_mgr::Message::SuccessfulHandshake(_name) => {
                    // Subscribe to orderUpdates
                    let order_updates_sub =
                        HyperliquidWssMessage::order_updates(&self.wallet_address).to_json();
                    if let Err(e) = self.ws_manager.write(
                        &conn_name,
                        roshar_ws_mgr::Message::TextMessage(conn_name.clone(), order_updates_sub),
                    ) {
                        log::error!("Failed to subscribe to orderUpdates: {}", e);
                        return;
                    }
                    log::info!("Subscribed to orderUpdates for {}", self.wallet_address);
                }
                roshar_ws_mgr::Message::TextMessage(_name, content) => {
                    if let Err(e) = self.handle_message(&content).await {
                        log::error!("Failed to handle order update message: {}", e);
                    }
                }
                roshar_ws_mgr::Message::ReadError(_name, err) => {
                    log::error!("Websocket read error in orders feed: {}", err);
                    // Trigger reconnection
                    if let Err(e) = self.ws_manager.reconnect_with_close(&conn_name, false).await {
                        log::error!("Failed to trigger reconnect after read error: {}", e);
                    }
                }
                roshar_ws_mgr::Message::WriteError(_name, err) => {
                    log::error!("Websocket write error in orders feed: {}", err);
                    // Write errors on already closed connection don't need reconnect trigger
                    // as ReadError or CloseMessage should have already triggered it
                }
                roshar_ws_mgr::Message::CloseMessage(_name, reason) => {
                    if let Some(close_reason) = reason.as_ref() {
                        log::error!("Websocket closed with reason in orders feed: {}", close_reason);
                    } else {
                        log::error!("Websocket closed without reason in orders feed");
                    }
                    // Trigger reconnection
                    if let Err(e) = self.ws_manager.reconnect_with_close(&conn_name, false).await {
                        log::error!("Failed to trigger reconnect after close: {}", e);
                    }
                }
                roshar_ws_mgr::Message::PongReceiveTimeoutError(_name) => {
                    log::warn!("Pong receive timeout in orders feed");
                    // Trigger reconnection - send close message since connection might still be open
                    if let Err(e) = self.ws_manager.reconnect(&conn_name).await {
                        log::error!("Failed to trigger reconnect after pong timeout: {}", e);
                    }
                }
                _ => {
                    // Other message types (ping, pong, etc.)
                }
            }
        }
    }

    async fn handle_message(&self, content: &str) -> Result<(), String> {
        // Parse into SupportedMessages
        let msg = match SupportedMessages::from_message(content, Venue::Hyperliquid) {
            Some(msg) => msg,
            None => {
                // Silently ignore unparseable messages (e.g., subscription responses)
                return Ok(());
            }
        };

        match msg {
            SupportedMessages::HyperliquidOrderUpdatesMessage(order_updates_msg) => {
                self.handle_order_updates(order_updates_msg).await?;
            }
            _ => {
                // Ignore other message types
            }
        }

        Ok(())
    }

    async fn handle_order_updates(
        &self,
        order_updates_msg: HyperliquidOrderUpdatesMessage,
    ) -> Result<(), String> {
        // Process each order in the message
        for ws_order in order_updates_msg.data {
            // Parse price and size from strings
            let px = ws_order
                .order
                .limit_px
                .parse::<f64>()
                .map_err(|e| format!("Failed to parse price: {}", e))?;

            let sz = ws_order
                .order
                .sz
                .parse::<f64>()
                .map_err(|e| format!("Failed to parse size: {}", e))?;

            // Determine direction from side
            let direction = match ws_order.order.side.as_str() {
                "B" => OrderDirection::Buy,
                "A" => OrderDirection::Sell,
                _ => {
                    log::warn!("Unknown order side: {}", ws_order.order.side);
                    continue;
                }
            };

            let order_id = ws_order.order.oid.to_string();
            let ticker = ws_order.order.coin.clone();
            let status = ws_order.status.as_str();

            // Determine if this is a spot or perp order based on ticker format
            // Spot tickers contain "@" (e.g., "@142", "@162")
            let is_spot = ticker.contains('@');

            // Convert spot ticker to base token name
            let state_ticker = if is_spot {
                // Get spot asset info to map ticker to tokens
                let spot_asset_info = match self.metadata_handle.get_spot_asset_info().await {
                    Ok(info) => info,
                    Err(e) => {
                        log::error!("Failed to get spot asset info for ticker {}: {}", ticker, e);
                        continue;
                    }
                };

                // Look up the spot pair to get the tokens
                let spot_info = match spot_asset_info.get(&ticker) {
                    Some(info) => info,
                    None => {
                        log::error!("Spot ticker {} not found in metadata", ticker);
                        continue;
                    }
                };

                // Use the base token name (tokens[0])
                spot_info.base_token.name.clone()
            } else {
                // Perp orders use ticker as-is
                ticker.clone()
            };

            // Only keep orders with status "open" (active orders)
            // Everything else (filled, canceled, rejected, etc.) should be removed from pending
            if status != "open" {
                let msg = StateMessage::OrderCancelled {
                    venue: Venue::Hyperliquid,
                    ticker: state_ticker.clone(),
                    order_id: order_id.clone(),
                };

                if is_spot {
                    if let Err(e) = self.spot_state_tx.send(msg).await {
                        log::error!("Failed to send OrderCancelled to spot StateManager: {}", e);
                    }
                } else if let Err(e) = self.perp_state_tx.send(msg).await {
                    log::error!("Failed to send OrderCancelled to perp StateManager: {}", e);
                }
            } else {
                // Order is active (status = "open")
                // Serialize the raw order to preserve all information
                let raw_order = serde_json::to_value(&ws_order).ok();

                let msg = StateMessage::OrderPlaced {
                    venue: Venue::Hyperliquid,
                    ticker: state_ticker.clone(),
                    order_id: order_id.clone(),
                    px,
                    qty: sz,
                    direction,
                    raw_order: raw_order.clone(),
                };

                if is_spot {
                    if let Err(e) = self.spot_state_tx.send(msg).await {
                        log::error!("Failed to send OrderPlaced to spot StateManager: {}", e);
                    }
                } else if let Err(e) = self.perp_state_tx.send(msg).await {
                    log::error!("Failed to send OrderPlaced to perp StateManager: {}", e);
                }
            }
        }

        Ok(())
    }
}
