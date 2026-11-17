mod spot;
mod ws;
pub use spot::*;
pub use ws::*;

#[cfg(test)]
mod tests;

use serde::{Deserialize, Deserializer, Serialize};

/// For fields that are always sent but may be `null`: the key must be present (no `default`).
pub(crate) fn nullable<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}

/// Futures product type (`productType` on REST, `instType` on the websocket).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BitgetProductType {
    /// USDT-M futures
    UsdtFutures,
    /// Coin-M futures
    CoinFutures,
    /// USDC-M futures
    UsdcFutures,
}

impl BitgetProductType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::UsdtFutures => "USDT-FUTURES",
            Self::CoinFutures => "COIN-FUTURES",
            Self::UsdcFutures => "USDC-FUTURES",
        }
    }
}

/// `kLineType` of `GET /api/v2/mix/market/candles`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BitgetKLineType {
    /// Traded price candles (Bitget's default).
    Market,
    Mark,
    Index,
}
impl BitgetKLineType {
    /// Bitget only honours the lowercase values; uppercase is silently treated as `market`.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Market => "market",
            Self::Mark => "mark",
            Self::Index => "index",
        }
    }
}

/// Optional time range and size of a candle request. Fields left `None` are not sent.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BitgetCandleParams {
    /// `startTime`, Unix ms.
    pub start_time: Option<i64>,
    /// `endTime`, Unix ms.
    pub end_time: Option<i64>,
    /// `limit`.
    pub limit: Option<u32>,
}

/// Optional paging of a `fills-history` request. Fields left `None` are not sent.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BitgetFillsHistoryParams {
    /// `limit`.
    pub limit: Option<u32>,
    /// `idLessThan`: return trades older than this `tradeId`.
    pub id_less_than: Option<String>,
    /// `startTime`, Unix ms.
    pub start_time: Option<i64>,
    /// `endTime`, Unix ms.
    pub end_time: Option<i64>,
}

/// Generic Bitget API response envelope.
/// Success is indicated by code "00000". On error `data` is `null`, so callers check
/// [`BitgetStatus`] before parsing the full envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BitgetResponse<T> {
    pub code: String,
    pub msg: String,
    pub data: T,
}

/// The `code`/`msg` part of every Bitget REST reply, present on success and error.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct BitgetStatus {
    pub code: String,
    pub msg: String,
}

/// `GET /api/v2/public/time`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BitgetServerTime {
    /// Unix ms
    #[serde(rename = "serverTime")]
    pub server_time: String,
}

/// `GET /api/v2/mix/market/contracts`. The deprecated `posLimit` and `openTime` fields are not
/// read.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BitgetContract {
    pub symbol: String,
    #[serde(rename = "baseCoin")]
    pub base_coin: String,
    #[serde(rename = "quoteCoin")]
    pub quote_coin: String,
    #[serde(rename = "buyLimitPriceRatio")]
    pub buy_limit_price_ratio: String,
    #[serde(rename = "sellLimitPriceRatio")]
    pub sell_limit_price_ratio: String,
    #[serde(rename = "feeRateUpRatio")]
    pub fee_rate_up_ratio: String,
    #[serde(rename = "makerFeeRate")]
    pub maker_fee_rate: String,
    #[serde(rename = "takerFeeRate")]
    pub taker_fee_rate: String,
    #[serde(rename = "openCostUpRatio")]
    pub open_cost_up_ratio: String,
    #[serde(rename = "supportMarginCoins")]
    pub support_margin_coins: Vec<String>,
    #[serde(rename = "minTradeNum")]
    pub min_trade_num: String,
    #[serde(rename = "priceEndStep")]
    pub price_end_step: String,
    #[serde(rename = "volumePlace")]
    pub volume_place: String,
    #[serde(rename = "pricePlace")]
    pub price_place: String,
    #[serde(rename = "sizeMultiplier")]
    pub size_multiplier: String,
    /// "perpetual" or "delivery"
    #[serde(rename = "symbolType")]
    pub symbol_type: String,
    #[serde(rename = "minTradeUSDT")]
    pub min_trade_usdt: String,
    #[serde(rename = "maxSymbolOrderNum")]
    pub max_symbol_order_num: String,
    #[serde(rename = "maxProductOrderNum")]
    pub max_product_order_num: String,
    #[serde(rename = "maxPositionNum")]
    pub max_position_num: String,
    /// "listed", "normal", "maintain", "limit_open", "restrictedAPI", "off"
    #[serde(rename = "symbolStatus")]
    pub symbol_status: String,
    /// "-1" when not scheduled
    #[serde(rename = "offTime")]
    pub off_time: String,
    /// "-1" when not scheduled
    #[serde(rename = "limitOpenTime")]
    pub limit_open_time: String,
    /// Empty for perpetuals
    #[serde(rename = "deliveryTime")]
    pub delivery_time: String,
    /// Empty for perpetuals
    #[serde(rename = "deliveryStartTime")]
    pub delivery_start_time: String,
    /// "this_quarter", "next_quarter"; empty for perpetuals
    #[serde(rename = "deliveryPeriod")]
    pub delivery_period: String,
    #[serde(rename = "launchTime")]
    pub launch_time: String,
    /// Funding interval in hours
    #[serde(rename = "fundInterval")]
    pub fund_interval: String,
    #[serde(rename = "minLever")]
    pub min_lever: String,
    #[serde(rename = "maxLever")]
    pub max_lever: String,
    #[serde(rename = "maintainTime")]
    pub maintain_time: String,
    #[serde(rename = "maxMarketOrderQty")]
    pub max_market_order_qty: String,
    #[serde(rename = "maxOrderQty")]
    pub max_order_qty: String,
    /// "YES" / "NO"
    #[serde(rename = "isRwa")]
    pub is_rwa: String,
}

/// `GET /api/v2/mix/market/ticker` and `/tickers`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BitgetTickerData {
    pub symbol: String,
    /// Last traded price
    #[serde(rename = "lastPr")]
    pub last_pr: String,
    #[serde(rename = "askPr")]
    pub ask_pr: String,
    #[serde(rename = "bidPr")]
    pub bid_pr: String,
    #[serde(rename = "bidSz")]
    pub bid_sz: String,
    #[serde(rename = "askSz")]
    pub ask_sz: String,
    #[serde(rename = "high24h")]
    pub high_24h: String,
    #[serde(rename = "low24h")]
    pub low_24h: String,
    /// Timestamp in milliseconds
    pub ts: String,
    #[serde(rename = "change24h")]
    pub change_24h: String,
    #[serde(rename = "baseVolume")]
    pub base_volume: String,
    #[serde(rename = "quoteVolume")]
    pub quote_volume: String,
    #[serde(rename = "usdtVolume")]
    pub usdt_volume: String,
    #[serde(rename = "openUtc")]
    pub open_utc: String,
    #[serde(rename = "changeUtc24h")]
    pub change_utc_24h: String,
    #[serde(rename = "fundingRate")]
    pub funding_rate: String,
    #[serde(rename = "markPrice")]
    pub mark_price: String,
    #[serde(rename = "indexPrice")]
    pub index_price: String,
    #[serde(rename = "holdingAmount")]
    pub holding_amount: String,
    #[serde(rename = "open24h")]
    pub open_24h: String,
    /// Delivery contracts only; `null` for perpetuals.
    #[serde(rename = "deliveryStartTime", deserialize_with = "nullable")]
    pub delivery_start_time: Option<String>,
    /// Delivery contracts only; `null` for perpetuals.
    #[serde(rename = "deliveryTime", deserialize_with = "nullable")]
    pub delivery_time: Option<String>,
    /// Delivery contracts only; empty for perpetuals.
    #[serde(rename = "deliveryStatus")]
    pub delivery_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BitgetHistoricalFundingRate {
    pub symbol: String,
    #[serde(rename = "fundingRate")]
    pub funding_rate: String,
    /// Settlement time in milliseconds
    #[serde(rename = "fundingTime")]
    pub funding_time: String,
}

/// `merge-depth` (futures and spot). Bitget sends the levels as JSON numbers, not strings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BitgetMergeDepth {
    /// `[price, size]`
    pub asks: Vec<[f64; 2]>,
    /// `[price, size]`
    pub bids: Vec<[f64; 2]>,
    /// Matching engine time, Unix ms
    pub ts: String,
    /// Actual price step of this reply
    pub scale: String,
    /// Requested precision (`scale0`..`scale3`)
    pub precision: String,
    /// "YES" / "NO"
    #[serde(rename = "isMaxPrecision")]
    pub is_max_precision: String,
}

/// A public trade from `fills` / `fills-history` (futures and spot).
/// `side` is lowercase on `fills` and capitalised (`Buy`/`Sell`) on `fills-history`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BitgetMarketTrade {
    pub symbol: String,
    #[serde(rename = "tradeId")]
    pub trade_id: String,
    pub side: String,
    pub price: String,
    pub size: String,
    /// Unix ms
    pub ts: String,
}

/// A futures REST candle: `[ts, open, high, low, close, baseVolume, quoteVolume]`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(from = "[String; 7]")]
pub struct BitgetCandle {
    /// Start time, Unix ms
    pub ts: String,
    pub open: String,
    pub high: String,
    pub low: String,
    pub close: String,
    pub base_volume: String,
    pub quote_volume: String,
}

impl From<[String; 7]> for BitgetCandle {
    fn from(a: [String; 7]) -> Self {
        let [ts, open, high, low, close, base_volume, quote_volume] = a;
        Self {
            ts,
            open,
            high,
            low,
            close,
            base_volume,
            quote_volume,
        }
    }
}

/// `GET /api/v2/mix/market/open-interest`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BitgetOpenInterest {
    #[serde(rename = "openInterestList")]
    pub open_interest_list: Vec<BitgetOpenInterestEntry>,
    /// Unix ms
    pub ts: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BitgetOpenInterestEntry {
    pub symbol: String,
    /// Open interest in base coin
    pub size: String,
}

/// `GET /api/v2/mix/market/current-fund-rate`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BitgetCurrentFundingRate {
    pub symbol: String,
    #[serde(rename = "fundingRate")]
    pub funding_rate: String,
    /// Hours
    #[serde(rename = "fundingRateInterval")]
    pub funding_rate_interval: String,
    /// Unix ms
    #[serde(rename = "nextUpdate")]
    pub next_update: String,
    /// `null` for some contracts (21 of 955 live on 2026-11-16).
    #[serde(rename = "minFundingRate", deserialize_with = "nullable")]
    pub min_funding_rate: Option<String>,
    /// `null` for some contracts (21 of 955 live on 2026-11-16).
    #[serde(rename = "maxFundingRate", deserialize_with = "nullable")]
    pub max_funding_rate: Option<String>,
}

/// `GET /api/v2/mix/market/funding-time`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BitgetNextFundingTime {
    pub symbol: String,
    /// Unix ms
    #[serde(rename = "nextFundingTime")]
    pub next_funding_time: String,
    /// Hours
    #[serde(rename = "ratePeriod")]
    pub rate_period: String,
}

/// `GET /api/v2/mix/market/symbol-price`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BitgetSymbolPrice {
    pub symbol: String,
    /// Last traded price
    pub price: String,
    #[serde(rename = "indexPrice")]
    pub index_price: String,
    #[serde(rename = "markPrice")]
    pub mark_price: String,
    /// Unix ms
    pub ts: String,
}

/// `GET /api/v2/mix/market/oi-limit`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BitgetOiLimit {
    pub symbol: String,
    /// Per-user position notional limit
    #[serde(rename = "notionalValue")]
    pub notional_value: String,
    /// Main + sub-account position notional limit
    #[serde(rename = "totalNotionalValue")]
    pub total_notional_value: String,
    /// Not in the docs; `null` on every contract live (2026-11-16).
    #[serde(rename = "singlePositionRatio", deserialize_with = "nullable")]
    pub single_position_ratio: Option<String>,
    /// Not in the docs; `null` on every contract live (2026-11-16).
    #[serde(rename = "totalPositionRatio", deserialize_with = "nullable")]
    pub total_position_ratio: Option<String>,
}
