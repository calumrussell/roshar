//! Market-data payloads shared by REST (`data` entries) and WebSocket pushes (`data` entries).
//!
//! OKX sends numbers as strings; they are kept as strings so no precision is lost. Fields OKX
//! sends as "" when not applicable stay `String`. Every field is required unless noted.

use serde::{Deserialize, Deserializer, Serialize};

/// Order-book snapshot: `GET /api/v5/market/books`, `GET /api/v5/market/books-rpi`, and the WS
/// `books5`, `bbo-tbt` and `bbo-rpi-tbt` pushes.
///
/// Levels are `[price, size, x, order count]`. For `books`/`books5`/`bbo*` `x` is a deprecated
/// "0"; for `books-rpi` it is the size excluding RPI orders.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OkxOrderBook {
    pub asks: Vec<[String; 4]>,
    pub bids: Vec<[String; 4]>,
    /// Book generation time (ms)
    pub ts: String,
    #[serde(rename = "seqId")]
    pub seq_id: i64,
    /// Only sent on the WS `books5` channel.
    #[serde(rename = "instId", default, skip_serializing_if = "Option::is_none")]
    pub inst_id: Option<String>,
}

/// `GET /api/v5/market/books-full`. Levels are `[price, size, order count]`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OkxOrderBookFull {
    pub asks: Vec<[String; 3]>,
    pub bids: Vec<[String; 3]>,
    pub ts: String,
    #[serde(rename = "seqId")]
    pub seq_id: i64,
}

/// Public trade: `GET /api/v5/market/trades`, `GET /api/v5/market/history-trades` and the WS
/// `trades-all` push (one entry per fill, no aggregation).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OkxTrade {
    #[serde(rename = "instId")]
    pub inst_id: String,
    #[serde(rename = "tradeId")]
    pub trade_id: String,
    pub px: String,
    pub sz: String,
    /// Taker side: "buy" or "sell"
    pub side: String,
    pub ts: String,
    /// Order source: "0" normal, "1" Enhanced Liquidity Program
    pub source: String,
}

/// Index ticker: `GET /api/v5/market/index-tickers` and WS `index-tickers`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OkxIndexTicker {
    #[serde(rename = "instId")]
    pub inst_id: String,
    #[serde(rename = "idxPx")]
    pub idx_px: String,
    #[serde(rename = "high24h")]
    pub high_24h: String,
    #[serde(rename = "low24h")]
    pub low_24h: String,
    #[serde(rename = "open24h")]
    pub open_24h: String,
    #[serde(rename = "sodUtc0")]
    pub sod_utc0: String,
    #[serde(rename = "sodUtc8")]
    pub sod_utc8: String,
    pub ts: String,
}

/// Current funding: `GET /api/v5/public/funding-rate` and WS `funding-rate`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OkxFundingRate {
    #[serde(rename = "instType")]
    pub inst_type: String,
    #[serde(rename = "instId")]
    pub inst_id: String,
    /// "withRate" or "noRate"
    #[serde(rename = "formulaType")]
    pub formula_type: String,
    /// Current funding rate
    #[serde(rename = "fundingRate")]
    pub funding_rate: String,
    /// Settlement time of the current period (ms)
    #[serde(rename = "fundingTime")]
    pub funding_time: String,
    #[serde(rename = "impactValue")]
    pub impact_value: String,
    #[serde(rename = "interestRate")]
    pub interest_rate: String,
    #[serde(rename = "maxFundingRate")]
    pub max_funding_rate: String,
    #[serde(rename = "minFundingRate")]
    pub min_funding_rate: String,
    /// "current_period"
    pub method: String,
    /// Deprecated by OKX; "" in live data
    #[serde(rename = "nextFundingRate")]
    pub next_funding_rate: String,
    #[serde(rename = "nextFundingTime")]
    pub next_funding_time: String,
    pub premium: String,
    #[serde(rename = "prevFundingTime")]
    pub prev_funding_time: String,
    /// Rate settled at `prev_funding_time`
    #[serde(rename = "settFundingRate")]
    pub sett_funding_rate: String,
    /// "processing" or "settled"
    #[serde(rename = "settState")]
    pub sett_state: String,
    pub ts: String,
}

/// `GET /api/v5/public/funding-rate-history`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OkxFundingRateHistory {
    #[serde(rename = "instType")]
    pub inst_type: String,
    #[serde(rename = "instId")]
    pub inst_id: String,
    #[serde(rename = "formulaType")]
    pub formula_type: String,
    #[serde(rename = "fundingRate")]
    pub funding_rate: String,
    #[serde(rename = "realizedRate")]
    pub realized_rate: String,
    #[serde(rename = "fundingTime")]
    pub funding_time: String,
    pub method: String,
}

/// `GET /api/v5/public/open-interest` and WS `open-interest`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OkxOpenInterest {
    #[serde(rename = "instType")]
    pub inst_type: String,
    #[serde(rename = "instId")]
    pub inst_id: String,
    /// Open interest in contracts
    pub oi: String,
    /// Open interest in currency
    #[serde(rename = "oiCcy")]
    pub oi_ccy: String,
    /// Open interest in USD
    #[serde(rename = "oiUsd")]
    pub oi_usd: String,
    pub ts: String,
}

/// `GET /api/v5/public/price-limit` and WS `price-limit`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OkxPriceLimit {
    #[serde(rename = "instType")]
    pub inst_type: String,
    #[serde(rename = "instId")]
    pub inst_id: String,
    /// Highest buy limit ("" when disabled)
    #[serde(rename = "buyLmt")]
    pub buy_lmt: String,
    /// Lowest sell limit ("" when disabled)
    #[serde(rename = "sellLmt")]
    pub sell_lmt: String,
    /// Whether the price limit is in effect
    pub enabled: bool,
    pub ts: String,
}

/// `GET /api/v5/public/mark-price` and WS `mark-price`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OkxMarkPrice {
    #[serde(rename = "instType")]
    pub inst_type: String,
    #[serde(rename = "instId")]
    pub inst_id: String,
    #[serde(rename = "markPx")]
    pub mark_px: String,
    pub ts: String,
}

/// `GET /api/v5/public/estimated-price` and WS `estimated-price` (delivery/exercise price,
/// published shortly before settlement; "0" otherwise).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OkxEstimatedPrice {
    #[serde(rename = "instType")]
    pub inst_type: String,
    #[serde(rename = "instId")]
    pub inst_id: String,
    #[serde(rename = "settlePx")]
    pub settle_px: String,
    pub ts: String,
}

/// `GET /api/v5/public/estimated-settlement-info` (only returns data 30 minutes before a
/// FUTURES settlement; otherwise OKX answers error 51001).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OkxEstimatedSettlement {
    #[serde(rename = "instId")]
    pub inst_id: String,
    #[serde(rename = "nextSettleTime")]
    pub next_settle_time: String,
    #[serde(rename = "estSettlePx")]
    pub est_settle_px: String,
    pub ts: String,
}

/// `GET /api/v5/public/delivery-exercise-history`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OkxDeliveryExercise {
    pub ts: String,
    pub details: Vec<OkxDeliveryExerciseDetail>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OkxDeliveryExerciseDetail {
    /// Delivery/exercise contract ID (OKX spells it `insId`)
    #[serde(rename = "insId")]
    pub ins_id: String,
    pub px: String,
    /// "delivery", "exercised" or "expired_otm"
    #[serde(rename = "type")]
    pub kind: String,
}

/// `GET /api/v5/public/settlement-history`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OkxSettlement {
    pub ts: String,
    pub details: Vec<OkxSettlementDetail>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OkxSettlementDetail {
    #[serde(rename = "instId")]
    pub inst_id: String,
    #[serde(rename = "settlePx")]
    pub settle_px: String,
}

/// `GET /api/v5/public/insurance-fund`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OkxInsuranceFund {
    #[serde(rename = "instType")]
    pub inst_type: String,
    /// "" for MARGIN
    #[serde(rename = "instFamily")]
    pub inst_family: String,
    /// Total balance of the insurance fund, in USD
    pub total: String,
    pub details: Vec<OkxInsuranceFundDetail>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OkxInsuranceFundDetail {
    pub balance: String,
    /// Change in the balance ("" when not applicable)
    pub amt: String,
    pub ccy: String,
    /// e.g. "regular_update", "liquidation_balance_deposit", "bankruptcy_loss"
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(rename = "maxBal")]
    pub max_bal: String,
    #[serde(rename = "maxBalTs")]
    pub max_bal_ts: String,
    #[serde(rename = "decRate")]
    pub dec_rate: String,
    #[serde(rename = "adlType")]
    pub adl_type: String,
    pub ts: String,
}

/// `GET /api/v5/public/time`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OkxSystemTime {
    pub ts: String,
}

/// Maintenance entry: `GET /api/v5/system/status` and WS `status`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OkxSystemStatus {
    pub title: String,
    /// "scheduled", "ongoing", "pre_open", "completed" or "canceled"
    pub state: String,
    pub begin: String,
    pub end: String,
    #[serde(rename = "preOpenBegin")]
    pub pre_open_begin: String,
    pub href: String,
    #[serde(rename = "serviceType")]
    pub service_type: String,
    pub system: String,
    #[serde(rename = "scheDesc")]
    pub sche_desc: String,
    #[serde(rename = "maintType")]
    pub maint_type: String,
    pub env: String,
    /// Push time; only sent on the WS `status` channel (absent from REST responses).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ts: Option<String>,
}

/// `GET /api/v5/public/premium-history`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OkxPremium {
    #[serde(rename = "instId")]
    pub inst_id: String,
    pub premium: String,
    pub ts: String,
}

/// Option greeks: `GET /api/v5/public/opt-summary` and WS `opt-summary`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OkxOptionSummary {
    #[serde(rename = "instType")]
    pub inst_type: String,
    #[serde(rename = "instId")]
    pub inst_id: String,
    pub uly: String,
    pub delta: String,
    pub gamma: String,
    pub vega: String,
    pub theta: String,
    #[serde(rename = "deltaBS")]
    pub delta_bs: String,
    #[serde(rename = "gammaBS")]
    pub gamma_bs: String,
    #[serde(rename = "vegaBS")]
    pub vega_bs: String,
    #[serde(rename = "thetaBS")]
    pub theta_bs: String,
    pub lever: String,
    #[serde(rename = "markVol")]
    pub mark_vol: String,
    #[serde(rename = "bidVol")]
    pub bid_vol: String,
    #[serde(rename = "askVol")]
    pub ask_vol: String,
    #[serde(rename = "realVol")]
    pub real_vol: String,
    #[serde(rename = "volLv")]
    pub vol_lv: String,
    #[serde(rename = "fwdPx")]
    pub fwd_px: String,
    pub distance: String,
    #[serde(rename = "buyApr")]
    pub buy_apr: String,
    #[serde(rename = "sellApr")]
    pub sell_apr: String,
    pub ts: String,
}

/// Option trade: `GET /api/v5/public/option-trades` and WS `option-trades`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OkxOptionTrade {
    #[serde(rename = "instId")]
    pub inst_id: String,
    #[serde(rename = "instFamily")]
    pub inst_family: String,
    #[serde(rename = "tradeId")]
    pub trade_id: String,
    pub px: String,
    pub sz: String,
    pub side: String,
    /// "C" or "P"
    #[serde(rename = "optType")]
    pub opt_type: String,
    #[serde(rename = "fillVol")]
    pub fill_vol: String,
    #[serde(rename = "fwdPx")]
    pub fwd_px: String,
    #[serde(rename = "idxPx")]
    pub idx_px: String,
    #[serde(rename = "markPx")]
    pub mark_px: String,
    pub ts: String,
}

/// `GET /api/v5/market/option/instrument-family-trades`: latest trades grouped by option type.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OkxOptionFamilyTrades {
    /// 24h volume of the option type, in contracts
    #[serde(rename = "vol24h")]
    pub vol_24h: String,
    /// "C" or "P"
    #[serde(rename = "optType")]
    pub opt_type: String,
    #[serde(rename = "tradeInfo")]
    pub trade_info: Vec<OkxOptionFamilyTrade>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OkxOptionFamilyTrade {
    #[serde(rename = "instId")]
    pub inst_id: String,
    #[serde(rename = "tradeId")]
    pub trade_id: String,
    pub px: String,
    pub sz: String,
    pub side: String,
    pub ts: String,
}

/// `GET /api/v5/public/position-tiers`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OkxPositionTier {
    pub uly: String,
    #[serde(rename = "instFamily")]
    pub inst_family: String,
    #[serde(rename = "instId")]
    pub inst_id: String,
    pub tier: String,
    #[serde(rename = "minSz")]
    pub min_sz: String,
    #[serde(rename = "maxSz")]
    pub max_sz: String,
    pub mmr: String,
    pub imr: String,
    #[serde(rename = "maxLever")]
    pub max_lever: String,
    #[serde(rename = "optMgnFactor")]
    pub opt_mgn_factor: String,
    #[serde(rename = "quoteMaxLoan")]
    pub quote_max_loan: String,
    #[serde(rename = "baseMaxLoan")]
    pub base_max_loan: String,
}

/// `GET /api/v5/market/index-components` (`data` is a single object).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OkxIndexComponents {
    pub index: String,
    pub last: String,
    pub ts: String,
    pub components: Vec<OkxIndexComponent>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OkxIndexComponent {
    pub exch: String,
    pub symbol: String,
    #[serde(rename = "symPx")]
    pub sym_px: String,
    pub wgt: String,
    #[serde(rename = "cnvPx")]
    pub cnv_px: String,
}

/// `GET /api/v5/market/exchange-rate`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OkxExchangeRate {
    #[serde(rename = "usdCny")]
    pub usd_cny: String,
}

/// `GET /api/v5/market/platform-24-volume`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OkxPlatformVolume {
    #[serde(rename = "volUsd")]
    pub vol_usd: String,
    #[serde(rename = "volCny")]
    pub vol_cny: String,
    pub ts: String,
}

/// `GET /api/v5/market/call-auction-details` and WS `call-auction-details`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OkxCallAuctionDetails {
    #[serde(rename = "instId")]
    pub inst_id: String,
    /// Equilibrium price ("" outside the auction)
    #[serde(rename = "eqPx")]
    pub eq_px: String,
    #[serde(rename = "matchedSz")]
    pub matched_sz: String,
    #[serde(rename = "unmatchedSz")]
    pub unmatched_sz: String,
    #[serde(rename = "auctionEndTime")]
    pub auction_end_time: String,
    /// "call_auction" or "continuous_trading"
    pub state: String,
    pub ts: String,
}

/// `GET /api/v5/public/event-contract/series`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OkxEventSeries {
    #[serde(rename = "seriesId")]
    pub series_id: String,
    pub title: String,
    pub category: String,
    pub freq: String,
    pub settlement: OkxEventSeriesSettlement,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OkxEventSeriesSettlement {
    pub method: String,
    pub underlying: String,
    #[serde(rename = "srcName")]
    pub src_name: String,
    #[serde(rename = "closeEarly")]
    pub close_early: bool,
}

/// `GET /api/v5/public/event-contract/events`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OkxEvent {
    #[serde(rename = "seriesId")]
    pub series_id: String,
    #[serde(rename = "eventId")]
    pub event_id: String,
    pub state: String,
    #[serde(rename = "expTime")]
    pub exp_time: String,
    #[serde(rename = "fixTime")]
    pub fix_time: String,
}

/// `GET /api/v5/public/event-contract/markets` and WS `event-contract-markets`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OkxEventMarket {
    #[serde(rename = "seriesId")]
    pub series_id: String,
    #[serde(rename = "eventId")]
    pub event_id: String,
    #[serde(rename = "instId")]
    pub inst_id: String,
    pub state: String,
    #[serde(rename = "listTime")]
    pub list_time: String,
    #[serde(rename = "expTime")]
    pub exp_time: String,
    #[serde(rename = "fixTime")]
    pub fix_time: String,
    #[serde(rename = "floorStrike")]
    pub floor_strike: String,
    #[serde(rename = "capStrike")]
    pub cap_strike: String,
    #[serde(rename = "hitDir")]
    pub hit_dir: String,
    pub outcome: String,
    #[serde(rename = "settleValue")]
    pub settle_value: String,
    pub disputed: bool,
}

/// `GET /api/v5/public/instrument-tick-bands`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OkxInstrumentTickBands {
    #[serde(rename = "instType")]
    pub inst_type: String,
    #[serde(rename = "instFamily")]
    pub inst_family: String,
    #[serde(rename = "seriesId")]
    pub series_id: String,
    #[serde(rename = "tickBand")]
    pub tick_band: Vec<OkxTickBand>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OkxTickBand {
    #[serde(rename = "minPx")]
    pub min_px: String,
    #[serde(rename = "maxPx")]
    pub max_px: String,
    #[serde(rename = "tickSz")]
    pub tick_sz: String,
}

/// Liquidation: WS `liquidation-orders`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OkxLiquidation {
    #[serde(rename = "instType")]
    pub inst_type: String,
    #[serde(rename = "instId")]
    pub inst_id: String,
    #[serde(rename = "instFamily")]
    pub inst_family: String,
    pub uly: String,
    pub details: Vec<OkxLiquidationDetail>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OkxLiquidationDetail {
    /// Order side: "buy" or "sell"
    pub side: String,
    /// "long", "short" or "net"
    #[serde(rename = "posSide")]
    pub pos_side: String,
    /// Bankruptcy price
    #[serde(rename = "bkPx")]
    pub bk_px: String,
    pub sz: String,
    /// Bankruptcy loss
    #[serde(rename = "bkLoss")]
    pub bk_loss: String,
    /// Liquidation currency; MARGIN only ("" otherwise)
    pub ccy: String,
    pub ts: String,
}

/// Mark-price or index candle: `[ts, o, h, l, c, confirm]`.
/// From `GET /api/v5/market/{history-,}{mark-price,index}-candles` and the WS
/// `mark-price-candle*` / `index-candle*` channels.
#[derive(Debug, Clone, Serialize)]
pub struct OkxPriceCandle {
    /// Opening time (ms)
    pub ts: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    /// 0 = incomplete, 1 = complete
    pub confirm: u8,
}

impl<'de> Deserialize<'de> for OkxPriceCandle {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::Error;
        let (ts, o, h, l, c, confirm) =
            <(String, String, String, String, String, String)>::deserialize(deserializer)?;
        fn num<T: std::str::FromStr, E: Error>(s: &str, name: &str) -> Result<T, E> {
            s.parse::<T>()
                .map_err(|_| E::custom(format!("failed to parse {name}: {s:?}")))
        }
        Ok(OkxPriceCandle {
            ts: num(&ts, "ts")?,
            open: num(&o, "open")?,
            high: num(&h, "high")?,
            low: num(&l, "low")?,
            close: num(&c, "close")?,
            confirm: num(&confirm, "confirm")?,
        })
    }
}
