use super::nullable;
use serde::{Deserialize, Deserializer, Serialize};

/// `GET /api/v2/spot/public/symbols`. The deprecated `openTime` field is not read.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BitgetSpotSymbol {
    pub symbol: String,
    #[serde(rename = "baseCoin")]
    pub base_coin: String,
    #[serde(rename = "quoteCoin")]
    pub quote_coin: String,
    /// Obsolete per the docs; see `min_trade_usdt`.
    #[serde(rename = "minTradeAmount")]
    pub min_trade_amount: String,
    /// Obsolete per the docs.
    #[serde(rename = "maxTradeAmount")]
    pub max_trade_amount: String,
    #[serde(rename = "takerFeeRate")]
    pub taker_fee_rate: String,
    #[serde(rename = "makerFeeRate")]
    pub maker_fee_rate: String,
    #[serde(rename = "pricePrecision")]
    pub price_precision: String,
    #[serde(rename = "quantityPrecision")]
    pub quantity_precision: String,
    #[serde(rename = "quotePrecision")]
    pub quote_precision: String,
    #[serde(rename = "minTradeUSDT")]
    pub min_trade_usdt: String,
    /// "offline", "gray", "online", "halt"
    pub status: String,
    #[serde(rename = "buyLimitPriceRatio")]
    pub buy_limit_price_ratio: String,
    #[serde(rename = "sellLimitPriceRatio")]
    pub sell_limit_price_ratio: String,
    #[serde(rename = "orderQuantity")]
    pub order_quantity: String,
    /// "yes" / "no"
    #[serde(rename = "areaSymbol")]
    pub area_symbol: String,
    #[serde(rename = "maxLimitOrderValue")]
    pub max_limit_order_value: String,
    #[serde(rename = "maxMarketOrderValue")]
    pub max_market_order_value: String,
    /// Unix ms; empty when not scheduled
    #[serde(rename = "offTime")]
    pub off_time: String,
}

/// `GET /api/v2/spot/market/tickers`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BitgetSpotTicker {
    pub symbol: String,
    /// 24h open price
    pub open: String,
    #[serde(rename = "high24h")]
    pub high_24h: String,
    #[serde(rename = "low24h")]
    pub low_24h: String,
    #[serde(rename = "lastPr")]
    pub last_pr: String,
    #[serde(rename = "quoteVolume")]
    pub quote_volume: String,
    #[serde(rename = "baseVolume")]
    pub base_volume: String,
    #[serde(rename = "usdtVolume")]
    pub usdt_volume: String,
    #[serde(rename = "bidPr")]
    pub bid_pr: String,
    #[serde(rename = "askPr")]
    pub ask_pr: String,
    /// `null` when the book side is empty (20 of 3666 live on 2026-11-16).
    #[serde(rename = "bidSz", deserialize_with = "nullable")]
    pub bid_sz: Option<String>,
    /// `null` when the book side is empty (21 of 3666 live on 2026-11-16).
    #[serde(rename = "askSz", deserialize_with = "nullable")]
    pub ask_sz: Option<String>,
    #[serde(rename = "openUtc")]
    pub open_utc: String,
    /// Unix ms
    pub ts: String,
    #[serde(rename = "changeUtc24h")]
    pub change_utc_24h: String,
    #[serde(rename = "change24h")]
    pub change_24h: String,
}

/// `GET /api/v2/spot/market/orderbook`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BitgetSpotOrderBook {
    /// `[price, size]`. Bitget sends strings for `type=step0` and JSON numbers for the merged
    /// steps; numbers are kept as their JSON text (e.g. `64652.0`).
    #[serde(deserialize_with = "levels")]
    pub asks: Vec<[String; 2]>,
    /// `[price, size]`, as `asks`.
    #[serde(deserialize_with = "levels")]
    pub bids: Vec<[String; 2]>,
    /// Matching engine time, Unix ms
    pub ts: String,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum StrOrNum {
    Str(String),
    Num(serde_json::Number),
}

impl From<StrOrNum> for String {
    fn from(v: StrOrNum) -> Self {
        match v {
            StrOrNum::Str(s) => s,
            StrOrNum::Num(n) => n.to_string(),
        }
    }
}

fn levels<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<[String; 2]>, D::Error> {
    let raw = Vec::<[StrOrNum; 2]>::deserialize(d)?;
    Ok(raw
        .into_iter()
        .map(|[p, s]| [String::from(p), String::from(s)])
        .collect())
}

/// A spot REST candle: `[ts, open, high, low, close, baseVolume, usdtVolume, quoteVolume]`
/// (field order as documented for `/api/v2/spot/market/candles` and `/history-candles`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(from = "[String; 8]")]
pub struct BitgetSpotCandle {
    /// Start time, Unix ms
    pub ts: String,
    pub open: String,
    pub high: String,
    pub low: String,
    pub close: String,
    pub base_volume: String,
    pub usdt_volume: String,
    pub quote_volume: String,
}

impl From<[String; 8]> for BitgetSpotCandle {
    fn from(a: [String; 8]) -> Self {
        let [ts, open, high, low, close, base_volume, usdt_volume, quote_volume] = a;
        Self {
            ts,
            open,
            high,
            low,
            close,
            base_volume,
            usdt_volume,
            quote_volume,
        }
    }
}

/// `GET /api/v2/spot/market/auction` and the spot `auction` websocket channel.
/// Every field is sent, and is `null` when the symbol has no call auction (all 17 symbols
/// checked live on 2026-11-16).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BitgetSpotAuction {
    /// "pre_market", "stage_1", "stage_2", "stage_3", "success", "failure"
    #[serde(deserialize_with = "nullable")]
    pub stage: Option<String>,
    /// Unix ms
    #[serde(rename = "stageEndTime", deserialize_with = "nullable")]
    pub stage_end_time: Option<String>,
    #[serde(rename = "estOpeningPrice", deserialize_with = "nullable")]
    pub est_opening_price: Option<String>,
    /// Base coin
    #[serde(rename = "matchedVolume", deserialize_with = "nullable")]
    pub matched_volume: Option<String>,
    /// Unix ms
    #[serde(rename = "auctionEndTime", deserialize_with = "nullable")]
    pub auction_end_time: Option<String>,
}
