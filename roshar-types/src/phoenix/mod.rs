use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ── WebSocket messages ────────────────────────────────────────────────────────

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
    #[serde(rename = "bypassExecutionBand", skip_serializing_if = "Option::is_none")]
    pub bypass_execution_band: Option<bool>,
}

impl PhoenixWssMessage {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("Failed to serialize PhoenixWssMessage")
    }

    pub fn ping() -> Self {
        Self {
            msg_type: "ping".to_string(),
            subscription: PhoenixSubscription {
                channel: String::new(),
                symbol: None,
                timeframe: None,
                authority: None,
                trader_pda_index: None,
                bypass_execution_band: None,
            },
        }
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

    /// Subscribe to L2 orderbook updates for a market.
    pub fn orderbook(symbol: &str) -> Self {
        let mut msg = Self::sub("orderbook");
        msg.subscription.symbol = Some(symbol.to_string());
        msg
    }

    pub fn orderbook_unsub(symbol: &str) -> Self {
        let mut msg = Self::unsub("orderbook");
        msg.subscription.symbol = Some(symbol.to_string());
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
    /// Valid timeframes: "1s","5s","1m","5m","15m","30m","1h","4h","1d".
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

    /// Subscribe to trader state updates (positions, orders, collateral).
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

// ── REST response types ───────────────────────────────────────────────────────

/// Exchange-wide configuration from `GET /exchange`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhoenixExchange {
    pub keys: PhoenixExchangeKeys,
    pub markets: Vec<PhoenixMarketConfig>,
}

/// On-chain key addresses for the exchange.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhoenixExchangeKeys {
    #[serde(default)]
    pub canonical_mint: String,
    #[serde(default)]
    pub global_vault: String,
    #[serde(default)]
    pub perp_asset_map: String,
}

/// Market configuration from `GET /exchange/markets` or `GET /exchange/market/{symbol}`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhoenixMarketConfig {
    pub symbol: String,
    pub asset_id: u32,
    #[serde(default)]
    pub market_status: String,
    #[serde(default)]
    pub market_pubkey: String,
    pub tick_size: u64,
    #[serde(default)]
    pub taker_fee: f64,
    #[serde(default)]
    pub maker_fee: f64,
    #[serde(default)]
    pub funding_interval_seconds: u32,
    #[serde(default)]
    pub funding_period_seconds: u32,
    #[serde(default)]
    pub max_funding_rate_per_interval: f64,
    pub leverage_tiers: Vec<PhoenixLeverageTier>,
    #[serde(default)]
    pub isolated_only: bool,
}

/// Single leverage tier with max leverage and notional cap.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhoenixLeverageTier {
    #[serde(default)]
    pub max_leverage: f64,
    #[serde(default)]
    pub notional_cap: f64,
}

/// Exchange snapshot from `GET /v1/exchange/snapshot`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhoenixExchangeSnapshot {
    pub markets: Vec<PhoenixMarketConfig>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// L2 orderbook from `GET /orderbook?symbol=...`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhoenixOrderBook {
    pub bids: Vec<[f64; 2]>,
    pub asks: Vec<[f64; 2]>,
    #[serde(default)]
    pub mid: Option<f64>,
}

/// OHLCV candle from `GET /candles`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhoenixCandle {
    pub time: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    #[serde(default)]
    pub volume: Option<f64>,
    #[serde(rename = "tradeCount", default)]
    pub trade_count: Option<u64>,
}

/// Trader state from `GET /trader/{authority}/state`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhoenixTraderState {
    #[serde(default)]
    pub authority: String,
    #[serde(default)]
    pub pda_index: u8,
    #[serde(default)]
    pub collateral_balance: f64,
    #[serde(default)]
    pub effective_collateral: f64,
    #[serde(default)]
    pub unrealized_pnl: f64,
    #[serde(default)]
    pub risk_state: String,
    #[serde(default)]
    pub risk_tier: String,
    #[serde(default)]
    pub positions: Vec<PhoenixPosition>,
    #[serde(default)]
    pub limit_orders: Vec<PhoenixLimitOrder>,
}

/// Open perp position.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhoenixPosition {
    pub symbol: String,
    #[serde(default)]
    pub position_size: f64,
    #[serde(default)]
    pub entry_price: f64,
    #[serde(default)]
    pub unrealized_pnl: f64,
    #[serde(default)]
    pub liquidation_price: f64,
    #[serde(default)]
    pub take_profit_price: Option<f64>,
    #[serde(default)]
    pub stop_loss_price: Option<f64>,
}

/// Open limit order on a trader account.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhoenixLimitOrder {
    #[serde(default)]
    pub symbol: String,
    #[serde(default)]
    pub side: String,
    #[serde(default)]
    pub price: f64,
    #[serde(default)]
    pub quantity: f64,
    #[serde(default)]
    pub remaining_quantity: f64,
}

/// PnL time-series point from `GET /trader/{authority}/pnl`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhoenixPnlPoint {
    pub timestamp: i64,
    pub start_time: i64,
    pub end_time: i64,
    #[serde(default)]
    pub cumulative_pnl: f64,
    #[serde(default)]
    pub unrealized_pnl: f64,
    #[serde(default)]
    pub cumulative_funding_payment: f64,
    #[serde(default)]
    pub cumulative_taker_fee: f64,
}

/// Historical trade from `GET /trader/{authority}/trades-history`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhoenixTradeHistory {
    pub market_symbol: String,
    #[serde(default)]
    pub signature: Option<String>,
    #[serde(default)]
    pub timestamp: String,
    #[serde(default)]
    pub price: f64,
    #[serde(default)]
    pub realized_pnl: String,
    #[serde(default)]
    pub fees: String,
    #[serde(default)]
    pub liquidity: String,
    #[serde(default)]
    pub trade_type: String,
}

/// Historical funding payment from `GET /trader/{authority}/funding-history`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhoenixFundingHistory {
    #[serde(default)]
    pub timestamp: String,
    pub symbol: String,
    #[serde(default)]
    pub funding_payment: String,
    #[serde(default)]
    pub funding_rate_percentage: String,
    #[serde(default)]
    pub position_size: String,
    #[serde(default)]
    pub position_side: String,
}

/// Historical order from `GET /trader/{authority}/order-history`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhoenixOrderHistory {
    #[serde(default)]
    pub order_sequence_number: String,
    pub market_symbol: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub side: String,
    #[serde(default)]
    pub is_reduce_only: bool,
    #[serde(default)]
    pub price: String,
    #[serde(default)]
    pub base_qty: String,
    #[serde(default)]
    pub filled_base_qty: String,
    #[serde(default)]
    pub placed_at: Option<String>,
    #[serde(default)]
    pub completed_at: Option<String>,
}

/// Collateral deposit/withdrawal event from `GET /trader/{authority}/collateral-history`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhoenixCollateralEvent {
    #[serde(default)]
    pub slot: i64,
    #[serde(default)]
    pub event_type: String,
    #[serde(default)]
    pub amount: i64,
    #[serde(default)]
    pub collateral_after: i64,
    #[serde(default)]
    pub timestamp: String,
}

/// Paginated wrapper used by order-history and trade-history responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhoenixPaginatedResponse<T> {
    pub items: Vec<T>,
    #[serde(default)]
    pub next_cursor: Option<String>,
}

/// All-mids WebSocket push: mid prices for every listed market.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhoenixAllMids {
    pub mids: HashMap<String, f64>,
    #[serde(default)]
    pub slot: u64,
    #[serde(rename = "slotIndex", default)]
    pub slot_index: u32,
}

/// Funding rate WebSocket push for a single market.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhoenixFundingRate {
    pub symbol: String,
    pub funding: f64,
}

/// Market stats WebSocket push (mark price, OI, volume, funding).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhoenixMarketStats {
    pub symbol: String,
    #[serde(default)]
    pub open_interest: f64,
    #[serde(default)]
    pub mark_price: f64,
    #[serde(default)]
    pub mid_price: f64,
    #[serde(default)]
    pub oracle_price: f64,
    #[serde(default)]
    pub prev_day_mark_price: f64,
    #[serde(default)]
    pub day_volume_usd: f64,
    #[serde(default)]
    pub funding_rate: f64,
}
