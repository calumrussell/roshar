use serde::{Deserialize, Deserializer, Serialize};

/// Deserialize Option<u8> from either boolean or integer
fn bool_or_int<'de, D>(deserializer: D) -> Result<Option<u8>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum BoolOrInt {
        Bool(bool),
        Int(u8),
    }

    match Option::<BoolOrInt>::deserialize(deserializer)? {
        None => Ok(None),
        Some(BoolOrInt::Bool(b)) => Ok(Some(if b { 1 } else { 0 })),
        Some(BoolOrInt::Int(i)) => Ok(Some(i)),
    }
}

/// Deserialize Option<f64> from either string or number
fn string_or_float<'de, D>(deserializer: D) -> Result<Option<f64>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum StringOrFloat {
        String(String),
        Float(f64),
    }

    match Option::<StringOrFloat>::deserialize(deserializer)? {
        None => Ok(None),
        Some(StringOrFloat::String(s)) => s.parse::<f64>().ok().map(Some).ok_or_else(|| {
            serde::de::Error::custom(format!("failed to parse '{}' as f64", s))
        }),
        Some(StringOrFloat::Float(f)) => Ok(Some(f)),
    }
}

/// Deserialize Vec<String> from either JSON string or array
fn json_string_or_vec<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum JsonStringOrVec {
        JsonString(String),
        Vec(Vec<String>),
    }

    match JsonStringOrVec::deserialize(deserializer)? {
        JsonStringOrVec::JsonString(s) => {
            serde_json::from_str(&s).map_err(serde::de::Error::custom)
        }
        JsonStringOrVec::Vec(v) => Ok(v),
    }
}

/// Query parameters for listing events
#[derive(Debug, Clone, Default, Serialize)]
pub struct ListEventsParams {
    /// Number of results to return
    pub limit: u32,
    /// Offset for pagination
    pub offset: u32,
    /// Comma-separated fields for ordering
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Sort direction (true for ascending)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ascending: Option<bool>,
    /// Filter by event IDs
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Vec<u64>>,
    /// Filter by tag ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag_id: Option<u64>,
    /// Exclude specific tags
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exclude_tag_id: Option<Vec<u64>>,
    /// Filter by event slugs
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slug: Option<Vec<String>>,
    /// Filter by tag slug
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag_slug: Option<String>,
    /// Include related tags
    #[serde(skip_serializing_if = "Option::is_none")]
    pub related_tags: Option<bool>,
    /// Filter active events
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active: Option<bool>,
    /// Filter archived status
    #[serde(skip_serializing_if = "Option::is_none")]
    pub archived: Option<bool>,
    /// Featured events only
    #[serde(skip_serializing_if = "Option::is_none")]
    pub featured: Option<bool>,
    /// "Create Your Own Market" filter
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cyom: Option<bool>,
    /// Include chat data
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_chat: Option<bool>,
    /// Include templates
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_template: Option<bool>,
    /// Series recurrence type
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recurrence: Option<String>,
    /// Filter closed events
    #[serde(skip_serializing_if = "Option::is_none")]
    pub closed: Option<bool>,
    /// Minimum liquidity filter
    #[serde(skip_serializing_if = "Option::is_none")]
    pub liquidity_min: Option<f64>,
    /// Maximum liquidity filter
    #[serde(skip_serializing_if = "Option::is_none")]
    pub liquidity_max: Option<f64>,
    /// Minimum volume filter
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volume_min: Option<f64>,
    /// Maximum volume filter
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volume_max: Option<f64>,
    /// Minimum start date (ISO 8601)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_date_min: Option<String>,
    /// Maximum start date (ISO 8601)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_date_max: Option<String>,
    /// Minimum end date (ISO 8601)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_date_min: Option<String>,
    /// Maximum end date (ISO 8601)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_date_max: Option<String>,
}

impl ListEventsParams {
    pub fn new(limit: u32, offset: u32) -> Self {
        Self {
            limit,
            offset,
            ..Default::default()
        }
    }
}

/// Polymarket event response
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PolymarketEvent {
    pub id: String,
    #[serde(default)]
    pub ticker: Option<String>,
    #[serde(default)]
    pub slug: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub subtitle: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(rename = "resolutionSource", default)]
    pub resolution_source: Option<String>,
    #[serde(rename = "startDate", default)]
    pub start_date: Option<String>,
    #[serde(rename = "creationDate", default)]
    pub creation_date: Option<String>,
    #[serde(rename = "endDate", default)]
    pub end_date: Option<String>,
    #[serde(default)]
    pub image: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default, deserialize_with = "bool_or_int")]
    pub active: Option<u8>,
    #[serde(default, deserialize_with = "bool_or_int")]
    pub closed: Option<u8>,
    #[serde(default, deserialize_with = "bool_or_int")]
    pub archived: Option<u8>,
    #[serde(default, deserialize_with = "bool_or_int")]
    pub new: Option<u8>,
    #[serde(default, deserialize_with = "bool_or_int")]
    pub featured: Option<u8>,
    #[serde(default, deserialize_with = "bool_or_int")]
    pub restricted: Option<u8>,
    #[serde(default, deserialize_with = "string_or_float")]
    pub liquidity: Option<f64>,
    #[serde(default, deserialize_with = "string_or_float")]
    pub volume: Option<f64>,
    #[serde(rename = "openInterest", default, deserialize_with = "string_or_float")]
    pub open_interest: Option<f64>,
    #[serde(default)]
    pub markets: Vec<PolymarketMarket>,
    #[serde(default)]
    pub series: Vec<PolymarketSeries>,
    #[serde(default)]
    pub categories: Vec<PolymarketCategory>,
    #[serde(default)]
    pub collections: Vec<PolymarketCollection>,
    #[serde(default)]
    pub tags: Vec<PolymarketTag>,
    #[serde(rename = "imageOptimized", default)]
    pub image_optimized: Option<ImageOptimized>,
    #[serde(rename = "iconOptimized", default)]
    pub icon_optimized: Option<ImageOptimized>,
    #[serde(rename = "featuredImageOptimized", default)]
    pub featured_image_optimized: Option<ImageOptimized>,
    #[serde(rename = "volume24hr", default, deserialize_with = "string_or_float")]
    pub volume_24hr: Option<f64>,
    #[serde(rename = "volume1wk", default, deserialize_with = "string_or_float")]
    pub volume_1wk: Option<f64>,
    #[serde(rename = "volume1mo", default, deserialize_with = "string_or_float")]
    pub volume_1mo: Option<f64>,
    #[serde(rename = "volume1yr", default, deserialize_with = "string_or_float")]
    pub volume_1yr: Option<f64>,
    #[serde(rename = "liquidityAmm", default, deserialize_with = "string_or_float")]
    pub liquidity_amm: Option<f64>,
    #[serde(rename = "liquidityClob", default, deserialize_with = "string_or_float")]
    pub liquidity_clob: Option<f64>,
    #[serde(default, deserialize_with = "string_or_float")]
    pub competitive: Option<f64>,
    #[serde(rename = "createdAt", default)]
    pub created_at: Option<String>,
    #[serde(rename = "updatedAt", default)]
    pub updated_at: Option<String>,
    #[serde(rename = "commentCount", default)]
    pub comment_count: Option<u64>,
    #[serde(rename = "eventDate", default)]
    pub event_date: Option<String>,
    #[serde(rename = "eventWeek", default)]
    pub event_week: Option<u64>,
    #[serde(default, deserialize_with = "bool_or_int")]
    pub live: Option<u8>,
    #[serde(default, deserialize_with = "bool_or_int")]
    pub ended: Option<u8>,
    #[serde(rename = "gameStatus", default)]
    pub game_status: Option<String>,
}

/// Market data within an event
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PolymarketMarket {
    pub id: String,
    #[serde(rename = "questionID", default)]
    pub question_id: Option<String>,
    #[serde(rename = "conditionId", default)]
    pub condition_id: Option<String>,
    #[serde(default)]
    pub question: Option<String>,
    #[serde(default)]
    pub ticker: Option<String>,
    #[serde(default)]
    pub slug: Option<String>,
    #[serde(rename = "resolutionSource", default)]
    pub resolution_source: Option<String>,
    #[serde(rename = "endDate", default)]
    pub end_date: Option<String>,
    #[serde(default, deserialize_with = "string_or_float")]
    pub liquidity: Option<f64>,
    #[serde(rename = "startDate", default)]
    pub start_date: Option<String>,
    #[serde(default)]
    pub image: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default, deserialize_with = "json_string_or_vec")]
    pub outcomes: Vec<String>,
    #[serde(rename = "outcomePrices", default, deserialize_with = "json_string_or_vec")]
    pub outcome_prices: Vec<String>,
    #[serde(default, deserialize_with = "string_or_float")]
    pub volume: Option<f64>,
    #[serde(default, deserialize_with = "bool_or_int")]
    pub active: Option<u8>,
    #[serde(default, deserialize_with = "bool_or_int")]
    pub closed: Option<u8>,
    #[serde(rename = "marketMakerAddress", default)]
    pub market_maker_address: Option<String>,
    #[serde(rename = "createdAt", default)]
    pub created_at: Option<String>,
    #[serde(rename = "updatedAt", default)]
    pub updated_at: Option<String>,
    #[serde(rename = "new", default, deserialize_with = "bool_or_int")]
    pub is_new: Option<u8>,
    #[serde(default, deserialize_with = "bool_or_int")]
    pub featured: Option<u8>,
    #[serde(default)]
    pub submitted_by: Option<String>,
    #[serde(default, deserialize_with = "bool_or_int")]
    pub archived: Option<u8>,
    #[serde(rename = "resolvedBy", default)]
    pub resolved_by: Option<String>,
    #[serde(default, deserialize_with = "bool_or_int")]
    pub restricted: Option<u8>,
    #[serde(rename = "groupItemTitle", default)]
    pub group_item_title: Option<String>,
    #[serde(rename = "groupItemThreshold", default)]
    pub group_item_threshold: Option<String>,
}

/// Series information
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PolymarketSeries {
    pub id: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub slug: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

/// Category information
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PolymarketCategory {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub slug: Option<String>,
}

/// Collection information
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PolymarketCollection {
    pub id: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub slug: Option<String>,
}

/// Tag information
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PolymarketTag {
    pub id: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub slug: Option<String>,
}

/// Optimized image metadata
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ImageOptimized {
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub width: Option<u32>,
    #[serde(default)]
    pub height: Option<u32>,
    #[serde(default)]
    pub format: Option<String>,
}
