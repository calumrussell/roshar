use std::time::Duration;

use anyhow::Result;
use log::info;
use roshar_ws_mgr::{Config, Manager, Message};
use serde::{Deserialize, Serialize};
use tokio::time::sleep;

#[derive(Debug, Serialize, Deserialize)]
struct KrakenSubscribe {
    event: String,
    product_ids: Vec<String>,
    feed: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    info!("Starting Kraken WebSocket depth feed test");

    let manager = Manager::new();

    let subscribe = KrakenSubscribe {
        event: "subscribe".to_string(),
        feed: "book".to_string(),
        product_ids: vec!["PF_ETHUSD".to_string()],
    };

    let subscribe_json = serde_json::to_string(&subscribe)?;

    let config = Config {
        name: "kraken".to_string(),
        url: "wss://futures.kraken.com/ws/v1".to_string(),
        ping_duration: 30,
        ping_message: r#"{"event":"ping"}"#.to_string(),
        ping_timeout: 60,
        reconnect_timeout: 5,
        use_text_ping: None,
        read_buffer_size: None,
        write_buffer_size: None,
        max_message_size: None,
        max_frame_size: None,
        tcp_recv_buffer_size: None,
        tcp_send_buffer_size: None,
        tcp_nodelay: None,
        broadcast_channel_size: Some(32_768),
    };

    // Two-stage setup: first setup reader, then create connection
    let mut read_channel = manager.setup_reader("kraken", 32_768);
    manager.new_conn("kraken", config)?;
    tokio::spawn(async move {
        while let Ok(msg) = read_channel.recv().await {
            info!("Received message: {:?}", msg);
        }
    });

    sleep(Duration::from_secs(2)).await;
    let subscription_msg = Message::TextMessage("kraken".to_string(), subscribe_json);
    manager.write("kraken", subscription_msg)?;

    info!("Listening to depth feed for 15 seconds...");
    sleep(Duration::from_secs(15)).await;

    info!("Closing connection...");
    manager.close_conn("kraken")?;

    Ok(())
}
