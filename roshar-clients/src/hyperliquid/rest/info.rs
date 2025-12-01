#![allow(dead_code)]

use std::collections::HashMap;

use anyhow::{Context, Result};

use roshar_types::{
    AssetInfo, FundingHistory, HistoricalFundingRate, InfoApiRequest, MetaAndAssetCtxs,
    SpotClearinghouseState, SpotMarketData, SpotMetaAndAssetCtxs, UserOrder, UserPerpetualsState,
};

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
