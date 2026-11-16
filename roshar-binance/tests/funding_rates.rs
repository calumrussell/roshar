// Live funding integration tests (hit the public API).
mod funding_rates {
    use roshar_binance::BinanceApi;
    use roshar_binance::BinanceClient;

    #[tokio::test]
    async fn test_get_premium_index_all() {
        let _ = env_logger::try_init();
        let client = BinanceClient::new(100);

        let all = client
            .get_premium_index(None)
            .await
            .expect("[Binance] premium index");
        assert!(all.len() >= 10, "[Binance] got {} symbols", all.len());

        let btc = all
            .iter()
            .find(|p| p.symbol == "BTCUSDT")
            .expect("[Binance] BTCUSDT in premium index");
        assert!(btc.mark_price.parse::<f64>().unwrap() > 0.0);
        btc.last_funding_rate.parse::<f64>().unwrap();
        assert!(btc.next_funding_time > 0);

        let one = client
            .get_premium_index(Some("BTCUSDT"))
            .await
            .expect("[Binance] premium index BTCUSDT");
        assert_eq!(one.len(), 1);
        assert_eq!(one[0].symbol, "BTCUSDT");
    }

    #[tokio::test]
    async fn test_get_funding_rate_history_and_info() {
        let client = BinanceClient::new(10);
        let rates = client
            .get_funding_rate_history(Some("BTCUSDT"), None, None, Some(3))
            .await
            .expect("[Binance] funding rate history");
        assert_eq!(rates.len(), 3);
        let info = client
            .get_funding_info()
            .await
            .expect("[Binance] funding info");
        assert!(!info.is_empty());
    }
}
