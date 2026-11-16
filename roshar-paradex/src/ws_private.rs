//! Authenticated websocket (`PARADEX_WSS_URL`): the `auth` request, private channel
//! subscriptions and their JSON payloads, and order entry (`order.*` methods).
//!
//! Builders only; the caller owns the connection. Requests are JSON-RPC 2.0 text frames carrying
//! the caller's `id`, which the reply echoes. Authenticate once per connection with a JWT from
//! `ParadexExchange::auth`; the connection stays authenticated after the JWT expires.
//!
//! On a connection that negotiated SBE, `orders`, `fills`, `positions` and `account` updates may
//! arrive as SBE binary frames (see `crate::sbe`) or as JSON depending on the account;
//! `balance_events` is always JSON. `parse_private` reads the JSON form.
//!
//! Replies to requests (`auth`, `subscribe`, `order.*`) are JSON-RPC results or errors; see
//! `crate::ws_control`.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::exchange::{
    ModifyOrderRequest, OrderRequest, ParadexFill, ParadexOrder, ParadexPosition,
};

/// A private channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrivateChannel<'a> {
    /// `orders.{market}`: our order updates. `"ALL"` for every market.
    Orders(&'a str),
    /// `fills.{market}`: our fills. `"ALL"` for every market.
    Fills(&'a str),
    /// `positions`.
    Positions,
    /// `account`: margin and collateral summary.
    Account,
    /// `balance_events`: settlement balance changes with PnL and funding.
    BalanceEvents,
}

impl PrivateChannel<'_> {
    pub fn name(&self) -> String {
        match self {
            PrivateChannel::Orders(m) => format!("orders.{m}"),
            PrivateChannel::Fills(m) => format!("fills.{m}"),
            PrivateChannel::Positions => "positions".to_string(),
            PrivateChannel::Account => "account".to_string(),
            PrivateChannel::BalanceEvents => "balance_events".to_string(),
        }
    }
}

#[derive(Serialize)]
struct Rpc<'a, P: Serialize> {
    jsonrpc: &'static str,
    method: &'a str,
    params: P,
    id: u64,
}

fn rpc<P: Serialize>(id: u64, method: &str, params: P) -> String {
    serde_json::to_string(&Rpc {
        jsonrpc: "2.0",
        method,
        params,
        id,
    })
    .expect("JSON-RPC request serialises")
}

#[derive(Serialize)]
struct Bearer<'a> {
    bearer: &'a str,
}

#[derive(Serialize)]
struct Channel {
    channel: String,
}

#[derive(Serialize)]
struct CancelById<'a> {
    id: &'a str,
}

#[derive(Serialize)]
struct CancelByClientId<'a> {
    client_id: &'a str,
    market: &'a str,
}

#[derive(Serialize)]
struct CancelAll<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    market: Option<&'a str>,
}

/// `{"jsonrpc":"2.0","method":"auth","params":{"bearer":jwt},"id":id}`.
pub fn auth(id: u64, jwt: &str) -> String {
    rpc(id, "auth", Bearer { bearer: jwt })
}

pub fn subscribe(id: u64, channel: PrivateChannel<'_>) -> String {
    rpc(
        id,
        "subscribe",
        Channel {
            channel: channel.name(),
        },
    )
}

pub fn unsubscribe(id: u64, channel: PrivateChannel<'_>) -> String {
    rpc(
        id,
        "unsubscribe",
        Channel {
            channel: channel.name(),
        },
    )
}

/// `order.create` with the same signed body as `POST /orders`.
pub fn order_create(id: u64, order: &OrderRequest) -> String {
    rpc(id, "order.create", order)
}

/// `order.modify` with the same signed body as `PUT /orders/{id}`.
pub fn order_modify(id: u64, order: &ModifyOrderRequest) -> String {
    rpc(id, "order.modify", order)
}

/// `order.cancel` by Paradex order id.
pub fn order_cancel(id: u64, order_id: &str) -> String {
    rpc(id, "order.cancel", CancelById { id: order_id })
}

/// `order.cancel` by client order id; the market is required.
pub fn order_cancel_by_client_id(id: u64, client_id: &str, market: &str) -> String {
    rpc(id, "order.cancel", CancelByClientId { client_id, market })
}

/// `order.cancel_all`, optionally limited to one market.
pub fn order_cancel_all(id: u64, market: Option<&str>) -> String {
    rpc(id, "order.cancel_all", CancelAll { market })
}

// ── Channel payloads ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WithdrawalLockStatus {
    Pending,
    Active,
}

/// Settlement asset reserved for a portfolio margin withdrawal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParadexWithdrawalLock {
    pub amount: String,
    pub expires_at: i64,
    pub status: WithdrawalLockStatus,
    pub request_id: Option<String>,
}

/// The last refused withdrawal lock request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParadexWithdrawalLockRejection {
    pub amount: String,
    pub reason: String,
    pub rejected_at: i64,
    pub request_id: Option<String>,
}

/// `account` channel update.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParadexAccountUpdate {
    pub account: String,
    pub status: String,
    pub settlement_asset: String,
    pub account_value: String,
    pub total_collateral: String,
    /// Account value in excess of the initial margin requirement.
    pub free_collateral: String,
    pub initial_margin_requirement: String,
    pub maintenance_margin_requirement: String,
    pub margin_cushion: String,
    pub last_seen_notification: i64,
    pub seq_no: i64,
    pub updated_at: i64,
    /// Absent when there is none.
    pub withdrawal_lock: Option<ParadexWithdrawalLock>,
    /// Absent once five minutes old.
    pub withdrawal_lock_rejection: Option<ParadexWithdrawalLockRejection>,
}

/// `balance_events` channel update.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParadexBalanceEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    pub status: String,
    pub market: String,
    pub fill_id: String,
    pub fees: String,
    pub funding_index: String,
    pub realized_funding: String,
    pub realized_pnl: String,
    pub settlement_asset_address: String,
    pub settlement_asset_name: String,
    pub settlement_asset_price: String,
    pub settlement_asset_balance_before: String,
    pub settlement_asset_balance_after: String,
    pub created_at: i64,
}

/// A JSON private channel update.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrivateUpdate {
    Order(ParadexOrder),
    Fill(ParadexFill),
    Position(ParadexPosition),
    Account(ParadexAccountUpdate),
    BalanceEvent(ParadexBalanceEvent),
}

#[derive(Deserialize)]
struct Head {
    method: Option<String>,
    params: Option<HeadParams>,
}

#[derive(Deserialize)]
struct HeadParams {
    channel: String,
}

#[derive(Deserialize)]
struct Subscription<T> {
    params: SubscriptionParams<T>,
}

#[derive(Deserialize)]
struct SubscriptionParams<T> {
    data: T,
}

/// Parse a JSON text frame
/// `{"jsonrpc":"2.0","method":"subscription","params":{"channel":..,"data":{..}}}` from a private
/// channel. `Ok(None)` for frames that are not subscription updates (request replies) or come
/// from a public channel; `Err` when a private channel's payload does not match its schema.
pub fn parse_private(text: &str) -> Result<Option<PrivateUpdate>> {
    let head: Head =
        serde_json::from_str(text).with_context(|| format!("Paradex frame: {text}"))?;
    if head.method.as_deref() != Some("subscription") {
        return Ok(None);
    }
    let channel = match head.params {
        Some(p) => p.channel,
        None => return Ok(None),
    };
    let update = if channel.starts_with("orders.") {
        PrivateUpdate::Order(data(text, &channel)?)
    } else if channel.starts_with("fills.") {
        PrivateUpdate::Fill(data(text, &channel)?)
    } else if channel == "positions" {
        PrivateUpdate::Position(data(text, &channel)?)
    } else if channel == "account" {
        PrivateUpdate::Account(data(text, &channel)?)
    } else if channel == "balance_events" {
        PrivateUpdate::BalanceEvent(data(text, &channel)?)
    } else {
        return Ok(None);
    };
    Ok(Some(update))
}

fn data<T: serde::de::DeserializeOwned>(text: &str, channel: &str) -> Result<T> {
    let s: Subscription<T> =
        serde_json::from_str(text).with_context(|| format!("Paradex {channel} update: {text}"))?;
    Ok(s.params.data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exchange::OrderStatus;

    #[test]
    fn requests() {
        assert_eq!(
            auth(0, "jwt"),
            r#"{"jsonrpc":"2.0","method":"auth","params":{"bearer":"jwt"},"id":0}"#
        );
        assert_eq!(
            subscribe(3, PrivateChannel::Orders("ALL")),
            r#"{"jsonrpc":"2.0","method":"subscribe","params":{"channel":"orders.ALL"},"id":3}"#
        );
        assert_eq!(
            unsubscribe(4, PrivateChannel::Fills("BTC-USD-PERP")),
            r#"{"jsonrpc":"2.0","method":"unsubscribe","params":{"channel":"fills.BTC-USD-PERP"},"id":4}"#
        );
        assert_eq!(
            order_cancel_by_client_id(5, "c1", "ETH-USD-PERP"),
            r#"{"jsonrpc":"2.0","method":"order.cancel","params":{"client_id":"c1","market":"ETH-USD-PERP"},"id":5}"#
        );
        assert_eq!(
            order_cancel_all(6, None),
            r#"{"jsonrpc":"2.0","method":"order.cancel_all","params":{},"id":6}"#
        );
        assert_eq!(
            order_cancel(7, "123"),
            r#"{"jsonrpc":"2.0","method":"order.cancel","params":{"id":"123"},"id":7}"#
        );
    }

    #[test]
    fn parses_private_updates() {
        let order = r#"{"jsonrpc":"2.0","method":"subscription","params":{"channel":"orders.ALL","data":{"account":"0x1","avg_fill_price":"","cancel_reason":"","client_id":"c1","created_at":1,"flags":[],"id":"9","instruction":"POST_ONLY","last_updated_at":2,"market":"BTC-USD-PERP","price":"65000","published_at":3,"received_at":4,"remaining_size":"0.1","seq_no":5,"side":"SELL","size":"0.1","status":"OPEN","stp":"EXPIRE_TAKER","timestamp":6,"trigger_price":"","type":"LIMIT"}}}"#;
        match parse_private(order).unwrap() {
            Some(PrivateUpdate::Order(o)) => {
                assert_eq!(o.status, OrderStatus::Open);
                assert!(o.request_info.is_none());
            }
            other => panic!("{other:?}"),
        }
        let account = r#"{"jsonrpc":"2.0","method":"subscription","params":{"channel":"account","data":{"account":"0x1","account_value":"100","free_collateral":"90","initial_margin_requirement":"10","last_seen_notification":0,"maintenance_margin_requirement":"5","margin_cushion":"95","seq_no":1,"settlement_asset":"USDC","status":"ACTIVE","total_collateral":"100","updated_at":2}}}"#;
        assert!(matches!(
            parse_private(account).unwrap(),
            Some(PrivateUpdate::Account(_))
        ));
        let public = r#"{"jsonrpc":"2.0","method":"subscription","params":{"channel":"bbo.BTC-USD-PERP","data":{}}}"#;
        assert!(parse_private(public).unwrap().is_none());
        let reply = r#"{"jsonrpc":"2.0","result":{"node_id":"a1"},"usIn":1,"usDiff":2,"id":0}"#;
        assert!(parse_private(reply).unwrap().is_none());
        let broken = r#"{"jsonrpc":"2.0","method":"subscription","params":{"channel":"positions","data":{"id":"1"}}}"#;
        assert!(parse_private(broken).is_err());
    }
}
