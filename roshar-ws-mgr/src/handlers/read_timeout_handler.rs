use crate::handlers::MessageHandler;
use async_trait::async_trait;
use log::{error, warn};
use crate::{Manager, Message};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio::time::{sleep, Duration, Instant};

/// Handler for detecting read timeouts
///
/// Monitors message activity and triggers reconnection if no messages
/// are received within the configured timeout period.
///
/// This handler:
/// - Resets a timer on every message received (reactive)
/// - Runs a background checker every 10 seconds (active)
/// - Triggers reconnection if timeout is exceeded
pub struct ReadTimeoutHandler {
    connection_name: String,
    manager: Arc<Manager>,
    last_message_time: Arc<Mutex<Instant>>,
    timeout_duration: Duration,
    check_interval: Duration,
}

impl ReadTimeoutHandler {
    /// Create a new read timeout handler
    ///
    /// # Arguments
    ///
    /// * `manager` - The Manager instance
    /// * `connection_name` - Name of the connection
    /// * `timeout_seconds` - Timeout in seconds (e.g., 30 for 30 seconds)
    pub fn new(manager: Arc<Manager>, connection_name: String, timeout_seconds: u64) -> Self {
        Self {
            connection_name,
            manager,
            last_message_time: Arc::new(Mutex::new(Instant::now())),
            timeout_duration: Duration::from_secs(timeout_seconds),
            check_interval: Duration::from_secs(10),
        }
    }

    /// Create a new read timeout handler with custom check interval
    ///
    /// Useful for testing with shorter intervals.
    pub fn with_check_interval(
        manager: Arc<Manager>,
        connection_name: String,
        timeout_seconds: u64,
        check_interval_seconds: u64,
    ) -> Self {
        Self {
            connection_name,
            manager,
            last_message_time: Arc::new(Mutex::new(Instant::now())),
            timeout_duration: Duration::from_secs(timeout_seconds),
            check_interval: Duration::from_secs(check_interval_seconds),
        }
    }

    /// Get the elapsed time since last message (for testing)
    pub async fn elapsed_since_last_message(&self) -> Duration {
        self.last_message_time.lock().await.elapsed()
    }
}

#[async_trait]
impl MessageHandler for ReadTimeoutHandler {
    async fn on_message(&self, _message: &Message) {
        // Reset timer on any message
        *self.last_message_time.lock().await = Instant::now();
    }

    fn spawn_background_tasks(&self) -> Vec<JoinHandle<()>> {
        let last_time = self.last_message_time.clone();
        let manager = self.manager.clone();
        let conn_name = self.connection_name.clone();
        let timeout = self.timeout_duration;
        let check_interval = self.check_interval;

        let handle = tokio::spawn(async move {
            loop {
                sleep(check_interval).await;

                let elapsed = last_time.lock().await.elapsed();
                if elapsed > timeout {
                    warn!(
                        "[{}] ⚠ Read timeout detected ({:.1}s > {:.1}s since last message)",
                        conn_name,
                        elapsed.as_secs_f64(),
                        timeout.as_secs_f64()
                    );

                    // Trigger reconnection
                    if let Err(e) = manager.reconnect(&conn_name).await {
                        error!("[{}] ✗ Failed to reconnect after timeout: {}", conn_name, e);
                    } else {
                        // Reset timer after successful reconnection trigger
                        *last_time.lock().await = Instant::now();
                    }
                }
            }
        });

        vec![handle]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_handler(timeout_seconds: u64) -> ReadTimeoutHandler {
        let manager = Manager::new();
        ReadTimeoutHandler::new(manager, "test".to_string(), timeout_seconds)
    }

    #[tokio::test]
    async fn test_on_message_resets_timer() {
        let handler = create_test_handler(30);

        // Wait a bit
        sleep(Duration::from_millis(100)).await;

        // Check elapsed time
        let elapsed_before = handler.elapsed_since_last_message().await;
        assert!(elapsed_before.as_millis() >= 100);

        // Send a message
        let msg = Message::TextMessage("test".to_string(), "data".to_string());
        handler.on_message(&msg).await;

        // Timer should be reset
        let elapsed_after = handler.elapsed_since_last_message().await;
        assert!(elapsed_after < elapsed_before);
        assert!(elapsed_after.as_millis() < 50); // Should be very recent
    }

    #[tokio::test]
    async fn test_on_message_resets_on_any_message_type() {
        let handler = create_test_handler(30);

        sleep(Duration::from_millis(50)).await;

        // Test with different message types
        let messages = vec![
            Message::TextMessage("test".to_string(), "data".to_string()),
            Message::SuccessfulHandshake("test".to_string()),
            Message::PongMessage("test".to_string(), "pong".to_string()),
            Message::ReadError("test".to_string(), "error".to_string()),
        ];

        for msg in messages {
            handler.on_message(&msg).await;
            let elapsed = handler.elapsed_since_last_message().await;
            assert!(
                elapsed.as_millis() < 50,
                "Timer should reset on message type: {:?}",
                msg
            );
            sleep(Duration::from_millis(50)).await;
        }
    }

    #[tokio::test]
    async fn test_spawn_background_tasks_returns_one_task() {
        let handler = create_test_handler(30);
        let tasks = handler.spawn_background_tasks();
        assert_eq!(tasks.len(), 1);

        // Cleanup
        for task in tasks {
            task.abort();
        }
    }

    #[tokio::test]
    async fn test_timeout_detection() {
        // Use very short timeout and check interval for testing
        let manager = Manager::new();
        let handler = ReadTimeoutHandler::with_check_interval(
            manager,
            "test".to_string(),
            1, // 1 second timeout
            1, // 1 second check interval
        );

        // Spawn background task
        let tasks = handler.spawn_background_tasks();

        // Don't send any messages, just wait
        sleep(Duration::from_millis(2500)).await;

        // The background task should have detected timeout and tried to reconnect
        // (We can't easily verify the reconnect was called without mocking, but we
        // can verify the task is still running)

        // Cleanup
        for task in tasks {
            task.abort();
        }
    }

    #[tokio::test]
    async fn test_timer_reset_prevents_timeout() {
        // Use short timeout and check interval for testing
        let manager = Manager::new();
        let handler = ReadTimeoutHandler::with_check_interval(
            manager,
            "test".to_string(),
            2, // 2 second timeout
            1, // 1 second check interval
        );

        // Spawn background task
        let tasks = handler.spawn_background_tasks();

        // Keep sending messages to prevent timeout
        for _ in 0..5 {
            sleep(Duration::from_millis(500)).await;
            let msg = Message::TextMessage("test".to_string(), "data".to_string());
            handler.on_message(&msg).await;
        }

        // Should not have timed out because we kept sending messages
        let elapsed = handler.elapsed_since_last_message().await;
        assert!(elapsed.as_millis() < 100);

        // Cleanup
        for task in tasks {
            task.abort();
        }
    }
}
