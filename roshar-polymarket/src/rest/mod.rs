//! REST clients, one per Polymarket API host:
//!
//! - [`gamma`]: Gamma API (`https://gamma-api.polymarket.com`): events, markets, series, tags,
//!   search and sports metadata.
//! - [`clob`]: CLOB API (`https://clob.polymarket.com`), public reads only: order books, prices,
//!   midpoints, spreads, tick sizes, fee rates and price history.
//! - [`data`]: Data API v2 (`https://data-api.polymarket.com`), market-level reads only: trades,
//!   holders, open interest and live volume.

pub mod clob;
pub mod data;
pub mod gamma;

pub use clob::ClobApi;
pub use data::DataApi;
pub use gamma::GammaApi;

use roshar_http::RateLimitedClient;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fmt;

/// Result of every REST call. The error is boxed to keep the original `GammaApi` signature; it is
/// a [`PolymarketApiError`] for exchange-side failures (downcast with `err.downcast_ref`) or a
/// `reqwest::Error` for transport failures.
pub type ApiResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// Exchange-side failure: a non-2xx status (Polymarket bodies are usually `{"error": "..."}`) or
/// a 2xx body that does not match the expected type. The raw body is kept in both cases.
#[derive(Debug)]
pub enum PolymarketApiError {
    Status {
        method: reqwest::Method,
        path: String,
        status: u16,
        body: String,
    },
    Decode {
        method: reqwest::Method,
        path: String,
        error: serde_json::Error,
        body: String,
    },
}

impl fmt::Display for PolymarketApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Status {
                method,
                path,
                status,
                body,
            } => write!(f, "{method} {path} failed with status {status}: {body}"),
            Self::Decode {
                method,
                path,
                error,
                body,
            } => write!(
                f,
                "{method} {path}: failed to decode response ({error}): {body}"
            ),
        }
    }
}

impl std::error::Error for PolymarketApiError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Status { .. } => None,
            Self::Decode { error, .. } => Some(error),
        }
    }
}

/// Sends one request and decodes the JSON body strictly. `path` is appended to `base_url` and is
/// also used in error messages; `query` pairs are sent as-is (repeated keys allowed).
pub(crate) async fn send<T, B>(
    client: &RateLimitedClient,
    method: reqwest::Method,
    base_url: &str,
    path: &str,
    query: &[(&str, String)],
    body: Option<&B>,
) -> ApiResult<T>
where
    T: DeserializeOwned,
    B: Serialize + ?Sized,
{
    let url = format!("{base_url}{path}");
    let mut request = client.request(method.clone(), &url).await;
    if !query.is_empty() {
        request = request.query(query);
    }
    if let Some(body) = body {
        request = request.json(body);
    }
    let response = request.send().await?;
    let status = response.status();
    let text = response.text().await?;
    if !status.is_success() {
        return Err(Box::new(PolymarketApiError::Status {
            method,
            path: path.to_string(),
            status: status.as_u16(),
            body: text,
        }));
    }
    serde_json::from_str(&text).map_err(|error| {
        Box::new(PolymarketApiError::Decode {
            method,
            path: path.to_string(),
            error,
            body: text,
        }) as Box<dyn std::error::Error + Send + Sync>
    })
}

/// `GET base_url + path` with query pairs.
pub(crate) async fn get<T: DeserializeOwned>(
    client: &RateLimitedClient,
    base_url: &str,
    path: &str,
    query: &[(&str, String)],
) -> ApiResult<T> {
    send::<T, ()>(client, reqwest::Method::GET, base_url, path, query, None).await
}

/// `POST base_url + path` with a JSON body.
pub(crate) async fn post<T: DeserializeOwned, B: Serialize + ?Sized>(
    client: &RateLimitedClient,
    base_url: &str,
    path: &str,
    body: &B,
) -> ApiResult<T> {
    send(
        client,
        reqwest::Method::POST,
        base_url,
        path,
        &[],
        Some(body),
    )
    .await
}

/// Query builder shared by the param structs: `None` values are skipped, lists become repeated
/// keys unless the caller joins them.
#[derive(Debug, Default)]
pub(crate) struct Query(pub(crate) Vec<(&'static str, String)>);

impl Query {
    pub(crate) fn opt<T: ToString>(&mut self, key: &'static str, value: &Option<T>) -> &mut Self {
        if let Some(v) = value {
            self.0.push((key, v.to_string()));
        }
        self
    }

    pub(crate) fn req<T: ToString>(&mut self, key: &'static str, value: &T) -> &mut Self {
        self.0.push((key, value.to_string()));
        self
    }

    /// Repeated keys: `key=a&key=b`.
    pub(crate) fn many<T: ToString>(
        &mut self,
        key: &'static str,
        values: &Option<Vec<T>>,
    ) -> &mut Self {
        if let Some(values) = values {
            self.0.extend(values.iter().map(|v| (key, v.to_string())));
        }
        self
    }

    /// One comma-joined value: `key=a,b`.
    pub(crate) fn joined<T: ToString>(
        &mut self,
        key: &'static str,
        values: &Option<Vec<T>>,
    ) -> &mut Self {
        if let Some(values) = values {
            let joined: Vec<String> = values.iter().map(ToString::to_string).collect();
            self.0.push((key, joined.join(",")));
        }
        self
    }
}
