macro_rules! ws_config_methods {
    () => {
        pub fn ws_config(&self) -> roshar_ws_mgr::Config {
            self.ws_config.clone()
        }

        pub fn set_ws_url(&mut self, url: &str) {
            self.ws_config.url = url.to_string();
        }

        pub fn set_ping_duration(&mut self, seconds: u64) {
            self.ws_config.ping_duration = seconds;
        }

        pub fn set_ping_timeout(&mut self, seconds: u64) {
            self.ws_config.ping_timeout = seconds;
        }

        pub fn set_reconnect_timeout(&mut self, timeout: u64) {
            self.ws_config.reconnect_timeout = timeout;
        }

        pub fn set_read_buffer_size(&mut self, size: usize) {
            self.ws_config.read_buffer_size = Some(size);
        }

        pub fn set_write_buffer_size(&mut self, size: usize) {
            self.ws_config.write_buffer_size = Some(size);
        }

        pub fn set_max_message_size(&mut self, size: usize) {
            self.ws_config.max_message_size = Some(size);
        }

        pub fn set_max_frame_size(&mut self, size: usize) {
            self.ws_config.max_frame_size = Some(size);
        }

        pub fn set_tcp_recv_buffer_size(&mut self, size: usize) {
            self.ws_config.tcp_recv_buffer_size = Some(size);
        }

        pub fn set_tcp_send_buffer_size(&mut self, size: usize) {
            self.ws_config.tcp_send_buffer_size = Some(size);
        }

        pub fn set_tcp_nodelay(&mut self, nodelay: bool) {
            self.ws_config.tcp_nodelay = Some(nodelay);
        }
    };
}
pub(crate) use ws_config_methods;
