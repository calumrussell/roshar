//! USDⓈ-M futures REST payloads (`https://fapi.binance.com`). Decimals stay strings, as sent.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Error body Binance returns with a non-2xx status, e.g. `{"code":-1121,"msg":"Invalid symbol."}`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BinanceApiError {
    pub code: i64,
    pub msg: String,
}

/// For fields that are always sent but may be `null`: the key must be present (no `default`).
fn nullable<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}

/// `GET /fapi/v1/time`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerTime {
    pub server_time: i64,
}

/// `GET /fapi/v1/exchangeInfo`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExchangeInfo {
    pub timezone: String,
    pub server_time: i64,
    /// `U_MARGINED`.
    pub futures_type: String,
    pub rate_limits: Vec<RateLimit>,
    /// Always `[]` in live data; the docs do not describe its elements.
    pub exchange_filters: Vec<serde_json::Value>,
    pub assets: Vec<ExchangeAsset>,
    pub symbols: Vec<SymbolInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RateLimit {
    /// `REQUEST_WEIGHT` or `ORDERS`.
    pub rate_limit_type: String,
    /// `SECOND`, `MINUTE`, ...
    pub interval: String,
    pub interval_num: u32,
    pub limit: u32,
}

/// Margin asset of the multi-assets mode.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExchangeAsset {
    pub asset: String,
    pub margin_available: bool,
    /// Auto-exchange threshold in Multi-Assets margin mode.
    pub auto_asset_exchange: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SymbolInfo {
    pub symbol: String,
    pub pair: String,
    /// `PERPETUAL`, `CURRENT_QUARTER`, `NEXT_QUARTER`, `TRADIFI_PERPETUAL`, ...
    pub contract_type: String,
    pub delivery_date: i64,
    pub onboard_date: i64,
    /// `TRADING`, `PENDING_TRADING`, `SETTLING`, ...
    pub status: String,
    pub maint_margin_percent: String,
    pub required_margin_percent: String,
    pub base_asset: String,
    pub quote_asset: String,
    pub margin_asset: String,
    pub price_precision: u32,
    pub quantity_precision: u32,
    pub base_asset_precision: u32,
    pub quote_precision: u32,
    /// `COIN`, `INDEX`, `EQUITY`, `COMMODITY`, `FX`, `PREMARKET`, ...
    pub underlying_type: String,
    pub underlying_sub_type: Vec<String>,
    pub trigger_protect: String,
    pub liquidation_fee: String,
    pub market_take_bound: String,
    pub max_move_order_limit: u32,
    pub filters: Vec<SymbolFilter>,
    pub order_types: Vec<String>,
    pub time_in_force: Vec<String>,
    pub permission_sets: Vec<String>,
}

/// Symbol filter, tagged by `filterType`. Filter types this crate does not know parse as `Other`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "filterType")]
pub enum SymbolFilter {
    #[serde(rename = "PRICE_FILTER", rename_all = "camelCase")]
    PriceFilter {
        min_price: String,
        max_price: String,
        tick_size: String,
    },
    #[serde(rename = "LOT_SIZE", rename_all = "camelCase")]
    LotSize {
        min_qty: String,
        max_qty: String,
        step_size: String,
    },
    #[serde(rename = "MARKET_LOT_SIZE", rename_all = "camelCase")]
    MarketLotSize {
        min_qty: String,
        max_qty: String,
        step_size: String,
    },
    #[serde(rename = "MAX_NUM_ORDERS")]
    MaxNumOrders { limit: u32 },
    #[serde(rename = "MAX_NUM_ALGO_ORDERS")]
    MaxNumAlgoOrders { limit: u32 },
    #[serde(rename = "MIN_NOTIONAL")]
    MinNotional { notional: String },
    #[serde(rename = "PERCENT_PRICE", rename_all = "camelCase")]
    PercentPrice {
        multiplier_up: String,
        multiplier_down: String,
        multiplier_decimal: String,
    },
    /// `positionControlSide`: `NONE`, `BOTH`, ...
    #[serde(rename = "POSITION_RISK_CONTROL", rename_all = "camelCase")]
    PositionRiskControl { position_control_side: String },
    #[serde(other)]
    Other,
}

/// `GET /fapi/v1/depth` and `GET /fapi/v1/rpiDepth`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BinanceOrderBookSnapshot {
    #[serde(rename = "lastUpdateId")]
    pub last_update_id: u64,
    /// Message output time.
    #[serde(rename = "E")]
    pub event_time: i64,
    /// Transaction time.
    #[serde(rename = "T")]
    pub transaction_time: i64,
    /// `[price, quantity]`.
    pub bids: Vec<[String; 2]>,
    pub asks: Vec<[String; 2]>,
}

/// `GET /fapi/v1/trades` and `GET /fapi/v1/historicalTrades`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BinanceTrade {
    pub id: i64,
    pub price: String,
    pub qty: String,
    pub quote_qty: String,
    pub time: i64,
    pub is_buyer_maker: bool,
    #[serde(rename = "isRPITrade")]
    pub is_rpi_trade: bool,
}

/// `GET /fapi/v1/aggTrades`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BinanceAggTrade {
    #[serde(rename = "a")]
    pub agg_trade_id: i64,
    #[serde(rename = "p")]
    pub price: String,
    #[serde(rename = "q")]
    pub quantity: String,
    /// Quantity without RPI orders.
    #[serde(rename = "nq")]
    pub normal_quantity: String,
    #[serde(rename = "f")]
    pub first_trade_id: i64,
    #[serde(rename = "l")]
    pub last_trade_id: i64,
    #[serde(rename = "T")]
    pub timestamp: i64,
    #[serde(rename = "m")]
    pub is_buyer_maker: bool,
}

/// `GET /fapi/v1/premiumIndex`: mark price, index price and funding.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BinancePremiumIndex {
    pub symbol: String,
    pub mark_price: String,
    pub index_price: String,
    /// Only meaningful in the last hour before delivery/settlement.
    pub estimated_settle_price: String,
    /// `0` for delivery contracts.
    pub last_funding_rate: String,
    pub interest_rate: String,
    /// `0` for delivery contracts.
    pub next_funding_time: i64,
    pub time: i64,
}

/// `GET /fapi/v1/fundingRate`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BinanceHistoricalFundingRate {
    pub symbol: String,
    pub funding_rate: String,
    pub funding_time: i64,
    /// Mark price at the funding charge. Always present; Binance returns `""` for old records
    /// (e.g. 2019 BTCUSDT entries).
    pub mark_price: String,
    /// `Regular` for the normal funding rate; `Special` for the additional funding rate generated
    /// by stock dividends (added 2026-07-23).
    pub rate_type: String,
}

/// `GET /fapi/v1/fundingInfo`: symbols with an adjusted funding cap/floor or interval.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FundingInfo {
    pub symbol: String,
    pub adjusted_funding_rate_cap: String,
    pub adjusted_funding_rate_floor: String,
    pub funding_interval_hours: u32,
    pub disclaimer: bool,
    /// Always sent; `null` on some symbols (58 of 824 live on 2026-11-16).
    #[serde(deserialize_with = "nullable")]
    pub update_time: Option<i64>,
}

/// `GET /fapi/v1/ticker/24hr`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TickerData {
    pub symbol: String,
    pub price_change: String,
    pub price_change_percent: String,
    pub weighted_avg_price: String,
    pub last_price: String,
    pub last_qty: String,
    pub open_price: String,
    pub high_price: String,
    pub low_price: String,
    pub volume: String,
    pub quote_volume: String,
    pub open_time: i64,
    pub close_time: i64,
    pub first_id: i64,
    pub last_id: i64,
    pub count: u64,
}

/// `GET /fapi/v1/ticker/price` and `GET /fapi/v2/ticker/price`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceTicker {
    pub symbol: String,
    pub price: String,
    pub time: i64,
}

/// `GET /fapi/v1/ticker/bookTicker`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BookTicker {
    pub symbol: String,
    pub bid_price: String,
    pub bid_qty: String,
    pub ask_price: String,
    pub ask_qty: String,
    pub time: i64,
    pub last_update_id: u64,
}

/// `GET /fapi/v1/openInterest`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenInterestData {
    pub symbol: String,
    pub open_interest: String,
    pub time: i64,
}

/// `GET /futures/data/delivery-price`. `deliveryPrice` is a JSON number on this endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeliveryPrice {
    pub delivery_time: i64,
    pub delivery_price: f64,
}

/// `GET /futures/data/openInterestHist`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenInterestHist {
    pub symbol: String,
    pub sum_open_interest: String,
    pub sum_open_interest_value: String,
    #[serde(rename = "CMCCirculatingSupply")]
    pub cmc_circulating_supply: String,
    pub timestamp: i64,
}

/// `GET /futures/data/topLongShortAccountRatio`, `topLongShortPositionRatio` and
/// `globalLongShortAccountRatio`. The position-ratio endpoint also names its fields
/// `longAccount`/`shortAccount`; they are position shares there.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LongShortRatio {
    pub symbol: String,
    pub long_short_ratio: String,
    pub long_account: String,
    pub short_account: String,
    pub timestamp: i64,
}

/// `GET /futures/data/takerlongshortRatio`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TakerBuySellVolume {
    pub buy_sell_ratio: String,
    pub buy_vol: String,
    pub sell_vol: String,
    pub timestamp: i64,
}

/// `GET /futures/data/basis`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Basis {
    pub index_price: String,
    pub contract_type: String,
    pub basis_rate: String,
    pub futures_price: String,
    /// `""` for perpetuals (seen live).
    pub annualized_basis_rate: String,
    pub basis: String,
    pub pair: String,
    pub timestamp: i64,
}

/// `GET /fapi/v1/indexInfo`: composite index symbols.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompositeIndexInfo {
    pub symbol: String,
    pub time: i64,
    /// Component asset type, `baseAsset` or `quoteAsset`.
    pub component: String,
    pub base_asset_list: Vec<CompositeIndexComponent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompositeIndexComponent {
    pub base_asset: String,
    pub quote_asset: String,
    pub weight_in_quantity: String,
    pub weight_in_percentage: String,
}

/// `GET /fapi/v1/assetIndex`: multi-assets mode asset index.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetIndex {
    pub symbol: String,
    pub time: i64,
    pub index: String,
    pub bid_buffer: String,
    pub ask_buffer: String,
    pub bid_rate: String,
    pub ask_rate: String,
    pub auto_exchange_bid_buffer: String,
    pub auto_exchange_ask_buffer: String,
    pub auto_exchange_bid_rate: String,
    pub auto_exchange_ask_rate: String,
}

/// `GET /fapi/v1/constituents`: index price constituents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexConstituents {
    pub symbol: String,
    pub time: i64,
    pub constituents: Vec<IndexConstituent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexConstituent {
    pub exchange: String,
    pub symbol: String,
    pub price: String,
    pub weight: String,
}

/// `GET /fapi/v1/insuranceBalance`: one insurance fund shared by `symbols`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InsuranceBalance {
    pub symbols: Vec<String>,
    pub assets: Vec<InsuranceAsset>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InsuranceAsset {
    pub asset: String,
    pub margin_balance: String,
    pub update_time: i64,
}

/// `GET /fapi/v1/symbolAdlRisk`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdlRisk {
    pub symbol: String,
    /// `LOW`, `MEDIUM`, `HIGH`.
    pub adl_risk: String,
    pub update_time: i64,
}

/// `GET /fapi/v1/tradingSchedule`: sessions of TradFi underlying markets.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TradingSchedule {
    pub update_time: i64,
    /// Keyed by market (`EQUITY`, `COMMODITY`, `FX`, `CN_EQUITY`, ...).
    pub market_schedules: HashMap<String, MarketSchedule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketSchedule {
    pub sessions: Vec<TradingSession>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TradingSession {
    pub start_time: i64,
    pub end_time: i64,
    /// `REGULAR`, `PRE_MARKET`, `AFTER_MARKET`, `OVERNIGHT`, `NO_TRADING`.
    #[serde(rename = "type")]
    pub session_type: String,
}

/// Period of the `/futures/data/*` statistics endpoints.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinanceStatsPeriod {
    M5,
    M15,
    M30,
    H1,
    H2,
    H4,
    H6,
    H12,
    D1,
}

impl BinanceStatsPeriod {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::M5 => "5m",
            Self::M15 => "15m",
            Self::M30 => "30m",
            Self::H1 => "1h",
            Self::H2 => "2h",
            Self::H4 => "4h",
            Self::H6 => "6h",
            Self::H12 => "12h",
            Self::D1 => "1d",
        }
    }
}

/// `contractType` of continuous klines, basis and continuous-kline streams.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinanceContractType {
    Perpetual,
    CurrentQuarter,
    NextQuarter,
    TradifiPerpetual,
}

impl BinanceContractType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Perpetual => "PERPETUAL",
            Self::CurrentQuarter => "CURRENT_QUARTER",
            Self::NextQuarter => "NEXT_QUARTER",
            Self::TradifiPerpetual => "TRADIFI_PERPETUAL",
        }
    }
}
