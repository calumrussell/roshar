use roshar_types::{AssetInfo, SpotAsset, SpotMarketData, SpotToken};
use std::collections::HashMap;
use tokio::sync::mpsc;

use super::InfoApi;

/// Spot asset info with token details (needed for validation and mapping)
#[derive(Debug, Clone)]
pub struct SpotAssetInfo {
    pub asset: SpotAsset,
    pub base_token: SpotToken,
    pub quote_token: SpotToken,
}

/// Handle for querying exchange metadata
/// Wraps the oneshot channel communication with the metadata manager
#[derive(Clone)]
pub struct ExchangeMetadataHandle {
    metadata_query_tx: mpsc::Sender<MetadataQuery>,
}

impl ExchangeMetadataHandle {
    fn new(metadata_query_tx: mpsc::Sender<MetadataQuery>) -> Self {
        Self { metadata_query_tx }
    }

    /// Get spot asset info from metadata manager
    pub async fn get_spot_asset_info(&self) -> Result<HashMap<String, SpotAssetInfo>, String> {
        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
        self.metadata_query_tx
            .send(MetadataQuery::GetHyperliquidSpotAssetInfo { reply: reply_tx })
            .await
            .map_err(|e| format!("Failed to query spot asset info: {}", e))?;
        reply_rx
            .await
            .map_err(|e| format!("Failed to receive spot asset info: {}", e))
    }

    /// Get spot market data from metadata manager
    pub async fn get_spot_market_data(&self) -> Result<HashMap<String, SpotMarketData>, String> {
        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
        self.metadata_query_tx
            .send(MetadataQuery::GetHyperliquidSpotMarketData { reply: reply_tx })
            .await
            .map_err(|e| format!("Failed to query spot market data: {}", e))?;
        reply_rx
            .await
            .map_err(|e| format!("Failed to receive spot market data: {}", e))
    }

    /// Get perp asset info from metadata manager
    pub async fn get_perp_asset_info(&self) -> Result<HashMap<String, AssetInfo>, String> {
        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
        self.metadata_query_tx
            .send(MetadataQuery::GetHyperliquidPerpsAssetInfo { reply: reply_tx })
            .await
            .map_err(|e| format!("Failed to query perp metadata: {}", e))?;
        reply_rx
            .await
            .map_err(|e| format!("Failed to receive perp metadata: {}", e))
    }

    /// Get funding rates from metadata manager
    pub async fn get_funding_rates(&self) -> Result<Vec<(String, f64, f64, f64)>, String> {
        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
        self.metadata_query_tx
            .send(MetadataQuery::GetHyperliquidFundingRates { reply: reply_tx })
            .await
            .map_err(|e| format!("Failed to query funding rates: {}", e))?;
        reply_rx
            .await
            .map_err(|e| format!("Failed to receive funding rates: {}", e))
    }
}

enum MetadataQuery {
    GetHyperliquidPerpsAssetInfo {
        reply: tokio::sync::oneshot::Sender<HashMap<String, AssetInfo>>,
    },
    GetHyperliquidSpotAssetInfo {
        reply: tokio::sync::oneshot::Sender<HashMap<String, SpotAssetInfo>>,
    },
    GetHyperliquidSpotMarketData {
        reply: tokio::sync::oneshot::Sender<HashMap<String, SpotMarketData>>,
    },
    GetHyperliquidFundingRates {
        reply: tokio::sync::oneshot::Sender<Vec<(String, f64, f64, f64)>>,
    },
}

/// Manages exchange metadata with periodic background updates
pub struct ExchangeMetadataManager {
    hyperliquid_perps_asset_info: HashMap<String, AssetInfo>,
    hyperliquid_spot_asset_info: HashMap<String, SpotAssetInfo>, // For validation/mapping
    hyperliquid_spot_market_data: HashMap<String, SpotMarketData>, // For prices
    hyperliquid_funding_rates: Vec<(String, f64, f64, f64)>, // (coin, funding_rate, open_interest, daily_volume)
    update_interval_secs: u64,
    query_rx: tokio::sync::mpsc::Receiver<MetadataQuery>,
    is_production: bool,
}

impl ExchangeMetadataManager {
    /// Spawn a new metadata manager and return the handle and query channel
    pub fn spawn(
        update_interval_secs: u64,
        is_production: bool,
    ) -> (ExchangeMetadataHandle, tokio::task::JoinHandle<()>) {
        let (query_tx, query_rx) = tokio::sync::mpsc::channel(100);
        let manager = Self::new(update_interval_secs, query_rx, is_production);
        let handle = tokio::spawn(async move {
            manager.run().await;
        });
        let metadata_handle = ExchangeMetadataHandle::new(query_tx);
        (metadata_handle, handle)
    }

    fn new(
        update_interval_secs: u64,
        query_rx: tokio::sync::mpsc::Receiver<MetadataQuery>,
        is_production: bool,
    ) -> Self {
        Self {
            hyperliquid_perps_asset_info: HashMap::new(),
            hyperliquid_spot_asset_info: HashMap::new(),
            hyperliquid_spot_market_data: HashMap::new(),
            hyperliquid_funding_rates: Vec::new(),
            update_interval_secs,
            query_rx,
            is_production,
        }
    }

    /// Run the metadata manager loop that periodically updates Hyperliquid perpetuals asset info
    /// and handles queries. Caller should spawn this with tokio::spawn() to run in background.
    pub async fn run(mut self) {
        // Initial fetch
        if let Err(e) = self.update_hyperliquid_perps_metadata().await {
            log::error!("Initial Hyperliquid perps metadata fetch failed: {}", e);
        }
        if let Err(e) = self.update_hyperliquid_spot_metadata().await {
            log::error!("Initial Hyperliquid spot metadata fetch failed: {}", e);
        }

        // Periodic updates
        let mut interval =
            tokio::time::interval(tokio::time::Duration::from_secs(self.update_interval_secs));

        loop {
            tokio::select! {
                // Handle queries
                Some(query) = self.query_rx.recv() => {
                    self.handle_query(query);
                }

                // Periodic metadata update
                _ = interval.tick() => {
                    if let Err(e) = self.update_hyperliquid_perps_metadata().await {
                        log::error!("Failed to update Hyperliquid perps metadata: {}", e);
                    }
                    if let Err(e) = self.update_hyperliquid_spot_metadata().await {
                        log::error!("Failed to update Hyperliquid spot metadata: {}", e);
                    }
                }

                else => {
                    log::info!("ExchangeMetadataManager channels closed, shutting down");
                    break;
                }
            }
        }
    }

    fn handle_query(&self, query: MetadataQuery) {
        match query {
            MetadataQuery::GetHyperliquidPerpsAssetInfo { reply } => {
                let _ = reply.send(self.hyperliquid_perps_asset_info.clone());
            }
            MetadataQuery::GetHyperliquidSpotAssetInfo { reply } => {
                let _ = reply.send(self.hyperliquid_spot_asset_info.clone());
            }
            MetadataQuery::GetHyperliquidSpotMarketData { reply } => {
                let _ = reply.send(self.hyperliquid_spot_market_data.clone());
            }
            MetadataQuery::GetHyperliquidFundingRates { reply } => {
                let _ = reply.send(self.hyperliquid_funding_rates.clone());
            }
        }
    }

    async fn update_hyperliquid_perps_metadata(&mut self) -> Result<(), String> {
        let info_api = if self.is_production {
            InfoApi::production()
        } else {
            InfoApi::testnet()
        };

        let asset_info = info_api
            .get_info()
            .await
            .map_err(|e| format!("Failed to fetch Hyperliquid perps metadata: {}", e))?;

        let count = asset_info.len();
        self.hyperliquid_perps_asset_info = asset_info;
        log::debug!("Updated Hyperliquid perpetuals metadata: {} assets", count);

        // Also extract funding rates from the same metaAndAssetCtxs data
        // This avoids making a duplicate API call since get_info() and get_all_funding_rates_with_size()
        // both use the same endpoint
        self.update_funding_rates_from_asset_info().await?;

        Ok(())
    }

    /// Extract funding rates from the already-fetched perps asset info
    /// This avoids duplicate API calls since both use metaAndAssetCtxs endpoint
    async fn update_funding_rates_from_asset_info(&mut self) -> Result<(), String> {
        let mut funding_rates = Vec::new();

        for (coin_name, asset_info) in &self.hyperliquid_perps_asset_info {
            let rate = asset_info
                .market_data
                .funding
                .as_ref()
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(0.0);

            let open_interest_in_base = asset_info
                .market_data
                .open_interest
                .as_ref()
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(0.0);

            let open_interest = if open_interest_in_base == 0.0 {
                0.0
            } else {
                open_interest_in_base
                    * asset_info
                        .market_data
                        .mark_price
                        .as_ref()
                        .and_then(|s| s.parse::<f64>().ok())
                        .unwrap_or(0.0)
            };

            let daily_volume = asset_info
                .market_data
                .day_notional_volume
                .as_ref()
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(0.0);

            funding_rates.push((coin_name.clone(), rate, open_interest, daily_volume));
        }

        let count = funding_rates.len();
        self.hyperliquid_funding_rates = funding_rates;
        log::debug!(
            "Updated Hyperliquid funding rates from perps metadata: {} coins",
            count
        );
        Ok(())
    }

    async fn update_hyperliquid_spot_metadata(&mut self) -> Result<(), String> {
        let info_api = if self.is_production {
            InfoApi::production()
        } else {
            InfoApi::testnet()
        };

        // Get raw data directly from API - single call gives us everything
        let meta = info_api
            .get_spot_meta_and_asset_ctxs()
            .await
            .map_err(|e| format!("Failed to fetch spot metadata: {}", e))?;

        // Build lookup map for tokens by index
        let tokens_by_index: HashMap<u32, &SpotToken> = meta
            .0
            .tokens
            .iter()
            .map(|token| (token.index, token))
            .collect();

        // Build HashMap of coin name -> asset context for O(1) lookup
        let asset_ctx_map: HashMap<String, &SpotMarketData> =
            meta.1.iter().map(|ctx| (ctx.coin.clone(), ctx)).collect();

        let mut spot_asset_info = HashMap::new();
        let mut spot_market_data = HashMap::new();

        for asset in &meta.0.universe {
            if asset.tokens.len() != 2 {
                log::warn!(
                    "Asset {} does not have exactly 2 tokens: {:?}",
                    asset.name,
                    asset.tokens
                );
                continue;
            }

            let base_token_index = asset.tokens[0];
            let quote_token_index = asset.tokens[1];

            // Build SpotAssetInfo for validation/mapping
            if let (Some(base_token), Some(quote_token)) = (
                tokens_by_index.get(&base_token_index),
                tokens_by_index.get(&quote_token_index),
            ) {
                spot_asset_info.insert(
                    asset.name.clone(),
                    SpotAssetInfo {
                        asset: asset.clone(),
                        base_token: (*base_token).clone(),
                        quote_token: (*quote_token).clone(),
                    },
                );
            }

            // Build SpotMarketData for prices
            if let Some(asset_ctx) = asset_ctx_map.get(&asset.name) {
                let mut spot_data = (*asset_ctx).clone();
                spot_data.tokens = asset.tokens.clone();
                spot_market_data.insert(asset.name.clone(), spot_data);
            }
        }

        let count = spot_asset_info.len();
        self.hyperliquid_spot_asset_info = spot_asset_info;
        self.hyperliquid_spot_market_data = spot_market_data;

        log::debug!("Updated Hyperliquid spot metadata: {} assets", count);
        Ok(())
    }
}
