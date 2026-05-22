#![allow(dead_code)]

use std::collections::HashMap;

use anyhow::{Context, Result};

use crate::http::RateLimitedClient;
use roshar_types::{
    AssetInfo, FundingHistory, HistoricalFundingRate, InfoApiRequest, MetaAndAssetCtxs,
    SpotClearinghouseState, SpotMarketData, SpotMetaAndAssetCtxs, UserOrder, UserPerpetualsState,
};

/// POST a JSON body to `{base_url}/info`, parse the response as `T`.
/// Logs the outgoing request and incoming response at `debug`, and on non-2xx
/// emits an `error!` log carrying both response body and outgoing request,
/// then bails with the same context.
async fn post_info_json<B, T>(
    client: &RateLimitedClient,
    base_url: &str,
    body: &B,
    label: &str,
) -> Result<T>
where
    B: serde::Serialize,
    T: serde::de::DeserializeOwned,
{
    let url = format!("{}/info", base_url);
    let body_str = serde_json::to_string(body)
        .with_context(|| format!("Failed to serialize {label} request"))?;
    log::debug!("POST {url} ({label}) body={body_str}");

    let response = client
        .post(&url)
        .await
        .header("Content-Type", "application/json")
        .body(body_str.clone())
        .send()
        .await
        .with_context(|| format!("Failed to send {label} request"))?;

    let status = response.status();
    let text = response
        .text()
        .await
        .with_context(|| format!("Failed to read {label} response body"))?;

    if !status.is_success() {
        log::error!(
            "{label} request failed: status={status} body={text} request={body_str}"
        );
        anyhow::bail!("{label} request failed with status {status}: {text}");
    }

    log::debug!("{label} response: status={status} body={text}");

    serde_json::from_str(&text)
        .with_context(|| format!("Failed to parse {label} response (body={text})"))
}

pub struct InfoApi {
    base_url: String,
    client: std::sync::Arc<RateLimitedClient>,
}

impl InfoApi {
    /// Create a new InfoApi with a shared rate-limited client.
    /// The client should be shared across all Hyperliquid API calls to ensure
    /// rate limiting is coordinated.
    pub fn new_with_client(
        base_url: Option<String>,
        client: std::sync::Arc<RateLimitedClient>,
    ) -> Self {
        Self {
            base_url: base_url.unwrap_or_else(|| "https://api.hyperliquid.xyz".to_string()),
            client,
        }
    }

    pub fn production_with_client(client: std::sync::Arc<RateLimitedClient>) -> Self {
        Self::new_with_client(None, client)
    }

    pub fn testnet_with_client(client: std::sync::Arc<RateLimitedClient>) -> Self {
        Self::new_with_client(
            Some("https://api.hyperliquid-testnet.xyz".to_string()),
            client,
        )
    }

    pub async fn get_spot_meta_and_asset_ctxs(&self) -> Result<SpotMetaAndAssetCtxs> {
        post_info_json(
            &self.client,
            &self.base_url,
            &InfoApiRequest {
                typ: "spotMetaAndAssetCtxs".to_string(),
            },
            "spotMetaAndAssetCtxs",
        )
        .await
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
        let meta: MetaAndAssetCtxs = post_info_json(
            &self.client,
            &self.base_url,
            &InfoApiRequest {
                typ: "metaAndAssetCtxs".to_string(),
            },
            "metaAndAssetCtxs",
        )
        .await?;

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
        let meta: MetaAndAssetCtxs = post_info_json(
            &self.client,
            &self.base_url,
            &InfoApiRequest {
                typ: "metaAndAssetCtxs".to_string(),
            },
            "metaAndAssetCtxs",
        )
        .await?;

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
        let request_body = serde_json::json!({
            "type": "openOrders",
            "user": user_address
        });

        let orders_data: serde_json::Value =
            post_info_json(&self.client, &self.base_url, &request_body, "openOrders").await?;
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
        let url = format!("{}/info", self.base_url);

        let request_body = serde_json::json!({
            "type": "userFunding",
            "user": user_address,
            "startTime": 0
        });
        let body_str = serde_json::to_string(&request_body)?;
        log::debug!("POST {url} (userFunding) body={body_str}");

        let response = self
            .client
            .post(&url)
            .await
            .header("Content-Type", "application/json")
            .body(body_str.clone())
            .send()
            .await
            .context("Failed to request funding history")?;

        let status = response.status();
        let text = response
            .text()
            .await
            .context("Failed to read funding history response body")?;

        if !status.is_success() {
            log::warn!(
                "Funding history endpoint not available (status={status} body={text}). This feature may not be implemented in the Hyperliquid API yet."
            );
            return Ok(vec![]);
        }

        log::debug!("userFunding response: status={status} body={text}");

        let funding_data: serde_json::Value = serde_json::from_str(&text)
            .with_context(|| format!("Failed to parse funding response (body={text})"))?;
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
        let mut all_rates = Vec::new();
        let mut current_start = start_time;

        loop {
            let mut request_body = serde_json::json!({
                "type": "fundingHistory",
                "coin": coin,
                "startTime": current_start
            });

            if let Some(end_time) = end_time {
                request_body["endTime"] =
                    serde_json::Value::Number(serde_json::Number::from(end_time));
            }

            let funding_rates: Vec<HistoricalFundingRate> = post_info_json(
                &self.client,
                &self.base_url,
                &request_body,
                "fundingHistory",
            )
            .await?;

            let num_results = funding_rates.len();

            if funding_rates.is_empty() {
                break;
            }

            // Get the last timestamp for pagination
            let last_time = funding_rates.last().map(|r| r.time).unwrap_or(0);

            all_rates.extend(funding_rates);

            // If we got fewer than 500 results, we've reached the end
            if num_results < 500 {
                break;
            }

            // If we have an end_time and last result is at or past it, stop
            if let Some(end) = end_time {
                if last_time >= end {
                    break;
                }
            }

            // Move start time forward for next page (add 1ms to avoid duplicates)
            current_start = last_time + 1;
        }

        Ok(all_rates)
    }

    pub async fn user_perpetuals_account_summary(
        &self,
        user_address: &str,
    ) -> Result<UserPerpetualsState> {
        let request_body = serde_json::json!({
            "type": "clearinghouseState",
            "user": user_address
        });

        post_info_json(
            &self.client,
            &self.base_url,
            &request_body,
            "clearinghouseState",
        )
        .await
    }

    pub async fn user_spot_state(&self, user_address: &str) -> Result<SpotClearinghouseState> {
        let request_body = serde_json::json!({
            "type": "spotClearinghouseState",
            "user": user_address
        });

        post_info_json(
            &self.client,
            &self.base_url,
            &request_body,
            "spotClearinghouseState",
        )
        .await
    }

    pub async fn meta(&self) -> Result<serde_json::Value> {
        let request_body = serde_json::json!({
            "type": "meta"
        });

        post_info_json(&self.client, &self.base_url, &request_body, "meta").await
    }

    pub async fn outcome_meta(&self) -> Result<serde_json::Value> {
        let request_body = serde_json::json!({
            "type": "outcomeMeta"
        });

        post_info_json(&self.client, &self.base_url, &request_body, "outcomeMeta").await
    }

    pub async fn get_candle_snapshot(
        &self,
        coin: &str,
        interval: &str,
        start_time: u64,
        end_time: u64,
    ) -> Result<Vec<roshar_types::HyperliquidCandleData>> {
        let request_body = serde_json::json!({
            "type": "candleSnapshot",
            "req": {
                "coin": coin,
                "interval": interval,
                "startTime": start_time,
                "endTime": end_time
            }
        });

        post_info_json(
            &self.client,
            &self.base_url,
            &request_body,
            "candleSnapshot",
        )
        .await
    }

    /// Get open orders for a wallet address
    pub async fn open_orders(&self, wallet_address: ethers::types::H160) -> Result<Vec<UserOrder>> {
        let address_str = format!("{:#x}", wallet_address);
        self.get_user_orders(&address_str).await
    }

    /// Get perpetuals account state for a wallet address
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

    fn create_test_client() -> std::sync::Arc<RateLimitedClient> {
        std::sync::Arc::new(RateLimitedClient::new(10, 1))
    }

    #[tokio::test]
    async fn test_get_all_funding_rates_with_size() {
        let info_api = InfoApi::production_with_client(create_test_client());
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

        let info_api = InfoApi::testnet_with_client(create_test_client());
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

        let info_api = InfoApi::testnet_with_client(create_test_client());
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

        let wallet_address: ethers::types::H160 = user_address_str
            .parse()
            .expect("Invalid wallet address format");

        let info_api = InfoApi::testnet_with_client(create_test_client());
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

        let wallet_address: ethers::types::H160 = user_address_str
            .parse()
            .expect("Invalid wallet address format");

        let info_api = InfoApi::testnet_with_client(create_test_client());
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
        let test_address: ethers::types::H160 = "0x1234567890123456789012345678901234567890"
            .parse()
            .unwrap();

        let formatted = format!("{:#x}", test_address);
        assert_eq!(formatted, "0x1234567890123456789012345678901234567890");
    }

    #[test]
    fn test_user_spot_state_mixed_balances_deserialize() {
        let payload = serde_json::json!({
            "balances": [
                {"coin":"USDC","token":0,"total":"0.0000001","hold":"0.0","entryNtl":"0.0"},
                {"coin":"+0","total":"187.0","hold":"0.0","entryNtl":"121.03849263"},
                {"coin":"+1","total":"1.0","hold":"0.0","entryNtl":"0.35261391"}
            ]
        });

        let state: SpotClearinghouseState = serde_json::from_value(payload).unwrap();
        assert_eq!(state.balances.len(), 3);
        assert_eq!(state.balances[0].token, Some(0));
        assert_eq!(state.balances[1].token, None);
        assert_eq!(state.balances[2].token, None);
    }

    #[tokio::test]
    async fn test_get_historical_funding_rates_pagination() {
        let end_time = chrono::Utc::now().timestamp_millis();
        let start_time = end_time - (730 * 24 * 60 * 60 * 1000); // 2 years ago

        let info_api = InfoApi::production_with_client(create_test_client());
        let result = info_api
            .get_historical_funding_rates("BTC", start_time as u64, Option::None)
            .await;

        assert!(
            result.is_ok(),
            "Failed to fetch funding rates: {:?}",
            result.err()
        );

        let rates = result.unwrap();

        // Should have more than 1000 results (proving pagination worked)
        // 2 years * 365 days * 3 per day = ~2190 funding rates
        assert!(
            rates.len() > 1000,
            "Expected more than 1000 rates (pagination), got {}",
            rates.len()
        );

        // Verify chronological order
        for window in rates.windows(2) {
            assert!(
                window[0].time <= window[1].time,
                "Rates not in chronological order"
            );
        }

        println!("Fetched {} funding rates (pagination working)", rates.len());
    }

    #[tokio::test]
    #[ignore = "requires endpoint availability and network access"]
    async fn test_outcome_meta() {
        let info_api = InfoApi::production_with_client(create_test_client());

        let outcome_meta_result = info_api.outcome_meta().await;
        assert!(
            outcome_meta_result.is_ok(),
            "Failed to get outcomeMeta: {:?}",
            outcome_meta_result.err()
        );
    }
}
