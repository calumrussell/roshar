use serde::{Deserialize, Serialize};

// ── WebSocket messages ────────────────────────────────────────────────────────

/// JSON-RPC 2.0 subscribe / unsubscribe message for Paradex WebSocket.
/// Format: `{"id":1,"jsonrpc":"2.0","method":"subscribe","params":{"channel":"order_book.BTC-USD-PERP"}}`
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ParadexWssMessage {
    pub id: u32,
    pub jsonrpc: String,
    pub method: String,
    pub params: ParadexWssParams,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ParadexWssParams {
    pub channel: String,
}

impl ParadexWssMessage {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("Failed to serialize ParadexWssMessage")
    }

    fn new(method: &str, channel: String) -> Self {
        Self {
            id: 1,
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params: ParadexWssParams { channel },
        }
    }

    /// Paradex server pings every 55s; client must pong within 5s.
    /// The ws manager handles standard ping/pong frames, so we send a heartbeat
    /// subscribe as the client-side ping to keep the connection alive.
    pub fn ping() -> Self {
        Self::new("ping", String::new())
    }

    pub fn depth(market: &str) -> Self {
        Self::new("subscribe", format!("order_book.{market}"))
    }

    pub fn depth_unsub(market: &str) -> Self {
        Self::new("unsubscribe", format!("order_book.{market}"))
    }

    pub fn trades(market: &str) -> Self {
        Self::new("subscribe", format!("trades.{market}"))
    }

    pub fn trades_unsub(market: &str) -> Self {
        Self::new("unsubscribe", format!("trades.{market}"))
    }

    pub fn bbo(market: &str) -> Self {
        Self::new("subscribe", format!("bbo.{market}"))
    }

    pub fn funding(market: &str) -> Self {
        Self::new("subscribe", format!("funding_data.{market}"))
    }

    pub fn markets_summary() -> Self {
        Self::new("subscribe", "markets_summary".to_string())
    }
}

// ── REST response types ───────────────────────────────────────────────────────

/// Single market definition from `GET /v1/markets`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParadexMarket {
    pub symbol: String,
    #[serde(default)]
    pub base_currency: String,
    #[serde(default)]
    pub quote_currency: String,
    #[serde(default)]
    pub settlement_currency: String,
    #[serde(default)]
    pub expiry_at: i64,
    #[serde(default)]
    pub asset_kind: String,
    #[serde(default)]
    pub min_notional: String,
    #[serde(default)]
    pub tick_size: String,
    #[serde(default)]
    pub order_size_increment: String,
    #[serde(default)]
    pub max_funding_rate: String,
    #[serde(default)]
    pub max_open_orders: u32,
    #[serde(default)]
    pub max_order_size: String,
    #[serde(default)]
    pub delta_strike: String,
    #[serde(default)]
    pub position_limit: String,
}

/// Ticker snapshot from `GET /v1/markets/summary`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParadexMarketSummary {
    pub symbol: String,
    #[serde(rename = "bid", default)]
    pub bid: String,
    #[serde(rename = "ask", default)]
    pub ask: String,
    #[serde(rename = "last_traded_price", default)]
    pub last_traded_price: String,
    #[serde(rename = "mark_price", default)]
    pub mark_price: String,
    #[serde(rename = "index_price", default)]
    pub index_price: String,
    #[serde(rename = "funding_rate", default)]
    pub funding_rate: String,
    #[serde(rename = "future_funding_rate", default)]
    pub future_funding_rate: String,
    #[serde(rename = "total_volume", default)]
    pub total_volume: String,
    #[serde(rename = "volume_24h", default)]
    pub volume_24h: String,
    #[serde(rename = "open_interest", default)]
    pub open_interest: String,
    #[serde(rename = "price_change_rate_24h", default)]
    pub price_change_rate_24h: String,
    #[serde(rename = "created_at", default)]
    pub created_at: i64,
    #[serde(rename = "underlying_price", default)]
    pub underlying_price: String,
}

/// Order book snapshot from `GET /v1/orderbook/{market}`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParadexOrderBook {
    pub bids: Vec<[String; 2]>,
    pub asks: Vec<[String; 2]>,
    #[serde(default)]
    pub seq_no: u64,
    #[serde(default)]
    pub last_updated_at: i64,
}

/// Single trade from `GET /v1/markets/{market}/trades`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParadexTrade {
    pub id: String,
    pub symbol: String,
    pub price: String,
    pub size: String,
    pub side: String,
    #[serde(rename = "trade_type", default)]
    pub trade_type: String,
    #[serde(rename = "created_at")]
    pub created_at: i64,
}

/// OHLCV kline from `GET /v1/markets/{market}/klines`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParadexKline {
    /// Unix timestamp in seconds.
    pub ts: i64,
    pub open: String,
    pub high: String,
    pub low: String,
    pub close: String,
    pub volume: String,
}

/// Historical funding rate from `GET /v1/markets/{market}/funding`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParadexFundingEntry {
    pub symbol: String,
    pub funding_index: String,
    pub funding_premium: String,
    pub funding_rate: String,
    pub created_at: i64,
}

/// Paginated list wrapper used by Paradex REST responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParadexListResponse<T> {
    pub results: Vec<T>,
    #[serde(default)]
    pub next: Option<String>,
    #[serde(default)]
    pub prev: Option<String>,
}
