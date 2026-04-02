use serde::{Deserialize, Serialize};

// ── WebSocket messages ────────────────────────────────────────────────────────

/// Subscribe / unsubscribe message for Aster WebSocket streams.
/// Aster mirrors the Binance Futures WebSocket protocol exactly.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AsterWssMessage {
    pub id: u32,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Vec<String>>,
}

impl AsterWssMessage {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("Failed to serialize AsterWssMessage")
    }

    pub fn ping() -> Self {
        Self {
            id: 1,
            method: "ping".to_string(),
            params: None,
        }
    }

    pub fn depth(symbol: &str) -> Self {
        Self {
            id: 1,
            method: "SUBSCRIBE".to_string(),
            params: Some(vec![format!("{}@depth@100ms", symbol.to_lowercase())]),
        }
    }

    pub fn depth_unsub(symbol: &str) -> Self {
        Self {
            id: 1,
            method: "UNSUBSCRIBE".to_string(),
            params: Some(vec![format!("{}@depth@100ms", symbol.to_lowercase())]),
        }
    }

    pub fn trades(symbol: &str) -> Self {
        Self {
            id: 1,
            method: "SUBSCRIBE".to_string(),
            params: Some(vec![format!("{}@aggTrade", symbol.to_lowercase())]),
        }
    }

    pub fn trades_unsub(symbol: &str) -> Self {
        Self {
            id: 1,
            method: "UNSUBSCRIBE".to_string(),
            params: Some(vec![format!("{}@aggTrade", symbol.to_lowercase())]),
        }
    }

    pub fn mark_price(symbol: &str) -> Self {
        Self {
            id: 1,
            method: "SUBSCRIBE".to_string(),
            params: Some(vec![format!("{}@markPrice", symbol.to_lowercase())]),
        }
    }

    pub fn klines(symbol: &str) -> Self {
        Self {
            id: 1,
            method: "SUBSCRIBE".to_string(),
            params: Some(vec![format!("{}@kline_1m", symbol.to_lowercase())]),
        }
    }
}

// ── REST response types ───────────────────────────────────────────────────────

/// Mark price and funding rate for a single symbol (`GET /fapi/v1/markPrice`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsterMarkPrice {
    pub symbol: String,
    #[serde(rename = "markPrice")]
    pub mark_price: String,
    #[serde(rename = "indexPrice")]
    pub index_price: String,
    #[serde(rename = "lastFundingRate")]
    pub last_funding_rate: String,
    #[serde(rename = "nextFundingTime")]
    pub next_funding_time: i64,
    pub time: i64,
}

/// Historical funding rate entry (`GET /fapi/v1/fundingRate`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsterFundingRate {
    pub symbol: String,
    #[serde(rename = "fundingRate")]
    pub funding_rate: String,
    #[serde(rename = "fundingTime")]
    pub funding_time: i64,
    #[serde(rename = "markPrice", default)]
    pub mark_price: String,
}

/// 24-hour ticker statistics (`GET /fapi/v1/ticker/24hr`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsterTicker24hr {
    pub symbol: String,
    #[serde(rename = "priceChange")]
    pub price_change: String,
    #[serde(rename = "priceChangePercent")]
    pub price_change_percent: String,
    #[serde(rename = "weightedAvgPrice")]
    pub weighted_avg_price: String,
    #[serde(rename = "lastPrice")]
    pub last_price: String,
    pub volume: String,
    #[serde(rename = "quoteVolume")]
    pub quote_volume: String,
    #[serde(rename = "openTime")]
    pub open_time: i64,
    #[serde(rename = "closeTime")]
    pub close_time: i64,
    #[serde(rename = "openPrice")]
    pub open_price: String,
    #[serde(rename = "highPrice")]
    pub high_price: String,
    #[serde(rename = "lowPrice")]
    pub low_price: String,
}

/// OHLCV kline entry.
/// Aster returns klines as arrays: [open_time, open, high, low, close, volume,
/// close_time, quote_volume, num_trades, taker_buy_base, taker_buy_quote, ignore]
#[derive(Debug, Clone, Serialize)]
pub struct AsterKline {
    pub open_time: i64,
    pub open: String,
    pub high: String,
    pub low: String,
    pub close: String,
    pub volume: String,
    pub close_time: i64,
    pub quote_volume: String,
    pub num_trades: i64,
}

impl<'de> Deserialize<'de> for AsterKline {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let arr: serde_json::Value = Deserialize::deserialize(deserializer)?;
        let a = arr.as_array().ok_or_else(|| serde::de::Error::custom("expected array"))?;
        Ok(Self {
            open_time: a[0].as_i64().unwrap_or(0),
            open: a[1].as_str().unwrap_or("").to_string(),
            high: a[2].as_str().unwrap_or("").to_string(),
            low: a[3].as_str().unwrap_or("").to_string(),
            close: a[4].as_str().unwrap_or("").to_string(),
            volume: a[5].as_str().unwrap_or("").to_string(),
            close_time: a[6].as_i64().unwrap_or(0),
            quote_volume: a[7].as_str().unwrap_or("").to_string(),
            num_trades: a[8].as_i64().unwrap_or(0),
        })
    }
}

/// Order book snapshot from `GET /fapi/v1/depth`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsterOrderBook {
    #[serde(rename = "lastUpdateId")]
    pub last_update_id: u64,
    #[serde(rename = "T", default)]
    pub transaction_time: u64,
    #[serde(rename = "E", default)]
    pub event_time: u64,
    pub bids: Vec<[String; 2]>,
    pub asks: Vec<[String; 2]>,
}

// ── WebSocket stream message types ───────────────────────────────────────────

/// Diff depth update stream (`{symbol}@depth@100ms`).
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AsterDepthMessage {
    #[serde(rename = "e")]
    pub event_type: String,
    #[serde(rename = "E")]
    pub event_time: u64,
    #[serde(rename = "s")]
    pub symbol: String,
    #[serde(rename = "U")]
    pub first_update_id: u64,
    #[serde(rename = "u")]
    pub final_update_id: u64,
    #[serde(rename = "b")]
    pub bids: Vec<[String; 2]>,
    #[serde(rename = "a")]
    pub asks: Vec<[String; 2]>,
    #[serde(rename = "pu")]
    pub previous_final_update_id: u64,
}

/// Aggregate trade stream (`{symbol}@aggTrade`).
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AsterAggTradeMessage {
    #[serde(rename = "e")]
    pub event_type: String,
    #[serde(rename = "E")]
    pub event_time: u64,
    #[serde(rename = "s")]
    pub symbol: String,
    #[serde(rename = "a")]
    pub agg_trade_id: u64,
    #[serde(rename = "p")]
    pub price: String,
    #[serde(rename = "q")]
    pub qty: String,
    #[serde(rename = "T")]
    pub trade_time: u64,
    #[serde(rename = "m")]
    pub is_buyer_maker: bool,
}

/// Mark price stream (`{symbol}@markPrice`).
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AsterMarkPriceMessage {
    #[serde(rename = "e")]
    pub event_type: String,
    #[serde(rename = "E")]
    pub event_time: u64,
    #[serde(rename = "s")]
    pub symbol: String,
    #[serde(rename = "p")]
    pub mark_price: String,
    #[serde(rename = "i")]
    pub index_price: String,
    #[serde(rename = "r")]
    pub funding_rate: String,
    #[serde(rename = "T")]
    pub next_funding_time: u64,
}
