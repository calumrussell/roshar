use roshar_types::{HyperliquidBboMessage, HyperliquidWssMessage, SupportedMessages, Venue};
use roshar_ws_mgr::Manager;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};

use crate::{HL_TESTNET_WSS_URL, HL_WSS_URL};

/// Commands for BBO feed management
enum BboCommand {
    Add { ticker: String },
    Remove { ticker: String },
    GetBbo {
        ticker: String,
        response: oneshot::Sender<Option<(f64, f64)>>,
    },
}

/// Handle for interacting with the BBO feed
#[derive(Clone)]
pub(crate) struct BboFeedHandle {
    command_tx: mpsc::Sender<BboCommand>,
}

impl BboFeedHandle {
    /// Add a ticker subscription dynamically
    pub async fn add_subscription(&self, ticker: &str) -> Result<(), String> {
        self.command_tx
            .send(BboCommand::Add {
                ticker: ticker.to_string(),
            })
            .await
            .map_err(|e| format!("Failed to send subscription request: {}", e))
    }

    /// Remove a ticker subscription dynamically
    pub async fn remove_subscription(&self, ticker: &str) -> Result<(), String> {
        self.command_tx
            .send(BboCommand::Remove {
                ticker: ticker.to_string(),
            })
            .await
            .map_err(|e| format!("Failed to send unsubscription request: {}", e))
    }

    /// Get the latest BBO for a ticker
    /// Returns None if not subscribed or no data received yet
    pub async fn get_latest_bbo(&self, ticker: &str) -> Result<Option<(f64, f64)>, String> {
        let (response_tx, response_rx) = oneshot::channel();
        self.command_tx
            .send(BboCommand::GetBbo {
                ticker: ticker.to_string(),
                response: response_tx,
            })
            .await
            .map_err(|e| format!("Failed to send get BBO request: {}", e))?;

        response_rx
            .await
            .map_err(|e| format!("Failed to receive BBO response: {}", e))
    }
}

/// Manages BBO (Best Bid/Offer) feed for Hyperliquid
/// - Uses shared WebSocket Manager
/// - Subscribes to BBO feed for specified tickers
/// - Parses BBO messages
/// - Maintains state for polling access via BboFeedHandle
/// - Supports dynamic subscription management
pub(crate) struct BboFeed {
    ws_manager: Arc<Manager>,
    is_testnet: bool,
    conn_name: String,

    // BBO data storage: ticker -> (bid, ask)
    bbo_data: HashMap<String, (f64, f64)>,

    // Command channel
    command_rx: mpsc::Receiver<BboCommand>,
    command_tx: mpsc::Sender<BboCommand>,

    // Track what we're subscribed to
    subscriptions: HashSet<String>,

    // Track connection state
    is_connected: bool,

    // Queue commands received before connection is established
    pending_commands: Vec<BboCommand>,
}

impl BboFeed {
    pub fn new(ws_manager: Arc<Manager>, is_testnet: bool) -> Self {
        let (command_tx, command_rx) = mpsc::channel(100);

        Self {
            ws_manager,
            is_testnet,
            conn_name: "hyperliquid-bbo".to_string(),
            bbo_data: HashMap::new(),
            command_rx,
            command_tx,
            subscriptions: HashSet::new(),
            is_connected: false,
            pending_commands: Vec::new(),
        }
    }

    /// Get a handle for interacting with the BBO feed
    pub fn get_handle(&self) -> BboFeedHandle {
        BboFeedHandle {
            command_tx: self.command_tx.clone(),
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

        // Process messages and commands
        loop {
            tokio::select! {
                msg = recv.recv() => {
                    match msg {
                        Ok(roshar_ws_mgr::Message::SuccessfulHandshake(_name)) => {
                            log::info!("BBO feed WebSocket connected: {}", self.conn_name);
                            self.is_connected = true;

                            // Process any pending commands that arrived before connection
                            let pending = std::mem::take(&mut self.pending_commands);
                            for cmd in pending {
                                self.handle_command(cmd);
                            }

                            // Resubscribe all existing subscriptions (for reconnects)
                            self.resubscribe_all();
                        }
                        Ok(roshar_ws_mgr::Message::TextMessage(_name, content)) => {
                            self.handle_message(&content);
                        }
                        Ok(roshar_ws_mgr::Message::ReadError(_name, err)) => {
                            log::error!("Websocket read error in BBO feed: {}", err);
                            self.is_connected = false;
                            // Trigger reconnection
                            if let Err(e) = self.ws_manager.reconnect_with_close(&self.conn_name, false).await {
                                log::error!("Failed to trigger reconnect after read error: {}", e);
                            }
                        }
                        Ok(roshar_ws_mgr::Message::WriteError(_name, err)) => {
                            log::error!("Websocket write error in BBO feed: {}", err);
                            // Write errors on already closed connection don't need reconnect trigger
                            // as ReadError or CloseMessage should have already triggered it
                        }
                        Ok(roshar_ws_mgr::Message::CloseMessage(_name, reason)) => {
                            if let Some(close_reason) = reason.as_ref() {
                                log::error!("Hyperliquid BBO websocket closed with reason: {}", close_reason);
                            } else {
                                log::error!("Hyperliquid BBO websocket closed without reason");
                            }
                            self.is_connected = false;
                            // Trigger reconnection
                            if let Err(e) = self.ws_manager.reconnect_with_close(&self.conn_name, false).await {
                                log::error!("Failed to trigger reconnect after close: {}", e);
                            }
                        }
                        Ok(roshar_ws_mgr::Message::PongReceiveTimeoutError(_name)) => {
                            log::warn!("Pong receive timeout in BBO feed");
                            self.is_connected = false;
                            // Trigger reconnection - send close message since connection might still be open
                            if let Err(e) = self.ws_manager.reconnect(&self.conn_name).await {
                                log::error!("Failed to trigger reconnect after pong timeout: {}", e);
                            }
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
                Some(cmd) = self.command_rx.recv() => {
                    // GetBbo commands can always be handled immediately
                    // Add/Remove commands need connection for subscription
                    match &cmd {
                        BboCommand::GetBbo { .. } => {
                            self.handle_command(cmd);
                        }
                        _ => {
                            if self.is_connected {
                                self.handle_command(cmd);
                            } else {
                                self.pending_commands.push(cmd);
                            }
                        }
                    }
                }
            }
        }
    }

    fn resubscribe_all(&self) {
        for ticker in &self.subscriptions {
            let sub_msg = HyperliquidWssMessage::bbo(ticker).to_json();
            if let Err(e) = self.ws_manager.write(
                &self.conn_name,
                roshar_ws_mgr::Message::TextMessage(self.conn_name.clone(), sub_msg),
            ) {
                log::error!("Failed to resubscribe to BBO for {}: {}", ticker, e);
            }
        }
    }

    fn handle_command(&mut self, cmd: BboCommand) {
        match cmd {
            BboCommand::Add { ticker } => {
                if self.subscriptions.insert(ticker.clone()) {
                    let sub_msg = HyperliquidWssMessage::bbo(&ticker).to_json();
                    if let Err(e) = self.ws_manager.write(
                        &self.conn_name,
                        roshar_ws_mgr::Message::TextMessage(self.conn_name.clone(), sub_msg),
                    ) {
                        log::error!("Failed to subscribe to BBO for {}: {}", ticker, e);
                        self.subscriptions.remove(&ticker);
                    } else {
                        log::info!("Subscribed to BBO feed for {}", ticker);
                    }
                }
            }
            BboCommand::Remove { ticker } => {
                if self.subscriptions.remove(&ticker) {
                    // Remove from state
                    self.bbo_data.remove(&ticker);

                    // Note: Hyperliquid may not have an explicit unsubscribe for BBO
                    // The subscription will be dropped on reconnect if not in the set
                    log::info!("Removed BBO subscription for {}", ticker);
                }
            }
            BboCommand::GetBbo { ticker, response } => {
                let result = self.bbo_data.get(&ticker).copied();
                let _ = response.send(result);
            }
        }
    }

    fn handle_message(&mut self, content: &str) {
        // Parse into SupportedMessages
        let msg = match SupportedMessages::from_message(content, Venue::Hyperliquid) {
            Some(msg) => msg,
            None => {
                // Silently ignore unparseable messages (e.g., subscription responses)
                return;
            }
        };

        if let SupportedMessages::HyperliquidBboMessage(bbo_msg) = msg {
            self.handle_bbo(bbo_msg);
        }
    }

    fn handle_bbo(&mut self, bbo_msg: HyperliquidBboMessage) {
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

        // Update state
        self.bbo_data.insert(ticker, (bid, ask));
    }
}
