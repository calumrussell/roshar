use roshar_ws_mgr::{Config, Manager};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

fn get_shard_for_key(key: &str, num_shards: usize) -> usize {
    let mut hasher = DefaultHasher::new();
    key.hash(&mut hasher);
    (hasher.finish() as usize) % num_shards
}

#[tokio::main]
async fn main() {
    env_logger::init();

    println!("Testing DashMap deadlock reproduction with forced shard collision...");

    let manager = Manager::new();

    // Find connection names that hash to the same shard
    let num_shards = 16; // DashMap default
    let mut same_shard_names = Vec::new();

    // Try different names until we find ones in the same shard
    let test_names = vec![
        "bybit",
        "kraken",
        "hyperliquid",
        "binance",
        "conn0",
        "conn1",
        "conn2",
        "conn3",
        "conn4",
        "conn5",
    ];
    let target_shard = get_shard_for_key("bybit", num_shards);

    for name in test_names {
        if get_shard_for_key(name, num_shards) == target_shard {
            same_shard_names.push(name);
            println!("Found '{}' in shard {}", name, target_shard);
        }
    }

    if same_shard_names.len() < 2 {
        println!("Couldn't find enough names in same shard, using first 2 anyway");
        same_shard_names = vec!["bybit", "kraken"];
    }

    println!("Using names: {:?}", same_shard_names);

    // Create connections for same-shard names
    for name in &same_shard_names {
        let config = Config {
            name: name.to_string(),
            url: "wss://stream.binance.com:9443/ws/btcusdt@ticker".to_string(),
            reconnect_timeout: 1,
            ping_duration: 30,
            ping_message: "ping".to_string(),
            ping_timeout: 10,
            use_text_ping: None,
            read_buffer_size: None,
            write_buffer_size: None,
            max_message_size: None,
            max_frame_size: None,
            tcp_recv_buffer_size: None,
            tcp_send_buffer_size: None,
            tcp_nodelay: None,
            broadcast_channel_size: Some(8_192),
        };

        println!("Starting connection: {}", name);
        if let Err(e) = manager.new_conn(name, config) {
            println!("Failed to start {}: {}", name, e);
        }
    }

    // Wait for connections to establish
    println!("Waiting for connections to establish...");
    sleep(Duration::from_secs(2)).await;

    println!("Now triggering SIMULTANEOUS reconnects on same shard...");

    // Trigger simultaneous reconnects with no delay to maximize contention
    let mut handles = vec![];

    for name in &same_shard_names {
        let manager_clone = Arc::clone(&manager);
        let name_owned = name.to_string();

        let handle = tokio::spawn(async move {
            println!("Thread starting IMMEDIATE reconnect for: {}", name_owned);
            let start = std::time::Instant::now();
            match manager_clone.reconnect(&name_owned).await {
                Ok(_) => println!(
                    "Reconnect succeeded for: {} (took {:?})",
                    name_owned,
                    start.elapsed()
                ),
                Err(e) => println!(
                    "Reconnect failed for {}: {} (took {:?})",
                    name_owned,
                    e,
                    start.elapsed()
                ),
            }
        });

        handles.push(handle);
    }

    // Wait for all reconnects with timeout
    println!("Waiting for reconnects to complete (30 second timeout to detect deadlock)...");

    for (i, handle) in handles.into_iter().enumerate() {
        match tokio::time::timeout(Duration::from_secs(30), handle).await {
            Ok(Ok(())) => println!("Reconnect {} completed successfully", i),
            Ok(Err(e)) => println!("Reconnect {} task failed: {}", i, e),
            Err(_) => {
                println!(
                    "❌ Reconnect {} timed out after 30s - DEADLOCK DETECTED!",
                    i
                );
                println!("This confirms the DashMap shard-level deadlock issue!");
                return;
            }
        }
    }

    println!("✅ All reconnects completed without deadlock - issue may not be reproduced");
}
