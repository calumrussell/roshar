use roshar_clients::{HyperliquidClient, HyperliquidConfig, MarketEvent};
use roshar_ws_mgr::Manager;
use std::sync::Arc;
use std::time::Duration;

/// Subscribes to BTC depth, waits for data, then polls get_latest_depth
#[tokio::test]
#[ignore] // Run with: cargo test --test market_data_feed_test -- --ignored --nocapture
async fn test_pattern_a_polling() {
    let _ = env_logger::try_init();

    let config = HyperliquidConfig {
        is_mainnet: true,
        metadata_update_interval_secs: 300,
        wallet_address: None,
    };

    let ws_manager: Arc<Manager> = Manager::new();
    let client = HyperliquidClient::new(config, ws_manager);

    // Subscribe to BTC depth
    client
        .create_depth_subscription("BTC")
        .await
        .expect("Failed to create depth subscription");

    println!("Subscribed to BTC depth, waiting for data...");

    // Wait for data to arrive
    tokio::time::sleep(Duration::from_secs(3)).await;

    // Poll for latest depth
    let mut attempts = 0;
    loop {
        if let Some(book) = client.get_latest_depth("BTC") {
            let view = book.as_view();
            let (bid, ask) = view.get_bbo();
            println!("BTC BBO: bid={}, ask={}", bid, ask);
            println!("Bid levels: {}", view.bid_prices().len());
            println!("Ask levels: {}", view.ask_prices().len());
            assert!(!bid.is_empty(), "Bid should not be empty");
            assert!(!ask.is_empty(), "Ask should not be empty");
            break;
        }

        attempts += 1;
        if attempts > 10 {
            panic!("Failed to receive depth data after 10 attempts");
        }

        println!("No data yet, waiting...");
        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    // Cleanup
    client
        .remove_depth_subscription("BTC")
        .await
        .expect("Failed to remove subscription");
}

/// Subscribes to BTC depth and trades, receives events via channel
#[tokio::test]
#[ignore] // Run with: cargo test --test market_data_feed_test -- --ignored --nocapture
async fn test_pattern_b_reactive() {
    let _ = env_logger::try_init();

    let config = HyperliquidConfig {
        is_mainnet: true,
        metadata_update_interval_secs: 300,
        wallet_address: None,
    };

    let ws_manager: Arc<Manager> = Manager::new();
    let mut client = HyperliquidClient::new(config, ws_manager);

    // Take the event receiver
    let mut rx = client
        .take_market_events_receiver()
        .expect("Failed to take event receiver");

    // Subscribe to BTC depth and trades
    client
        .create_depth_subscription("BTC")
        .await
        .expect("Failed to create depth subscription");
    client
        .create_trades_subscription("BTC")
        .await
        .expect("Failed to create trades subscription");

    println!("Subscribed to BTC depth and trades, waiting for events...");

    let mut depth_count = 0;
    let mut trade_count = 0;
    let target_events = 5;

    let timeout = tokio::time::timeout(Duration::from_secs(30), async {
        while depth_count < target_events {
            if let Some(event) = rx.recv().await {
                match event {
                    MarketEvent::DepthUpdate { coin, book } => {
                        depth_count += 1;
                        let view = book.as_view();
                        let (bid, ask) = view.get_bbo();
                        println!(
                            "[{}] DepthUpdate #{}: bid={}, ask={}",
                            coin, depth_count, bid, ask
                        );
                    }
                    MarketEvent::TradeUpdate { coin, trades } => {
                        trade_count += 1;
                        println!(
                            "[{}] TradeUpdate #{}: {} trades",
                            coin,
                            trade_count,
                            trades.len()
                        );
                        for trade in trades.iter().take(3) {
                            println!(
                                "  - {} {} @ {} (side={})",
                                trade.sz, trade.coin, trade.px, trade.side
                            );
                        }
                    }
                }
            }
        }
    });

    match timeout.await {
        Ok(_) => {
            println!(
                "Received {} depth updates and {} trade updates",
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

/// Test multiple coin subscriptions
#[tokio::test]
#[ignore] // Run with: cargo test --test market_data_feed_test -- --ignored --nocapture
async fn test_multiple_coins() {
    let _ = env_logger::try_init();

    let config = HyperliquidConfig {
        is_mainnet: true,
        metadata_update_interval_secs: 300,
        wallet_address: None,
    };

    let ws_manager: Arc<Manager> = Manager::new();
    let mut client = HyperliquidClient::new(config, ws_manager);

    let mut rx = client
        .take_market_events_receiver()
        .expect("Failed to take event receiver");

    // Subscribe to multiple coins
    let coins = vec!["BTC", "ETH", "SOL"];
    for coin in &coins {
        client
            .create_depth_subscription(coin)
            .await
            .expect(&format!("Failed to subscribe to {}", coin));
        println!("Subscribed to {} depth", coin);
    }

    // Track which coins we've seen
    let mut seen_coins = std::collections::HashSet::new();

    let timeout = tokio::time::timeout(Duration::from_secs(30), async {
        while seen_coins.len() < coins.len() {
            if let Some(event) = rx.recv().await {
                if let MarketEvent::DepthUpdate { coin, book } = event {
                    if !seen_coins.contains(&coin) {
                        let view = book.as_view();
                        let (bid, ask) = view.get_bbo();
                        println!("{}: bid={}, ask={}", coin, bid, ask);
                        seen_coins.insert(coin);
                    }
                }
            }
        }
    });

    match timeout.await {
        Ok(_) => {
            println!("Received data for all coins: {:?}", seen_coins);
            assert_eq!(seen_coins.len(), coins.len());
        }
        Err(_) => {
            panic!("Timeout waiting for all coins. Only got: {:?}", seen_coins);
        }
    }
}

/// Test dynamic subscription add/remove
#[tokio::test]
#[ignore] // Run with: cargo test --test market_data_feed_test -- --ignored --nocapture
async fn test_dynamic_subscriptions() {
    let _ = env_logger::try_init();

    let config = HyperliquidConfig {
        is_mainnet: true,
        metadata_update_interval_secs: 300,
        wallet_address: None,
    };

    let ws_manager: Arc<Manager> = Manager::new();
    let client = HyperliquidClient::new(config, ws_manager);

    // Subscribe to BTC
    client
        .create_depth_subscription("BTC")
        .await
        .expect("Failed to subscribe to BTC");
    println!("Subscribed to BTC");

    // Wait for data
    tokio::time::sleep(Duration::from_secs(3)).await;

    // Verify we have BTC data
    assert!(
        client.get_latest_depth("BTC").is_some(),
        "Should have BTC data"
    );
    println!("Got BTC data");

    // Subscribe to ETH dynamically
    client
        .create_depth_subscription("ETH")
        .await
        .expect("Failed to subscribe to ETH");
    println!("Subscribed to ETH");

    // Wait for ETH data
    tokio::time::sleep(Duration::from_secs(3)).await;

    // Verify we have both
    assert!(
        client.get_latest_depth("BTC").is_some(),
        "Should still have BTC data"
    );
    assert!(
        client.get_latest_depth("ETH").is_some(),
        "Should have ETH data"
    );
    println!("Got both BTC and ETH data");

    // Remove BTC subscription
    client
        .remove_depth_subscription("BTC")
        .await
        .expect("Failed to remove BTC subscription");
    println!("Removed BTC subscription");

    // Wait for the command to be processed
    tokio::time::sleep(Duration::from_millis(500)).await;

    // BTC data should be gone after command is processed
    assert!(
        client.get_latest_depth("BTC").is_none(),
        "BTC data should be removed"
    );
    assert!(
        client.get_latest_depth("ETH").is_some(),
        "ETH data should still be there"
    );
    println!("BTC removed, ETH still present");
}
