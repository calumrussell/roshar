use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use roshar_types::{
    Candle, HlOrderBook, HyperliquidBookMessage, HyperliquidCandleMessage,
    HyperliquidTradesMessage, HyperliquidWssMessage, OrderBookState, SupportedMessages, Trade,
    Venue,
};
use roshar_ws_mgr::Manager;
use tokio::sync::{mpsc, oneshot};

use crate::{HL_TESTNET_WSS_URL, HL_WSS_URL};

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
    CandleUpdate {
        coin: String,
        candle: Arc<Candle>,
    },
}

pub enum SubscriptionCommand {
    AddDepth { coin: String },
    RemoveDepth { coin: String },
    AddTrades { coin: String },
    RemoveTrades { coin: String },
    AddCandles { coin: String },
    RemoveCandles { coin: String },
    GetDepth {
        coin: String,
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

    pub async fn add_candles(&self, coin: &str) -> Result<(), String> {
        self.command_tx
            .send(SubscriptionCommand::AddCandles {
                coin: coin.to_string(),
            })
            .await
            .map_err(|e| format!("Failed to send add_candles command: {}", e))
    }

    pub async fn remove_candles(&self, coin: &str) -> Result<(), String> {
        self.command_tx
            .send(SubscriptionCommand::RemoveCandles {
                coin: coin.to_string(),
            })
            .await
            .map_err(|e| format!("Failed to send remove_candles command: {}", e))
    }

    pub async fn get_latest_depth(&self, coin: &str) -> Result<Option<OrderBookState>, String> {
        let (response_tx, response_rx) = oneshot::channel();
        self.command_tx
            .send(SubscriptionCommand::GetDepth {
                coin: coin.to_string(),
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
    is_testnet: bool,
    conn_name: String,

    order_books: HashMap<String, HlOrderBook>,
    event_tx: mpsc::Sender<MarketEvent>,
    event_rx: Option<mpsc::Receiver<MarketEvent>>,
    raw_tx: mpsc::Sender<String>,
    raw_rx: Option<mpsc::Receiver<String>>,

    command_rx: mpsc::Receiver<SubscriptionCommand>,
    command_tx: mpsc::Sender<SubscriptionCommand>,

    depth_subscriptions: HashSet<String>,
    trades_subscriptions: HashSet<String>,
    candles_subscriptions: HashSet<String>,

    is_connected: bool,
    pending_commands: Vec<SubscriptionCommand>,

    raw_mode: bool,
    channel_size: usize,
}

impl MarketDataFeed {
    pub fn new(
        ws_manager: Arc<Manager>,
        is_testnet: bool,
        channel_size: usize,
    ) -> Self {
        let (command_tx, command_rx) = mpsc::channel(100);
        let (event_tx, event_rx) = mpsc::channel(channel_size);
        let (raw_tx, raw_rx) = mpsc::channel(channel_size);

        Self {
            ws_manager,
            is_testnet,
            conn_name: "hyperliquid-market-data".to_string(),
            order_books: HashMap::new(),
            event_tx,
            event_rx: Some(event_rx),
            raw_tx,
            raw_rx: Some(raw_rx),
            command_rx,
            command_tx,
            depth_subscriptions: HashSet::new(),
            trades_subscriptions: HashSet::new(),
            candles_subscriptions: HashSet::new(),
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
                            log::error!("Websocket read error in market data feed: {}", err);
                            self.is_connected = false;
                            if let Err(e) = self.ws_manager.reconnect_with_close(&self.conn_name, false).await {
                                log::error!("Failed to trigger reconnect after read error: {}", e);
                            }
                        }
                        Ok(roshar_ws_mgr::Message::WriteError(_name, err)) => {
                            log::error!("Websocket write error in market data feed: {}", err);
                            // Don't reconnect - ReadError or CloseMessage should trigger it
                        }
                        Ok(roshar_ws_mgr::Message::CloseMessage(_name, reason)) => {
                            if let Some(close_reason) = reason.as_ref() {
                                log::error!("Hyperliquid websocket closed with reason: {}", close_reason);
                            } else {
                                log::error!("Hyperliquid websocket closed without reason");
                            }
                            self.is_connected = false;
                            if let Err(e) = self.ws_manager.reconnect_with_close(&self.conn_name, false).await {
                                log::error!("Failed to trigger reconnect after close: {}", e);
                            }
                        }
                        Ok(roshar_ws_mgr::Message::PongReceiveTimeoutError(_name)) => {
                            log::warn!("Pong receive timeout in market data feed");
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

        for coin in &self.candles_subscriptions {
            let sub_msg = HyperliquidWssMessage::candle(coin).to_json();
            if let Err(e) = self.ws_manager.write(
                &self.conn_name,
                roshar_ws_mgr::Message::TextMessage(self.conn_name.clone(), sub_msg),
            ) {
                log::error!("Failed to resubscribe to candles for {}: {}", coin, e);
            }
        }
    }

    async fn handle_command(&mut self, cmd: SubscriptionCommand) {
        match cmd {
            SubscriptionCommand::AddDepth { coin } => {
                if self.depth_subscriptions.insert(coin.clone()) {
                    self.order_books
                        .insert(coin.clone(), HlOrderBook::new(coin.clone()));

                    let sub_msg = HyperliquidWssMessage::l2_book(&coin).to_json();
                    if let Err(e) = self.ws_manager.write(
                        &self.conn_name,
                        roshar_ws_mgr::Message::TextMessage(self.conn_name.clone(), sub_msg),
                    ) {
                        log::error!("Failed to subscribe to depth for {}: {}", coin, e);
                        self.depth_subscriptions.remove(&coin);
                        self.order_books.remove(&coin);
                    } else {
                        log::info!("Subscribed to depth for {}", coin);
                    }
                }
            }
            SubscriptionCommand::RemoveDepth { coin } => {
                if self.depth_subscriptions.remove(&coin) {
                    self.order_books.remove(&coin);

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
            SubscriptionCommand::AddCandles { coin } => {
                if self.candles_subscriptions.insert(coin.clone()) {
                    let sub_msg = HyperliquidWssMessage::candle(&coin).to_json();
                    if let Err(e) = self.ws_manager.write(
                        &self.conn_name,
                        roshar_ws_mgr::Message::TextMessage(self.conn_name.clone(), sub_msg),
                    ) {
                        log::error!("Failed to subscribe to candles for {}: {}", coin, e);
                        self.candles_subscriptions.remove(&coin);
                    } else {
                        log::info!("Subscribed to candles for {}", coin);
                    }
                }
            }
            SubscriptionCommand::RemoveCandles { coin } => {
                if self.candles_subscriptions.remove(&coin) {
                    let unsub_msg = HyperliquidWssMessage::candle_unsub(&coin).to_json();
                    if let Err(e) = self.ws_manager.write(
                        &self.conn_name,
                        roshar_ws_mgr::Message::TextMessage(self.conn_name.clone(), unsub_msg),
                    ) {
                        log::error!("Failed to unsubscribe from candles for {}: {}", coin, e);
                    } else {
                        log::info!("Unsubscribed from candles for {}", coin);
                    }
                }
            }
            SubscriptionCommand::GetDepth { coin, response } => {
                let result = self
                    .order_books
                    .get(&coin)
                    .and_then(|book| book.book.clone());
                let _ = response.send(result);
            }
            SubscriptionCommand::GetEventChannel { response } => {
                if let Some(event_rx) = self.event_rx.take() {
                    self.raw_mode = false;
                    log::info!("Event channel requested for Hyperliquid market data feed, raw_mode disabled");
                    let _ = response.send(event_rx);
                } else {
                    log::warn!("Event channel already taken for Hyperliquid market data feed");
                }
            }
            SubscriptionCommand::GetRawChannel { response } => {
                if let Some(raw_rx) = self.raw_rx.take() {
                    self.raw_mode = true;
                    log::info!("Raw channel requested for Hyperliquid market data feed, raw_mode enabled");
                    let _ = response.send(raw_rx);
                } else {
                    log::warn!("Raw channel already taken for Hyperliquid market data feed");
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

        // Parse and send to event channel
        // Try to parse as candle message first (not in SupportedMessages)
        if let Ok(candle_msg) = serde_json::from_str::<HyperliquidCandleMessage>(content) {
            self.handle_candle(candle_msg).await;
            return;
        }

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

        let book_state = if let Some(order_book) = self.order_books.get_mut(&coin) {
            if let Err(e) = order_book.new_message(&msg) {
                log::error!("Failed to update order book for {}: {:?}", coin, e);
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

    async fn handle_candle(&self, msg: HyperliquidCandleMessage) {
        let candle = msg.to_candle();
        let coin = candle.coin.clone();

        let _ = self
            .event_tx
            .send(MarketEvent::CandleUpdate {
                coin,
                candle: Arc::new(candle),
            })
            .await;
    }
}
