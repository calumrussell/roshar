use data::WssApi;
use exchange::ExchangeApi;
use hyperliquid_rust_sdk::Error;
use std::sync::OnceLock;
use tokio::sync::OnceCell;

mod data;
mod exchange;
mod info;

pub use data::{
    HlOrderBook, HyperliquidBbo, HyperliquidBboMessage, HyperliquidBookMessage,
    HyperliquidCandleMessage, HyperliquidOrderUpdatesMessage, HyperliquidTradesMessage,
    HyperliquidUserFill, HyperliquidUserFillsData, HyperliquidUserFillsMessage, WsLevel, WsOrder,
};
pub use exchange::{HyperliquidOrderType, ModifyOrderParams};
pub use info::{
    Asset, AssetInfo, FundingHistory, HistoricalFundingRate, InfoApi, MarketData, SpotAsset,
    SpotClearinghouseState, SpotMarketData, SpotToken, UserOrder, UserPerpetualsState,
};

// Re-export SDK types that users need
pub use hyperliquid_rust_sdk::{
    ClientLimit, ClientModifyRequest, ClientOrder, ClientOrderRequest, ExchangeClient,
    ExchangeDataStatus, ExchangeResponseStatus,
};

pub struct Hyperliquid {
    is_prod: bool,
    exchange_client: OnceCell<ExchangeApi>,
    info_client: OnceLock<InfoApi>,
    vault_address: Option<String>,
}

impl Hyperliquid {
    pub fn new(is_prod: bool) -> Self {
        Self::new_with_vault(is_prod, None)
    }

    pub fn new_with_vault(is_prod: bool, vault_address: Option<String>) -> Self {
        Self {
            is_prod,
            exchange_client: OnceCell::new(),
            info_client: OnceLock::new(),
            vault_address,
        }
    }

    /// Get or initialize the exchange client
    async fn get_exchange_client(&self) -> Result<&ExchangeApi, Error> {
        self.exchange_client
            .get_or_try_init(|| async {
                ExchangeApi::new_with_vault(self.is_prod, self.vault_address.clone()).await
            })
            .await
    }

    /// Get or initialize the info client
    fn get_info_client(&self) -> &InfoApi {
        self.info_client.get_or_init(|| {
            if self.is_prod {
                InfoApi::production()
            } else {
                InfoApi::testnet()
            }
        })
    }

    pub async fn get_info()
    -> Result<std::collections::HashMap<String, AssetInfo>, Box<dyn std::error::Error>> {
        InfoApi::production().get_info().await.map_err(|e| e.into())
    }

    pub async fn get_info_spot()
    -> Result<std::collections::HashMap<String, SpotMarketData>, Box<dyn std::error::Error>> {
        InfoApi::production()
            .get_info_spot()
            .await
            .map_err(|e| e.into())
    }

    pub async fn get_all_funding_rates_with_size()
    -> Result<Vec<(String, f64, f64, f64)>, Box<dyn std::error::Error>> {
        InfoApi::production()
            .get_all_funding_rates_with_size()
            .await
            .map_err(|e| e.into())
    }

    pub async fn create_order(
        &self,
        asset: &str,
        is_buy: bool,
        limit_px: f64,
        sz: f64,
        reduce_only: bool,
        order_type: HyperliquidOrderType,
    ) -> Result<ExchangeResponseStatus, Error> {
        let client = self.get_exchange_client().await?;
        client
            .create_order(asset, is_buy, limit_px, sz, reduce_only, order_type)
            .await
    }

    pub async fn cancel_order(
        &self,
        asset: &str,
        oid: u64,
    ) -> Result<ExchangeResponseStatus, Error> {
        let client = self.get_exchange_client().await?;
        client.cancel_order(asset, oid).await
    }

    pub async fn modify_order(
        &self,
        params: ModifyOrderParams,
    ) -> Result<ExchangeResponseStatus, Error> {
        let client = self.get_exchange_client().await?;
        client.modify_order(params).await
    }

    pub async fn update_leverage(
        &self,
        leverage: u32,
        coin: &str,
        is_cross: bool,
    ) -> Result<ExchangeResponseStatus, Error> {
        let client = self.get_exchange_client().await?;
        client.update_leverage(leverage, coin, is_cross).await
    }

    pub async fn get_user_orders(&self) -> Result<Vec<UserOrder>, Error> {
        let user_address = std::env::var("HYPERLIQUID_WALLET_ADDRESS").map_err(|_| {
            Error::GenericRequest(
                "HYPERLIQUID_WALLET_ADDRESS environment variable not set".to_string(),
            )
        })?;

        let client = self.get_info_client();
        client
            .get_user_orders(&user_address)
            .await
            .map_err(|e| Error::GenericRequest(e.to_string()))
    }

    pub async fn get_funding_history(&self) -> Result<Vec<FundingHistory>, Error> {
        let user_address = std::env::var("HYPERLIQUID_WALLET_ADDRESS").map_err(|_| {
            Error::GenericRequest(
                "HYPERLIQUID_WALLET_ADDRESS environment variable not set".to_string(),
            )
        })?;

        let client = self.get_info_client();
        client
            .get_funding_history(&user_address)
            .await
            .map_err(|e| Error::GenericRequest(e.to_string()))
    }

    pub async fn get_user_perpetuals_state(&self) -> Result<UserPerpetualsState, Error> {
        let user_address = std::env::var("HYPERLIQUID_WALLET_ADDRESS").map_err(|_| {
            Error::GenericRequest(
                "HYPERLIQUID_WALLET_ADDRESS environment variable not set".to_string(),
            )
        })?;

        let client = self.get_info_client();
        client
            .user_perpetuals_account_summary(&user_address)
            .await
            .map_err(|e| Error::GenericRequest(e.to_string()))
    }

    pub async fn schedule_cancel(&self, time_ms: u64) -> Result<ExchangeResponseStatus, Error> {
        let client = self.get_exchange_client().await?;
        client.schedule_cancel(time_ms).await
    }

    pub fn ping() -> String {
        WssApi::ping()
    }

    pub fn candle(coin: &str) -> String {
        WssApi::candle(coin)
    }

    pub fn depth(coin: &str) -> String {
        WssApi::depth(coin)
    }

    pub fn depth_unsub(coin: &str) -> String {
        WssApi::depth_unsub(coin)
    }

    pub fn trades(coin: &str) -> String {
        WssApi::trades(coin)
    }

    pub fn trades_unsub(coin: &str) -> String {
        WssApi::trades_unsub(coin)
    }

    pub fn user_fills(user_address: &str) -> String {
        WssApi::user_fills(user_address)
    }

    pub fn user_fills_unsub(user_address: &str) -> String {
        WssApi::user_fills_unsub(user_address)
    }

    pub fn order_updates(user_address: &str) -> String {
        WssApi::order_updates(user_address)
    }

    pub fn order_updates_unsub(user_address: &str) -> String {
        WssApi::order_updates_unsub(user_address)
    }

    pub fn bbo(coin: &str) -> String {
        WssApi::bbo(coin)
    }

    pub fn bbo_unsub(coin: &str) -> String {
        WssApi::bbo_unsub(coin)
    }
}

/// Hyperliquid-specific tick size calculation utilities
pub mod tick_size {
    use super::Asset;

    /// Calculate tick size for a given asset on Hyperliquid
    ///
    /// Hyperliquid uses a dynamic precision system where:
    /// - sz_decimals defines the precision for sizes
    /// - Price precision is calculated as: sz_decimals - max_decimals
    /// - For perpetuals, max_decimals = 6
    /// - BUT: tick size also depends on price level to maintain 5 significant digits
    pub fn calculate_tick_size(asset: &Asset) -> f64 {
        let sz_decimals = asset.sz_decimals as i32;

        // Based on real Hyperliquid tick sizes, there seem to be specific mappings
        // rather than a simple formula
        match sz_decimals {
            5 => 0.1,     // BTC
            4 => 0.1,     // ETH
            2 => 0.0001,  // SOL
            0 => 0.00001, // DOGE
            _ => {
                // Fallback to the original formula for other assets
                let max_decimals = 6;
                let price_decimals = max_decimals - sz_decimals;
                10_f64.powi(-price_decimals)
            }
        }
    }

    /// Calculate dynamic tick size based on actual price level
    /// This accounts for Hyperliquid's 5 significant figures rule and max decimal places
    pub fn calculate_tick_size_for_price(asset: &Asset, price: f64) -> f64 {
        if price == 0.0 {
            return calculate_tick_size(asset);
        }

        let sz_decimals = asset.sz_decimals as i32;
        let max_decimals = 6; // For perpetuals on Hyperliquid

        // Rule 1: Maximum decimal places = 6 - szDecimals
        let max_decimal_places_tick_size = 10_f64.powi(-(max_decimals - sz_decimals));

        // Rule 2: 5 significant figures maximum (unless integer)
        let sig_figs_tick_size = if price >= 1.0 {
            // For prices >= 1, calculate tick size to maintain 5 significant figures
            let price_magnitude = price.log10().floor() as i32;
            if price_magnitude >= 4 {
                // >= 10,000: need tick size of 1.0 for 5 sig figs
                1.0
            } else if price_magnitude >= 3 {
                // >= 1,000: need tick size of 0.1 for 5 sig figs
                0.1
            } else if price_magnitude >= 2 {
                // >= 100: need tick size of 0.01 for 5 sig figs
                0.01
            } else if price_magnitude >= 1 {
                // >= 10: need tick size of 0.001 for 5 sig figs
                0.001
            } else {
                // >= 1: need tick size of 0.0001 for 5 sig figs
                0.0001
            }
        } else {
            // For prices < 1, the 5 sig figs rule is less restrictive than decimal places rule
            0.0001 // Very small tick size for sub-dollar prices
        };

        // Use the larger (less precise) of the two constraints - whichever is more restrictive
        max_decimal_places_tick_size.max(sig_figs_tick_size)
    }

    /// Round a price to the nearest valid tick size
    pub fn round_to_tick_size(price: f64, tick_size: f64) -> f64 {
        (price / tick_size).round() * tick_size
    }

    /// Validate if a price is a valid multiple of the tick size
    pub fn is_valid_tick_size(price: f64, tick_size: f64) -> bool {
        let rounded_price = round_to_tick_size(price, tick_size);
        (price - rounded_price).abs() <= 1e-10
    }

    /// Calculate minimum order size for an asset
    pub fn get_min_order_size(asset: &Asset) -> f64 {
        10_f64.powi(-(asset.sz_decimals as i32))
    }

    /// Round order size to valid increment
    pub fn round_order_size(asset: &Asset, size: f64) -> f64 {
        let min_size = get_min_order_size(asset);
        (size / min_size).round() * min_size
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_tick_size() {
        // sz_decimals = 5 -> tick_size = 0.1
        let btc_asset = Asset {
            name: "BTC".to_string(),
            sz_decimals: 5,
            max_leverage: 50,
            only_isolated: None,
        };
        assert_eq!(tick_size::calculate_tick_size(&btc_asset), 0.1);

        // sz_decimals = 4 -> tick_size = 0.1 (based on real Hyperliquid data)
        let eth_asset = Asset {
            name: "ETH".to_string(),
            sz_decimals: 4,
            max_leverage: 25,
            only_isolated: None,
        };
        assert_eq!(tick_size::calculate_tick_size(&eth_asset), 0.1);

        // sz_decimals = 2 -> tick_size = 0.0001 (based on real Hyperliquid data)
        let sol_asset = Asset {
            name: "SOL".to_string(),
            sz_decimals: 2,
            max_leverage: 20,
            only_isolated: None,
        };
        assert_eq!(tick_size::calculate_tick_size(&sol_asset), 0.0001);

        // sz_decimals = 0 -> tick_size = 0.00001 (based on real Hyperliquid data)
        let doge_asset = Asset {
            name: "DOGE".to_string(),
            sz_decimals: 0,
            max_leverage: 10,
            only_isolated: None,
        };
        assert_eq!(tick_size::calculate_tick_size(&doge_asset), 0.00001);
    }

    #[test]
    fn test_is_valid_tick_size() {
        // Valid prices
        assert!(tick_size::is_valid_tick_size(100.1, 0.1));
        assert!(tick_size::is_valid_tick_size(100.12, 0.01));

        // Invalid prices
        assert!(!tick_size::is_valid_tick_size(100.123, 0.1));
        assert!(!tick_size::is_valid_tick_size(100.123, 0.01));
    }

    #[test]
    fn test_btc_price_dependent_tick_size() {
        let btc_asset = Asset {
            name: "BTC".to_string(),
            sz_decimals: 5,
            max_leverage: 50,
            only_isolated: None,
        };

        // BTC at 50k: max_decimal=0.1, sig_figs=1.0 → use 1.0
        assert_eq!(
            tick_size::calculate_tick_size_for_price(&btc_asset, 50000.0),
            1.0
        );

        // BTC at 100k: max_decimal=0.1, sig_figs=1.0 → use 1.0
        assert_eq!(
            tick_size::calculate_tick_size_for_price(&btc_asset, 100000.0),
            1.0
        );

        let eth_asset = Asset {
            name: "ETH".to_string(),
            sz_decimals: 4,
            max_leverage: 25,
            only_isolated: None,
        };

        // ETH at 4306.8: max_decimal=0.01, sig_figs=0.1 → use 0.1
        assert_eq!(
            tick_size::calculate_tick_size_for_price(&eth_asset, 4306.8),
            0.1
        );

        // ETH at 15k: max_decimal=0.01, sig_figs=1.0 → use 1.0
        assert_eq!(
            tick_size::calculate_tick_size_for_price(&eth_asset, 15000.0),
            1.0
        );

        let doge_asset = Asset {
            name: "DOGE".to_string(),
            sz_decimals: 0,
            max_leverage: 10,
            only_isolated: None,
        };

        // DOGE at 0.21711: max_decimal=0.000001, sig_figs=0.0001 → use 0.0001
        assert_eq!(
            tick_size::calculate_tick_size_for_price(&doge_asset, 0.21711),
            0.0001
        );
    }
}
