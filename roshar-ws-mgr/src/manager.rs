use std::{
    net::{SocketAddr, ToSocketAddrs},
    sync::{
        Arc, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use anyhow::{Result, anyhow};
use dashmap::DashMap;
use futures_util::StreamExt;
use log::{debug, error};
use tokio::{net::TcpSocket, task::JoinHandle, time::sleep};
use tokio_tungstenite;
use tokio_util::sync::CancellationToken;
use url::Url;

use crate::{
    config::Config,
    connection::{Connection, PingActorHandle, ReadActorHandle, WriteActorHandle},
};

#[derive(Clone, Debug)]
pub enum Message {
    PingMessage(String, String),
    PongMessage(String, String),
    TextMessage(String, String),
    BinaryMessage(String, Vec<u8>),
    CloseMessage(String, Option<String>),
    FrameMessage(String, String),
    ReadError(String, String),
    WriteError(String, String),
    PongReceiveTimeoutError(String),
    SuccessfulHandshake(String),
    FailedHandshake(String, i32),
    ConnectionStarting(String),
}

#[derive(Debug)]
pub enum ManagerError {
    Unknown(String),
    Inactive(String),
    Restarting(String),
}

impl std::fmt::Display for ManagerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ManagerError::Unknown(conn) => write!(f, "Connection Unknown: {:?}", conn),
            ManagerError::Inactive(conn) => write!(f, "Connection Inactive: {:?}", conn),
            ManagerError::Restarting(conn) => write!(f, "Connection Restarting: {:?}", conn),
        }
    }
}

pub struct Manager {
    state: DashMap<String, Connection>,
    exchange_channels: DashMap<String, Arc<tokio::sync::broadcast::Sender<Message>>>,
    global_send: Arc<tokio::sync::broadcast::Sender<Message>>, // Keep for backwards compatibility
    loops_started: AtomicBool,
    pong_handle: OnceLock<JoinHandle<()>>,
}

impl Manager {
    pub fn new() -> Arc<Self> {
        let (global_send, _global_recv) = tokio::sync::broadcast::channel::<Message>(32_768);

        Arc::new(Self {
            state: DashMap::new(),
            exchange_channels: DashMap::new(),
            global_send: Arc::new(global_send),
            loops_started: AtomicBool::new(false),
            pong_handle: OnceLock::new(),
        })
    }

    fn ensure_loops_started(self: &Arc<Self>) {
        if self
            .loops_started
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            let manager_clone1 = Arc::clone(self);
            let _pong_handle = tokio::spawn(async move {
                manager_clone1.pong_loop().await;
            });

            let _ = self.pong_handle.set(_pong_handle);
        }
    }

    pub fn setup_reader(
        self: &Arc<Self>,
        exchange: &str,
        channel_size: usize,
    ) -> tokio::sync::broadcast::Receiver<Message> {
        let sender = if let Some(existing_sender) = self.exchange_channels.get(exchange) {
            existing_sender.clone()
        } else {
            let (sender, _) = tokio::sync::broadcast::channel::<Message>(channel_size);
            let sender_arc = Arc::new(sender);
            self.exchange_channels
                .insert(exchange.to_string(), sender_arc.clone());
            log::info!(
                "Created new broadcast channel for exchange: {} (buffer: {})",
                exchange,
                channel_size
            );
            sender_arc
        };

        sender.subscribe()
    }

    async fn establish_connection(
        &self,
        name: &str,
        config: &Config,
        exchange_sender: Arc<tokio::sync::broadcast::Sender<Message>>,
    ) -> Result<(
        WriteActorHandle,
        ReadActorHandle,
        PingActorHandle,
        CancellationToken,
    )> {
        // Configure WebSocket settings
        let mut ws_config = tokio_tungstenite::tungstenite::protocol::WebSocketConfig::default();

        if let Some(read_buffer) = config.read_buffer_size {
            ws_config.read_buffer_size = read_buffer;
        }
        if let Some(write_buffer) = config.write_buffer_size {
            ws_config.write_buffer_size = write_buffer;
        }
        if let Some(max_message) = config.max_message_size {
            ws_config.max_message_size = Some(max_message);
        }
        if let Some(max_frame) = config.max_frame_size {
            ws_config.max_frame_size = Some(max_frame);
        }

        let (ws_stream, response) = if config.tcp_recv_buffer_size.is_some()
            || config.tcp_send_buffer_size.is_some()
            || config.tcp_nodelay.is_some()
        {
            // Custom TCP socket configuration
            let url = Url::parse(&config.url)?;
            let host = url
                .host_str()
                .ok_or_else(|| anyhow!("Invalid URL: no host"))?;
            let port = url
                .port()
                .unwrap_or(if url.scheme() == "wss" { 443 } else { 80 });

            let socket = TcpSocket::new_v4()?;

            if let Some(recv_buf_size) = config.tcp_recv_buffer_size {
                socket.set_recv_buffer_size(recv_buf_size as u32)?;
            }
            if let Some(send_buf_size) = config.tcp_send_buffer_size {
                socket.set_send_buffer_size(send_buf_size as u32)?;
            }
            if let Some(nodelay) = config.tcp_nodelay {
                socket.set_nodelay(nodelay)?;
            }

            let socket_addr: Vec<SocketAddr> = format!("{}:{}", host, port)
                .to_socket_addrs()?
                .filter(|addr| addr.is_ipv4())
                .collect();

            if socket_addr.is_empty() {
                log::error!("No IPv4 address found for the host");
            }

            let first = socket_addr.first().unwrap();
            let tcp_stream = socket.connect(*first).await?;

            match tokio::time::timeout(
                Duration::from_secs(30),
                tokio_tungstenite::client_async_tls_with_config(
                    &config.url,
                    tcp_stream,
                    Some(ws_config),
                    None,
                ),
            )
            .await
            {
                Ok(Ok(res)) => res,
                Ok(Err(e)) => {
                    error!("Failed to connect to '{}': {}", config.url, e);
                    return Err(anyhow!("Connection failed: {}", e));
                }
                Err(_) => {
                    error!("Connection timeout for '{}'", config.url);
                    return Err(anyhow!("Connection timeout"));
                }
            }
        } else {
            // Standard connection without custom TCP configuration
            match tokio::time::timeout(
                Duration::from_secs(30),
                tokio_tungstenite::connect_async_with_config(&config.url, Some(ws_config), false),
            )
            .await
            {
                Ok(Ok(res)) => res,
                Ok(Err(e)) => {
                    error!("Failed to connect to '{}': {}", config.url, e);
                    return Err(anyhow!("Connection failed: {}", e));
                }
                Err(_) => {
                    error!("Connection timeout for '{}'", config.url);
                    return Err(anyhow!("Connection timeout"));
                }
            }
        };

        debug!("Handshake response: {:?}", response);
        let (write_stream, read_stream) = ws_stream.split();

        let cancel_token = CancellationToken::new();

        let writer = WriteActorHandle::new(
            name.to_string(),
            write_stream,
            cancel_token.clone(),
            exchange_sender.clone(),
        );
        let reader = ReadActorHandle::new(
            name.to_string(),
            read_stream,
            cancel_token.clone(),
            exchange_sender.clone(),
        );
        let ping = PingActorHandle::new(
            name.to_string(),
            config.clone(),
            Arc::clone(&writer.sender),
            exchange_sender.clone(),
            cancel_token.clone(),
        );

        Ok((writer, reader, ping, cancel_token))
    }

    pub fn write(self: &Arc<Self>, name: &str, message: Message) -> Result<()> {
        if let Some(connection) = self.state.get(name) {
            if let Ok(writer) = connection.write() {
                let _ = writer.send(message);
                return Ok(());
            }
            return Err(anyhow!(ManagerError::Inactive(name.to_string())));
        }
        Err(anyhow!(ManagerError::Unknown(name.to_string())))
    }

    pub fn close_conn(self: &Arc<Self>, name: &str) -> Result<Config> {
        self.close_conn_internal(name, true)
    }

    fn close_conn_internal(self: &Arc<Self>, name: &str, send_close: bool) -> Result<Config> {
        if let Some((name, connection)) = self.state.remove(name) {
            if send_close {
                if let Ok(writer) = connection.write() {
                    let _ = writer.send(Message::CloseMessage(name.to_string(), None));
                    log::info!("Sent close message: {}", name);
                }
            } else {
                log::info!("Skipping close message for: {}", name);
            }
            if let Some(cancel_token) = connection.get_cancel_token() {
                cancel_token.cancel();
                log::info!("Called cancel token: {}", name);
            }
            return Ok(connection.config);
        }
        Err(anyhow!(ManagerError::Unknown(name.to_string())))
    }

    pub fn new_conn(self: &Arc<Self>, name: &str, config: Config) -> Result<()> {
        self.ensure_loops_started();

        let name_owned = name.to_string();

        // Get the exchange-specific channel (should already exist from setup_reader)
        let exchange = config.name.clone();
        let exchange_send = if let Some(sender) = self.exchange_channels.get(&exchange) {
            sender.clone()
        } else {
            return Err(anyhow!(ManagerError::Unknown(format!(
                "Exchange '{}' not set up. Call setup_reader first.",
                exchange
            ))));
        };

        let manager = Arc::clone(self);
        let config_clone = config.clone();

        tokio::spawn(async move {
            log::info!("Started reconnect loop for {}", name_owned.clone());
            let initial_back_off = 1.0;
            let mut attempt = 0;

            let _ = exchange_send.send(Message::ConnectionStarting(name_owned.clone()));
            loop {
                let back_off = initial_back_off * 2.0_f64.powi(attempt).min(60.0);
                log::info!(
                    "Connection attempt {} for {} (backoff: {}s)",
                    attempt + 1,
                    name_owned,
                    back_off as u64
                );

                match manager
                    .establish_connection(&name_owned, &config_clone, exchange_send.clone())
                    .await
                {
                    Ok((writer, reader, ping, cancel_token)) => {
                        manager.state.insert(
                            name_owned.to_string(),
                            Connection::new_initialized(
                                &name_owned,
                                config.clone(),
                                writer,
                                reader,
                                ping,
                                cancel_token,
                            ),
                        );
                        log::info!("Inserted connection: {}", name_owned);

                        log::info!("Connected, pausing: {}", name_owned);
                        sleep(Duration::from_secs(1)).await;
                        let _ =
                            exchange_send.send(Message::SuccessfulHandshake(name_owned.clone()));
                        log::info!("Sent successful handshake: {}", name_owned);

                        break;
                    }
                    Err(e) => {
                        log::error!(
                            "Connection attempt {} failed for {}: {}",
                            attempt,
                            name_owned,
                            e
                        );
                        let _ = exchange_send
                            .send(Message::FailedHandshake(name_owned.clone(), attempt));
                        log::info!("Sent failed handshake: {}", name_owned);
                        attempt += 1;
                        sleep(Duration::from_secs(back_off as u64)).await;
                    }
                }
            }
        });
        Ok(())
    }

    pub async fn reconnect(self: &Arc<Self>, name: &str) -> Result<()> {
        self.reconnect_with_close(name, true).await
    }

    pub async fn reconnect_with_close(
        self: &Arc<Self>,
        name: &str,
        send_close: bool,
    ) -> Result<()> {
        match self.close_conn_internal(name, send_close) {
            Ok(config) => {
                let _ = self.new_conn(name, config);
                Ok(())
            }
            Err(e) => Err(anyhow!(e)),
        }
    }

    pub async fn pong_loop(self: &Arc<Self>) {
        let mut global_recv = self.global_send.subscribe();

        loop {
            if let Ok(Message::PingMessage(name, val)) = global_recv.recv().await {
                if let Some(connection) = self.state.get(&name) {
                    if let Ok(writer) = connection.write() {
                        let _ = writer.send(Message::PongMessage(name, val));
                    }
                }
            }
        }
    }
}

impl Drop for Manager {
    fn drop(&mut self) {
        let connection_names: Vec<String> =
            self.state.iter().map(|entry| entry.key().clone()).collect();

        for name in connection_names {
            if let Some((_, connection)) = self.state.remove(&name) {
                if let Ok(writer) = connection.write() {
                    let _ = writer.send(Message::CloseMessage(name.clone(), None));
                }
                if let Some(cancel_token) = connection.get_cancel_token() {
                    cancel_token.cancel();
                }
            }
        }

        if let Some(pong_handle) = self.pong_handle.get() {
            pong_handle.abort();
        }
    }
}
