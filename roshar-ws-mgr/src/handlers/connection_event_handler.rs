use crate::handlers::MessageHandler;
use async_trait::async_trait;
use crate::{Manager, Message};
use std::sync::Arc;

/// Handler for connection errors that triggers reconnection
pub struct ConnectionEventHandler {
    connection_name: String,
    manager: Arc<Manager>,
}

impl ConnectionEventHandler {
    pub fn new(manager: Arc<Manager>, connection_name: String) -> Self {
        Self {
            connection_name,
            manager,
        }
    }
}

#[async_trait]
impl MessageHandler for ConnectionEventHandler {
    async fn on_message(&self, message: &Message) {
        match message {
            Message::ReadError(_, _) | Message::WriteError(_, _) | Message::CloseMessage(_, _) | Message::PongReceiveTimeoutError(_) => {
                let mgr = self.manager.clone();
                let name = self.connection_name.clone();
                tokio::spawn(async move {
                    let _ = mgr.reconnect_with_close(&name, false).await;
                });
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_handler() -> ConnectionEventHandler {
        let manager = Manager::new();
        ConnectionEventHandler::new(manager, "test".to_string())
    }

    #[tokio::test]
    async fn test_on_message_handles_read_error() {
        let handler = create_test_handler();
        let msg = Message::ReadError("test".to_string(), "error".to_string());
        handler.on_message(&msg).await;
    }

    #[tokio::test]
    async fn test_on_message_handles_write_error() {
        let handler = create_test_handler();
        let msg = Message::WriteError("test".to_string(), "error".to_string());
        handler.on_message(&msg).await;
    }

    #[tokio::test]
    async fn test_on_message_handles_close_message() {
        let handler = create_test_handler();
        let msg = Message::CloseMessage("test".to_string(), Some("close".to_string()));
        handler.on_message(&msg).await;
    }

    #[tokio::test]
    async fn test_on_message_handles_pong_timeout() {
        let handler = create_test_handler();
        let msg = Message::PongReceiveTimeoutError("test".to_string());
        handler.on_message(&msg).await;
    }

    #[tokio::test]
    async fn test_on_message_ignores_other_messages() {
        let handler = create_test_handler();
        let msg = Message::SuccessfulHandshake("test".to_string());
        handler.on_message(&msg).await;
    }
}
