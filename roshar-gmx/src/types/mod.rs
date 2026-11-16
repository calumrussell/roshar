//! Wire types: GMX API and Oracle API reads, the order transaction workflow, the stream, and
//! the API error envelope.

pub mod market;
pub mod orders;
pub mod ws;

pub use market::*;
pub use orders::*;
pub use ws::*;

use serde::Deserialize;
use serde_json::Value;
use std::fmt;

/// Non-2xx response of the GMX API or the Oracle API.
///
/// GMX API bodies are `{"message": string | {field: {message, value?}}, "code"?, "details"?}`
/// (`code` e.g. `INSUFFICIENT_LIQUIDITY`, `MARKET_NOT_FOUND`); Oracle API bodies are
/// `{"error": string}` or `{"errors": [string]}`. `message` holds the text, or the field map as
/// JSON. A body that matches none of these is kept verbatim in `message`.
#[derive(Debug, Clone)]
pub struct GmxApiError {
    pub status: u16,
    pub code: Option<String>,
    pub message: String,
    pub details: Option<Value>,
}

#[derive(Deserialize)]
struct Envelope {
    #[serde(default)]
    message: Option<Value>,
    #[serde(default)]
    code: Option<String>,
    #[serde(default)]
    details: Option<Value>,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    errors: Option<Vec<String>>,
}

impl GmxApiError {
    pub fn from_body(status: u16, body: &str) -> Self {
        let parsed = serde_json::from_str::<Envelope>(body).ok();
        let Some(env) = parsed else {
            return Self {
                status,
                code: None,
                message: body.to_string(),
                details: None,
            };
        };
        let message = match (env.message, env.error, env.errors) {
            (Some(Value::String(s)), _, _) => s,
            (Some(other), _, _) => other.to_string(),
            (None, Some(e), _) => e,
            (None, None, Some(es)) => es.join("; "),
            (None, None, None) => body.to_string(),
        };
        Self {
            status,
            code: env.code,
            message,
            details: env.details,
        }
    }
}

impl fmt::Display for GmxApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.code {
            Some(code) => write!(f, "GMX API error {} {code}: {}", self.status, self.message),
            None => write!(f, "GMX API error {}: {}", self.status, self.message),
        }
    }
}

impl std::error::Error for GmxApiError {}
