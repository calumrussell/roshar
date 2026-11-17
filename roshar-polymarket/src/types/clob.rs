//! CLOB API (`https://clob.polymarket.com`) public market-data wire types.
//!
//! Spec: https://docs.polymarket.com/api-spec/clob-openapi.yaml. Field optionality was checked
//! against full live responses on 2026-11-16 (10,000 `/markets` rows, 8,000
//! `/simplified-markets` rows, 100 `/clob-markets` rows). Where live data disagrees with the spec
//! (string vs number, field names) the live shape wins and is noted.

use crate::types::de::{f64_from_str, nullable, opt_f64_from_str_empty_none, u64_from_str};
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;

/// `next_cursor` value marking the last page of the cursor-paged market lists (base64 of `-1`).
/// Sending it back is an error (`next item should be greater than or equal to 0`).
pub const CLOB_END_CURSOR: &str = "LTE=";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub enum ClobSide {
    #[serde(rename = "BUY")]
    Buy,
    #[serde(rename = "SELL")]
    Sell,
}

impl ClobSide {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Buy => "BUY",
            Self::Sell => "SELL",
        }
    }
}

/// `side` of the last-trade-price routes: `""` (here `None`) when the token never traded.
fn opt_side_empty_none<'de, D: Deserializer<'de>>(d: D) -> Result<Option<ClobSide>, D::Error> {
    let s = String::deserialize(d)?;
    match s.as_str() {
        "" => Ok(None),
        "BUY" => Ok(Some(ClobSide::Buy)),
        "SELL" => Ok(Some(ClobSide::Sell)),
        other => Err(serde::de::Error::custom(format!("unknown side '{other}'"))),
    }
}

/// Error body of every CLOB route, e.g. `{"error": "No orderbook exists for the requested token
/// id"}`. Returned as the body of a [`crate::PolymarketApiError::Status`].
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ClobError {
    pub error: String,
}

/// A decimal sent as a JSON string (`"0.45"`), used as a map value.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
pub struct ClobDecimal(#[serde(deserialize_with = "f64_from_str")] pub f64);

/// One page of `/markets`, `/simplified-markets`, `/sampling-markets` or
/// `/sampling-simplified-markets`. Pass `next_cursor` back until it equals [`CLOB_END_CURSOR`].
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ClobMarketsPage<T> {
    pub data: Vec<T>,
    pub next_cursor: String,
    pub limit: u32,
    pub count: u32,
}

/// Full CLOB market (`Market` schema plus the live-only `version`).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ClobMarket {
    pub enable_order_book: bool,
    pub active: bool,
    pub closed: bool,
    pub archived: bool,
    pub accepting_orders: bool,
    /// RFC 3339; `null` for markets that never accepted orders.
    #[serde(deserialize_with = "nullable")]
    pub accepting_order_timestamp: Option<String>,
    pub minimum_order_size: f64,
    pub minimum_tick_size: f64,
    /// `""` for some legacy markets.
    pub condition_id: String,
    pub question_id: String,
    pub question: String,
    pub description: String,
    pub market_slug: String,
    #[serde(deserialize_with = "nullable")]
    pub end_date_iso: Option<String>,
    #[serde(deserialize_with = "nullable")]
    pub game_start_time: Option<String>,
    pub seconds_delay: u64,
    /// FPMM (AMM) contract address; `""` for CLOB-only markets.
    pub fpmm: String,
    /// Basis points.
    pub maker_base_fee: i64,
    pub taker_base_fee: i64,
    pub notifications_enabled: bool,
    pub neg_risk: bool,
    /// `""` outside neg-risk markets.
    pub neg_risk_market_id: String,
    pub neg_risk_request_id: String,
    pub icon: String,
    pub image: String,
    pub rewards: ClobRewards,
    pub is_50_50_outcome: bool,
    pub tokens: Vec<ClobToken>,
    #[serde(deserialize_with = "nullable")]
    pub tags: Option<Vec<String>>,
    /// Not in the spec; `"v1"` on every live row.
    pub version: String,
}

/// Simplified market (`SimplifiedMarket` schema plus `version`).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ClobSimplifiedMarket {
    pub condition_id: String,
    pub rewards: ClobRewards,
    pub tokens: Vec<ClobToken>,
    pub active: bool,
    pub closed: bool,
    pub archived: bool,
    pub accepting_orders: bool,
    pub version: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ClobRewards {
    /// `null` when the market has no reward program.
    #[serde(deserialize_with = "nullable")]
    pub rates: Option<Vec<ClobRewardRate>>,
    pub min_size: f64,
    pub max_spread: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ClobRewardRate {
    pub asset_address: String,
    pub rewards_daily_rate: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ClobToken {
    /// `""` on some legacy markets.
    pub token_id: String,
    pub outcome: String,
    pub price: f64,
    pub winner: bool,
}

/// `GET /clob-markets/{condition_id}` (`ClobMarketDetails`): compact CLOB parameters. Every key
/// except `c`, `mts`, `r`, `t` and `v` was absent on some live rows. Keys marked "not in the
/// spec" are named by their observed meaning.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ClobMarketInfo {
    #[serde(rename = "c")]
    pub condition_id: String,
    #[serde(rename = "mts")]
    pub min_tick_size: f64,
    #[serde(rename = "r")]
    pub rewards: ClobMarketInfoRewards,
    #[serde(rename = "t")]
    pub tokens: Vec<ClobMarketInfoToken>,
    /// Not in the spec; `"v1"` live.
    #[serde(rename = "v")]
    pub version: String,
    #[serde(rename = "mos", default)]
    pub min_order_size: Option<f64>,
    /// Game start time (sports), RFC 3339.
    #[serde(rename = "gst", default)]
    pub game_start_time: Option<String>,
    /// Not in the spec; matches `/markets` `seconds_delay`.
    #[serde(rename = "sd", default)]
    pub seconds_delay: Option<u64>,
    /// Basis points.
    #[serde(rename = "mbf", default)]
    pub maker_base_fee: Option<i64>,
    #[serde(rename = "tbf", default)]
    pub taker_base_fee: Option<i64>,
    #[serde(rename = "rfqe", default)]
    pub rfq_enabled: Option<bool>,
    /// Marketable orders are held for the taker-delay window.
    #[serde(rename = "itode", default)]
    pub taker_order_delay_enabled: Option<bool>,
    #[serde(rename = "ibce", default)]
    pub blockaid_check_enabled: Option<bool>,
    #[serde(rename = "fd", default)]
    pub fee_details: Option<ClobFeeDetails>,
    /// Minimum order age in seconds.
    #[serde(rename = "oas", default)]
    pub min_order_age_seconds: Option<u64>,
    /// Not in the spec; matches `/markets` `accepting_order_timestamp`.
    #[serde(rename = "aot", default)]
    pub accepting_order_timestamp: Option<String>,
    /// Not in the spec; sent `true` on neg-risk markets.
    #[serde(rename = "nr", default)]
    pub neg_risk: Option<bool>,
    /// Not in the spec; meaning undocumented.
    #[serde(default)]
    pub cbos: Option<bool>,
    /// Not in the spec; meaning undocumented.
    #[serde(default)]
    pub ao: Option<bool>,
}

/// `r` of [`ClobMarketInfo`] (`ClobRewards`, which the spec leaves unspecified).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ClobMarketInfoRewards {
    /// Not documented; always sent live.
    pub moas: f64,
    /// Not documented; matches `/markets` `rewards.min_size` when sent.
    #[serde(default)]
    pub mi: Option<f64>,
    /// Not documented; matches `/markets` `rewards.max_spread` when sent.
    #[serde(default)]
    pub ma: Option<f64>,
    /// Not documented.
    #[serde(default)]
    pub e: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ClobMarketInfoToken {
    #[serde(rename = "t")]
    pub token_id: String,
    #[serde(rename = "o")]
    pub outcome: String,
}

/// `fd` of [`ClobMarketInfo`] (`FeeDetails`); the spec marks each key nullable.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ClobFeeDetails {
    /// Fee rate.
    #[serde(rename = "r", default)]
    pub rate: Option<f64>,
    /// Fee curve exponent.
    #[serde(rename = "e", default)]
    pub exponent: Option<f64>,
    /// Fees apply to takers only.
    #[serde(rename = "to", default)]
    pub taker_only: Option<bool>,
}

/// `GET /markets-by-token/{token_id}`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ClobMarketByToken {
    pub condition_id: String,
    pub primary_token_id: String,
    pub secondary_token_id: String,
}

/// `/markets/live-activity` row (`LiveActivityMarket`).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ClobLiveActivityMarket {
    pub condition_id: String,
    /// Gamma market id.
    pub id: u64,
    pub question: String,
    pub market_slug: String,
    pub event_slug: String,
    /// `""` when the event has no series.
    pub series_slug: String,
    pub icon: String,
    pub image: String,
    pub tags: Vec<String>,
}

/// One item of the POST batch routes: `{"token_id": ..., "side"?: ...}`. `side` is required by
/// `POST /prices` and ignored by the others.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ClobBookRequest {
    pub token_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub side: Option<ClobSide>,
}

impl ClobBookRequest {
    pub fn new(token_id: &str) -> Self {
        Self {
            token_id: token_id.to_string(),
            side: None,
        }
    }

    pub fn with_side(token_id: &str, side: ClobSide) -> Self {
        Self {
            token_id: token_id.to_string(),
            side: Some(side),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct ClobLevel {
    #[serde(deserialize_with = "f64_from_str")]
    pub price: f64,
    #[serde(deserialize_with = "f64_from_str")]
    pub size: f64,
}

/// Order book summary (`OrderBookSummary`).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ClobBook {
    /// Condition id.
    pub market: String,
    pub asset_id: String,
    /// Milliseconds.
    #[serde(deserialize_with = "u64_from_str")]
    pub timestamp: u64,
    pub hash: String,
    /// Live order: ascending price, best bid last (the spec says descending).
    pub bids: Vec<ClobLevel>,
    /// Live order: descending price, best ask last (the spec says ascending).
    pub asks: Vec<ClobLevel>,
    #[serde(deserialize_with = "f64_from_str")]
    pub min_order_size: f64,
    #[serde(deserialize_with = "f64_from_str")]
    pub tick_size: f64,
    pub neg_risk: bool,
    /// `""` (here `None`) when the token never traded.
    #[serde(deserialize_with = "opt_f64_from_str_empty_none")]
    pub last_trade_price: Option<f64>,
}

/// `GET /price`. Live sends a string (`{"price": "0.45"}`); the spec says number. Both parse.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
pub struct ClobPrice {
    #[serde(deserialize_with = "f64_from_str")]
    pub price: f64,
}

/// `GET /midpoint`. Live key is `mid` (the spec says `mid_price`).
#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
pub struct ClobMidpoint {
    #[serde(deserialize_with = "f64_from_str")]
    pub mid: f64,
}

/// `GET /spread`.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
pub struct ClobSpread {
    #[serde(deserialize_with = "f64_from_str")]
    pub spread: f64,
}

/// `GET /last-trade-price`. A token that never traded returns `{"price": "0.5", "side": ""}`.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
pub struct ClobLastTradePrice {
    #[serde(deserialize_with = "f64_from_str")]
    pub price: f64,
    #[serde(deserialize_with = "opt_side_empty_none")]
    pub side: Option<ClobSide>,
}

/// `POST /last-trades-prices` item. Tokens without a book are left out.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct ClobLastTradePriceItem {
    pub token_id: String,
    #[serde(deserialize_with = "f64_from_str")]
    pub price: f64,
    #[serde(deserialize_with = "opt_side_empty_none")]
    pub side: Option<ClobSide>,
}

/// `POST /midpoints` and `POST /spreads`: token id -> value. Tokens without a book are left out.
pub type ClobTokenValues = HashMap<String, ClobDecimal>;

/// `POST /prices`: token id -> side -> best price. Live values are strings (the spec says
/// numbers).
pub type ClobTokenPrices = HashMap<String, HashMap<ClobSide, ClobDecimal>>;

/// `GET /tick-size`.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
pub struct ClobTickSize {
    pub minimum_tick_size: f64,
}

/// `GET /neg-risk`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub struct ClobNegRisk {
    pub neg_risk: bool,
}

/// `GET /fee-rate`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub struct ClobFeeRate {
    /// Basis points.
    pub base_fee: i64,
}

/// Query of `GET /prices-history`. The server requires either `start_ts`/`end_ts` or `interval`
/// (`invalid filters: the time component is mandatory`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ClobPricesHistoryParams {
    /// Epoch seconds (`startTs`).
    pub start_ts: Option<i64>,
    /// Epoch seconds (`endTs`).
    pub end_ts: Option<i64>,
    /// `max`, `all`, `1m`, `1w`, `1d`, `6h`, `1h`, sent verbatim.
    pub interval: Option<String>,
    /// Bucket width in minutes (exchange default 1).
    pub fidelity: Option<u32>,
}

/// Body of `POST /batch-prices-history` (`BatchPricesHistoryRequest`).
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct ClobBatchPricesHistoryRequest {
    /// Token ids, at most 20.
    pub markets: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_ts: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_ts: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interval: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fidelity: Option<u32>,
}

/// `GET /prices-history`. Unknown tokens return an empty history.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ClobPricesHistory {
    pub history: Vec<ClobPricePoint>,
}

/// `POST /batch-prices-history`: token id -> points.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ClobBatchPricesHistory {
    pub history: HashMap<String, Vec<ClobPricePoint>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
pub struct ClobPricePoint {
    /// Epoch seconds.
    pub t: i64,
    pub p: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn batch_request_body() {
        let body = vec![
            ClobBookRequest::new("1"),
            ClobBookRequest::with_side("2", ClobSide::Sell),
        ];
        assert_eq!(
            serde_json::to_string(&body).unwrap(),
            r#"[{"token_id":"1"},{"token_id":"2","side":"SELL"}]"#
        );
    }

    #[test]
    fn last_trade_side_empty_is_none() {
        let p: ClobLastTradePrice = serde_json::from_str(r#"{"price":"0.5","side":""}"#).unwrap();
        assert_eq!(p.side, None);
        assert!(
            serde_json::from_str::<ClobLastTradePrice>(r#"{"price":"0.5","side":"X"}"#).is_err()
        );
        assert!(serde_json::from_str::<ClobLastTradePrice>(r#"{"price":"0.5"}"#).is_err());
    }
}
