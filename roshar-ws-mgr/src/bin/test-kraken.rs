use std::time::Duration;

use anyhow::Result;
use log::info;
use roshar_ws_mgr::{Config, Manager, Message};
use tokio::time::sleep;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    info!("Starting Kraken WebSocket depth feed test");

    let manager = Manager::new();

    // Unified WebSocket v2 subscribe (Kraken deprecated futures.kraken.com/ws/v1; shut down 2027-03-01).
    let subscribe_json = r#"{"method":"subscribe","params":{"channel":"book","asset_class":"derivatives","symbol":["PF_ETHUSD"],"depth":10,"snapshot":true}}"#.to_string();

    let config = Config {
        name: "kraken".to_string(),
        url: "wss://uws.kraken.com/v2/public".to_string(),
        ping_duration: 20,
        ping_message: r#"{"method":"ping"}"#.to_string(),
        ping_timeout: 45,
        reconnect_timeout: 5,
        use_text_ping: Some(false),
        read_buffer_size: None,
        write_buffer_size: None,
        max_message_size: None,
        max_frame_size: None,
        tcp_recv_buffer_size: None,
        tcp_send_buffer_size: None,
        tcp_nodelay: None,
        broadcast_channel_size: Some(32_768),
        headers: vec![],
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
