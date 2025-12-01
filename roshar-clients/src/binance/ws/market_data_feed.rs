use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};

use roshar_types::{
    BinanceDepthDiffMessage, BinanceOrderBook, BinanceTradeMessage, OrderBookState,
    SupportedMessages, Trade, Venue,
};
use roshar_ws_mgr::Manager;
use tokio::sync::{mpsc, Semaphore};

use crate::BINANCE_WSS_URL;

#[derive(Debug, Clone)]
pub enum MarketEvent {
    DepthUpdate {
        symbol: String,
        book: Arc<OrderBookState>,
    },
    TradeUpdate {
        symbol: String,
        trades: Arc<Vec<Trade>>,
    },
}

#[derive(Debug)]
pub enum SubscriptionCommand {
    AddDepth { symbol: String },
    RemoveDepth { symbol: String },
    AddTrades { symbol: String },
    RemoveTrades { symbol: String },
}

#[derive(Clone)]
pub struct MarketDataFeedHandle {
    command_tx: mpsc::Sender<SubscriptionCommand>,
}

impl MarketDataFeedHandle {
    pub async fn add_depth(&self, symbol: &str) -> Result<(), String> {
        self.command_tx
            .send(SubscriptionCommand::AddDepth {
                symbol: symbol.to_string(),
            })
            .await
            .map_err(|e| format!("Failed to send add_depth command: {}", e))
    }

    pub async fn remove_depth(&self, symbol: &str) -> Result<(), String> {
        self.command_tx
            .send(SubscriptionCommand::RemoveDepth {
                symbol: symbol.to_string(),
            })
            .await
            .map_err(|e| format!("Failed to send remove_depth command: {}", e))
    }

    pub async fn add_trades(&self, symbol: &str) -> Result<(), String> {
        self.command_tx
            .send(SubscriptionCommand::AddTrades {
                symbol: symbol.to_string(),
            })
            .await
            .map_err(|e| format!("Failed to send add_trades command: {}", e))
    }

    pub async fn remove_trades(&self, symbol: &str) -> Result<(), String> {
        self.command_tx
            .send(SubscriptionCommand::RemoveTrades {
                symbol: symbol.to_string(),
            })
            .await
            .map_err(|e| format!("Failed to send remove_trades command: {}", e))
    }
}

#[derive(Clone)]
pub struct MarketDataState {
    order_books: Arc<RwLock<HashMap<String, OrderBookState>>>,
}

impl MarketDataState {
    pub fn new() -> Self {
        Self {
            order_books: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn get_latest_depth(&self, symbol: &str) -> Option<OrderBookState> {
        let books = self.order_books.read().ok()?;
        books.get(symbol).cloned()
    }

    fn update_book(&self, symbol: &str, book: OrderBookState) {
        if let Ok(mut books) = self.order_books.write() {
            books.insert(symbol.to_string(), book);
        }
    }

    fn remove_book(&self, symbol: &str) {
        if let Ok(mut books) = self.order_books.write() {
            books.remove(symbol);
        }
    }
}

impl Default for MarketDataState {
    fn default() -> Self {
        Self::new()
    }
}

pub struct MarketDataFeed {
    ws_manager: Arc<Manager>,
    conn_name: String,

    state: MarketDataState,
    order_books: HashMap<String, BinanceOrderBook>,
    event_tx: mpsc::Sender<MarketEvent>,

    command_rx: mpsc::Receiver<SubscriptionCommand>,
    command_tx: mpsc::Sender<SubscriptionCommand>,

    depth_subscriptions: HashSet<String>,
    trades_subscriptions: HashSet<String>,

    is_connected: bool,
    pending_commands: Vec<SubscriptionCommand>,

    snapshot_semaphore: Arc<Semaphore>,
}

impl MarketDataFeed {
    pub fn new(ws_manager: Arc<Manager>, event_tx: mpsc::Sender<MarketEvent>) -> Self {
        let (command_tx, command_rx) = mpsc::channel(100);

        Self {
            ws_manager,
            conn_name: "binance-market-data".to_string(),
            state: MarketDataState::new(),
            order_books: HashMap::new(),
            event_tx,
            command_rx,
            command_tx,
            depth_subscriptions: HashSet::new(),
            trades_subscriptions: HashSet::new(),
            is_connected: false,
            pending_commands: Vec::new(),
            snapshot_semaphore: Arc::new(Semaphore::new(2)),
        }
    }

    pub fn get_handle(&self) -> MarketDataFeedHandle {
        MarketDataFeedHandle {
            command_tx: self.command_tx.clone(),
        }
    }

    pub fn get_state(&self) -> MarketDataState {
        self.state.clone()
    }

    pub async fn run(mut self) {
        let mut recv = self.ws_manager.setup_reader(&self.conn_name, 1000);
        log::info!(
            "WebSocket reader set up for Binance market data feed: {}",
            self.conn_name
        );

        let ws_config = roshar_ws_mgr::Config {
            name: self.conn_name.clone(),
            url: BINANCE_WSS_URL.to_string(),
            ping_duration: 10,
            ping_message: "ping".to_string(),
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
            use_text_ping: Some(false),
        };

        if let Err(e) = self.ws_manager.new_conn(&self.conn_name, ws_config) {
            log::error!(
                "Failed to create Binance market data WebSocket connection: {}",
                e
            );
            return;
        }

        loop {
            tokio::select! {
                msg = recv.recv() => {
                    match msg {
                        Ok(roshar_ws_mgr::Message::SuccessfulHandshake(_name)) => {
                            log::info!("Binance market data feed WebSocket connected: {}", self.conn_name);
                            self.is_connected = true;

                            let pending = std::mem::take(&mut self.pending_commands);
                            for cmd in pending {
                                self.handle_command(cmd).await;
                            }

                            self.resubscribe_all();
                        }
                        Ok(roshar_ws_mgr::Message::TextMessage(_name, content)) => {
                            self.handle_message(&content).await;
                        }
                        Ok(roshar_ws_mgr::Message::ReadError(_name, err)) => {
                            log::error!("Websocket read error in Binance market data feed: {}", err);
                            self.is_connected = false;
                            for book in self.order_books.values_mut() {
                                book.reset();
                            }
                        }
                        Ok(roshar_ws_mgr::Message::WriteError(_name, err)) => {
                            log::error!("Websocket write error in Binance market data feed: {}", err);
                        }
                        Ok(roshar_ws_mgr::Message::PongReceiveTimeoutError(_name)) => {
                            log::warn!("Pong receive timeout in Binance market data feed");
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
                        self.handle_command(cmd).await;
                    } else {
                        self.pending_commands.push(cmd);
                    }
                }
            }
        }
    }

    fn resubscribe_all(&mut self) {
        for symbol in &self.depth_subscriptions {
            let sub_msg = roshar_types::BinanceWssMessage::depth(symbol).to_json();
            if let Err(e) = self.ws_manager.write(
                &self.conn_name,
                roshar_ws_mgr::Message::TextMessage(self.conn_name.clone(), sub_msg),
            ) {
                log::error!("Failed to resubscribe to depth for {}: {}", symbol, e);
            }
        }

        for symbol in &self.trades_subscriptions {
            let sub_msg = roshar_types::BinanceWssMessage::trades(symbol).to_json();
            if let Err(e) = self.ws_manager.write(
                &self.conn_name,
                roshar_ws_mgr::Message::TextMessage(self.conn_name.clone(), sub_msg),
            ) {
                log::error!("Failed to resubscribe to trades for {}: {}", symbol, e);
            }
        }
    }

    async fn handle_command(&mut self, cmd: SubscriptionCommand) {
        match cmd {
            SubscriptionCommand::AddDepth { symbol } => {
                if self.depth_subscriptions.insert(symbol.clone()) {
                    self.order_books
                        .insert(symbol.clone(), BinanceOrderBook::new(symbol.clone()));

                    let sub_msg = roshar_types::BinanceWssMessage::depth(&symbol).to_json();
                    if let Err(e) = self.ws_manager.write(
                        &self.conn_name,
                        roshar_ws_mgr::Message::TextMessage(self.conn_name.clone(), sub_msg),
                    ) {
                        log::error!("Failed to subscribe to depth for {}: {}", symbol, e);
                        self.depth_subscriptions.remove(&symbol);
                        self.order_books.remove(&symbol);
                        self.state.remove_book(&symbol);
                    } else {
                        log::info!("Subscribed to Binance depth for {}", symbol);
                    }
                }
            }
            SubscriptionCommand::RemoveDepth { symbol } => {
                if self.depth_subscriptions.remove(&symbol) {
                    self.order_books.remove(&symbol);
                    self.state.remove_book(&symbol);

                    let unsub_msg = roshar_types::BinanceWssMessage::depth_unsub(&symbol).to_json();
                    if let Err(e) = self.ws_manager.write(
                        &self.conn_name,
                        roshar_ws_mgr::Message::TextMessage(self.conn_name.clone(), unsub_msg),
                    ) {
                        log::error!("Failed to unsubscribe from depth for {}: {}", symbol, e);
                    } else {
                        log::info!("Unsubscribed from Binance depth for {}", symbol);
                    }
                }
            }
            SubscriptionCommand::AddTrades { symbol } => {
                if self.trades_subscriptions.insert(symbol.clone()) {
                    let sub_msg = roshar_types::BinanceWssMessage::trades(&symbol).to_json();
                    if let Err(e) = self.ws_manager.write(
                        &self.conn_name,
                        roshar_ws_mgr::Message::TextMessage(self.conn_name.clone(), sub_msg),
                    ) {
                        log::error!("Failed to subscribe to trades for {}: {}", symbol, e);
                        self.trades_subscriptions.remove(&symbol);
                    } else {
                        log::info!("Subscribed to Binance trades for {}", symbol);
                    }
                }
            }
            SubscriptionCommand::RemoveTrades { symbol } => {
                if self.trades_subscriptions.remove(&symbol) {
                    let unsub_msg =
                        roshar_types::BinanceWssMessage::trades_unsub(&symbol).to_json();
                    if let Err(e) = self.ws_manager.write(
                        &self.conn_name,
                        roshar_ws_mgr::Message::TextMessage(self.conn_name.clone(), unsub_msg),
                    ) {
                        log::error!("Failed to unsubscribe from trades for {}: {}", symbol, e);
                    } else {
                        log::info!("Unsubscribed from Binance trades for {}", symbol);
                    }
                }
            }
        }
    }

    async fn handle_message(&mut self, content: &str) {
        let msg = match SupportedMessages::from_message(content, Venue::Binance) {
            Some(msg) => msg,
            None => return,
        };

        match msg {
            SupportedMessages::BinanceDepthDiffMessage(depth_msg) => {
                self.handle_depth(depth_msg).await;
            }
            SupportedMessages::BinanceTradeMessage(trade_msg) => {
                self.handle_trade(trade_msg).await;
            }
            _ => {}
        }
    }

    async fn handle_depth(&mut self, msg: BinanceDepthDiffMessage) {
        let symbol = msg.symbol.clone();

        let book_state = if let Some(order_book) = self.order_books.get_mut(&symbol) {
            if let Err(e) = order_book.new_update_diff(&msg, &self.snapshot_semaphore) {
                log::warn!("Failed to update order book for {}: {:?}", symbol, e);
            }
            order_book.book.clone()
        } else {
            None
        };

        if let Some(book) = book_state {
            // Update the shared state for polling
            self.state.update_book(&symbol, book.clone());

            let _ = self
                .event_tx
                .send(MarketEvent::DepthUpdate {
                    symbol,
                    book: Arc::new(book),
                })
                .await;
        }
    }

    async fn handle_trade(&mut self, msg: BinanceTradeMessage) {
        let symbol = msg.symbol.clone();

        let px: f64 = match msg.price.parse() {
            Ok(v) => v,
            Err(_) => {
                log::error!("Failed to parse trade price: {}", msg.price);
                return;
            }
        };

        let sz: f64 = match msg.qty.parse() {
            Ok(v) => v,
            Err(_) => {
                log::error!("Failed to parse trade qty: {}", msg.qty);
                return;
            }
        };

        let trade = Trade {
            coin: symbol.clone(),
            side: !msg.is_buyer_maker,
            px,
            sz,
            time: msg.trade_time as i64,
            exchange: "binance".to_string(),
        };

        let _ = self
            .event_tx
            .send(MarketEvent::TradeUpdate {
                symbol,
                trades: Arc::new(vec![trade]),
            })
            .await;
    }
}
