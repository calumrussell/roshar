use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoricalFundingRate {
    pub coin: String,
    #[serde(rename = "fundingRate")]
    pub funding_rate: String,
    pub premium: String,
    pub time: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpotBalance {
    pub coin: String,
    #[serde(default)]
    pub token: Option<u32>,
    pub hold: String,
    pub total: String,
    #[serde(rename = "entryNtl")]
    pub entry_ntl: String,
}

impl SpotBalance {
    /// Converts outcome token notation (`+<encoding>`) into outcome order coin notation (`#<encoding>`).
    /// Returns the original value for non-outcome coins.
    pub fn normalized_order_coin(&self) -> String {
        normalize_outcome_token_to_order_coin(&self.coin)
    }
}

/// Converts outcome token notation (`+<encoding>`) into outcome order coin notation (`#<encoding>`).
/// Returns the input unchanged for non-outcome coins.
pub fn normalize_outcome_token_to_order_coin(coin: &str) -> String {
    if let Some(rest) = coin.strip_prefix('+') {
        format!("#{rest}")
    } else {
        coin.to_string()
    }
}

/// Converts outcome order coin notation (`#<encoding>`) into outcome token notation (`+<encoding>`).
/// Returns the input unchanged for non-outcome coins.
pub fn normalize_outcome_order_coin_to_token(coin: &str) -> String {
    if let Some(rest) = coin.strip_prefix('#') {
        format!("+{rest}")
    } else {
        coin.to_string()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpotClearinghouseState {
    pub balances: Vec<SpotBalance>,
}

/// `clearinghouseState` response (one perp dex).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPerpetualsState {
    #[serde(rename = "assetPositions")]
    pub asset_positions: Vec<AssetPosition>,
    #[serde(rename = "crossMaintenanceMarginUsed")]
    pub cross_maintenance_margin_used: String,
    #[serde(rename = "crossMarginSummary")]
    pub cross_margin_summary: CrossMarginSummary,
    /// Whole-account summary (cross and isolated).
    #[serde(rename = "marginSummary")]
    pub margin_summary: CrossMarginSummary,
    pub withdrawable: String,
    /// Server time (ms).
    pub time: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetPosition {
    pub position: Position,
    #[serde(rename = "type")]
    pub position_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub coin: String,
    #[serde(rename = "entryPx")]
    pub entry_px: Option<String>,
    pub leverage: Leverage,
    #[serde(rename = "liquidationPx")]
    pub liquidation_px: Option<String>,
    #[serde(rename = "marginUsed")]
    pub margin_used: String,
    #[serde(rename = "maxLeverage")]
    pub max_leverage: u32,
    #[serde(rename = "positionValue")]
    pub position_value: String,
    #[serde(rename = "returnOnEquity")]
    pub return_on_equity: String,
    pub szi: String,
    #[serde(rename = "unrealizedPnl")]
    pub unrealized_pnl: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Leverage {
    #[serde(rename = "type")]
    pub leverage_type: String,
    pub value: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossMarginSummary {
    #[serde(rename = "accountValue")]
    pub account_value: String,
    #[serde(rename = "totalMarginUsed")]
    pub total_margin_used: String,
    #[serde(rename = "totalNtlPos")]
    pub total_ntl_pos: String,
    #[serde(rename = "totalRawUsd")]
    pub total_raw_usd: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct InfoApiRequest {
    #[serde(rename = "type")]
    pub typ: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MetaAndAssetCtxs(pub Universe, pub Vec<MarketData>);

#[derive(Debug, Serialize, Deserialize)]
pub struct Universe {
    pub universe: Vec<Asset>,
    /// Spot token index of the dex's collateral (0 = USDC).
    #[serde(rename = "collateralToken")]
    pub collateral_token: u32,
    /// `[marginTableId, table]` pairs. HIP-3 dexes may omit tables their assets reference.
    #[serde(rename = "marginTables")]
    pub margin_tables: Vec<(u32, MarginTable)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarginTable {
    pub description: String,
    #[serde(rename = "marginTiers")]
    pub margin_tiers: Vec<MarginTier>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarginTier {
    #[serde(rename = "lowerBound")]
    pub lower_bound: String,
    #[serde(rename = "maxLeverage")]
    pub max_leverage: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Asset {
    /// HIP-3 assets are named `{dex}:{coin}`.
    pub name: String,
    #[serde(rename = "szDecimals")]
    pub sz_decimals: u8,
    #[serde(rename = "maxLeverage")]
    pub max_leverage: u32,
    #[serde(rename = "marginTableId")]
    pub margin_table_id: u32,
    /// Absent unless true. Deprecated by the exchange in favour of `marginMode`.
    #[serde(rename = "onlyIsolated", skip_serializing_if = "Option::is_none")]
    pub only_isolated: Option<bool>,
    /// `"strictIsolated"` or `"noCross"`; absent for cross-capable assets.
    #[serde(rename = "marginMode", skip_serializing_if = "Option::is_none")]
    pub margin_mode: Option<String>,
    /// Absent unless true.
    #[serde(rename = "isDelisted", skip_serializing_if = "Option::is_none")]
    pub is_delisted: Option<bool>,
    /// HIP-3 only (e.g. `"enabled"`); absent otherwise.
    #[serde(rename = "growthMode", skip_serializing_if = "Option::is_none")]
    pub growth_mode: Option<String>,
    /// HIP-3 only (documented; not seen live on 2026-11-16).
    #[serde(
        rename = "lastGrowthModeChangeTime",
        skip_serializing_if = "Option::is_none"
    )]
    pub last_growth_mode_change_time: Option<String>,
    /// HIP-3 only.
    #[serde(
        rename = "lastFeeScaleChangeTime",
        skip_serializing_if = "Option::is_none"
    )]
    pub last_fee_scale_change_time: Option<String>,
    /// HIP-3 only.
    #[serde(rename = "deployerFeeScale", skip_serializing_if = "Option::is_none")]
    pub deployer_fee_scale: Option<String>,
}

/// Perp asset context. `impactPxs`, `midPx` and `premium` are null when the book is empty (e.g. delisted).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketData {
    #[serde(rename = "dayNtlVlm")]
    pub day_notional_volume: String,
    #[serde(rename = "dayBaseVlm")]
    pub day_base_volume: String,
    pub funding: String,
    #[serde(rename = "impactPxs", deserialize_with = "super::nullable")]
    pub impact_prices: Option<Vec<String>>,
    #[serde(rename = "markPx")]
    pub mark_price: String,
    #[serde(rename = "midPx", deserialize_with = "super::nullable")]
    pub mid_price: Option<String>,
    #[serde(rename = "openInterest")]
    pub open_interest: String,
    #[serde(rename = "oraclePx")]
    pub oracle_price: String,
    #[serde(deserialize_with = "super::nullable")]
    pub premium: Option<String>,
    #[serde(rename = "prevDayPx")]
    pub prev_day_price: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetInfo {
    pub asset: Asset,
    pub market_data: MarketData,
}

// Spot-specific structures
#[derive(Debug, Serialize, Deserialize)]
pub struct SpotMetaAndAssetCtxs(pub SpotMeta, pub Vec<SpotMarketData>);

#[derive(Debug, Serialize, Deserialize)]
pub struct SpotMeta {
    pub tokens: Vec<SpotToken>,
    pub universe: Vec<SpotAsset>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpotToken {
    pub name: String,
    #[serde(rename = "szDecimals")]
    pub sz_decimals: u8,
    #[serde(rename = "weiDecimals")]
    pub wei_decimals: u8,
    pub index: u32,
    #[serde(rename = "tokenId")]
    pub token_id: String,
    #[serde(rename = "isCanonical")]
    pub is_canonical: bool,
    #[serde(rename = "evmContract", deserialize_with = "super::nullable")]
    pub evm_contract: Option<EvmContract>,
    #[serde(rename = "fullName", deserialize_with = "super::nullable")]
    pub full_name: Option<String>,
    #[serde(rename = "deployerTradingFeeShare")]
    pub deployer_trading_fee_share: String,
    /// Absent for most tokens.
    #[serde(rename = "deployerLabel", skip_serializing_if = "Option::is_none")]
    pub deployer_label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvmContract {
    pub address: String,
    pub evm_extra_wei_decimals: i8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpotAsset {
    pub name: String,
    pub tokens: Vec<u32>,
    pub index: u32,
    #[serde(rename = "isCanonical")]
    pub is_canonical: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpotMarketData {
    #[serde(rename = "dayNtlVlm")]
    pub day_notional_volume: String,
    #[serde(rename = "markPx")]
    pub mark_price: String,
    #[serde(rename = "midPx", deserialize_with = "super::nullable")]
    pub mid_price: Option<String>,
    #[serde(rename = "prevDayPx")]
    pub prev_day_price: String,
    #[serde(rename = "circulatingSupply")]
    pub circulating_supply: String,
    pub coin: String,
    #[serde(rename = "totalSupply")]
    pub total_supply: String,
    #[serde(rename = "dayBaseVlm")]
    pub day_base_volume: String,
    #[serde(skip)]
    pub tokens: Vec<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Shapes as returned live by `metaAndAssetCtxs` (first perp dex and `dex: "xyz"`), 2026-11-15.
    #[test]
    fn test_asset_meta_ctx_deserialisation() {
        let json = r#"[
            {
                "universe": [
                    {"szDecimals": 5, "name": "BTC", "maxLeverage": 40, "marginTableId": 56},
                    {"szDecimals": 1, "name": "LOOM", "maxLeverage": 3, "marginTableId": 3,
                     "isDelisted": true, "onlyIsolated": true, "marginMode": "strictIsolated"},
                    {"szDecimals": 4, "name": "xyz:XYZ100", "maxLeverage": 30, "marginTableId": 30,
                     "growthMode": "enabled", "deployerFeeScale": "1.0"}
                ],
                "marginTables": [[56, {"description": "", "marginTiers": [{"lowerBound": "0.0", "maxLeverage": 40}]}]],
                "collateralToken": 0
            },
            [
                {"funding": "0.0000125", "openInterest": "688.11", "prevDayPx": "15.322", "dayNtlVlm": "1169046.29406",
                 "premium": "0.00031774", "oraclePx": "14.32", "markPx": "14.3161", "midPx": "14.314",
                 "impactPxs": ["14.3047", "14.3444"], "dayBaseVlm": "81234.1"},
                {"funding": "0.0", "openInterest": "0.0", "prevDayPx": "0.1", "dayNtlVlm": "0.0",
                 "premium": null, "oraclePx": "0.1", "markPx": "0.1", "midPx": null,
                 "impactPxs": null, "dayBaseVlm": "0.0"},
                {"funding": "0.00000625", "openInterest": "0.0854", "prevDayPx": "25956.0", "dayNtlVlm": "462.9758",
                 "premium": "0.0031136686", "oraclePx": "25372.0", "markPx": "25451.0", "midPx": "25451.0",
                 "impactPxs": ["24946.0", "25956.0"], "dayBaseVlm": "0.0183"}
            ]
        ]"#;

        let response: MetaAndAssetCtxs = serde_json::from_str(json).unwrap();
        assert_eq!(response.0.universe.len(), 3);
        assert_eq!(response.0.collateral_token, 0);
        assert_eq!(response.0.universe[1].is_delisted, Some(true));
        assert_eq!(
            response.0.universe[1].margin_mode.as_deref(),
            Some("strictIsolated")
        );
        assert_eq!(response.0.universe[0].is_delisted, None);
        assert_eq!(response.1[1].mid_price, None);
        assert_eq!(response.1[2].day_base_volume, "0.0183");
    }

    #[test]
    fn asset_ctx_missing_required_field_is_an_error() {
        let json = r#"{"funding": "0.0", "openInterest": "0.0", "prevDayPx": "0.1", "dayNtlVlm": "0.0",
                       "premium": null, "oraclePx": "0.1", "midPx": null, "impactPxs": null, "dayBaseVlm": "0.0"}"#;
        assert!(
            serde_json::from_str::<MarketData>(json).is_err(),
            "markPx is required"
        );
    }

    #[test]
    fn test_spot_asset_meta_ctx_deserialisation() {
        let json = r#"[
            {
                "tokens": [
                    {
                        "name": "USDC",
                        "szDecimals": 8,
                        "weiDecimals": 8,
                        "index": 0,
                        "tokenId": "0x6d1e7cde53ba9467b783cb7c530ce054",
                        "isCanonical": true,
                        "evmContract": null,
                        "fullName": null,
                        "deployerTradingFeeShare": "0.0"
                    },
                    {
                        "name": "PURR",
                        "szDecimals": 0,
                        "weiDecimals": 5,
                        "index": 1,
                        "tokenId": "0xc1fb593aeffbeb02f85e0308e9956a90",
                        "isCanonical": true,
                        "evmContract": null,
                        "fullName": null,
                        "deployerTradingFeeShare": "0.0"
                    }
                ],
                "universe": [
                    {
                        "name": "PURR/USDC",
                        "tokens": [1, 0],
                        "index": 0,
                        "isCanonical": true
                    }
                ]
            },
            [
                {
                    "prevDayPx": "0.19752",
                    "dayNtlVlm": "6223041.469820004",
                    "markPx": "0.19537",
                    "midPx": "0.19535",
                    "circulatingSupply": "596541406.8187199831",
                    "coin": "PURR/USDC",
                    "totalSupply": "596541413.3415000439",
                    "dayBaseVlm": "32247437.0"
                }
            ]
        ]"#;

        let response: SpotMetaAndAssetCtxs = serde_json::from_str(json).unwrap();
        assert_eq!(response.0.tokens.len(), 2);
        assert_eq!(response.0.universe.len(), 1);
        assert_eq!(response.1.len(), 1);

        // Test token parsing
        let usdc_token = &response.0.tokens[0];
        assert_eq!(usdc_token.name, "USDC");
        assert_eq!(usdc_token.sz_decimals, 8);
        assert_eq!(usdc_token.wei_decimals, 8);
        assert_eq!(usdc_token.index, 0);
        assert_eq!(usdc_token.token_id, "0x6d1e7cde53ba9467b783cb7c530ce054");
        assert!(usdc_token.is_canonical);
        assert_eq!(usdc_token.evm_contract, None);
        assert_eq!(usdc_token.full_name, None);

        // Test universe parsing
        let purr_usdc = &response.0.universe[0];
        assert_eq!(purr_usdc.name, "PURR/USDC");
        assert_eq!(purr_usdc.tokens, vec![1, 0]);
        assert_eq!(purr_usdc.index, 0);
        assert!(purr_usdc.is_canonical);

        // Test market data parsing
        let market_data = &response.1[0];
        assert_eq!(market_data.day_notional_volume, "6223041.469820004");
        assert_eq!(market_data.mark_price, "0.19537");
        assert_eq!(market_data.mid_price, Some("0.19535".to_string()));
        assert_eq!(market_data.prev_day_price, "0.19752");
        assert_eq!(market_data.circulating_supply, "596541406.8187199831");
        assert_eq!(market_data.coin, "PURR/USDC");
        assert_eq!(market_data.total_supply, "596541413.3415000439");
        assert_eq!(market_data.day_base_volume, "32247437.0");
    }

    #[test]
    fn test_spot_clearinghouse_state_deserialises_with_optional_token() {
        let json = r#"{
            "balances": [
                {"coin":"USDC","token":0,"total":"0.0000001","hold":"0.0","entryNtl":"0.0"},
                {"coin":"+0","total":"187.0","hold":"0.0","entryNtl":"121.03849263"},
                {"coin":"+1","total":"1.0","hold":"0.0","entryNtl":"0.35261391"}
            ]
        }"#;

        let state: SpotClearinghouseState = serde_json::from_str(json).unwrap();
        assert_eq!(state.balances.len(), 3);
        assert_eq!(state.balances[0].coin, "USDC");
        assert_eq!(state.balances[0].token, Some(0));
        assert_eq!(state.balances[1].coin, "+0");
        assert_eq!(state.balances[1].token, None);
        assert_eq!(state.balances[2].coin, "+1");
        assert_eq!(state.balances[2].token, None);
    }

    #[test]
    fn test_outcome_coin_normalization_helpers() {
        assert_eq!(normalize_outcome_token_to_order_coin("+1"), "#1");
        assert_eq!(normalize_outcome_token_to_order_coin("USDC"), "USDC");
        assert_eq!(normalize_outcome_order_coin_to_token("#1"), "+1");
        assert_eq!(normalize_outcome_order_coin_to_token("USDC"), "USDC");
    }
}
