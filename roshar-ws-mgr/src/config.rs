#[derive(Clone, Debug)]
pub struct Config {
    pub name: String,
    pub url: String,
    pub ping_duration: u64,
    pub ping_message: String,
    pub ping_timeout: u64,
    pub reconnect_timeout: u64,
    pub use_text_ping: Option<bool>,
    pub read_buffer_size: Option<usize>,
    pub write_buffer_size: Option<usize>,
    pub max_message_size: Option<usize>,
    pub max_frame_size: Option<usize>,
    pub tcp_recv_buffer_size: Option<usize>,
    pub tcp_send_buffer_size: Option<usize>,
    pub tcp_nodelay: Option<bool>,
    pub broadcast_channel_size: Option<usize>,
}
