use crate::state_manager::StateMessage;
use std::collections::HashMap;
use tokio::sync::mpsc;

/// Handle for querying position state
/// Wraps the oneshot channel communication with the state manager
#[derive(Clone)]
pub struct StateHandle {
    state_query_tx: mpsc::Sender<StateMessage>,
}

impl StateHandle {
    pub fn new(state_query_tx: mpsc::Sender<StateMessage>) -> Self {
        Self { state_query_tx }
    }

    /// Get a clone of the underlying sender for direct message passing (e.g., WebSocket feeds)
    pub fn sender(&self) -> mpsc::Sender<StateMessage> {
        self.state_query_tx.clone()
    }

    /// Get all positions from state manager
    pub async fn get_positions(&self) -> Result<HashMap<String, f64>, String> {
        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
        self.state_query_tx
            .send(StateMessage::GetAllPositions { reply: reply_tx })
            .await
            .map_err(|e| format!("Failed to query positions: {}", e))?;
        reply_rx
            .await
            .map_err(|e| format!("Failed to receive positions: {}", e))
    }

    /// Get position for a specific ticker
    pub async fn get_position(
        &self,
        ticker: &str,
    ) -> Result<crate::state_manager::PositionState, String> {
        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
        self.state_query_tx
            .send(StateMessage::GetPosition {
                ticker: ticker.to_string(),
                reply: reply_tx,
            })
            .await
            .map_err(|e| format!("Failed to query position for {}: {}", ticker, e))?;
        reply_rx
            .await
            .map_err(|e| format!("Failed to receive position for {}: {}", ticker, e))
    }

    /// Get pending orders for a specific ticker
    pub async fn get_pending_orders(
        &self,
        ticker: &str,
    ) -> Result<Vec<crate::state_manager::PendingOrderInfo>, String> {
        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
        self.state_query_tx
            .send(StateMessage::GetPendingOrders {
                ticker: ticker.to_string(),
                reply: reply_tx,
            })
            .await
            .map_err(|e| format!("Failed to query pending orders for {}: {}", ticker, e))?;
        reply_rx
            .await
            .map_err(|e| format!("Failed to receive pending orders for {}: {}", ticker, e))
    }

    /// Get order status by order_id
    pub async fn get_order(
        &self,
        order_id: &str,
    ) -> Result<Option<crate::state_manager::OrderStatus>, String> {
        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
        self.state_query_tx
            .send(StateMessage::GetOrder {
                order_id: order_id.to_string(),
                reply: reply_tx,
            })
            .await
            .map_err(|e| format!("Failed to query order {}: {}", order_id, e))?;
        reply_rx
            .await
            .map_err(|e| format!("Failed to receive order status for {}: {}", order_id, e))
    }

    /// Check if an order is completed (fully filled)
    pub async fn is_order_completed(&self, order_id: &str) -> Result<bool, String> {
        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
        self.state_query_tx
            .send(StateMessage::IsOrderCompleted {
                order_id: order_id.to_string(),
                reply: reply_tx,
            })
            .await
            .map_err(|e| format!("Failed to query order completion for {}: {}", order_id, e))?;
        reply_rx.await.map_err(|e| {
            format!(
                "Failed to receive order completion status for {}: {}",
                order_id, e
            )
        })
    }

    /// Initialize positions from a HashMap
    pub async fn initialize_positions(
        &self,
        positions: HashMap<String, f64>,
    ) -> Result<(), String> {
        self.state_query_tx
            .send(StateMessage::InitializePositions { positions })
            .await
            .map_err(|e| format!("Failed to send initialize positions message: {}", e))
    }

    /// Get all pending orders across all tickers
    pub async fn get_all_pending_orders(
        &self,
    ) -> Result<Vec<crate::state_manager::PendingOrderInfo>, String> {
        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
        self.state_query_tx
            .send(StateMessage::GetAllPendingOrders { reply: reply_tx })
            .await
            .map_err(|e| format!("Failed to query all pending orders: {}", e))?;
        reply_rx
            .await
            .map_err(|e| format!("Failed to receive all pending orders: {}", e))
    }
}
