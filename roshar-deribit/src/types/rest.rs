use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::ws::{nullable, DeribitTradeData};

/// JSON-RPC response envelope, identical over REST and the websocket. Exactly one of
/// `result` / `error` is present; use `into_result`.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeribitRpcResponse<T> {
    pub jsonrpc: String,
    /// The request id (websocket only; REST replies carry none).
    pub id: Option<u64>,
    pub result: Option<T>,
    pub error: Option<DeribitRpcError>,
    /// Server receive time, microseconds since the UNIX epoch.
    #[serde(rename = "usIn")]
    pub us_in: u64,
    /// Server send time, microseconds since the UNIX epoch.
    #[serde(rename = "usOut")]
    pub us_out: u64,
    #[serde(rename = "usDiff")]
    pub us_diff: u64,
    pub testnet: bool,
}

impl<T> DeribitRpcResponse<T> {
    pub fn into_result(self) -> Result<T, DeribitRpcError> {
        match (self.result, self.error) {
            (_, Some(error)) => Err(error),
            (Some(result), None) => Ok(result),
            (None, None) => Err(DeribitRpcError {
                code: 0,
                message: "response has neither result nor error".to_string(),
                data: None,
            }),
        }
    }
}

/// `{"code":13020,"message":"not_found","data":{...}}`. `data` describes the offending
/// parameter when there is one.
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
pub struct DeribitRpcError {
    pub code: i64,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

impl std::fmt::Display for DeribitRpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Deribit error {}: {}", self.code, self.message)?;
        if let Some(data) = &self.data {
            write!(f, " ({data})")?;
        }
        Ok(())
    }
}

impl std::error::Error for DeribitRpcError {}

/// `public/get_instruments`, `public/get_instrument`, and `instrument.creation.*` notifications.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeribitInstrument {
    pub instrument_name: String,
    pub instrument_id: u64,
    /// `future`, `option`, `spot`, `future_combo` or `option_combo`.
    pub kind: String,
    /// `open`, `settlement`, `delivered`, `inactive`, `locked`, `halted` or `archivized`.
    pub state: String,
    pub is_active: bool,
    pub base_currency: String,
    /// Absent for some index futures (e.g. `BTCDVOL_USDC-*`).
    pub base_currency_uuid: Option<String>,
    pub quote_currency: String,
    pub quote_currency_uuid: String,
    pub counter_currency: String,
    /// Absent for spot.
    pub settlement_currency: Option<String>,
    /// `minute`, `hour`, `day`, `week`, `month` or `perpetual`; absent for spot.
    pub settlement_period: Option<String>,
    pub price_index: String,
    pub index_id: u64,
    /// `BTC`, `ETH`, `TIER_2` or `TIER_3`.
    pub product_group: String,
    /// `linear` or `reversed`.
    pub instrument_type: String,
    /// `crypto`, `equity`, `commodity`, ...
    pub underlying_type: String,
    pub creation_timestamp: u64,
    pub expiration_timestamp: u64,
    pub contract_size: f64,
    pub min_trade_amount: f64,
    pub tick_size: f64,
    pub tick_size_steps: Vec<DeribitTickSizeStep>,
    pub maker_commission: f64,
    pub taker_commission: f64,
    /// Futures and options.
    pub lot_size: Option<f64>,
    /// Futures only: `linear` or `reversed`.
    pub future_type: Option<String>,
    /// Options only.
    pub strike: Option<f64>,
    /// Options only: `call` or `put`.
    pub option_type: Option<String>,
    pub max_leverage: Option<u32>,
    pub max_non_default_leverage: Option<f64>,
    pub max_liquidation_commission: Option<f64>,
    pub block_trade_commission: Option<f64>,
    pub block_trade_min_trade_amount: Option<f64>,
    pub block_trade_tick_size: Option<f64>,
    /// Spot only: routed to Coinbase Exchange.
    pub is_cbe_routed: Option<bool>,
    /// Spot only.
    pub is_csr: Option<bool>,
    /// Only on `instrument.creation.*` notifications.
    pub timestamp: Option<u64>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeribitTickSizeStep {
    pub above_price: f64,
    pub tick_size: f64,
}

/// `public/get_currencies`.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeribitCurrency {
    pub currency: String,
    pub currency_long: String,
    pub currency_uuid: String,
    pub coin_type: String,
    pub decimals: u32,
    pub apr: f64,
    pub in_cross_collateral_pool: bool,
    pub min_confirmations: u32,
    pub network_currency: String,
    pub network_fee: f64,
    pub withdrawal_fee: f64,
    pub min_withdrawal_fee: f64,
    pub onchain_operations_precision: u32,
    pub withdrawal_priorities: Vec<DeribitWithdrawalPriority>,
    /// Currencies with Coinbase-routed spot only.
    pub coinbase_networks: Option<Vec<DeribitCoinbaseNetwork>>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeribitWithdrawalPriority {
    pub name: String,
    pub value: f64,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeribitCoinbaseNetwork {
    pub display_name: String,
    pub resource_name: String,
}

/// `public/get_book_summary_by_currency` / `_by_instrument`.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeribitBookSummary {
    pub instrument_name: String,
    pub base_currency: String,
    pub quote_currency: String,
    pub creation_timestamp: u64,
    pub mark_price: f64,
    /// `null` for options expiring today.
    #[serde(deserialize_with = "nullable")]
    pub estimated_delivery_price: Option<f64>,
    #[serde(deserialize_with = "nullable")]
    pub bid_price: Option<f64>,
    #[serde(deserialize_with = "nullable")]
    pub ask_price: Option<f64>,
    #[serde(deserialize_with = "nullable")]
    pub mid_price: Option<f64>,
    #[serde(deserialize_with = "nullable")]
    pub last: Option<f64>,
    #[serde(deserialize_with = "nullable")]
    pub high: Option<f64>,
    #[serde(deserialize_with = "nullable")]
    pub low: Option<f64>,
    /// Percent over 24 hours; `null` without a 24-hour price.
    #[serde(deserialize_with = "nullable")]
    pub price_change: Option<f64>,
    pub volume: f64,
    pub volume_realtime: f64,
    pub volume_delayed: f64,
    pub volume_usd: Option<f64>,
    pub volume_realtime_usd: Option<f64>,
    pub volume_delayed_usd: Option<f64>,
    pub volume_notional: Option<f64>,
    pub volume_realtime_notional: Option<f64>,
    pub volume_delayed_notional: Option<f64>,
    /// Futures and options (absent for spot).
    pub open_interest: Option<f64>,
    /// Perpetuals only.
    pub funding_8h: Option<f64>,
    pub current_funding: Option<f64>,
    // Options only.
    pub mark_iv: Option<f64>,
    pub interest_rate: Option<f64>,
    pub underlying_index: Option<String>,
    pub underlying_price: Option<f64>,
}

/// `[price, amount]`.
pub type DeribitPriceLevel = (f64, f64);

/// `public/get_order_book` / `_by_instrument_id`: the ticker fields plus book levels.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeribitOrderBookSnapshot {
    pub change_id: u64,
    pub bids: Vec<DeribitPriceLevel>,
    pub asks: Vec<DeribitPriceLevel>,
    #[serde(flatten)]
    pub ticker: super::ws::DeribitTickerData,
}

/// `public/get_last_trades_by_*`.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeribitTradesPage {
    pub trades: Vec<DeribitTradeData>,
    pub has_more: bool,
}

/// `public/get_tradingview_chart_data`: column arrays of equal length.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeribitTradingViewChartData {
    /// `ok` or `no_data` (all arrays empty).
    pub status: String,
    pub ticks: Vec<u64>,
    pub open: Vec<f64>,
    pub high: Vec<f64>,
    pub low: Vec<f64>,
    pub close: Vec<f64>,
    pub volume: Vec<f64>,
    pub cost: Vec<f64>,
}

/// One hourly record of `public/get_funding_rate_history`.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeribitFundingRateHistory {
    pub timestamp: u64,
    pub index_price: f64,
    pub prev_index_price: f64,
    pub interest_1h: f64,
    pub interest_8h: f64,
}

/// `public/get_funding_chart_data`.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeribitFundingChartData {
    pub current_interest: f64,
    pub interest_8h: f64,
    pub data: Vec<DeribitFundingChartPoint>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeribitFundingChartPoint {
    pub timestamp: u64,
    pub index_price: f64,
    pub interest_8h: f64,
}

/// `[timestamp_ms, value]`, used by `get_historical_volatility`, `get_index_chart_data` and
/// `get_mark_price_history`.
pub type DeribitTimeValue = (u64, f64);

/// `[timestamp_ms, open, high, low, close]`.
pub type DeribitVolatilityCandle = (u64, f64, f64, f64, f64);

/// `public/get_volatility_index_data`.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeribitVolatilityIndexData {
    pub data: Vec<DeribitVolatilityCandle>,
    /// Pass as `end_timestamp` for the next (older) page; `null` when there is none.
    #[serde(deserialize_with = "nullable")]
    pub continuation: Option<u64>,
}

/// `public/get_index_price`.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeribitIndexPrice {
    pub index_price: f64,
    pub estimated_delivery_price: f64,
}

/// `public/get_index_price_names` with `extended=true`.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeribitIndexPriceName {
    pub name: String,
    /// Only for indices that combos can be created on.
    pub future_combo_creation_enabled: Option<bool>,
    pub option_combo_creation_enabled: Option<bool>,
}

/// `public/get_basket_index_composition`. Numbers are sent as strings and kept as such.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeribitBasketIndexComposition {
    pub product_id: String,
    pub divisor: String,
    /// RFC 3339.
    pub timestamp: String,
    pub inception_timestamp: String,
    pub last_rebalance: String,
    pub constituents: Vec<DeribitBasketConstituent>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeribitBasketConstituent {
    pub symbol: String,
    pub name: String,
    pub rank: u32,
    pub amount: String,
    pub cap_factor: String,
    pub market_cap: String,
    pub index_market_cap: String,
    pub weight: String,
    pub running_weight: String,
}

/// `public/get_delivery_prices`.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeribitDeliveryPrices {
    pub records_total: u64,
    pub data: Vec<DeribitDeliveryPrice>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeribitDeliveryPrice {
    /// `YYYY-MM-DD`.
    pub date: String,
    pub delivery_price: f64,
}

/// `public/get_last_settlements_by_*`.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeribitSettlementsPage {
    pub continuation: String,
    pub settlements: Vec<DeribitSettlement>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeribitSettlement {
    /// `settlement`, `delivery`, `exercise` or `bankruptcy`.
    #[serde(rename = "type")]
    pub settlement_type: String,
    pub timestamp: u64,
    // Absent on `bankruptcy` records, present on the others.
    pub instrument_name: Option<String>,
    pub index_price: Option<f64>,
    pub position: Option<f64>,
    pub session_profit_loss: f64,
    pub mark_price: Option<f64>,
    pub profit_loss: Option<f64>,
    pub funding: Option<f64>,
    pub funded: Option<f64>,
    pub session_bankruptcy: Option<f64>,
    pub session_tax: Option<f64>,
    pub session_tax_rate: Option<f64>,
    pub socialized: Option<f64>,
}

/// `public/get_contract_size`.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeribitContractSize {
    pub contract_size: f64,
}

/// Expiry codes (`17NOV26`, `PERPETUAL`) per kind.
#[derive(Debug, Deserialize, Serialize, Clone, Default, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DeribitExpirationKinds {
    pub future: Option<Vec<String>>,
    pub option: Option<Vec<String>>,
}

/// `public/get_expirations`. With `currency=any` the kinds are top level; otherwise they are
/// keyed by lower-case currency or currency pair (`btc`, `btc_usdc`).
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(untagged)]
pub enum DeribitExpirations {
    Kinds(DeribitExpirationKinds),
    ByCurrency(HashMap<String, DeribitExpirationKinds>),
}

/// `public/get_apr_history`.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeribitAprHistory {
    pub data: Vec<DeribitAprPoint>,
    /// Pass as `before` for the next page; absent on the last page.
    pub continuation: Option<u64>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeribitAprPoint {
    /// Days since the UNIX epoch.
    pub day: u64,
    pub apr: f64,
}

/// `public/status`.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeribitStatus {
    /// `true`, `false` or `partial`.
    pub locked: String,
    /// Documented as optional.
    pub locked_indices: Option<Vec<String>>,
}

/// `public/test` and `public/hello`.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeribitVersion {
    pub version: String,
}

/// `public/get_trade_volumes`.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeribitTradeVolume {
    pub currency: String,
    pub currency_pair: String,
    pub calls_volume: f64,
    pub puts_volume: f64,
    pub futures_volume: f64,
    /// Omitted for currencies whose spot is routed to Coinbase.
    pub spot_volume: Option<f64>,
    // `extended=true` only (spot ones also omitted for Coinbase-routed spot).
    pub calls_volume_7d: Option<f64>,
    pub puts_volume_7d: Option<f64>,
    pub futures_volume_7d: Option<f64>,
    pub spot_volume_7d: Option<f64>,
    pub calls_volume_30d: Option<f64>,
    pub puts_volume_30d: Option<f64>,
    pub futures_volume_30d: Option<f64>,
    pub spot_volume_30d: Option<f64>,
}

/// `public/get_combos` / `public/get_combo_details`.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeribitCombo {
    pub id: String,
    pub instrument_id: u64,
    /// `active` or `inactive`.
    pub state: String,
    pub state_timestamp: u64,
    pub creation_timestamp: u64,
    pub legs: Vec<DeribitComboLeg>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeribitComboLeg {
    pub instrument_name: String,
    /// Signed ratio: negative is sell.
    pub amount: i64,
}

/// `public/get_block_rfq_trades`.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeribitBlockRfqTradesPage {
    pub continuation: Option<String>,
    pub block_rfqs: Vec<DeribitBlockRfq>,
}

/// A completed Block RFQ (`public/get_block_rfq_trades`, `block_rfq.trades.*`).
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeribitBlockRfq {
    pub id: u64,
    pub timestamp: u64,
    /// Taker direction.
    pub direction: String,
    pub amount: f64,
    pub mark_price: f64,
    /// `null` when the legs do not form a listed combo.
    #[serde(deserialize_with = "nullable")]
    pub combo_id: Option<String>,
    pub legs: Vec<DeribitBlockRfqLeg>,
    pub trades: Vec<DeribitBlockRfqTrade>,
    pub hedge: Option<DeribitBlockRfqHedge>,
    /// Index name to price at execution. Absent in the documented notification example.
    pub index_prices: Option<HashMap<String, f64>>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeribitBlockRfqLeg {
    pub instrument_name: String,
    pub direction: String,
    pub price: f64,
    pub ratio: u32,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeribitBlockRfqTrade {
    pub direction: String,
    pub price: f64,
    pub amount: f64,
    pub hedge_amount: Option<f64>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeribitBlockRfqHedge {
    pub instrument_name: String,
    pub direction: String,
    pub price: f64,
    pub amount: f64,
}
