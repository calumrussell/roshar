//! Polymarket prediction-market public market data: Gamma metadata, CLOB order books and prices,
//! Data API market-level reads (REST) and the CLOB market websocket.

/// Wire and message types for this exchange (websocket messages, REST payloads).
pub mod types;

pub mod rest;
pub mod ws;

pub use rest::*;
pub use ws::{PolymarketMarketWs, POLYMARKET_MARKET_WSS_URL};
