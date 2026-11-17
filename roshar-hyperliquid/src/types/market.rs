//! Public `/info` market-data responses not covered by `info.rs` (dexes, funding predictions,
//! annotations, spot token details, outcome markets). Shapes follow live responses of 2026-11-16.

use serde::{Deserialize, Serialize};

/// One `perpDexs` entry. The first perp dex is returned as `null` (see [`crate::Info::perp_dexs`]).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PerpDex {
    pub name: String,
    pub full_name: String,
    pub deployer: String,
    #[serde(deserialize_with = "super::nullable")]
    pub oracle_updater: Option<String>,
    #[serde(deserialize_with = "super::nullable")]
    pub fee_recipient: Option<String>,
    /// `[coin, oi cap]` pairs.
    pub asset_to_streaming_oi_cap: Vec<(String, String)>,
    /// `[action, [addresses]]` pairs.
    pub sub_deployers: Vec<(String, Vec<String>)>,
    pub asset_to_funding_multiplier: Vec<(String, String)>,
    pub asset_to_funding_interest_rate: Vec<(String, String)>,
    pub asset_to_funding_clamp: Vec<(String, String)>,
}

/// One venue's prediction in `predictedFundings`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PredictedFunding {
    pub funding_rate: String,
    pub next_funding_time: u64,
    /// Absent for some venue/coin pairs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub funding_interval_hours: Option<u32>,
}

/// `predictedFundings`: `[coin, [[venue, prediction or null]]]`. Venues are e.g. `"HlPerp"`,
/// `"BinPerp"`, `"BybitPerp"`; `null` when the venue does not list the coin.
pub type PredictedFundings = Vec<(String, Vec<(String, Option<PredictedFunding>)>)>;

/// `perpDexLimits` of a HIP-3 dex.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PerpDexLimits {
    pub total_oi_cap: String,
    pub oi_sz_cap_per_perp: String,
    pub max_transfer_ntl: String,
    /// `[coin, oi cap]` pairs.
    pub coin_to_oi_cap: Vec<(String, String)>,
}

/// `perpDexStatus`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PerpDexStatus {
    pub total_net_deposit: String,
}

/// `perpAnnotation`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PerpAnnotation {
    pub category: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keywords: Option<Vec<String>>,
}

/// One `perpConciseAnnotations` value.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PerpConciseAnnotation {
    pub category: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keywords: Option<Vec<String>>,
}

/// `tokenDetails`. Genesis/deploy fields are `null` for tokens without a genesis (e.g. USDC, PURR).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenDetails {
    pub name: String,
    pub max_supply: String,
    pub total_supply: String,
    pub circulating_supply: String,
    pub sz_decimals: u8,
    pub wei_decimals: u8,
    /// Null when the token has no book.
    #[serde(deserialize_with = "super::nullable")]
    pub mid_px: Option<String>,
    pub mark_px: String,
    pub prev_day_px: String,
    #[serde(deserialize_with = "super::nullable")]
    pub genesis: Option<TokenGenesis>,
    #[serde(deserialize_with = "super::nullable")]
    pub deployer: Option<String>,
    #[serde(deserialize_with = "super::nullable")]
    pub deploy_gas: Option<String>,
    #[serde(deserialize_with = "super::nullable")]
    pub deploy_time: Option<String>,
    pub seeded_usdc: String,
    /// `[address, balance]` pairs.
    pub non_circulating_user_balances: Vec<(String, String)>,
    pub future_emissions: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenGenesis {
    /// `[address, balance]` pairs.
    pub user_balances: Vec<(String, String)>,
    /// `[token index, balance]` pairs.
    pub existing_token_balances: Vec<(u32, String)>,
    pub blacklist_users: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutcomeSideSpec {
    pub name: String,
}

/// An outcome (`outcomeMeta.outcomes[]`, `settledOutcome.spec`, `outcomeCreated`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutcomeSpec {
    pub outcome: u64,
    pub name: String,
    /// `|`-separated `key:value` pairs; see the outcome-meta docs.
    pub description: String,
    pub side_specs: Vec<OutcomeSideSpec>,
    pub quote_token: String,
    /// Absent on legacy `Recurring` outcomes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub venue: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deployer_fee_scale: Option<String>,
}

/// A question grouping outcomes (`outcomeMeta.questions[]`, `questionUpdated`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuestionSpec {
    pub question: u64,
    pub name: String,
    pub description: String,
    pub fallback_outcome: u64,
    pub named_outcomes: Vec<u64>,
    pub settled_named_outcomes: Vec<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutcomeDeployer {
    pub deployer: String,
    pub venue: String,
    /// `[action, [addresses]]` pairs.
    pub sub_deployers: Vec<(String, Vec<String>)>,
}

/// `outcomeMeta`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutcomeMeta {
    pub outcomes: Vec<OutcomeSpec>,
    pub questions: Vec<QuestionSpec>,
    pub deployers: Vec<OutcomeDeployer>,
    pub fee_scale: String,
}

/// `outcomeTemplates[].role`: `{"standaloneOutcome": {..}}`, `"question"` or `{"questionOutcome": {..}}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OutcomeTemplateRole {
    StandaloneOutcome {
        #[serde(rename = "sideNames")]
        side_names: Vec<String>,
    },
    Question,
    QuestionOutcome {
        /// Parent question template id.
        parent: String,
    },
}

/// One `outcomeTemplates` entry. `name`/`description` contain `{keyword}` placeholders.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutcomeTemplate {
    pub id: String,
    pub role: OutcomeTemplateRole,
    pub name: String,
    pub description: String,
    /// `[keyword, kind]` pairs (kinds seen: `string`, `shortString`, `hlPerp`, `uInt`, `uDecimal`, `dateTime`).
    pub keywords: Vec<(String, String)>,
}

/// `settledOutcome`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettledOutcome {
    pub spec: OutcomeSpec,
    pub settle_fraction: String,
    pub details: String,
}
