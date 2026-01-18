use leaky_bucket::RateLimiter;
use reqwest::Client;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

static HTTP_CLIENT: OnceLock<Client> = OnceLock::new();

/// Get a shared HTTP client for all exchange API calls.
/// This client is created once and reused across all exchanges to:
/// - Prevent resource exhaustion from creating many clients
/// - Efficiently reuse connections via connection pooling
/// - Provide consistent timeout and pool settings
///
/// Note: This client has no rate limiting. For rate-limited requests,
/// use `RateLimitedClient` instead.
pub fn get_http_client() -> &'static Client {
    HTTP_CLIENT.get_or_init(|| {
        Client::builder()
            .pool_idle_timeout(Duration::from_secs(90))
            .pool_max_idle_per_host(10)
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client")
    })
}

/// A rate-limited HTTP client wrapper.
///
/// This wraps a reqwest Client with a leaky bucket rate limiter.
/// Before each request, it acquires a token from the rate limiter,
/// blocking if the rate limit would be exceeded.
#[derive(Clone)]
pub struct RateLimitedClient {
    client: Client,
    rate_limiter: Arc<RateLimiter>,
}

impl RateLimitedClient {
    /// Create a new rate-limited client.
    ///
    /// # Arguments
    /// * `requests_per_second` - Maximum requests per second
    ///
    /// # Example
    /// ```ignore
    /// let client = RateLimitedClient::new(10); // 10 req/sec
    /// let response = client.get("https://api.example.com").await?;
    /// ```
    pub fn new(requests_per_second: u32) -> Self {
        let client = Client::builder()
            .pool_idle_timeout(Duration::from_secs(90))
            .pool_max_idle_per_host(10)
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        let rate_limiter = RateLimiter::builder()
            .initial(requests_per_second as usize)
            .max(requests_per_second as usize)
            .refill(requests_per_second as usize)
            .interval(Duration::from_secs(1))
            .build();

        Self {
            client,
            rate_limiter: Arc::new(rate_limiter),
        }
    }

    /// Acquire a rate limit token and make a GET request.
    pub async fn get(&self, url: &str) -> Result<reqwest::Response, reqwest::Error> {
        self.rate_limiter.acquire_one().await;
        self.client.get(url).send().await
    }

    /// Acquire a rate limit token and return a POST request builder.
    /// The token is acquired before returning the builder.
    pub async fn post(&self, url: &str) -> reqwest::RequestBuilder {
        self.rate_limiter.acquire_one().await;
        self.client.post(url)
    }

    /// Acquire a rate limit token and return a RequestBuilder for any HTTP method.
    /// The token is acquired before returning the builder.
    pub async fn request(&self, method: reqwest::Method, url: &str) -> reqwest::RequestBuilder {
        self.rate_limiter.acquire_one().await;
        self.client.request(method, url)
    }

    /// Get the underlying client for requests that need custom configuration.
    /// Note: This bypasses rate limiting - use `acquire()` first if rate limiting is needed.
    pub fn inner(&self) -> &Client {
        &self.client
    }

    /// Manually acquire a rate limit token.
    /// Use this when you need to make a request with custom configuration.
    pub async fn acquire(&self) {
        self.rate_limiter.acquire_one().await;
    }
}
