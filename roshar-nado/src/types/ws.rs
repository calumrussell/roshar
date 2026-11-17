//! Subscriptions websocket (`/v1/subscribe`): stream selectors, request frames and events.

use super::num::{I128, U128, U64};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;

/// A stream selector, serialized as the `stream` object of subscribe / unsubscribe requests.
/// `None` product ids serialize as `null` (all products), where the stream allows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NadoStream {
    /// Requires an authenticated connection.
    OrderUpdate {
        subaccount: String,
        product_id: Option<u32>,
    },
    Trade {
        product_id: u32,
    },
    BestBidOffer {
        product_id: u32,
    },
    Fill {
        subaccount: String,
        product_id: Option<u32>,
    },
    PositionChange {
        subaccount: String,
        product_id: Option<u32>,
    },
    /// Incremental depth diffs, ~50 ms batches.
    BookDepth {
        product_id: u32,
    },
    Liquidation {
        product_id: Option<u32>,
    },
    /// `granularity` in seconds (see the candlesticks query for the allowed values).
    LatestCandlestick {
        product_id: u32,
        granularity: u32,
    },
    FundingPayment {
        product_id: u32,
    },
    FundingRate {
        product_id: Option<u32>,
    },
    /// Top of book for every product, ~300 ms snapshots.
    AllBbo,
}

fn with_id(mut v: Value, id: Option<u64>) -> String {
    if let Some(id) = id {
        v["id"] = json!(id);
    }
    v.to_string()
}

impl NadoStream {
    /// `{"method":"subscribe","stream":{..},"id"?}`.
    pub fn subscribe_message(&self, id: Option<u64>) -> String {
        with_id(json!({"method": "subscribe", "stream": self}), id)
    }

    /// `{"method":"unsubscribe","stream":{..},"id"?}`.
    pub fn unsubscribe_message(&self, id: Option<u64>) -> String {
        with_id(json!({"method": "unsubscribe", "stream": self}), id)
    }
}

/// `{"method":"subscribe_multi","streams":[..],"id"?}`. Not on the docs site; the frame is the
/// official SDKs'. On 2026-11-16 the server answered `multi subscriptions not enabled for this
/// IP: ..` (per-IP allowlist).
pub fn subscribe_multi_message(streams: &[NadoStream], id: Option<u64>) -> String {
    with_id(json!({"method": "subscribe_multi", "streams": streams}), id)
}

/// `{"method":"unsubscribe_multi","streams":[..],"id"?}`. Same allowlist as
/// [`subscribe_multi_message`].
pub fn unsubscribe_multi_message(streams: &[NadoStream], id: Option<u64>) -> String {
    with_id(
        json!({"method": "unsubscribe_multi", "streams": streams}),
        id,
    )
}

/// `{"method":"list","id"?}`: lists the connection's subscriptions.
pub fn list_message(id: Option<u64>) -> String {
    with_id(json!({"method": "list"}), id)
}

/// `{"method":"ping","id"?,"client_time"?}` control message (not the protocol keepalive).
pub fn ping_message(id: Option<u64>, client_time_ms: Option<u64>) -> String {
    let mut v = json!({"method": "ping"});
    if let Some(t) = client_time_ms {
        v["client_time"] = json!(t.to_string());
    }
    with_id(v, id)
}

/// `{"method":"time","id"?}` control message.
pub fn time_message(id: Option<u64>) -> String {
    with_id(json!({"method": "time"}), id)
}

/// Reply to a request frame: `{"result", "error"?, "id"?}`. `result` is `null` for
/// subscribe / unsubscribe / authenticate, an array for `list`, an object for `ping` / `time`.
#[derive(Debug, Clone, Deserialize)]
pub struct NadoWsResponse {
    pub result: Value,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub id: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NadoOrderUpdateReason {
    Placed,
    Filled,
    Cancelled,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NadoOrderUpdateEvent {
    /// Nanoseconds.
    pub timestamp: U64,
    pub product_id: u32,
    pub digest: String,
    /// Remaining unfilled amount, x18.
    pub amount: I128,
    pub reason: NadoOrderUpdateReason,
    /// Only with `reason: filled`.
    #[serde(default)]
    pub filled_qty: Option<I128>,
    /// Only with `reason: filled`.
    #[serde(default)]
    pub filled_price: Option<I128>,
    /// Only when the order carried a client id.
    #[serde(default)]
    pub id: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NadoTradeEvent {
    /// Nanoseconds.
    pub timestamp: U64,
    pub product_id: u32,
    pub price: I128,
    pub taker_qty: I128,
    pub maker_qty: I128,
    pub is_taker_buyer: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NadoBestBidOfferEvent {
    /// Nanoseconds.
    pub timestamp: U64,
    pub product_id: u32,
    /// `0` when there are no bids.
    pub bid_price: I128,
    pub bid_qty: I128,
    /// `i128::MAX` when there are no asks.
    pub ask_price: I128,
    pub ask_qty: I128,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NadoFillEvent {
    /// Nanoseconds.
    pub timestamp: U64,
    pub product_id: u32,
    pub subaccount: String,
    pub order_digest: String,
    pub appendix: U128,
    pub filled_qty: I128,
    pub remaining_qty: I128,
    pub original_qty: I128,
    pub price: I128,
    pub is_taker: bool,
    pub is_bid: bool,
    /// Negative for maker rebates.
    pub fee: I128,
    /// Documented as a number; live events send a string.
    pub submission_idx: U64,
    /// Only when the order carried a client id.
    #[serde(default)]
    pub id: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NadoPositionChangeEvent {
    /// Nanoseconds.
    pub timestamp: U64,
    pub product_id: u32,
    pub subaccount: String,
    pub isolated: bool,
    /// New absolute position, x18.
    pub amount: I128,
    pub v_quote_amount: I128,
    /// `match_orders`, `deposit_collateral`, `withdraw_collateral`, `transfer_quote`, ...
    pub reason: String,
}

/// Depth diff: `[price_x18, new_qty_x18]`, `0` removes the level.
#[derive(Debug, Clone, Deserialize)]
pub struct NadoBookDepthEvent {
    pub min_timestamp: U64,
    pub max_timestamp: U64,
    /// `max_timestamp` of the previous event for this product; a mismatch means a gap.
    pub last_max_timestamp: U64,
    pub product_id: u32,
    pub bids: Vec<[I128; 2]>,
    pub asks: Vec<[I128; 2]>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NadoLiquidationEvent {
    /// Nanoseconds.
    pub timestamp: U64,
    /// One id, or `[spot_id, perp_id]` for spread liquidations.
    pub product_ids: Vec<u32>,
    pub liquidator: String,
    pub liquidatee: String,
    pub amount: I128,
    pub price: I128,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NadoLatestCandlestickEvent {
    /// Candle start, unix seconds.
    pub timestamp: U64,
    pub product_id: u32,
    pub granularity: u32,
    pub open_x18: I128,
    pub high_x18: I128,
    pub low_x18: I128,
    pub close_x18: I128,
    pub volume: I128,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NadoFundingPaymentEvent {
    /// Nanoseconds.
    pub timestamp: U64,
    pub product_id: u32,
    pub payment_amount: I128,
    pub open_interest: I128,
    pub cumulative_funding_long_x18: I128,
    pub cumulative_funding_short_x18: I128,
    /// Nanoseconds.
    pub dt: U64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NadoFundingRateEvent {
    /// Nanoseconds.
    pub timestamp: U64,
    pub product_id: u32,
    /// 24h rate, x18.
    pub funding_rate_x18: I128,
    /// Unix seconds.
    pub update_time: U64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NadoBbo {
    pub bid: I128,
    pub ask: I128,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NadoAllBboEvent {
    /// Milliseconds.
    pub time: U64,
    /// Keyed by product id (as a string).
    pub bbos: HashMap<String, NadoBbo>,
}

/// A pushed stream event, discriminated by `type`.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NadoEvent {
    OrderUpdate(NadoOrderUpdateEvent),
    Trade(NadoTradeEvent),
    BestBidOffer(NadoBestBidOfferEvent),
    Fill(NadoFillEvent),
    PositionChange(NadoPositionChangeEvent),
    BookDepth(NadoBookDepthEvent),
    Liquidation(NadoLiquidationEvent),
    LatestCandlestick(NadoLatestCandlestickEvent),
    FundingPayment(NadoFundingPaymentEvent),
    FundingRate(NadoFundingRateEvent),
    AllBbo(NadoAllBboEvent),
}

/// Any frame received on the subscriptions socket.
#[derive(Debug, Clone)]
pub enum NadoWsMessage {
    Event(NadoEvent),
    Response(NadoWsResponse),
}

impl NadoWsMessage {
    /// Frames with a `type` field are events; all others are request replies.
    pub fn parse(text: &str) -> Result<Self, serde_json::Error> {
        let value: Value = serde_json::from_str(text)?;
        if value.get("type").is_some() {
            serde_json::from_value(value).map(Self::Event)
        } else {
            serde_json::from_value(value).map(Self::Response)
        }
    }
}
