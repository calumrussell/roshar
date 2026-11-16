pub mod account;
pub mod market;
pub mod trading;

pub use account::AccountApi;
pub use market::{MarketApi, OrderlyMarketDetailQuery};
pub use trading::TradingApi;

use serde::de::DeserializeOwned;
use serde_json::Value;

pub const ORDERLY_API_URL: &str = "https://api.orderly.org";
pub const ORDERLY_TESTNET_API_URL: &str = "https://testnet-api.orderly.org";

pub type ApiResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// A failed Orderly call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OrderlyError {
    /// `{"success": false, "code": .., "message": ..}`. Orderly sends this with HTTP 200 as well
    /// as 4xx.
    Api {
        http_status: u16,
        code: i64,
        message: String,
    },
    /// Public Info API (`POST /v1/public/query`) error: `{"success": false, "code":
    /// "INVALID_PARAM", "message", "type", "ts"}`; `retry_after` (ms epoch) comes with
    /// `RATE_LIMIT_EXCEEDED`.
    Query {
        http_status: u16,
        code: String,
        message: String,
        retry_after: Option<u64>,
    },
    /// Non-2xx without the error envelope.
    Http { http_status: u16, body: String },
    /// A 2xx body that does not match the documented shape.
    Parse { what: String, error: String },
}

impl std::fmt::Display for OrderlyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Api {
                http_status,
                code,
                message,
            } => write!(f, "Orderly error {code} (HTTP {http_status}): {message}"),
            Self::Query {
                http_status,
                code,
                message,
                ..
            } => write!(
                f,
                "Orderly query error {code} (HTTP {http_status}): {message}"
            ),
            Self::Http { http_status, body } => write!(f, "Orderly HTTP {http_status}: {body}"),
            Self::Parse { what, error } => write!(f, "Failed to parse Orderly {what}: {error}"),
        }
    }
}

impl std::error::Error for OrderlyError {}

fn parse_err(what: &str, e: impl std::fmt::Display) -> OrderlyError {
    OrderlyError::Parse {
        what: what.to_string(),
        error: e.to_string(),
    }
}

/// Returns the error envelope's code/message if `v` is `{"success": false, ...}`.
fn envelope_error(http_status: u16, v: &Value, what: &str) -> Option<OrderlyError> {
    if v.get("success") != Some(&Value::Bool(false)) {
        return None;
    }
    let code = v.get("code").and_then(Value::as_i64);
    let message = v.get("message").and_then(Value::as_str);
    Some(match (code, message) {
        (Some(code), Some(message)) => OrderlyError::Api {
            http_status,
            code,
            message: message.to_string(),
        },
        _ => parse_err(what, format!("error envelope without code/message: {v}")),
    })
}

/// Parses `{"success": true, "data": T, "timestamp": ..}`; surfaces the error envelope.
pub(crate) fn parse_data<T: DeserializeOwned>(
    http_status: u16,
    body: &str,
    what: &str,
) -> Result<T, OrderlyError> {
    let v: Value = match serde_json::from_str(body) {
        Ok(v) => v,
        Err(_) if !(200..300).contains(&http_status) => {
            return Err(OrderlyError::Http {
                http_status,
                body: body.to_string(),
            })
        }
        Err(e) => return Err(parse_err(what, e)),
    };
    if let Some(e) = envelope_error(http_status, &v, what) {
        return Err(e);
    }
    if !(200..300).contains(&http_status) {
        return Err(OrderlyError::Http {
            http_status,
            body: body.to_string(),
        });
    }
    let Value::Object(mut map) = v else {
        return Err(parse_err(what, "response is not a JSON object"));
    };
    if map.get("success") != Some(&Value::Bool(true)) {
        return Err(parse_err(what, "missing \"success\": true"));
    }
    let data = map
        .remove("data")
        .ok_or_else(|| parse_err(what, "missing \"data\""))?;
    serde_json::from_value(data).map_err(|e| parse_err(what, e))
}

/// Parses an un-enveloped body (the TradingView endpoints); still surfaces the error envelope.
pub(crate) fn parse_bare<T: DeserializeOwned>(
    http_status: u16,
    body: &str,
    what: &str,
) -> Result<T, OrderlyError> {
    let v: Value = match serde_json::from_str(body) {
        Ok(v) => v,
        Err(_) if !(200..300).contains(&http_status) => {
            return Err(OrderlyError::Http {
                http_status,
                body: body.to_string(),
            })
        }
        Err(e) => return Err(parse_err(what, e)),
    };
    if let Some(e) = envelope_error(http_status, &v, what) {
        return Err(e);
    }
    if !(200..300).contains(&http_status) {
        return Err(OrderlyError::Http {
            http_status,
            body: body.to_string(),
        });
    }
    serde_json::from_value(v).map_err(|e| parse_err(what, e))
}

/// Parses a Public Info API reply `{"success": true, "data": T, "ts": ..}`; its errors carry
/// string codes and become `OrderlyError::Query`.
pub(crate) fn parse_query<T: DeserializeOwned>(
    http_status: u16,
    body: &str,
    what: &str,
) -> Result<T, OrderlyError> {
    let v: Value = match serde_json::from_str(body) {
        Ok(v) => v,
        Err(_) if !(200..300).contains(&http_status) => {
            return Err(OrderlyError::Http {
                http_status,
                body: body.to_string(),
            })
        }
        Err(e) => return Err(parse_err(what, e)),
    };
    if v.get("success") == Some(&Value::Bool(false)) {
        let code = v.get("code").and_then(Value::as_str);
        let message = v.get("message").and_then(Value::as_str);
        return Err(match (code, message) {
            (Some(code), Some(message)) => OrderlyError::Query {
                http_status,
                code: code.to_string(),
                message: message.to_string(),
                retry_after: v.get("retry_after").and_then(Value::as_u64),
            },
            _ => parse_err(what, format!("error envelope without code/message: {v}")),
        });
    }
    if !(200..300).contains(&http_status) {
        return Err(OrderlyError::Http {
            http_status,
            body: body.to_string(),
        });
    }
    let Value::Object(mut map) = v else {
        return Err(parse_err(what, "response is not a JSON object"));
    };
    if map.get("success") != Some(&Value::Bool(true)) {
        return Err(parse_err(what, "missing \"success\": true"));
    }
    let data = map
        .remove("data")
        .ok_or_else(|| parse_err(what, "missing \"data\""))?;
    serde_json::from_value(data).map_err(|e| parse_err(what, e))
}

/// `data: {"rows": [...]}` wrapper of the list endpoints.
#[derive(serde::Deserialize)]
pub(crate) struct Rows<T> {
    pub(crate) rows: Vec<T>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{OrderlyFundingRate, OrderlySystemInfo, OrderlyTvBars};

    #[test]
    fn error_envelope_with_http_401() {
        let body = include_str!("../../tests/fixtures/rest_error_no_account_header.json");
        let err = parse_data::<Value>(401, body, "orderbook").unwrap_err();
        assert_eq!(
            err,
            OrderlyError::Api {
                http_status: 401,
                code: -1002,
                message: "orderly-account-id header is empty".to_string(),
            }
        );
    }

    #[test]
    fn error_envelope_with_http_200() {
        let body = include_str!("../../tests/fixtures/rest_error_account_not_found.json");
        let err = parse_data::<Value>(200, body, "account").unwrap_err();
        assert_eq!(
            err,
            OrderlyError::Api {
                http_status: 200,
                code: -1607,
                message: "Account not found.".to_string(),
            }
        );
    }

    #[test]
    fn data_envelope() {
        let body = include_str!("../../tests/fixtures/rest_funding_rate_symbol.json");
        let rate: OrderlyFundingRate = parse_data(200, body, "funding rate").unwrap();
        assert_eq!(rate.symbol, "PERP_BTC_USDC");
        let sys: OrderlySystemInfo = parse_data(
            200,
            include_str!("../../tests/fixtures/rest_system_info.json"),
            "system info",
        )
        .unwrap();
        assert_eq!(sys.status, 0);
        assert!(sys.scheduled_maintenance.is_none());
    }

    #[test]
    fn bare_and_errors() {
        let bars: OrderlyTvBars = parse_bare(
            200,
            include_str!("../../tests/fixtures/rest_tv_kline_history.json"),
            "kline history",
        )
        .unwrap();
        assert_eq!(bars.s, "ok");
        let err = parse_bare::<OrderlyTvBars>(
            400,
            r#"{"success":false,"code":-1005,"message":"Required request parameter 'to' for method parameter type long is not present"}"#,
            "kline history",
        )
        .unwrap_err();
        assert!(matches!(err, OrderlyError::Api { code: -1005, .. }));
        assert!(matches!(
            parse_data::<Value>(502, "<html>bad gateway</html>", "x").unwrap_err(),
            OrderlyError::Http {
                http_status: 502,
                ..
            }
        ));
        assert!(matches!(
            parse_data::<Value>(200, r#"{"success":true}"#, "x").unwrap_err(),
            OrderlyError::Parse { .. }
        ));
    }

    #[test]
    fn query_envelope_and_errors() {
        use crate::types::OrderlyQueryOrderbook;
        let book: OrderlyQueryOrderbook = parse_query(
            200,
            include_str!("../../tests/fixtures/query_orderbook.json"),
            "orderbook",
        )
        .unwrap();
        assert_eq!(book.symbol, "PERP_BTC_USDC");
        let err = parse_query::<Value>(
            400,
            include_str!("../../tests/fixtures/query_error_invalid_param.json"),
            "orderbook",
        )
        .unwrap_err();
        assert_eq!(
            err,
            OrderlyError::Query {
                http_status: 400,
                code: "INVALID_PARAM".to_string(),
                message: "symbol PERP_NOPE_USDC not found".to_string(),
                retry_after: None,
            }
        );
        let limited = parse_query::<Value>(
            429,
            r#"{"success":false,"code":"RATE_LIMIT_EXCEEDED","message":"Rate limit exceeded. Retry after 1779269200000","retry_after":1779269200000,"ts":1779269143700}"#,
            "candles",
        )
        .unwrap_err();
        assert!(matches!(
            limited,
            OrderlyError::Query {
                retry_after: Some(1779269200000),
                ..
            }
        ));
    }
}
