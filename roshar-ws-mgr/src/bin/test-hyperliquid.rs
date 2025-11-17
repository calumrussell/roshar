use std::sync::Arc;
use std::time::Duration;

use anyhow::{Result, anyhow};
use roshar_ws_mgr::{Config, Manager, Message};
use log::{error, info};
use serde::{Deserialize, Serialize};
use tokio::time::sleep;

#[derive(Debug, Serialize, Deserialize, Clone)]
struct HyperliquidSubscribe {
    method: String,
    subscription: HyperliquidSubscriptionMessage,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct HyperliquidSubscriptionMessage {
    #[serde(rename = "type")]
    typ: String,
    coin: String,
    interval: String,
}

async fn test_connections_and_reconnect(
    manager: Arc<Manager>,
    config: Config,
    _subscribe_json: String,
) -> Result<()> {
    info!("Starting simple connection and reconnect test");
    let num_conns = 3;

    for i in 0..num_conns {
        let conn_name = format!("test-{}", i);
        manager.new_conn(&conn_name, config.clone())?;
    }

    info!("Waiting for connections to establish...");
    sleep(Duration::from_secs(3)).await;

    info!("Waiting for initial data...");
    sleep(Duration::from_secs(30)).await;

    info!("Testing reconnect...");
    for i in 0..num_conns {
        let conn_name = format!("test-{}", i);
        match manager.reconnect(&conn_name).await {
            Ok(_) => info!("Reconnect initiated for {}", conn_name),
            Err(e) => error!("Reconnect failed for {}: {:?}", conn_name, e),
        }
    }

    info!("Waiting for reconnection...");
    sleep(Duration::from_secs(8)).await;

    info!("Final data collection...");
    sleep(Duration::from_secs(10)).await;

    info!("Test completed");
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    info!("Starting Hyperliquid WebSocket test");

    let manager = Manager::new();

    let config = Config {
        name: "hyperliquid".to_string(),
        url: "wss://api.hyperliquid.xyz/ws".to_string(),
        ping_duration: 10,
        ping_message: r#"{"method":"ping"}"#.to_string(),
        ping_timeout: 30,
        reconnect_timeout: 2,
        use_text_ping: None,
        read_buffer_size: None,
        write_buffer_size: None,
        max_message_size: None,
        max_frame_size: None,
        tcp_recv_buffer_size: None,
        tcp_send_buffer_size: None,
        tcp_nodelay: None,
        broadcast_channel_size: Some(16_384),
    };

    let subscribe = HyperliquidSubscribe {
        method: "subscribe".to_string(),
        subscription: HyperliquidSubscriptionMessage {
            typ: "candle".to_string(),
            coin: "BTC".to_string(),
            interval: "1m".to_string(),
        },
    };
    let subscribe_json = serde_json::to_string(&subscribe)?;

    // Two-stage setup: first setup reader, then create connection
    let mut read_channel = manager.setup_reader("hyperliquid", 16_384);
    let manager_clone = Arc::clone(&manager);
    let subscribe_json_clone = subscribe_json.clone();
    tokio::spawn(async move {
        loop {
            match read_channel.recv().await {
                Ok(Message::SuccessfulHandshake(name)) => {
                    info!("[Global Log] Received: SuccessfulHandshake({})", name);
                    if name.starts_with("test-") {
                        let subscription_msg =
                            Message::TextMessage(name.clone(), subscribe_json_clone.clone());
                        match manager_clone.write(&name, subscription_msg) {
                            Ok(_) => info!("[Auto-Subscribe] Subscribed to {}", name),
                            Err(e) => {
                                error!("[Auto-Subscribe] Failed to subscribe to {}: {:?}", name, e)
                            }
                        }
                    }
                }
                Ok(msg) => info!("[Global Log] Received: {:?}", msg),
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    error!("[Global Log] Lagged by {} messages", n);
                }
                Err(_) => break,
            }
        }
    });

    let test_timeout = Duration::from_secs(90);
    match tokio::time::timeout(
        test_timeout,
        test_connections_and_reconnect(manager, config, subscribe_json),
    )
    .await
    {
        Ok(Ok(_)) => info!("Test passed."),
        Ok(Err(e)) => {
            error!("Test failed: {:?}", e);
            return Err(e);
        }
        Err(_) => {
            error!("Test timed out");
            return Err(anyhow!("Test timed out"));
        }
    }

    Ok(())
}
