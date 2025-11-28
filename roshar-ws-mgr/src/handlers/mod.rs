pub mod connection_event_handler;
pub mod exchange_subscription_manager;
pub mod read_timeout_handler;
mod connection_handlers;
mod config;

use async_trait::async_trait;
use crate::Message;
use tokio::task::JoinHandle;

pub use connection_event_handler::ConnectionEventHandler;
pub use exchange_subscription_manager::ExchangeSubscriptionManager;
pub use read_timeout_handler::ReadTimeoutHandler;
pub use connection_handlers::ConnectionHandlers;
pub use config::HandlerConfig;

/// Trait for handlers that process WebSocket messages
///
/// Handlers implement reactive logic in response to messages.
/// For handlers that need background tasks (e.g., periodic checks),
/// implement spawn_background_tasks().
#[async_trait]
pub trait MessageHandler: Send + Sync {
    /// Process a message
    ///
    /// Called by the dispatcher for each message received.
    /// Should be fast and non-blocking where possible.
    async fn on_message(&self, message: &Message);

    /// Spawn any background tasks needed by this handler
    ///
    /// Returns handles to tasks that should run alongside message processing.
    /// Default implementation returns no tasks.
    fn spawn_background_tasks(&self) -> Vec<JoinHandle<()>> {
        Vec::new()
    }
}
