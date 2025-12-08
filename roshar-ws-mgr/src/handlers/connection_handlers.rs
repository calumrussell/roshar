use crate::handlers::config::HandlerConfig;
use crate::handlers::{
    ConnectionEventHandler, ExchangeSubscriptionManager, MessageHandler, ReadTimeoutHandler,
};
use anyhow::Result;
use log::{info, warn};
use roshar_types::WebsocketSupportedExchanges;
use crate::Manager;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;

/// Builder and runtime manager for WebSocket connection handlers
///
/// This struct serves dual purposes:
/// 1. Configuration: Use builder methods to configure handlers
/// 2. Runtime: After calling start(), use control methods (stop_all, wait_all)
///
/// # Example
///
/// ```no_run
/// use roshar_ws_mgr::{Manager, Config, handlers::{ConnectionHandlers, HandlerConfig}};
/// use std::sync::Arc;
///
/// # async fn example() -> anyhow::Result<()> {
/// let mgr = Manager::new();
/// // mgr.new_conn("hyperliquid", config)?;
///
/// let mut handlers = ConnectionHandlers::new(mgr, "hyperliquid".to_string())
///     .with_config(HandlerConfig { channel_size: 65_536 })
///     .with_error_handling()
///     .with_read_timeout(30)
///     .start();
///
/// // Application code runs in parallel with handlers
/// // let mut rx = mgr.setup_reader("hyperliquid", 65_536);
/// // while let Ok(msg) = rx.recv().await {
/// //     // Business logic
/// // }
///
/// // Clean shutdown
/// handlers.stop_all();
/// # Ok(())
/// # }
/// ```
pub struct ConnectionHandlers {
    manager: Arc<Manager>,
    connection_name: String,
    config: HandlerConfig,
    enable_error_handling: bool,
    exchange_subscription: Option<(WebsocketSupportedExchanges, HashMap<String, Vec<String>>)>,
    read_timeout_seconds: Option<u64>,
    // Runtime state (populated after start())
    handlers: Vec<Arc<dyn MessageHandler>>,
    receiver_handle: Option<JoinHandle<Result<()>>>,
    background_handles: Vec<JoinHandle<()>>,
}

impl ConnectionHandlers {
    /// Create a new handler builder
    ///
    /// # Arguments
    ///
    /// * `manager` - The Manager instance
    /// * `connection_name` - Name of the connection (must match the name used in new_conn)
    pub fn new(manager: Arc<Manager>, connection_name: String) -> Self {
        Self {
            manager,
            connection_name,
            config: HandlerConfig::default(),
            enable_error_handling: false,
            exchange_subscription: None,
            read_timeout_seconds: None,
            handlers: Vec::new(),
            receiver_handle: None,
            background_handles: Vec::new(),
        }
    }

    /// Configure handler settings
    ///
    /// # Arguments
    ///
    /// * `config` - Handler configuration (channel sizes, etc.)
    pub fn with_config(mut self, config: HandlerConfig) -> Self {
        self.config = config;
        self
    }

    /// Enable automatic error detection and reconnection
    ///
    /// When enabled, monitors for:
    /// - ReadError
    /// - WriteError
    /// - CloseMessage
    /// - PongReceiveTimeoutError
    ///
    /// And triggers `manager.reconnect()` when detected.
    pub fn with_error_handling(mut self) -> Self {
        self.enable_error_handling = true;
        self
    }

    /// Enable read timeout detection
    ///
    /// # Arguments
    ///
    /// * `timeout_seconds` - Number of seconds without messages before reconnecting
    ///
    /// Spawns a background task that checks every 10 seconds and triggers
    /// reconnection if no messages have been received within the timeout period.
    pub fn with_read_timeout(mut self, timeout_seconds: u64) -> Self {
        self.read_timeout_seconds = Some(timeout_seconds);
        self
    }

    /// Enable automatic exchange-specific subscription management
    ///
    /// # Arguments
    ///
    /// * `exchange` - Exchange type
    /// * `channels` - HashMap of channel types to coin lists
    ///
    /// Automatically generates and sends exchange-specific subscription messages
    /// on successful handshake.
    pub fn with_exchange_subscription(
        mut self,
        exchange: WebsocketSupportedExchanges,
        channels: HashMap<String, Vec<String>>,
    ) -> Self {
        self.exchange_subscription = Some((exchange, channels));
        self
    }

    /// Start all configured handlers
    ///
    /// Spawns a dispatcher with all configured handlers. The dispatcher runs a single
    /// receiver loop that dispatches messages to all handlers.
    ///
    /// After calling this, the struct transitions to runtime mode and you can use
    /// stop_all() and wait_all().
    pub fn start(mut self) -> Self {
        info!("[{}] Starting connection handlers", self.connection_name);

        // Add error handling handler
        if self.enable_error_handling {
            info!(
                "[{}] → Enabling connection event handler",
                self.connection_name
            );
            let handler =
                ConnectionEventHandler::new(self.manager.clone(), self.connection_name.clone());
            self.handlers.push(Arc::new(handler));
        }

        // Add exchange subscription handler
        if let Some((exchange, channels)) = self.exchange_subscription.clone() {
            info!(
                "[{}] → Enabling exchange subscription manager for {:?}",
                self.connection_name, exchange
            );
            let handler = ExchangeSubscriptionManager::new(
                self.manager.clone(),
                self.connection_name.clone(),
                exchange,
                channels,
            );
            self.handlers.push(Arc::new(handler));
        }

        // Add read timeout handler
        if let Some(timeout_seconds) = self.read_timeout_seconds {
            info!(
                "[{}] → Enabling read timeout handler ({}s timeout)",
                self.connection_name, timeout_seconds
            );
            let handler = ReadTimeoutHandler::new(
                self.manager.clone(),
                self.connection_name.clone(),
                timeout_seconds,
            );
            self.handlers.push(Arc::new(handler));
        }

        // Spawn background tasks from each handler
        for handler in &self.handlers {
            let tasks = handler.spawn_background_tasks();
            self.background_handles.extend(tasks);
        }

        if !self.background_handles.is_empty() {
            info!(
                "[{}] → Spawned {} background tasks",
                self.connection_name,
                self.background_handles.len()
            );
        }

        // Set up receiver and start dispatcher
        let mut receiver = self
            .manager
            .setup_reader(&self.connection_name, self.config.channel_size);

        let connection_name = self.connection_name.clone();
        let handlers = self.handlers.clone();

        let handle = tokio::spawn(async move {
            loop {
                match receiver.recv().await {
                    Ok(msg) => {
                        // Dispatch to all handlers
                        for handler in &handlers {
                            handler.on_message(&msg).await;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(skipped)) => {
                        warn!(
                            "[{}] ⚠ Dispatcher lagged, skipped {} messages",
                            connection_name, skipped
                        );
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        info!(
                            "[{}] → Dispatcher stopping (channel closed)",
                            connection_name
                        );
                        break;
                    }
                }
            }
            Ok(())
        });

        self.receiver_handle = Some(handle);

        let task_count =
            self.background_handles.len() + if self.receiver_handle.is_some() { 1 } else { 0 };

        info!(
            "[{}] ✓ Started dispatcher with {} handlers ({} total tasks)",
            self.connection_name,
            self.handlers.len(),
            task_count
        );

        self
    }

    /// Stop all handlers and background tasks
    ///
    /// This will abort the receiver task and all background tasks.
    pub fn stop_all(&mut self) {
        info!(
            "[{}] Stopping dispatcher and {} background tasks",
            self.connection_name,
            self.background_handles.len() + if self.receiver_handle.is_some() { 1 } else { 0 }
        );

        if let Some(handle) = self.receiver_handle.take() {
            handle.abort();
        }

        for handle in &self.background_handles {
            handle.abort();
        }
    }

    /// Wait for all tasks to complete
    ///
    /// This will block until the dispatcher and all tasks have finished.
    /// Returns errors from any tasks that failed.
    pub async fn wait_all(self) -> Vec<Result<()>> {
        info!(
            "[{}] Waiting for dispatcher and {} background tasks",
            self.connection_name,
            self.background_handles.len() + if self.receiver_handle.is_some() { 1 } else { 0 }
        );

        let mut results = Vec::new();

        // Wait for receiver task
        if let Some(handle) = self.receiver_handle {
            match handle.await {
                Ok(result) => results.push(result),
                Err(e) if e.is_cancelled() => {
                    // Expected during shutdown
                }
                Err(e) => {
                    results.push(Err(anyhow::anyhow!("Dispatcher task panicked: {}", e)));
                }
            }
        }

        // Wait for background tasks
        for handle in self.background_handles {
            match handle.await {
                Ok(_) => {
                    // Background tasks return (), not Result
                }
                Err(e) if e.is_cancelled() => {
                    // Expected during shutdown
                }
                Err(e) => {
                    results.push(Err(anyhow::anyhow!("Background task panicked: {}", e)));
                }
            }
        }

        results
    }

    /// Get the number of handlers
    pub fn len(&self) -> usize {
        self.handlers.len()
    }

    /// Check if there are no handlers
    pub fn is_empty(&self) -> bool {
        self.handlers.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_default() {
        let manager = Manager::new();
        let builder = ConnectionHandlers::new(manager, "test".to_string());
        assert!(!builder.enable_error_handling);
        assert!(builder.read_timeout_seconds.is_none());
        assert_eq!(builder.config.channel_size, 32_768);
    }

    #[test]
    fn test_builder_with_config() {
        let manager = Manager::new();
        let config = HandlerConfig::new().with_channel_size(65_536);
        let builder = ConnectionHandlers::new(manager, "test".to_string()).with_config(config);
        assert_eq!(builder.config.channel_size, 65_536);
    }

    #[test]
    fn test_builder_with_error_handling() {
        let manager = Manager::new();
        let builder = ConnectionHandlers::new(manager, "test".to_string()).with_error_handling();
        assert!(builder.enable_error_handling);
    }

    #[test]
    fn test_builder_with_read_timeout() {
        let manager = Manager::new();
        let builder = ConnectionHandlers::new(manager, "test".to_string()).with_read_timeout(30);
        assert_eq!(builder.read_timeout_seconds, Some(30));
    }

    #[test]
    fn test_builder_composition() {
        let manager = Manager::new();
        let config = HandlerConfig::new().with_channel_size(100_000);

        let builder = ConnectionHandlers::new(manager, "test".to_string())
            .with_config(config)
            .with_error_handling()
            .with_read_timeout(30);

        assert_eq!(builder.config.channel_size, 100_000);
        assert!(builder.enable_error_handling);
        assert_eq!(builder.read_timeout_seconds, Some(30));
    }
}
