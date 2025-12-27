use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use roshar_types::{
    KrakenBookDeltaMessage, KrakenBookSnapshotMessage, KrakenOrderBook, KrakenTradeDeltaMessage,
    KrakenTradeSnapshotMessage, KrakenWssMessage, OrderBookState, SupportedMessages, Trade, Venue,
};
use roshar_ws_mgr::Manager;
use tokio::sync::{mpsc, oneshot};

use crate::KRAKEN_WSS_URL;

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

pub enum SubscriptionCommand {
    AddDepth { symbol: String },
    RemoveDepth { symbol: String },
    AddTrades { symbol: String },
    RemoveTrades { symbol: String },
    GetDepth {
        symbol: String,
        response: oneshot::Sender<Option<OrderBookState>>,
    },
    GetEventChannel {
        response: oneshot::Sender<mpsc::Receiver<MarketEvent>>,
    },
    GetRawChannel {
        response: oneshot::Sender<mpsc::Receiver<String>>,
    },
    Restart,
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

    pub async fn get_latest_depth(&self, symbol: &str) -> Result<Option<OrderBookState>, String> {
        let (response_tx, response_rx) = oneshot::channel();
        self.command_tx
            .send(SubscriptionCommand::GetDepth {
                symbol: symbol.to_string(),
                response: response_tx,
            })
            .await
            .map_err(|e| format!("Failed to send get_depth command: {}", e))?;

        response_rx
            .await
            .map_err(|e| format!("Failed to receive depth response: {}", e))
    }

    pub async fn get_event_channel(&self) -> Result<mpsc::Receiver<MarketEvent>, String> {
        let (response_tx, response_rx) = oneshot::channel();
        self.command_tx
            .send(SubscriptionCommand::GetEventChannel {
                response: response_tx,
            })
            .await
            .map_err(|e| format!("Failed to send get_event_channel command: {}", e))?;

        response_rx
            .await
            .map_err(|e| format!("Failed to receive event channel: {}", e))
    }

    pub async fn get_raw_channel(&self) -> Result<mpsc::Receiver<String>, String> {
        let (response_tx, response_rx) = oneshot::channel();
        self.command_tx
            .send(SubscriptionCommand::GetRawChannel {
                response: response_tx,
            })
            .await
            .map_err(|e| format!("Failed to send get_raw_channel command: {}", e))?;

        response_rx
            .await
            .map_err(|e| format!("Failed to receive raw channel: {}", e))
    }

    pub async fn restart_feed(&self) -> Result<(), String> {
        self.command_tx
            .send(SubscriptionCommand::Restart)
            .await
            .map_err(|e| format!("Failed to send restart: {}", e))
    }
}

pub struct MarketDataFeed {
    ws_manager: Arc<Manager>,
    conn_name: String,

    order_books: HashMap<String, KrakenOrderBook>,
    event_tx: mpsc::Sender<MarketEvent>,
    event_rx: Option<mpsc::Receiver<MarketEvent>>,
    raw_tx: mpsc::Sender<String>,
    raw_rx: Option<mpsc::Receiver<String>>,

    command_rx: mpsc::Receiver<SubscriptionCommand>,
    command_tx: mpsc::Sender<SubscriptionCommand>,

    depth_subscriptions: HashSet<String>,
    trades_subscriptions: HashSet<String>,

    is_connected: bool,
    pending_commands: Vec<SubscriptionCommand>,

    raw_mode: bool,
    channel_size: usize,
}

impl MarketDataFeed {
    pub fn new(
        ws_manager: Arc<Manager>,
        channel_size: usize,
    ) -> Self {
        let (command_tx, command_rx) = mpsc::channel(100);
        let (event_tx, event_rx) = mpsc::channel(channel_size);
        let (raw_tx, raw_rx) = mpsc::channel(channel_size);

        Self {
            ws_manager,
            conn_name: "kraken-market-data".to_string(),
            order_books: HashMap::new(),
            event_tx,
            event_rx: Some(event_rx),
            raw_tx,
            raw_rx: Some(raw_rx),
            command_rx,
            command_tx,
            depth_subscriptions: HashSet::new(),
            trades_subscriptions: HashSet::new(),
            is_connected: false,
            pending_commands: Vec::new(),
            raw_mode: false,
            channel_size,
        }
    }

    pub fn get_handle(&self) -> MarketDataFeedHandle {
        MarketDataFeedHandle {
            command_tx: self.command_tx.clone(),
        }
    }

    pub async fn run(mut self) {
        let mut recv = self.ws_manager.setup_reader(&self.conn_name, self.channel_size);
        log::info!(
            "WebSocket reader set up for Kraken market data feed: {}",
            self.conn_name
        );

        let ws_config = roshar_ws_mgr::Config {
            name: self.conn_name.clone(),
            url: KRAKEN_WSS_URL.to_string(),
            ping_duration: 10,
            ping_message: KrakenWssMessage::ping().to_json(),
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
                "Failed to create Kraken market data WebSocket connection: {}",
                e
            );
            return;
        }

        loop {
            tokio::select! {
                msg = recv.recv() => {
                    match msg {
                        Ok(roshar_ws_mgr::Message::SuccessfulHandshake(_name)) => {
                            log::info!("Kraken market data feed WebSocket connected: {}", self.conn_name);
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
                            log::error!("Websocket read error in Kraken market data feed: {}", err);
                            self.is_connected = false;
                            if let Err(e) = self.ws_manager.reconnect_with_close(&self.conn_name, false).await {
                                log::error!("Failed to trigger reconnect after read error: {}", e);
                            }
                        }
                        Ok(roshar_ws_mgr::Message::WriteError(_name, err)) => {
                            log::error!("Websocket write error in Kraken market data feed: {}", err);
                            // Don't reconnect - ReadError or CloseMessage should trigger it
                        }
                        Ok(roshar_ws_mgr::Message::CloseMessage(_name, reason)) => {
                            if let Some(close_reason) = reason.as_ref() {
                                log::error!("Kraken websocket closed with reason: {}", close_reason);
                            } else {
                                log::error!("Kraken websocket closed without reason");
                            }
                            self.is_connected = false;
                            if let Err(e) = self.ws_manager.reconnect_with_close(&self.conn_name, false).await {
                                log::error!("Failed to trigger reconnect after close: {}", e);
                            }
                        }
                        Ok(roshar_ws_mgr::Message::PongReceiveTimeoutError(_name)) => {
                            log::warn!("Pong receive timeout in Kraken market data feed");
                            self.is_connected = false;
                            if let Err(e) = self.ws_manager.reconnect(&self.conn_name).await {
                                log::error!("Failed to trigger reconnect after pong timeout: {}", e);
                            }
                        }
                        Ok(_) => {}
                        Err(e) => {
                            use tokio::sync::broadcast::error::RecvError;
                            match e {
                                RecvError::Lagged(skipped) => {
                                    log::warn!("Broadcast channel lagged, skipped {} messages", skipped);
                                }
                                RecvError::Closed => {
                                    log::error!("Broadcast channel closed");
                                    break;
                                }
                            }
                        }
                    }
                }
                Some(cmd) = self.command_rx.recv() => {
                    match &cmd {
                        SubscriptionCommand::GetDepth { .. } => {
                            self.handle_command(cmd).await;
                        }
                        _ => {
                            if self.is_connected {
                                self.handle_command(cmd).await;
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
        for symbol in &self.depth_subscriptions {
            let sub_msg = KrakenWssMessage::depth(symbol).to_json();
            if let Err(e) = self.ws_manager.write(
                &self.conn_name,
                roshar_ws_mgr::Message::TextMessage(self.conn_name.clone(), sub_msg),
            ) {
                log::error!("Failed to resubscribe to depth for {}: {}", symbol, e);
            }
        }

        for symbol in &self.trades_subscriptions {
            let sub_msg = KrakenWssMessage::trades(symbol).to_json();
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
                        .insert(symbol.clone(), KrakenOrderBook::new(symbol.clone()));

                    let sub_msg = KrakenWssMessage::depth(&symbol).to_json();
                    if let Err(e) = self.ws_manager.write(
                        &self.conn_name,
                        roshar_ws_mgr::Message::TextMessage(self.conn_name.clone(), sub_msg),
                    ) {
                        log::error!("Failed to subscribe to depth for {}: {}", symbol, e);
                        self.depth_subscriptions.remove(&symbol);
                        self.order_books.remove(&symbol);
                    } else {
                        log::info!("Subscribed to Kraken depth for {}", symbol);
                    }
                }
            }
            SubscriptionCommand::RemoveDepth { symbol } => {
                if self.depth_subscriptions.remove(&symbol) {
                    self.order_books.remove(&symbol);

                    let unsub_msg = KrakenWssMessage::depth_unsub(&symbol).to_json();
                    if let Err(e) = self.ws_manager.write(
                        &self.conn_name,
                        roshar_ws_mgr::Message::TextMessage(self.conn_name.clone(), unsub_msg),
                    ) {
                        log::error!("Failed to unsubscribe from depth for {}: {}", symbol, e);
                    } else {
                        log::info!("Unsubscribed from Kraken depth for {}", symbol);
                    }
                }
            }
            SubscriptionCommand::AddTrades { symbol } => {
                if self.trades_subscriptions.insert(symbol.clone()) {
                    let sub_msg = KrakenWssMessage::trades(&symbol).to_json();
                    if let Err(e) = self.ws_manager.write(
                        &self.conn_name,
                        roshar_ws_mgr::Message::TextMessage(self.conn_name.clone(), sub_msg),
                    ) {
                        log::error!("Failed to subscribe to trades for {}: {}", symbol, e);
                        self.trades_subscriptions.remove(&symbol);
                    } else {
                        log::info!("Subscribed to Kraken trades for {}", symbol);
                    }
                }
            }
            SubscriptionCommand::RemoveTrades { symbol } => {
                if self.trades_subscriptions.remove(&symbol) {
                    let unsub_msg = KrakenWssMessage::trades_unsub(&symbol).to_json();
                    if let Err(e) = self.ws_manager.write(
                        &self.conn_name,
                        roshar_ws_mgr::Message::TextMessage(self.conn_name.clone(), unsub_msg),
                    ) {
                        log::error!("Failed to unsubscribe from trades for {}: {}", symbol, e);
                    } else {
                        log::info!("Unsubscribed from Kraken trades for {}", symbol);
                    }
                }
            }
            SubscriptionCommand::GetDepth { symbol, response } => {
                let result = self
                    .order_books
                    .get(&symbol)
                    .and_then(|book| book.book.clone());
                let _ = response.send(result);
            }
            SubscriptionCommand::GetEventChannel { response } => {
                if let Some(event_rx) = self.event_rx.take() {
                    self.raw_mode = false;
                    log::info!("Event channel requested for Kraken market data feed, raw_mode disabled");
                    let _ = response.send(event_rx);
                } else {
                    log::warn!("Event channel already taken for Kraken market data feed");
                }
            }
            SubscriptionCommand::GetRawChannel { response } => {
                if let Some(raw_rx) = self.raw_rx.take() {
                    self.raw_mode = true;
                    log::info!("Raw channel requested for Kraken market data feed, raw_mode enabled");
                    let _ = response.send(raw_rx);
                } else {
                    log::warn!("Raw channel already taken for Kraken market data feed");
                }
            }
            SubscriptionCommand::Restart => {
                self.is_connected = false;
                let _ = self.ws_manager.reconnect_with_close(&self.conn_name, true).await;
                self.order_books.clear();
                // Resubcription will happen after SuccessfulHandshake
            }
        }
    }

    async fn handle_message(&mut self, content: &str) {
        if self.raw_mode {
            let _ = self.raw_tx.send(content.to_string()).await;
            return;
        }

        let msg = match SupportedMessages::from_message(content, Venue::Kraken) {
            Some(msg) => msg,
            None => return,
        };

        match msg {
            SupportedMessages::KrakenBookSnapshotMessage(snapshot_msg) => {
                self.handle_snapshot(snapshot_msg).await;
            }
            SupportedMessages::KrakenBookDeltaMessage(delta_msg) => {
                self.handle_delta(delta_msg).await;
            }
            SupportedMessages::KrakenTradeSnapshotMessage(trade_snapshot_msg) => {
                self.handle_trade_snapshot(trade_snapshot_msg).await;
            }
            SupportedMessages::KrakenTradeDeltaMessage(trade_delta_msg) => {
                self.handle_trade_delta(trade_delta_msg).await;
            }
            _ => {}
        }
    }

    async fn handle_snapshot(&mut self, msg: KrakenBookSnapshotMessage) {
        let symbol = msg.product_id.clone();

        let book_state = if let Some(order_book) = self.order_books.get_mut(&symbol) {
            order_book.new_snapshot(&msg);
            order_book.book.clone()
        } else {
            None
        };

        if let Some(book) = book_state {
            let _ = self
                .event_tx
                .send(MarketEvent::DepthUpdate {
                    symbol,
                    book: Arc::new(book),
                })
                .await;
        }
    }

    async fn handle_delta(&mut self, msg: KrakenBookDeltaMessage) {
        let symbol = msg.product_id.clone();

        let book_state = if let Some(order_book) = self.order_books.get_mut(&symbol) {
            if let Err(e) = order_book.new_update(&msg) {
                log::error!("Failed to update order book for {}: {:?}", symbol, e);
                return;
            }
            order_book.book.clone()
        } else {
            None
        };

        if let Some(book) = book_state {
            let _ = self
                .event_tx
                .send(MarketEvent::DepthUpdate {
                    symbol,
                    book: Arc::new(book),
                })
                .await;
        }
    }

    async fn handle_trade_snapshot(&self, msg: KrakenTradeSnapshotMessage) {
        let symbol = msg.product_id.clone();
        let trades = msg.to_trades();

        if !trades.is_empty() {
            let _ = self
                .event_tx
                .send(MarketEvent::TradeUpdate {
                    symbol,
                    trades: Arc::new(trades),
                })
                .await;
        }
    }

    async fn handle_trade_delta(&self, msg: KrakenTradeDeltaMessage) {
        let symbol = msg.product_id.clone();
        let trade = msg.to_trade();

        let _ = self
            .event_tx
            .send(MarketEvent::TradeUpdate {
                symbol,
                trades: Arc::new(vec![trade]),
            })
            .await;
    }
}
