//! V2 REST market data (`GET /gateway/v2/*`, `GET /archive/v2/*`). Values are JSON decimals,
//! not x18 strings, except in `symbols`.

use super::gateway::NadoSymbol;
use super::num::I128;
use serde::Deserialize;
use std::collections::HashMap;

/// `GET /gateway/v2/assets`.
#[derive(Debug, Clone, Deserialize)]
pub struct NadoAsset {
    pub product_id: u32,
    /// `null` for assets without a market (USDT0).
    pub ticker_id: Option<String>,
    /// `spot` / `perp`; `null` for assets without a market.
    pub market_type: Option<String>,
    pub name: String,
    pub symbol: String,
    /// `null` in live replies for assets without a market.
    pub maker_fee: Option<f64>,
    /// `null` in live replies for assets without a market.
    pub taker_fee: Option<f64>,
    pub can_withdraw: bool,
    pub can_deposit: bool,
}

/// `GET /gateway/v2/pairs`.
#[derive(Debug, Clone, Deserialize)]
pub struct NadoPair {
    pub product_id: u32,
    pub ticker_id: String,
    pub base: String,
    pub quote: String,
}

/// `GET /gateway/v2/apr`.
#[derive(Debug, Clone, Deserialize)]
pub struct NadoApr {
    pub name: String,
    pub symbol: String,
    pub product_id: u32,
    pub deposit_apr: f64,
    pub borrow_apr: f64,
    pub tvl: f64,
}

/// `GET /gateway/v2/orderbook`. Levels are `[price, quantity]`.
#[derive(Debug, Clone, Deserialize)]
pub struct NadoOrderbook {
    pub product_id: u32,
    pub ticker_id: String,
    pub bids: Vec<[f64; 2]>,
    pub asks: Vec<[f64; 2]>,
    /// Milliseconds.
    pub timestamp: u64,
}

/// `GET /archive/v2/tickers` values (24h stats), keyed by ticker id.
#[derive(Debug, Clone, Deserialize)]
pub struct NadoTicker {
    pub product_id: u32,
    pub ticker_id: String,
    pub base_currency: String,
    pub quote_currency: String,
    pub last_price: f64,
    pub base_volume: f64,
    pub quote_volume: f64,
    pub price_change_percent_24h: f64,
}

pub type NadoTickers = HashMap<String, NadoTicker>;

/// `GET /archive/v2/contracts` values (perp summary), keyed by ticker id.
#[derive(Debug, Clone, Deserialize)]
pub struct NadoContract {
    pub product_id: u32,
    pub ticker_id: String,
    pub base_currency: String,
    pub quote_currency: String,
    pub last_price: f64,
    pub base_volume: f64,
    pub quote_volume: f64,
    pub product_type: String,
    /// Documented as a string; live replies send a number.
    pub contract_price: f64,
    pub contract_price_currency: String,
    pub open_interest: f64,
    pub open_interest_usd: f64,
    pub index_price: f64,
    pub mark_price: f64,
    /// Current 24h funding rate.
    pub funding_rate: f64,
    /// Unix seconds.
    pub next_funding_rate_timestamp: u64,
    pub price_change_percent_24h: f64,
    pub funding_interval_minutes: f64,
    pub long_open_interest: f64,
    pub long_open_interest_usd: f64,
    pub short_open_interest: f64,
    pub short_open_interest_usd: f64,
}

pub type NadoContractsSummary = HashMap<String, NadoContract>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NadoTradeSide {
    Buy,
    Sell,
}

/// `GET /archive/v2/trades`, newest first. Each match appears once per side.
#[derive(Debug, Clone, Deserialize)]
pub struct NadoTrade {
    pub product_id: u32,
    pub ticker_id: String,
    pub trade_id: u64,
    pub price: f64,
    pub base_filled: f64,
    pub quote_filled: f64,
    /// Unix seconds.
    pub timestamp: u64,
    pub trade_type: NadoTradeSide,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NadoMarketHours {
    pub is_open: bool,
    /// `weekend` / `holiday`; `null` while open.
    pub reason: Option<String>,
    /// ISO 8601 UTC.
    pub next_close: Option<String>,
    /// ISO 8601 UTC.
    pub next_open: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NadoPriceFeedLeg {
    pub primary: String,
    /// Omitted when the primary source is continuous.
    #[serde(default)]
    pub weekend: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NadoPriceFeed {
    pub oracle: NadoPriceFeedLeg,
    /// Perps only.
    #[serde(default)]
    pub spot_index: Option<NadoPriceFeedLeg>,
}

/// `GET /archive/v2/symbols` values, keyed by symbol: the gateway `symbols` fields plus market
/// hours, price feeds and reward boosts.
#[derive(Debug, Clone, Deserialize)]
pub struct NadoV2Symbol {
    #[serde(flatten)]
    pub symbol: NadoSymbol,
    /// `null` for 24/7 markets.
    pub market_hours: Option<NadoMarketHours>,
    /// xStocks spot products only.
    #[serde(default)]
    pub exchange_rate_x18: Option<I128>,
    /// Boosted products only.
    #[serde(default)]
    pub boost_type: Option<u32>,
    #[serde(default)]
    pub taker_multiplier: Option<u32>,
    #[serde(default)]
    pub maker_multiplier: Option<u32>,
    /// Documented as nullable; present in every live entry.
    #[serde(default)]
    pub price_feed: Option<NadoPriceFeed>,
}

pub type NadoV2Symbols = HashMap<String, NadoV2Symbol>;
