#![allow(dead_code)]

use std::collections::HashMap;

use anyhow::{Context, Result};
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

pub struct InfoApi {
    base_url: String,
}

impl InfoApi {
    pub fn new(base_url: Option<String>) -> Self {
        Self {
            base_url: base_url.unwrap_or_else(|| "https://api.hyperliquid.xyz".to_string()),
        }
    }

    pub fn production() -> Self {
        Self::new(None)
    }

    pub fn testnet() -> Self {
        Self::new(Some("https://api.hyperliquid-testnet.xyz".to_string()))
    }

    pub async fn get_spot_meta_and_asset_ctxs(&self) -> Result<SpotMetaAndAssetCtxs> {
        let client = crate::http::get_http_client();
        let url = format!("{}/info", self.base_url);

        let response = client
            .post(url)
            .json(&InfoApiRequest {
                typ: "spotMetaAndAssetCtxs".to_string(),
            })
            .send()
            .await?;

        let meta: SpotMetaAndAssetCtxs = response.json().await?;
        Ok(meta)
    }

    pub async fn get_info_spot(&self) -> Result<HashMap<String, SpotMarketData>> {
        let meta = self.get_spot_meta_and_asset_ctxs().await?;

        // Build HashMap of coin name -> asset context for O(1) lookup
        let asset_ctx_map: HashMap<String, &SpotMarketData> =
            meta.1.iter().map(|ctx| (ctx.coin.clone(), ctx)).collect();

        let mut res = HashMap::new();
        for universe_coin in &meta.0.universe {
            // Find the matching asset context by coin name
            if let Some(asset_ctx) = asset_ctx_map.get(&universe_coin.name) {
                let mut spot_data = (*asset_ctx).clone();
                spot_data.tokens = universe_coin.tokens.clone();
                res.insert(universe_coin.name.clone(), spot_data);
            }
        }
        Ok(res)
    }

    pub async fn get_info(&self) -> Result<HashMap<String, AssetInfo>> {
        let client = crate::http::get_http_client();
        let url = format!("{}/info", self.base_url);

        let response = client
            .post(url)
            .json(&InfoApiRequest {
                typ: "metaAndAssetCtxs".to_string(),
            })
            .send()
            .await?;

        let meta: MetaAndAssetCtxs = response.json().await?;

        let mut res = HashMap::new();
        for (i, universe_coin) in meta.0.universe.iter().enumerate() {
            if let Some(asset_ctx) = meta.1.get(i) {
                let asset_info = AssetInfo {
                    asset: universe_coin.clone(),
                    market_data: asset_ctx.clone(),
                };
                res.insert(universe_coin.name.clone(), asset_info);
            }
        }
        Ok(res)
    }

    pub async fn get_all_funding_rates_with_size(&self) -> Result<Vec<(String, f64, f64, f64)>> {
        let client = crate::http::get_http_client();
        let url = format!("{}/info", self.base_url);

        let response = client
            .post(url)
            .json(&InfoApiRequest {
                typ: "metaAndAssetCtxs".to_string(),
            })
            .send()
            .await?;

        let meta: MetaAndAssetCtxs = response.json().await?;

        let mut funding_rates = Vec::new();
        for (i, universe_coin) in meta.0.universe.iter().enumerate() {
            if let Some(asset_ctx) = meta.1.get(i) {
                let rate = asset_ctx
                    .funding
                    .as_ref()
                    .and_then(|s| s.parse::<f64>().ok())
                    .unwrap_or(0.0);

                let open_interest_in_base = asset_ctx
                    .open_interest
                    .as_ref()
                    .and_then(|s| s.parse::<f64>().ok())
                    .unwrap_or(0.0);

                let open_interest = if open_interest_in_base == 0.0 {
                    0.0
                } else {
                    open_interest_in_base
                        * asset_ctx
                            .mark_price
                            .as_ref()
                            .and_then(|s| s.parse::<f64>().ok())
                            .unwrap_or(0.0)
                };

                let daily_volume = asset_ctx
                    .day_notional_volume
                    .as_ref()
                    .and_then(|s| s.parse::<f64>().ok())
                    .unwrap_or(0.0);

                funding_rates.push((
                    universe_coin.name.clone(),
                    rate,
                    open_interest,
                    daily_volume,
                ));
            }
        }

        Ok(funding_rates)
    }

    pub async fn get_user_orders(&self, user_address: &str) -> Result<Vec<UserOrder>> {
        let client = crate::http::get_http_client();
        let url = format!("{}/info", self.base_url);

        let request_body = serde_json::json!({
            "type": "openOrders",
            "user": user_address
        });

        let response = client
            .post(url)
            .json(&request_body)
            .send()
            .await
            .context("Failed to request user orders")?;

        if !response.status().is_success() {
            anyhow::bail!("Info API request failed with status: {}", response.status());
        }

        let orders_data: serde_json::Value = response
            .json()
            .await
            .context("Failed to parse orders response")?;
        let mut user_orders = Vec::new();
        if let Some(orders) = orders_data.as_array() {
            for order in orders {
                if let Ok(user_order) = serde_json::from_value::<UserOrder>(order.clone()) {
                    user_orders.push(user_order);
                } else if let Some(obj) = order.as_object() {
                    let user_order = UserOrder {
                        asset: obj
                            .get("coin")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        is_buy: obj.get("side").and_then(|v| v.as_str()).unwrap_or("") == "B",
                        limit_px: obj
                            .get("limitPx")
                            .and_then(|v| v.as_str())
                            .unwrap_or("0")
                            .to_string(),
                        sz: obj
                            .get("sz")
                            .and_then(|v| v.as_str())
                            .unwrap_or("0")
                            .to_string(),
                        oid: obj.get("oid").and_then(|v| v.as_u64()).unwrap_or(0),
                        timestamp: obj.get("timestamp").and_then(|v| v.as_u64()).unwrap_or(0),
                        order_type: "limit".to_string(),
                        reduce_only: false,
                    };
                    user_orders.push(user_order);
                }
            }
        }

        Ok(user_orders)
    }

    pub async fn get_funding_history(&self, user_address: &str) -> Result<Vec<FundingHistory>> {
        let client = crate::http::get_http_client();
        let url = format!("{}/info", self.base_url);

        let request_body = serde_json::json!({
            "type": "userFunding",
            "user": user_address,
            "startTime": 0
        });

        let response = client
            .post(url)
            .json(&request_body)
            .send()
            .await
            .context("Failed to request funding history")?;

        if !response.status().is_success() {
            log::warn!(
                "Funding history endpoint not available (status: {}). This feature may not be implemented in the Hyperliquid API yet.",
                response.status()
            );
            return Ok(vec![]);
        }

        let funding_data: serde_json::Value = response
            .json()
            .await
            .context("Failed to parse funding response")?;
        let mut funding_history = Vec::new();
        if let Some(fundings) = funding_data.as_array() {
            for funding in fundings {
                if let Some(obj) = funding.as_object() {
                    let funding_entry = FundingHistory {
                        asset: obj
                            .get("asset")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        time: obj.get("time").and_then(|v| v.as_u64()).unwrap_or(0),
                        funding_rate: obj
                            .get("fundingRate")
                            .and_then(|v| v.as_str())
                            .unwrap_or("0")
                            .to_string(),
                        payment: obj
                            .get("payment")
                            .and_then(|v| v.as_str())
                            .unwrap_or("0")
                            .to_string(),
                        position_size: obj
                            .get("positionSize")
                            .and_then(|v| v.as_str())
                            .unwrap_or("0")
                            .to_string(),
                    };
                    funding_history.push(funding_entry);
                }
            }
        }

        Ok(funding_history)
    }

    pub async fn get_historical_funding_rates(
        &self,
        coin: &str,
        start_time: u64,
        end_time: Option<u64>,
    ) -> Result<Vec<HistoricalFundingRate>> {
        let client = crate::http::get_http_client();
        let url = format!("{}/info", self.base_url);

        let mut request_body = serde_json::json!({
            "type": "fundingHistory",
            "coin": coin,
            "startTime": start_time
        });

        if let Some(end_time) = end_time {
            request_body["endTime"] = serde_json::Value::Number(serde_json::Number::from(end_time));
        }

        let response = client
            .post(url)
            .json(&request_body)
            .send()
            .await
            .context("Failed to request historical funding rates")?;

        if !response.status().is_success() {
            anyhow::bail!(
                "Historical funding rates endpoint failed (status: {})",
                response.status()
            );
        }

        let funding_rates: Vec<HistoricalFundingRate> = response
            .json()
            .await
            .context("Failed to parse historical funding rates response")?;

        Ok(funding_rates)
    }

    pub async fn user_perpetuals_account_summary(
        &self,
        user_address: &str,
    ) -> Result<UserPerpetualsState> {
        let client = crate::http::get_http_client();
        let url = format!("{}/info", self.base_url);

        let request_body = serde_json::json!({
            "type": "clearinghouseState",
            "user": user_address
        });

        let response = client
            .post(url)
            .json(&request_body)
            .send()
            .await
            .context("Failed to request user state")?;

        if !response.status().is_success() {
            anyhow::bail!(
                "User state API request failed with status: {}",
                response.status()
            );
        }

        let user_state: UserPerpetualsState = response
            .json()
            .await
            .context("Failed to parse user state response")?;

        Ok(user_state)
    }

    pub async fn user_spot_state(&self, user_address: &str) -> Result<SpotClearinghouseState> {
        let client = crate::http::get_http_client();
        let url = format!("{}/info", self.base_url);

        let request_body = serde_json::json!({
            "type": "spotClearinghouseState",
            "user": user_address
        });

        let response = client
            .post(url)
            .json(&request_body)
            .send()
            .await
            .context("Failed to request spot user state")?;

        if !response.status().is_success() {
            anyhow::bail!(
                "Spot user state API request failed with status: {}",
                response.status()
            );
        }

        let spot_state: SpotClearinghouseState = response
            .json()
            .await
            .context("Failed to parse spot user state response")?;

        Ok(spot_state)
    }

    pub async fn meta(&self) -> Result<serde_json::Value> {
        let client = crate::http::get_http_client();
        let url = format!("{}/info", self.base_url);

        let request_body = serde_json::json!({
            "type": "meta"
        });

        let response = client
            .post(url)
            .json(&request_body)
            .send()
            .await
            .context("Failed to request meta info")?;

        if !response.status().is_success() {
            anyhow::bail!("Meta API request failed with status: {}", response.status());
        }

        let meta_data: serde_json::Value = response
            .json()
            .await
            .context("Failed to parse meta response")?;

        Ok(meta_data)
    }

    /// Get open orders for a wallet address
    ///
    /// # Arguments
    /// * `wallet_address` - The Ethereum wallet address (H160) to query orders for
    ///
    /// # Returns
    /// A vector of UserOrder structs representing all open orders for the wallet
    pub async fn open_orders(&self, wallet_address: ethers::types::H160) -> Result<Vec<UserOrder>> {
        let address_str = format!("{:#x}", wallet_address);
        self.get_user_orders(&address_str).await
    }

    /// Get perpetuals account state for a wallet address
    ///
    /// # Arguments
    /// * `wallet_address` - The Ethereum wallet address (H160) to query state for
    ///
    /// # Returns
    /// UserPerpetualsState containing positions, margin, and account information
    pub async fn user_perpetuals_state(
        &self,
        wallet_address: ethers::types::H160,
    ) -> Result<UserPerpetualsState> {
        let address_str = format!("{:#x}", wallet_address);
        self.user_perpetuals_account_summary(&address_str).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_all_funding_rates_with_size() {
        let info_api = InfoApi::production();
        let result = info_api.get_all_funding_rates_with_size().await;

        assert!(
            result.is_ok(),
            "Failed to get funding rates: {:?}",
            result.err()
        );

        let funding_rates = result.unwrap();
        assert!(
            !funding_rates.is_empty(),
            "Funding rates should not be empty"
        );

        // Check structure of first entry
        if let Some((symbol, rate, open_interest, volume)) = funding_rates.first() {
            assert!(!symbol.is_empty(), "Symbol should not be empty");
            assert!(rate.is_finite(), "Funding rate should be a finite number");
            assert!(
                open_interest >= &0.0,
                "Open interest should be non-negative"
            );
            assert!(volume >= &0.0, "Volume should be non-negative");
        }
    }

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

    #[tokio::test]
    #[ignore = "requires testnet access"]
    async fn test_get_user_orders() {
        let user_address = std::env::var("HYPERLIQUID_WALLET_ADDRESS")
            .expect("HYPERLIQUID_WALLET_ADDRESS environment variable not set");

        let info_api = InfoApi::testnet();
        let result = info_api.get_user_orders(&user_address).await;

        assert!(
            result.is_ok(),
            "Failed to get user orders: {:?}",
            result.err()
        );

        let user_orders = result.unwrap();
        for order in &user_orders {
            assert!(!order.asset.is_empty(), "Asset should not be empty");
            assert!(
                !order.limit_px.is_empty(),
                "Limit price should not be empty"
            );
            assert!(!order.sz.is_empty(), "Size should not be empty");
            assert!(order.oid > 0, "Order ID should be positive");
            assert!(order.timestamp > 0, "Timestamp should be positive");
            assert!(
                !order.order_type.is_empty(),
                "Order type should not be empty"
            );
        }
    }

    #[tokio::test]
    #[ignore = "requires testnet access"]
    async fn test_get_funding_history() {
        let user_address = std::env::var("HYPERLIQUID_WALLET_ADDRESS")
            .expect("HYPERLIQUID_WALLET_ADDRESS environment variable not set");

        let info_api = InfoApi::testnet();
        let result = info_api.get_funding_history(&user_address).await;

        assert!(
            result.is_ok(),
            "Failed to get funding history: {:?}",
            result.err()
        );

        let funding_history = result.unwrap();
        for entry in &funding_history {
            assert!(!entry.asset.is_empty(), "Asset should not be empty");
            assert!(entry.time > 0, "Time should be positive");
            assert!(
                !entry.funding_rate.is_empty(),
                "Funding rate should not be empty"
            );
            assert!(!entry.payment.is_empty(), "Payment should not be empty");
            assert!(
                !entry.position_size.is_empty(),
                "Position size should not be empty"
            );
        }
    }

    #[tokio::test]
    #[ignore = "requires testnet access"]
    async fn test_open_orders() {
        let user_address_str = std::env::var("HYPERLIQUID_WALLET_ADDRESS")
            .expect("HYPERLIQUID_WALLET_ADDRESS environment variable not set");

        // Parse the string address into H160
        let wallet_address: ethers::types::H160 = user_address_str
            .parse()
            .expect("Invalid wallet address format");

        let info_api = InfoApi::testnet();
        let result = info_api.open_orders(wallet_address).await;

        assert!(
            result.is_ok(),
            "Failed to get open orders: {:?}",
            result.err()
        );

        let orders = result.unwrap();
        for order in &orders {
            assert!(!order.asset.is_empty(), "Asset should not be empty");
            assert!(
                !order.limit_px.is_empty(),
                "Limit price should not be empty"
            );
            assert!(!order.sz.is_empty(), "Size should not be empty");
            assert!(order.oid > 0, "Order ID should be positive");
            assert!(order.timestamp > 0, "Timestamp should be positive");
            assert!(
                !order.order_type.is_empty(),
                "Order type should not be empty"
            );
        }
    }

    #[tokio::test]
    #[ignore = "requires testnet access"]
    async fn test_user_perpetuals_state() {
        let user_address_str = std::env::var("HYPERLIQUID_WALLET_ADDRESS")
            .expect("HYPERLIQUID_WALLET_ADDRESS environment variable not set");

        // Parse the string address into H160
        let wallet_address: ethers::types::H160 = user_address_str
            .parse()
            .expect("Invalid wallet address format");

        let info_api = InfoApi::testnet();
        let result = info_api.user_perpetuals_state(wallet_address).await;

        assert!(
            result.is_ok(),
            "Failed to get user perpetuals state: {:?}",
            result.err()
        );

        let state = result.unwrap();
        assert!(
            !state.withdrawable.is_empty(),
            "Withdrawable should not be empty"
        );
        assert!(
            !state.cross_maintenance_margin_used.is_empty(),
            "Cross maintenance margin used should not be empty"
        );
    }

    #[test]
    fn test_h160_address_formatting() {
        // Test that H160 address formatting works correctly
        let test_address: ethers::types::H160 = "0x1234567890123456789012345678901234567890"
            .parse()
            .unwrap();

        let formatted = format!("{:#x}", test_address);
        assert_eq!(formatted, "0x1234567890123456789012345678901234567890");
    }
}
