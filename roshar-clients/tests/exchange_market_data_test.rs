use roshar_ws_mgr::Manager;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

// Binance tests
mod binance {
    use super::*;
    use roshar_clients::binance::{MarketDataFeed, MarketEvent};

    #[tokio::test]
    #[ignore]
    async fn test_depth_polling() {
        let _ = env_logger::try_init();

        let ws_manager: Arc<Manager> = Manager::new();
        let (event_tx, _event_rx) = mpsc::channel::<MarketEvent>(1000);

        let feed = MarketDataFeed::new(ws_manager.clone(), event_tx);
        let handle = feed.get_handle();
        let state = feed.get_state();

        tokio::spawn(async move {
            feed.run().await;
        });

        tokio::time::sleep(Duration::from_secs(2)).await;

        handle
            .add_depth("BTCUSDT")
            .await
            .expect("Failed to subscribe to BTCUSDT");
        println!("[Binance] Subscribed to BTCUSDT depth");

        let mut attempts = 0;
        loop {
            if let Some(book) = state.get_latest_depth("BTCUSDT") {
                let view = book.as_view();
                let (bid, ask) = view.get_bbo();
                println!("[Binance] BTCUSDT BBO: bid={}, ask={}", bid, ask);
                println!("[Binance] Bid levels: {}", view.bid_prices().len());
                println!("[Binance] Ask levels: {}", view.ask_prices().len());
                assert!(!bid.is_empty(), "Bid should not be empty");
                assert!(!ask.is_empty(), "Ask should not be empty");
                break;
            }

            attempts += 1;
            if attempts > 15 {
                panic!("Failed to receive depth data after 15 attempts");
            }

            println!("[Binance] No data yet, waiting... (attempt {})", attempts);
            tokio::time::sleep(Duration::from_secs(1)).await;
        }

        println!("[Binance] Depth polling test passed!");
    }

    #[tokio::test]
    #[ignore]
    async fn test_reactive() {
        let _ = env_logger::try_init();

        let ws_manager: Arc<Manager> = Manager::new();
        let (event_tx, mut event_rx) = mpsc::channel::<MarketEvent>(1000);

        let feed = MarketDataFeed::new(ws_manager.clone(), event_tx);
        let handle = feed.get_handle();

        tokio::spawn(async move {
            feed.run().await;
        });

        tokio::time::sleep(Duration::from_secs(2)).await;

        handle
            .add_depth("BTCUSDT")
            .await
            .expect("Failed to subscribe to BTCUSDT depth");
        handle
            .add_trades("BTCUSDT")
            .await
            .expect("Failed to subscribe to BTCUSDT trades");
        println!("[Binance] Subscribed to BTCUSDT depth and trades");

        let mut depth_count = 0;
        let mut trade_count = 0;
        let target_events = 5;

        let timeout = tokio::time::timeout(Duration::from_secs(30), async {
            while depth_count < target_events {
                if let Some(event) = event_rx.recv().await {
                    match event {
                        MarketEvent::DepthUpdate { symbol, book } => {
                            depth_count += 1;
                            let view = book.as_view();
                            let (bid, ask) = view.get_bbo();
                            println!(
                                "[Binance] [{}] DepthUpdate #{}: bid={}, ask={}",
                                symbol, depth_count, bid, ask
                            );
                        }
                        MarketEvent::TradeUpdate { symbol, trades } => {
                            trade_count += 1;
                            println!(
                                "[Binance] [{}] TradeUpdate #{}: {} trades",
                                symbol,
                                trade_count,
                                trades.len()
                            );
                        }
                    }
                }
            }
        });

        match timeout.await {
            Ok(_) => {
                println!(
                    "[Binance] Received {} depth updates and {} trade updates",
                    depth_count, trade_count
                );
                assert!(depth_count >= target_events, "Should receive depth updates");
            }
            Err(_) => {
                panic!(
                    "Timeout waiting for events. Got {} depth, {} trades",
                    depth_count, trade_count
                );
            }
        }
    }
}

// ByBit tests
mod bybit {
    use super::*;
    use roshar_clients::bybit::{MarketDataFeed, MarketEvent};

    #[tokio::test]
    #[ignore]
    async fn test_depth_polling() {
        let _ = env_logger::try_init();

        let ws_manager: Arc<Manager> = Manager::new();
        let (event_tx, _event_rx) = mpsc::channel::<MarketEvent>(1000);

        let feed = MarketDataFeed::new(ws_manager.clone(), event_tx);
        let handle = feed.get_handle();
        let state = feed.get_state();

        tokio::spawn(async move {
            feed.run().await;
        });

        tokio::time::sleep(Duration::from_secs(2)).await;

        handle
            .add_depth("BTCUSDT")
            .await
            .expect("Failed to subscribe to BTCUSDT");
        println!("[ByBit] Subscribed to BTCUSDT depth");

        let mut attempts = 0;
        loop {
            if let Some(book) = state.get_latest_depth("BTCUSDT") {
                let view = book.as_view();
                let (bid, ask) = view.get_bbo();
                println!("[ByBit] BTCUSDT BBO: bid={}, ask={}", bid, ask);
                println!("[ByBit] Bid levels: {}", view.bid_prices().len());
                println!("[ByBit] Ask levels: {}", view.ask_prices().len());
                assert!(!bid.is_empty(), "Bid should not be empty");
                assert!(!ask.is_empty(), "Ask should not be empty");
                break;
            }

            attempts += 1;
            if attempts > 15 {
                panic!("Failed to receive depth data after 15 attempts");
            }

            println!("[ByBit] No data yet, waiting... (attempt {})", attempts);
            tokio::time::sleep(Duration::from_secs(1)).await;
        }

        println!("[ByBit] Depth polling test passed!");
    }

    #[tokio::test]
    #[ignore]
    async fn test_reactive() {
        let _ = env_logger::try_init();

        let ws_manager: Arc<Manager> = Manager::new();
        let (event_tx, mut event_rx) = mpsc::channel::<MarketEvent>(1000);

        let feed = MarketDataFeed::new(ws_manager.clone(), event_tx);
        let handle = feed.get_handle();

        tokio::spawn(async move {
            feed.run().await;
        });

        tokio::time::sleep(Duration::from_secs(2)).await;

        handle
            .add_depth("BTCUSDT")
            .await
            .expect("Failed to subscribe to BTCUSDT depth");
        handle
            .add_trades("BTCUSDT")
            .await
            .expect("Failed to subscribe to BTCUSDT trades");
        println!("[ByBit] Subscribed to BTCUSDT depth and trades");

        let mut depth_count = 0;
        let mut trade_count = 0;
        let target_events = 5;

        let timeout = tokio::time::timeout(Duration::from_secs(30), async {
            while depth_count < target_events {
                if let Some(event) = event_rx.recv().await {
                    match event {
                        MarketEvent::DepthUpdate { symbol, book } => {
                            depth_count += 1;
                            let view = book.as_view();
                            let (bid, ask) = view.get_bbo();
                            println!(
                                "[ByBit] [{}] DepthUpdate #{}: bid={}, ask={}",
                                symbol, depth_count, bid, ask
                            );
                        }
                        MarketEvent::TradeUpdate { symbol, trades } => {
                            trade_count += 1;
                            println!(
                                "[ByBit] [{}] TradeUpdate #{}: {} trades",
                                symbol,
                                trade_count,
                                trades.len()
                            );
                        }
                    }
                }
            }
        });

        match timeout.await {
            Ok(_) => {
                println!(
                    "[ByBit] Received {} depth updates and {} trade updates",
                    depth_count, trade_count
                );
                assert!(depth_count >= target_events, "Should receive depth updates");
            }
            Err(_) => {
                panic!(
                    "Timeout waiting for events. Got {} depth, {} trades",
                    depth_count, trade_count
                );
            }
        }
    }
}

// Kraken tests
mod kraken {
    use super::*;
    use roshar_clients::kraken::{MarketDataFeed, MarketEvent};

    #[tokio::test]
    #[ignore]
    async fn test_depth_polling() {
        let _ = env_logger::try_init();

        let ws_manager: Arc<Manager> = Manager::new();
        let (event_tx, _event_rx) = mpsc::channel::<MarketEvent>(1000);

        let feed = MarketDataFeed::new(ws_manager.clone(), event_tx);
        let handle = feed.get_handle();
        let state = feed.get_state();

        tokio::spawn(async move {
            feed.run().await;
        });

        tokio::time::sleep(Duration::from_secs(2)).await;

        handle
            .add_depth("PI_XBTUSD")
            .await
            .expect("Failed to subscribe to PI_XBTUSD");
        println!("[Kraken] Subscribed to PI_XBTUSD depth");

        let mut attempts = 0;
        loop {
            if let Some(book) = state.get_latest_depth("PI_XBTUSD") {
                let view = book.as_view();
                let (bid, ask) = view.get_bbo();
                println!("[Kraken] PI_XBTUSD BBO: bid={}, ask={}", bid, ask);
                println!("[Kraken] Bid levels: {}", view.bid_prices().len());
                println!("[Kraken] Ask levels: {}", view.ask_prices().len());
                assert!(!bid.is_empty(), "Bid should not be empty");
                assert!(!ask.is_empty(), "Ask should not be empty");
                break;
            }

            attempts += 1;
            if attempts > 15 {
                panic!("Failed to receive depth data after 15 attempts");
            }

            println!("[Kraken] No data yet, waiting... (attempt {})", attempts);
            tokio::time::sleep(Duration::from_secs(1)).await;
        }

        println!("[Kraken] Depth polling test passed!");
    }

    #[tokio::test]
    #[ignore]
    async fn test_reactive() {
        let _ = env_logger::try_init();

        let ws_manager: Arc<Manager> = Manager::new();
        let (event_tx, mut event_rx) = mpsc::channel::<MarketEvent>(1000);

        let feed = MarketDataFeed::new(ws_manager.clone(), event_tx);
        let handle = feed.get_handle();

        tokio::spawn(async move {
            feed.run().await;
        });

        tokio::time::sleep(Duration::from_secs(2)).await;

        handle
            .add_depth("PI_XBTUSD")
            .await
            .expect("Failed to subscribe to PI_XBTUSD depth");
        handle
            .add_trades("PI_XBTUSD")
            .await
            .expect("Failed to subscribe to PI_XBTUSD trades");
        println!("[Kraken] Subscribed to PI_XBTUSD depth and trades");

        let mut depth_count = 0;
        let mut trade_count = 0;
        let target_events = 5;

        let timeout = tokio::time::timeout(Duration::from_secs(30), async {
            while depth_count < target_events {
                if let Some(event) = event_rx.recv().await {
                    match event {
                        MarketEvent::DepthUpdate { symbol, book } => {
                            depth_count += 1;
                            let view = book.as_view();
                            let (bid, ask) = view.get_bbo();
                            println!(
                                "[Kraken] [{}] DepthUpdate #{}: bid={}, ask={}",
                                symbol, depth_count, bid, ask
                            );
                        }
                        MarketEvent::TradeUpdate { symbol, trades } => {
                            trade_count += 1;
                            println!(
                                "[Kraken] [{}] TradeUpdate #{}: {} trades",
                                symbol,
                                trade_count,
                                trades.len()
                            );
                        }
                    }
                }
            }
        });

        match timeout.await {
            Ok(_) => {
                println!(
                    "[Kraken] Received {} depth updates and {} trade updates",
                    depth_count, trade_count
                );
                assert!(depth_count >= target_events, "Should receive depth updates");
            }
            Err(_) => {
                panic!(
                    "Timeout waiting for events. Got {} depth, {} trades",
                    depth_count, trade_count
                );
            }
        }
    }
}
