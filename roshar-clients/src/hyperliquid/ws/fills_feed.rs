use roshar_types::{HyperliquidUserFillsMessage, HyperliquidWssMessage, SupportedMessages, Venue};
use roshar_ws_mgr::Manager;
use tokio::sync::mpsc;

use crate::hyperliquid::ExchangeMetadataHandle;
use crate::state_manager::{OrderDirection, StateMessage};
use crate::{HL_TESTNET_WSS_URL, HL_WSS_URL};

/// Manages order fill tracking for Hyperliquid
/// - Uses shared WebSocket Manager
/// - Subscribes to userFills
/// - Parses userFills messages
/// - Sends OrderFilled messages to StateManager
pub struct FillsFeedHandler {
    wallet_address: String,
    perp_state_tx: mpsc::Sender<StateMessage>,
    spot_state_tx: mpsc::Sender<StateMessage>,
    ws_manager: std::sync::Arc<Manager>,
    metadata_handle: ExchangeMetadataHandle,
    is_testnet: bool,
}

impl FillsFeedHandler {
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
        let conn_name = format!("hyperliquid-fills-{}", self.wallet_address);

        // Set up reader before establishing connection
        let mut recv = self.ws_manager.setup_reader(&conn_name, 1000);
        log::info!("WebSocket reader set up for userFills: {}", conn_name);

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
            log::error!("Failed to create userFills WebSocket connection: {}", e);
            return;
        }

        // Process messages
        while let Ok(msg) = recv.recv().await {
            match msg {
                roshar_ws_mgr::Message::SuccessfulHandshake(_name) => {
                    // Subscribe to userFills
                    let user_fills_sub =
                        HyperliquidWssMessage::user_fills(&self.wallet_address).to_json();
                    if let Err(e) = self.ws_manager.write(
                        &conn_name,
                        roshar_ws_mgr::Message::TextMessage(conn_name.clone(), user_fills_sub),
                    ) {
                        log::error!("Failed to subscribe to userFills: {}", e);
                        return;
                    }
                    log::info!("Subscribed to userFills for {}", self.wallet_address);
                }
                roshar_ws_mgr::Message::TextMessage(_name, content) => {
                    if let Err(e) = self.handle_message(&content).await {
                        log::error!("Failed to handle fills message: {}", e);
                    }
                }
                roshar_ws_mgr::Message::ReadError(_name, err) => {
                    log::error!("Websocket read error in fills feed: {}", err);
                    // Trigger reconnection
                    if let Err(e) = self.ws_manager.reconnect_with_close(&conn_name, false).await {
                        log::error!("Failed to trigger reconnect after read error: {}", e);
                    }
                }
                roshar_ws_mgr::Message::WriteError(_name, err) => {
                    log::error!("Websocket write error in fills feed: {}", err);
                    // Write errors on already closed connection don't need reconnect trigger
                    // as ReadError or CloseMessage should have already triggered it
                }
                roshar_ws_mgr::Message::CloseMessage(_name, reason) => {
                    if let Some(close_reason) = reason.as_ref() {
                        log::error!("Websocket closed with reason in fills feed: {}", close_reason);
                    } else {
                        log::error!("Websocket closed without reason in fills feed");
                    }
                    // Trigger reconnection
                    if let Err(e) = self.ws_manager.reconnect_with_close(&conn_name, false).await {
                        log::error!("Failed to trigger reconnect after close: {}", e);
                    }
                }
                roshar_ws_mgr::Message::PongReceiveTimeoutError(_name) => {
                    log::warn!("Pong receive timeout in fills feed");
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
            SupportedMessages::HyperliquidUserFillsMessage(fills_msg) => {
                self.handle_fills(fills_msg).await?;
            }
            _ => {
                // Ignore other message types
            }
        }

        Ok(())
    }

    async fn handle_fills(&self, fills_msg: HyperliquidUserFillsMessage) -> Result<(), String> {
        // Skip snapshot messages (historical fills)
        if fills_msg.data.is_snapshot == Some(true) {
            log::debug!(
                "Skipping snapshot message with {} historical fills",
                fills_msg.data.fills.len()
            );
            return Ok(());
        }

        // Process each fill in the message
        for fill in fills_msg.data.fills {
            // Parse price and size from strings
            let filled_px = fill
                .px
                .parse::<f64>()
                .map_err(|e| format!("Failed to parse price: {}", e))?;

            let filled_qty = fill
                .sz
                .parse::<f64>()
                .map_err(|e| format!("Failed to parse size: {}", e))?;

            // Convert oid to string
            let order_id = fill.oid.to_string();

            // Use coin as ticker
            let ticker = fill.coin.clone();

            // Determine direction from side field
            // "B" = Buy, "A" = Ask (Sell)
            let direction = match fill.side.as_str() {
                "B" => OrderDirection::Buy,
                "A" => OrderDirection::Sell,
                _ => {
                    log::warn!("Unknown fill side: {}", fill.side);
                    continue;
                }
            };

            // Determine if this is a spot or perp fill based on ticker format
            // Spot tickers contain "@" (e.g., "@142", "@162")
            let is_spot = ticker.contains('@');

            if is_spot {
                // For spot fills, we need to convert the ticker (@162) to the base token name (UFART)
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

                // Get the base and quote token names
                let base_token_name = &spot_info.base_token.name;
                let quote_token_name = &spot_info.quote_token.name;

                // Send fill for base token (e.g., UETH)
                let base_msg = StateMessage::OrderFilled {
                    venue: Venue::Hyperliquid,
                    ticker: base_token_name.clone(),
                    order_id: order_id.clone(),
                    filled_qty,
                    filled_px,
                    direction,
                };

                if let Err(e) = self.spot_state_tx.send(base_msg).await {
                    log::error!(
                        "Failed to send OrderFilled for base token to spot StateManager: {}",
                        e
                    );
                }

                // Send opposite fill for quote token (e.g., USDC)
                // If we BUY base, we SELL quote (spend USDC)
                // If we SELL base, we BUY quote (receive USDC)
                let quote_direction = match direction {
                    OrderDirection::Buy => OrderDirection::Sell,
                    OrderDirection::Sell => OrderDirection::Buy,
                };

                let quote_qty = filled_qty * filled_px;

                let quote_msg = StateMessage::OrderFilled {
                    venue: Venue::Hyperliquid,
                    ticker: quote_token_name.clone(),
                    order_id: order_id.clone(),
                    filled_qty: quote_qty,
                    filled_px: 1.0, // Quote token is priced at 1.0 (USDC/USDC)
                    direction: quote_direction,
                };

                if let Err(e) = self.spot_state_tx.send(quote_msg).await {
                    log::error!(
                        "Failed to send OrderFilled for quote token to spot StateManager: {}",
                        e
                    );
                }
            } else {
                // Perp fills use ticker as-is
                let msg = StateMessage::OrderFilled {
                    venue: Venue::Hyperliquid,
                    ticker: ticker.clone(),
                    order_id: order_id.clone(),
                    filled_qty,
                    filled_px,
                    direction,
                };

                if let Err(e) = self.perp_state_tx.send(msg).await {
                    log::error!("Failed to send OrderFilled to perp StateManager: {}", e);
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state_manager::StateManager;
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_spot_buy_updates_both_tokens() {
        // Setup: Start with 100 USDC and 0 UETH
        let (state_handle, _manager_handle) = StateManager::spawn_with_init(
            None::<fn() -> std::future::Ready<Result<HashMap<String, f64>, String>>>,
        );

        // Initialize positions
        let mut initial_positions = HashMap::new();
        initial_positions.insert("USDC".to_string(), 100.0);
        initial_positions.insert("UETH".to_string(), 0.0);
        state_handle
            .initialize_positions(initial_positions)
            .await
            .unwrap();

        // Simulate BUY 0.039 UETH at price 2718
        // This should:
        // - Increase UETH by +0.039
        // - Decrease USDC by -(0.039 * 2718) = -105.702

        let base_msg = StateMessage::OrderFilled {
            venue: Venue::Hyperliquid,
            ticker: "UETH".to_string(),
            order_id: "123".to_string(),
            filled_qty: 0.039,
            filled_px: 2718.0,
            direction: OrderDirection::Buy,
        };

        let quote_msg = StateMessage::OrderFilled {
            venue: Venue::Hyperliquid,
            ticker: "USDC".to_string(),
            order_id: "123".to_string(),
            filled_qty: 0.039 * 2718.0,
            filled_px: 1.0,
            direction: OrderDirection::Sell, // Selling USDC (spending it)
        };

        state_handle.sender().send(base_msg).await.unwrap();
        state_handle.sender().send(quote_msg).await.unwrap();

        // Wait a bit for messages to be processed
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Check final positions
        let positions = state_handle.get_positions().await.unwrap();

        let ueth_balance = positions.get("UETH").copied().unwrap_or(0.0);
        let usdc_balance = positions.get("USDC").copied().unwrap_or(0.0);

        assert!(
            (ueth_balance - 0.039).abs() < 0.0001,
            "UETH should be 0.039, got {}",
            ueth_balance
        );
        assert!(
            (usdc_balance - (100.0 - 105.702)).abs() < 1.0,
            "USDC should be approximately -5.702, got {}",
            usdc_balance
        );
    }

    #[tokio::test]
    async fn test_spot_sell_updates_both_tokens() {
        // Setup: Start with 0 USDC and 0.039 UETH
        let (state_handle, _manager_handle) = StateManager::spawn_with_init(
            None::<fn() -> std::future::Ready<Result<HashMap<String, f64>, String>>>,
        );

        let mut initial_positions = HashMap::new();
        initial_positions.insert("USDC".to_string(), 0.0);
        initial_positions.insert("UETH".to_string(), 0.039);
        state_handle
            .initialize_positions(initial_positions)
            .await
            .unwrap();

        // Simulate SELL 0.039 UETH at price 2718
        // This should:
        // - Decrease UETH by -0.039
        // - Increase USDC by +(0.039 * 2718) = +105.702

        let base_msg = StateMessage::OrderFilled {
            venue: Venue::Hyperliquid,
            ticker: "UETH".to_string(),
            order_id: "456".to_string(),
            filled_qty: 0.039,
            filled_px: 2718.0,
            direction: OrderDirection::Sell,
        };

        let quote_msg = StateMessage::OrderFilled {
            venue: Venue::Hyperliquid,
            ticker: "USDC".to_string(),
            order_id: "456".to_string(),
            filled_qty: 0.039 * 2718.0,
            filled_px: 1.0,
            direction: OrderDirection::Buy, // Buying USDC (receiving it)
        };

        state_handle.sender().send(base_msg).await.unwrap();
        state_handle.sender().send(quote_msg).await.unwrap();

        // Wait a bit for messages to be processed
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Check final positions
        let positions = state_handle.get_positions().await.unwrap();

        let ueth_balance = positions.get("UETH").copied().unwrap_or(0.0);
        let usdc_balance = positions.get("USDC").copied().unwrap_or(0.0);

        assert!(
            ueth_balance.abs() < 0.0001,
            "UETH should be 0, got {}",
            ueth_balance
        );
        assert!(
            (usdc_balance - 105.702).abs() < 1.0,
            "USDC should be approximately 105.702, got {}",
            usdc_balance
        );
    }
}
