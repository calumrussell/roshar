use serde::{Deserialize, Deserializer, Serialize};

/// For fields that are always sent but may be `null`: the key is required (no `default`),
/// `null` maps to `None`.
fn required_nullable<'de, D, T>(d: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(d)
}

// ── WebSocket messages ────────────────────────────────────────────────────────

/// JSON-RPC 2.0 request for the Paradex WebSocket (`subscribe`, `unsubscribe`,
/// `modify_subscription`).
/// Format: `{"id":1,"jsonrpc":"2.0","method":"subscribe","params":{"channel":"trades.BTC-USD-PERP"}}`
///
/// Requests and their acknowledgements are JSON text frames. Public channel payloads are
/// SBE binary frames (see `PARADEX_WSS_URL`); `markets` and `system_status` have no SBE template
/// and arrive as JSON (`ws_control::parse`).
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ParadexWssMessage {
    pub id: u32,
    pub jsonrpc: String,
    pub method: String,
    pub params: ParadexWssParams,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ParadexWssParams {
    pub channel: String,
    /// Scoped `markets_summary` subscription: markets in the filter.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub markets: Option<Vec<String>>,
    /// `modify_subscription`: markets to add to the scoped filter.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub add: Option<Vec<String>>,
    /// `modify_subscription`: markets to remove from the scoped filter.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remove: Option<Vec<String>>,
}

/// Refresh rate of the periodic order book feeds.
///
/// The exchange accepts 50ms and 100ms on `snapshot`, and 100ms, 200ms and 400ms on
/// `interactive`; other combinations are rejected with a subscribe error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParadexBookRefresh {
    Ms50,
    Ms100,
    Ms200,
    Ms400,
}

impl ParadexBookRefresh {
    fn as_str(self) -> &'static str {
        match self {
            Self::Ms50 => "50ms",
            Self::Ms100 => "100ms",
            Self::Ms200 => "200ms",
            Self::Ms400 => "400ms",
        }
    }
}

/// Refresh rate of `funding_rate_comparison.ALL@{refresh}`; the exchange accepts no others.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParadexFundingComparisonRefresh {
    Ms500,
    Ms1000,
}

impl ParadexFundingComparisonRefresh {
    fn as_str(self) -> &'static str {
        match self {
            Self::Ms500 => "500ms",
            Self::Ms1000 => "1000ms",
        }
    }
}

/// Order book feed selector for the `order_book.{market}.…` channel family.
///
/// Periodic feeds are always 15 levels deep; the exchange accepts no other depth.
/// `price_tick` is the exchange's tick token, e.g. `"0_01"`, `"1"`, `"10"`.
/// Only one order book feed per market can share an SBE connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParadexBookFeed<'a> {
    /// `order_book.{market}.deltas`: incremental updates (API orders, excludes RPI).
    Deltas,
    /// `order_book.{market}.interactive_deltas`: incremental updates including RPI orders.
    InteractiveDeltas,
    /// `order_book.{market}.snapshot@15@{refresh}[@{price_tick}]`: periodic 15-level snapshot.
    Snapshot {
        refresh: ParadexBookRefresh,
        price_tick: Option<&'a str>,
    },
    /// `order_book.{market}.interactive@15@{refresh}[@{price_tick}]`: periodic 15-level
    /// snapshot including RPI orders.
    Interactive {
        refresh: ParadexBookRefresh,
        price_tick: Option<&'a str>,
    },
}

impl ParadexBookFeed<'_> {
    /// Full channel name for `market`.
    pub fn channel(&self, market: &str) -> String {
        match self {
            Self::Deltas => format!("order_book.{market}.deltas"),
            Self::InteractiveDeltas => format!("order_book.{market}.interactive_deltas"),
            Self::Snapshot {
                refresh,
                price_tick,
            } => Self::periodic(market, "snapshot", *refresh, *price_tick),
            Self::Interactive {
                refresh,
                price_tick,
            } => Self::periodic(market, "interactive", *refresh, *price_tick),
        }
    }

    fn periodic(
        market: &str,
        feed: &str,
        refresh: ParadexBookRefresh,
        price_tick: Option<&str>,
    ) -> String {
        let refresh = refresh.as_str();
        match price_tick {
            Some(tick) => format!("order_book.{market}.{feed}@15@{refresh}@{tick}"),
            None => format!("order_book.{market}.{feed}@15@{refresh}"),
        }
    }
}

impl ParadexWssMessage {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("Failed to serialize ParadexWssMessage")
    }

    fn new(method: &str, channel: String) -> Self {
        Self::with_params(
            method,
            ParadexWssParams {
                channel,
                markets: None,
                add: None,
                remove: None,
            },
        )
    }

    fn with_params(method: &str, params: ParadexWssParams) -> Self {
        Self {
            id: 1,
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params,
        }
    }

    fn owned(markets: &[&str]) -> Vec<String> {
        markets.iter().map(|m| m.to_string()).collect()
    }

    pub fn depth(market: &str, feed: ParadexBookFeed<'_>) -> Self {
        Self::new("subscribe", feed.channel(market))
    }

    pub fn depth_unsub(market: &str, feed: ParadexBookFeed<'_>) -> Self {
        Self::new("unsubscribe", feed.channel(market))
    }

    pub fn trades(market: &str) -> Self {
        Self::new("subscribe", format!("trades.{market}"))
    }

    pub fn trades_unsub(market: &str) -> Self {
        Self::new("unsubscribe", format!("trades.{market}"))
    }

    pub fn bbo(market: &str) -> Self {
        Self::new("subscribe", format!("bbo.{market}"))
    }

    pub fn bbo_unsub(market: &str) -> Self {
        Self::new("unsubscribe", format!("bbo.{market}"))
    }

    pub fn funding(market: &str) -> Self {
        Self::new("subscribe", format!("funding_data.{market}"))
    }

    pub fn funding_unsub(market: &str) -> Self {
        Self::new("unsubscribe", format!("funding_data.{market}"))
    }

    fn markets_summary_channel(market: Option<&str>) -> String {
        match market {
            Some(m) => format!("markets_summary.{m}"),
            None => "markets_summary".to_string(),
        }
    }

    /// `markets_summary` (all markets) or `markets_summary.{market}`.
    pub fn markets_summary(market: Option<&str>) -> Self {
        Self::new("subscribe", Self::markets_summary_channel(market))
    }

    /// Unsubscribe `markets_summary[.{market}]`. A scoped subscription is closed with `None`.
    pub fn markets_summary_unsub(market: Option<&str>) -> Self {
        Self::new("unsubscribe", Self::markets_summary_channel(market))
    }

    /// Scoped `markets_summary`: `params.markets` filters the stream; the filter can be
    /// changed in place with [`Self::modify_subscription`].
    pub fn markets_summary_scoped(markets: &[&str]) -> Self {
        Self::with_params(
            "subscribe",
            ParadexWssParams {
                channel: "markets_summary".to_string(),
                markets: Some(Self::owned(markets)),
                add: None,
                remove: None,
            },
        )
    }

    /// `modify_subscription`: add and/or remove markets on a live scoped subscription
    /// (e.g. `channel = "markets_summary"`). Unset lists are omitted.
    pub fn modify_subscription(
        channel: &str,
        add: Option<&[&str]>,
        remove: Option<&[&str]>,
    ) -> Self {
        Self::with_params(
            "modify_subscription",
            ParadexWssParams {
                channel: channel.to_string(),
                markets: None,
                add: add.map(Self::owned),
                remove: remove.map(Self::owned),
            },
        )
    }

    fn markets_channel(market: Option<&str>) -> String {
        match market {
            Some(m) => format!("markets.{m}"),
            None => "markets".to_string(),
        }
    }

    /// `markets[.{market}]`: market spec change events (JSON notifications).
    pub fn markets(market: Option<&str>) -> Self {
        Self::new("subscribe", Self::markets_channel(market))
    }

    pub fn markets_unsub(market: Option<&str>) -> Self {
        Self::new("unsubscribe", Self::markets_channel(market))
    }

    /// `system_status` (JSON notifications).
    pub fn system_status() -> Self {
        Self::new("subscribe", "system_status".to_string())
    }

    pub fn system_status_unsub() -> Self {
        Self::new("unsubscribe", "system_status".to_string())
    }

    /// `funding_rate_comparison.ALL@{refresh}`: per-market funding rates across venues.
    pub fn funding_rate_comparison(refresh: ParadexFundingComparisonRefresh) -> Self {
        Self::new(
            "subscribe",
            format!("funding_rate_comparison.ALL@{}", refresh.as_str()),
        )
    }

    pub fn funding_rate_comparison_unsub(refresh: ParadexFundingComparisonRefresh) -> Self {
        Self::new(
            "unsubscribe",
            format!("funding_rate_comparison.ALL@{}", refresh.as_str()),
        )
    }
}

// ── REST response types ───────────────────────────────────────────────────────
//
// Paradex's OpenAPI marks every response field optional. Fields below are required where every
// live response carries them, and `Option` where the docs or live responses show them absent for
// some markets (e.g. option-only or perp-only fields). Many string fields are `""` when there is
// no value (e.g. `bid` with an empty book); that is passed through as-is.

/// Single market definition from `GET /v1/markets`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParadexMarket {
    pub symbol: String,
    pub base_currency: String,
    pub quote_currency: String,
    pub settlement_currency: String,
    /// `PERP`, `PERP_OPTION`, `SPOT`, `OPTION` or `FUTURE`.
    pub asset_kind: String,
    /// Margin mode: `""`, `cross`, `isolated` or `isolated_margin`.
    pub market_kind: String,
    /// `STANDARD` or `RFQ_ONLY`.
    pub trading_mode: String,
    /// Market open time, unix ms.
    pub open_at: i64,
    /// Market expiry time, unix ms; `0` for perps and spot.
    pub expiry_at: i64,
    pub price_tick_size: String,
    pub order_size_increment: String,
    pub min_notional: String,
    pub max_order_size: String,
    pub position_limit: String,
    pub max_open_orders: u32,
    pub price_bands_width: String,
    pub max_slippage: String,
    pub max_funding_rate: String,
    pub max_funding_rate_change: String,
    pub funding_period_hours: f64,
    pub funding_multiplier: f64,
    pub interest_rate: String,
    pub clamp_rate: String,
    pub tags: Vec<String>,
    /// Options only: `PUT` or `CALL`.
    pub option_type: Option<String>,
    /// Options only.
    pub strike_price: Option<String>,
}

/// Ticker snapshot from `GET /v1/markets/summary`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParadexMarketSummary {
    pub symbol: String,
    pub bid: String,
    pub bid_size: String,
    pub ask: String,
    pub ask_size: String,
    pub last_traded_price: String,
    pub mark_price: String,
    pub underlying_price: String,
    /// Raw rate for the market's own funding period, not normalised to 8h.
    pub funding_rate: String,
    pub volume_24h: String,
    pub total_volume: String,
    pub open_interest: String,
    pub price_change_rate_24h: String,
    pub created_at: i64,
    /// Options only: smoothed funding rate of the underlying future.
    pub future_funding_rate: Option<String>,
    /// Period `funding_rate` is expressed over, in minutes. Omitted for markets without funding.
    pub funding_interval_minutes: Option<f64>,
    /// Perps only.
    pub external_fair_price: Option<String>,
    /// Perps only.
    pub open_interest_long: Option<String>,
    /// Perps only.
    pub open_interest_short: Option<String>,
    /// Options only.
    pub mark_iv: Option<String>,
}

/// Order book snapshot from `GET /v1/orderbook/{market}`. Levels are `[price, size]`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParadexOrderBook {
    pub market: String,
    pub bids: Vec<[String; 2]>,
    pub asks: Vec<[String; 2]>,
    pub seq_no: u64,
    /// Unix ms.
    pub last_updated_at: i64,
}

/// Single trade from `GET /v1/trades`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParadexTrade {
    pub id: String,
    pub market: String,
    pub price: String,
    pub size: String,
    /// Taker side: `BUY` or `SELL`.
    pub side: String,
    /// `FILL`, `LIQUIDATION`, `RPI`, `UNWIND_TRANSFER`, `SETTLE_MARKET` or `BLOCK_TRADE`.
    pub trade_type: String,
    /// Unix ms.
    pub created_at: i64,
    /// Present on block-trade prints only.
    pub block_id: Option<String>,
}

/// OHLCV kline from `GET /v1/markets/klines`.
/// The wire format is a JSON array of numbers: `[ts, open, high, low, close, volume]`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(from = "(i64, f64, f64, f64, f64, f64)")]
pub struct ParadexKline {
    /// Bucket start, unix ms.
    pub ts: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
}

impl From<(i64, f64, f64, f64, f64, f64)> for ParadexKline {
    fn from((ts, open, high, low, close, volume): (i64, f64, f64, f64, f64, f64)) -> Self {
        Self {
            ts,
            open,
            high,
            low,
            close,
            volume,
        }
    }
}

/// Historical funding data from `GET /v1/funding/data`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParadexFundingEntry {
    pub market: String,
    pub funding_index: String,
    pub funding_premium: String,
    /// Raw rate for the market's own funding period.
    pub funding_rate: String,
    /// Rate normalised to 8 hours.
    pub funding_rate_8h: String,
    pub funding_period_hours: f64,
    /// Unix ms.
    pub created_at: i64,
    /// Omitted when impact aggregation did not produce the rate.
    pub impact_premium_rate: Option<String>,
}

/// Cursor-paginated list returned by Paradex REST endpoints (`/trades`, `/funding/data`).
/// `next` / `prev` are `null` when there is no further page.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParadexListResponse<T> {
    pub results: Vec<T>,
    pub next: Option<String>,
    pub prev: Option<String>,
}

/// Query for cursor-paginated endpoints. Unset fields are omitted, so the exchange defaults
/// apply (page size 100; trades accept at most 1000 per page).
#[derive(Debug, Clone, Default)]
pub struct ParadexPageQuery {
    /// Unix ms.
    pub start_at: Option<i64>,
    /// Unix ms.
    pub end_at: Option<i64>,
    pub page_size: Option<u32>,
    /// `next` or `prev` from a previous page.
    pub cursor: Option<String>,
}

/// Best bid/offer from `GET /v1/bbo/{market}` and `GET /v1/bbo/{market}/interactive`.
/// A market with an empty book returns `"0"` prices and sizes, `seq_no` 0 and
/// `last_updated_at` 0.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParadexBbo {
    pub market: String,
    pub seq_no: u64,
    pub ask: String,
    pub ask_size: String,
    pub bid: String,
    pub bid_size: String,
    /// Unix ms.
    pub last_updated_at: i64,
}

/// Order book including RPI orders from `GET /v1/orderbook/{market}/interactive`.
/// Levels are `[price, size]`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParadexInteractiveOrderBook {
    pub market: String,
    pub bids: Vec<[String; 2]>,
    pub asks: Vec<[String; 2]>,
    pub seq_no: u64,
    /// Unix ms.
    pub last_updated_at: i64,
    /// Best bid from API orders only (excluding RPI); omitted when there is none.
    pub best_bid_api: Option<[String; 2]>,
    /// Best ask from API orders only (excluding RPI); omitted when there is none.
    pub best_ask_api: Option<[String; 2]>,
    /// Documented; not seen in live responses.
    pub best_bid_interactive: Option<[String; 2]>,
    /// Documented; not seen in live responses.
    pub best_ask_interactive: Option<[String; 2]>,
}

/// Impact price from `GET /v1/orderbook/{market}/impact-price`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParadexImpactPrice {
    pub market: String,
    /// `BUY` or `SELL`.
    pub side: String,
    pub size: String,
    pub impact_price: String,
}

/// One market spec change from `GET /v1/markets/history` and the `markets[.{market}]`
/// websocket channel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParadexMarketEvent {
    pub symbol: String,
    /// `CREATED`, `DELETED`, `SETTLED` or `UPDATED`.
    pub event_type: String,
    /// Unix ms.
    pub updated_at: i64,
    pub changed_fields: ParadexMarketChangedFields,
}

/// Fields changed by a [`ParadexMarketEvent`]. Only the changed fields are present, so every
/// field is optional. Numeric spec values are JSON numbers here (strings on `/markets`).
/// Margin and fee parameter objects are documented as untyped and kept as JSON.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParadexMarketChangedFields {
    pub asset_kind: Option<String>,
    pub base_currency: Option<String>,
    pub quote_currency: Option<String>,
    pub settlement_currency: Option<String>,
    pub market_kind: Option<String>,
    pub option_type: Option<String>,
    pub strike_price: Option<String>,
    /// Unix ms.
    pub open_at: Option<i64>,
    /// Unix ms.
    pub expiry_at: Option<i64>,
    pub price_tick_size: Option<String>,
    pub order_size_increment: Option<String>,
    pub min_notional: Option<String>,
    pub max_order_size: Option<String>,
    pub position_limit: Option<String>,
    pub max_open_orders: Option<u32>,
    pub price_bands_width: Option<f64>,
    pub max_slippage: Option<f64>,
    pub max_tob_spread: Option<f64>,
    pub max_funding_rate: Option<f64>,
    pub max_funding_rate_change: Option<f64>,
    pub funding_period_hours: Option<f64>,
    pub funding_multiplier: Option<f64>,
    pub interest_rate: Option<f64>,
    pub clamp_rate: Option<f64>,
    pub oracle_ewma_factor: Option<f64>,
    pub settlement_price: Option<f64>,
    pub underlying_twap_price: Option<f64>,
    pub fee_config: Option<serde_json::Value>,
    pub delta1_cross_margin_params: Option<serde_json::Value>,
    pub option_cross_margin_params: Option<serde_json::Value>,
}

/// Settlement price from `GET /v1/markets/settlement-price` (HTTP 404 `MARKET_NOT_SETTLED`
/// before settlement).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParadexSettlementPrice {
    pub symbol: String,
    /// Option payoff for options; spot TWAP for perps and futures.
    pub settlement_price: String,
    /// 30-minute spot TWAP at expiry.
    pub underlying_twap_price: String,
    /// Scheduled expiry, unix ms.
    pub expiry_at: i64,
}

/// Insurance fund account from `GET /v1/insurance`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParadexInsuranceFund {
    pub account: String,
    pub account_value: String,
    pub settlement_asset: String,
    /// Unix ms.
    pub updated_at: i64,
}

/// Exchange status from `GET /v1/system/state` and the `system_status` websocket channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParadexSystemStatus {
    Ok,
    Maintenance,
    CancelOnly,
    PostOnly,
}

/// `GET /v1/system/state` and `system_status` notification data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParadexSystemState {
    pub status: ParadexSystemStatus,
}

/// `GET /v1/system/time`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParadexSystemTime {
    /// Unix ms, as a decimal string on the wire.
    pub server_time: String,
}

/// `GET /v1/system/stats`. Refreshed about hourly.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParadexExchangeStats {
    /// USD volume over every market ever listed.
    pub lifetime_volume: String,
}

/// Bridged token entry of [`ParadexSystemConfig`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParadexBridgedToken {
    pub name: String,
    pub symbol: String,
    pub decimals: u32,
    pub l1_token_address: String,
    pub l1_bridge_address: String,
    pub l2_token_address: String,
    pub l2_bridge_address: String,
}

/// Clearing and settlement layer config from `GET /v1/system/config`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParadexSystemConfig {
    pub environment: String,
    /// Deprecated; `""` live.
    pub starknet_gateway_url: String,
    pub starknet_fullnode_rpc_url: String,
    pub starknet_fullnode_rpc_base_url: String,
    /// Shortstring used as the typed-data `chainId` (see `signer`).
    pub starknet_chain_id: String,
    pub block_explorer_url: String,
    pub paraclear_address: String,
    pub paraclear_decimals: u32,
    pub universal_deployer_address: String,
    pub paraclear_account_proxy_hash: String,
    pub paraclear_account_hash: String,
    pub paraclear_evm_account_hash: String,
    pub oracle_address: String,
    pub bridged_tokens: Vec<ParadexBridgedToken>,
    pub l1_core_contract_address: String,
    pub l1_operator_address: String,
    pub l1_chain_id: String,
    pub liquidation_fee: String,
    pub partial_liquidation_buffer: String,
    pub partial_liquidation_share_increment: String,
    pub l1_relayer_address: String,
    pub l2_relayer_address: String,
    pub dime_staking_contract_address: String,
    pub assets_limit_per_account: u32,
    pub rfq_default_max_slippage: String,
}

/// Portfolio margin scenario of [`ParadexMarginConfig`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParadexMarginScenario {
    pub spot_shock: f64,
    pub vol_shock: f64,
    pub weight: f64,
}

/// Volatility shock parameters of [`ParadexMarginConfig`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParadexVolShockParams {
    pub dte_floor_days: f64,
    pub min_vol_shock_up: f64,
    pub vega_power_long_dte: f64,
    pub vega_power_short_dte: f64,
}

/// Per-base-asset margin configuration from `GET /v1/system/margin-config`.
/// Assets without portfolio margin parameters have no factors, empty `scenarios` and
/// `null` `vol_shock_params`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParadexMarginConfig {
    pub base_asset: String,
    pub rfq_max_slippage: String,
    pub scenarios: Vec<ParadexMarginScenario>,
    /// Always sent; `null` for assets without portfolio margin parameters.
    #[serde(deserialize_with = "required_nullable")]
    pub vol_shock_params: Option<ParadexVolShockParams>,
    pub mmf_factor: Option<f64>,
    pub hedged_margin_factor: Option<f64>,
    pub unhedged_margin_factor: Option<f64>,
    pub funding_provision_hour: Option<f64>,
}
