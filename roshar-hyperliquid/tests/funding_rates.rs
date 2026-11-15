// Live funding-rate integration tests (hit the public API).
mod funding_rates {
    use roshar_hyperliquid::{HyperliquidApi, HyperliquidClient, HyperliquidConfig};
    use std::time::Duration;

    #[tokio::test]
    async fn test_get_all_funding_rates_with_size() {
        let _ = env_logger::try_init();

        let config = HyperliquidConfig {
            is_mainnet: true,
            metadata_update_interval_secs: 60,
            wallet_address: None,
            rate_limit_refill: 10,
            rate_limit_interval_secs: 1,
        };
        let client = HyperliquidClient::new(config);

        // Wait for initial metadata fetch
        tokio::time::sleep(Duration::from_secs(3)).await;

        let result = client.get_all_funding_rates_with_size().await;

        assert!(
            result.is_ok(),
            "[Hyperliquid] Failed to get funding rates: {:?}",
            result.err()
        );

        let funding_rates = result.unwrap();
        assert!(
            !funding_rates.is_empty(),
            "[Hyperliquid] Funding rates should not be empty"
        );

        // Check that we have multiple symbols (at least 10)
        assert!(
            funding_rates.len() >= 10,
            "[Hyperliquid] Expected at least 10 symbols with funding rates, got {}",
            funding_rates.len()
        );

        // Check structure of entries
        let mut found_btc = false;
        for (symbol, rate, open_interest, volume) in &funding_rates {
            assert!(
                !symbol.is_empty(),
                "[Hyperliquid] Symbol should not be empty"
            );
            assert!(
                rate.is_finite(),
                "[Hyperliquid] Funding rate should be a finite number"
            );
            assert!(
                *open_interest >= 0.0,
                "[Hyperliquid] Open interest should be non-negative"
            );
            assert!(
                *volume >= 0.0,
                "[Hyperliquid] Volume should be non-negative"
            );

            if symbol == "BTC" {
                found_btc = true;
            }
        }

        assert!(found_btc, "[Hyperliquid] Expected BTC in funding rates");

        // Find BTC entry for logging
        if let Some((_, rate, oi, vol)) = funding_rates.iter().find(|(s, _, _, _)| s == "BTC") {
            println!(
                    "[Hyperliquid] Fetched funding rates for {} symbols. BTC: rate={}, OI={}, volume={}",
                    funding_rates.len(),
                    rate,
                    oi,
                    vol
                );
        }
    }
}
