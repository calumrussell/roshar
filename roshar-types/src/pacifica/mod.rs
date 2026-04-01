use serde::{Deserialize, Serialize};

// ── WebSocket messages ────────────────────────────────────────────────────────

/// Generic subscribe / unsubscribe wrapper.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PacificaWssMessage {
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<PacificaSubscribeParams>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PacificaSubscribeParams {
    pub channel: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub market: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interval: Option<String>,
}

impl PacificaWssMessage {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("Failed to serialize PacificaWssMessage")
    }

    pub fn ping() -> Self {
        Self {
            method: "ping".to_string(),
            params: None,
        }
    }

    pub fn depth(symbol: &str) -> Self {
        Self {
            method: "subscribe".to_string(),
            params: Some(PacificaSubscribeParams {
                channel: "orderbook".to_string(),
                market: Some(symbol.to_string()),
                interval: None,
            }),
        }
    }

    pub fn depth_unsub(symbol: &str) -> Self {
        Self {
            method: "unsubscribe".to_string(),
            params: Some(PacificaSubscribeParams {
                channel: "orderbook".to_string(),
                market: Some(symbol.to_string()),
                interval: None,
            }),
        }
    }

    pub fn trades(symbol: &str) -> Self {
        Self {
            method: "subscribe".to_string(),
            params: Some(PacificaSubscribeParams {
                channel: "trade".to_string(),
                market: Some(symbol.to_string()),
                interval: None,
            }),
        }
    }

    pub fn trades_unsub(symbol: &str) -> Self {
        Self {
            method: "unsubscribe".to_string(),
            params: Some(PacificaSubscribeParams {
                channel: "trade".to_string(),
                market: Some(symbol.to_string()),
                interval: None,
            }),
        }
    }

    pub fn candles(symbol: &str) -> Self {
        Self {
            method: "subscribe".to_string(),
            params: Some(PacificaSubscribeParams {
                channel: "candle".to_string(),
                market: Some(symbol.to_string()),
                interval: Some("1m".to_string()),
            }),
        }
    }

    pub fn mark_price(symbol: &str) -> Self {
        Self {
            method: "subscribe".to_string(),
            params: Some(PacificaSubscribeParams {
                channel: "ticker".to_string(),
                market: Some(symbol.to_string()),
                interval: None,
            }),
        }
    }
}

// ── REST response types ───────────────────────────────────────────────────────

/// Market metadata from `GET /api/v1/markets`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PacificaMarket {
    pub symbol: String,
    #[serde(rename = "baseAsset", default)]
    pub base_asset: String,
    #[serde(rename = "quoteAsset", default)]
    pub quote_asset: String,
    #[serde(default)]
    pub status: String,
    #[serde(rename = "minQty", default)]
    pub min_qty: String,
    #[serde(rename = "maxQty", default)]
    pub max_qty: String,
    #[serde(rename = "tickSize", default)]
    pub tick_size: String,
    #[serde(rename = "stepSize", default)]
    pub step_size: String,
    #[serde(rename = "maxLeverage", default)]
    pub max_leverage: String,
}

/// Ticker snapshot from `GET /api/v1/ticker`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PacificaTicker {
    pub symbol: String,
    #[serde(rename = "lastPrice", default)]
    pub last_price: String,
    #[serde(rename = "markPrice", default)]
    pub mark_price: String,
    #[serde(rename = "indexPrice", default)]
    pub index_price: String,
    #[serde(rename = "fundingRate", default)]
    pub funding_rate: String,
    #[serde(rename = "nextFundingTime", default)]
    pub next_funding_time: i64,
    #[serde(rename = "openInterest", default)]
    pub open_interest: String,
    #[serde(rename = "volume24h", default)]
    pub volume_24h: String,
    #[serde(rename = "priceChange24h", default)]
    pub price_change_24h: String,
    #[serde(rename = "highPrice24h", default)]
    pub high_price_24h: String,
    #[serde(rename = "lowPrice24h", default)]
    pub low_price_24h: String,
}

/// Order book snapshot from `GET /api/v1/orderbook`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PacificaOrderBook {
    pub bids: Vec<[String; 2]>,
    pub asks: Vec<[String; 2]>,
    #[serde(default)]
    pub timestamp: i64,
    #[serde(default)]
    pub symbol: String,
}

/// Single trade from `GET /api/v1/trades`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PacificaTrade {
    #[serde(default)]
    pub symbol: String,
    pub price: String,
    #[serde(alias = "size", alias = "quantity")]
    pub qty: String,
    pub side: String,
    #[serde(alias = "time", alias = "ts")]
    pub timestamp: i64,
    #[serde(rename = "tradeId", default)]
    pub trade_id: String,
}

/// Historical funding rate from `GET /api/v1/funding-rate`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PacificaFundingRate {
    pub symbol: String,
    #[serde(rename = "fundingRate")]
    pub funding_rate: String,
    #[serde(rename = "fundingTime")]
    pub funding_time: i64,
    #[serde(rename = "markPrice", default)]
    pub mark_price: String,
}

/// OHLCV kline from `GET /api/v1/klines`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PacificaKline {
    #[serde(alias = "openTime", alias = "ts", alias = "t")]
    pub open_time: i64,
    #[serde(alias = "o")]
    pub open: String,
    #[serde(alias = "h")]
    pub high: String,
    #[serde(alias = "l")]
    pub low: String,
    #[serde(alias = "c")]
    pub close: String,
    #[serde(alias = "v")]
    pub volume: String,
    #[serde(rename = "closeTime", default)]
    pub close_time: i64,
}
