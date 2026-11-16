pub mod market;
pub mod trading;
pub use market::MarketApi;
pub use trading::TradingApi;

use crate::types::{
    NadoArchiveError, NadoEdgeTime, NadoExecuteReply, NadoQueryReply, NadoReplyStatus,
};
use crate::NadoNetwork;
use roshar_http::RateLimitedClient;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fmt;

pub type BoxError = Box<dyn std::error::Error + Send + Sync>;
pub type ApiResult<T> = Result<T, BoxError>;

/// A request Nado rejected. Returned (boxed) by every REST call; downcast to inspect
/// `error_code`.
#[derive(Debug, Clone)]
pub struct NadoApiError {
    /// What was requested, e.g. `query_market_liquidity`, `execute_place_order`,
    /// `archive candlesticks`.
    pub request: String,
    pub http_status: u16,
    /// The exchange's numeric code, when the reply carried one.
    pub error_code: Option<i64>,
    /// The exchange's message, or the raw body when it was not a JSON error envelope.
    pub error: String,
}

impl fmt::Display for NadoApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.error_code {
            Some(code) => write!(
                f,
                "Nado {} failed (HTTP {}, code {code}): {}",
                self.request, self.http_status, self.error
            ),
            None => write!(
                f,
                "Nado {} failed (HTTP {}): {}",
                self.request, self.http_status, self.error
            ),
        }
    }
}

impl std::error::Error for NadoApiError {}

fn parse_error(what: &str, text: &str, e: serde_json::Error) -> BoxError {
    format!("Failed to parse Nado {what} reply: {e}: {text}").into()
}

/// HTTP transport for one network. Requests are POSTed as JSON, or GET for the V2 endpoints.
///
/// `RateLimitedClient` builds a reqwest 0.11 client with the `gzip` feature enabled by this crate,
/// so every request carries `Accept-Encoding: gzip`; Nado answers 403 to requests without it.
#[derive(Clone)]
pub(crate) struct Transport {
    client: RateLimitedClient,
    network: NadoNetwork,
}

impl Transport {
    pub(crate) fn new(client: RateLimitedClient, network: NadoNetwork) -> Self {
        Self { client, network }
    }

    pub(crate) fn network(&self) -> NadoNetwork {
        self.network
    }

    async fn post_text<B: Serialize + ?Sized>(
        &self,
        url: &str,
        body: &B,
    ) -> ApiResult<(u16, String)> {
        let response = self.client.post(url).await.json(body).send().await?;
        let status = response.status().as_u16();
        let text = response.text().await?;
        Ok((status, text))
    }

    async fn get_text(&self, url: &str) -> ApiResult<(u16, String)> {
        let response = self.client.get(url).await?;
        let status = response.status().as_u16();
        let text = response.text().await?;
        Ok((status, text))
    }

    /// `POST {gateway}/query` with `{"type": .., ..}`; returns `data` of a `success` reply.
    pub(crate) async fn query<T: DeserializeOwned, B: Serialize + ?Sized>(
        &self,
        body: &B,
        what: &str,
    ) -> ApiResult<T> {
        let url = format!("{}/query", self.network.gateway_url());
        let (status, text) = self.post_text(&url, body).await?;
        parse_query_reply(status, &text, what)
    }

    /// `POST {gateway}/execute`; returns the reply of a `success` execute.
    pub(crate) async fn execute<T: DeserializeOwned, B: Serialize + ?Sized>(
        &self,
        body: &B,
        what: &str,
    ) -> ApiResult<NadoExecuteReply<T>> {
        let url = format!("{}/execute", self.network.gateway_url());
        let (status, text) = self.post_text(&url, body).await?;
        parse_execute_reply(status, &text, what)
    }

    /// `POST {gateway}/edge/query` with `{"type": "cached_*", ..}`; returns `data` of a
    /// `success` reply (same envelope as live queries).
    pub(crate) async fn edge_query<T: DeserializeOwned, B: Serialize + ?Sized>(
        &self,
        body: &B,
        what: &str,
    ) -> ApiResult<T> {
        let url = format!("{}/edge/query", self.network.gateway_url());
        let (status, text) = self.post_text(&url, body).await?;
        parse_query_reply(status, &text, what)
    }

    /// `POST {gateway}/edge/query` with `{"type": "ping" | "time", ..}`.
    pub(crate) async fn edge_control<B: Serialize + ?Sized>(
        &self,
        body: &B,
        what: &str,
    ) -> ApiResult<NadoEdgeTime> {
        let url = format!("{}/edge/query", self.network.gateway_url());
        let (status, text) = self.post_text(&url, body).await?;
        parse_edge_control_reply(status, &text, what)
    }

    /// `POST {archive}` with `{"<query>": {..}}`.
    pub(crate) async fn archive<T: DeserializeOwned, B: Serialize + ?Sized>(
        &self,
        body: &B,
        what: &str,
    ) -> ApiResult<T> {
        let (status, text) = self.post_text(self.network.archive_url(), body).await?;
        parse_archive_reply(status, &text, what)
    }

    /// `GET {base}/{path_and_query}` for the V2 REST endpoints.
    pub(crate) async fn get_v2<T: DeserializeOwned>(&self, url: &str, what: &str) -> ApiResult<T> {
        let (status, text) = self.get_text(url).await?;
        parse_archive_reply(status, &text, what)
    }
}

/// Parses a gateway query reply. A non-2xx status, `status: failure` or a missing `data` is an
/// error.
pub fn parse_query_reply<T: DeserializeOwned>(
    http_status: u16,
    text: &str,
    what: &str,
) -> ApiResult<T> {
    let reply: NadoQueryReply<T> = match serde_json::from_str(text) {
        Ok(r) => r,
        Err(e) if (200..300).contains(&http_status) => return Err(parse_error(what, text, e)),
        Err(_) => {
            return Err(Box::new(NadoApiError {
                request: what.to_string(),
                http_status,
                error_code: None,
                error: text.to_string(),
            }))
        }
    };
    if reply.status == NadoReplyStatus::Failure || !(200..300).contains(&http_status) {
        return Err(Box::new(NadoApiError {
            request: reply.request_type,
            http_status,
            error_code: reply.error_code,
            error: reply.error.unwrap_or_else(|| text.to_string()),
        }));
    }
    reply
        .data
        .ok_or_else(|| format!("Nado {what} reply has no data: {text}").into())
}

/// Parses an execute reply. A non-2xx status or `status: failure` is an error; `data` is left
/// optional because some executes (`link_signer`) return none.
pub fn parse_execute_reply<T: DeserializeOwned>(
    http_status: u16,
    text: &str,
    what: &str,
) -> ApiResult<NadoExecuteReply<T>> {
    let reply: NadoExecuteReply<T> = match serde_json::from_str(text) {
        Ok(r) => r,
        Err(e) if (200..300).contains(&http_status) => return Err(parse_error(what, text, e)),
        Err(_) => {
            return Err(Box::new(NadoApiError {
                request: what.to_string(),
                http_status,
                error_code: None,
                error: text.to_string(),
            }))
        }
    };
    if reply.status == NadoReplyStatus::Failure || !(200..300).contains(&http_status) {
        return Err(Box::new(NadoApiError {
            request: reply.request_type,
            http_status,
            error_code: reply.error_code,
            error: reply.error.unwrap_or_else(|| text.to_string()),
        }));
    }
    Ok(reply)
}

#[derive(serde::Deserialize)]
struct EdgeControlStatus {
    status: NadoReplyStatus,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    error_code: Option<i64>,
}

/// Parses the flat reply of an edge `ping` / `time` control message. A non-2xx status or
/// `status: failure` is an error.
pub fn parse_edge_control_reply(
    http_status: u16,
    text: &str,
    what: &str,
) -> ApiResult<NadoEdgeTime> {
    let fail = |error_code: Option<i64>, error: Option<String>| -> BoxError {
        Box::new(NadoApiError {
            request: what.to_string(),
            http_status,
            error_code,
            error: error.unwrap_or_else(|| text.to_string()),
        })
    };
    let head: EdgeControlStatus = match serde_json::from_str(text) {
        Ok(h) => h,
        Err(e) if (200..300).contains(&http_status) => return Err(parse_error(what, text, e)),
        Err(_) => return Err(fail(None, None)),
    };
    if head.status == NadoReplyStatus::Failure || !(200..300).contains(&http_status) {
        return Err(fail(head.error_code, head.error));
    }
    serde_json::from_str(text).map_err(|e| parse_error(what, text, e))
}

/// Parses an archive or V2 reply: the body itself on 2xx, `{"error","error_code"}` or plain text
/// otherwise.
pub fn parse_archive_reply<T: DeserializeOwned>(
    http_status: u16,
    text: &str,
    what: &str,
) -> ApiResult<T> {
    if !(200..300).contains(&http_status) {
        let (error_code, error) = match serde_json::from_str::<NadoArchiveError>(text) {
            Ok(e) => (Some(e.error_code), e.error),
            Err(_) => (None, text.to_string()),
        };
        return Err(Box::new(NadoApiError {
            request: what.to_string(),
            http_status,
            error_code,
            error,
        }));
    }
    serde_json::from_str(text).map_err(|e| parse_error(what, text, e))
}
