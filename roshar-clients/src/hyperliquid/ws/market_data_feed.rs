use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};

use roshar_types::{
    HlOrderBook, HyperliquidBookMessage, HyperliquidTradesMessage, HyperliquidWssMessage,
    OrderBookState, SupportedMessages, Trade, Venue,
};
use roshar_ws_mgr::Manager;
use tokio::sync::mpsc;

use crate::{HL_TESTNET_WSS_URL, HL_WSS_URL};

/// Events emitted by the market data feed
#[derive(Debug, Clone)]
pub enum MarketEvent {
    DepthUpdate {
        coin: String,
        book: Arc<OrderBookState>,
    },
    TradeUpdate {
        coin: String,
        trades: Arc<Vec<Trade>>,
    },
}

/// Commands for dynamic subscription management
#[derive(Debug)]
pub enum SubscriptionCommand {
    AddDepth { coin: String },
    RemoveDepth { coin: String },
    AddTrades { coin: String },
    RemoveTrades { coin: String },
}

/// Handle for sending subscription commands to the market data feed
#[derive(Clone)]
pub struct MarketDataFeedHandle {
    command_tx: mpsc::Sender<SubscriptionCommand>,
}

impl MarketDataFeedHandle {
    pub async fn add_depth(&self, coin: &str) -> Result<(), String> {
        self.command_tx
            .send(SubscriptionCommand::AddDepth {
                coin: coin.to_string(),
            })
            .await
            .map_err(|e| format!("Failed to send add_depth command: {}", e))
    }

    pub async fn remove_depth(&self, coin: &str) -> Result<(), String> {
        self.command_tx
            .send(SubscriptionCommand::RemoveDepth {
                coin: coin.to_string(),
            })
            .await
            .map_err(|e| format!("Failed to send remove_depth command: {}", e))
    }

    pub async fn add_trades(&self, coin: &str) -> Result<(), String> {
        self.command_tx
            .send(SubscriptionCommand::AddTrades {
                coin: coin.to_string(),
            })
            .await
            .map_err(|e| format!("Failed to send add_trades command: {}", e))
    }

    pub async fn remove_trades(&self, coin: &str) -> Result<(), String> {
        self.command_tx
            .send(SubscriptionCommand::RemoveTrades {
                coin: coin.to_string(),
            })
            .await
            .map_err(|e| format!("Failed to send remove_trades command: {}", e))
    }
}

/// Shared state for order books, accessible from HyperliquidClient
#[derive(Clone)]
pub struct MarketDataState {
    order_books: Arc<RwLock<HashMap<String, HlOrderBook>>>,
}

impl MarketDataState {
    pub fn new() -> Self {
        Self {
            order_books: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn get_latest_depth(&self, coin: &str) -> Option<OrderBookState> {
        let books = self.order_books.read().ok()?;
        books.get(coin).and_then(|ob| ob.book.clone())
    }

    pub fn get_order_book_view(&self, coin: &str) -> Option<OrderBookState> {
        self.get_latest_depth(coin)
    }
}

impl Default for MarketDataState {
    fn default() -> Self {
        Self::new()
    }
}

/// Manages market data feeds (depth, trades) for Hyperliquid
/// - Uses shared WebSocket Manager
/// - Subscribes to l2Book and trades feeds
/// - Maintains order book state (for Pattern A - polling)
/// - Sends MarketEvent updates (for Pattern B - reactive)
pub struct MarketDataFeed {
    ws_manager: Arc<Manager>,
    is_testnet: bool,
    conn_name: String,

    // Shared state accessible from HyperliquidClient
    state: MarketDataState,

    // Event channel (for Pattern B - reactive)
    event_tx: mpsc::Sender<MarketEvent>,

    // Dynamic subscription commands
    command_rx: mpsc::Receiver<SubscriptionCommand>,
    command_tx: mpsc::Sender<SubscriptionCommand>,

    // Track what we're subscribed to
    depth_subscriptions: HashSet<String>,
    trades_subscriptions: HashSet<String>,

    // Track connection state
    is_connected: bool,

    // Queue commands received before connection is established
    pending_commands: Vec<SubscriptionCommand>,
}

impl MarketDataFeed {
    pub fn new(
        ws_manager: Arc<Manager>,
        is_testnet: bool,
        event_tx: mpsc::Sender<MarketEvent>,
    ) -> Self {
        let (command_tx, command_rx) = mpsc::channel(100);

        Self {
            ws_manager,
            is_testnet,
            conn_name: "hyperliquid-market-data".to_string(),
            state: MarketDataState::new(),
            event_tx,
            command_rx,
            command_tx,
            depth_subscriptions: HashSet::new(),
            trades_subscriptions: HashSet::new(),
            is_connected: false,
            pending_commands: Vec::new(),
        }
    }

    /// Get a handle for sending subscription commands
    pub fn get_handle(&self) -> MarketDataFeedHandle {
        MarketDataFeedHandle {
            command_tx: self.command_tx.clone(),
        }
    }

    /// Get the shared state for accessing order books
    pub fn get_state(&self) -> MarketDataState {
        self.state.clone()
    }

    pub async fn run(mut self) {
        let mut recv = self.ws_manager.setup_reader(&self.conn_name, 1000);
        log::info!(
            "WebSocket reader set up for market data feed: {}",
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

        if let Err(e) = self.ws_manager.new_conn(&self.conn_name, ws_config) {
            log::error!("Failed to create market data WebSocket connection: {}", e);
            return;
        }

        loop {
            tokio::select! {
                msg = recv.recv() => {
                    match msg {
                        Ok(roshar_ws_mgr::Message::SuccessfulHandshake(_name)) => {
                            log::info!("Market data feed WebSocket connected: {}", self.conn_name);
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
                            self.handle_message(&content).await;
                        }
                        Ok(roshar_ws_mgr::Message::ReadError(_name, err)) => {
                            log::error!("Websocket read error in market data feed: {}", err);
                            self.is_connected = false;
                        }
                        Ok(roshar_ws_mgr::Message::WriteError(_name, err)) => {
                            log::error!("Websocket write error in market data feed: {}", err);
                        }
                        Ok(roshar_ws_mgr::Message::PongReceiveTimeoutError(_name)) => {
                            log::warn!("Pong receive timeout in market data feed");
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
                    if self.is_connected {
                        self.handle_command(cmd);
                    } else {
                        // Queue command until connection is established
                        self.pending_commands.push(cmd);
                    }
                }
            }
        }
    }

    fn resubscribe_all(&self) {
        for coin in &self.depth_subscriptions {
            let sub_msg = HyperliquidWssMessage::l2_book(coin).to_json();
            if let Err(e) = self.ws_manager.write(
                &self.conn_name,
                roshar_ws_mgr::Message::TextMessage(self.conn_name.clone(), sub_msg),
            ) {
                log::error!("Failed to resubscribe to depth for {}: {}", coin, e);
            }
        }

        for coin in &self.trades_subscriptions {
            let sub_msg = HyperliquidWssMessage::trades(coin).to_json();
            if let Err(e) = self.ws_manager.write(
                &self.conn_name,
                roshar_ws_mgr::Message::TextMessage(self.conn_name.clone(), sub_msg),
            ) {
                log::error!("Failed to resubscribe to trades for {}: {}", coin, e);
            }
        }
    }

    fn handle_command(&mut self, cmd: SubscriptionCommand) {
        match cmd {
            SubscriptionCommand::AddDepth { coin } => {
                if self.depth_subscriptions.insert(coin.clone()) {
                    // Add order book to shared state
                    if let Ok(mut books) = self.state.order_books.write() {
                        books.insert(coin.clone(), HlOrderBook::new(coin.clone()));
                    }

                    let sub_msg = HyperliquidWssMessage::l2_book(&coin).to_json();
                    if let Err(e) = self.ws_manager.write(
                        &self.conn_name,
                        roshar_ws_mgr::Message::TextMessage(self.conn_name.clone(), sub_msg),
                    ) {
                        log::error!("Failed to subscribe to depth for {}: {}", coin, e);
                        self.depth_subscriptions.remove(&coin);
                        if let Ok(mut books) = self.state.order_books.write() {
                            books.remove(&coin);
                        }
                    } else {
                        log::info!("Subscribed to depth for {}", coin);
                    }
                }
            }
            SubscriptionCommand::RemoveDepth { coin } => {
                if self.depth_subscriptions.remove(&coin) {
                    // Remove order book from shared state
                    if let Ok(mut books) = self.state.order_books.write() {
                        books.remove(&coin);
                    }

                    let unsub_msg = HyperliquidWssMessage::l2_book_unsub(&coin).to_json();
                    if let Err(e) = self.ws_manager.write(
                        &self.conn_name,
                        roshar_ws_mgr::Message::TextMessage(self.conn_name.clone(), unsub_msg),
                    ) {
                        log::error!("Failed to unsubscribe from depth for {}: {}", coin, e);
                    } else {
                        log::info!("Unsubscribed from depth for {}", coin);
                    }
                }
            }
            SubscriptionCommand::AddTrades { coin } => {
                if self.trades_subscriptions.insert(coin.clone()) {
                    let sub_msg = HyperliquidWssMessage::trades(&coin).to_json();
                    if let Err(e) = self.ws_manager.write(
                        &self.conn_name,
                        roshar_ws_mgr::Message::TextMessage(self.conn_name.clone(), sub_msg),
                    ) {
                        log::error!("Failed to subscribe to trades for {}: {}", coin, e);
                        self.trades_subscriptions.remove(&coin);
                    } else {
                        log::info!("Subscribed to trades for {}", coin);
                    }
                }
            }
            SubscriptionCommand::RemoveTrades { coin } => {
                if self.trades_subscriptions.remove(&coin) {
                    let unsub_msg = HyperliquidWssMessage::trades_unsub(&coin).to_json();
                    if let Err(e) = self.ws_manager.write(
                        &self.conn_name,
                        roshar_ws_mgr::Message::TextMessage(self.conn_name.clone(), unsub_msg),
                    ) {
                        log::error!("Failed to unsubscribe from trades for {}: {}", coin, e);
                    } else {
                        log::info!("Unsubscribed from trades for {}", coin);
                    }
                }
            }
        }
    }

    async fn handle_message(&mut self, content: &str) {
        let msg = match SupportedMessages::from_message(content, Venue::Hyperliquid) {
            Some(msg) => msg,
            None => return,
        };

        match msg {
            SupportedMessages::HyperliquidBookMessage(book_msg) => {
                self.handle_depth(book_msg).await;
            }
            SupportedMessages::HyperliquidTradesMessage(trades_msg) => {
                self.handle_trades(trades_msg).await;
            }
            _ => {}
        }
    }

    async fn handle_depth(&mut self, msg: HyperliquidBookMessage) {
        let coin = msg.data.coin.clone();

        let book_state = {
            let Ok(mut books) = self.state.order_books.write() else {
                return;
            };
            if let Some(order_book) = books.get_mut(&coin) {
                if let Err(e) = order_book.new_message(&msg) {
                    log::error!("Failed to update order book for {}: {:?}", coin, e);
                    return;
                }
                order_book.book.clone()
            } else {
                return;
            }
        };

        if let Some(book) = book_state {
            let _ = self
                .event_tx
                .send(MarketEvent::DepthUpdate {
                    coin,
                    book: Arc::new(book),
                })
                .await;
        }
    }

    async fn handle_trades(&self, msg: HyperliquidTradesMessage) {
        if msg.data.is_empty() {
            return;
        }

        let coin = msg.data[0].coin.clone();
        let trades = msg.to_trades();

        if !trades.is_empty() {
            let _ = self
                .event_tx
                .send(MarketEvent::TradeUpdate {
                    coin,
                    trades: Arc::new(trades),
                })
                .await;
        }
    }
}
