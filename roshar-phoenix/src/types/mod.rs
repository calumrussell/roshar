use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;

// ── Wire helpers ──────────────────────────────────────────────────────────────

/// Phoenix `JsSafeU64`/`JsSafeI64`: 64-bit integers sent as decimal strings so JS
/// clients keep full precision. Like the reference SDK (`PickFirst<(DisplayFromStr, _)>`),
/// a bare JSON integer is also accepted.
#[derive(Deserialize)]
#[serde(untagged)]
enum JsSafeInt<T> {
    Str(String),
    Num(T),
}

fn js_safe_u64<'de, D: Deserializer<'de>>(d: D) -> Result<u64, D::Error> {
    match JsSafeInt::<u64>::deserialize(d)? {
        JsSafeInt::Str(s) => s.parse().map_err(serde::de::Error::custom),
        JsSafeInt::Num(n) => Ok(n),
    }
}

fn js_safe_i64<'de, D: Deserializer<'de>>(d: D) -> Result<i64, D::Error> {
    match JsSafeInt::<i64>::deserialize(d)? {
        JsSafeInt::Str(s) => s.parse().map_err(serde::de::Error::custom),
        JsSafeInt::Num(n) => Ok(n),
    }
}

/// Optional `JsSafeU64` (string or number) that may be absent or null.
fn js_safe_u64_opt<'de, D: Deserializer<'de>>(d: D) -> Result<Option<u64>, D::Error> {
    match Option::<JsSafeInt<u64>>::deserialize(d)? {
        None => Ok(None),
        Some(JsSafeInt::Str(s)) => s.parse().map(Some).map_err(serde::de::Error::custom),
        Some(JsSafeInt::Num(n)) => Ok(Some(n)),
    }
}

/// For fields the server always sends but which may be `null`: the key is required
/// (a missing key is an error), the value is nullable.
fn nullable<'de, D: Deserializer<'de>, T: Deserialize<'de>>(d: D) -> Result<Option<T>, D::Error> {
    Option::<T>::deserialize(d)
}

/// Fixed-point number as Phoenix sends it: `value / 10^decimals`, plus a display string.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PhoenixDecimal {
    pub value: i64,
    pub decimals: i8,
    pub ui: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PhoenixSide {
    Bid,
    Ask,
}

// ── WebSocket client messages ─────────────────────────────────────────────────

/// Client → server envelope: `{"type":"subscribe"|"unsubscribe","subscription":{...}}`.
/// The protocol has no JSON ping; keepalive is WebSocket ping/pong frames.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PhoenixWssMessage {
    #[serde(rename = "type")]
    pub msg_type: String,
    pub subscription: PhoenixSubscription,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PhoenixSubscription {
    pub channel: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeframe: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authority: Option<String>,
    #[serde(rename = "traderPdaIndex", skip_serializing_if = "Option::is_none")]
    pub trader_pda_index: Option<u8>,
    #[serde(
        rename = "bypassExecutionBand",
        skip_serializing_if = "Option::is_none"
    )]
    pub bypass_execution_band: Option<bool>,
    /// `exchange` channel only: `"json"` for a JSON snapshot; omitted for the default
    /// `base64+zstd` encoded snapshot.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encoding: Option<String>,
}

impl PhoenixWssMessage {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("Failed to serialize PhoenixWssMessage")
    }

    fn sub(channel: &str) -> Self {
        Self {
            msg_type: "subscribe".to_string(),
            subscription: PhoenixSubscription {
                channel: channel.to_string(),
                symbol: None,
                timeframe: None,
                authority: None,
                trader_pda_index: None,
                bypass_execution_band: None,
                encoding: None,
            },
        }
    }

    fn unsub(channel: &str) -> Self {
        Self {
            msg_type: "unsubscribe".to_string(),
            subscription: PhoenixSubscription {
                channel: channel.to_string(),
                symbol: None,
                timeframe: None,
                authority: None,
                trader_pda_index: None,
                bypass_execution_band: None,
                encoding: None,
            },
        }
    }

    /// Subscribe to mid prices for all markets.
    pub fn all_mids() -> Self {
        Self::sub("allMids")
    }

    pub fn all_mids_unsub() -> Self {
        Self::unsub("allMids")
    }

    /// Subscribe to funding rate updates for a market symbol (e.g. "SOL").
    pub fn funding_rate(symbol: &str) -> Self {
        let mut msg = Self::sub("fundingRate");
        msg.subscription.symbol = Some(symbol.to_string());
        msg
    }

    pub fn funding_rate_unsub(symbol: &str) -> Self {
        let mut msg = Self::unsub("fundingRate");
        msg.subscription.symbol = Some(symbol.to_string());
        msg
    }

    /// Subscribe to exchange/market configuration: one snapshot, then ordered deltas.
    /// `encoding`: `Some("json")` for a JSON snapshot; `None` for the server default
    /// (`base64+zstd` encoded snapshot).
    pub fn exchange(encoding: Option<&str>) -> Self {
        let mut msg = Self::sub("exchange");
        msg.subscription.encoding = encoding.map(str::to_string);
        msg
    }

    pub fn exchange_unsub(encoding: Option<&str>) -> Self {
        let mut msg = Self::unsub("exchange");
        msg.subscription.encoding = encoding.map(str::to_string);
        msg
    }

    /// Subscribe to L2 orderbook updates for a market. `bypass_execution_band: Some(true)`
    /// returns the full book during commodities after-hours (levels outside the
    /// execution band); omitted when `None`.
    pub fn orderbook(symbol: &str, bypass_execution_band: Option<bool>) -> Self {
        let mut msg = Self::sub("orderbook");
        msg.subscription.symbol = Some(symbol.to_string());
        msg.subscription.bypass_execution_band = bypass_execution_band;
        msg
    }

    /// Pass the same `bypass_execution_band` used to subscribe.
    pub fn orderbook_unsub(symbol: &str, bypass_execution_band: Option<bool>) -> Self {
        let mut msg = Self::unsub("orderbook");
        msg.subscription.symbol = Some(symbol.to_string());
        msg.subscription.bypass_execution_band = bypass_execution_band;
        msg
    }

    /// Subscribe to market stats (mark price, OI, funding, volume) for a market.
    pub fn market(symbol: &str) -> Self {
        let mut msg = Self::sub("market");
        msg.subscription.symbol = Some(symbol.to_string());
        msg
    }

    pub fn market_unsub(symbol: &str) -> Self {
        let mut msg = Self::unsub("market");
        msg.subscription.symbol = Some(symbol.to_string());
        msg
    }

    /// Subscribe to trade executions for a market.
    pub fn trades(symbol: &str) -> Self {
        let mut msg = Self::sub("trades");
        msg.subscription.symbol = Some(symbol.to_string());
        msg
    }

    pub fn trades_unsub(symbol: &str) -> Self {
        let mut msg = Self::unsub("trades");
        msg.subscription.symbol = Some(symbol.to_string());
        msg
    }

    /// Subscribe to OHLCV candles for a market and timeframe.
    /// Documented timeframes: "1s","5s","1m","5m","15m","30m","1h","4h","1d".
    pub fn candles(symbol: &str, timeframe: &str) -> Self {
        let mut msg = Self::sub("candles");
        msg.subscription.symbol = Some(symbol.to_string());
        msg.subscription.timeframe = Some(timeframe.to_string());
        msg
    }

    pub fn candles_unsub(symbol: &str, timeframe: &str) -> Self {
        let mut msg = Self::unsub("candles");
        msg.subscription.symbol = Some(symbol.to_string());
        msg.subscription.timeframe = Some(timeframe.to_string());
        msg
    }

    /// Subscribe to trader state updates (snapshot then deltas) for an authority.
    pub fn trader_state(authority: &str, trader_pda_index: u8) -> Self {
        let mut msg = Self::sub("traderState");
        msg.subscription.authority = Some(authority.to_string());
        msg.subscription.trader_pda_index = Some(trader_pda_index);
        msg
    }

    pub fn trader_state_unsub(authority: &str, trader_pda_index: u8) -> Self {
        let mut msg = Self::unsub("traderState");
        msg.subscription.authority = Some(authority.to_string());
        msg.subscription.trader_pda_index = Some(trader_pda_index);
        msg
    }
}

// ── WebSocket server messages ─────────────────────────────────────────────────

/// `allMids` push: mid prices for every listed market.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhoenixAllMids {
    pub mids: HashMap<String, f64>,
    pub slot: u64,
    pub slot_index: u32,
}

/// `fundingRate` push for a single market.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhoenixFundingRate {
    pub symbol: String,
    pub funding: f64,
}

/// `market` push: mark price, OI, volume, funding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhoenixMarketStats {
    pub symbol: String,
    #[serde(rename = "openInterest")]
    pub open_interest: f64,
    #[serde(rename = "markPx")]
    pub mark_price: f64,
    /// Omitted by the server when the book has no midpoint (empty or one-sided).
    #[serde(rename = "midPx")]
    pub mid_price: Option<f64>,
    #[serde(rename = "oraclePx")]
    pub oracle_price: f64,
    #[serde(rename = "prevDayPx")]
    pub prev_day_mark_price: f64,
    #[serde(rename = "dayNtlVlm")]
    pub day_volume_usd: f64,
    #[serde(rename = "funding")]
    pub funding_rate: f64,
}

/// `orderbook` push.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhoenixL2BookUpdate {
    pub symbol: String,
    pub slot: u64,
    pub slot_index: u32,
    pub orderbook: PhoenixL2Book,
    /// Present (true) only on subscriptions made with `bypassExecutionBand: true`;
    /// the server omits it otherwise.
    #[serde(default)]
    pub bypass_execution_band: bool,
}

/// Book levels as `[price, size]`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhoenixL2Book {
    pub bids: Vec<[f64; 2]>,
    pub asks: Vec<[f64; 2]>,
    pub mid: Option<f64>,
}

/// `trades` push.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhoenixTradesMessage {
    pub symbol: String,
    pub trades: Vec<PhoenixTradeEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhoenixTradeEvent {
    #[serde(deserialize_with = "js_safe_u64")]
    pub slot: u64,
    pub slot_index: u32,
    /// Unix seconds.
    #[serde(deserialize_with = "js_safe_u64")]
    pub timestamp: u64,
    pub symbol: String,
    /// Taker trader account (PDA) key.
    pub taker: String,
    #[serde(deserialize_with = "js_safe_u64")]
    pub trade_sequence_number: u64,
    /// Side of the taker order.
    pub side: PhoenixSide,
    #[serde(deserialize_with = "js_safe_u64")]
    pub base_lots_filled: u64,
    #[serde(deserialize_with = "js_safe_u64")]
    pub quote_lots_filled: u64,
    #[serde(deserialize_with = "js_safe_u64")]
    pub fee_in_quote_lots: u64,
    pub base_amount: f64,
    pub quote_amount: f64,
    pub num_fills: u32,
}

/// `candles` push (server sends `channel: "candle"`). `candle.time` is unix seconds here.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhoenixCandleUpdate {
    pub symbol: String,
    pub timeframe: String,
    pub candle: PhoenixCandle,
}

/// Any server frame, discriminated by `channel`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "channel")]
pub enum PhoenixWsServerMessage {
    #[serde(rename = "allMids")]
    AllMids(PhoenixAllMids),
    #[serde(rename = "fundingRate")]
    FundingRate(PhoenixFundingRate),
    #[serde(rename = "orderbook")]
    Orderbook(PhoenixL2BookUpdate),
    #[serde(rename = "market")]
    Market(PhoenixMarketStats),
    #[serde(rename = "trades")]
    Trades(PhoenixTradesMessage),
    #[serde(rename = "candle")]
    Candle(PhoenixCandleUpdate),
    #[serde(rename = "exchange")]
    Exchange(PhoenixExchangeMessage),
    /// Trader-specific; payload left untyped (outside this crate's market-data scope).
    #[serde(rename = "traderState")]
    TraderState(serde_json::Value),
    #[serde(rename = "subscriptionStatus")]
    SubscriptionStatus(PhoenixSubscriptionStatus),
    #[serde(rename = "error")]
    Error(PhoenixWsError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PhoenixSubscriptionState {
    Subscribed,
    Unsubscribed,
}

/// Subscribe/unsubscribe ack: `{"channel":"subscriptionStatus","status":..,"subscription":..,"clientId":..}`.
/// (The documented `subscriptionConfirmed` / `subscriptionError` frames are never sent.)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhoenixSubscriptionStatus {
    pub status: PhoenixSubscriptionState,
    pub subscription: PhoenixSubscription,
    pub client_id: String,
}

/// `{"channel":"error","code":400,"error":"...","subscription":{...}}`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhoenixWsError {
    pub code: u16,
    pub error: String,
    /// Echo of the rejected request; absent when the frame was unparseable.
    pub subscription: Option<PhoenixSubscription>,
}

/// `exchange` channel payload, discriminated by `messageType`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "messageType", rename_all = "camelCase")]
pub enum PhoenixExchangeMessage {
    /// Sent first when subscribed with `encoding: "json"`.
    Snapshot(Box<PhoenixExchangeSnapshotMessage>),
    /// Sent first by default: the snapshot as base64 of zstd-compressed JSON.
    EncodedSnapshot(PhoenixExchangeEncodedSnapshot),
    Delta(PhoenixExchangeDelta),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhoenixExchangeSnapshotMessage {
    pub version: u16,
    #[serde(deserialize_with = "js_safe_u64")]
    pub sequence_number: u64,
    pub slot: u64,
    pub slot_index: u32,
    /// `snapshot`.
    pub reason: String,
    pub exchange: PhoenixExchangeState,
    pub markets: Vec<PhoenixMarketSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhoenixExchangeEncodedSnapshot {
    pub version: u16,
    #[serde(deserialize_with = "js_safe_u64")]
    pub sequence_number: u64,
    pub slot: u64,
    pub slot_index: u32,
    pub reason: String,
    /// `base64+zstd`.
    pub encoding: String,
    pub payload: String,
}

/// Apply `ops` in order; a skipped `sequence_number` requires a resubscribe.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhoenixExchangeDelta {
    pub version: u16,
    #[serde(deserialize_with = "js_safe_u64")]
    pub sequence_number: u64,
    pub slot: u64,
    pub slot_index: u32,
    pub ops: Vec<PhoenixExchangeDeltaOp>,
}

/// Delta op, discriminated by `kind`. Op fields are snake_case on the wire (per docs
/// and the reference SDK).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PhoenixExchangeDeltaOp {
    ExchangeKeysUpdated {
        exchange: PhoenixExchangeState,
    },
    ExchangeStatusChanged {
        previous_bits: u8,
        new_bits: u8,
        previous_features: Vec<String>,
        new_features: Vec<String>,
        enabled_features: Vec<String>,
        disabled_features: Vec<String>,
        /// Defaulted in the reference SDK (older servers omit it).
        previous_running_state: Option<PhoenixExchangeRunningState>,
        running_state: Option<PhoenixExchangeRunningState>,
        active: bool,
        gated: bool,
        withdrawals_available: Option<bool>,
    },
    /// Spot collateral asset configs; left untyped.
    SpotCollateralsUpdated {
        assets: Vec<serde_json::Value>,
    },
    MarketAdded {
        market: PhoenixMarketSnapshot,
    },
    MarketStatusChanged {
        symbol: String,
        previous_market_status: PhoenixMarketStatus,
        new_market_status: PhoenixMarketStatus,
    },
    MarketClosed {
        symbol: String,
        previous_market_status: PhoenixMarketStatus,
        #[serde(deserialize_with = "js_safe_u64")]
        finalized_mark_price: u64,
    },
    MarketTombstoned {
        symbol: String,
        previous_market_status: PhoenixMarketStatus,
        #[serde(deserialize_with = "js_safe_u64")]
        final_sequence_number: u64,
        #[serde(deserialize_with = "js_safe_u64")]
        final_trade_sequence_number: u64,
        #[serde(deserialize_with = "js_safe_u64")]
        final_order_sequence_number: u64,
    },
    MarketDeleted {
        symbol: String,
        asset_id: u32,
    },
    MarketParameterUpdated {
        symbol: String,
        update: PhoenixMarketParameterUpdate,
    },
    MarketMetadataUpdated {
        symbol: String,
        #[serde(deserialize_with = "nullable")]
        metadata: Option<PhoenixMarketMetadata>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PhoenixMarketParameterUpdate {
    CancelRiskFactorUpdated {
        previous: f64,
        new: f64,
        previous_bps: Option<u16>,
        new_bps: Option<u16>,
    },
    IsolatedOnlyUpdated {
        previous: bool,
        new: bool,
    },
    LeverageTiersUpdated {
        previous: Vec<PhoenixSnapshotLeverageTier>,
        new: Vec<PhoenixSnapshotLeverageTier>,
    },
    MarkPriceParametersUpdated {
        previous: PhoenixMarkPriceParameters,
        new: PhoenixMarkPriceParameters,
    },
    OpenInterestCapUpdated {
        #[serde(deserialize_with = "js_safe_u64")]
        previous_base_lots: u64,
        #[serde(deserialize_with = "js_safe_u64")]
        new_base_lots: u64,
    },
    MaxLiquidationSizeUpdated {
        #[serde(deserialize_with = "js_safe_u64")]
        previous_base_lots: u64,
        #[serde(deserialize_with = "js_safe_u64")]
        new_base_lots: u64,
    },
    UpnlRiskFactorUpdated {
        previous: f64,
        new: f64,
        previous_bps: Option<u16>,
        new_bps: Option<u16>,
    },
    UpnlRiskFactorForWithdrawalsUpdated {
        previous: f64,
        new: f64,
        previous_bps: Option<u16>,
        new_bps: Option<u16>,
    },
    FundingParametersUpdated {
        previous: PhoenixFundingConfig,
        new: PhoenixFundingConfig,
    },
    MarketFeesUpdated {
        previous: PhoenixFeeConfig,
        new: PhoenixFeeConfig,
    },
    CommodityMetadataUpdated {
        previous: PhoenixSnapshotCommodityMetadata,
        new: PhoenixSnapshotCommodityMetadata,
    },
    DisplayNameUpdated {
        previous: Option<String>,
        new: Option<String>,
    },
}

// ── REST: exchange / markets ──────────────────────────────────────────────────

/// `GET /v1/view/exchange`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhoenixExchange {
    pub keys: PhoenixExchangeKeys,
    pub markets: Vec<PhoenixMarketConfig>,
}

/// On-chain account keys for the exchange.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhoenixExchangeKeys {
    pub global_config: String,
    pub canonical_mint: String,
    pub global_vault: String,
    pub perp_asset_map: String,
    pub withdraw_queue: String,
    pub active_trader_buffer: Vec<String>,
    pub global_trader_index: Vec<String>,
    pub current_authorities: PhoenixAuthoritySet,
    pub pending_authorities: PhoenixAuthoritySet,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhoenixAuthoritySet {
    pub root_authority: String,
    pub risk_authority: String,
    pub market_authority: String,
    pub oracle_authority: String,
    pub adl_authority: String,
    pub cancel_authority: String,
    pub backstop_authority: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PhoenixMarketStatus {
    Uninitialized,
    Active,
    PostOnly,
    Paused,
    Closed,
    Tombstoned,
}

/// `GET /v1/view/exchange/markets` element and `GET /v1/view/exchange/market/{symbol}`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhoenixMarketConfig {
    pub symbol: String,
    pub asset_id: u32,
    pub market_status: PhoenixMarketStatus,
    pub market_pubkey: String,
    pub spline_pubkey: String,
    /// Quote lots per base lot per tick.
    pub tick_size: u64,
    pub base_lots_decimals: i8,
    pub taker_fee: f64,
    pub maker_fee: f64,
    pub leverage_tiers: Vec<PhoenixLeverageTier>,
    pub risk_factors: PhoenixRiskFactors,
    pub funding_interval_seconds: u32,
    pub funding_period_seconds: u32,
    pub max_funding_rate_per_interval: i64,
    /// Optional in the OpenAPI schema.
    pub max_funding_rate_per_interval_percentage: Option<f64>,
    #[serde(deserialize_with = "js_safe_u64")]
    pub open_interest_cap_base_lots: u64,
    #[serde(deserialize_with = "js_safe_u64")]
    pub max_liquidation_size_base_lots: u64,
    pub isolated_only: bool,
    /// Present only for commodity / real-world-asset markets.
    pub commodity_metadata: Option<PhoenixCommodityMetadata>,
    pub metadata: Option<PhoenixMarketMetadata>,
    pub stats_snapshot: Option<PhoenixMarketStatsSnapshot>,
}

impl PhoenixMarketConfig {
    /// Decimals of the quote (USDC) lot: one quote lot is `10^-6` USD.
    const QUOTE_LOT_DECIMALS: i32 = 6;

    /// Price increment in USD per base unit.
    pub fn price_tick(&self) -> f64 {
        self.tick_size as f64
            * 10f64.powi(i32::from(self.base_lots_decimals) - Self::QUOTE_LOT_DECIMALS)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhoenixLeverageTier {
    pub max_leverage: f64,
    #[serde(deserialize_with = "js_safe_u64")]
    pub max_size_base_lots: u64,
    pub limit_order_risk_factor: f64,
    pub limit_order_risk_factor_bps: Option<u32>,
}

/// Risk factors in percent; `*Bps` variants are optional in the schema.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhoenixRiskFactors {
    pub maintenance: f64,
    pub maintenance_bps: Option<u32>,
    pub backstop: f64,
    pub backstop_bps: Option<u32>,
    pub high_risk: f64,
    pub high_risk_bps: Option<u32>,
    pub upnl: f64,
    pub upnl_bps: Option<u32>,
    pub upnl_for_withdrawals: f64,
    pub upnl_for_withdrawals_bps: Option<u32>,
    pub cancel_order: f64,
    pub cancel_order_bps: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PhoenixCommodityMarketState {
    Active,
    AfterHours,
    Reopen,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhoenixCommodityMetadata {
    pub is_commodity: bool,
    pub is_reopen: bool,
    pub is_after_hours: bool,
    pub status: PhoenixCommodityMarketState,
    pub after_hours_radius: PhoenixDecimal,
    pub last_known_index_price: Option<PhoenixDecimal>,
    pub mark_price_band: Option<PhoenixPriceBand>,
    pub execution_price_band: Option<PhoenixPriceBand>,
    pub last_index_expiry_timestamp: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhoenixPriceBand {
    pub min: PhoenixDecimal,
    pub max: PhoenixDecimal,
}

/// Off-chain market metadata; every field is optional in the schema.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhoenixMarketMetadata {
    pub name: Option<String>,
    pub description: Option<String>,
    pub logo_uri: Option<String>,
    pub coin_gecko_id: Option<String>,
    pub coin_market_cap_id: Option<i64>,
    pub tokens_xyz_asset_id: Option<String>,
    pub display_color: Option<String>,
    pub calendar: Option<PhoenixMarketCalendar>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhoenixMarketCalendar {
    pub id: String,
    pub description: String,
    pub calendar_uri: String,
    pub content_sha256: String,
    pub next_market_transition_utc: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhoenixMarketStatsSnapshot {
    pub slot: u64,
    pub slot_index: u32,
    #[serde(deserialize_with = "js_safe_u64")]
    pub open_interest_base_lots: u64,
    #[serde(deserialize_with = "js_safe_u64")]
    pub funding_start_interval_timestamp: u64,
    #[serde(deserialize_with = "js_safe_i64")]
    pub cumulative_funding_rate: i64,
}

// ── REST: market data ─────────────────────────────────────────────────────────

/// `GET /v1/view/orderbook/{symbol}`. Levels are `[price, size]`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhoenixOrderBook {
    pub slot: u64,
    pub symbol: String,
    pub bids: Vec<[f64; 2]>,
    pub asks: Vec<[f64; 2]>,
    /// Null when the book has no midpoint.
    pub mid: Option<f64>,
    /// Spline (market-maker curve) liquidity. Present only when requested with
    /// `includeSplines=true`.
    #[serde(default)]
    pub splines: Option<Vec<PhoenixSpline>>,
}

/// One trader's spline in `GET /v1/view/orderbook/{symbol}?includeSplines=true`.
/// OpenAPI types the amounts as strings; live they are `{value, decimals, ui}` objects.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhoenixSpline {
    pub trader_key: String,
    pub mid_price: f64,
    pub bid_filled_amount: PhoenixDecimal,
    pub ask_filled_amount: PhoenixDecimal,
    pub bid_regions: Vec<PhoenixTickRegion>,
    pub ask_regions: Vec<PhoenixTickRegion>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhoenixTickRegion {
    pub start: f64,
    pub end: f64,
    pub density: f64,
    pub total_size: PhoenixDecimal,
    pub filled_size: PhoenixDecimal,
}

/// OHLCV candle. REST (`GET /v1/candles/{symbol}`) `time` is unix milliseconds;
/// WebSocket candle `time` is unix seconds. Optional fields are optional in the schema.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhoenixCandle {
    pub time: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub mark_open: Option<f64>,
    pub mark_high: Option<f64>,
    pub mark_low: Option<f64>,
    pub mark_close: Option<f64>,
    pub volume: Option<f64>,
    pub volume_quote: Option<f64>,
    pub trade_count: Option<u64>,
    pub external_source: Option<String>,
}

// ── REST: exchange status / snapshot ──────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PhoenixExchangeRunningState {
    Unknown,
    Active,
    Maintenance,
    Suspended,
}

/// `GET /v1/view/exchange/status`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhoenixExchangeStatus {
    pub active: bool,
    pub gated: bool,
    /// Optional in the schema.
    pub withdrawals_available: Option<bool>,
    /// Sent live; not in the OpenAPI schema.
    pub running_state: Option<PhoenixExchangeRunningState>,
}

/// `GET /v1/exchange/snapshot`: exchange state plus full market parameters
/// (the same payload as the `exchange` websocket snapshot).
/// The `spotCollaterals` list (spot collateral asset configs) is not parsed.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhoenixExchangeSnapshot {
    pub version: u16,
    /// Optional in the schema; absent from the REST response.
    #[serde(default, deserialize_with = "js_safe_u64_opt")]
    pub sequence_number: Option<u64>,
    pub slot: u64,
    pub slot_index: u32,
    pub exchange: PhoenixExchangeState,
    pub markets: Vec<PhoenixMarketSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhoenixExchangeState {
    pub program_id: String,
    pub global_config: String,
    pub current_authorities: PhoenixAuthoritySet,
    pub canonical_mint: String,
    pub usdc_mint: String,
    pub global_vault: String,
    pub perp_asset_map: String,
    pub global_trader_index: Vec<String>,
    pub active_trader_buffer: Vec<String>,
    pub withdraw_queue: String,
    #[serde(deserialize_with = "js_safe_u64")]
    pub pending_withdraw_throttle_replenishment: u64,
    pub exchange_status_bits: u8,
    /// e.g. `initialized`, `active`, `gated`.
    pub exchange_status_features: Vec<String>,
    pub running_state: PhoenixExchangeRunningState,
    pub active: bool,
    pub gated: bool,
    pub withdrawals_available: bool,
}

/// Market entry in the exchange snapshot / `exchange` websocket channel.
/// Differs from `PhoenixMarketConfig`: nested `fundingConfig`, `markPriceParameters`,
/// integer leverage tiers, and `lower`/`upper` price bands.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhoenixMarketSnapshot {
    pub symbol: String,
    /// Optional in the reference SDK; not seen live.
    pub display_name: Option<String>,
    pub asset_id: u32,
    pub market_status: PhoenixMarketStatus,
    pub market_pubkey: String,
    pub spline_pubkey: String,
    pub tick_size: u64,
    pub base_lots_decimals: i8,
    pub taker_fee: f64,
    pub maker_fee: f64,
    pub leverage_tiers: Vec<PhoenixSnapshotLeverageTier>,
    pub risk_factors: PhoenixRiskFactors,
    pub funding_config: PhoenixFundingConfig,
    #[serde(deserialize_with = "js_safe_u64")]
    pub open_interest_cap_base_lots: u64,
    #[serde(deserialize_with = "js_safe_u64")]
    pub max_liquidation_size_base_lots: u64,
    pub isolated_only: bool,
    pub mark_price_parameters: PhoenixMarkPriceParameters,
    /// Present only for commodity / real-world-asset markets.
    pub commodity_metadata: Option<PhoenixSnapshotCommodityMetadata>,
    pub metadata: Option<PhoenixMarketMetadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhoenixSnapshotLeverageTier {
    pub max_leverage: u64,
    #[serde(deserialize_with = "js_safe_u64")]
    pub max_size_base_lots: u64,
    /// Basis points.
    pub limit_order_risk_factor: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhoenixFundingConfig {
    pub funding_interval_seconds: u64,
    pub funding_period_seconds: u64,
    pub max_funding_rate_per_interval: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhoenixFeeConfig {
    pub taker_fee: f64,
    pub maker_fee: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PhoenixValidationRule {
    Ignore,
    Require,
    Forbid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhoenixMarkPriceParameters {
    #[serde(deserialize_with = "js_safe_u64")]
    pub ema_period_slots: u64,
    #[serde(deserialize_with = "js_safe_u64")]
    pub ema_diff_radius: u64,
    #[serde(deserialize_with = "js_safe_u64")]
    pub book_price_radius: u64,
    #[serde(deserialize_with = "js_safe_u64")]
    pub commodities_after_hours_radius: u64,
    #[serde(deserialize_with = "js_safe_u64")]
    pub commodities_after_hours_radius_bps: u64,
    #[serde(deserialize_with = "js_safe_u64")]
    pub adjusted_exchange_spot_price_weight: u64,
    #[serde(deserialize_with = "js_safe_u64")]
    pub book_price_weight: u64,
    #[serde(deserialize_with = "js_safe_u64")]
    pub exchange_perp_price_weight: u64,
    #[serde(deserialize_with = "js_safe_u64")]
    pub spot_price_stale_threshold: u64,
    #[serde(deserialize_with = "js_safe_u64")]
    pub book_price_stale_threshold: u64,
    pub book_hard_stale_multiplier: u8,
    #[serde(deserialize_with = "js_safe_u64")]
    pub perp_price_stale_threshold: u64,
    pub risk_action_price_validity_rules: Vec<Vec<Vec<PhoenixValidationRule>>>,
    pub oracle_divergence_radius: u16,
    pub oracle_hard_stale_multiplier: u8,
    pub min_oracle_responses: u8,
}

/// Commodity metadata in the snapshot. Same as `PhoenixCommodityMetadata` except the
/// price bands use `lower`/`upper` (REST market configs use `min`/`max`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhoenixSnapshotCommodityMetadata {
    pub is_commodity: bool,
    pub is_reopen: bool,
    pub is_after_hours: bool,
    pub status: PhoenixCommodityMarketState,
    pub after_hours_radius: PhoenixDecimal,
    /// Absent unless the market is after-hours / reopening (observed live).
    pub last_known_index_price: Option<PhoenixDecimal>,
    pub mark_price_band: Option<PhoenixSnapshotPriceBand>,
    pub execution_price_band: Option<PhoenixSnapshotPriceBand>,
    pub last_index_expiry_timestamp: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhoenixSnapshotPriceBand {
    pub lower: PhoenixDecimal,
    pub upper: PhoenixDecimal,
}

// ── REST: market prices, stats, funding, fills ────────────────────────────────

/// `GET /v1/market/{symbol}/mark-price`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhoenixMarkPrice {
    pub slot: u64,
    pub slot_index: u32,
    pub symbol: String,
    /// Nullable in the schema.
    #[serde(deserialize_with = "nullable")]
    pub mark_price: Option<PhoenixPrice>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhoenixPrice {
    pub price: f64,
    /// Slot the price was computed at.
    pub slot: u64,
}

/// `GET /v1/market/{symbol}/stats`. Field names are snake_case on the wire.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhoenixMarketStatsHistory {
    pub market_id: i64,
    pub symbol: String,
    /// Echo of the `timeframe` query; null when raw (unaggregated) points are returned.
    #[serde(deserialize_with = "nullable")]
    pub timeframe: Option<String>,
    pub stats: Vec<PhoenixMarketStatsPoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhoenixMarketStatsPoint {
    pub open_interest: f64,
    pub mark_price: f64,
    pub spot_price: f64,
    /// RFC 3339.
    pub timestamp: String,
    pub slot: u64,
    /// Cumulative; nullable in the schema.
    #[serde(deserialize_with = "nullable")]
    pub total_maker_fees: Option<f64>,
    /// Cumulative; nullable in the schema.
    #[serde(deserialize_with = "nullable")]
    pub total_taker_fees: Option<f64>,
}

/// `GET /v1/funding/{symbol}/rates`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhoenixFundingRateHistory {
    pub market_id: i64,
    pub symbol: String,
    pub rates: Vec<PhoenixFundingRatePoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhoenixFundingRatePoint {
    /// Unix seconds (OpenAPI says an ISO 8601 string; live sends an integer).
    pub timestamp: i64,
    /// Rate applied for the interval, in percent.
    pub funding_rate_percentage: String,
}

/// `GET /v1/funding/overview`: funding series for every market.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhoenixFundingOverview {
    pub series: Vec<PhoenixFundingOverviewSeries>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhoenixFundingOverviewSeries {
    pub market_id: i64,
    pub symbol: String,
    pub points: Vec<PhoenixFundingOverviewPoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhoenixFundingOverviewPoint {
    /// Unix seconds (OpenAPI says an ISO 8601 string; live sends an integer).
    pub timestamp: i64,
    /// Quote units per base unit.
    pub funding_amount_per_unit: String,
    pub mark_price: String,
    /// Decimal (not percent).
    pub funding_rate: String,
}

/// `GET /v1/trades/{symbol}/fills` item (wrapped in `PhoenixPaginatedResponse`).
/// Newest first.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhoenixFill {
    pub market_symbol: String,
    /// Signed (taker perspective).
    pub base_qty: String,
    pub quote_qty: String,
    pub price: String,
    /// RFC 3339.
    pub timestamp: String,
    pub transaction_signature: String,
    /// e.g. `PlaceMarketOrder`, `LiquidateViaMarketOrder`.
    pub instruction_type: String,
}

// ── REST: market calendars ────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PhoenixCalendarState {
    Open,
    AfterHours,
}

/// Parsed calendar: schedules keyed by weekday (`Mon`..`Sun`) and by date (`YYYY-MM-DD`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhoenixCalendarView {
    pub weekly_schedule: HashMap<String, PhoenixCalendarDay>,
    pub date_overrides: HashMap<String, PhoenixCalendarDay>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhoenixCalendarDay {
    pub sessions: Vec<PhoenixCalendarSession>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhoenixCalendarSession {
    /// `HH:MM:SS` UTC.
    pub start: String,
    pub end: String,
    /// e.g. `EXTERNAL`, `INTERNAL`, `CASH_SESSION`, `PRE_MARKET`, `POST_MARKET`, `OVERNIGHT`.
    pub mode: String,
}

/// `GET /v1/market/{symbol}/market-calendar`. 404 `market_calendar_not_configured`
/// for markets without a calendar (e.g. crypto).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhoenixMarketCalendarResponse {
    pub market: String,
    pub market_calendar_id: String,
    pub kind: String,
    pub description: String,
    pub calendar_uri: String,
    pub content_sha256: String,
    /// RFC 3339.
    pub loaded_at: String,
    /// The calendar source document as a JSON string.
    pub raw_json: String,
    #[serde(deserialize_with = "nullable")]
    pub calendar: Option<PhoenixCalendarView>,
}

/// `GET /v1/market-calendar/{market_calendar_id}`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhoenixMarketCalendarRecord {
    pub market_calendar_id: String,
    pub kind: String,
    pub description: String,
    pub s3_path: String,
    pub calendar_uri: String,
    pub content_sha256: String,
    /// RFC 3339.
    pub loaded_at: String,
    /// RFC 3339.
    pub updated_at: String,
    pub raw_json: String,
    #[serde(deserialize_with = "nullable")]
    pub calendar: Option<PhoenixCalendarView>,
}

/// `GET /v1/market/commodity-calendar`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhoenixCommodityCalendar {
    pub market: String,
    pub loaded_at: String,
    pub raw_json: String,
    pub calendar: PhoenixCalendarView,
}

/// `GET /v1/market/{symbol}/next-market-calendar-transition`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhoenixNextMarketCalendarTransition {
    pub market: String,
    pub market_calendar_id: String,
    pub calendar_uri: String,
    pub loaded_at: String,
    pub current_state: PhoenixCalendarState,
    /// Nullable in the schema.
    #[serde(deserialize_with = "nullable")]
    pub next_market_state: Option<PhoenixCalendarState>,
    /// RFC 3339; nullable in the schema.
    #[serde(deserialize_with = "nullable")]
    pub utc_next_transition: Option<String>,
}

/// `GET /v1/market/next-commodity-market-transition`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhoenixNextCommodityMarketTransition {
    pub market: String,
    pub loaded_at: String,
    pub current_state: PhoenixCalendarState,
    #[serde(deserialize_with = "nullable")]
    pub next_market_state: Option<PhoenixCalendarState>,
    #[serde(deserialize_with = "nullable")]
    pub utc_next_transition: Option<String>,
}

// ── REST: trader state ────────────────────────────────────────────────────────

/// `GET /v1/trader/state/{authority}`: snapshot of one trader PDA under an authority.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhoenixTraderState {
    pub authority: String,
    pub trader_pda_index: u8,
    pub slot: u64,
    pub slot_index: u32,
    pub snapshot: PhoenixTraderStateSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhoenixTraderStateSnapshot {
    pub version: u32,
    pub capabilities: PhoenixTraderCapabilities,
    pub maker_fee_override_multiplier: f64,
    pub taker_fee_override_multiplier: f64,
    pub subaccounts: Vec<PhoenixSubaccount>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PhoenixTraderActivityState {
    Uninitialized,
    Cold,
    Active,
    ReduceOnly,
    Frozen,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhoenixTraderCapabilities {
    pub flags: u32,
    pub state: PhoenixTraderActivityState,
    pub capabilities: PhoenixCapabilitySet,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhoenixCapabilitySet {
    pub place_limit_order: PhoenixCapabilityAccess,
    pub place_market_order: PhoenixCapabilityAccess,
    pub risk_increasing_trade: PhoenixCapabilityAccess,
    pub risk_reducing_trade: PhoenixCapabilityAccess,
    pub deposit_collateral: PhoenixCapabilityAccess,
    pub withdraw_collateral: PhoenixCapabilityAccess,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhoenixCapabilityAccess {
    pub immediate: bool,
    /// Optional in the schema.
    pub via_cold_activation: Option<bool>,
}

/// One subaccount (index 0 = cross margin, others isolated).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhoenixSubaccount {
    pub subaccount_index: u8,
    pub sequence: u64,
    /// Quote lots (USDC, 6 decimals).
    #[serde(deserialize_with = "js_safe_i64")]
    pub collateral: i64,
    /// Optional in the schema; the server omits it when there are no positions.
    #[serde(default)]
    pub positions: Vec<PhoenixPosition>,
    /// Optional in the schema; the server omits it when there are no open orders.
    #[serde(default)]
    pub orders: Vec<PhoenixMarketOrders>,
    /// Omitted when margin cannot be computed; see `margin_error`.
    pub margin: Option<PhoenixMargin>,
    pub margin_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhoenixPosition {
    pub symbol: String,
    #[serde(deserialize_with = "js_safe_u64")]
    pub position_sequence_number: u64,
    /// Signed: negative is short.
    #[serde(deserialize_with = "js_safe_i64")]
    pub base_position_lots: i64,
    #[serde(deserialize_with = "js_safe_u64")]
    pub entry_price_ticks: u64,
    pub entry_price_usd: String,
    #[serde(deserialize_with = "js_safe_i64")]
    pub virtual_quote_position_lots: i64,
    #[serde(deserialize_with = "js_safe_i64")]
    pub unsettled_funding_quote_lots: i64,
    #[serde(deserialize_with = "js_safe_i64")]
    pub accumulated_funding_quote_lots: i64,
}

/// Open limit orders for one market.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhoenixMarketOrders {
    pub symbol: String,
    pub orders: Vec<PhoenixLimitOrder>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhoenixLimitOrder {
    #[serde(deserialize_with = "js_safe_u64")]
    pub order_sequence_number: u64,
    pub side: PhoenixSide,
    pub order_type: String,
    #[serde(deserialize_with = "js_safe_u64")]
    pub price_ticks: u64,
    pub price_usd: String,
    #[serde(deserialize_with = "js_safe_u64")]
    pub size_remaining_lots: u64,
    #[serde(deserialize_with = "js_safe_u64")]
    pub initial_size_lots: u64,
    pub reduce_only: bool,
    pub status: String,
    /// Optional in the schema.
    pub is_stop_loss: Option<bool>,
    /// Optional in the schema.
    pub conditional_kind: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PhoenixRiskState {
    Healthy,
    Unhealthy,
    Underwater,
    ZeroCollateralNoPositions,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PhoenixRiskTier {
    Safe,
    AtRisk,
    Cancellable,
    Liquidatable,
    BackstopLiquidatable,
    HighRisk,
}

/// Server-computed margin for a subaccount.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhoenixMargin {
    pub effective_collateral: PhoenixDecimal,
    pub effective_collateral_for_withdrawals: PhoenixDecimal,
    pub withdrawable_quote_collateral: PhoenixDecimal,
    pub locked_deposit_remaining: PhoenixDecimal,
    pub unrealized_pnl: PhoenixDecimal,
    pub discounted_unrealized_pnl: PhoenixDecimal,
    pub unsettled_funding_owed: PhoenixDecimal,
    pub accumulated_funding: PhoenixDecimal,
    pub portfolio_value: PhoenixDecimal,
    pub maintenance_margin: PhoenixDecimal,
    pub cancel_margin: PhoenixDecimal,
    pub initial_margin: PhoenixDecimal,
    pub initial_margin_for_withdrawals: PhoenixDecimal,
    pub risk_state: PhoenixRiskState,
    pub risk_tier: PhoenixRiskTier,
    /// Keyed by market symbol.
    pub positions: HashMap<String, PhoenixPositionMargin>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhoenixPositionMargin {
    pub position_value: PhoenixDecimal,
    pub position_initial_margin: PhoenixDecimal,
    pub initial_margin: PhoenixDecimal,
    pub maintenance_margin: PhoenixDecimal,
    pub backstop_margin: PhoenixDecimal,
    pub limit_order_margin: PhoenixDecimal,
    pub liquidation_price: PhoenixDecimal,
}

// ── REST: trader history ──────────────────────────────────────────────────────

/// `GET /v1/users/{authority}/pnl` point. Times are unix seconds (observed live).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhoenixPnlPoint {
    pub timestamp: i64,
    pub start_time: i64,
    pub end_time: i64,
    pub cumulative_pnl: f64,
    pub unrealized_pnl: f64,
    pub cumulative_funding_payment: f64,
    pub cumulative_taker_fee: f64,
    pub cumulative_maker_fee: f64,
}

/// Cursor-paginated wrapper used by order-history, trades-history and collateral-history.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhoenixPaginatedResponse<T> {
    pub data: Vec<T>,
    /// Cursor for older items; absent on the last page.
    pub next_cursor: Option<String>,
    /// Cursor for newer items.
    pub prev_cursor: Option<String>,
    pub has_more: bool,
}

/// `GET /v1/trader/{authority}/order-history` item.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhoenixOrderHistory {
    pub order_sequence_number: String,
    pub market_symbol: String,
    /// OpenAPI enumerates `active|cancelled|filled`; the reference SDK also lists
    /// `open|expired`. Kept as the raw string.
    pub status: String,
    pub side: PhoenixSide,
    pub is_reduce_only: bool,
    pub is_stop_loss: Option<bool>,
    pub is_conditional_order: Option<bool>,
    pub price: String,
    pub base_qty: String,
    pub remaining_base_qty: String,
    pub filled_base_qty: String,
    /// RFC 3339.
    pub placed_at: Option<String>,
    /// RFC 3339.
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PhoenixLiquidityRole {
    Maker,
    Taker,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PhoenixTradeType {
    Limit,
    Market,
    Liquidation,
    Adl,
}

/// `GET /v1/trader/{authority}/trades-history` item.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhoenixTradeHistory {
    pub user_id: i64,
    pub trader_id: i64,
    pub trader_pda_index: i32,
    pub subaccount_index: i32,
    pub market_symbol: String,
    pub signature: Option<String>,
    pub fill_id: Option<String>,
    /// RFC 3339.
    pub timestamp: String,
    pub slot: u64,
    pub slot_index: u32,
    pub event_index: u32,
    pub instruction_index: u32,
    pub instruction_type: String,
    pub base_lots_before: String,
    pub base_lots_after: String,
    pub base_lots_delta: String,
    pub virtual_quote_lots_before: String,
    pub virtual_quote_lots_after: String,
    pub virtual_quote_lots_delta: String,
    pub price: String,
    pub realized_pnl: String,
    pub fees: String,
    pub liquidity: PhoenixLiquidityRole,
    pub order_sequence_number: Option<i64>,
    pub spline_sequence_number: Option<i64>,
    pub trade_type: PhoenixTradeType,
}

/// `GET /v1/trader/{authority}/funding-history`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhoenixFundingHistoryResponse {
    pub events: Vec<PhoenixFundingHistory>,
    pub next_cursor: Option<String>,
    pub prev_cursor: Option<String>,
    pub has_more: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhoenixFundingHistory {
    /// RFC 3339.
    pub timestamp: String,
    pub symbol: String,
    /// USDC; negative = paid.
    pub funding_payment: String,
    pub funding_rate_percentage: String,
    pub position_size: String,
    /// "Long" or "Short".
    pub position_side: String,
}

/// `GET /v1/trader/{authority}/collateral-history` item.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhoenixCollateralEvent {
    pub slot: u64,
    pub slot_index: u32,
    pub event_index: u32,
    pub trader_pda_index: i32,
    pub trader_subaccount_index: i32,
    pub event_type: String,
    /// Quote lots (USDC, 6 decimals), signed.
    pub amount: i64,
    /// Quote lots (USDC, 6 decimals).
    pub collateral_after: i64,
    /// RFC 3339.
    pub timestamp: String,
}
