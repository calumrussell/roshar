use reqwest::Client;
use std::sync::OnceLock;

static HTTP_CLIENT: OnceLock<Client> = OnceLock::new();

/// Get a shared HTTP client for all exchange API calls.
/// This client is created once and reused across all exchanges to:
/// - Prevent resource exhaustion from creating many clients
/// - Efficiently reuse connections via connection pooling
/// - Provide consistent timeout and pool settings
pub fn get_http_client() -> &'static Client {
    HTTP_CLIENT.get_or_init(|| {
        Client::builder()
            .pool_idle_timeout(std::time::Duration::from_secs(90))
            .pool_max_idle_per_host(10)
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client")
    })
}
