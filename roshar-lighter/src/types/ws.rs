//! Public websocket frames (https://docs.lighter.xyz/api/websocket). Every data frame has
//! `type: "subscribed/<channel>"` (the snapshot sent on subscribe) or `"update/<channel>"`;
//! `channel` uses colons (`order_book:0`, `candle:0:1m`, `market_stats:all`). Shapes were checked
//! against `wss://mainnet.zklighter.elliot.ai/stream?readonly=true` on 2026-11-16.

use std::collections::BTreeMap;

use anyhow::{anyhow, bail, Context, Result};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::Value;

use super::{
    LighterBoMarketStats, LighterCandle, LighterMarkPriceCandle, LighterTrade, LighterUpDownCandle,
};

/// A data frame. `snapshot` is true for `subscribed/…` and false for `update/…`.
#[derive(Debug, Clone)]
pub struct LighterWsEvent<T> {
    pub snapshot: bool,
    pub data: T,
}

/// `{"type":"connected","session_id":…}`, the first frame on a socket.
#[derive(Debug, Clone, Deserialize)]
pub struct LighterWsConnected {
    pub session_id: String,
}

/// `{"type":"unsubscribed","channel":…}`. `channel` echoes the request (slash form).
#[derive(Debug, Clone, Deserialize)]
pub struct LighterWsUnsubscribed {
    pub channel: String,
}

/// `{"error":{"code":…,"message":…}}`. The frame names neither the channel nor the request
/// unless the request carried an `id`.
#[derive(Debug, Clone)]
pub struct LighterWsError {
    pub code: i64,
    pub message: String,
    pub id: Option<String>,
}

#[derive(Deserialize)]
struct ErrorFrame {
    error: ErrorBody,
    id: Option<String>,
}

#[derive(Deserialize)]
struct ErrorBody {
    code: i64,
    message: String,
}

/// One aggregated level. In updates, size `"0.0000"` removes the level.
#[derive(Debug, Clone, Deserialize)]
pub struct LighterPriceLevel {
    pub price: String,
    pub size: String,
}

/// `order_book:{m}`. The snapshot holds the full book; updates hold changed levels only.
/// Continuity: an update's `begin_nonce` equals the previous frame's `nonce`.
#[derive(Debug, Clone, Deserialize)]
pub struct LighterWsOrderBook {
    pub channel: String,
    /// Unix microseconds.
    pub last_updated_at: i64,
    pub offset: i64,
    pub order_book: LighterWsOrderBookData,
    /// Unix ms.
    pub timestamp: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LighterWsOrderBookData {
    pub code: i64,
    pub asks: Vec<LighterPriceLevel>,
    pub bids: Vec<LighterPriceLevel>,
    pub offset: i64,
    pub nonce: i64,
    /// Unix microseconds.
    pub last_updated_at: i64,
    pub begin_nonce: i64,
}

/// `ticker:{m}`: best bid and offer.
#[derive(Debug, Clone, Deserialize)]
pub struct LighterWsTicker {
    pub channel: String,
    /// Unix microseconds. Absent on the subscription snapshot of a market whose book has never
    /// updated (an empty book, e.g. a closed stock market); 0 then.
    #[serde(default)]
    pub last_updated_at: i64,
    pub nonce: i64,
    pub ticker: LighterWsTickerData,
    /// Unix ms.
    pub timestamp: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LighterWsTickerData {
    /// Symbol.
    pub s: String,
    /// Best ask.
    pub a: LighterPriceLevel,
    /// Best bid.
    pub b: LighterPriceLevel,
    /// Unix microseconds.
    pub last_updated_at: i64,
}

/// `trade:{m}`. The snapshot carries recent trades; updates carry new ones.
#[derive(Debug, Clone, Deserialize)]
pub struct LighterWsTrades {
    pub channel: String,
    pub nonce: i64,
    pub trades: Vec<LighterTrade>,
    pub liquidation_trades: Vec<LighterTrade>,
}

/// Perp market stats (`market_stats` channel).
#[derive(Debug, Clone, Deserialize)]
pub struct LighterMarketStats {
    pub symbol: String,
    pub market_id: u32,
    pub index_price: String,
    pub mark_price: String,
    pub mid_price: String,
    pub best_ask_price: String,
    pub best_bid_price: String,
    pub open_interest: String,
    pub open_interest_limit: String,
    /// Not in the documented structure; sent for every market.
    pub open_interest_usd: String,
    pub funding_clamp_small: String,
    pub funding_clamp_big: String,
    pub last_trade_price: String,
    /// Estimate of the next funding payment.
    pub current_funding_rate: String,
    /// Last funding payment, made at `funding_timestamp`.
    pub funding_rate: String,
    /// Unix ms.
    pub funding_timestamp: i64,
    pub daily_base_token_volume: f64,
    pub daily_quote_token_volume: f64,
    pub daily_price_low: f64,
    pub daily_price_high: f64,
    /// Percent.
    pub daily_price_change: f64,
    pub base_interest_rate: String,
    /// Percent.
    pub premium: String,
}

/// A `partial: true` update entry: `market_id` plus the fields that changed.
#[derive(Debug, Clone, Deserialize)]
pub struct LighterMarketStatsPatch {
    pub market_id: u32,
    pub symbol: Option<String>,
    pub index_price: Option<String>,
    pub mark_price: Option<String>,
    pub mid_price: Option<String>,
    pub best_ask_price: Option<String>,
    pub best_bid_price: Option<String>,
    pub open_interest: Option<String>,
    pub open_interest_limit: Option<String>,
    pub open_interest_usd: Option<String>,
    pub funding_clamp_small: Option<String>,
    pub funding_clamp_big: Option<String>,
    pub last_trade_price: Option<String>,
    pub current_funding_rate: Option<String>,
    pub funding_rate: Option<String>,
    pub funding_timestamp: Option<i64>,
    pub daily_base_token_volume: Option<f64>,
    pub daily_quote_token_volume: Option<f64>,
    pub daily_price_low: Option<f64>,
    pub daily_price_high: Option<f64>,
    pub daily_price_change: Option<f64>,
    pub base_interest_rate: Option<String>,
    pub premium: Option<String>,
}

/// Spot market stats (`spot_market_stats` channel).
#[derive(Debug, Clone, Deserialize)]
pub struct LighterSpotMarketStats {
    pub symbol: String,
    pub market_id: u32,
    pub index_price: String,
    pub mid_price: String,
    /// Not in the documented structure; sent for every market.
    pub best_ask_price: String,
    /// Not in the documented structure; sent for every market.
    pub best_bid_price: String,
    pub last_trade_price: String,
    pub daily_base_token_volume: f64,
    pub daily_quote_token_volume: f64,
    pub daily_price_low: f64,
    pub daily_price_high: f64,
    pub daily_price_change: f64,
}

/// A `partial: true` spot update entry. May carry `market_id` only.
#[derive(Debug, Clone, Deserialize)]
pub struct LighterSpotMarketStatsPatch {
    pub market_id: u32,
    pub symbol: Option<String>,
    pub index_price: Option<String>,
    pub mid_price: Option<String>,
    pub best_ask_price: Option<String>,
    pub best_bid_price: Option<String>,
    pub last_trade_price: Option<String>,
    pub daily_base_token_volume: Option<f64>,
    pub daily_quote_token_volume: Option<f64>,
    pub daily_price_low: Option<f64>,
    pub daily_price_high: Option<f64>,
    pub daily_price_change: Option<f64>,
}

/// A stats frame. `stats` is the `market_stats`, `spot_market_stats` or `bo_market_stats` key:
/// one object for `…/{m}`, an object keyed by market id for `…/all` and the `market_ids` list.
#[derive(Debug, Clone, Deserialize)]
pub struct LighterWsStats<T> {
    pub channel: String,
    #[serde(
        rename = "market_stats",
        alias = "spot_market_stats",
        alias = "bo_market_stats"
    )]
    pub stats: T,
    /// Unix ms.
    pub timestamp: i64,
}

/// `candle`, `mark_price_candle` and `up_down_candle` frames. Up to 2 candles: on rollover
/// `candles[0]` is the one that just closed. The `up_down_candle` snapshot holds up to 300.
#[derive(Debug, Clone, Deserialize)]
pub struct LighterWsCandles<T> {
    pub channel: String,
    pub candles: Vec<T>,
    /// Unix ms.
    pub timestamp: i64,
}

/// `height`: L2 block height.
#[derive(Debug, Clone, Deserialize)]
pub struct LighterWsHeight {
    pub channel: String,
    pub height: i64,
    /// Unix ms.
    pub timestamp: i64,
}

/// A parsed public websocket frame.
#[derive(Debug, Clone)]
pub enum LighterWsMessage {
    Connected(LighterWsConnected),
    Pong,
    Unsubscribed(LighterWsUnsubscribed),
    Error(LighterWsError),
    OrderBook(LighterWsEvent<LighterWsOrderBook>),
    Ticker(LighterWsEvent<LighterWsTicker>),
    Trade(LighterWsEvent<LighterWsTrades>),
    /// `market_stats/{m}`.
    MarketStats(LighterWsEvent<LighterWsStats<LighterMarketStats>>),
    /// `market_stats/all` or the `market_ids` list, full entries.
    MarketStatsMap(LighterWsEvent<LighterWsStats<BTreeMap<u32, LighterMarketStats>>>),
    /// `update/market_stats` on a `partial` subscription (only from `parse_partial`).
    MarketStatsPatch(LighterWsStats<BTreeMap<u32, LighterMarketStatsPatch>>),
    SpotMarketStats(LighterWsEvent<LighterWsStats<LighterSpotMarketStats>>),
    SpotMarketStatsMap(LighterWsEvent<LighterWsStats<BTreeMap<u32, LighterSpotMarketStats>>>),
    SpotMarketStatsPatch(LighterWsStats<BTreeMap<u32, LighterSpotMarketStatsPatch>>),
    BoMarketStats(LighterWsEvent<LighterWsStats<LighterBoMarketStats>>),
    BoMarketStatsMap(LighterWsEvent<LighterWsStats<BTreeMap<u32, LighterBoMarketStats>>>),
    Candle(LighterWsEvent<LighterWsCandles<LighterCandle>>),
    MarkPriceCandle(LighterWsEvent<LighterWsCandles<LighterMarkPriceCandle>>),
    UpDownCandle(LighterWsEvent<LighterWsCandles<LighterUpDownCandle>>),
    Height(LighterWsEvent<LighterWsHeight>),
}

fn event<T: DeserializeOwned>(snapshot: bool, v: Value) -> Result<LighterWsEvent<T>> {
    Ok(LighterWsEvent {
        snapshot,
        data: serde_json::from_value(v)?,
    })
}

impl LighterWsMessage {
    /// Parses a frame of a subscription without `partial`. Unknown types are an error.
    pub fn parse(text: &str) -> Result<Self> {
        Self::parse_with(text, false)
    }

    /// Parses a frame on a socket whose `market_stats` / `spot_market_stats` `/all` or list
    /// subscriptions use `partial: true`: their updates become `*Patch`. The wire does not mark
    /// partial frames, so the caller decides by what it subscribed to.
    pub fn parse_partial(text: &str) -> Result<Self> {
        Self::parse_with(text, true)
    }

    fn parse_with(text: &str, partial: bool) -> Result<Self> {
        Self::decode(text, partial).with_context(|| format!("Lighter ws frame: {text}"))
    }

    fn decode(text: &str, partial: bool) -> Result<Self> {
        let v: Value = serde_json::from_str(text)?;
        if v.get("error").is_some() {
            let e: ErrorFrame = serde_json::from_value(v)?;
            return Ok(Self::Error(LighterWsError {
                code: e.error.code,
                message: e.error.message,
                id: e.id,
            }));
        }
        let ty = v
            .get("type")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("no type"))?
            .to_owned();
        match ty.as_str() {
            "connected" => return Ok(Self::Connected(serde_json::from_value(v)?)),
            "pong" => return Ok(Self::Pong),
            "unsubscribed" => return Ok(Self::Unsubscribed(serde_json::from_value(v)?)),
            _ => {}
        }
        let (snapshot, kind) = if let Some(k) = ty.strip_prefix("subscribed/") {
            (true, k)
        } else if let Some(k) = ty.strip_prefix("update/") {
            (false, k)
        } else {
            bail!("unknown type {ty}");
        };
        // `…:all` and the bare list channel carry an object keyed by market id.
        let many = match v.get("channel").and_then(Value::as_str) {
            Some(c) => c.ends_with(":all") || !c.contains(':'),
            None => bail!("no channel"),
        };
        let patch = partial && !snapshot && many;
        Ok(match kind {
            "order_book" => Self::OrderBook(event(snapshot, v)?),
            "ticker" => Self::Ticker(event(snapshot, v)?),
            "trade" => Self::Trade(event(snapshot, v)?),
            "market_stats" if patch => Self::MarketStatsPatch(serde_json::from_value(v)?),
            "market_stats" if many => Self::MarketStatsMap(event(snapshot, v)?),
            "market_stats" => Self::MarketStats(event(snapshot, v)?),
            "spot_market_stats" if patch => Self::SpotMarketStatsPatch(serde_json::from_value(v)?),
            "spot_market_stats" if many => Self::SpotMarketStatsMap(event(snapshot, v)?),
            "spot_market_stats" => Self::SpotMarketStats(event(snapshot, v)?),
            "bo_market_stats" if many => Self::BoMarketStatsMap(event(snapshot, v)?),
            "bo_market_stats" => Self::BoMarketStats(event(snapshot, v)?),
            "candle" => Self::Candle(event(snapshot, v)?),
            "mark_price_candle" => Self::MarkPriceCandle(event(snapshot, v)?),
            "up_down_candle" => Self::UpDownCandle(event(snapshot, v)?),
            "height" => Self::Height(event(snapshot, v)?),
            _ => bail!("unknown channel type {ty}"),
        })
    }
}
