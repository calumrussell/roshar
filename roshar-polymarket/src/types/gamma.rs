//! Gamma API (`https://gamma-api.polymarket.com`) wire types.
//!
//! Field optionality follows the Gamma OpenAPI spec
//! (https://docs.polymarket.com/api-reference/events/list-events-keyset-pagination), which marks
//! almost every field nullable; live responses omit such fields instead of sending `null`. Fields
//! the spec marks non-nullable and live responses always carry are required.

use crate::rest::Query;
use serde::{de, Deserialize, Deserializer, Serialize};

/// Gamma sends some market decimals as JSON strings (e.g. `"liquidity": "1234.5"`).
fn opt_f64_from_str<'de, D>(deserializer: D) -> Result<Option<f64>, D::Error>
where
    D: Deserializer<'de>,
{
    match Option::<String>::deserialize(deserializer)? {
        None => Ok(None),
        Some(s) => s
            .parse::<f64>()
            .map(Some)
            .map_err(|_| de::Error::custom(format!("failed to parse '{s}' as f64"))),
    }
}

/// Gamma sends some string lists as JSON text inside a string
/// (e.g. `"outcomes": "[\"Yes\", \"No\"]"`).
fn opt_json_string_list<'de, D>(deserializer: D) -> Result<Option<Vec<String>>, D::Error>
where
    D: Deserializer<'de>,
{
    match Option::<String>::deserialize(deserializer)? {
        None => Ok(None),
        Some(s) => serde_json::from_str(&s)
            .map(Some)
            .map_err(de::Error::custom),
    }
}

/// Query parameters for `GET /events/keyset`. `None` fields are not sent, so the exchange default
/// applies (e.g. `limit` defaults to 20, max 100). Array fields are sent as repeated keys
/// (`id=1&id=2`), as Gamma requires.
#[derive(Debug, Clone, Default)]
pub struct ListEventsParams {
    /// Page size, 1..=100.
    pub limit: Option<u32>,
    /// Comma-separated JSON field names to order by.
    pub order: Option<String>,
    /// Sort direction; only used when `order` is set.
    pub ascending: Option<bool>,
    /// `next_cursor` from the previous page.
    pub after_cursor: Option<String>,
    pub id: Option<Vec<u64>>,
    pub slug: Option<Vec<String>>,
    /// `true` closed only, `false` open only, `None` both.
    pub closed: Option<bool>,
    pub live: Option<bool>,
    pub featured: Option<bool>,
    /// "Create Your Own Market" events.
    pub cyom: Option<bool>,
    pub title_search: Option<String>,
    pub liquidity_min: Option<f64>,
    pub liquidity_max: Option<f64>,
    pub volume_min: Option<f64>,
    pub volume_max: Option<f64>,
    /// ISO 8601 date-time.
    pub start_date_min: Option<String>,
    pub start_date_max: Option<String>,
    pub end_date_min: Option<String>,
    pub end_date_max: Option<String>,
    pub start_time_min: Option<String>,
    pub start_time_max: Option<String>,
    pub tag_id: Option<Vec<u64>>,
    pub tag_slug: Option<String>,
    /// Must not overlap `tag_id`.
    pub exclude_tag_id: Option<Vec<u64>>,
    pub related_tags: Option<bool>,
    pub tag_match: Option<String>,
    pub series_id: Option<Vec<u64>>,
    pub game_id: Option<Vec<u64>>,
    pub event_date: Option<String>,
    pub event_week: Option<u64>,
    pub featured_order: Option<bool>,
    /// Series recurrence type.
    pub recurrence: Option<String>,
    pub created_by: Option<Vec<String>>,
    pub parent_event_id: Option<u64>,
    pub include_children: Option<bool>,
    pub locale: Option<String>,
}

impl ListEventsParams {
    pub fn new(limit: u32) -> Self {
        Self {
            limit: Some(limit),
            ..Default::default()
        }
    }

    /// Query string pairs, arrays expanded to repeated keys.
    pub(crate) fn query_pairs(&self) -> Vec<(&'static str, String)> {
        fn one<T: ToString>(q: &mut Vec<(&'static str, String)>, k: &'static str, v: &Option<T>) {
            if let Some(v) = v {
                q.push((k, v.to_string()));
            }
        }
        fn many<T: ToString>(
            q: &mut Vec<(&'static str, String)>,
            k: &'static str,
            v: &Option<Vec<T>>,
        ) {
            if let Some(v) = v {
                q.extend(v.iter().map(|x| (k, x.to_string())));
            }
        }

        let mut q = Vec::new();
        one(&mut q, "limit", &self.limit);
        one(&mut q, "order", &self.order);
        one(&mut q, "ascending", &self.ascending);
        one(&mut q, "after_cursor", &self.after_cursor);
        many(&mut q, "id", &self.id);
        many(&mut q, "slug", &self.slug);
        one(&mut q, "closed", &self.closed);
        one(&mut q, "live", &self.live);
        one(&mut q, "featured", &self.featured);
        one(&mut q, "cyom", &self.cyom);
        one(&mut q, "title_search", &self.title_search);
        one(&mut q, "liquidity_min", &self.liquidity_min);
        one(&mut q, "liquidity_max", &self.liquidity_max);
        one(&mut q, "volume_min", &self.volume_min);
        one(&mut q, "volume_max", &self.volume_max);
        one(&mut q, "start_date_min", &self.start_date_min);
        one(&mut q, "start_date_max", &self.start_date_max);
        one(&mut q, "end_date_min", &self.end_date_min);
        one(&mut q, "end_date_max", &self.end_date_max);
        one(&mut q, "start_time_min", &self.start_time_min);
        one(&mut q, "start_time_max", &self.start_time_max);
        many(&mut q, "tag_id", &self.tag_id);
        one(&mut q, "tag_slug", &self.tag_slug);
        many(&mut q, "exclude_tag_id", &self.exclude_tag_id);
        one(&mut q, "related_tags", &self.related_tags);
        one(&mut q, "tag_match", &self.tag_match);
        many(&mut q, "series_id", &self.series_id);
        many(&mut q, "game_id", &self.game_id);
        one(&mut q, "event_date", &self.event_date);
        one(&mut q, "event_week", &self.event_week);
        one(&mut q, "featured_order", &self.featured_order);
        one(&mut q, "recurrence", &self.recurrence);
        many(&mut q, "created_by", &self.created_by);
        one(&mut q, "parent_event_id", &self.parent_event_id);
        one(&mut q, "include_children", &self.include_children);
        one(&mut q, "locale", &self.locale);
        q
    }
}

/// One page of `GET /events/keyset`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EventsPage {
    pub events: Vec<PolymarketEvent>,
    /// Present only when the page is full; absent on the last page.
    pub next_cursor: Option<String>,
}

/// Polymarket event (`Event` schema).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PolymarketEvent {
    pub id: String,
    pub ticker: Option<String>,
    pub slug: Option<String>,
    pub title: Option<String>,
    pub subtitle: Option<String>,
    pub description: Option<String>,
    #[serde(rename = "resolutionSource")]
    pub resolution_source: Option<String>,
    #[serde(rename = "startDate")]
    pub start_date: Option<String>,
    #[serde(rename = "creationDate")]
    pub creation_date: Option<String>,
    #[serde(rename = "endDate")]
    pub end_date: Option<String>,
    pub image: Option<String>,
    pub icon: Option<String>,
    pub active: Option<bool>,
    pub closed: Option<bool>,
    pub archived: Option<bool>,
    pub new: Option<bool>,
    pub featured: Option<bool>,
    pub restricted: Option<bool>,
    pub liquidity: Option<f64>,
    pub volume: Option<f64>,
    #[serde(rename = "openInterest")]
    pub open_interest: Option<f64>,
    pub markets: Vec<PolymarketMarket>,
    pub series: Option<Vec<PolymarketSeries>>,
    pub categories: Option<Vec<PolymarketCategory>>,
    pub collections: Option<Vec<PolymarketCollection>>,
    pub tags: Option<Vec<PolymarketTag>>,
    #[serde(rename = "imageOptimized")]
    pub image_optimized: Option<ImageOptimized>,
    #[serde(rename = "iconOptimized")]
    pub icon_optimized: Option<ImageOptimized>,
    #[serde(rename = "featuredImageOptimized")]
    pub featured_image_optimized: Option<ImageOptimized>,
    #[serde(rename = "volume24hr")]
    pub volume_24hr: Option<f64>,
    #[serde(rename = "volume1wk")]
    pub volume_1wk: Option<f64>,
    #[serde(rename = "volume1mo")]
    pub volume_1mo: Option<f64>,
    #[serde(rename = "volume1yr")]
    pub volume_1yr: Option<f64>,
    /// Not in the spec; present on events with AMM markets.
    #[serde(rename = "liquidityAmm")]
    pub liquidity_amm: Option<f64>,
    #[serde(rename = "liquidityClob")]
    pub liquidity_clob: Option<f64>,
    pub competitive: Option<f64>,
    #[serde(rename = "createdAt")]
    pub created_at: Option<String>,
    #[serde(rename = "updatedAt")]
    pub updated_at: Option<String>,
    /// Signed: live responses include negative counts.
    #[serde(rename = "commentCount")]
    pub comment_count: Option<i64>,
    #[serde(rename = "eventDate")]
    pub event_date: Option<String>,
    #[serde(rename = "eventWeek")]
    pub event_week: Option<u64>,
    pub live: Option<bool>,
    pub ended: Option<bool>,
    #[serde(rename = "gameStatus")]
    pub game_status: Option<String>,
    #[serde(rename = "negRisk")]
    pub neg_risk: Option<bool>,
    #[serde(rename = "enableNegRisk")]
    pub enable_neg_risk: Option<bool>,
    #[serde(rename = "negRiskMarketID")]
    pub neg_risk_market_id: Option<String>,
    #[serde(rename = "seriesSlug")]
    pub series_slug: Option<String>,
    #[serde(rename = "closedTime")]
    pub closed_time: Option<String>,
}

/// Market within an event (`Market` schema).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PolymarketMarket {
    pub id: String,
    #[serde(rename = "questionID")]
    pub question_id: Option<String>,
    /// Can be empty for markets never deployed on-chain.
    #[serde(rename = "conditionId")]
    pub condition_id: String,
    pub question: Option<String>,
    pub slug: Option<String>,
    #[serde(rename = "resolutionSource")]
    pub resolution_source: Option<String>,
    #[serde(rename = "endDate")]
    pub end_date: Option<String>,
    /// Sent as a decimal string.
    #[serde(default, deserialize_with = "opt_f64_from_str")]
    pub liquidity: Option<f64>,
    #[serde(rename = "startDate")]
    pub start_date: Option<String>,
    pub image: Option<String>,
    pub icon: Option<String>,
    pub description: Option<String>,
    /// Sent as a JSON-encoded string array.
    #[serde(default, deserialize_with = "opt_json_string_list")]
    pub outcomes: Option<Vec<String>>,
    /// Sent as a JSON-encoded string array, one price per outcome.
    #[serde(
        rename = "outcomePrices",
        default,
        deserialize_with = "opt_json_string_list"
    )]
    pub outcome_prices: Option<Vec<String>>,
    /// CLOB token ids, one per outcome. Sent as a JSON-encoded string array.
    #[serde(
        rename = "clobTokenIds",
        default,
        deserialize_with = "opt_json_string_list"
    )]
    pub clob_token_ids: Option<Vec<String>>,
    /// Sent as a decimal string.
    #[serde(default, deserialize_with = "opt_f64_from_str")]
    pub volume: Option<f64>,
    pub active: Option<bool>,
    pub closed: Option<bool>,
    #[serde(rename = "enableOrderBook")]
    pub enable_order_book: Option<bool>,
    #[serde(rename = "acceptingOrders")]
    pub accepting_orders: Option<bool>,
    #[serde(rename = "orderPriceMinTickSize")]
    pub order_price_min_tick_size: Option<f64>,
    #[serde(rename = "orderMinSize")]
    pub order_min_size: Option<f64>,
    /// Not in the spec's `Market` schema; present on live markets.
    #[serde(rename = "negRisk")]
    pub neg_risk: Option<bool>,
    /// Not in the spec; present on AMM (FPMM) markets.
    #[serde(rename = "marketMakerAddress")]
    pub market_maker_address: Option<String>,
    #[serde(rename = "createdAt")]
    pub created_at: Option<String>,
    #[serde(rename = "updatedAt")]
    pub updated_at: Option<String>,
    #[serde(rename = "new")]
    pub is_new: Option<bool>,
    pub featured: Option<bool>,
    /// Not in the spec; present on live markets.
    pub submitted_by: Option<String>,
    pub archived: Option<bool>,
    #[serde(rename = "resolvedBy")]
    pub resolved_by: Option<String>,
    pub restricted: Option<bool>,
    #[serde(rename = "groupItemTitle")]
    pub group_item_title: Option<String>,
    #[serde(rename = "groupItemThreshold")]
    pub group_item_threshold: Option<String>,
    // Market-data fields below are optional in the spec; each was absent on some live rows of
    // `/markets/keyset` (2026-11-16, 1,400 rows across open, closed and sports filters).
    #[serde(rename = "bestBid", default)]
    pub best_bid: Option<f64>,
    #[serde(rename = "bestAsk", default)]
    pub best_ask: Option<f64>,
    #[serde(default)]
    pub spread: Option<f64>,
    #[serde(rename = "lastTradePrice", default)]
    pub last_trade_price: Option<f64>,
    #[serde(rename = "volumeNum", default)]
    pub volume_num: Option<f64>,
    #[serde(rename = "liquidityNum", default)]
    pub liquidity_num: Option<f64>,
    #[serde(rename = "volume24hr", default)]
    pub volume_24hr: Option<f64>,
    #[serde(rename = "volume1wk", default)]
    pub volume_1wk: Option<f64>,
    #[serde(rename = "volume1mo", default)]
    pub volume_1mo: Option<f64>,
    #[serde(rename = "volume1yr", default)]
    pub volume_1yr: Option<f64>,
    #[serde(rename = "volumeClob", default)]
    pub volume_clob: Option<f64>,
    #[serde(rename = "volume24hrClob", default)]
    pub volume_24hr_clob: Option<f64>,
    #[serde(rename = "liquidityClob", default)]
    pub liquidity_clob: Option<f64>,
    #[serde(default)]
    pub competitive: Option<f64>,
    #[serde(rename = "oneHourPriceChange", default)]
    pub one_hour_price_change: Option<f64>,
    #[serde(rename = "oneDayPriceChange", default)]
    pub one_day_price_change: Option<f64>,
    #[serde(rename = "oneWeekPriceChange", default)]
    pub one_week_price_change: Option<f64>,
    #[serde(rename = "oneMonthPriceChange", default)]
    pub one_month_price_change: Option<f64>,
    #[serde(rename = "oneYearPriceChange", default)]
    pub one_year_price_change: Option<f64>,
    #[serde(rename = "negRiskMarketID", default)]
    pub neg_risk_market_id: Option<String>,
    #[serde(rename = "negRiskRequestID", default)]
    pub neg_risk_request_id: Option<String>,
    #[serde(rename = "negRiskOther", default)]
    pub neg_risk_other: Option<bool>,
    #[serde(rename = "endDateIso", default)]
    pub end_date_iso: Option<String>,
    #[serde(rename = "startDateIso", default)]
    pub start_date_iso: Option<String>,
    /// E.g. `2020-11-02 16:31:01+00`.
    #[serde(rename = "closedTime", default)]
    pub closed_time: Option<String>,
    #[serde(rename = "acceptingOrdersTimestamp", default)]
    pub accepting_orders_timestamp: Option<String>,
    #[serde(rename = "umaResolutionStatus", default)]
    pub uma_resolution_status: Option<String>,
    /// Sent as a JSON-encoded string array.
    #[serde(
        rename = "umaResolutionStatuses",
        default,
        deserialize_with = "opt_json_string_list"
    )]
    pub uma_resolution_statuses: Option<Vec<String>>,
    /// Basis points.
    #[serde(rename = "makerBaseFee", default)]
    pub maker_base_fee: Option<i64>,
    #[serde(rename = "takerBaseFee", default)]
    pub taker_base_fee: Option<i64>,
    #[serde(rename = "feesEnabled", default)]
    pub fees_enabled: Option<bool>,
    /// Present but `null` on some markets.
    #[serde(rename = "feeType", default)]
    pub fee_type: Option<String>,
    #[serde(rename = "feeSchedule", default)]
    pub fee_schedule: Option<GammaFeeSchedule>,
    #[serde(rename = "rewardsMinSize", default)]
    pub rewards_min_size: Option<f64>,
    #[serde(rename = "rewardsMaxSpread", default)]
    pub rewards_max_spread: Option<f64>,
    #[serde(rename = "clobRewards", default)]
    pub clob_rewards: Option<Vec<GammaClobReward>>,
    #[serde(rename = "rfqEnabled", default)]
    pub rfq_enabled: Option<bool>,
    #[serde(rename = "secondsDelay", default)]
    pub seconds_delay: Option<u64>,
    /// `normal` or `scalar`.
    #[serde(rename = "marketType", default)]
    pub market_type: Option<String>,
    /// Scalar markets only; decimal string.
    #[serde(rename = "lowerBound", default, deserialize_with = "opt_f64_from_str")]
    pub lower_bound: Option<f64>,
    #[serde(rename = "upperBound", default, deserialize_with = "opt_f64_from_str")]
    pub upper_bound: Option<f64>,
    #[serde(rename = "sportsMarketType", default)]
    pub sports_market_type: Option<String>,
    #[serde(rename = "gameId", default)]
    pub game_id: Option<String>,
    #[serde(rename = "gameStartTime", default)]
    pub game_start_time: Option<String>,
    #[serde(rename = "eventStartTime", default)]
    pub event_start_time: Option<String>,
    /// Spread/total line of a sports market.
    #[serde(default)]
    pub line: Option<f64>,
    pub category: Option<String>,
    /// Parent events; sent by the `/markets*` routes, not on markets embedded in an event.
    #[serde(default)]
    pub events: Option<Vec<GammaEventRef>>,
    /// Sent with `include_tag=true`.
    #[serde(default)]
    pub tags: Option<Vec<PolymarketTag>>,
}

/// Series (`Series` schema). Embedded in events with few fields; `/series` rows carry more.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PolymarketSeries {
    pub id: String,
    pub title: Option<String>,
    pub slug: Option<String>,
    pub description: Option<String>,
    pub ticker: Option<String>,
    #[serde(rename = "seriesType")]
    pub series_type: Option<String>,
    pub recurrence: Option<String>,
    pub active: Option<bool>,
    pub closed: Option<bool>,
    pub archived: Option<bool>,
    pub featured: Option<bool>,
    pub restricted: Option<bool>,
    pub image: Option<String>,
    pub icon: Option<String>,
    #[serde(rename = "startDate")]
    pub start_date: Option<String>,
    pub volume: Option<f64>,
    #[serde(rename = "volume24hr")]
    pub volume_24hr: Option<f64>,
    pub liquidity: Option<f64>,
    /// Sent as a decimal string on series (a number on events).
    #[serde(default, deserialize_with = "opt_f64_from_str")]
    pub competitive: Option<f64>,
    #[serde(rename = "createdAt")]
    pub created_at: Option<String>,
    #[serde(rename = "updatedAt")]
    pub updated_at: Option<String>,
    /// Child events (`/series` without `exclude_events=true`); these carry no markets.
    pub events: Option<Vec<GammaEventRef>>,
}

/// Category (`Category` schema).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PolymarketCategory {
    pub id: String,
    pub label: Option<String>,
    pub slug: Option<String>,
}

/// Collection (`Collection` schema).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PolymarketCollection {
    pub id: String,
    pub title: Option<String>,
    pub slug: Option<String>,
}

/// Tag (`Tag` schema). Only `id`, `label` and `slug` were sent on every live row.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PolymarketTag {
    pub id: String,
    pub label: Option<String>,
    pub slug: Option<String>,
    #[serde(rename = "forceShow")]
    pub force_show: Option<bool>,
    #[serde(rename = "forceHide")]
    pub force_hide: Option<bool>,
    #[serde(rename = "isCarousel")]
    pub is_carousel: Option<bool>,
    #[serde(rename = "publishedAt")]
    pub published_at: Option<String>,
    #[serde(rename = "createdAt")]
    pub created_at: Option<String>,
    #[serde(rename = "updatedAt")]
    pub updated_at: Option<String>,
    /// Sent by `/tags/.../related-tags/tags`.
    #[serde(rename = "activeEventsCount")]
    pub active_events_count: Option<u64>,
}

/// Parent event embedded in a market or series (an `Event` without `markets`). Only `id` is
/// required; the other keys were absent on some live rows.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GammaEventRef {
    pub id: String,
    pub ticker: Option<String>,
    pub slug: Option<String>,
    pub title: Option<String>,
    #[serde(rename = "startDate")]
    pub start_date: Option<String>,
    #[serde(rename = "endDate")]
    pub end_date: Option<String>,
    pub active: Option<bool>,
    pub closed: Option<bool>,
    pub archived: Option<bool>,
    #[serde(rename = "negRisk")]
    pub neg_risk: Option<bool>,
    #[serde(rename = "enableNegRisk")]
    pub enable_neg_risk: Option<bool>,
    #[serde(rename = "negRiskMarketID")]
    pub neg_risk_market_id: Option<String>,
    #[serde(rename = "seriesSlug")]
    pub series_slug: Option<String>,
    pub liquidity: Option<f64>,
    pub volume: Option<f64>,
    #[serde(rename = "volume24hr")]
    pub volume_24hr: Option<f64>,
    #[serde(rename = "openInterest")]
    pub open_interest: Option<f64>,
    #[serde(rename = "closedTime")]
    pub closed_time: Option<String>,
}

/// `feeSchedule` of a market.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GammaFeeSchedule {
    pub exponent: f64,
    pub rate: f64,
    #[serde(rename = "rebateRate")]
    pub rebate_rate: f64,
    #[serde(rename = "takerOnly")]
    pub taker_only: bool,
}

/// `clobRewards` item of a market.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GammaClobReward {
    pub id: String,
    #[serde(rename = "conditionId")]
    pub condition_id: String,
    #[serde(rename = "assetAddress")]
    pub asset_address: String,
    #[serde(rename = "rewardsAmount")]
    pub rewards_amount: f64,
    #[serde(rename = "rewardsDailyRate")]
    pub rewards_daily_rate: f64,
    #[serde(rename = "startDate")]
    pub start_date: String,
    #[serde(rename = "endDate")]
    pub end_date: String,
}

/// One page of `GET /markets/keyset`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GammaMarketsPage {
    pub markets: Vec<PolymarketMarket>,
    /// Present only when the page is full; absent on the last page.
    pub next_cursor: Option<String>,
}

/// `closed` filter of `/markets/keyset`. The exchange default (`None`) is open markets only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GammaClosedFilter {
    Open,
    Closed,
    /// Both states; the exchange accepts it only together with an identifier filter (`id`,
    /// `slug`, `clob_token_ids`, `condition_ids`, `question_ids`).
    Any,
}

impl std::fmt::Display for GammaClosedFilter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Open => "false",
            Self::Closed => "true",
            Self::Any => "any",
        })
    }
}

/// Query parameters for `GET /markets/keyset`. `None` fields are not sent. Arrays are sent as
/// repeated keys.
#[derive(Debug, Clone, Default)]
pub struct GammaListMarketsParams {
    /// Page size, 1..=100 (exchange default 20).
    pub limit: Option<u32>,
    /// Comma-separated JSON field names, e.g. `volume_num,liquidity_num`.
    pub order: Option<String>,
    pub ascending: Option<bool>,
    /// `next_cursor` from the previous page.
    pub after_cursor: Option<String>,
    pub id: Option<Vec<u64>>,
    pub slug: Option<Vec<String>>,
    pub closed: Option<GammaClosedFilter>,
    pub decimalized: Option<bool>,
    pub clob_token_ids: Option<Vec<String>>,
    pub condition_ids: Option<Vec<String>>,
    pub question_ids: Option<Vec<String>>,
    pub liquidity_num_min: Option<f64>,
    pub liquidity_num_max: Option<f64>,
    pub volume_num_min: Option<f64>,
    pub volume_num_max: Option<f64>,
    /// ISO 8601 date-time.
    pub start_date_min: Option<String>,
    pub start_date_max: Option<String>,
    pub end_date_min: Option<String>,
    pub end_date_max: Option<String>,
    pub tag_id: Option<Vec<u64>>,
    pub related_tags: Option<bool>,
    pub tag_match: Option<String>,
    pub cyom: Option<bool>,
    pub rfq_enabled: Option<bool>,
    pub uma_resolution_status: Option<String>,
    pub game_id: Option<String>,
    pub sports_market_types: Option<Vec<String>>,
    /// Adds `tags` to each market.
    pub include_tag: Option<bool>,
    pub locale: Option<String>,
}

impl GammaListMarketsParams {
    pub(crate) fn query(&self) -> Query {
        let mut q = Query::default();
        q.opt("limit", &self.limit)
            .opt("order", &self.order)
            .opt("ascending", &self.ascending)
            .opt("after_cursor", &self.after_cursor)
            .many("id", &self.id)
            .many("slug", &self.slug)
            .opt("closed", &self.closed)
            .opt("decimalized", &self.decimalized)
            .many("clob_token_ids", &self.clob_token_ids)
            .many("condition_ids", &self.condition_ids)
            .many("question_ids", &self.question_ids)
            .opt("liquidity_num_min", &self.liquidity_num_min)
            .opt("liquidity_num_max", &self.liquidity_num_max)
            .opt("volume_num_min", &self.volume_num_min)
            .opt("volume_num_max", &self.volume_num_max)
            .opt("start_date_min", &self.start_date_min)
            .opt("start_date_max", &self.start_date_max)
            .opt("end_date_min", &self.end_date_min)
            .opt("end_date_max", &self.end_date_max)
            .many("tag_id", &self.tag_id)
            .opt("related_tags", &self.related_tags)
            .opt("tag_match", &self.tag_match)
            .opt("cyom", &self.cyom)
            .opt("rfq_enabled", &self.rfq_enabled)
            .opt("uma_resolution_status", &self.uma_resolution_status)
            .opt("game_id", &self.game_id)
            .many("sports_market_types", &self.sports_market_types)
            .opt("include_tag", &self.include_tag)
            .opt("locale", &self.locale);
        q
    }
}

/// Body of `POST /markets/information` and `POST /markets/abridged` (`MarketsInformationBody`).
/// `None` / unset fields are omitted.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GammaMarketsInformationBody {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Vec<u64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slug: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub closed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clob_token_ids: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub condition_ids: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub liquidity_num_min: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub liquidity_num_max: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volume_num_min: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volume_num_max: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_date_min: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_date_max: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_date_min: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_date_max: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub related_tags: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag_id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cyom: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uma_resolution_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub game_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sports_market_types: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rewards_min_size: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub question_ids: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_tags: Option<bool>,
}

/// `GET /markets/{id}/description`. Live sends `id` and `conditionId` as `""`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GammaMarketDescription {
    pub id: String,
    #[serde(rename = "conditionId")]
    pub condition_id: String,
    #[serde(deserialize_with = "crate::types::de::nullable")]
    pub description: Option<String>,
}

/// Offset paging used by `/tags`, `/series`, `/teams`: `None` fields are not sent.
#[derive(Debug, Clone, Default)]
pub struct GammaPageParams {
    /// The exchange caps pages at 100 rows (`/series` at 50 by default).
    pub limit: Option<u32>,
    pub offset: Option<u32>,
    /// Comma-separated field names.
    pub order: Option<String>,
    pub ascending: Option<bool>,
}

impl GammaPageParams {
    pub(crate) fn query(&self) -> Query {
        let mut q = Query::default();
        q.opt("limit", &self.limit)
            .opt("offset", &self.offset)
            .opt("order", &self.order)
            .opt("ascending", &self.ascending);
        q
    }
}

/// Filters of `GET /tags`.
#[derive(Debug, Clone, Default)]
pub struct GammaTagsParams {
    pub page: GammaPageParams,
    pub include_template: Option<bool>,
    pub is_carousel: Option<bool>,
}

/// `status` filter of the related-tags routes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GammaRelatedTagStatus {
    Active,
    Closed,
    All,
}

impl std::fmt::Display for GammaRelatedTagStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Active => "active",
            Self::Closed => "closed",
            Self::All => "all",
        })
    }
}

/// Filters of the four related-tags routes.
#[derive(Debug, Clone, Default)]
pub struct GammaRelatedTagsParams {
    pub omit_empty: Option<bool>,
    pub status: Option<GammaRelatedTagStatus>,
}

impl GammaRelatedTagsParams {
    pub(crate) fn query(&self) -> Query {
        let mut q = Query::default();
        q.opt("omit_empty", &self.omit_empty)
            .opt("status", &self.status);
        q
    }
}

/// `GET /tags/{id}/related-tags` row (`RelatedTag`).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GammaRelatedTag {
    pub id: String,
    #[serde(rename = "tagID")]
    pub tag_id: u64,
    #[serde(rename = "relatedTagID")]
    pub related_tag_id: u64,
    pub rank: u32,
}

/// Filters of `GET /series`.
#[derive(Debug, Clone, Default)]
pub struct GammaSeriesParams {
    pub page: GammaPageParams,
    pub slug: Option<Vec<String>>,
    pub categories_ids: Option<Vec<u64>>,
    pub categories_labels: Option<Vec<String>>,
    pub closed: Option<bool>,
    pub recurrence: Option<String>,
    /// Leave out each series' `events`.
    pub exclude_events: Option<bool>,
}

impl GammaSeriesParams {
    pub(crate) fn query(&self) -> Query {
        let mut q = self.page.query();
        q.many("slug", &self.slug)
            .many("categories_ids", &self.categories_ids)
            .many("categories_labels", &self.categories_labels)
            .opt("closed", &self.closed)
            .opt("recurrence", &self.recurrence)
            .opt("exclude_events", &self.exclude_events);
        q
    }
}

/// `GET /series-summary/{id}` (`SeriesSummary`).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GammaSeriesSummary {
    pub id: String,
    pub title: Option<String>,
    pub slug: Option<String>,
    /// `YYYY-MM-DD`.
    #[serde(rename = "eventDates")]
    pub event_dates: Vec<String>,
    #[serde(rename = "eventWeeks")]
    pub event_weeks: Vec<u32>,
    #[serde(rename = "volume24hr")]
    pub volume_24hr: Option<f64>,
    pub earliest_open_week: Option<u32>,
    pub earliest_open_date: Option<String>,
}

/// `GET /sports` row (`SportsMetadata`).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GammaSport {
    pub id: u64,
    /// League code, e.g. `nba`.
    pub sport: String,
    pub name: String,
    pub image: String,
    /// Resolution source URL.
    pub resolution: String,
    /// `home` or `away`.
    pub ordering: String,
    /// Comma-separated tag ids, as sent.
    pub tags: String,
    #[serde(rename = "primaryTagId")]
    pub primary_tag_id: u64,
    /// Series id, as sent (a string).
    pub series: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
}

/// `GET /sports/market-types`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GammaSportsMarketTypes {
    #[serde(rename = "marketTypes")]
    pub market_types: Vec<String>,
}

/// Filters of `GET /teams`.
#[derive(Debug, Clone, Default)]
pub struct GammaTeamsParams {
    pub page: GammaPageParams,
    pub league: Option<Vec<String>>,
    pub name: Option<Vec<String>>,
    pub abbreviation: Option<Vec<String>>,
}

impl GammaTeamsParams {
    pub(crate) fn query(&self) -> Query {
        let mut q = self.page.query();
        q.many("league", &self.league)
            .many("name", &self.name)
            .many("abbreviation", &self.abbreviation);
        q
    }
}

/// `GET /teams` row (`Team`). Optional keys were absent on some of 3,000 live rows.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GammaTeam {
    pub id: u64,
    pub name: String,
    pub league: String,
    pub abbreviation: String,
    pub logo: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    pub record: Option<String>,
    #[serde(rename = "providerId")]
    pub provider_id: Option<u64>,
    pub color: Option<String>,
    pub alias: Option<String>,
    #[serde(rename = "updatedAt")]
    pub updated_at: Option<String>,
}

/// Query of `GET /public-search`. `q` is passed separately.
#[derive(Debug, Clone, Default)]
pub struct GammaSearchParams {
    pub cache: Option<bool>,
    pub events_status: Option<String>,
    pub limit_per_type: Option<u32>,
    pub page: Option<u32>,
    pub events_tag: Option<Vec<String>>,
    pub keep_closed_markets: Option<u32>,
    pub sort: Option<String>,
    pub ascending: Option<bool>,
    /// Adds `tags` to the result.
    pub search_tags: Option<bool>,
    /// Adds `profiles` (not modelled; out of scope).
    pub search_profiles: Option<bool>,
    pub recurrence: Option<String>,
    pub exclude_tag_id: Option<Vec<u64>>,
    pub optimized: Option<bool>,
}

impl GammaSearchParams {
    pub(crate) fn query(&self, q_text: &str) -> Query {
        let mut q = Query::default();
        q.req("q", &q_text)
            .opt("cache", &self.cache)
            .opt("events_status", &self.events_status)
            .opt("limit_per_type", &self.limit_per_type)
            .opt("page", &self.page)
            .many("events_tag", &self.events_tag)
            .opt("keep_closed_markets", &self.keep_closed_markets)
            .opt("sort", &self.sort)
            .opt("ascending", &self.ascending)
            .opt("search_tags", &self.search_tags)
            .opt("search_profiles", &self.search_profiles)
            .opt("recurrence", &self.recurrence)
            .many("exclude_tag_id", &self.exclude_tag_id)
            .opt("optimized", &self.optimized);
        q
    }
}

/// `GET /public-search` (`Search`). Wallet `profiles` are not modelled (ignored).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GammaSearch {
    pub events: Option<Vec<PolymarketEvent>>,
    /// Sent with `search_tags=true`.
    pub tags: Option<Vec<GammaSearchTag>>,
    pub pagination: GammaPagination,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GammaSearchTag {
    pub id: String,
    pub label: String,
    pub slug: String,
    pub event_count: u64,
}

/// `GET /events/pagination`: `{ "data": [...], "pagination": {...} }`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GammaEventsPagination {
    pub data: Vec<PolymarketEvent>,
    pub pagination: GammaPagination,
}

/// `Pagination` of `/public-search` and `/events/pagination`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GammaPagination {
    #[serde(rename = "hasMore")]
    pub has_more: bool,
    #[serde(rename = "totalResults")]
    pub total_results: u64,
}

/// Gamma error body, e.g. `{"type": "not found error", "error": "id not found"}`. Returned as
/// the body of a [`crate::PolymarketApiError::Status`].
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GammaError {
    #[serde(rename = "type")]
    pub kind: String,
    pub error: String,
}

/// Optimized image metadata (`ImageOptimization` schema).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ImageOptimized {
    pub id: String,
    #[serde(rename = "imageUrlSource")]
    pub image_url_source: Option<String>,
    #[serde(rename = "imageUrlOptimized")]
    pub image_url_optimized: Option<String>,
    #[serde(rename = "imageSizeKbSource")]
    pub image_size_kb_source: Option<f64>,
    #[serde(rename = "imageSizeKbOptimized")]
    pub image_size_kb_optimized: Option<f64>,
    #[serde(rename = "imageOptimizedComplete")]
    pub image_optimized_complete: Option<bool>,
    #[serde(rename = "imageOptimizedLastUpdated")]
    pub image_optimized_last_updated: Option<String>,
    #[serde(rename = "relID")]
    pub rel_id: Option<i64>,
    pub field: Option<String>,
    pub relname: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_pairs_repeat_array_keys_and_skip_unset() {
        let params = ListEventsParams {
            limit: Some(100),
            after_cursor: Some("abc".into()),
            id: Some(vec![2890, 3933]),
            closed: Some(false),
            volume_min: Some(1000.5),
            ..Default::default()
        };
        let pairs = params.query_pairs();
        let expected: Vec<(&str, String)> = vec![
            ("limit", "100".into()),
            ("after_cursor", "abc".into()),
            ("id", "2890".into()),
            ("id", "3933".into()),
            ("closed", "false".into()),
            ("volume_min", "1000.5".into()),
        ];
        assert_eq!(pairs, expected);
        assert!(ListEventsParams::default().query_pairs().is_empty());
    }

    #[test]
    fn market_string_fields_are_strict() {
        let base =
            r#"{"id":"1","conditionId":"0x01","liquidity":"12.5","outcomes":"[\"Yes\",\"No\"]"}"#;
        let m: PolymarketMarket = serde_json::from_str(base).unwrap();
        assert_eq!(m.liquidity, Some(12.5));
        assert_eq!(
            m.outcomes.as_deref(),
            Some(&["Yes".to_string(), "No".to_string()][..])
        );
        assert_eq!(m.volume, None);

        let bad_number = r#"{"id":"1","conditionId":"0x01","liquidity":"abc"}"#;
        assert!(serde_json::from_str::<PolymarketMarket>(bad_number).is_err());
        let bad_list = r#"{"id":"1","conditionId":"0x01","outcomes":"Yes,No"}"#;
        assert!(serde_json::from_str::<PolymarketMarket>(bad_list).is_err());
        let missing_condition = r#"{"id":"1"}"#;
        assert!(serde_json::from_str::<PolymarketMarket>(missing_condition).is_err());
    }
}
