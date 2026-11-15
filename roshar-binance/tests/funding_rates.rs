// Live funding-rate integration tests (hit the public API).
mod funding_rates {
    use roshar_binance::BinanceApi;
    use roshar_binance::BinanceClient;

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
