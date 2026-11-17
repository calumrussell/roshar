//! Payloads of the public Advanced Trade REST endpoints (`/api/v3/brokerage/market/*`,
//! `/api/v3/brokerage/time`).
//!
//! Field rules: fields the OpenAPI spec marks `required` are plain types. In the nested product
//! detail objects the spec marks nothing `required` (proto3 JSON), so fields that live responses
//! always return are plain types and fields seen as `null` are `Option`. Fields the server
//! returns but the spec does not document are not mapped.

use serde::{Deserialize, Deserializer, Serialize};

/// For fields that the server always sends but that can be `null`: a missing key is an error,
/// `null` is `None`. (A plain `Option` field would accept a missing key.)
fn nullable<'de, D, T>(d: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(d)
}

/// A product from GET /api/v3/brokerage/market/products and
/// GET /api/v3/brokerage/market/products/{product_id}.
///
/// Fields listed as `required` in the Advanced Trade OpenAPI spec
/// (`coinbase.public_api.authed.retail_brokerage_api.Product`) are plain types; a missing one is a
/// parse error. Fields the spec leaves optional are `Option`. Many values are empty strings on
/// some products, e.g. `status`, `base_name` and the 24h figures on FUTURE products.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoinbaseProduct {
    pub product_id: String,
    pub price: String,
    pub price_percentage_change_24h: String,
    pub volume_24h: String,
    pub volume_percentage_change_24h: String,
    pub base_increment: String,
    pub quote_increment: String,
    pub quote_min_size: String,
    pub quote_max_size: String,
    pub base_min_size: String,
    pub base_max_size: String,
    pub base_name: String,
    pub quote_name: String,
    pub watched: bool,
    pub is_disabled: bool,
    pub new: bool,
    /// For example "online" or "delisted". Empty on FUTURE products.
    pub status: String,
    pub cancel_only: bool,
    pub limit_only: bool,
    pub post_only: bool,
    pub trading_disabled: bool,
    pub auction_mode: bool,
    pub base_display_symbol: String,
    pub quote_display_symbol: String,
    /// One of "SPOT", "FUTURE", "EQUITY", "OPTION_GROUP", "FUTURE_GROUP", "UNKNOWN_PRODUCT_TYPE".
    #[serde(deserialize_with = "nullable")]
    pub product_type: Option<String>,
    #[serde(deserialize_with = "nullable")]
    pub quote_currency_id: Option<String>,
    #[serde(deserialize_with = "nullable")]
    pub base_currency_id: Option<String>,
    /// Present (non-null) on FCM futures and on the legacy INTX perpetuals. `null` on SPOT,
    /// EQUITY, and some expired futures.
    #[serde(deserialize_with = "nullable")]
    pub fcm_trading_session_details: Option<CoinbaseFcmTradingSessionDetails>,
    #[serde(deserialize_with = "nullable")]
    pub mid_market_price: Option<String>,
    /// The product id of the unified book that this product uses, e.g. "BTC-USD" for "BTC-USDC".
    #[serde(deserialize_with = "nullable")]
    pub alias: Option<String>,
    /// The product ids that use this product's book.
    #[serde(deserialize_with = "nullable")]
    pub alias_to: Option<Vec<String>>,
    #[serde(deserialize_with = "nullable")]
    pub view_only: Option<bool>,
    /// Tick size of the price.
    #[serde(deserialize_with = "nullable")]
    pub price_increment: Option<String>,
    #[serde(deserialize_with = "nullable")]
    pub display_name: Option<String>,
    /// The spec lists "CBE", "FCM", "INTX", "UNKNOWN_VENUE_TYPE"; live EQUITY products return
    /// "CCM".
    #[serde(deserialize_with = "nullable")]
    pub product_venue: Option<String>,
    #[serde(deserialize_with = "nullable")]
    pub approximate_quote_24h_volume: Option<String>,
    /// RFC 3339. `null` on many FUTURE and EQUITY products.
    #[serde(deserialize_with = "nullable")]
    pub new_at: Option<String>,
    #[serde(deserialize_with = "nullable")]
    pub market_cap: Option<String>,
    #[serde(deserialize_with = "nullable")]
    pub best_bid_price: Option<String>,
    #[serde(deserialize_with = "nullable")]
    pub best_ask_price: Option<String>,
    #[serde(deserialize_with = "nullable")]
    pub high_24h: Option<String>,
    #[serde(deserialize_with = "nullable")]
    pub low_24h: Option<String>,
    /// Present only on FUTURE products.
    pub future_product_details: Option<CoinbaseFutureProductDetails>,
    /// Present only on EQUITY products.
    pub equity_product_details: Option<CoinbaseEquityProductDetails>,
}

/// `fcm_trading_session_details` on a product.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoinbaseFcmTradingSessionDetails {
    pub is_session_open: bool,
    /// RFC 3339. `null` on the INTX perpetuals.
    #[serde(deserialize_with = "nullable")]
    pub open_time: Option<String>,
    /// RFC 3339. `null` on the INTX perpetuals.
    #[serde(deserialize_with = "nullable")]
    pub close_time: Option<String>,
    /// For example "FCM_TRADING_SESSION_STATE_OPEN".
    pub session_state: String,
    pub after_hours_order_entry_disabled: bool,
    /// For example "FCM_TRADING_SESSION_CLOSED_REASON_UNDEFINED".
    pub closed_reason: String,
    /// `null` when no maintenance is scheduled.
    #[serde(deserialize_with = "nullable")]
    pub maintenance: Option<CoinbaseFcmScheduledMaintenance>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoinbaseFcmScheduledMaintenance {
    pub start_time: String,
    pub end_time: String,
}

/// `future_product_details` on a FUTURE product.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoinbaseFutureProductDetails {
    /// For example "cde".
    pub venue: String,
    pub contract_code: String,
    /// RFC 3339. `null` on the INTX perpetuals.
    #[serde(deserialize_with = "nullable")]
    pub contract_expiry: Option<String>,
    pub contract_size: String,
    pub contract_root_unit: String,
    pub group_description: String,
    pub contract_expiry_timezone: String,
    pub group_short_description: String,
    /// For example "MANAGED_BY_FCM".
    pub risk_managed_by: String,
    /// "EXPIRING" or "PERPETUAL" ("UNKNOWN_CONTRACT_EXPIRY_TYPE" otherwise).
    pub contract_expiry_type: String,
    pub perpetual_details: CoinbasePerpetualProductDetails,
    pub contract_display_name: String,
    pub time_to_expiry_ms: String,
    pub non_crypto: bool,
    pub contract_expiry_name: String,
    pub twenty_four_by_seven: bool,
    /// For example "3600s". `null` on products without funding.
    #[serde(deserialize_with = "nullable")]
    pub funding_interval: Option<String>,
    pub open_interest: String,
    pub funding_rate: String,
    /// RFC 3339. `null` on products without funding.
    #[serde(deserialize_with = "nullable")]
    pub funding_time: Option<String>,
    pub display_name: String,
    /// `null` on the INTX perpetuals.
    #[serde(deserialize_with = "nullable")]
    pub intraday_margin_rate: Option<CoinbaseMarginRate>,
    /// `null` on the INTX perpetuals.
    #[serde(deserialize_with = "nullable")]
    pub overnight_margin_rate: Option<CoinbaseMarginRate>,
    pub settlement_price: String,
    /// For example "FUTURES_ASSET_TYPE_CRYPTO".
    pub futures_asset_type: String,
    pub index_price: String,
    pub contract_code_display_name: String,
    pub product_group_cbrn: String,
    pub futures_asset_types: Vec<String>,
    /// For example "TRADING_HOURS_TYPE_UNSPECIFIED".
    pub trading_hours_type: String,
    /// Documented for `product_venue` DERIBIT. `null` on every product returned on 2026-11-16.
    #[serde(deserialize_with = "nullable")]
    pub deribit_product_details: Option<CoinbaseDeribitProductDetails>,
}

/// `perpetual_details` inside `future_product_details`. Values are empty strings on expiring
/// contracts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoinbasePerpetualProductDetails {
    pub open_interest: String,
    pub funding_rate: String,
    /// RFC 3339. `null` on expiring contracts.
    #[serde(deserialize_with = "nullable")]
    pub funding_time: Option<String>,
    pub max_leverage: String,
    pub base_asset_uuid: String,
    pub underlying_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoinbaseMarginRate {
    pub long_margin_rate: String,
    pub short_margin_rate: String,
}

/// `deribit_product_details` inside `future_product_details`. No live product carries this
/// object yet, so every field follows the spec and is `Option`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoinbaseDeribitProductDetails {
    pub instrument_id: Option<String>,
    pub settlement_currency: Option<String>,
    pub counter_currency: Option<String>,
    pub settlement_period: Option<String>,
    pub price_index: Option<String>,
    pub instrument_type: Option<String>,
    pub maker_commission: Option<String>,
    pub taker_commission: Option<String>,
    pub block_trade_commission: Option<String>,
    pub block_trade_tick_size: Option<String>,
    pub block_trade_min_trade_amount: Option<String>,
    pub creation_time: Option<String>,
    pub max_leverage: Option<String>,
    pub max_liquidation_commission: Option<String>,
    pub tick_size_steps: Option<Vec<CoinbaseTickSizeStep>>,
    pub qty_tick_size: Option<String>,
    pub index_id: Option<String>,
    pub product_group: Option<String>,
    pub base_currency_uuid: Option<String>,
    pub quote_currency_uuid: Option<String>,
    pub state: Option<String>,
    pub delivery_price: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoinbaseTickSizeStep {
    pub above_price: Option<String>,
    pub tick_size: Option<String>,
}

/// `equity_product_details` on an EQUITY product. Only the fields in the spec are mapped; the
/// undocumented `fundamentals`, `ipo_details` and price-change fields are ignored.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoinbaseEquityProductDetails {
    /// For example "EQUITY_PRODUCT_SUBTYPE_COMMON_STOCK".
    pub equity_subtype: String,
    pub fractionable: bool,
    pub liquidate_only: bool,
    pub ticker: String,
    pub description: String,
    pub trading_halted: bool,
    /// RFC 3339. `null` unless a halt is recorded.
    #[serde(deserialize_with = "nullable")]
    pub trading_halted_start_time: Option<String>,
    /// RFC 3339. `null` unless a halt has ended.
    #[serde(deserialize_with = "nullable")]
    pub trading_halted_end_time: Option<String>,
    pub fractional_notional_min_size: String,
    pub cik: String,
    pub short_name: String,
    pub company_description: String,
    pub company_website: String,
    pub trading_day_info: CoinbaseEquityTradingDayInfo,
    /// For example "EQUITY_TRADING_SESSION_NORMAL".
    pub current_session: String,
    pub opol: String,
    pub recent_trading_days: CoinbaseEquityTradingDays,
    pub equity_trading_flags: CoinbaseEquityTradingFlags,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoinbaseEquityTradingDayInfo {
    pub venue_id: String,
    /// YYYY-MM-DD.
    pub date: String,
    /// For example "TRADE_DATE_TYPE_REGULAR".
    pub trade_date_type: String,
    pub holiday_name: String,
    pub version_id: String,
    pub trading_sessions: Vec<CoinbaseEquityTradingSessionWindow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoinbaseEquityTradingSessionWindow {
    /// For example "EQUITY_TRADING_SESSION_PRE_MARKET".
    pub session_type: String,
    pub session_start_time: String,
    pub session_end_time: String,
    pub support_fractional: bool,
    pub limit_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoinbaseEquityTradingDays {
    pub trading_days: Vec<CoinbaseEquityTradingDayInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoinbaseEquityTradingFlags {
    pub tradable: bool,
    pub searchable: bool,
    pub buy_enabled: bool,
    pub buy_whole_shares: bool,
    pub buy_fractional_shares: bool,
    pub buy_notional: bool,
    pub sell_enabled: bool,
    pub sell_whole_shares: bool,
    pub sell_fractional_shares: bool,
    pub sell_notional: bool,
}

/// Top-level response from GET /api/v3/brokerage/market/products.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CoinbaseProductsResponse {
    pub products: Vec<CoinbaseProduct>,
    pub num_products: i32,
    /// `null` when the request filters by `product_ids`.
    #[serde(deserialize_with = "nullable")]
    pub pagination: Option<CoinbasePagination>,
}

/// Cursor paging metadata. Pass `next_cursor` as `CoinbaseProductsQuery::cursor` to read the
/// next page.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CoinbasePagination {
    pub prev_cursor: String,
    pub next_cursor: String,
    pub has_next: bool,
    pub has_prev: bool,
}

/// `product_type` filter of the products list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoinbaseProductType {
    Spot,
    Future,
    Equity,
    OptionGroup,
    FutureGroup,
}

impl CoinbaseProductType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Spot => "SPOT",
            Self::Future => "FUTURE",
            Self::Equity => "EQUITY",
            Self::OptionGroup => "OPTION_GROUP",
            Self::FutureGroup => "FUTURE_GROUP",
        }
    }
}

/// `contract_expiry_type` filter. Applies only with `product_type` FUTURE.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoinbaseContractExpiryType {
    Expiring,
    Perpetual,
}

impl CoinbaseContractExpiryType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Expiring => "EXPIRING",
            Self::Perpetual => "PERPETUAL",
        }
    }
}

/// `expiring_contract_status` filter. The server default is unexpired contracts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoinbaseExpiringContractStatus {
    Unexpired,
    Expired,
    All,
}

impl CoinbaseExpiringContractStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Unexpired => "STATUS_UNEXPIRED",
            Self::Expired => "STATUS_EXPIRED",
            Self::All => "STATUS_ALL",
        }
    }
}

/// `products_sort_order`. The server default is 24h quote volume, descending.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoinbaseProductsSortOrder {
    Volume24hDescending,
    ListTimeDescending,
}

impl CoinbaseProductsSortOrder {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Volume24hDescending => "PRODUCTS_SORT_ORDER_VOLUME_24H_DESCENDING",
            Self::ListTimeDescending => "PRODUCTS_SORT_ORDER_LIST_TIME_DESCENDING",
        }
    }
}

/// `futures_underlying_type` filter. Applies only with `product_type` FUTURE.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoinbaseFuturesUnderlyingType {
    Spot,
    Index,
    Equity,
    EquityIndex,
    EquityEtf,
    PreIpo,
    Commod,
    CommodEtf,
    CommodIndex,
    Adr,
    ForeignEquity,
    Otc,
}

impl CoinbaseFuturesUnderlyingType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Spot => "FUTURES_UNDERLYING_TYPE_SPOT",
            Self::Index => "FUTURES_UNDERLYING_TYPE_INDEX",
            Self::Equity => "FUTURES_UNDERLYING_TYPE_EQUITY",
            Self::EquityIndex => "FUTURES_UNDERLYING_TYPE_EQUITY_INDEX",
            Self::EquityEtf => "FUTURES_UNDERLYING_TYPE_EQUITY_ETF",
            Self::PreIpo => "FUTURES_UNDERLYING_TYPE_PREIPO",
            Self::Commod => "FUTURES_UNDERLYING_TYPE_COMMOD",
            Self::CommodEtf => "FUTURES_UNDERLYING_TYPE_COMMOD_ETF",
            Self::CommodIndex => "FUTURES_UNDERLYING_TYPE_COMMOD_INDEX",
            Self::Adr => "FUTURES_UNDERLYING_TYPE_ADR",
            Self::ForeignEquity => "FUTURES_UNDERLYING_TYPE_FOREIGN_EQUITY",
            Self::Otc => "FUTURES_UNDERLYING_TYPE_OTC",
        }
    }
}

/// Query parameters of GET /api/v3/brokerage/market/products. Every field is optional and
/// omitted when `None`; `Default` sends no parameters (the server then returns SPOT only).
#[derive(Debug, Clone, Default)]
pub struct CoinbaseProductsQuery {
    pub limit: Option<u32>,
    pub offset: Option<u32>,
    pub product_type: Option<CoinbaseProductType>,
    /// Sent as repeated `product_ids` parameters. The response then has `pagination: null`.
    pub product_ids: Option<Vec<String>>,
    pub contract_expiry_type: Option<CoinbaseContractExpiryType>,
    pub expiring_contract_status: Option<CoinbaseExpiringContractStatus>,
    /// Documented as "all products of all product types". Unauthenticated, the server returned
    /// SPOT only on 2026-11-16.
    pub get_all_products: Option<bool>,
    pub products_sort_order: Option<CoinbaseProductsSortOrder>,
    /// `pagination.next_cursor` / `prev_cursor` of a previous page.
    pub cursor: Option<String>,
    pub futures_underlying_type: Option<CoinbaseFuturesUnderlyingType>,
    pub user_country_code: Option<String>,
    /// Documented for `product_venue` DERIBIT only; ignored for other venues.
    pub expired: Option<bool>,
}

impl CoinbaseProductsQuery {
    /// Query pairs in the order sent; `None` fields are omitted.
    pub fn to_query(&self) -> Vec<(&'static str, String)> {
        let mut q = Vec::new();
        if let Some(v) = self.limit {
            q.push(("limit", v.to_string()));
        }
        if let Some(v) = self.offset {
            q.push(("offset", v.to_string()));
        }
        if let Some(v) = self.product_type {
            q.push(("product_type", v.as_str().to_string()));
        }
        if let Some(ids) = &self.product_ids {
            for id in ids {
                q.push(("product_ids", id.clone()));
            }
        }
        if let Some(v) = self.contract_expiry_type {
            q.push(("contract_expiry_type", v.as_str().to_string()));
        }
        if let Some(v) = self.expiring_contract_status {
            q.push(("expiring_contract_status", v.as_str().to_string()));
        }
        if let Some(v) = self.get_all_products {
            q.push(("get_all_products", v.to_string()));
        }
        if let Some(v) = self.products_sort_order {
            q.push(("products_sort_order", v.as_str().to_string()));
        }
        if let Some(v) = &self.cursor {
            q.push(("cursor", v.clone()));
        }
        if let Some(v) = self.futures_underlying_type {
            q.push(("futures_underlying_type", v.as_str().to_string()));
        }
        if let Some(v) = &self.user_country_code {
            q.push(("user_country_code", v.clone()));
        }
        if let Some(v) = self.expired {
            q.push(("expired", v.to_string()));
        }
        q
    }
}

/// Response of GET /api/v3/brokerage/market/product_book.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoinbaseProductBookResponse {
    pub pricebook: CoinbasePriceBook,
    pub last: String,
    pub mid_market: String,
    pub spread_bps: String,
    pub spread_absolute: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoinbasePriceBook {
    pub product_id: String,
    pub bids: Vec<CoinbaseBookLevel>,
    pub asks: Vec<CoinbaseBookLevel>,
    /// RFC 3339.
    pub time: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoinbaseBookLevel {
    pub price: String,
    pub size: String,
}

/// Response of GET /api/v3/brokerage/market/products/{product_id}/ticker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoinbaseMarketTradesResponse {
    pub trades: Vec<CoinbaseMarketTrade>,
    pub best_bid: String,
    pub best_ask: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoinbaseMarketTrade {
    pub trade_id: String,
    pub product_id: String,
    pub price: String,
    pub size: String,
    /// RFC 3339.
    pub time: String,
    /// The maker side of the trade.
    pub side: CoinbaseTradeSide,
    /// For example "coinbase".
    pub exchange: String,
}

/// Maker side of a trade (REST `side`, WebSocket `market_trades` `side`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum CoinbaseTradeSide {
    Buy,
    Sell,
}

/// Candle bucket size of GET /api/v3/brokerage/market/products/{product_id}/candles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoinbaseGranularity {
    OneMinute,
    FiveMinute,
    FifteenMinute,
    ThirtyMinute,
    OneHour,
    TwoHour,
    FourHour,
    SixHour,
    OneDay,
}

impl CoinbaseGranularity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::OneMinute => "ONE_MINUTE",
            Self::FiveMinute => "FIVE_MINUTE",
            Self::FifteenMinute => "FIFTEEN_MINUTE",
            Self::ThirtyMinute => "THIRTY_MINUTE",
            Self::OneHour => "ONE_HOUR",
            Self::TwoHour => "TWO_HOUR",
            Self::FourHour => "FOUR_HOUR",
            Self::SixHour => "SIX_HOUR",
            Self::OneDay => "ONE_DAY",
        }
    }
}

/// Response of GET /api/v3/brokerage/market/products/{product_id}/candles. Newest first.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoinbaseCandlesResponse {
    pub candles: Vec<CoinbaseCandle>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoinbaseCandle {
    /// UNIX seconds of the bucket start.
    pub start: String,
    pub low: String,
    pub high: String,
    pub open: String,
    pub close: String,
    pub volume: String,
}

/// Response of GET /api/v3/brokerage/time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoinbaseServerTime {
    pub iso: String,
    #[serde(rename = "epochSeconds")]
    pub epoch_seconds: String,
    #[serde(rename = "epochMillis")]
    pub epoch_millis: String,
}

/// Error body of a failed request, e.g.
/// `{"error":"NOT_FOUND","error_details":"...","message":"..."}`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoinbaseErrorResponse {
    pub error: String,
    pub error_details: Option<String>,
    pub message: String,
}
