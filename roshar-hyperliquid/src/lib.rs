//! Hyperliquid: L1 action signing, websocket and REST order posting, user event streams, nonces,
//! `/info` queries, tick rounding and the market-data websocket config.
//!
//! Wire types (`Action`, `OrderReq`, `Tif`, `Cloid`, `OrderStatus`, ...) are defined in
//! `types::exchange` and re-exported here.
/// Wire and message types for this exchange (websocket messages, REST payloads).
pub mod types;

pub mod info;
pub mod nonce;
pub mod poster;
pub mod rest;
pub mod signer;
pub mod tick;
pub mod user_stream;
pub mod ws;

pub use crate::types::exchange::*;
pub use info::{perp_dex_of, Fill, Info, OpenOrder, HL_INFO_URL, HL_TESTNET_INFO_URL};
pub use nonce::NonceGen;
pub use poster::{
    decode_status, is_duplicate_rejection, now_ns, post_frame, spawn_fan_writer, FanJob,
    PendingPost, PostResult, WsPoster,
};
pub use rest::{RestPoster, HL_EXCHANGE_URL};
pub use signer::{float_to_wire, Signer};
pub use tick::to_tick;
pub use user_stream::{
    parse_cloid, parse_user_frame, OrderState, OrderUpdate, UserEvent, UserFill, UserStream,
};
#[cfg(feature = "ws-mgr")]
pub use ws::ws_config;
pub use ws::{HL_TESTNET_WSS_URL, HL_WSS_URL};
