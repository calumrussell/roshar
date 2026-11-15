// Live funding-rate integration tests (hit the public API).
mod funding_rates {
    use roshar_bybit::ByBitApi;
    use roshar_bybit::ByBitClient;

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
