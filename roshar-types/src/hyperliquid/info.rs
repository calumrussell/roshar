use serde::{Deserialize, Serialize};

// User order management data structures
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserOrder {
    pub asset: String,
    pub is_buy: bool,
    pub limit_px: String,
    pub sz: String,
    pub oid: u64,
    pub timestamp: u64,
    pub order_type: String,
    pub reduce_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FundingHistory {
    pub asset: String,
    pub time: u64,
    pub funding_rate: String,
    pub payment: String,
    pub position_size: String,
}

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
    pub token: u32,
    pub hold: String,
    pub total: String,
    #[serde(rename = "entryNtl")]
    pub entry_ntl: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpotClearinghouseState {
    pub balances: Vec<SpotBalance>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPerpetualsState {
    #[serde(rename = "assetPositions")]
    pub asset_positions: Vec<AssetPosition>,
    #[serde(rename = "crossMaintenanceMarginUsed")]
    pub cross_maintenance_margin_used: String,
    #[serde(rename = "crossMarginSummary")]
    pub cross_margin_summary: CrossMarginSummary,
    pub withdrawable: String,
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Asset {
    pub name: String,
    #[serde(rename = "szDecimals")]
    pub sz_decimals: u8,
    #[serde(rename = "maxLeverage")]
    pub max_leverage: u32,
    #[serde(rename = "onlyIsolated", skip_serializing_if = "Option::is_none")]
    pub only_isolated: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketData {
    #[serde(rename = "dayNtlVlm", skip_serializing_if = "Option::is_none")]
    pub day_notional_volume: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub funding: Option<String>,
    #[serde(rename = "impactPxs", skip_serializing_if = "Option::is_none")]
    pub impact_prices: Option<Vec<String>>,
    #[serde(rename = "markPx", skip_serializing_if = "Option::is_none")]
    pub mark_price: Option<String>,
    #[serde(rename = "midPx", skip_serializing_if = "Option::is_none")]
    pub mid_price: Option<String>,
    #[serde(rename = "openInterest", skip_serializing_if = "Option::is_none")]
    pub open_interest: Option<String>,
    #[serde(rename = "oraclePx", skip_serializing_if = "Option::is_none")]
    pub oracle_price: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub premium: Option<String>,
    #[serde(rename = "prevDayPx", skip_serializing_if = "Option::is_none")]
    pub prev_day_price: Option<String>,
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
    #[serde(rename = "evmContract")]
    pub evm_contract: Option<EvmContract>,
    #[serde(rename = "fullName")]
    pub full_name: Option<String>,
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
    #[serde(rename = "midPx")]
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

    #[test]
    fn test_asset_meta_ctx_deserialisation() {
        let json = r#"[
            {
                "universe": [
                    {
                        "name": "BTC",
                        "szDecimals": 5,
                        "maxLeverage": 50
                    },
                    {
                        "name": "ETH",
                        "szDecimals": 4,
                        "maxLeverage": 50
                    },
                    {
                        "name": "HPOS",
                        "szDecimals": 0,
                        "maxLeverage": 3,
                        "onlyIsolated": true
                    }
                ]
            },
            [
                {
                    "dayNtlVlm":"1169046.29406",
                    "funding":"0.0000125",
                    "impactPxs":[
                        "14.3047",
                        "14.3444"
                    ],
                    "markPx":"14.3161",
                    "midPx":"14.314",
                    "openInterest":"688.11",
                    "oraclePx":"14.32",
                    "premium":"0.00031774",
                    "prevDayPx":"15.322"
                }
            ]
        ]"#;

        let response: MetaAndAssetCtxs = serde_json::from_str(json).unwrap();
        assert!(response.0.universe.len() == 3);
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
                        "fullName": null
                    },
                    {
                        "name": "PURR",
                        "szDecimals": 0,
                        "weiDecimals": 5,
                        "index": 1,
                        "tokenId": "0xc1fb593aeffbeb02f85e0308e9956a90",
                        "isCanonical": true,
                        "evmContract": null,
                        "fullName": null
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
}
