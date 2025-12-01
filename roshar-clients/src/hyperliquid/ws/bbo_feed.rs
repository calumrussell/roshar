use roshar_types::{HyperliquidBboMessage, HyperliquidWssMessage, SupportedMessages, Venue};
use roshar_ws_mgr::Manager;
use std::collections::HashSet;
use tokio::sync::{broadcast, mpsc};

use crate::{HL_TESTNET_WSS_URL, HL_WSS_URL};

/// Handle for adding subscriptions to the BBO feed
#[derive(Clone)]
pub struct BboFeedHandle {
    subscription_tx: mpsc::Sender<String>,
}

impl BboFeedHandle {
    /// Add a ticker subscription dynamically
    pub async fn add_subscription(&self, ticker: String) -> Result<(), String> {
        self.subscription_tx
            .send(ticker)
            .await
            .map_err(|e| format!("Failed to send subscription request: {}", e))
    }
}

/// Manages BBO (Best Bid/Offer) feed for Hyperliquid
/// - Uses shared WebSocket Manager
/// - Subscribes to BBO feed for specified tickers
/// - Parses BBO messages
/// - Sends (ticker, bid, ask) tuples via broadcast channel (supports multiple receivers)
/// - Supports dynamic subscription via BboFeedHandle
pub struct BboFeedHandler {
    tickers: Vec<String>,
    bbo_tx: broadcast::Sender<(String, f64, f64)>,
    subscription_rx: mpsc::Receiver<String>,
    subscription_tx: mpsc::Sender<String>,
    ws_manager: std::sync::Arc<Manager>,
    is_testnet: bool,
    conn_name: String,
}

impl BboFeedHandler {
    pub fn new(
        tickers: Vec<String>,
        bbo_tx: broadcast::Sender<(String, f64, f64)>,
        ws_manager: std::sync::Arc<Manager>,
        is_testnet: bool,
    ) -> Self {
        let (subscription_tx, subscription_rx) = mpsc::channel(100);
        Self {
            tickers,
            bbo_tx,
            subscription_rx,
            subscription_tx,
            ws_manager,
            is_testnet,
            conn_name: "hyperliquid-bbo".to_string(),
        }
    }

    /// Get a handle for adding subscriptions
    pub fn get_handle(&self) -> BboFeedHandle {
        BboFeedHandle {
            subscription_tx: self.subscription_tx.clone(),
        }
    }

    pub async fn run(mut self) {
        // Set up reader before establishing connection
        let mut recv = self.ws_manager.setup_reader(&self.conn_name, 1000);
        log::info!("WebSocket reader set up for BBO feed: {}", self.conn_name);

        // Determine WebSocket URL
        let ws_url = if self.is_testnet {
            HL_TESTNET_WSS_URL
        } else {
            HL_WSS_URL
        };

        // Create WebSocket configuration
        let ws_config = roshar_ws_mgr::Config {
            name: self.conn_name.clone(),
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
        if let Err(e) = self.ws_manager.new_conn(&self.conn_name, ws_config) {
            log::error!("Failed to create BBO WebSocket connection: {}", e);
            return;
        }

        // Track subscribed tickers
        let mut subscribed_tickers: HashSet<String> = HashSet::new();

        // Process messages and subscription requests
        loop {
            tokio::select! {
                msg = recv.recv() => {
                    match msg {
                        Ok(roshar_ws_mgr::Message::SuccessfulHandshake(_name)) => {
                            log::info!("BBO feed WebSocket connected");
                            // Subscribe to initial tickers
                            for ticker in &self.tickers {
                                if subscribed_tickers.insert(ticker.clone()) {
                                    let bbo_sub = HyperliquidWssMessage::bbo(ticker).to_json();
                                    if let Err(e) = self.ws_manager.write(
                                        &self.conn_name,
                                        roshar_ws_mgr::Message::TextMessage(self.conn_name.clone(), bbo_sub),
                                    ) {
                                        log::error!("Failed to subscribe to BBO for {}: {}", ticker, e);
                                        subscribed_tickers.remove(ticker);
                                    } else {
                                        log::info!("Subscribed to BBO feed for {}", ticker);
                                    }
                                }
                            }
                        }
                        Ok(roshar_ws_mgr::Message::TextMessage(_name, content)) => {
                            if let Err(e) = self.handle_message(&content).await {
                                log::error!("Failed to handle BBO message: {}", e);
                            }
                        }
                        Ok(roshar_ws_mgr::Message::ReadError(_name, err)) => {
                            log::error!("Websocket read error in BBO feed: {}", err);
                        }
                        Ok(roshar_ws_mgr::Message::WriteError(_name, err)) => {
                            log::error!("Websocket write error in BBO feed: {}", err);
                        }
                        Ok(roshar_ws_mgr::Message::PongReceiveTimeoutError(_name)) => {
                            log::warn!("Pong receive timeout in BBO feed");
                        }
                        Ok(_) => {
                            // Other message types (ping, pong, close, etc.)
                        }
                        Err(e) => {
                            log::error!("Failed to receive WebSocket message: {}", e);
                            break;
                        }
                    }
                }
                Some(ticker) = self.subscription_rx.recv() => {
                    // Add subscription dynamically
                    if subscribed_tickers.insert(ticker.clone()) {
                        let bbo_sub = HyperliquidWssMessage::bbo(&ticker).to_json();
                        if let Err(e) = self.ws_manager.write(
                            &self.conn_name,
                            roshar_ws_mgr::Message::TextMessage(self.conn_name.clone(), bbo_sub),
                        ) {
                            log::error!("Failed to subscribe to BBO for {}: {}", ticker, e);
                            subscribed_tickers.remove(&ticker);
                        } else {
                            log::info!("Subscribed to BBO feed for {}", ticker);
                        }
                    } else {
                        log::debug!("Already subscribed to BBO for {}", ticker);
                    }
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
            SupportedMessages::HyperliquidBboMessage(bbo_msg) => {
                self.handle_bbo(bbo_msg).await?;
            }
            _ => {
                // Ignore other message types
            }
        }

        Ok(())
    }

    async fn handle_bbo(&self, bbo_msg: HyperliquidBboMessage) -> Result<(), String> {
        let ticker = bbo_msg.data.coin.clone();

        // Extract bid and ask from the BBO array
        let bid = bbo_msg.data.bbo[0]
            .as_ref()
            .and_then(|level| level.px.parse::<f64>().ok())
            .unwrap_or(0.0);

        let ask = bbo_msg.data.bbo[1]
            .as_ref()
            .and_then(|level| level.px.parse::<f64>().ok())
            .unwrap_or(0.0);

        // Send BBO update via broadcast channel (ignore if no receivers)
        let _ = self.bbo_tx.send((ticker.clone(), bid, ask));

        Ok(())
    }
}
