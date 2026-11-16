//! REST request enums and response payloads for `/v5/market/*`, `/v5/spread/*` and
//! `/v5/system/status`. Every struct is the `result` object of ByBit's
//! `{retCode, retMsg, result, ...}` envelope; the envelope is checked by `MarketApi`.

use serde::{Deserialize, Serialize};

/// `category` request parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ByBitCategory {
    Spot,
    Linear,
    Inverse,
    Option,
}

impl ByBitCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Spot => "spot",
            Self::Linear => "linear",
            Self::Inverse => "inverse",
            Self::Option => "option",
        }
    }
}

/// `category` for endpoints whose response shape is the derivatives one (`linear` / `inverse`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ByBitContractCategory {
    Linear,
    Inverse,
}

impl ByBitContractCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Linear => "linear",
            Self::Inverse => "inverse",
        }
    }
}

/// `intervalTime` (open interest) / `period` (long/short ratio).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ByBitDataPeriod {
    Min5,
    Min15,
    Min30,
    Hour1,
    Hour4,
    Day1,
}

impl ByBitDataPeriod {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Min5 => "5min",
            Self::Min15 => "15min",
            Self::Min30 => "30min",
            Self::Hour1 => "1h",
            Self::Hour4 => "4h",
            Self::Day1 => "1d",
        }
    }
}

/// Filters for `GET /v5/market/instruments-info`. All optional; unset fields are omitted.
/// `limit` / `cursor` are ignored by ByBit for `spot`.
#[derive(Debug, Clone, Default)]
pub struct ByBitInstrumentsQuery {
    pub symbol: Option<String>,
    pub symbol_type: Option<String>,
    pub status: Option<String>,
    /// `option` only: `All` returns every base coin.
    pub base_coin: Option<String>,
    pub limit: Option<u32>,
    /// `nextPageCursor` from a previous page, passed through as returned (already URL-encoded).
    pub cursor: Option<String>,
}

// ---------------------------------------------------------------------------------------------
// Server time / system status
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ByBitServerTime {
    pub time_second: String,
    pub time_nano: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ByBitSystemStatusResult {
    pub list: Vec<ByBitSystemStatus>,
}

/// One maintenance / incident record (`GET /v5/system/status` and the `system.status` stream).
/// No record was live when this was written; types follow the documented response example
/// (`maintainType` / `env` are integers there although the table says string).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ByBitSystemStatus {
    pub id: String,
    pub title: String,
    pub state: String,
    pub begin: String,
    pub end: String,
    pub href: String,
    pub service_types: Vec<i64>,
    pub product: Vec<i64>,
    pub uid_suffix: Vec<i64>,
    pub maintain_type: i64,
    pub env: i64,
}

// ---------------------------------------------------------------------------------------------
// Klines
// ---------------------------------------------------------------------------------------------

/// `GET /v5/market/kline`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ByBitKlineResult {
    pub category: String,
    pub symbol: String,
    /// Newest first.
    pub list: Vec<ByBitKline>,
}

/// `[startTime, open, high, low, close, volume, turnover]`. Volume is in base coin for
/// linear/spot and quote coin for inverse; turnover the other way round.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(from = "[String; 7]", into = "[String; 7]")]
pub struct ByBitKline {
    pub start_time: String,
    pub open: String,
    pub high: String,
    pub low: String,
    pub close: String,
    pub volume: String,
    pub turnover: String,
}

impl From<[String; 7]> for ByBitKline {
    fn from(a: [String; 7]) -> Self {
        let [start_time, open, high, low, close, volume, turnover] = a;
        Self {
            start_time,
            open,
            high,
            low,
            close,
            volume,
            turnover,
        }
    }
}

impl From<ByBitKline> for [String; 7] {
    fn from(k: ByBitKline) -> Self {
        [
            k.start_time,
            k.open,
            k.high,
            k.low,
            k.close,
            k.volume,
            k.turnover,
        ]
    }
}

/// Mark / index / premium-index price klines.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ByBitPriceKlineResult {
    pub category: String,
    pub symbol: String,
    /// Newest first.
    pub list: Vec<ByBitPriceKline>,
}

/// `[startTime, open, high, low, close]`.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(from = "[String; 5]", into = "[String; 5]")]
pub struct ByBitPriceKline {
    pub start_time: String,
    pub open: String,
    pub high: String,
    pub low: String,
    pub close: String,
}

impl From<[String; 5]> for ByBitPriceKline {
    fn from(a: [String; 5]) -> Self {
        let [start_time, open, high, low, close] = a;
        Self {
            start_time,
            open,
            high,
            low,
            close,
        }
    }
}

impl From<ByBitPriceKline> for [String; 5] {
    fn from(k: ByBitPriceKline) -> Self {
        [k.start_time, k.open, k.high, k.low, k.close]
    }
}

// ---------------------------------------------------------------------------------------------
// Instruments
// ---------------------------------------------------------------------------------------------

/// `GET /v5/market/instruments-info` for `linear` / `inverse` / `option`.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ByBitInstrumentsResult<T> {
    pub category: String,
    pub list: Vec<T>,
    /// Already URL-encoded; empty on the last page.
    pub next_page_cursor: String,
}

/// `GET /v5/market/instruments-info?category=spot` (spot is not paginated: no cursor).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ByBitSpotInstrumentsResult {
    pub category: String,
    pub list: Vec<ByBitSpotInstrumentInfo>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ByBitSpotInstrumentInfo {
    pub symbol_id: u64,
    pub symbol: String,
    pub base_coin: String,
    pub quote_coin: String,
    /// Deprecated by ByBit, always "0".
    pub innovation: String,
    pub status: String,
    /// `none`, `utaOnly`, ...
    pub margin_trading: String,
    /// `1` when the pair carries the ST (special treatment) tag.
    pub st_tag: String,
    pub lot_size_filter: ByBitSpotLotSizeFilter,
    pub price_filter: ByBitSpotPriceFilter,
    pub risk_parameters: crate::rest::ByBitRiskParameters,
    pub symbol_type: String,
    pub xstock_multiplier: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ByBitSpotLotSizeFilter {
    pub base_precision: String,
    pub quote_precision: String,
    pub min_order_qty: String,
    pub max_order_qty: String,
    pub min_order_amt: String,
    pub max_order_amt: String,
    pub max_limit_order_qty: String,
    pub max_market_order_qty: String,
    pub post_only_max_limit_order_size: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ByBitSpotPriceFilter {
    pub tick_size: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ByBitOptionInstrumentInfo {
    pub symbol_id: u64,
    pub symbol: String,
    pub status: String,
    pub base_coin: String,
    pub quote_coin: String,
    pub settle_coin: String,
    /// `Call` / `Put`.
    pub options_type: String,
    pub launch_time: String,
    pub delivery_time: String,
    pub delivery_fee_rate: String,
    pub price_filter: crate::rest::ByBitPriceFilter,
    pub lot_size_filter: ByBitOptionLotSizeFilter,
    pub display_name: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ByBitOptionLotSizeFilter {
    pub max_order_qty: String,
    pub min_order_qty: String,
    pub qty_step: String,
}

/// `GET /v5/market/instrument-summary` (`linear`, `option`).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ByBitInstrumentSummaryResult {
    pub category: String,
    pub list: Vec<ByBitInstrumentSummary>,
    pub next_page_cursor: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ByBitInstrumentSummary {
    pub symbol: String,
    pub symbol_type: String,
    pub display_name: String,
    pub base_coin: String,
    pub settle_coin: String,
    /// Empty for `option`.
    pub full_name: String,
    /// Empty for `option`.
    pub market_region: String,
    /// Empty for `option`.
    pub underlying_ticker: String,
    pub qty_step: String,
    pub tick_size: String,
    /// Empty for `linear`.
    pub price_index: String,
    /// "0" for `linear`.
    pub delivery_time: String,
}

/// `GET /v5/market/option-base-coins`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ByBitOptionBaseCoinsResult {
    pub list: Vec<ByBitOptionBaseCoin>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ByBitOptionBaseCoin {
    pub base_coin: String,
    pub quote_coin: String,
    pub settle_coin: String,
    pub option_show_name: String,
    pub option_online_time: u64,
    /// `1` when tradable option symbols exist.
    pub has_symbol: u8,
    /// `0` crypto, `1` commodity, `2` stock, `3` forex, `4` oil.
    pub underlying_type: u8,
}

/// `GET /v5/market/corporate-action` (TradFi splits / dividends).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ByBitCorporateActionResult {
    pub next_page_cursor: String,
    pub list: Vec<ByBitCorporateAction>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ByBitCorporateAction {
    pub category: String,
    /// Empty for option-level actions.
    pub symbol: String,
    pub base_coin: String,
    /// `ForwardSplit`, `ReverseSplit`, `Dividend`.
    pub action_type: String,
    pub status: String,
    pub effective_time: u64,
    pub details: ByBitCorporateActionDetails,
}

/// Fields are documented as present only for the matching action type.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ByBitCorporateActionDetails {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub split_factor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub split_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dividend_amount_per_share: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dividend_currency: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub installment_count: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub installment_amount_per_share: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub installment_coin: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_settlement_time_utc: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub installments: Option<Vec<ByBitDividendInstallment>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ByBitDividendInstallment {
    pub installment_no: u32,
    pub settlement_time_utc: u64,
    pub status: String,
}

// ---------------------------------------------------------------------------------------------
// Order books
// ---------------------------------------------------------------------------------------------

/// `GET /v5/market/orderbook`, `/v5/market/full_orderbook`, `/v5/spread/orderbook`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ByBitOrderbook {
    pub s: String,
    /// `[price, size]`, best first.
    pub b: Vec<[String; 2]>,
    pub a: Vec<[String; 2]>,
    pub ts: u64,
    pub u: u64,
    pub seq: u64,
    pub cts: u64,
}

/// `GET /v5/market/rpi_orderbook`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ByBitRpiOrderbook {
    pub s: String,
    /// `[price, non-RPI size, RPI size]`, best first.
    pub b: Vec<[String; 3]>,
    pub a: Vec<[String; 3]>,
    pub ts: u64,
    pub u: u64,
    pub seq: u64,
    pub cts: u64,
}

// ---------------------------------------------------------------------------------------------
// Tickers
// ---------------------------------------------------------------------------------------------

/// `GET /v5/market/tickers`; `T` is the per-category row type.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ByBitTickersResult<T> {
    pub category: String,
    pub list: Vec<T>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ByBitSpotTicker {
    pub symbol: String,
    pub bid1_price: String,
    pub bid1_size: String,
    pub ask1_price: String,
    pub ask1_size: String,
    pub last_price: String,
    pub prev_price_24h: String,
    pub price_24h_pcnt: String,
    pub high_price_24h: String,
    pub low_price_24h: String,
    pub turnover_24h: String,
    pub volume_24h: String,
    /// Absent for pairs without a USD index (observed live on ~1 in 8 pairs).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usd_index_price: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ByBitOptionTicker {
    pub symbol: String,
    pub bid1_price: String,
    pub bid1_size: String,
    pub bid1_iv: String,
    pub ask1_price: String,
    pub ask1_size: String,
    pub ask1_iv: String,
    pub last_price: String,
    pub high_price_24h: String,
    pub low_price_24h: String,
    pub mark_price: String,
    pub index_price: String,
    pub mark_iv: String,
    pub underlying_price: String,
    pub open_interest: String,
    pub turnover_24h: String,
    pub volume_24h: String,
    pub total_volume: String,
    pub total_turnover: String,
    pub delta: String,
    pub gamma: String,
    pub vega: String,
    pub theta: String,
    pub predicted_delivery_price: String,
    pub change_24h: String,
}

/// `GET /v5/spread/tickers`. Book fields are "" when that side is empty.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ByBitSpreadTickersResult {
    pub list: Vec<ByBitSpreadTicker>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ByBitSpreadTicker {
    pub symbol: String,
    pub bid_price: String,
    pub bid_size: String,
    pub ask_price: String,
    pub ask_size: String,
    pub last_price: String,
    pub high_price_24h: String,
    pub low_price_24h: String,
    pub prev_price_24h: String,
    pub volume_24h: String,
}

// ---------------------------------------------------------------------------------------------
// Trades
// ---------------------------------------------------------------------------------------------

/// Filters for `GET /v5/market/recent-trade`.
#[derive(Debug, Clone, Default)]
pub struct ByBitRecentTradesQuery {
    /// Required by ByBit for spot/linear/inverse; optional for option.
    pub symbol: Option<String>,
    /// Option only.
    pub base_coin: Option<String>,
    /// Option only: `Call` / `Put`.
    pub option_type: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ByBitRecentTradesResult {
    pub category: String,
    pub list: Vec<ByBitRecentTrade>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ByBitRecentTrade {
    pub exec_id: String,
    pub symbol: String,
    pub price: String,
    pub size: String,
    /// Taker side.
    pub side: String,
    pub time: String,
    pub is_block_trade: bool,
    /// Absent for option (live).
    #[serde(
        rename = "isRPITrade",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub is_rpi_trade: Option<bool>,
    /// Option only: mark price.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub m_p: Option<String>,
    /// Option only: index price.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub i_p: Option<String>,
    /// Option only: mark IV.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub m_iv: Option<String>,
    /// Option only: trade IV.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iv: Option<String>,
    pub seq: String,
}

/// `GET /v5/spread/recent-trade`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ByBitSpreadRecentTradesResult {
    pub list: Vec<ByBitSpreadRecentTrade>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ByBitSpreadRecentTrade {
    pub exec_id: String,
    pub symbol: String,
    pub price: String,
    pub size: String,
    pub side: String,
    pub time: String,
    pub seq: String,
}

// ---------------------------------------------------------------------------------------------
// Funding / open interest / volatility / ratios
// ---------------------------------------------------------------------------------------------

/// `GET /v5/market/funding/history`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ByBitFundingHistoryResult {
    pub category: String,
    /// Newest first.
    pub list: Vec<crate::types::ByBitHistoricalFundingRate>,
}

/// `GET /v5/market/open-interest`.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ByBitOpenInterestResult {
    pub category: String,
    pub symbol: String,
    pub list: Vec<ByBitOpenInterest>,
    pub next_page_cursor: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ByBitOpenInterest {
    /// Both sides summed (base coin for linear, USD for inverse).
    pub open_interest: String,
    pub single_open_interest: String,
    pub timestamp: String,
}

/// One row of `GET /v5/market/historical-volatility` (whose `result` is a bare array).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ByBitHistoricalVolatility {
    /// Days.
    pub period: u32,
    pub value: String,
    pub time: String,
}

/// `GET /v5/market/account-ratio`.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ByBitLongShortRatioResult {
    pub list: Vec<ByBitLongShortRatio>,
    pub next_page_cursor: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ByBitLongShortRatio {
    pub symbol: String,
    pub buy_ratio: String,
    pub sell_ratio: String,
    pub timestamp: String,
}

// ---------------------------------------------------------------------------------------------
// Insurance / ADL / risk limit / delivery / index / price limit
// ---------------------------------------------------------------------------------------------

/// `GET /v5/market/insurance`.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ByBitInsuranceResult {
    pub updated_time: String,
    pub list: Vec<ByBitInsurance>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ByBitInsurance {
    pub coin: String,
    /// Comma-separated symbols sharing this pool (one symbol for an isolated pool).
    pub symbols: String,
    pub balance: String,
    /// USD value.
    pub value: String,
}

/// `GET /v5/market/adlAlert`.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ByBitAdlAlertResult {
    pub updated_time: String,
    pub list: Vec<ByBitAdlAlert>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ByBitAdlAlert {
    pub coin: String,
    pub symbol: String,
    pub balance: String,
    /// Deprecated, always "".
    pub max_balance: String,
    pub insurance_pnl_ratio: String,
    pub pnl_ratio: String,
    pub adl_trigger_threshold: String,
    pub adl_stop_ratio: String,
}

/// `GET /v5/market/risk-limit`.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ByBitRiskLimitResult {
    pub category: String,
    pub list: Vec<ByBitRiskLimit>,
    pub next_page_cursor: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ByBitRiskLimit {
    pub id: u64,
    pub symbol: String,
    pub risk_limit_value: String,
    /// Documented as number, sent as string.
    pub maintenance_margin: String,
    /// Documented as number, sent as string.
    pub initial_margin: String,
    /// `1` true, `0` false.
    pub is_lowest_risk: u8,
    pub max_leverage: String,
    pub mm_deduction: String,
}

/// `GET /v5/market/delivery-price`.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ByBitDeliveryPriceResult {
    pub category: String,
    pub list: Vec<ByBitDeliveryPrice>,
    pub next_page_cursor: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ByBitDeliveryPrice {
    pub symbol: String,
    pub delivery_price: String,
    pub delivery_time: String,
}

/// `GET /v5/market/new-delivery-price` (option, per base coin).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ByBitNewDeliveryPriceResult {
    pub category: String,
    pub list: Vec<ByBitNewDeliveryPrice>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ByBitNewDeliveryPrice {
    pub delivery_price: String,
    pub delivery_time: String,
}

/// `GET /v5/market/index-price-components`.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ByBitIndexPriceComponents {
    pub index_name: String,
    pub last_price: String,
    pub update_time: String,
    pub components: Vec<ByBitIndexPriceComponent>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ByBitIndexPriceComponent {
    pub exchange: String,
    pub spot_pair: String,
    pub equivalent_price: String,
    pub multiplier: String,
    pub price: String,
    pub weight: String,
}

/// `GET /v5/market/price-limit`.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ByBitOrderPriceLimit {
    pub symbol: String,
    /// Highest allowed bid price.
    pub buy_lmt: String,
    /// Lowest allowed ask price.
    pub sell_lmt: String,
    pub ts: String,
}

// ---------------------------------------------------------------------------------------------
// Spread instruments
// ---------------------------------------------------------------------------------------------

/// `GET /v5/spread/instrument`.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ByBitSpreadInstrumentsResult {
    pub list: Vec<ByBitSpreadInstrument>,
    pub next_page_cursor: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ByBitSpreadInstrument {
    pub symbol: String,
    /// `FundingRateArb`, `CarryTrade`, `FutureSpread`, `PerpBasis`.
    pub contract_type: String,
    pub status: String,
    pub base_coin: String,
    pub quote_coin: String,
    pub settle_coin: String,
    pub tick_size: String,
    pub min_price: String,
    pub max_price: String,
    pub lot_size: String,
    pub min_size: String,
    pub max_size: String,
    pub launch_time: String,
    pub delivery_time: String,
    pub legs: Vec<ByBitSpreadLeg>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ByBitSpreadLeg {
    pub symbol: String,
    pub contract_type: String,
}
