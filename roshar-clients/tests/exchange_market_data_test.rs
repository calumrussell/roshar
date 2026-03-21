// Funding Rate Integration Tests
mod funding_rates {
    mod binance {
        use roshar_clients::binance::BinanceClient;
        use roshar_clients::BinanceApi;

        #[tokio::test]
        async fn test_get_realtime_funding_rates() {
            let _ = env_logger::try_init();

            let client = BinanceClient::new(100);

            let result = client.get_realtime_funding_rates().await;

            assert!(
                result.is_ok(),
                "[Binance] Failed to fetch realtime funding rates: {:?}",
                result.err()
            );

            let funding_rates = result.unwrap();

            // Binance has many perpetual contracts, should have data
            assert!(
                !funding_rates.is_empty(),
                "[Binance] Expected funding rates data, got empty HashMap"
            );

            // Check that we have multiple symbols (at least 10 major pairs)
            assert!(
                funding_rates.len() >= 10,
                "[Binance] Expected at least 10 symbols with funding rates, got {}",
                funding_rates.len()
            );

            // Check that BTCUSDT exists (it's always available)
            assert!(
                funding_rates.contains_key("BTCUSDT"),
                "[Binance] Expected BTCUSDT in funding rates"
            );

            // Check that ETHUSDT exists (another major pair)
            assert!(
                funding_rates.contains_key("ETHUSDT"),
                "[Binance] Expected ETHUSDT in funding rates"
            );

            // Verify the structure of a funding rate entry
            if let Some(btc_data) = funding_rates.get("BTCUSDT") {
                assert_eq!(btc_data.symbol, "BTCUSDT");
                // Mark price should be parseable as a float
                let mark_price: f64 = btc_data
                    .mark_price
                    .parse()
                    .expect("[Binance] mark_price should be a valid number");
                assert!(mark_price > 0.0, "[Binance] mark_price should be positive");

                // Funding rate should be parseable as a float
                let _funding_rate: f64 = btc_data
                    .last_funding_rate
                    .parse()
                    .expect("[Binance] last_funding_rate should be a valid number");

                // Next funding time should be positive
                assert!(
                    btc_data.next_funding_time > 0,
                    "[Binance] next_funding_time should be positive"
                );
            }

            println!(
                "[Binance] Fetched funding rates for {} symbols. BTCUSDT funding_rate: {}",
                funding_rates.len(),
                funding_rates.get("BTCUSDT").unwrap().last_funding_rate
            );
        }
    }

    mod bybit {
        use roshar_clients::bybit::ByBitClient;
        use roshar_clients::ByBitApi;

        #[tokio::test]
        async fn test_get_realtime_funding_rates() {
            let _ = env_logger::try_init();

            let client = ByBitClient::new();

            let result = client.get_realtime_funding_rates().await;

            assert!(
                result.is_ok(),
                "[ByBit] Failed to fetch realtime funding rates: {:?}",
                result.err()
            );

            let tickers = result.unwrap();

            // Should have some tickers
            assert!(
                !tickers.is_empty(),
                "[ByBit] Expected some tickers, got none"
            );

            // Check that we have multiple symbols (at least 10 major pairs)
            assert!(
                tickers.len() >= 10,
                "[ByBit] Expected at least 10 symbols with funding rates, got {}",
                tickers.len()
            );

            // Check that BTCUSDT is present (a known perpetual)
            assert!(
                tickers.contains_key("BTCUSDT"),
                "[ByBit] Expected BTCUSDT ticker to be present"
            );

            // Check that ETHUSDT is present (another major pair)
            assert!(
                tickers.contains_key("ETHUSDT"),
                "[ByBit] Expected ETHUSDT ticker to be present"
            );

            // Verify funding rate data is present and valid
            let btc_ticker = tickers.get("BTCUSDT").unwrap();
            assert!(
                !btc_ticker.funding_rate.is_empty(),
                "[ByBit] Expected funding_rate to be present"
            );
            assert!(
                !btc_ticker.next_funding_time.is_empty(),
                "[ByBit] Expected next_funding_time to be present"
            );

            // Funding rate should be parseable as a float
            let _funding_rate: f64 = btc_ticker
                .funding_rate
                .parse()
                .expect("[ByBit] funding_rate should be a valid number");

            println!(
                "[ByBit] Fetched funding rates for {} symbols. BTCUSDT funding_rate: {}, next_funding_time: {}",
                tickers.len(),
                btc_ticker.funding_rate,
                btc_ticker.next_funding_time
            );
        }
    }

    mod hyperliquid {
        use roshar_clients::{HyperliquidApi, HyperliquidClient, HyperliquidConfig};
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
}
