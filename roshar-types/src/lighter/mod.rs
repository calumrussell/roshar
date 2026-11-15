use serde::{Deserialize, Serialize};

// ── WebSocket messages ────────────────────────────────────────────────────────

/// Subscribe / unsubscribe message for Lighter WebSocket streams.
/// Format: `{"type": "subscribe", "channel": "order_book/0"}`
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct LighterWssMessage {
    #[serde(rename = "type")]
    pub msg_type: String,
    pub channel: String,
}

impl LighterWssMessage {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("Failed to serialize LighterWssMessage")
    }

    /// Lighter requires any frame within 2 minutes; use a subscribe ping pattern.
    pub fn ping() -> Self {
        Self {
            msg_type: "ping".to_string(),
            channel: String::new(),
        }
    }

    pub fn depth(market_index: u32) -> Self {
        Self {
            msg_type: "subscribe".to_string(),
            channel: format!("order_book/{market_index}"),
        }
    }

    pub fn depth_unsub(market_index: u32) -> Self {
        Self {
            msg_type: "unsubscribe".to_string(),
            channel: format!("order_book/{market_index}"),
        }
    }

    pub fn trades(market_index: u32) -> Self {
        Self {
            msg_type: "subscribe".to_string(),
            channel: format!("trade/{market_index}"),
        }
    }

    pub fn trades_unsub(market_index: u32) -> Self {
        Self {
            msg_type: "unsubscribe".to_string(),
            channel: format!("trade/{market_index}"),
        }
    }

    pub fn market_stats(market_index: u32) -> Self {
        Self {
            msg_type: "subscribe".to_string(),
            channel: format!("market_stats/{market_index}"),
        }
    }

    pub fn all_market_stats() -> Self {
        Self {
            msg_type: "subscribe".to_string(),
            channel: "market_stats/all".to_string(),
        }
    }

    pub fn ticker(market_index: u32) -> Self {
        Self {
            msg_type: "subscribe".to_string(),
            channel: format!("ticker/{market_index}"),
        }
    }
}

// ── REST response types ───────────────────────────────────────────────────────

/// Market metadata from `GET /api/v1/orderBooks`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LighterMarket {
    pub market_id: u32,
    pub name: String,
    #[serde(default)]
    pub maker_fee: String,
    #[serde(default)]
    pub taker_fee: String,
    #[serde(default)]
    pub min_order_size: String,
    #[serde(default)]
    pub min_order_size_increment: String,
    #[serde(default)]
    pub min_price_increment: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub contract_type: String,
}

/// Market detail from `GET /api/v1/orderBookDetails` (`order_book_details[]`). Field names and
/// JSON types as the endpoint sends them (checked against mainnet 2026-11-15); numbers the API
/// sends as JSON numbers are `f64`, numbers it sends as strings stay `String`. Fields not listed
/// here are ignored.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LighterMarketDetail {
    pub market_id: u32,
    pub symbol: String,
    pub market_type: String,
    pub status: String,
    pub size_decimals: u32,
    pub price_decimals: u32,
    pub min_base_amount: String,
    pub min_quote_amount: String,
    pub maker_fee: String,
    pub taker_fee: String,
    pub mark_price: String,
    pub index_price: String,
    pub last_trade_price: f64,
    pub daily_base_token_volume: f64,
    pub daily_quote_token_volume: f64,
    pub daily_price_change: f64,
    pub daily_price_high: f64,
    pub daily_price_low: f64,
    pub open_interest: f64,
    pub open_interest_usd: String,
}

/// Order book snapshot from `GET /api/v1/orderBookOrders`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LighterOrderBook {
    pub asks: Vec<LighterLevel>,
    pub bids: Vec<LighterLevel>,
}

/// Single price level in the order book.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LighterLevel {
    pub price: String,
    pub size: String,
}

/// Single trade from `GET /api/v1/recentTrades`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LighterTrade {
    pub price: String,
    pub size: String,
    pub side: String,
    #[serde(alias = "timestamp", alias = "created_at")]
    pub timestamp: i64,
    #[serde(default)]
    pub market_id: u32,
}

/// Funding rate entry from `GET /api/v1/funding-rates`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LighterFundingRate {
    pub market_id: u32,
    #[serde(default)]
    pub name: String,
    pub funding_rate: String,
    #[serde(default)]
    pub funding_period: String,
    #[serde(default)]
    pub binance_funding_rate: String,
    #[serde(default)]
    pub bybit_funding_rate: String,
    #[serde(default)]
    pub hyperliquid_funding_rate: String,
}

/// Wrapper returned by the `/orderBooks` endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LighterMarketsResponse {
    pub order_books: Vec<LighterMarket>,
}

/// Wrapper returned by the `/orderBookDetails` endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LighterMarketDetailsResponse {
    pub order_book_details: Vec<LighterMarketDetail>,
}

/// Wrapper returned by the `/funding-rates` endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LighterFundingRatesResponse {
    pub funding_rates: Vec<LighterFundingRate>,
}
