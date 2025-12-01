use roshar_types::Venue;
use rust_decimal::Decimal;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub struct PendingOrder {
    pub order_id: String,
    pub ticker: String,
    pub qty: f64, // Signed quantity
    pub px: f64,
    pub venue: Venue,
    pub raw_order: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Copy)]
pub enum OrderDirection {
    Buy,
    Sell,
}

#[allow(dead_code)]
pub enum StateMessage {
    OrderPlaced {
        venue: Venue,
        ticker: String,
        order_id: String,
        px: f64,
        qty: f64,
        direction: OrderDirection,
        raw_order: Option<serde_json::Value>,
    },
    OrderFilled {
        venue: Venue,
        ticker: String,
        order_id: String,
        filled_qty: f64,
        filled_px: f64,
        direction: OrderDirection, // Added: direction from fill message
    },
    OrderCancelled {
        venue: Venue,
        ticker: String,
        order_id: String,
    },
    GetPosition {
        ticker: String,
        reply: tokio::sync::oneshot::Sender<PositionState>,
    },
    GetAllPositions {
        reply: tokio::sync::oneshot::Sender<HashMap<String, f64>>,
    },
    GetPendingOrders {
        ticker: String,
        reply: tokio::sync::oneshot::Sender<Vec<PendingOrderInfo>>,
    },
    GetOrder {
        order_id: String,
        reply: tokio::sync::oneshot::Sender<Option<OrderStatus>>,
    },
    IsOrderCompleted {
        order_id: String,
        reply: tokio::sync::oneshot::Sender<bool>,
    },
    InitializePositions {
        positions: HashMap<String, f64>,
    },
}

#[derive(Debug, Clone)]
pub struct PositionState {
    pub actual: f64,
    pub pending: f64,
}

#[derive(Debug, Clone)]
pub struct PendingOrderInfo {
    pub order_id: String,
    pub ticker: String,
    pub qty: f64,
    pub px: f64,
    pub venue: Venue,
    pub raw_order: Option<serde_json::Value>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct OrderStatus {
    pub order_id: String,
    pub ticker: String,
    pub qty: f64,
    pub px: f64,
    pub venue: Venue,
    pub raw_order: Option<serde_json::Value>,
}

/// Generic state manager that tracks positions, pending orders, and completed orders
/// Updates only from WebSocket feeds (orders and fills)
pub struct StateManager {
    message_rx: tokio::sync::mpsc::Receiver<StateMessage>,
    positions: HashMap<String, Decimal>,
    pending_orders: HashMap<String, PendingOrder>,
    completed_orders: HashSet<String>, // Track order IDs that have been fully filled
}

impl StateManager {
    /// Spawn a new state manager with optional initialization
    /// Returns the handle and task join handle
    pub fn spawn_with_init<F, Fut>(
        init_fn: Option<F>,
    ) -> (
        crate::state_handle::StateHandle,
        tokio::task::JoinHandle<()>,
    )
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: std::future::Future<Output = Result<HashMap<String, f64>, String>> + Send + 'static,
    {
        let (query_tx, query_rx) = tokio::sync::mpsc::channel(100);
        let manager = Self::new(query_rx);
        let state_handle = crate::state_handle::StateHandle::new(query_tx);

        if let Some(init_fn) = init_fn {
            let handle_clone = state_handle.clone();
            tokio::spawn(async move {
                match init_fn().await {
                    Ok(positions) => {
                        if let Err(e) = handle_clone.initialize_positions(positions).await {
                            log::error!("Failed to initialize positions: {}", e);
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to fetch positions for initialization: {}", e);
                    }
                }
            });
        }

        let handle = tokio::spawn(async move {
            manager.run().await;
        });

        (state_handle, handle)
    }

    fn new(message_rx: tokio::sync::mpsc::Receiver<StateMessage>) -> Self {
        Self {
            message_rx,
            positions: HashMap::new(),
            pending_orders: HashMap::new(),
            completed_orders: HashSet::new(),
        }
    }

    pub async fn run(mut self) {
        loop {
            tokio::select! {
                Some(message) = self.message_rx.recv() => {
                    self.handle_message(message).await;
                }
                else => {
                    log::info!("StateManager channels closed, shutting down");
                    break;
                }
            }
        }
    }

    async fn handle_message(&mut self, message: StateMessage) {
        match message {
            StateMessage::GetPosition { ticker, reply } => {
                let actual = self
                    .positions
                    .get(&ticker)
                    .copied()
                    .unwrap_or(Decimal::ZERO);
                let pending: f64 = self
                    .pending_orders
                    .values()
                    .filter(|o| o.ticker == ticker)
                    .map(|o| o.qty)
                    .sum();

                let state = PositionState {
                    actual: actual.to_string().parse().unwrap_or(0.0),
                    pending,
                };

                let _ = reply.send(state);
            }
            StateMessage::OrderPlaced {
                venue,
                ticker,
                order_id,
                px,
                qty,
                direction,
                raw_order,
            } => {
                let signed_qty = match direction {
                    OrderDirection::Buy => qty,
                    OrderDirection::Sell => -qty,
                };

                let pending = PendingOrder {
                    order_id: order_id.clone(),
                    ticker: ticker.clone(),
                    qty: signed_qty,
                    px,
                    venue,
                    raw_order,
                };

                self.pending_orders.insert(order_id.clone(), pending);
            }
            StateMessage::OrderFilled {
                venue: _,
                ticker,
                order_id,
                filled_qty,
                filled_px,
                direction,
            } => {
                log::debug!(
                    "OrderFilled: order_id={}, ticker={}, filled_qty={}, filled_px={}, direction={:?}",
                    order_id,
                    ticker,
                    filled_qty,
                    filled_px,
                    direction
                );

                // Always use filled_qty from the fill message
                // Each fill message represents the actual filled quantity for that specific fill
                // Never use pending.qty because that's the total order size, not the fill size
                let signed_qty = match direction {
                    OrderDirection::Buy => filled_qty,
                    OrderDirection::Sell => -filled_qty,
                };

                // Update position
                let position_delta = Decimal::from_f64_retain(signed_qty).unwrap();
                *self
                    .positions
                    .entry(ticker.clone())
                    .or_insert(Decimal::ZERO) += position_delta;

                // Check if order is fully filled and remove from pending_orders
                // For partial fills, the order remains in pending_orders
                if let Some(pending_order) = self.pending_orders.get(&order_id) {
                    let remaining_qty = pending_order.qty.abs() - filled_qty;
                    log::debug!(
                        "Order {} in pending_orders: pending_qty={}, filled_qty={}, remaining_qty={}",
                        order_id,
                        pending_order.qty.abs(),
                        filled_qty,
                        remaining_qty
                    );
                    if remaining_qty.abs() < 0.0001 {
                        // Order is fully filled, remove it and mark as completed
                        log::info!(
                            "Order {} fully filled, removing from pending_orders",
                            order_id
                        );
                        self.pending_orders.remove(&order_id);
                        self.completed_orders.insert(order_id.clone());
                    } else {
                        log::debug!(
                            "Order {} partially filled, remaining in pending_orders (remaining: {})",
                            order_id,
                            remaining_qty
                        );
                    }
                } else {
                    log::debug!(
                        "Order {} not found in pending_orders (already completed or cancelled)",
                        order_id
                    );
                    // Order not in pending_orders - might have been cancelled or already completed
                    // Still update position and mark as completed if not already
                    if !self.completed_orders.contains(&order_id) {
                        self.completed_orders.insert(order_id.clone());
                    }
                }
            }
            StateMessage::OrderCancelled {
                venue: _,
                ticker: _,
                order_id,
            } => {
                if self.pending_orders.remove(&order_id).is_some() {
                    log::debug!(
                        "Order {} removed from pending_orders (cancelled/filled)",
                        order_id
                    );
                } else {
                    log::debug!("Order {} not in pending_orders (already removed)", order_id);
                }
            }
            StateMessage::GetAllPositions { reply } => {
                let positions: HashMap<String, f64> = self
                    .positions
                    .iter()
                    .filter(|(_, qty)| !qty.is_zero())
                    .map(|(ticker, qty)| (ticker.clone(), qty.to_string().parse().unwrap_or(0.0)))
                    .collect();
                let _ = reply.send(positions);
            }
            StateMessage::InitializePositions { positions } => {
                for (ticker, qty) in positions {
                    if let Some(decimal_qty) = Decimal::from_f64_retain(qty) {
                        self.positions.insert(ticker.clone(), decimal_qty);
                        log::info!("Initialized position: {} = {}", ticker, qty);
                    } else {
                        log::warn!("Failed to convert position qty for {}: {}", ticker, qty);
                    }
                }
                log::info!(
                    "Position initialization complete: {} positions loaded",
                    self.positions.len()
                );
            }
            StateMessage::GetPendingOrders { ticker, reply } => {
                let pending_orders: Vec<PendingOrderInfo> = self
                    .pending_orders
                    .values()
                    .filter(|o| o.ticker == ticker)
                    .map(|o| PendingOrderInfo {
                        order_id: o.order_id.clone(),
                        ticker: o.ticker.clone(),
                        qty: o.qty,
                        px: o.px,
                        venue: o.venue,
                        raw_order: o.raw_order.clone(),
                    })
                    .collect();
                log::debug!(
                    "GetPendingOrders for {}: found {} pending orders",
                    ticker,
                    pending_orders.len()
                );
                for order in &pending_orders {
                    log::debug!(
                        "  Pending order {}: qty={}, px={}, raw_order={:?}",
                        order.order_id,
                        order.qty,
                        order.px,
                        order.raw_order
                    );
                }
                let _ = reply.send(pending_orders);
            }
            StateMessage::GetOrder { order_id, reply } => {
                let status = self.pending_orders.get(&order_id).map(|o| OrderStatus {
                    order_id: o.order_id.clone(),
                    ticker: o.ticker.clone(),
                    qty: o.qty,
                    px: o.px,
                    venue: o.venue,
                    raw_order: o.raw_order.clone(),
                });
                let _ = reply.send(status);
            }
            StateMessage::IsOrderCompleted { order_id, reply } => {
                let is_completed = self.completed_orders.contains(&order_id);
                let _ = reply.send(is_completed);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use roshar_types::WsOrder;

    #[tokio::test]
    async fn test_order_placed_adds_pending_order() {
        let (tx, rx) = tokio::sync::mpsc::channel(10);
        let mut manager = StateManager::new(rx);

        tx.send(StateMessage::OrderPlaced {
            venue: Venue::Hyperliquid,
            ticker: "BTC".to_string(),
            order_id: "order123".to_string(),
            px: 50000.0,
            qty: 1.0,
            direction: OrderDirection::Buy,
            raw_order: None,
        })
        .await
        .unwrap();

        if let Some(msg) = manager.message_rx.recv().await {
            manager.handle_message(msg).await;
        }

        assert_eq!(manager.pending_orders.len(), 1);
        let pending = manager.pending_orders.get("order123").unwrap();
        assert_eq!(pending.ticker, "BTC");
        assert_eq!(pending.qty, 1.0);
        assert_eq!(pending.px, 50000.0);
    }

    #[tokio::test]
    async fn test_order_filled_updates_position() {
        let (tx, rx) = tokio::sync::mpsc::channel(10);
        let mut manager = StateManager::new(rx);

        tx.send(StateMessage::OrderPlaced {
            venue: Venue::Hyperliquid,
            ticker: "BTC".to_string(),
            order_id: "order123".to_string(),
            px: 50000.0,
            qty: 1.0,
            direction: OrderDirection::Buy,
            raw_order: None,
        })
        .await
        .unwrap();

        if let Some(msg) = manager.message_rx.recv().await {
            manager.handle_message(msg).await;
        }

        tx.send(StateMessage::OrderFilled {
            venue: Venue::Hyperliquid,
            ticker: "BTC".to_string(),
            order_id: "order123".to_string(),
            filled_qty: 1.0,
            filled_px: 50000.0,
            direction: OrderDirection::Buy,
        })
        .await
        .unwrap();

        if let Some(msg) = manager.message_rx.recv().await {
            manager.handle_message(msg).await;
        }

        let position = manager.positions.get("BTC").unwrap();
        assert_eq!(*position, Decimal::from_f64_retain(1.0).unwrap());
        assert_eq!(manager.pending_orders.len(), 0);
    }

    #[tokio::test]
    async fn test_order_cancelled_removes_pending() {
        let (tx, rx) = tokio::sync::mpsc::channel(10);
        let mut manager = StateManager::new(rx);

        tx.send(StateMessage::OrderPlaced {
            venue: Venue::Hyperliquid,
            ticker: "BTC".to_string(),
            order_id: "order789".to_string(),
            px: 50000.0,
            qty: 1.0,
            direction: OrderDirection::Buy,
            raw_order: None,
        })
        .await
        .unwrap();

        if let Some(msg) = manager.message_rx.recv().await {
            manager.handle_message(msg).await;
        }

        assert_eq!(manager.pending_orders.len(), 1);

        tx.send(StateMessage::OrderCancelled {
            venue: Venue::Hyperliquid,
            ticker: "BTC".to_string(),
            order_id: "order789".to_string(),
        })
        .await
        .unwrap();

        if let Some(msg) = manager.message_rx.recv().await {
            manager.handle_message(msg).await;
        }

        assert_eq!(manager.pending_orders.len(), 0);
        assert!(!manager.positions.contains_key("BTC"));
    }

    #[tokio::test]
    async fn test_get_order_retrieves_raw_ws_order() {
        let (tx, rx) = tokio::sync::mpsc::channel(10);
        let mut manager = StateManager::new(rx);

        // Create a WsOrder JSON structure (matching Hyperliquid format)
        let raw_order = serde_json::json!({
            "order": {
                "coin": "BTC",
                "side": "B",
                "limitPx": "50000.5",
                "sz": "1.0",
                "oid": 12345,
                "timestamp": 1234567890,
                "origSz": "1.0",
                "cloid": "cloid-123"
            },
            "status": "open",
            "statusTimestamp": 1234567890
        });

        // Place order with raw_order
        tx.send(StateMessage::OrderPlaced {
            venue: Venue::Hyperliquid,
            ticker: "BTC".to_string(),
            order_id: "12345".to_string(),
            px: 50000.5,
            qty: 1.0,
            direction: OrderDirection::Buy,
            raw_order: Some(raw_order.clone()),
        })
        .await
        .unwrap();

        if let Some(msg) = manager.message_rx.recv().await {
            manager.handle_message(msg).await;
        }

        // Retrieve order via GetOrder
        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
        tx.send(StateMessage::GetOrder {
            order_id: "12345".to_string(),
            reply: reply_tx,
        })
        .await
        .unwrap();

        if let Some(msg) = manager.message_rx.recv().await {
            manager.handle_message(msg).await;
        }

        let order_status = reply_rx.await.unwrap();
        assert!(order_status.is_some());
        let order_status = order_status.unwrap();

        // Verify basic fields
        assert_eq!(order_status.order_id, "12345");
        assert_eq!(order_status.ticker, "BTC");
        assert_eq!(order_status.qty, 1.0);
        assert_eq!(order_status.px, 50000.5);
        assert_eq!(order_status.venue, Venue::Hyperliquid);

        // Verify raw_order is present and can be deserialized
        assert!(order_status.raw_order.is_some());
        let deserialized: WsOrder =
            serde_json::from_value(order_status.raw_order.unwrap()).unwrap();
        assert_eq!(deserialized.order.coin, "BTC");
        assert_eq!(deserialized.order.side, "B");
        assert_eq!(deserialized.order.limit_px, "50000.5");
        assert_eq!(deserialized.order.sz, "1.0");
        assert_eq!(deserialized.order.oid, 12345);
        assert_eq!(deserialized.status, "open");
        assert_eq!(deserialized.order.cloid, Some("cloid-123".to_string()));
    }
}
