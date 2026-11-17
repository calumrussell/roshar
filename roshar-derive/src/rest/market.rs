use roshar_http::RateLimitedClient;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

// JSON-RPC over HTTP: the POST body is the method's `params` object and the
// reply is `{ "id", "result" }` or `{ "error": { code, message, data } }`,
// both with HTTP 200. https://docs.derive.xyz/json-rpc

#[derive(Debug, Deserialize)]
struct RpcResponse<T> {
    result: Option<T>,
    error: Option<RpcError>,
}

#[derive(Debug, Deserialize)]
struct RpcError {
    code: i64,
    message: String,
    data: Option<String>,
}

/// Nullable field the exchange always sends: `null` maps to `None`, a missing
/// key is an error (use without `#[serde(default)]`).
fn nullable<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> Result<Option<T>, D::Error> {
    Option::<T>::deserialize(d)
}

// public/get_all_instruments

#[derive(Debug, Serialize)]
struct GetAllInstrumentsParams<'a> {
    instrument_type: &'a str,
    expired: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    currency: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    risk_universe_id: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    page: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    page_size: Option<u64>,
}

/// `Instrument` from `public/get_all_instruments`. Exactly one of
/// `option_details` / `perp_details` / `erc20_details` is non-null, matching
/// `instrument_type`.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeriveInstrument {
    pub instrument_name: String,
    /// `"option"`, `"perp"` or `"erc20"`.
    pub instrument_type: String,
    pub risk_universe_id: u32,
    pub scheduled_activation: i64,
    pub scheduled_deactivation: i64,
    pub is_active: bool,
    pub tick_size: String,
    pub minimum_amount: String,
    pub maximum_amount: String,
    pub amount_step: String,
    pub mark_price_fee_rate_cap: Option<String>,
    pub maker_fee_rate: String,
    pub taker_fee_rate: String,
    pub base_fee: String,
    pub base_currency: String,
    pub quote_currency: String,
    pub option_details: Option<DeriveOptionDetails>,
    pub perp_details: Option<DerivePerpDetails>,
    pub erc20_details: Option<DeriveErc20Details>,
    pub base_asset_address: String,
    pub base_asset_sub_id: String,
    pub pro_rata_fraction: String,
    pub fifo_min_allocation: String,
    pub pro_rata_amount_step: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeriveOptionDetails {
    pub index: String,
    pub expiry: i64,
    pub strike: String,
    /// `"C"` or `"P"`.
    pub option_type: String,
    /// Null until the option settles.
    pub settlement_price: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DerivePerpDetails {
    pub index: String,
    pub max_rate_per_hour: String,
    pub min_rate_per_hour: String,
    pub aggregate_funding: String,
    pub funding_rate: String,
    /// Null when no SRM in the instrument's risk universe prices its currency.
    pub srm_perp_margin_requirements: Option<DeriveSrmPerpMarginRequirements>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeriveSrmPerpMarginRequirements {
    pub im_perp_req: String,
    pub mm_perp_req: String,
    pub max_leverage: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeriveErc20Details {
    pub decimals: u8,
    pub underlying_erc20_address: String,
    pub borrow_index: String,
    pub supply_index: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DerivePagination {
    pub count: u64,
    pub num_pages: u64,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeriveAllInstrumentsResponse {
    pub instruments: Vec<DeriveInstrument>,
    pub pagination: DerivePagination,
}

// public/get_funding_rate_history

#[derive(Debug, Serialize)]
struct GetFundingRateHistoryParams<'a> {
    instrument_name: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    start_timestamp: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    end_timestamp: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    period: Option<i64>,
}

/// Funding-rate OHLC candle. `funding_rate` mirrors `close`; `timestamp` is
/// the bucket start in ms.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeriveFundingRate {
    pub timestamp: i64,
    pub risk_universe_id: u32,
    pub currency: String,
    pub open: String,
    pub high: String,
    pub low: String,
    pub close: String,
    pub funding_rate: String,
}

#[derive(Debug, Deserialize)]
pub struct DeriveFundingRateHistoryResponse {
    pub funding_rate_history: Vec<DeriveFundingRate>,
}

// public/get_tickers

#[derive(Debug, Serialize)]
struct GetTickersParams<'a> {
    instrument_type: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    currency: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    expiry_date: Option<i64>,
}

/// `DailyTradingStatistics` (`stats` of the slim ticker), last 24h.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeriveStats {
    /// `c`: contracts traded.
    #[serde(rename = "c")]
    pub contracts_traded: String,
    /// `v`: notional volume.
    #[serde(rename = "v")]
    pub notional_volume: String,
    /// `pr`: premium volume.
    #[serde(rename = "pr")]
    pub premium_volume: String,
    /// `n`: number of trades.
    #[serde(rename = "n")]
    pub trade_count: u64,
    /// `oi`: current total open interest.
    #[serde(rename = "oi")]
    pub open_interest: String,
    /// `h`: highest trade price.
    #[serde(rename = "h")]
    pub high: String,
    /// `l`: lowest trade price.
    #[serde(rename = "l")]
    pub low: String,
    /// `p`: options: % change in premium; perps: % change in mark price.
    #[serde(rename = "p")]
    pub percent_change: String,
}

/// `OptionPricing` of the slim ticker.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeriveOptionPricing {
    #[serde(rename = "d")]
    pub delta: String,
    #[serde(rename = "g")]
    pub gamma: String,
    #[serde(rename = "t")]
    pub theta: String,
    #[serde(rename = "v")]
    pub vega: String,
    #[serde(rename = "r")]
    pub rho: String,
    /// `i`: implied volatility.
    #[serde(rename = "i")]
    pub iv: String,
    /// `bi`: implied volatility of the best bid.
    #[serde(rename = "bi")]
    pub bid_iv: String,
    /// `ai`: implied volatility of the best ask.
    #[serde(rename = "ai")]
    pub ask_iv: String,
    /// `f`: forward price.
    #[serde(rename = "f")]
    pub forward_price: String,
    /// `df`: discount factor.
    #[serde(rename = "df")]
    pub discount_factor: String,
    /// `m`: option mark price.
    #[serde(rename = "m")]
    pub mark_price: String,
}

/// `TickerSlimSnapshot`: one value of the `public/get_tickers` map (keyed by
/// instrument name), same payload as the `ticker_slim` channel.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeriveTicker {
    /// `t`: snapshot time, ms.
    #[serde(rename = "t")]
    pub timestamp: i64,
    #[serde(rename = "b")]
    pub best_bid_price: String,
    #[serde(rename = "B")]
    pub best_bid_amount: String,
    #[serde(rename = "a")]
    pub best_ask_price: String,
    #[serde(rename = "A")]
    pub best_ask_amount: String,
    #[serde(rename = "I")]
    pub index_price: String,
    #[serde(rename = "M")]
    pub mark_price: String,
    /// `minp`: minimum matchable price.
    #[serde(rename = "minp")]
    pub min_price: String,
    /// `maxp`: maximum matchable price.
    #[serde(rename = "maxp")]
    pub max_price: String,
    /// `f`: current hourly funding rate (perps only; null otherwise).
    #[serde(rename = "f")]
    pub funding_rate: Option<String>,
    /// Options only; null otherwise.
    pub option_pricing: Option<DeriveOptionPricing>,
    pub stats: DeriveStats,
}

#[derive(Debug, Deserialize)]
struct DeriveTickersResult {
    tickers: HashMap<String, DeriveTicker>,
}

pub type DeriveTickersResponse = HashMap<String, DeriveTicker>;

// public/get_instrument, public/get_ticker, public/get_currency,
// public/get_option_settlement_prices: single required name parameter.

#[derive(Debug, Serialize)]
struct InstrumentNameParams<'a> {
    instrument_name: &'a str,
}

#[derive(Debug, Serialize)]
struct CurrencyParams<'a> {
    currency: &'a str,
}

#[derive(Debug, Serialize)]
struct NoParams {}

// public/get_trade_history

/// Filters for `public/get_trade_history`. Every field is optional and
/// omitted when `None`; `trade_id` overrides all other filters.
#[derive(Debug, Serialize, Default, Clone)]
pub struct DeriveTradeHistoryParams<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instrument_name: Option<&'a str>,
    /// `"erc20"`, `"option"` or `"perp"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instrument_type: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub currency: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subaccount_id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trade_id: Option<&'a str>,
    /// `BatchStatus` filter, e.g. `"Batching"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub batch_status: Option<&'a str>,
    /// UTC ms.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_timestamp: Option<i64>,
    /// UTC ms.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_timestamp: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_size: Option<u64>,
}

/// Public, anonymized trade. Same shape as the `trades.*` channel items.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeriveTrade {
    pub trade_id: String,
    pub instrument_name: String,
    /// ms.
    pub timestamp: i64,
    pub trade_price: String,
    pub trade_amount: String,
    pub mark_price: String,
    pub index_price: String,
    /// `"buy"` or `"sell"`.
    pub direction: String,
    /// `"maker"` or `"taker"`.
    pub liquidity_role: String,
    #[serde(deserialize_with = "nullable")]
    pub quote_id: Option<String>,
    #[serde(deserialize_with = "nullable")]
    pub rfq_id: Option<String>,
    #[serde(deserialize_with = "nullable")]
    pub block_trade_id: Option<String>,
    pub wallet: String,
    pub subaccount_id: u64,
    /// Settlement batch status; absent on the websocket trade channels.
    #[serde(default)]
    pub batch_status: Option<String>,
    /// Settling transaction; empty until the batch settles. Absent on the
    /// websocket trade channels.
    #[serde(default)]
    pub tx_hash: Option<String>,
    pub trade_fee: String,
    pub expected_rebate: String,
    pub realized_pnl: String,
    pub realized_pnl_excl_fees: String,
    pub extra_fee: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeriveTradeHistoryResponse {
    pub trades: Vec<DeriveTrade>,
    pub pagination: DerivePagination,
}

// public/get_all_currencies, public/get_currency

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeriveOpenInterestStats {
    pub current_open_interest: String,
    pub interest_cap: String,
}

/// `Erc20Details` of a spot asset / manager collateral.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeriveAssetErc20 {
    pub decimals: u8,
    /// Absent for assets without an underlying token.
    #[serde(default)]
    pub underlying_erc20: Option<String>,
}

/// `UniverseManagers`: managers pricing a currency in one universe.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeriveCurrencyManagers {
    pub risk_universe_id: u32,
    #[serde(default)]
    pub risk_universe_name: Option<String>,
    /// SRM manager id.
    #[serde(default)]
    pub sm: Option<u32>,
    /// PMRM2 manager id.
    #[serde(default)]
    pub pm: Option<u32>,
}

/// `AssetUniverse` of an option/perp asset.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeriveAssetUniverse {
    pub risk_universe_id: u32,
    #[serde(default)]
    pub risk_universe_name: Option<String>,
    pub oi: DeriveOpenInterestStats,
    /// Absent on option assets and where no SRM prices the currency.
    #[serde(default)]
    pub srm_perp_margin_requirements: Option<DeriveSrmPerpMarginRequirements>,
}

/// `AssetEntry`: the option or perp asset of a currency.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeriveAssetEntry {
    pub name: String,
    pub address: String,
    pub universes: Vec<DeriveAssetUniverse>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeriveLendingDetails {
    pub borrow_apy: String,
    pub supply_apy: String,
    pub total_borrow: String,
    pub total_borrow_cap: String,
}

/// `SpotUniverse`: a spot asset's risk details within one universe.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeriveSpotUniverse {
    pub risk_universe_id: u32,
    pub oi: DeriveOpenInterestStats,
    pub pm2_im_discount: String,
    pub pm2_mm_discount: String,
    pub srm_im_discount: String,
    pub srm_mm_discount: String,
    /// Null if the asset has no lending pool in this universe.
    #[serde(deserialize_with = "nullable")]
    pub lending: Option<DeriveLendingDetails>,
}

/// `SpotAssetEntry`.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeriveSpotAsset {
    pub name: String,
    pub address: String,
    pub erc20: DeriveAssetErc20,
    pub min_deposit_usd: String,
    pub universes: Vec<DeriveSpotUniverse>,
}

/// `Currency` from `public/get_currency` / `public/get_all_currencies`.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeriveCurrency {
    pub currency: String,
    /// `ALL`, `SRM_BASE_ONLY`, `SRM_OPTION_ONLY`, `SRM_PERP_ONLY` or `CASH`.
    pub market_type: String,
    pub spot_price: String,
    #[serde(deserialize_with = "nullable")]
    pub spot_price_24h: Option<String>,
    pub managers: Vec<DeriveCurrencyManagers>,
    /// Absent when no option asset is registered.
    #[serde(default)]
    pub option: Option<DeriveAssetEntry>,
    /// Absent when no perp asset is registered.
    #[serde(default)]
    pub perp: Option<DeriveAssetEntry>,
    pub spot: Vec<DeriveSpotAsset>,
}

// public/get_risk_universes

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeriveManagerCollateral {
    pub name: String,
    pub address: String,
    pub erc20: DeriveAssetErc20,
    pub min_deposit_usd: String,
    pub im_discount: String,
    pub mm_discount: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeriveRiskUniverseManager {
    pub manager_id: u32,
    /// `"SM"` or `"PM"`.
    pub margin_type: String,
    /// Derivative asset names, e.g. `"BTC-OPTION"`, `"BTC-PERP"`.
    pub instruments: Vec<String>,
    pub collaterals: Vec<DeriveManagerCollateral>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeriveSecurityModule {
    pub subaccount_id: u64,
    pub cash_asset: String,
    pub cash_currency: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeriveRiskUniverse {
    pub risk_universe_id: u32,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    pub managers: Vec<DeriveRiskUniverseManager>,
    pub security_module: DeriveSecurityModule,
}

// public/get_index_chart_data, public/get_tradingview_chart_data

#[derive(Debug, Serialize)]
struct IndexChartParams<'a> {
    currency: &'a str,
    start_timestamp: i64,
    end_timestamp: i64,
    period: i64,
}

#[derive(Debug, Serialize)]
struct TradingViewChartParams<'a> {
    instrument_name: &'a str,
    start_timestamp: i64,
    end_timestamp: i64,
    period: i64,
}

/// Spot index OHLC candle. Timestamps are UTC seconds.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeriveIndexCandle {
    pub timestamp: i64,
    pub timestamp_bucket: i64,
    pub open_price: String,
    pub high_price: String,
    pub low_price: String,
    pub close_price: String,
    pub price: String,
}

/// Traded-price OHLCV candle. Timestamps are UTC seconds.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeriveTradeCandle {
    pub timestamp: i64,
    pub timestamp_bucket: i64,
    pub open_price: String,
    pub high_price: String,
    pub low_price: String,
    pub close_price: String,
    pub volume_usd: String,
    pub volume_contracts: String,
}

// public/get_interest_rate_history

#[derive(Debug, Serialize)]
struct InterestRateHistoryParams<'a> {
    currency: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    start_timestamp: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    end_timestamp: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    period: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    risk_universe_id: Option<u32>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeriveOhlc {
    pub open: String,
    pub high: String,
    pub low: String,
    pub close: String,
}

/// Lending-pool APY candle. `timestamp` is UTC ms.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeriveInterestRateCandle {
    pub timestamp: i64,
    pub risk_universe_id: u32,
    pub borrow_apy: DeriveOhlc,
    pub supply_apy: DeriveOhlc,
    pub total_borrow: String,
    pub total_supply: String,
}

#[derive(Debug, Deserialize)]
struct InterestRateHistoryResult {
    interest_rate_history: Vec<DeriveInterestRateCandle>,
}

// public/get_option_settlement_prices

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeriveOptionSettlement {
    /// `YYYYMMDD`.
    pub expiry_date: String,
    pub utc_expiry_sec: i64,
    /// Documented nullable; only settled expiries are returned.
    #[serde(deserialize_with = "nullable")]
    pub price: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OptionSettlementResult {
    expiries: Vec<DeriveOptionSettlement>,
}

// public/get_perp_impact_twap

#[derive(Debug, Serialize)]
struct PerpImpactTwapParams<'a> {
    currency: &'a str,
    start_time: i64,
    end_time: i64,
}

/// TWAPs of perp mid / ask impact / bid impact minus spot.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DerivePerpImpactTwap {
    pub currency: String,
    pub mid_price_diff_twap: String,
    pub ask_impact_diff_twap: String,
    pub bid_impact_diff_twap: String,
}

// public/get_liquidation_history

#[derive(Debug, Serialize)]
struct LiquidationHistoryParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    start_timestamp: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    end_timestamp: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    subaccount_id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    page: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    page_size: Option<u64>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeriveLiquidationBid {
    pub bidder_id: u64,
    pub operation_id: String,
    pub risk_universe_id: u32,
    /// ms.
    pub timestamp: i64,
    pub tx_hash: String,
    pub unit_price: String,
    pub percent_liquidated: String,
    pub cash_received: String,
    /// Balance change per asset name.
    pub amounts_liquidated: HashMap<String, String>,
    pub discount_pnl: String,
    pub realized_pnl: String,
    pub realized_pnl_excl_fees: String,
    pub positions_realized_pnl: HashMap<String, String>,
    pub positions_realized_pnl_excl_fees: HashMap<String, String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeriveLiquidationAuction {
    /// `{subaccount_id}-{start_time_sec}`.
    pub auction_id: String,
    /// `"solvent"` or `"insolvent"`.
    pub auction_type: String,
    pub subaccount_id: u64,
    pub risk_universe_id: u32,
    /// ms.
    pub start_timestamp: i64,
    /// ms; null while the auction is live.
    #[serde(deserialize_with = "nullable")]
    pub end_timestamp: Option<i64>,
    pub fee: String,
    pub tx_hash: String,
    pub bids: Vec<DeriveLiquidationBid>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeriveLiquidationHistoryResponse {
    pub auctions: Vec<DeriveLiquidationAuction>,
    pub pagination: DerivePagination,
}

// public/get_live_auctions

/// Open liquidation auction priced for a maximal bid.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeriveLiveAuction {
    pub subaccount_id: u64,
    pub risk_universe_id: u32,
    /// ms.
    pub timestamp: i64,
    pub margin_type: String,
    /// Only set when the account is not on standard margin.
    #[serde(default)]
    pub currency: Option<String>,
    pub estimated_bid_price: String,
    pub estimated_discount_pnl: String,
    pub estimated_mtm: String,
    pub estimated_percent_bid: String,
    pub min_price_limit: String,
    pub subaccount_balances: HashMap<String, String>,
}

#[derive(Debug, Deserialize)]
struct LiveAuctionsResult {
    auctions: Vec<DeriveLiveAuction>,
}

// public/get_latest_signed_feeds

#[derive(Debug, Serialize)]
struct SignedFeedsParams<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    currency: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    expiry: Option<i64>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeriveOracleSignatures {
    pub signers: Vec<String>,
    pub signatures: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeriveSpotFeedData {
    pub currency: String,
    pub price: String,
    pub confidence: String,
    /// UTC seconds.
    pub timestamp: i64,
    pub deadline: i64,
    pub signatures: DeriveOracleSignatures,
    #[serde(default)]
    pub feed_source_type: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeriveForwardFeedData {
    pub currency: String,
    pub expiry: i64,
    pub fwd_diff: String,
    pub spot_aggregate_start: String,
    pub spot_aggregate_latest: String,
    pub confidence: String,
    pub timestamp: i64,
    pub deadline: i64,
    pub signatures: DeriveOracleSignatures,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeriveSviParams {
    #[serde(rename = "SVI_a")]
    pub a: String,
    #[serde(rename = "SVI_b")]
    pub b: String,
    #[serde(rename = "SVI_fwd")]
    pub fwd: String,
    #[serde(rename = "SVI_m")]
    pub m: String,
    #[serde(rename = "SVI_refTau")]
    pub ref_tau: String,
    #[serde(rename = "SVI_rho")]
    pub rho: String,
    #[serde(rename = "SVI_sigma")]
    pub sigma: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeriveVolFeedData {
    pub currency: String,
    pub expiry: i64,
    pub vol_data: DeriveSviParams,
    pub confidence: String,
    pub timestamp: i64,
    pub deadline: i64,
    pub signatures: DeriveOracleSignatures,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeriveRateFeedData {
    pub currency: String,
    pub expiry: i64,
    pub rate: String,
    pub confidence: String,
    pub timestamp: i64,
    pub deadline: i64,
    pub signatures: DeriveOracleSignatures,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DerivePerpFeedData {
    pub currency: String,
    /// Feed type, e.g. `"P"`.
    #[serde(rename = "type")]
    pub feed_type: String,
    pub spot_diff_value: String,
    pub confidence: String,
    pub timestamp: i64,
    pub deadline: i64,
    pub signatures: DeriveOracleSignatures,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeriveFundingFeedData {
    pub currency: String,
    pub funding_rate: String,
    pub confidence: String,
    pub timestamp: i64,
    pub deadline: i64,
    pub signatures: DeriveOracleSignatures,
}

/// Latest oracle-signed feeds. Expiry-keyed maps use the expiry in UTC
/// seconds as a string key.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeriveSignedFeeds {
    /// currency -> feed.
    pub spot_data: HashMap<String, DeriveSpotFeedData>,
    /// currency -> expiry -> feed.
    pub fwd_data: HashMap<String, HashMap<String, DeriveForwardFeedData>>,
    /// currency -> expiry -> feed.
    pub vol_data: HashMap<String, HashMap<String, DeriveVolFeedData>>,
    /// currency -> expiry -> feed.
    pub rate_data: HashMap<String, HashMap<String, DeriveRateFeedData>>,
    /// currency -> feed type -> feed.
    pub perp_data: HashMap<String, HashMap<String, DerivePerpFeedData>>,
    /// currency -> feed.
    pub funding_data: HashMap<String, DeriveFundingFeedData>,
}

// public/get_live_incidents

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeriveIncident {
    pub creation_timestamp_sec: i64,
    pub label: String,
    pub message: String,
    /// `"manual"` or `"auto"`.
    pub monitor_type: String,
    /// `"low"`, `"medium"` or `"high"`.
    pub severity: String,
}

#[derive(Debug, Deserialize)]
struct LiveIncidentsResult {
    incidents: Vec<DeriveIncident>,
}

/// Derive (formerly Lyra) v3 market data over JSON-RPC HTTP POST.
pub struct MarketApi {
    client: Arc<RateLimitedClient>,
}

impl MarketApi {
    pub fn new(requests_per_second: u32) -> Self {
        Self {
            client: Arc::new(RateLimitedClient::new(requests_per_second, 1)),
        }
    }

    async fn call<P: Serialize, R: DeserializeOwned>(
        &self,
        method: &str,
        params: &P,
    ) -> Result<R, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/{method}", super::DERIVE_REST_URL);
        let response = self.client.post(&url).await.json(params).send().await?;

        let status = response.status();
        let text = response.text().await?;
        if !status.is_success() {
            return Err(format!("Derive {method} failed with status {status}: {text}").into());
        }

        let rpc: RpcResponse<R> = serde_json::from_str(&text)
            .map_err(|e| format!("Failed to parse Derive {method} response: {e}"))?;
        match (rpc.result, rpc.error) {
            (_, Some(err)) => Err(format!(
                "Derive {method} error {}: {} ({})",
                err.code,
                err.message,
                err.data.as_deref().unwrap_or("")
            )
            .into()),
            (Some(result), None) => Ok(result),
            (None, None) => {
                Err(format!("Derive {method} response has neither result nor error").into())
            }
        }
    }

    /// `public/get_all_instruments`: one page of instrument definitions,
    /// name-descending. For options, `expired` selects live (`false`) or
    /// only expired (`true`) options; perps and spot ignore it. `page_size`
    /// is capped at 1000 by the exchange. An empty listing is returned as
    /// exchange error 12001 (`instrument_not_found`).
    pub async fn get_all_instruments(
        &self,
        instrument_type: &str,
        currency: Option<&str>,
        expired: bool,
        risk_universe_id: Option<u32>,
        page: Option<u64>,
        page_size: Option<u64>,
    ) -> Result<DeriveAllInstrumentsResponse, Box<dyn std::error::Error + Send + Sync>> {
        self.call(
            "public/get_all_instruments",
            &GetAllInstrumentsParams {
                instrument_type,
                expired,
                currency,
                risk_universe_id,
                page,
                page_size,
            },
        )
        .await
    }

    /// `public/get_funding_rate_history`: funding-rate OHLC candles for a
    /// perp. Timestamps are UTC ms; `period` is the candle size in seconds
    /// (exchange default 3600).
    pub async fn get_funding_rate_history(
        &self,
        instrument_name: &str,
        start_timestamp: Option<i64>,
        end_timestamp: Option<i64>,
        period: Option<i64>,
    ) -> Result<Vec<DeriveFundingRate>, Box<dyn std::error::Error + Send + Sync>> {
        let result: DeriveFundingRateHistoryResponse = self
            .call(
                "public/get_funding_rate_history",
                &GetFundingRateHistoryParams {
                    instrument_name,
                    start_timestamp,
                    end_timestamp,
                    period,
                },
            )
            .await?;
        Ok(result.funding_rate_history)
    }

    /// `public/get_tickers`: slim tickers keyed by instrument name. Options
    /// require both `currency` and `expiry_date` (YYYYMMDD); perps and spot
    /// (`erc20`) take an optional `currency` and reject `expiry_date`.
    pub async fn get_tickers(
        &self,
        instrument_type: &str,
        currency: Option<&str>,
        expiry_date: Option<i64>,
    ) -> Result<DeriveTickersResponse, Box<dyn std::error::Error + Send + Sync>> {
        let result: DeriveTickersResult = self
            .call(
                "public/get_tickers",
                &GetTickersParams {
                    instrument_type,
                    currency,
                    expiry_date,
                },
            )
            .await?;
        Ok(result.tickers)
    }

    /// `public/get_instrument`: one instrument definition.
    pub async fn get_instrument(
        &self,
        instrument_name: &str,
    ) -> Result<DeriveInstrument, Box<dyn std::error::Error + Send + Sync>> {
        self.call(
            "public/get_instrument",
            &InstrumentNameParams { instrument_name },
        )
        .await
    }

    /// `public/get_all_live_instruments`: sorted names of every live
    /// instrument.
    pub async fn get_all_live_instruments(
        &self,
    ) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
        self.call("public/get_all_live_instruments", &NoParams {})
            .await
    }

    /// `public/get_ticker`: slim ticker for one instrument.
    pub async fn get_ticker(
        &self,
        instrument_name: &str,
    ) -> Result<DeriveTicker, Box<dyn std::error::Error + Send + Sync>> {
        self.call(
            "public/get_ticker",
            &InstrumentNameParams { instrument_name },
        )
        .await
    }

    /// `public/get_trade_history`: one page of public trades.
    pub async fn get_trade_history(
        &self,
        params: &DeriveTradeHistoryParams<'_>,
    ) -> Result<DeriveTradeHistoryResponse, Box<dyn std::error::Error + Send + Sync>> {
        self.call("public/get_trade_history", params).await
    }

    /// `public/get_all_currencies`.
    pub async fn get_all_currencies(
        &self,
    ) -> Result<Vec<DeriveCurrency>, Box<dyn std::error::Error + Send + Sync>> {
        self.call("public/get_all_currencies", &NoParams {}).await
    }

    /// `public/get_currency`.
    pub async fn get_currency(
        &self,
        currency: &str,
    ) -> Result<DeriveCurrency, Box<dyn std::error::Error + Send + Sync>> {
        self.call("public/get_currency", &CurrencyParams { currency })
            .await
    }

    /// `public/get_risk_universes`.
    pub async fn get_risk_universes(
        &self,
    ) -> Result<Vec<DeriveRiskUniverse>, Box<dyn std::error::Error + Send + Sync>> {
        self.call("public/get_risk_universes", &NoParams {}).await
    }

    /// `public/get_index_chart_data`: spot index candles. Timestamps are UTC
    /// seconds; `period` is the candle size in seconds (1m to 1w).
    pub async fn get_index_chart_data(
        &self,
        currency: &str,
        start_timestamp: i64,
        end_timestamp: i64,
        period: i64,
    ) -> Result<Vec<DeriveIndexCandle>, Box<dyn std::error::Error + Send + Sync>> {
        self.call(
            "public/get_index_chart_data",
            &IndexChartParams {
                currency,
                start_timestamp,
                end_timestamp,
                period,
            },
        )
        .await
    }

    /// `public/get_tradingview_chart_data`: traded-price OHLCV candles.
    /// Timestamps are UTC seconds; `period` is in seconds (1m to 1w).
    pub async fn get_tradingview_chart_data(
        &self,
        instrument_name: &str,
        start_timestamp: i64,
        end_timestamp: i64,
        period: i64,
    ) -> Result<Vec<DeriveTradeCandle>, Box<dyn std::error::Error + Send + Sync>> {
        self.call(
            "public/get_tradingview_chart_data",
            &TradingViewChartParams {
                instrument_name,
                start_timestamp,
                end_timestamp,
                period,
            },
        )
        .await
    }

    /// `public/get_interest_rate_history`: lending APY candles. Timestamps
    /// are UTC ms; `period` in seconds, at least 1h (exchange default 1h).
    pub async fn get_interest_rate_history(
        &self,
        currency: &str,
        start_timestamp: Option<i64>,
        end_timestamp: Option<i64>,
        period: Option<i64>,
        risk_universe_id: Option<u32>,
    ) -> Result<Vec<DeriveInterestRateCandle>, Box<dyn std::error::Error + Send + Sync>> {
        let result: InterestRateHistoryResult = self
            .call(
                "public/get_interest_rate_history",
                &InterestRateHistoryParams {
                    currency,
                    start_timestamp,
                    end_timestamp,
                    period,
                    risk_universe_id,
                },
            )
            .await?;
        Ok(result.interest_rate_history)
    }

    /// `public/get_option_settlement_prices`: settled option expiries.
    pub async fn get_option_settlement_prices(
        &self,
        currency: &str,
    ) -> Result<Vec<DeriveOptionSettlement>, Box<dyn std::error::Error + Send + Sync>> {
        let result: OptionSettlementResult = self
            .call(
                "public/get_option_settlement_prices",
                &CurrencyParams { currency },
            )
            .await?;
        Ok(result.expiries)
    }

    /// `public/get_perp_impact_twap`: times are unix seconds.
    pub async fn get_perp_impact_twap(
        &self,
        currency: &str,
        start_time: i64,
        end_time: i64,
    ) -> Result<DerivePerpImpactTwap, Box<dyn std::error::Error + Send + Sync>> {
        self.call(
            "public/get_perp_impact_twap",
            &PerpImpactTwapParams {
                currency,
                start_time,
                end_time,
            },
        )
        .await
    }

    /// `public/get_time`: server time, ms.
    pub async fn get_time(&self) -> Result<i64, Box<dyn std::error::Error + Send + Sync>> {
        self.call("public/get_time", &NoParams {}).await
    }

    /// `public/get_liquidation_history`: one page of liquidation auctions,
    /// newest first. Timestamps are UTC ms.
    pub async fn get_liquidation_history(
        &self,
        start_timestamp: Option<i64>,
        end_timestamp: Option<i64>,
        subaccount_id: Option<u64>,
        page: Option<u64>,
        page_size: Option<u64>,
    ) -> Result<DeriveLiquidationHistoryResponse, Box<dyn std::error::Error + Send + Sync>> {
        self.call(
            "public/get_liquidation_history",
            &LiquidationHistoryParams {
                start_timestamp,
                end_timestamp,
                subaccount_id,
                page,
                page_size,
            },
        )
        .await
    }

    /// `public/get_live_auctions`: liquidation auctions open now.
    pub async fn get_live_auctions(
        &self,
    ) -> Result<Vec<DeriveLiveAuction>, Box<dyn std::error::Error + Send + Sync>> {
        let result: LiveAuctionsResult =
            self.call("public/get_live_auctions", &NoParams {}).await?;
        Ok(result.auctions)
    }

    /// `public/get_latest_signed_feeds`: latest oracle feeds. `expiry`
    /// filters forward/vol/rate data; `0` returns none of them (observed live).
    pub async fn get_latest_signed_feeds(
        &self,
        currency: Option<&str>,
        expiry: Option<i64>,
    ) -> Result<DeriveSignedFeeds, Box<dyn std::error::Error + Send + Sync>> {
        self.call(
            "public/get_latest_signed_feeds",
            &SignedFeedsParams { currency, expiry },
        )
        .await
    }

    /// `public/get_live_incidents`: unresolved exchange incidents.
    pub async fn get_live_incidents(
        &self,
    ) -> Result<Vec<DeriveIncident>, Box<dyn std::error::Error + Send + Sync>> {
        let result: LiveIncidentsResult =
            self.call("public/get_live_incidents", &NoParams {}).await?;
        Ok(result.incidents)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Payloads captured from https://api.derive.xyz/v3 on 2026-11-15.

    #[test]
    fn parses_perp_instrument() {
        let json = r#"{"id":"8dc5","result":{"instruments":[{"instrument_type":"perp","instrument_name":"ETH-PERP","risk_universe_id":1,"scheduled_activation":1701840228,"scheduled_deactivation":9223372036854775807,"is_active":true,"tick_size":"0.01","minimum_amount":"0.1","maximum_amount":"10000","amount_step":"0.001","mark_price_fee_rate_cap":"0","maker_fee_rate":"0.0001","taker_fee_rate":"0.0003","base_fee":"0.01","base_currency":"ETH","quote_currency":"USDC","option_details":null,"perp_details":{"index":"ETH-USD","max_rate_per_hour":"0.000171232876","min_rate_per_hour":"-0.000171232876","aggregate_funding":"1096.587546632068","funding_rate":"0","srm_perp_margin_requirements":{"im_perp_req":"0.066","mm_perp_req":"0.05","max_leverage":"15.15"}},"erc20_details":null,"base_asset_address":"0xAf65752C4643E25C02F693f9D4FE19cF23a095E3","base_asset_sub_id":"0","pro_rata_fraction":"0","fifo_min_allocation":"0","pro_rata_amount_step":"0.1"}],"pagination":{"num_pages":1,"count":1}}}"#;
        let rpc: RpcResponse<DeriveAllInstrumentsResponse> = serde_json::from_str(json).unwrap();
        let res = rpc.result.unwrap();
        assert_eq!(res.pagination.count, 1);
        let perp = res.instruments[0].perp_details.as_ref().unwrap();
        assert_eq!(perp.index, "ETH-USD");
        assert_eq!(
            perp.srm_perp_margin_requirements
                .as_ref()
                .unwrap()
                .max_leverage,
            "15.15"
        );
    }

    #[test]
    fn parses_option_instrument() {
        let json = r#"{"instrument_type":"option","instrument_name":"ZEC-20270625-900-P","risk_universe_id":3,"scheduled_activation":1798185600,"scheduled_deactivation":1813910340,"is_active":false,"tick_size":"0.1","minimum_amount":"1","maximum_amount":"100000","amount_step":"0.1","mark_price_fee_rate_cap":"0.125","maker_fee_rate":"0.0001","taker_fee_rate":"0.0003","base_fee":"0.5","base_currency":"ZEC","quote_currency":"USDC","option_details":{"index":"ZEC-USD","expiry":1813910400,"strike":"900","option_type":"P","settlement_price":null},"perp_details":null,"erc20_details":null,"base_asset_address":"0x5003551215f9f31954A2775FF42870ce8D902E27","base_asset_sub_id":"386547056641813910400","pro_rata_fraction":"0.8","fifo_min_allocation":"100","pro_rata_amount_step":"10"}"#;
        let ins: DeriveInstrument = serde_json::from_str(json).unwrap();
        let opt = ins.option_details.unwrap();
        assert_eq!(opt.expiry, 1813910400);
        assert!(opt.settlement_price.is_none());
    }

    #[test]
    fn parses_perp_and_option_tickers() {
        let json = r#"{"id":"x","result":{"tickers":{"ETH-PERP":{"t":1794779458112,"A":"1","a":"1823.77","B":"22.542","b":"1823.76","f":"0.000012500","option_pricing":null,"I":"1823.49","M":"1823.48","stats":{"c":"17329.41","v":"30872584.806","pr":"30859575.438","n":9275,"oi":"13567.412","h":"1839.99","l":"1710.36","p":"0.017"},"minp":"1787.74","maxp":"1859.95"},"ETH-20261116-2450-P":{"t":1794779508719,"A":"0","a":"0","B":"0","b":"0","f":null,"option_pricing":{"d":"-0.99999","t":"0","g":"0","v":"0","i":"1.28504","r":"0.72798","f":"1820.8","m":"629.1","df":"1.000","bi":"0","ai":"0"},"I":"1822.1","M":"629.1","stats":{"c":"0","v":"0","pr":"0","n":0,"oi":"0","h":"0","l":"0","p":"0"},"minp":"601.8","maxp":"656"}}}}"#;
        let rpc: RpcResponse<DeriveTickersResult> = serde_json::from_str(json).unwrap();
        let tickers = rpc.result.unwrap().tickers;
        let perp = &tickers["ETH-PERP"];
        assert_eq!(perp.best_ask_price, "1823.77");
        assert_eq!(perp.best_ask_amount, "1");
        assert_eq!(perp.funding_rate.as_deref(), Some("0.000012500"));
        assert_eq!(perp.stats.trade_count, 9275);
        let opt = &tickers["ETH-20261116-2450-P"];
        assert!(opt.funding_rate.is_none());
        assert_eq!(opt.option_pricing.as_ref().unwrap().forward_price, "1820.8");
    }

    #[test]
    fn rejects_ticker_missing_required_field() {
        let json = r#"{"t":1,"A":"1","a":"1","B":"1","b":"1","f":null,"option_pricing":null,"I":"1","M":"1","minp":"1","maxp":"1"}"#;
        assert!(serde_json::from_str::<DeriveTicker>(json).is_err());
    }

    #[test]
    fn parses_funding_candle() {
        let json = r#"{"id":"x","result":{"funding_rate_history":[{"timestamp":1791309600000,"risk_universe_id":1,"currency":"ETH","open":"0.000012500","high":"0.000012500","low":"0.000012500","close":"0.000012500","funding_rate":"0.000012500"}]}}"#;
        let rpc: RpcResponse<DeriveFundingRateHistoryResponse> =
            serde_json::from_str(json).unwrap();
        let candle = &rpc.result.unwrap().funding_rate_history[0];
        assert_eq!(candle.timestamp, 1791309600000);
        assert_eq!(candle.close, candle.funding_rate);
    }

    #[test]
    fn parses_error_envelope() {
        let json = r#"{"error":{"code":-32602,"message":"Invalid params","data":"Expiry date is required for options"}}"#;
        let rpc: RpcResponse<DeriveTickersResult> = serde_json::from_str(json).unwrap();
        assert!(rpc.result.is_none());
        assert_eq!(rpc.error.unwrap().code, -32602);
    }

    #[tokio::test]
    async fn test_get_all_instruments_perp() {
        let api = MarketApi::new(5);
        let result = api
            .get_all_instruments("perp", Some("ETH"), false, None, None, None)
            .await;
        assert!(
            result.is_ok(),
            "Failed to fetch Derive instruments: {:?}",
            result.err()
        );
        let res = result.unwrap();
        assert!(!res.instruments.is_empty(), "Expected ETH perp instruments");
    }

    #[tokio::test]
    async fn test_get_funding_rate_history() {
        let api = MarketApi::new(5);
        let result = api
            .get_funding_rate_history("ETH-PERP", None, None, None)
            .await;
        assert!(
            result.is_ok(),
            "Failed to fetch Derive funding rate history: {:?}",
            result.err()
        );
        assert!(!result.unwrap().is_empty(), "Expected funding rate history");
    }

    #[tokio::test]
    async fn test_get_tickers_perp() {
        let api = MarketApi::new(5);
        let result = api.get_tickers("perp", None, None).await;
        assert!(
            result.is_ok(),
            "Failed to fetch Derive tickers: {:?}",
            result.err()
        );
        assert!(!result.unwrap().is_empty(), "Expected perp tickers");
    }

    #[tokio::test]
    async fn test_get_tickers_option_requires_expiry() {
        let api = MarketApi::new(5);
        let result = api.get_tickers("option", Some("ETH"), None).await;
        assert!(
            result.is_err(),
            "Options without expiry_date must be rejected"
        );
    }
}

#[cfg(test)]
mod fixture_tests {
    use super::*;

    // Captures from https://api.derive.xyz/v3 on 2026-11-16 (arrays trimmed).

    fn parse<T: DeserializeOwned>(json: &str) -> T {
        let rpc: RpcResponse<T> = serde_json::from_str(json).unwrap();
        assert!(rpc.error.is_none());
        rpc.result.unwrap()
    }

    /// Parses `result` after deleting `field` from the first object found at
    /// `pointer` (JSON pointer within `result`).
    fn parse_without<T: DeserializeOwned>(json: &str, pointer: &str, field: &str) -> bool {
        let mut v: serde_json::Value = serde_json::from_str(json).unwrap();
        let obj = v["result"]
            .pointer_mut(pointer)
            .and_then(|o| o.as_object_mut())
            .unwrap();
        assert!(obj.remove(field).is_some(), "fixture lacks {field}");
        serde_json::from_value::<T>(v["result"].take()).is_ok()
    }

    const INSTRUMENT: &str = include_str!("../../tests/fixtures/get_instrument.json");
    const LIVE_INSTRUMENTS: &str =
        include_str!("../../tests/fixtures/get_all_live_instruments.json");
    const TICKER: &str = include_str!("../../tests/fixtures/get_ticker.json");
    const TRADES: &str = include_str!("../../tests/fixtures/get_trade_history.json");
    const CURRENCIES: &str = include_str!("../../tests/fixtures/get_all_currencies.json");
    const CURRENCY: &str = include_str!("../../tests/fixtures/get_currency.json");
    const UNIVERSES: &str = include_str!("../../tests/fixtures/get_risk_universes.json");
    const INDEX_CHART: &str = include_str!("../../tests/fixtures/get_index_chart_data.json");
    const TV_CHART: &str = include_str!("../../tests/fixtures/get_tradingview_chart_data.json");
    const INTEREST: &str = include_str!("../../tests/fixtures/get_interest_rate_history.json");
    const SETTLEMENT: &str = include_str!("../../tests/fixtures/get_option_settlement_prices.json");
    const TWAP: &str = include_str!("../../tests/fixtures/get_perp_impact_twap.json");
    const TIME: &str = include_str!("../../tests/fixtures/get_time.json");
    const LIQUIDATIONS: &str = include_str!("../../tests/fixtures/get_liquidation_history.json");
    const LIVE_AUCTIONS: &str = include_str!("../../tests/fixtures/get_live_auctions.json");
    const FEEDS: &str = include_str!("../../tests/fixtures/get_latest_signed_feeds.json");
    const INCIDENTS: &str = include_str!("../../tests/fixtures/get_live_incidents.json");

    #[test]
    fn instrument_and_live_instruments() {
        let ins: DeriveInstrument = parse(INSTRUMENT);
        assert_eq!(ins.instrument_name, "ETH-PERP");
        assert!(ins.perp_details.is_some());
        assert!(!parse_without::<DeriveInstrument>(
            INSTRUMENT,
            "",
            "tick_size"
        ));
        let names: Vec<String> = parse(LIVE_INSTRUMENTS);
        assert!(!names.is_empty());
    }

    #[test]
    fn ticker() {
        let t: DeriveTicker = parse(TICKER);
        assert!(t.funding_rate.is_some());
        assert!(!parse_without::<DeriveTicker>(TICKER, "", "M"));
    }

    #[test]
    fn trade_history() {
        let res: DeriveTradeHistoryResponse = parse(TRADES);
        let t = &res.trades[0];
        assert_eq!(t.instrument_name, "ETH-PERP");
        assert_eq!(t.batch_status.as_deref(), Some("Batching"));
        assert!(t.quote_id.is_none());
        assert!(!parse_without::<DeriveTradeHistoryResponse>(
            TRADES,
            "/trades/0",
            "trade_price"
        ));
        // Nullable but always sent: a missing key is an error.
        assert!(!parse_without::<DeriveTradeHistoryResponse>(
            TRADES,
            "/trades/0",
            "quote_id"
        ));
    }

    #[test]
    fn currencies() {
        let all: Vec<DeriveCurrency> = parse(CURRENCIES);
        assert!(all.iter().any(|c| c.option.is_some() && c.perp.is_some()));
        assert!(all.iter().any(|c| c.option.is_none() && c.perp.is_none()));
        assert!(all
            .iter()
            .flat_map(|c| &c.spot)
            .any(|s| s.erc20.underlying_erc20.is_none()));
        let eth: DeriveCurrency = parse(CURRENCY);
        assert_eq!(eth.currency, "ETH");
        assert!(eth.managers.iter().any(|m| m.pm.is_none()));
        assert!(!parse_without::<DeriveCurrency>(CURRENCY, "", "spot_price"));
        assert!(!parse_without::<DeriveCurrency>(
            CURRENCY,
            "/spot/0/universes/0",
            "srm_im_discount"
        ));
    }

    #[test]
    fn risk_universes() {
        let all: Vec<DeriveRiskUniverse> = parse(UNIVERSES);
        assert_eq!(all[0].risk_universe_id, 0);
        assert!(!all[0].managers[0].collaterals.is_empty());
        assert!(!parse_without::<Vec<DeriveRiskUniverse>>(
            UNIVERSES,
            "/0",
            "security_module"
        ));
    }

    #[test]
    fn chart_candles() {
        let idx: Vec<DeriveIndexCandle> = parse(INDEX_CHART);
        assert_eq!(idx[0].timestamp, idx[0].timestamp_bucket);
        assert!(!parse_without::<Vec<DeriveIndexCandle>>(
            INDEX_CHART,
            "/0",
            "close_price"
        ));
        let tv: Vec<DeriveTradeCandle> = parse(TV_CHART);
        assert!(!tv[0].volume_usd.is_empty());
        assert!(!parse_without::<Vec<DeriveTradeCandle>>(
            TV_CHART,
            "/0",
            "volume_contracts"
        ));
    }

    #[test]
    fn interest_rate_history() {
        let res: InterestRateHistoryResult = parse(INTEREST);
        assert!(!res.interest_rate_history.is_empty());
        assert!(!parse_without::<InterestRateHistoryResult>(
            INTEREST,
            "/interest_rate_history/0/borrow_apy",
            "close"
        ));
    }

    #[test]
    fn option_settlements() {
        let res: OptionSettlementResult = parse(SETTLEMENT);
        assert_eq!(res.expiries[0].expiry_date, "20261007");
        assert!(res.expiries[0].price.is_some());
        assert!(!parse_without::<OptionSettlementResult>(
            SETTLEMENT,
            "/expiries/0",
            "utc_expiry_sec"
        ));
    }

    #[test]
    fn perp_impact_twap_and_time() {
        let twap: DerivePerpImpactTwap = parse(TWAP);
        assert_eq!(twap.currency, "ETH");
        assert!(!parse_without::<DerivePerpImpactTwap>(
            TWAP,
            "",
            "mid_price_diff_twap"
        ));
        let t: i64 = parse(TIME);
        assert!(t > 1_700_000_000_000);
    }

    #[test]
    fn liquidations() {
        let res: DeriveLiquidationHistoryResponse = parse(LIQUIDATIONS);
        let a = &res.auctions[0];
        assert!(a.end_timestamp.is_some());
        assert!(!a.bids[0].amounts_liquidated.is_empty());
        assert!(!parse_without::<DeriveLiquidationHistoryResponse>(
            LIQUIDATIONS,
            "/auctions/0/bids/0",
            "unit_price"
        ));
        let live: LiveAuctionsResult = parse(LIVE_AUCTIONS);
        assert!(live.auctions.is_empty());
        let json = r#"{"subaccount_id":1,"risk_universe_id":1,"timestamp":1,"margin_type":"SM","estimated_bid_price":"1","estimated_discount_pnl":"1","estimated_mtm":"1","estimated_percent_bid":"1","subaccount_balances":{}}"#;
        assert!(serde_json::from_str::<DeriveLiveAuction>(json).is_err());
    }

    #[test]
    fn signed_feeds() {
        let f: DeriveSignedFeeds = parse(FEEDS);
        assert!(f.spot_data.contains_key("ETH"));
        let vol = f.vol_data["ETH"].values().next().unwrap();
        assert!(!vol.vol_data.sigma.is_empty());
        assert!(f.perp_data["ETH"].contains_key("P"));
        assert!(!parse_without::<DeriveSignedFeeds>(
            FEEDS,
            "/spot_data/ETH",
            "price"
        ));
    }

    #[test]
    fn incidents() {
        let res: LiveIncidentsResult = parse(INCIDENTS);
        assert!(res.incidents.is_empty());
        let json = r#"{"creation_timestamp_sec":1,"label":"x","message":"y","severity":"low"}"#;
        assert!(serde_json::from_str::<DeriveIncident>(json).is_err());
    }
}
