use roshar_types::{Candle, HyperliquidCandleMessage, HyperliquidWssMessage};
use roshar_ws_mgr::Manager;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};

use crate::{HL_TESTNET_WSS_URL, HL_WSS_URL};

/// Commands for Candle feed management
enum CandleCommand {
    Add { coin: String },
    Remove { coin: String },
    GetCandle {
        coin: String,
        response: oneshot::Sender<Option<Candle>>,
    },
}

/// Handle for interacting with the Candle feed
#[derive(Clone)]
pub(crate) struct CandleFeedHandle {
    command_tx: mpsc::Sender<CandleCommand>,
}

impl CandleFeedHandle {
    /// Add a coin subscription dynamically
    pub async fn add_subscription(&self, coin: &str) -> Result<(), String> {
        self.command_tx
            .send(CandleCommand::Add {
                coin: coin.to_string(),
            })
            .await
            .map_err(|e| format!("Failed to send subscription request: {}", e))
    }

    /// Remove a coin subscription dynamically
    pub async fn remove_subscription(&self, coin: &str) -> Result<(), String> {
        self.command_tx
            .send(CandleCommand::Remove {
                coin: coin.to_string(),
            })
            .await
            .map_err(|e| format!("Failed to send unsubscription request: {}", e))
    }

    /// Get the latest candle for a coin
    /// Returns None if not subscribed or no data received yet
    pub async fn get_latest_candle(&self, coin: &str) -> Result<Option<Candle>, String> {
        let (response_tx, response_rx) = oneshot::channel();
        self.command_tx
            .send(CandleCommand::GetCandle {
                coin: coin.to_string(),
                response: response_tx,
            })
            .await
            .map_err(|e| format!("Failed to send get candle request: {}", e))?;

        response_rx
            .await
            .map_err(|e| format!("Failed to receive candle response: {}", e))
    }
}

/// Manages Candle feed for Hyperliquid
pub(crate) struct CandleFeed {
    ws_manager: Arc<Manager>,
    is_testnet: bool,
    conn_name: String,

    // Candle data storage: coin -> latest candle
    candle_data: HashMap<String, Candle>,

    // Command channel
    command_rx: mpsc::Receiver<CandleCommand>,
    command_tx: mpsc::Sender<CandleCommand>,

    // Track what we're subscribed to
    subscriptions: HashSet<String>,

    // Track connection state
    is_connected: bool,

    // Queue commands received before connection is established
    pending_commands: Vec<CandleCommand>,
}

impl CandleFeed {
    pub fn new(ws_manager: Arc<Manager>, is_testnet: bool) -> Self {
        let (command_tx, command_rx) = mpsc::channel(100);

        Self {
            ws_manager,
            is_testnet,
            conn_name: "hyperliquid-candle".to_string(),
            candle_data: HashMap::new(),
            command_rx,
            command_tx,
            subscriptions: HashSet::new(),
            is_connected: false,
            pending_commands: Vec::new(),
        }
    }

    /// Get a handle for interacting with the Candle feed
    pub fn get_handle(&self) -> CandleFeedHandle {
        CandleFeedHandle {
            command_tx: self.command_tx.clone(),
        }
    }

    pub async fn run(mut self) {
        let mut recv = self.ws_manager.setup_reader(&self.conn_name, 1000);
        log::info!(
            "WebSocket reader set up for Candle feed: {}",
            self.conn_name
        );

        let ws_url = if self.is_testnet {
            HL_TESTNET_WSS_URL
        } else {
            HL_WSS_URL
        };

        let ws_config = roshar_ws_mgr::Config {
            name: self.conn_name.clone(),
            url: ws_url.to_string(),
            ping_duration: 10,
            ping_message: HyperliquidWssMessage::ping().to_json(),
            ping_timeout: 10,
            reconnect_timeout: 90,
            read_buffer_size: Some(16 * 1024 * 1024),
            write_buffer_size: Some(1024 * 1024),
            max_message_size: Some(20 * 1024 * 1024),
            max_frame_size: Some(10 * 1024 * 1024),
            tcp_recv_buffer_size: Some(8 * 1024 * 1024),
            tcp_send_buffer_size: Some(2 * 1024 * 1024),
            tcp_nodelay: Some(true),
            broadcast_channel_size: Some(16384),
            use_text_ping: Some(true),
        };

        if let Err(e) = self.ws_manager.new_conn(&self.conn_name, ws_config) {
            log::error!("Failed to create Candle WebSocket connection: {}", e);
            return;
        }

        loop {
            tokio::select! {
                msg = recv.recv() => {
                    match msg {
                        Ok(roshar_ws_mgr::Message::SuccessfulHandshake(_name)) => {
                            log::info!("Candle feed WebSocket connected: {}", self.conn_name);
                            self.is_connected = true;

                            let pending = std::mem::take(&mut self.pending_commands);
                            for cmd in pending {
                                self.handle_command(cmd);
                            }

                            self.resubscribe_all();
                        }
                        Ok(roshar_ws_mgr::Message::TextMessage(_name, content)) => {
                            self.handle_message(&content);
                        }
                        Ok(roshar_ws_mgr::Message::ReadError(_name, err)) => {
                            log::error!("Websocket read error in Candle feed: {}", err);
                            self.is_connected = false;
                        }
                        Ok(roshar_ws_mgr::Message::WriteError(_name, err)) => {
                            log::error!("Websocket write error in Candle feed: {}", err);
                        }
                        Ok(roshar_ws_mgr::Message::PongReceiveTimeoutError(_name)) => {
                            log::warn!("Pong receive timeout in Candle feed");
                            self.is_connected = false;
                        }
                        Ok(_) => {}
                        Err(e) => {
                            log::error!("Failed to receive WebSocket message: {}", e);
                            break;
                        }
                    }
                }
                Some(cmd) = self.command_rx.recv() => {
                    match &cmd {
                        CandleCommand::GetCandle { .. } => {
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
        for coin in &self.subscriptions {
            let sub_msg = HyperliquidWssMessage::candle(coin).to_json();
            if let Err(e) = self.ws_manager.write(
                &self.conn_name,
                roshar_ws_mgr::Message::TextMessage(self.conn_name.clone(), sub_msg),
            ) {
                log::error!("Failed to resubscribe to candle for {}: {}", coin, e);
            }
        }
    }

    fn handle_command(&mut self, cmd: CandleCommand) {
        match cmd {
            CandleCommand::Add { coin } => {
                if self.subscriptions.insert(coin.clone()) {
                    let sub_msg = HyperliquidWssMessage::candle(&coin).to_json();
                    if let Err(e) = self.ws_manager.write(
                        &self.conn_name,
                        roshar_ws_mgr::Message::TextMessage(self.conn_name.clone(), sub_msg),
                    ) {
                        log::error!("Failed to subscribe to candle for {}: {}", coin, e);
                        self.subscriptions.remove(&coin);
                    } else {
                        log::info!("Subscribed to Candle feed for {}", coin);
                    }
                }
            }
            CandleCommand::Remove { coin } => {
                if self.subscriptions.remove(&coin) {
                    self.candle_data.remove(&coin);

                    let unsub_msg = HyperliquidWssMessage::candle_unsub(&coin).to_json();
                    if let Err(e) = self.ws_manager.write(
                        &self.conn_name,
                        roshar_ws_mgr::Message::TextMessage(self.conn_name.clone(), unsub_msg),
                    ) {
                        log::error!("Failed to unsubscribe from candle for {}: {}", coin, e);
                    } else {
                        log::info!("Removed Candle subscription for {}", coin);
                    }
                }
            }
            CandleCommand::GetCandle { coin, response } => {
                let result = self.candle_data.get(&coin).cloned();
                let _ = response.send(result);
            }
        }
    }

    fn handle_message(&mut self, content: &str) {
        if let Ok(candle_msg) = serde_json::from_str::<HyperliquidCandleMessage>(content) {
            let candle = candle_msg.to_candle();
            let coin = candle.coin.clone();
            self.candle_data.insert(coin, candle);
        }
    }
}
