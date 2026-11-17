pub mod market;
pub mod trading;

use serde::de::DeserializeOwned;
use serde::Deserialize;
use std::fmt::{self, Display};

pub type ApiResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// A non-2xx reply. Single-object endpoints return `{"status":"err","error":..}`; batch endpoints
/// return the same object as the only element of an array.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PolymarketPerpsApiError {
    pub http_status: u16,
    /// The exchange's error identifier or validation detail (`unauthorized`,
    /// `invalid signature`, `ip_rate_limited`, ...). Holds the raw body when it is not an error
    /// envelope.
    pub error: String,
    /// Support trace id (`ref`), when sent.
    pub reference: Option<String>,
}

impl Display for PolymarketPerpsApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Polymarket Perps HTTP {}: {}",
            self.http_status, self.error
        )?;
        if let Some(r) = &self.reference {
            write!(f, " (ref {r})")?;
        }
        Ok(())
    }
}

impl std::error::Error for PolymarketPerpsApiError {}

#[derive(Deserialize)]
struct ErrorEnvelope {
    status: String,
    error: String,
    #[serde(rename = "ref")]
    reference: Option<String>,
}

impl PolymarketPerpsApiError {
    pub(crate) fn from_body(http_status: u16, body: &str) -> Self {
        let envelope = serde_json::from_str::<ErrorEnvelope>(body)
            .ok()
            .or_else(|| {
                serde_json::from_str::<Vec<ErrorEnvelope>>(body)
                    .ok()
                    .and_then(|mut v| (v.len() == 1).then(|| v.remove(0)))
            })
            .filter(|e| e.status == "err");
        match envelope {
            Some(e) => PolymarketPerpsApiError {
                http_status,
                error: e.error,
                reference: e.reference,
            },
            None => PolymarketPerpsApiError {
                http_status,
                error: body.to_string(),
                reference: None,
            },
        }
    }
}

/// Builds `{base}{path}?k=v&...`, skipping unset parameters. Values are percent-encoded.
pub(crate) struct Query {
    url: String,
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
        for b in value.to_string().bytes() {
            if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
                self.url.push(b as char);
            } else {
                self.url.push_str(&format!("%{b:02X}"));
            }
        }
        self.has_params = true;
        self
    }

    pub(crate) fn opt(self, key: &str, value: Option<impl Display>) -> Self {
        match value {
            Some(v) => self.param(key, v),
            None => self,
        }
    }

    pub(crate) fn url(self) -> String {
        self.url
    }
}

/// Reads a reply: non-2xx becomes `PolymarketPerpsApiError`, 2xx is parsed strictly as `T`.
pub(crate) async fn read_json<T: DeserializeOwned>(
    response: reqwest::Response,
    what: &str,
) -> ApiResult<T> {
    let status = response.status();
    let text = response.text().await?;
    if !status.is_success() {
        return Err(Box::new(PolymarketPerpsApiError::from_body(
            status.as_u16(),
            &text,
        )));
    }
    serde_json::from_str(&text)
        .map_err(|e| format!("Failed to parse Polymarket Perps {what}: {e}: {text}").into())
}

#[cfg(test)]
mod tests {
    use super::PolymarketPerpsApiError;

    #[test]
    fn error_envelopes() {
        let e = PolymarketPerpsApiError::from_body(
            400,
            include_str!("../../tests/fixtures/error_400.json"),
        );
        assert_eq!(e.error, "depth must be one of 10, 100, 500, 1000");
        assert_eq!(e.reference.as_deref(), Some("g-50c89589c4469"));

        let e = PolymarketPerpsApiError::from_body(
            401,
            include_str!("../../tests/fixtures/error_401.json"),
        );
        assert_eq!(
            (e.http_status, e.error.as_str(), e.reference),
            (401, "unauthorized", None)
        );

        // Batch routes wrap the request-level rejection in a one-element array.
        let e = PolymarketPerpsApiError::from_body(
            400,
            include_str!("../../tests/fixtures/error_batch_400.json"),
        );
        assert_eq!(e.error, "account not found for proxy");

        let e = PolymarketPerpsApiError::from_body(502, "<html>bad gateway</html>");
        assert_eq!(e.error, "<html>bad gateway</html>");
    }

    #[test]
    fn query_encodes_values() {
        let url = super::Query::new("https://x", "/v1/account/fills")
            .param("cursor", "eyJ0cyI6MX0=")
            .opt("sort", None::<&str>)
            .param("instrument_id", 6)
            .url();
        assert_eq!(
            url,
            "https://x/v1/account/fills?cursor=eyJ0cyI6MX0%3D&instrument_id=6"
        );
    }
}
