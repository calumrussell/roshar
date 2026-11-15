//! Market-data websocket configuration for roshar-ws-mgr consumers.
//! Subscription messages are `roshar_types::HyperliquidWssMessage`.

pub const HL_WSS_URL: &str = "wss://api.hyperliquid.xyz/ws";
pub const HL_TESTNET_WSS_URL: &str = "wss://api.hyperliquid-testnet.xyz/ws";

/// Connection config for the public market-data websocket.
#[cfg(feature = "ws-mgr")]
pub fn ws_config(mainnet: bool) -> roshar_ws_mgr::Config {
    roshar_ws_mgr::Config {
        name: "hyperliquid".to_string(),
        url: if mainnet {
            HL_WSS_URL
        } else {
            HL_TESTNET_WSS_URL
        }
        .to_string(),
        ping_duration: 20,
        ping_message: roshar_types::HyperliquidWssMessage::ping().to_json(),
        ping_timeout: 10,
        reconnect_timeout: 5000,
        use_text_ping: Some(true),
        read_buffer_size: None,
        write_buffer_size: None,
        max_message_size: None,
        max_frame_size: None,
        tcp_recv_buffer_size: None,
        tcp_send_buffer_size: None,
        tcp_nodelay: None,
        broadcast_channel_size: None,
    }
}
