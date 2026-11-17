//! Websocket transaction requests: `{"method": ..., "params": {"tx": "<base64>"}, "id": ..}`.
//!
//! Results arrive as `BulletWsMessage::OrderResult` / `OrderError` with the same `id`.

use serde_json::json;

use crate::signer::{SignedTransaction, UserAction};

/// The websocket method for a signed transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BulletTxMethod {
    /// `order.place`: `PlaceOrders` (including `replace`).
    OrderPlace,
    /// `order.cancel`: `CancelOrders`.
    OrderCancel,
    /// `order.amend`: `AmendOrders`.
    OrderAmend,
    /// `order.cancelAll`: `CancelMarketOrders`, `CancelAllOrders`.
    OrderCancelAll,
    /// `submit`: any other relayed call.
    Submit,
}

impl BulletTxMethod {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OrderPlace => "order.place",
            Self::OrderCancel => "order.cancel",
            Self::OrderAmend => "order.amend",
            Self::OrderCancelAll => "order.cancelAll",
            Self::Submit => "submit",
        }
    }

    /// The method the transaction-signing guide pairs with each call.
    pub fn for_action(action: &UserAction) -> Self {
        match action {
            UserAction::PlaceOrders { .. } => Self::OrderPlace,
            UserAction::CancelOrders { .. } => Self::OrderCancel,
            UserAction::AmendOrders { .. } => Self::OrderAmend,
            UserAction::CancelMarketOrders { .. } | UserAction::CancelAllOrders { .. } => {
                Self::OrderCancelAll
            }
            UserAction::CancelAndPlaceOrders { .. } | UserAction::UpdateMaxLeverage { .. } => {
                Self::Submit
            }
        }
    }
}

/// Builds the request text frame for `tx`.
pub fn tx_request(id: Option<u64>, method: BulletTxMethod, tx: &SignedTransaction) -> String {
    let mut msg = json!({
        "method": method.as_str(),
        "params": { "tx": tx.to_base64() },
    });
    if let Some(id) = id {
        msg["id"] = json!(id);
    }
    msg.to_string()
}
