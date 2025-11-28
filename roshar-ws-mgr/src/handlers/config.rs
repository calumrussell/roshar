/// Configuration for connection handlers
#[derive(Clone, Debug)]
pub struct HandlerConfig {
    /// Size of the broadcast channel for each handler
    /// Default: 32_768
    pub channel_size: usize,
}

impl Default for HandlerConfig {
    fn default() -> Self {
        Self {
            channel_size: 32_768,
        }
    }
}

impl HandlerConfig {
    /// Create a new configuration with the default channel size
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the channel size for handlers
    pub fn with_channel_size(mut self, size: usize) -> Self {
        self.channel_size = size;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = HandlerConfig::default();
        assert_eq!(config.channel_size, 32_768);
    }

    #[test]
    fn test_new_config() {
        let config = HandlerConfig::new();
        assert_eq!(config.channel_size, 32_768);
    }

    #[test]
    fn test_with_channel_size() {
        let config = HandlerConfig::new().with_channel_size(65_536);
        assert_eq!(config.channel_size, 65_536);
    }
}
