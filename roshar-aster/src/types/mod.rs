use serde::{Deserialize, Serialize};

// ── WebSocket messages ────────────────────────────────────────────────────────

/// Kline intervals of the `<symbol>@kline_<interval>` stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsterKlineInterval {
    M1,
    M3,
    M5,
    M15,
    M30,
    H1,
    H2,
    H4,
    H6,
    H8,
    H12,
    D1,
    D3,
    W1,
    /// One month (`1M`).
    Mo1,
}

impl AsterKlineInterval {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::M1 => "1m",
            Self::M3 => "3m",
            Self::M5 => "5m",
            Self::M15 => "15m",
            Self::M30 => "30m",
            Self::H1 => "1h",
            Self::H2 => "2h",
            Self::H4 => "4h",
            Self::H6 => "6h",
            Self::H8 => "8h",
            Self::H12 => "12h",
            Self::D1 => "1d",
            Self::D3 => "3d",
            Self::W1 => "1w",
            Self::Mo1 => "1M",
        }
    }
}

/// Update speed of the diff-depth stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsterDepthSpeed {
    /// `<symbol>@depth` (250ms).
    Default,
    /// `<symbol>@depth@500ms`.
    Ms500,
    /// `<symbol>@depth@100ms`.
    Ms100,
}

impl AsterDepthSpeed {
    fn suffix(&self) -> &'static str {
        match self {
            Self::Default => "",
            Self::Ms500 => "@500ms",
            Self::Ms100 => "@100ms",
        }
    }
}

fn depth_stream(symbol: &str, speed: AsterDepthSpeed) -> String {
    format!("{}@depth{}", symbol.to_lowercase(), speed.suffix())
}

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

    pub fn depth(symbol: &str, speed: AsterDepthSpeed) -> Self {
        Self {
            id: 1,
            method: "SUBSCRIBE".to_string(),
            params: Some(vec![depth_stream(symbol, speed)]),
        }
    }

    pub fn depth_unsub(symbol: &str, speed: AsterDepthSpeed) -> Self {
        Self {
            id: 1,
            method: "UNSUBSCRIBE".to_string(),
            params: Some(vec![depth_stream(symbol, speed)]),
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

    pub fn klines(symbol: &str, interval: AsterKlineInterval) -> Self {
        Self {
            id: 1,
            method: "SUBSCRIBE".to_string(),
            params: Some(vec![format!(
                "{}@kline_{}",
                symbol.to_lowercase(),
                interval.as_str()
            )]),
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
        use serde::de::Error;

        let arr: serde_json::Value = Deserialize::deserialize(deserializer)?;
        let a = arr
            .as_array()
            .ok_or_else(|| D::Error::custom("AsterKline: expected array"))?;
        let int = |i: usize| -> Result<i64, D::Error> {
            a.get(i)
                .ok_or_else(|| D::Error::custom(format!("AsterKline: missing index {i}")))?
                .as_i64()
                .ok_or_else(|| D::Error::custom(format!("AsterKline: index {i} is not an integer")))
        };
        let string = |i: usize| -> Result<String, D::Error> {
            a.get(i)
                .ok_or_else(|| D::Error::custom(format!("AsterKline: missing index {i}")))?
                .as_str()
                .map(str::to_string)
                .ok_or_else(|| D::Error::custom(format!("AsterKline: index {i} is not a string")))
        };
        Ok(Self {
            open_time: int(0)?,
            open: string(1)?,
            high: string(2)?,
            low: string(3)?,
            close: string(4)?,
            volume: string(5)?,
            close_time: int(6)?,
            quote_volume: string(7)?,
            num_trades: int(8)?,
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

/// Diff depth update stream (`{symbol}@depth[@500ms|@100ms]`).
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kline_parses_wire_row() {
        let row = r#"[1499040000000,"0.016","0.8","0.015","0.0158","148976.1",1499644799999,"2434.2",308,"1756.8","28.4","0"]"#;
        let k: AsterKline = serde_json::from_str(row).unwrap();
        assert_eq!(k.open_time, 1499040000000);
        assert_eq!(k.open, "0.016");
        assert_eq!(k.close, "0.0158");
        assert_eq!(k.close_time, 1499644799999);
        assert_eq!(k.quote_volume, "2434.2");
        assert_eq!(k.num_trades, 308);
    }

    #[test]
    fn kline_short_row_is_error() {
        let row = r#"[1499040000000,"0.016","0.8"]"#;
        let err = serde_json::from_str::<AsterKline>(row).unwrap_err();
        assert!(err.to_string().contains("missing index 3"), "{err}");
    }

    #[test]
    fn kline_wrong_type_is_error() {
        let row =
            r#"[1499040000000,0.016,"0.8","0.015","0.0158","148976.1",1499644799999,"2434.2",308]"#;
        let err = serde_json::from_str::<AsterKline>(row).unwrap_err();
        assert!(err.to_string().contains("index 1 is not a string"), "{err}");
    }

    #[test]
    fn subscription_streams() {
        let p = |m: AsterWssMessage| m.params.unwrap();
        assert_eq!(
            p(AsterWssMessage::depth("BTCUSDT", AsterDepthSpeed::Ms100)),
            vec!["btcusdt@depth@100ms"]
        );
        assert_eq!(
            p(AsterWssMessage::depth_unsub(
                "BTCUSDT",
                AsterDepthSpeed::Default
            )),
            vec!["btcusdt@depth"]
        );
        assert_eq!(
            p(AsterWssMessage::klines("BTCUSDT", AsterKlineInterval::M1)),
            vec!["btcusdt@kline_1m"]
        );
        assert_eq!(
            p(AsterWssMessage::klines("BTCUSDT", AsterKlineInterval::Mo1)),
            vec!["btcusdt@kline_1M"]
        );
    }

    #[test]
    fn kline_not_array_is_error() {
        assert!(serde_json::from_str::<AsterKline>("{}").is_err());
    }
}
