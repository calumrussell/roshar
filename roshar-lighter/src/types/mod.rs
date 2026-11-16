use serde::{Deserialize, Serialize};

/// Typed parsing of public websocket frames.
pub mod ws;
pub use ws::*;

// ── WebSocket messages ────────────────────────────────────────────────────────

/// Application-level keepalive. Lighter closes a connection that sends no frame for 2 minutes and
/// answers this with `{"type":"pong"}`.
pub const LIGHTER_WS_PING: &str = r#"{"type":"ping"}"#;

/// Subscribe / unsubscribe message for Lighter WebSocket streams.
/// Format: `{"type": "subscribe", "channel": "order_book/0"}`. The list form of `market_stats` and
/// `spot_market_stats` adds `market_ids`, and their `/all` and list forms accept `partial`.
/// https://docs.lighter.xyz/api/websocket
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
pub struct LighterWssMessage {
    #[serde(rename = "type")]
    pub msg_type: String,
    pub channel: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub market_ids: Option<Vec<u32>>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub partial: Option<bool>,
}

impl LighterWssMessage {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("Failed to serialize LighterWssMessage")
    }

    /// `{"type":"subscribe","channel":<channel>}` for any channel name.
    pub fn subscribe(channel: impl Into<String>) -> Self {
        Self {
            msg_type: "subscribe".to_string(),
            channel: channel.into(),
            market_ids: None,
            partial: None,
        }
    }

    /// `{"type":"unsubscribe","channel":<channel>}` for any channel name.
    pub fn unsubscribe(channel: impl Into<String>) -> Self {
        Self {
            msg_type: "unsubscribe".to_string(),
            ..Self::subscribe(channel)
        }
    }

    /// The unsubscribe message for the same channel (and `market_ids`, if any). `partial` is
    /// dropped: it only applies to subscribe.
    pub fn into_unsubscribe(self) -> Self {
        Self {
            msg_type: "unsubscribe".to_string(),
            partial: None,
            ..self
        }
    }

    pub fn depth(market_index: u32) -> Self {
        Self::subscribe(format!("order_book/{market_index}"))
    }

    pub fn depth_unsub(market_index: u32) -> Self {
        Self::unsubscribe(format!("order_book/{market_index}"))
    }

    pub fn trades(market_index: u32) -> Self {
        Self::subscribe(format!("trade/{market_index}"))
    }

    pub fn trades_unsub(market_index: u32) -> Self {
        Self::unsubscribe(format!("trade/{market_index}"))
    }

    /// `ticker/{m}`: best bid and offer.
    pub fn ticker(market_index: u32) -> Self {
        Self::subscribe(format!("ticker/{market_index}"))
    }

    /// `market_stats/{m}` (perp markets). `partial` is rejected on this form (error 30005).
    pub fn market_stats(market_index: u32) -> Self {
        Self::subscribe(format!("market_stats/{market_index}"))
    }

    /// `market_stats/all`, with `partial` sent only when `Some`.
    pub fn all_market_stats(partial: Option<bool>) -> Self {
        Self {
            partial,
            ..Self::subscribe("market_stats/all")
        }
    }

    /// Bare `market_stats` with `market_ids` (at most 500). Another subscribe adds markets.
    pub fn market_stats_list(market_ids: &[u32], partial: Option<bool>) -> Self {
        Self {
            market_ids: Some(market_ids.to_vec()),
            partial,
            ..Self::subscribe("market_stats")
        }
    }

    /// Removes `market_ids` from the bare `market_stats` subscription, or ends it when `None`.
    pub fn market_stats_list_unsub(market_ids: Option<&[u32]>) -> Self {
        Self {
            market_ids: market_ids.map(<[u32]>::to_vec),
            ..Self::unsubscribe("market_stats")
        }
    }

    /// `spot_market_stats/{m}`.
    pub fn spot_market_stats(market_index: u32) -> Self {
        Self::subscribe(format!("spot_market_stats/{market_index}"))
    }

    /// `spot_market_stats/all`, with `partial` sent only when `Some`.
    pub fn all_spot_market_stats(partial: Option<bool>) -> Self {
        Self {
            partial,
            ..Self::subscribe("spot_market_stats/all")
        }
    }

    /// Bare `spot_market_stats` with `market_ids`.
    pub fn spot_market_stats_list(market_ids: &[u32], partial: Option<bool>) -> Self {
        Self {
            market_ids: Some(market_ids.to_vec()),
            partial,
            ..Self::subscribe("spot_market_stats")
        }
    }

    /// Removes `market_ids` from the bare `spot_market_stats` subscription, or ends it when `None`.
    pub fn spot_market_stats_list_unsub(market_ids: Option<&[u32]>) -> Self {
        Self {
            market_ids: market_ids.map(<[u32]>::to_vec),
            ..Self::unsubscribe("spot_market_stats")
        }
    }

    /// `bo_market_stats/{m}` (binary options).
    pub fn bo_market_stats(market_index: u32) -> Self {
        Self::subscribe(format!("bo_market_stats/{market_index}"))
    }

    /// `bo_market_stats/all`. Includes settled and not-yet-open markets.
    pub fn all_bo_market_stats() -> Self {
        Self::subscribe("bo_market_stats/all")
    }

    /// `candle/{m}/{resolution}`: trade candles. Resolutions: `1m`, `5m`, `15m`, `30m`, `1h`,
    /// `4h`, `12h`, `1d`.
    pub fn candle(market_index: u32, resolution: &str) -> Self {
        Self::subscribe(format!("candle/{market_index}/{resolution}"))
    }

    /// `mark_price_candle/{m}/{resolution}`. Same resolutions as `candle`.
    pub fn mark_price_candle(market_index: u32, resolution: &str) -> Self {
        Self::subscribe(format!("mark_price_candle/{market_index}/{resolution}"))
    }

    /// `up_down_candle/{symbol}/{duration}/{resolution}`: underlying price of an Up/Down series,
    /// e.g. `("eth", "15m", "1s")`. Resolutions: `1s`, `30s`.
    pub fn up_down_candle(symbol: &str, duration: &str, resolution: &str) -> Self {
        Self::subscribe(format!("up_down_candle/{symbol}/{duration}/{resolution}"))
    }

    /// `height`: L2 block height.
    pub fn height() -> Self {
        Self::subscribe("height")
    }
}

// ── REST response types ───────────────────────────────────────────────────────
//
// Field names, JSON types and optionality follow the OpenAPI schemas on docs.lighter.xyz/api and
// were checked against mainnet on 2026-11-15. `Option` marks fields the schema does not require
// (and that live responses omit for some markets or trades). Fields not listed are ignored.

/// Market metadata from `GET /api/v1/orderBooks` (`OrderBook` schema). Covers perp and spot
/// markets.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LighterMarket {
    pub symbol: String,
    pub market_id: u32,
    /// `perp`, `spot` or `binary_options`.
    pub market_type: String,
    pub base_asset_id: i64,
    pub quote_asset_id: i64,
    /// `inactive`, `active`, `in-settlement`, `waiting-for-settlement` or `settled`.
    pub status: String,
    /// Percent.
    pub taker_fee: String,
    pub is_taker_fee_enabled: bool,
    /// Percent.
    pub maker_fee: String,
    pub is_maker_fee_enabled: bool,
    pub liquidation_fee: String,
    pub min_base_amount: String,
    pub min_quote_amount: String,
    pub order_quote_limit: String,
    pub supported_size_decimals: u32,
    pub supported_price_decimals: u32,
    pub supported_quote_decimals: u32,
    /// Unix ms as a decimal string.
    pub created_at: String,
    pub multiplier: String,
    pub start_timestamp: i64,
    pub end_timestamp: i64,
    pub open_interest: String,
    /// Absent for spot markets.
    pub open_interest_limit: Option<String>,
    /// Absent for spot markets.
    pub open_interest_usd: Option<String>,
    pub outcome: i64,
    pub operator_account_index: i64,
    pub settlement_cap: i64,
    pub settlement_type: i64,
    pub settlement_price: i64,
    pub default_price: i64,
    pub is_frozen: bool,
    pub market_deployer: String,
}

/// Shared fields of every `orderBookDetails` entry (the `OrderBook` fields that the perp and spot
/// detail schemas both carry).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LighterMarketDetailCommon {
    pub symbol: String,
    pub market_id: u32,
    /// `perp`, `spot` or `binary_options`.
    pub market_type: String,
    pub base_asset_id: i64,
    pub quote_asset_id: i64,
    /// `inactive`, `active`, `in-settlement`, `waiting-for-settlement` or `settled`.
    pub status: String,
    pub taker_fee: String,
    pub is_taker_fee_enabled: bool,
    pub maker_fee: String,
    pub is_maker_fee_enabled: bool,
    pub liquidation_fee: String,
    pub min_base_amount: String,
    pub min_quote_amount: String,
    pub order_quote_limit: String,
    pub supported_size_decimals: u32,
    pub supported_price_decimals: u32,
    pub supported_quote_decimals: u32,
    /// Unix ms as a decimal string.
    pub created_at: String,
    pub multiplier: String,
    /// Unix ms.
    pub start_timestamp: i64,
    /// Unix ms.
    pub end_timestamp: i64,
    pub outcome: i64,
    pub operator_account_index: i64,
    pub settlement_cap: i64,
    pub settlement_type: i64,
    pub settlement_price: i64,
    pub default_price: i64,
    pub is_frozen: bool,
    pub market_deployer: String,
    pub size_decimals: u32,
    pub price_decimals: u32,
    pub last_trade_price: f64,
    pub daily_trades_count: i64,
    pub daily_base_token_volume: f64,
    pub daily_quote_token_volume: f64,
    pub daily_price_low: f64,
    pub daily_price_high: f64,
    /// Percent.
    pub daily_price_change: f64,
    /// Hourly prices keyed by Unix seconds (as a string). Empty for markets without trades.
    pub daily_chart: std::collections::BTreeMap<String, f64>,
}

/// Perp market detail from `GET /api/v1/orderBookDetails` (`order_book_details[]`,
/// `PerpsOrderBookDetail` schema). Numbers the API sends as JSON numbers are `f64`, numbers it
/// sends as strings stay `String`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LighterMarketDetail {
    #[serde(flatten)]
    pub common: LighterMarketDetailCommon,
    pub quote_multiplier: i64,
    /// Basis points of 1 (`500` = 5%), as are the other margin fractions.
    pub default_initial_margin_fraction: u32,
    pub min_initial_margin_fraction: u32,
    pub maintenance_margin_fraction: u32,
    pub closeout_margin_fraction: u32,
    pub mark_price: String,
    pub index_price: String,
    /// Base units (a JSON number here, a string in `orderBooks`).
    pub open_interest: f64,
    pub open_interest_limit: String,
    pub open_interest_usd: String,
    pub market_config: LighterMarketConfig,
    pub strategy_index: u32,
    pub market_flags: i64,
    pub funding_premium_multiplier: u32,
    pub funding_clamp_small: String,
    pub funding_clamp_big: String,
    pub base_interest_rate: String,
}

/// `market_config` of a perp market detail (`MarketConfig` schema).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LighterMarketConfig {
    pub market_margin_mode: i64,
    pub insurance_fund_account_index: i64,
    pub liquidation_mode: i64,
    pub force_reduce_only: bool,
    /// Not required by the schema.
    pub funding_fee_discounts_enabled: Option<bool>,
    pub trading_hours: String,
    pub hidden: bool,
    pub rfq_enabled: bool,
}

/// Spot market detail from `GET /api/v1/orderBookDetails` (`spot_order_book_details[]`,
/// `SpotOrderBookDetail` schema).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LighterSpotMarketDetail {
    #[serde(flatten)]
    pub common: LighterMarketDetailCommon,
    pub open_interest: String,
    /// Not sent for spot markets today; optional in the schema.
    pub open_interest_limit: Option<String>,
    /// Not sent for spot markets today; optional in the schema.
    pub open_interest_usd: Option<String>,
}

/// `GET /api/v1/orderBookDetails` (`OrderBookDetails` schema). Both lists are always present;
/// `filter` empties the other one.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LighterOrderBookDetails {
    pub order_book_details: Vec<LighterMarketDetail>,
    pub spot_order_book_details: Vec<LighterSpotMarketDetail>,
}

/// Resting orders from `GET /api/v1/orderBookOrders` (`OrderBookOrders` schema). Each side is a
/// list of individual orders, not aggregated price levels.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LighterOrderBook {
    pub total_asks: i64,
    pub asks: Vec<LighterSimpleOrder>,
    pub total_bids: i64,
    pub bids: Vec<LighterSimpleOrder>,
}

/// One resting order (`SimpleOrder` schema).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LighterSimpleOrder {
    pub order_index: i64,
    pub order_id: String,
    pub owner_account_index: i64,
    pub initial_base_amount: String,
    pub remaining_base_amount: String,
    pub price: String,
    pub order_expiry: i64,
    /// Unix microseconds.
    pub transaction_time: i64,
}

/// One trade from `GET /api/v1/recentTrades` (`Trade` schema). The optional fields are omitted by
/// the API when zero or empty.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LighterTrade {
    pub trade_id: i64,
    pub trade_id_str: String,
    pub tx_hash: String,
    /// `trade`, `liquidation`, `deleverage` or `market-settlement`.
    #[serde(rename = "type")]
    pub trade_type: String,
    pub market_id: u32,
    pub market_symbol: String,
    /// `perps`, `spot` or `bo`.
    pub market_kind: String,
    pub size: String,
    pub price: String,
    pub usd_amount: String,
    pub ask_id: i64,
    /// Not required by the schema; sent on every live trade.
    pub ask_id_str: Option<String>,
    pub bid_id: i64,
    /// Not required by the schema; sent on every live trade.
    pub bid_id_str: Option<String>,
    /// Realized PnL of the queried account (account-filtered `trades` only).
    pub ask_account_pnl: Option<String>,
    pub bid_account_pnl: Option<String>,
    pub ask_client_id: i64,
    pub ask_client_id_str: String,
    pub bid_client_id: i64,
    pub bid_client_id_str: String,
    pub ask_account_id: i64,
    pub bid_account_id: i64,
    pub is_maker_ask: bool,
    pub block_height: i64,
    /// Unix ms.
    pub timestamp: i64,
    /// Unix microseconds.
    pub transaction_time: i64,
    pub ask_order_version: i64,
    pub bid_order_version: i64,
    pub taker_fee: Option<i64>,
    pub taker_position_size_before: Option<String>,
    pub taker_entry_quote_before: Option<String>,
    pub taker_initial_margin_fraction_before: Option<u32>,
    pub taker_position_sign_changed: Option<bool>,
    pub maker_fee: Option<i64>,
    pub maker_position_size_before: Option<String>,
    pub maker_entry_quote_before: Option<String>,
    pub maker_initial_margin_fraction_before: Option<u32>,
    pub maker_position_sign_changed: Option<bool>,
    pub integrator_taker_fee: Option<i64>,
    pub integrator_taker_fee_collector_index: Option<i64>,
    pub integrator_maker_fee: Option<i64>,
    pub integrator_maker_fee_collector_index: Option<i64>,
    pub taker_allocated_margin_usdc_before: Option<i64>,
    pub taker_allocated_margin_usdc_after: Option<i64>,
    pub maker_allocated_margin_usdc_before: Option<i64>,
    pub maker_allocated_margin_usdc_after: Option<i64>,
}

/// One entry from `GET /api/v1/funding-rates` (`FundingRate` schema): the current rate of one
/// market on one venue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LighterFundingRate {
    pub market_id: u32,
    /// `binance`, `bybit`, `hyperliquid` or `lighter`.
    pub exchange: String,
    pub symbol: String,
    pub rate: f64,
}

/// Wrapper returned by the `/orderBooks` endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LighterMarketsResponse {
    pub order_books: Vec<LighterMarket>,
}

/// Wrapper returned by `/recentTrades` and `/trades`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LighterTradesResponse {
    /// `/trades` only: pass back as `cursor` for the next page.
    pub next_cursor: Option<String>,
    pub trades: Vec<LighterTrade>,
}

/// Wrapper returned by the `/funding-rates` endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LighterFundingRatesResponse {
    pub funding_rates: Vec<LighterFundingRate>,
}

/// `GET /` (`Status` schema).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LighterStatus {
    /// `200` when up.
    pub status: i64,
    pub network_id: i64,
    /// Unix seconds.
    pub timestamp: i64,
}

/// One trade candle (`Candle` schema), from `GET /api/v1/candles` and the `candle` channel.
/// The REST API omits zero values, so `v`, `V` and `i` default to 0.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LighterCandle {
    /// Open time, Unix ms (close time with `set_timestamp_to_end`).
    pub t: i64,
    pub o: f64,
    pub h: f64,
    pub l: f64,
    pub c: f64,
    /// Base volume.
    #[serde(default)]
    pub v: f64,
    /// Quote volume.
    #[serde(rename = "V", default)]
    pub quote_volume: f64,
    /// Last trade id.
    #[serde(default)]
    pub i: i64,
    /// Unadjusted open, sent only when it differs from `o`.
    #[serde(rename = "O")]
    pub unadjusted_open: Option<f64>,
    #[serde(rename = "H")]
    pub unadjusted_high: Option<f64>,
    #[serde(rename = "L")]
    pub unadjusted_low: Option<f64>,
    #[serde(rename = "C")]
    pub unadjusted_close: Option<f64>,
}

/// `GET /api/v1/candles` (`Candles` schema).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LighterCandles {
    /// Resolution.
    pub r: String,
    pub c: Vec<LighterCandle>,
}

/// One mark-price candle (`MarkPriceCandle` schema), from `GET /api/v1/markPriceCandles` and the
/// `mark_price_candle` channel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LighterMarkPriceCandle {
    /// Unix ms.
    pub t: i64,
    pub o: f64,
    pub h: f64,
    pub l: f64,
    pub c: f64,
    /// Mark price samples in the candle.
    pub sc: i64,
}

/// `GET /api/v1/markPriceCandles` (`MarkPriceCandles` schema).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LighterMarkPriceCandles {
    pub r: String,
    pub c: Vec<LighterMarkPriceCandle>,
}

/// `GET /api/v1/marketPriceCharts` (`MarketPriceCharts` schema): last 24 hourly prices (mark
/// price for perps, index price for spot).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LighterMarketPriceCharts {
    pub resolution: String,
    pub price_charts: Vec<LighterMarketPriceChart>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LighterMarketPriceChart {
    pub market_id: u32,
    /// Oldest first.
    pub prices: Vec<String>,
}

/// One funding payment from `GET /api/v1/fundings` (`Funding` schema).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LighterFunding {
    /// Unix seconds.
    pub timestamp: i64,
    pub value: String,
    /// Percent.
    pub rate: String,
    /// `long` or `short`: the side that pays.
    pub direction: String,
}

/// `GET /api/v1/fundings` (`Fundings` schema).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LighterFundings {
    pub resolution: String,
    pub fundings: Vec<LighterFunding>,
}

/// `GET /api/v1/exchangeStats` (`ExchangeStats` schema).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LighterExchangeStats {
    pub total: i64,
    pub order_book_stats: Vec<LighterOrderBookStats>,
    pub daily_usd_volume: f64,
    pub daily_trades_count: i64,
}

/// Per-market 24h stats (`OrderBookStats` schema).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LighterOrderBookStats {
    pub symbol: String,
    pub last_trade_price: f64,
    pub daily_trades_count: i64,
    pub daily_base_token_volume: f64,
    pub daily_quote_token_volume: f64,
    pub daily_price_change: f64,
}

/// One asset from `GET /api/v1/assetDetails` (`Asset` schema). Spot markets reference these by
/// `base_asset_id` / `quote_asset_id`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LighterAsset {
    pub asset_id: i64,
    pub symbol: String,
    pub l1_decimals: u32,
    pub decimals: u32,
    pub min_transfer_amount: String,
    pub min_withdrawal_amount: String,
    /// `enabled`, `disabled` or `priced_only`.
    pub margin_mode: String,
    pub index_price: String,
    pub price_decimals: u32,
    pub l1_address: String,
    pub loan_to_value: String,
    pub liquidation_threshold: String,
    pub liquidation_factor: String,
    pub liquidation_fee: String,
    pub global_supply_cap: String,
    pub user_supply_cap: String,
    pub total_supplied: String,
    pub multiplier: String,
}

/// `GET /api/v1/assetDetails` (`AssetDetails` schema).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LighterAssetDetails {
    pub asset_details: Vec<LighterAsset>,
}

/// `GET /api/v1/priceOracleInfo` (`RespPriceOracleInfo` schema).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LighterPriceOracleInfo {
    pub computed_at_ms: i64,
    pub markets: Vec<LighterPriceOracleMarket>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LighterPriceOracleMarket {
    pub symbol: String,
    /// True while the market is pre-market (required on Lighter mainnet).
    pub pre_market: bool,
    pub index: LighterPriceOraclePrice,
    pub mark: LighterPriceOraclePrice,
}

/// How one index or mark price is sourced (`PriceOraclePrice` schema). The last four fields are
/// not in the schema but are sent for every market.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LighterPriceOraclePrice {
    pub source: String,
    pub using_median: bool,
    pub using_internal_price: bool,
    pub median_sources: Vec<String>,
    pub price_cap_bps: f64,
    /// Cap bounds are `ref * (1 -/+ bps / 10000)`.
    pub price_cap_reference_price: f64,
    /// Oracle feed identifier; sent for some markets only.
    pub underlying: Option<String>,
    pub last_accepted_at_ms: i64,
    pub awaiting_sources: bool,
    pub awaiting_verification: bool,
    pub stale: bool,
}

/// `GET /api/v1/feeTiers` (`FeeTiers` schema). Fields of the tiers that the schema does not list
/// (`twap_fee_ticks`, `send_tx_rate_limit`, …) are ignored.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LighterFeeTiers {
    pub user_tiers: Vec<LighterUserFeeTier>,
    pub bo_tiers: Vec<LighterBoFeeTier>,
    pub staking_tiers: Vec<LighterStakingFeeTier>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LighterUserFeeTier {
    pub name: String,
    pub level: i64,
    pub maker_fee_ticks: i64,
    pub taker_fee_ticks: i64,
    pub required_lit_stakes: i64,
    pub required_volume: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LighterBoFeeTier {
    pub name: String,
    pub required_volume: i64,
    pub fee_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LighterStakingFeeTier {
    pub name: String,
    pub required_lit_stakes: i64,
    pub fee_rate: f64,
}

/// Binary-options market stats (`BinaryOptionMarketStats` schema): `events` `stats` and the
/// `bo_market_stats` channel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LighterBoMarketStats {
    pub symbol: String,
    pub market_id: u32,
    /// Empty when that side of the book is empty.
    pub mid_price: String,
    pub best_ask_price: String,
    pub best_bid_price: String,
    pub last_trade_price: String,
    pub daily_base_token_volume: f64,
    pub daily_quote_token_volume: f64,
    pub daily_price_low: f64,
    pub daily_price_high: f64,
    pub daily_price_change: f64,
    pub lifetime_quote_token_volume: f64,
    /// Outcome shares traded over the market's life.
    pub lifetime_base_token_volume: f64,
    pub open_interest: String,
    /// `inactive`, `active`, `in-settlement`, `waiting-for-settlement` or `settled`.
    pub status: String,
    /// `0` none, `1` No/Down, `2` Yes/Up.
    pub outcome: i64,
}

/// A binary-options market inside an event (`EventOrderBookEntry` schema): the `orderBooks`
/// fields plus `size_decimals`, `price_decimals` and `stats`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LighterEventMarket {
    #[serde(flatten)]
    pub market: LighterMarket,
    pub size_decimals: u32,
    pub price_decimals: u32,
    pub stats: LighterBoMarketStats,
}

/// One event from `GET /api/v1/events` (`EventDetail` schema).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LighterEvent {
    pub event_id: i64,
    pub title: String,
    pub description: String,
    pub creator_account_index: i64,
    /// Unix ms.
    pub created_at: i64,
    pub tag: String,
    /// `one_off` or `recurrent`.
    pub event_kind: String,
    pub recurrence_period_seconds: i64,
    /// Unix ms.
    pub start_at: i64,
    /// Unix ms.
    pub end_at: i64,
    /// Free-form; Up/Down events carry `symbol`, `duration`, `provider`, `feed_id`, ….
    pub metadata: serde_json::Map<String, serde_json::Value>,
    pub order_books: Vec<LighterEventMarket>,
    pub lifetime_quote_token_volume: f64,
}

/// `GET /api/v1/events` (`RespGetEvents` schema).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LighterEvents {
    /// Pass back as `cursor`; absent on the last page.
    pub next_cursor: Option<i64>,
    pub events: Vec<LighterEvent>,
}

/// One Up/Down underlying-price candle (`UpDownCandle` schema), from `GET /api/v1/upDownCandles`
/// and the `up_down_candle` channel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LighterUpDownCandle {
    pub symbol: String,
    pub duration: String,
    pub resolution: String,
    /// Bucket start, Unix seconds.
    pub timestamp: i64,
    pub open: String,
    pub high: String,
    pub low: String,
    pub close: String,
    pub provider: String,
    pub feed_id: String,
}

/// `GET /api/v1/upDownCandles` (`RespGetUpDownCandles` schema).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LighterUpDownCandles {
    /// Oldest first.
    pub candles: Vec<LighterUpDownCandle>,
    /// `start_timestamp` of the next page; 0 when the range is exhausted. Optional in the schema.
    pub next_cursor: Option<i64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ping_is_documented_form() {
        assert_eq!(LIGHTER_WS_PING, r#"{"type":"ping"}"#);
        assert_eq!(
            LighterWssMessage::depth(0).to_json(),
            r#"{"type":"subscribe","channel":"order_book/0"}"#
        );
    }

    #[test]
    fn parses_live_shapes() {
        // Spot entry from mainnet orderBooks (no open_interest_limit / open_interest_usd).
        let m: LighterMarketsResponse = serde_json::from_str(r#"{"code":200,"order_books":[{"symbol":"ETH/USDC","market_id":2048,"market_type":"spot","base_asset_id":1,"quote_asset_id":3,"status":"active","taker_fee":"0.0000","is_taker_fee_enabled":true,"maker_fee":"0.0000","is_maker_fee_enabled":true,"liquidation_fee":"0.0000","min_base_amount":"0.0050","min_quote_amount":"10.000000","order_quote_limit":"281474976.710655","supported_size_decimals":4,"supported_price_decimals":2,"supported_quote_decimals":6,"created_at":"1752000000000","multiplier":"1.000000000000000000","start_timestamp":0,"end_timestamp":0,"open_interest":"0","outcome":0,"operator_account_index":1,"fee_collector_account_index":0,"settlement_cap":0,"settlement_type":0,"settlement_price":0,"default_price":0,"is_frozen":false,"market_deployer":"lighter"}]}"#).unwrap();
        assert!(m.order_books[0].open_interest_usd.is_none());

        let b: LighterOrderBook = serde_json::from_str(r#"{"code":200,"total_asks":1,"asks":[{"order_index":281478291218511,"order_id":"281478291218511","owner_account_index":366058,"initial_base_amount":"5.4892","remaining_base_amount":"5.4892","price":"1822.14","order_expiry":1794781095497,"transaction_time":0}],"total_bids":0,"bids":[]}"#).unwrap();
        assert_eq!(b.asks[0].price, "1822.14");

        let t: LighterTradesResponse = serde_json::from_str(r#"{"code":200,"trades":[{"trade_id":40605550550,"trade_id_str":"40605550550","tx_hash":"00","type":"trade","market_id":0,"market_symbol":"ETH","market_kind":"perps","size":"0.0005","price":"1821.83","usd_amount":"0.910915","ask_id":281478291218598,"ask_id_str":"281478291218598","bid_id":562946521103272,"bid_id_str":"562946521103272","ask_client_id":117622656008192,"ask_client_id_str":"117622656008192","bid_client_id":0,"bid_client_id_str":"0","ask_account_id":768370,"bid_account_id":719354,"is_maker_ask":true,"block_height":389140908,"timestamp":1794779311041,"taker_fee":100,"maker_fee":40,"transaction_time":1794779311043326,"ask_order_version":84,"bid_order_version":0}]}"#).unwrap();
        assert_eq!(t.trades[0].taker_fee, Some(100));
        assert!(t.trades[0].taker_position_sign_changed.is_none());

        let f: LighterFundingRatesResponse = serde_json::from_str(
            r#"{"code":200,"funding_rates":[{"market_id":59,"exchange":"binance","symbol":"HBAR","rate":0.00005003}]}"#,
        )
        .unwrap();
        assert_eq!(f.funding_rates[0].exchange, "binance");
    }
}
