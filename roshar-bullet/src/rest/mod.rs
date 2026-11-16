pub mod account;
pub mod market;

pub use account::AccountApi;
pub use market::MarketApi;

use crate::types::BulletApiError;
use roshar_http::RateLimitedClient;
use serde::de::DeserializeOwned;
use std::fmt::{self, Display};

/// A failed REST call.
#[derive(Debug)]
pub enum BulletRestError {
    /// Non-2xx with the API's `{"status","message",...}` envelope.
    Api {
        http_status: u16,
        error: BulletApiError,
    },
    /// Non-2xx with a body that is not the envelope (e.g. query-string rejections, edge 429s).
    Status { http_status: u16, body: String },
    /// Connection, TLS or timeout failure.
    Transport(String),
    /// 2xx body that does not match the documented type.
    Parse {
        what: &'static str,
        error: String,
        body: String,
    },
}

impl Display for BulletRestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Api { http_status, error } => write!(
                f,
                "Bullet API error {http_status} (status {}): {}",
                error.status, error.message
            ),
            Self::Status { http_status, body } => write!(f, "Bullet HTTP {http_status}: {body}"),
            Self::Transport(e) => write!(f, "Bullet request failed: {e}"),
            Self::Parse { what, error, body } => {
                write!(f, "failed to parse Bullet {what}: {error}; body: {body}")
            }
        }
    }
}

impl std::error::Error for BulletRestError {}

pub type RestResult<T> = Result<T, BulletRestError>;

/// Builds `{base}{path}?k=v&...`, skipping unset parameters.
pub(crate) struct Query {
    pub(crate) url: String,
    has_params: bool,
}

impl Query {
    pub(crate) fn new(base: &str, path: &str) -> Self {
        Self {
            url: format!("{base}{path}"),
            has_params: false,
        }
    }

    pub(crate) fn param(mut self, key: &str, value: impl Display) -> Self {
        self.url.push(if self.has_params { '&' } else { '?' });
        self.url.push_str(key);
        self.url.push('=');
        self.url.push_str(&value.to_string());
        self.has_params = true;
        self
    }

    pub(crate) fn opt(self, key: &str, value: Option<impl Display>) -> Self {
        match value {
            Some(v) => self.param(key, v),
            None => self,
        }
    }
}

/// Shared transport: one rate limiter per client, applied to every REST call.
#[derive(Clone)]
pub(crate) struct Http {
    client: RateLimitedClient,
    pub(crate) base_url: String,
}

impl Http {
    pub(crate) fn new(client: RateLimitedClient, base_url: &str) -> Self {
        Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
        }
    }

    pub(crate) fn query(&self, path: &str) -> Query {
        Query::new(&self.base_url, path)
    }

    pub(crate) async fn get<T: DeserializeOwned>(
        &self,
        url: &str,
        what: &'static str,
    ) -> RestResult<T> {
        let response = self
            .client
            .get(url)
            .await
            .map_err(|e| BulletRestError::Transport(e.to_string()))?;
        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|e| BulletRestError::Transport(e.to_string()))?;
        Self::decode(status.is_success(), status.as_u16(), body, what)
    }

    pub(crate) async fn post_json<T: DeserializeOwned>(
        &self,
        path: &str,
        body: String,
        what: &'static str,
    ) -> RestResult<T> {
        let url = format!("{}{path}", self.base_url);
        let response = self
            .client
            .post(&url)
            .await
            .header("content-type", "application/json")
            .body(body)
            .send()
            .await
            .map_err(|e| BulletRestError::Transport(e.to_string()))?;
        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|e| BulletRestError::Transport(e.to_string()))?;
        Self::decode(status.is_success(), status.as_u16(), body, what)
    }

    fn decode<T: DeserializeOwned>(
        success: bool,
        http_status: u16,
        body: String,
        what: &'static str,
    ) -> RestResult<T> {
        if !success {
            return Err(match serde_json::from_str::<BulletApiError>(&body) {
                Ok(error) => BulletRestError::Api { http_status, error },
                Err(_) => BulletRestError::Status { http_status, body },
            });
        }
        serde_json::from_str(&body).map_err(|e| BulletRestError::Parse {
            what,
            error: e.to_string(),
            body,
        })
    }
}
