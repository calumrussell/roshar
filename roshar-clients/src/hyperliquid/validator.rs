use super::rest::SpotAssetInfo;
use roshar_types::{AssetInfo, Venue};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct OrderRequest {
    pub venue: Venue,
    pub asset: String,
    pub is_buy: bool,
    pub limit_px: f64,
    pub sz: f64,
    /// For Hyperliquid only: if true, treat as spot trading (use spot validation)
    /// For other venues, this field is ignored
    pub hyperliquid_is_spot: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct ValidatedOrder {
    pub venue: Venue,
    pub asset: String,
    pub is_buy: bool,
    pub limit_px: f64,
    pub sz: f64,
}

pub struct OrderValidator;

impl Default for OrderValidator {
    fn default() -> Self {
        Self
    }
}

impl OrderValidator {
    pub fn new() -> Self {
        Self
    }

    /// Calculate the tick size needed to maintain 5 significant figures for a given price
    ///
    /// For price p, floor(log10(p)) gives the magnitude of the first significant digit:
    /// - 50000.0 → floor(4.7) = 4, first sig digit at 10^4
    /// - 0.28127 → floor(-0.55) = -1, first sig digit at 10^-1
    /// - 0.002849 → floor(-2.545) = -3, first sig digit at 10^-3
    ///
    /// The 5th significant figure is 4 positions after the first, so:
    /// tick_size = 10^(magnitude - 4)
    fn calc_sig_digits(price: f64) -> f64 {
        if price <= 0.0 {
            return 0.00001; // fallback
        }
        let magnitude = price.log10().floor() as i32;
        let fifth_sig_digit_pos = magnitude - 4;
        10_f64.powi(fifth_sig_digit_pos)
    }

    /// Calculate tick size for Hyperliquid spot orders
    ///
    /// Spot tick size is constrained by two rules:
    /// 1. Maximum decimal places = 8 - base_sz_decimals
    /// 2. Maximum 5 significant figures
    ///
    /// The tick size is the larger (less precise) of these two constraints.
    fn calc_spot_tick_size(price: f64, base_sz_decimals: i32) -> f64 {
        const MAX_DECIMALS: i32 = 8;
        let max_decimal_places = MAX_DECIMALS - base_sz_decimals;
        let max_decimal_tick = 10_f64.powi(-max_decimal_places);
        let sig_figs_tick = Self::calc_sig_digits(price);
        max_decimal_tick.max(sig_figs_tick)
    }

    /// Calculate tick size for Hyperliquid perpetual orders
    ///
    /// Perp tick size is constrained by two rules:
    /// 1. Maximum decimal places = 6 - sz_decimals
    /// 2. Maximum 5 significant figures
    ///
    /// The tick size is the larger (less precise) of these two constraints.
    fn calc_perp_tick_size(price: f64, sz_decimals: i32) -> f64 {
        const MAX_DECIMALS: i32 = 6;
        let max_decimal_places = MAX_DECIMALS - sz_decimals;
        let max_decimal_tick = 10_f64.powi(-max_decimal_places);
        let sig_figs_tick = Self::calc_sig_digits(price);
        max_decimal_tick.max(sig_figs_tick)
    }

    /// Round a price to the nearest valid tick size
    fn round_to_tick_size(price: f64, tick_size: f64) -> f64 {
        (price / tick_size).round() * tick_size
    }

    /// Validate and round an order request for Hyperliquid perpetuals
    ///
    /// Performs the following validations and transformations:
    /// - Basic validation (non-empty asset, positive size, positive price)
    /// - Rounds price to valid tick size based on price level
    /// - Rounds size to valid increment based on sz_decimals
    /// - Validates minimum order size
    pub fn validate_and_round_hyperliquid_perps(
        &self,
        request: OrderRequest,
        asset_info_map: &HashMap<String, AssetInfo>,
    ) -> Result<ValidatedOrder, String> {
        // Basic validation
        if request.asset.is_empty() {
            return Err("Asset cannot be empty".to_string());
        }

        if request.sz <= 0.0 {
            return Err("Order size must be positive".to_string());
        }

        if request.limit_px <= 0.0 {
            return Err("Limit price must be positive".to_string());
        }

        // Get asset metadata
        let asset_info = asset_info_map.get(&request.asset).ok_or_else(|| {
            format!(
                "Asset metadata not found for {}. Ensure ExchangeMetadataManager is running",
                request.asset
            )
        })?;

        let sz_decimals = asset_info.asset.sz_decimals as i32;

        // Round price to valid tick size
        let tick_size = Self::calc_perp_tick_size(request.limit_px, sz_decimals);
        let rounded_px = Self::round_to_tick_size(request.limit_px, tick_size);

        // Round size to valid increment based on sz_decimals
        let min_order_size = 10_f64.powi(-sz_decimals);
        let rounded_sz = (request.sz / min_order_size).round() * min_order_size;

        // Validate minimum order size
        if rounded_sz < min_order_size {
            return Err(format!(
                "Order size {} (rounded from {}) is below minimum {} for {}",
                rounded_sz, request.sz, min_order_size, request.asset
            ));
        }

        log::debug!(
            "Validated order: {} {} {} @ {} (rounded from {} @ {})",
            if request.is_buy { "BUY" } else { "SELL" },
            rounded_sz,
            request.asset,
            rounded_px,
            request.sz,
            request.limit_px
        );

        Ok(ValidatedOrder {
            venue: request.venue,
            asset: request.asset,
            is_buy: request.is_buy,
            limit_px: rounded_px,
            sz: rounded_sz,
        })
    }

    /// Validate and round an order request for Hyperliquid spot trading
    ///
    /// For spot trading:
    /// - Asset name is the trading pair (e.g., "PURR/USDC")
    /// - Returns asset as the ticker name (SDK handles conversion to index)
    /// - Uses base token's sz_decimals for size rounding
    pub fn validate_and_round_hyperliquid_spot(
        &self,
        request: OrderRequest,
        spot_asset_info_map: &HashMap<String, SpotAssetInfo>,
    ) -> Result<ValidatedOrder, String> {
        // Basic validation
        if request.asset.is_empty() {
            return Err("Asset cannot be empty".to_string());
        }

        if request.sz <= 0.0 {
            return Err("Order size must be positive".to_string());
        }

        if request.limit_px <= 0.0 {
            return Err("Limit price must be positive".to_string());
        }

        // Get spot asset metadata
        let spot_info = spot_asset_info_map.get(&request.asset).ok_or_else(|| {
            format!(
                "Spot asset metadata not found for {}. Ensure ExchangeMetadataManager is running",
                request.asset
            )
        })?;

        // For spot, price validation rules:
        // 1. Up to 5 significant figures (unless integer)
        // 2. No more than (8 - base_sz_decimals) decimal places
        //    where MAX_DECIMALS = 8 for spot (vs 6 for perps)
        let base_sz_decimals = spot_info.base_token.sz_decimals as i32;

        log::debug!(
            "Validating spot order for {}: base_token={} (sz_decimals={}), quote_token={} (sz_decimals={})",
            request.asset,
            spot_info.base_token.name,
            base_sz_decimals,
            spot_info.quote_token.name,
            spot_info.quote_token.sz_decimals
        );

        // Calculate tick size and round price
        let tick_size = Self::calc_spot_tick_size(request.limit_px, base_sz_decimals);
        let rounded_px = Self::round_to_tick_size(request.limit_px, tick_size);

        // Round size based on base token decimals
        let min_order_size = 10_f64.powi(-base_sz_decimals);
        let rounded_sz = (request.sz / min_order_size).round() * min_order_size;

        // Validate minimum order size
        if rounded_sz < min_order_size {
            return Err(format!(
                "Order size {} (rounded from {}) is below minimum {} for {}",
                rounded_sz, request.sz, min_order_size, request.asset
            ));
        }

        log::debug!(
            "Validated spot order: {} {} {} @ {} (rounded from {} @ {})",
            if request.is_buy { "BUY" } else { "SELL" },
            rounded_sz,
            request.asset,
            rounded_px,
            request.sz,
            request.limit_px,
        );

        Ok(ValidatedOrder {
            venue: request.venue,
            asset: request.asset.clone(),
            is_buy: request.is_buy,
            limit_px: rounded_px,
            sz: rounded_sz,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_validation() {
        let asset_info_map = HashMap::new();
        let validator = OrderValidator::new();

        // Empty asset
        let request = OrderRequest {
            venue: Venue::Hyperliquid,
            asset: "".to_string(),
            is_buy: true,
            limit_px: 100.0,
            sz: 1.0,
            hyperliquid_is_spot: Some(false),
        };
        assert!(validator
            .validate_and_round_hyperliquid_perps(request, &asset_info_map)
            .is_err());

        // Zero size
        let request = OrderRequest {
            venue: Venue::Hyperliquid,
            asset: "BTC".to_string(),
            is_buy: true,
            limit_px: 100.0,
            sz: 0.0,
            hyperliquid_is_spot: Some(false),
        };
        assert!(validator
            .validate_and_round_hyperliquid_perps(request, &asset_info_map)
            .is_err());

        // Negative size
        let request = OrderRequest {
            venue: Venue::Hyperliquid,
            asset: "ETH".to_string(),
            is_buy: true,
            limit_px: 2000.0,
            sz: -0.1,
            hyperliquid_is_spot: Some(false),
        };
        assert!(validator
            .validate_and_round_hyperliquid_perps(request, &asset_info_map)
            .is_err());

        // Negative price
        let request = OrderRequest {
            venue: Venue::Hyperliquid,
            asset: "BTC".to_string(),
            is_buy: false,
            limit_px: -1000.0,
            sz: 1.0,
            hyperliquid_is_spot: Some(false),
        };
        assert!(validator
            .validate_and_round_hyperliquid_perps(request, &asset_info_map)
            .is_err());
    }

    #[test]
    fn test_round_to_tick_size() {
        // Valid prices (use approximate comparison due to floating point)
        let rounded = OrderValidator::round_to_tick_size(100.1, 0.1);
        assert!((rounded - 100.1).abs() < 1e-9);

        let rounded = OrderValidator::round_to_tick_size(100.12, 0.01);
        assert!((rounded - 100.12).abs() < 1e-9);

        // Invalid prices should be rounded
        let rounded = OrderValidator::round_to_tick_size(100.123, 0.1);
        assert!((rounded - 100.1).abs() < 1e-9);

        let rounded = OrderValidator::round_to_tick_size(100.126, 0.1);
        assert!((rounded - 100.1).abs() < 1e-9);
    }

    #[test]
    fn test_spot_tick_size() {
        // UBTC (szDecimals=5) @ 88839
        // max_decimal_tick = 10^-(8-5) = 0.001
        // sig_figs_tick = 10^(4-4) = 1
        // tick = max(0.001, 1) = 1
        assert_eq!(OrderValidator::calc_spot_tick_size(88839.0, 5), 1.0);

        // UETH (szDecimals=4) @ 2958.3
        // max_decimal_tick = 10^-(8-4) = 0.0001
        // sig_figs_tick = 10^(3-4) = 0.1
        // tick = max(0.0001, 0.1) = 0.1
        assert_eq!(OrderValidator::calc_spot_tick_size(2958.3, 4), 0.1);

        // UFART (szDecimals=1) @ 0.27997
        // max_decimal_tick = 10^-(8-1) = 0.0000001
        // sig_figs_tick = 10^(-1-4) = 0.00001
        // tick = max(0.0000001, 0.00001) = 0.00001
        assert_eq!(OrderValidator::calc_spot_tick_size(0.27997, 1), 0.00001);

        // UPUMP (szDecimals=0) @ 0.0028284
        // max_decimal_tick = 10^-(8-0) = 0.00000001
        // sig_figs_tick = 10^(-3-4) = 0.0000001
        // tick = max(0.00000001, 0.0000001) = 0.0000001
        assert_eq!(OrderValidator::calc_spot_tick_size(0.0028284, 0), 0.0000001);
    }

    #[test]
    fn test_perp_tick_size() {
        // BTC (szDecimals=5) @ 88839
        // max_decimal_tick = 10^-(6-5) = 0.1
        // sig_figs_tick = 10^(4-4) = 1
        // tick = max(0.1, 1) = 1
        assert_eq!(OrderValidator::calc_perp_tick_size(88839.0, 5), 1.0);

        // ETH (szDecimals=4) @ 2958.3
        // max_decimal_tick = 10^-(6-4) = 0.01
        // sig_figs_tick = 10^(3-4) = 0.1
        // tick = max(0.01, 0.1) = 0.1
        assert_eq!(OrderValidator::calc_perp_tick_size(2958.3, 4), 0.1);

        // HYPE (szDecimals=2) @ 33.582
        // max_decimal_tick = 10^-(6-2) = 0.0001
        // sig_figs_tick = 10^(1-4) = 0.001
        // tick = max(0.0001, 0.001) = 0.001
        assert_eq!(OrderValidator::calc_perp_tick_size(33.582, 2), 0.001);

        // FARTCOIN (szDecimals=1) @ 0.27985
        // max_decimal_tick = 10^-(6-1) = 0.00001
        // sig_figs_tick = 10^(-1-4) = 0.00001
        // tick = max(0.00001, 0.00001) = 0.00001
        assert_eq!(OrderValidator::calc_perp_tick_size(0.27985, 1), 0.00001);

        // PUMP (szDecimals=0) @ 0.002819
        // max_decimal_tick = 10^-(6-0) = 0.000001
        // sig_figs_tick = 10^(-3-4) = 0.0000001
        // tick = max(0.000001, 0.0000001) = 0.000001
        assert_eq!(OrderValidator::calc_perp_tick_size(0.002819, 0), 0.000001);
    }
}
