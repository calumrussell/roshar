//! Public market history (`https://futures.kraken.com/api/history/v3/market/{tradeable}/...`).

use roshar_http::RateLimitedClient;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

const HISTORY_URL: &str = "https://futures.kraken.com/api/history/v3";

/// For fields that are always sent but may be `null`: a missing key is an error.
fn nullable<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}

/// `sort` of the market history endpoints. Kraken defaults to `desc`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KrakenSortOrder {
    Asc,
    Desc,
}

impl KrakenSortOrder {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Asc => "asc",
            Self::Desc => "desc",
        }
    }
}

/// Query of the market history endpoints. Every field is optional and omitted when `None`.
#[derive(Debug, Clone, Default)]
pub struct KrakenHistoryQuery<'a> {
    /// Milliseconds.
    pub since: Option<i64>,
    /// Milliseconds.
    pub before: Option<i64>,
    pub sort: Option<KrakenSortOrder>,
    /// `continuationToken` of the previous page; `sort` must be unchanged.
    pub continuation_token: Option<&'a str>,
    /// Kraken caps this at 1000.
    pub count: Option<u64>,
}

/// One page of a market history listing.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct KrakenHistoryPage<T> {
    pub len: u64,
    pub elements: Vec<T>,
    /// Absent on the last page.
    #[serde(rename = "continuationToken")]
    pub continuation_token: Option<String>,
}

/// An order as shown in public history (no account data). Decimals are strings as sent.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct KrakenHistoryOrder {
    pub uid: String,
    pub tradeable: String,
    /// `Buy`, `Sell` or `Unknown`.
    pub direction: String,
    pub quantity: String,
    /// Milliseconds.
    pub timestamp: i64,
    #[serde(rename = "limitPrice")]
    pub limit_price: String,
    /// E.g. `Limit`, `Post`, `PartialLiquidation`.
    #[serde(rename = "orderType")]
    pub order_type: String,
    #[serde(rename = "reduceOnly")]
    pub reduce_only: bool,
    #[serde(rename = "lastUpdateTimestamp")]
    pub last_update_timestamp: i64,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct KrakenPublicExecution {
    pub uid: String,
    #[serde(rename = "makerOrder")]
    pub maker_order: KrakenHistoryOrder,
    #[serde(rename = "takerOrder")]
    pub taker_order: KrakenHistoryOrder,
    /// Taker order before this execution; sent for some executions.
    #[serde(rename = "oldTakerOrder")]
    pub old_taker_order: Option<KrakenHistoryOrder>,
    /// Milliseconds.
    pub timestamp: i64,
    pub quantity: String,
    pub price: String,
    #[serde(rename = "markPrice")]
    pub mark_price: String,
    #[serde(rename = "limitFilled")]
    pub limit_filled: bool,
    #[serde(rename = "usdValue")]
    pub usd_value: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct KrakenPublicExecutionDetails {
    pub execution: KrakenPublicExecution,
    /// Often an empty string.
    #[serde(rename = "takerReducedQuantity")]
    pub taker_reduced_quantity: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub enum KrakenPublicExecutionEvent {
    Execution(KrakenPublicExecutionDetails),
}

/// Element of `GET /market/{tradeable}/executions`.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct KrakenPublicExecutionElement {
    pub uid: String,
    /// Milliseconds.
    pub timestamp: i64,
    pub event: KrakenPublicExecutionEvent,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct KrakenOrderPlaced {
    pub order: KrakenHistoryOrder,
    pub reason: String,
    /// Always an empty string per the spec.
    #[serde(rename = "reducedQuantity")]
    pub reduced_quantity: String,
    /// Not in the spec; sent as `null` on every live event (2026-11-16), so its type is unknown.
    #[serde(deserialize_with = "nullable")]
    pub origin: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct KrakenOrderUpdated {
    #[serde(rename = "oldOrder")]
    pub old_order: KrakenHistoryOrder,
    #[serde(rename = "newOrder")]
    pub new_order: KrakenHistoryOrder,
    pub reason: String,
    #[serde(rename = "reducedQuantity")]
    pub reduced_quantity: String,
    /// Not in the spec; sent as `null` on every live event (2026-11-16), so its type is unknown.
    #[serde(deserialize_with = "nullable")]
    pub origin: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct KrakenOrderCancelled {
    pub order: KrakenHistoryOrder,
    pub reason: String,
    /// Not in the spec; sent as `null` on every live event (2026-11-16), so its type is unknown.
    #[serde(deserialize_with = "nullable")]
    pub origin: Option<serde_json::Value>,
}

/// Event of `GET /market/{tradeable}/orders`, externally tagged as sent.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub enum KrakenPublicOrderEvent {
    OrderPlaced(KrakenOrderPlaced),
    OrderUpdated(KrakenOrderUpdated),
    OrderCancelled(KrakenOrderCancelled),
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct KrakenPublicOrderElement {
    pub uid: String,
    /// Milliseconds.
    pub timestamp: i64,
    pub event: KrakenPublicOrderEvent,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct KrakenMarkPriceChanged {
    pub price: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub enum KrakenMarkPriceEvent {
    MarkPriceChanged(KrakenMarkPriceChanged),
}

/// Element of `GET /market/{tradeable}/price`. `uid` is sent empty.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct KrakenMarkPriceElement {
    pub uid: String,
    /// Milliseconds.
    pub timestamp: i64,
    pub event: KrakenMarkPriceEvent,
}

fn parse_page<T: serde::de::DeserializeOwned>(
    status: reqwest::StatusCode,
    text: &str,
) -> Result<KrakenHistoryPage<T>, Box<dyn std::error::Error>> {
    // Unknown markets get HTTP 404 `{"status":"not_found","reason":...}`.
    if !status.is_success() {
        return Err(format!("Kraken history request failed ({status}): {text}").into());
    }
    serde_json::from_str(text)
        .map_err(|e| format!("Failed to parse Kraken history response ({e}): {text}").into())
}

pub struct MarketHistoryApi {
    client: Arc<RateLimitedClient>,
}

impl MarketHistoryApi {
    pub fn new_with_client(client: Arc<RateLimitedClient>) -> Self {
        Self { client }
    }

    async fn list<T: serde::de::DeserializeOwned>(
        &self,
        tradeable: &str,
        kind: &str,
        query: &KrakenHistoryQuery<'_>,
    ) -> Result<KrakenHistoryPage<T>, Box<dyn std::error::Error>> {
        let mut params: Vec<(&str, String)> = Vec::new();
        if let Some(v) = query.since {
            params.push(("since", v.to_string()));
        }
        if let Some(v) = query.before {
            params.push(("before", v.to_string()));
        }
        if let Some(v) = query.sort {
            params.push(("sort", v.as_str().to_string()));
        }
        if let Some(v) = query.continuation_token {
            params.push(("continuation_token", v.to_string()));
        }
        if let Some(v) = query.count {
            params.push(("count", v.to_string()));
        }
        let mut url = format!("{HISTORY_URL}/market/{tradeable}/{kind}");
        if !params.is_empty() {
            url.push('?');
            url.push_str(&serde_urlencoded::to_string(&params)?);
        }
        let response = self.client.get(&url).await?;
        let status = response.status();
        let text = response.text().await?;
        parse_page(status, &text)
    }

    /// `GET /market/{tradeable}/executions`: public trades with both orders.
    pub async fn get_executions(
        &self,
        tradeable: &str,
        query: &KrakenHistoryQuery<'_>,
    ) -> Result<KrakenHistoryPage<KrakenPublicExecutionElement>, Box<dyn std::error::Error>> {
        self.list(tradeable, "executions", query).await
    }

    /// `GET /market/{tradeable}/orders`: order placements, updates and cancellations.
    pub async fn get_order_events(
        &self,
        tradeable: &str,
        query: &KrakenHistoryQuery<'_>,
    ) -> Result<KrakenHistoryPage<KrakenPublicOrderElement>, Box<dyn std::error::Error>> {
        self.list(tradeable, "orders", query).await
    }

    /// `GET /market/{tradeable}/price`: mark price changes.
    pub async fn get_mark_price_events(
        &self,
        tradeable: &str,
        query: &KrakenHistoryQuery<'_>,
    ) -> Result<KrakenHistoryPage<KrakenMarkPriceElement>, Box<dyn std::error::Error>> {
        self.list(tradeable, "price", query).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> String {
        std::fs::read_to_string(format!(
            "{}/tests/fixtures/{name}",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap()
    }

    #[test]
    fn parses_executions() {
        let p: KrakenHistoryPage<KrakenPublicExecutionElement> =
            parse_page(reqwest::StatusCode::OK, &fixture("market_executions.json")).unwrap();
        assert_eq!(p.elements.len(), 3);
        assert!(p.continuation_token.is_some());
        let KrakenPublicExecutionEvent::Execution(d) = &p.elements[2].event;
        assert!(d.execution.old_taker_order.is_some());

        let text =
            fixture("market_executions.json").replacen("\"markPrice\":", "\"markPriceX\":", 1);
        assert!(
            parse_page::<KrakenPublicExecutionElement>(reqwest::StatusCode::OK, &text).is_err()
        );
    }

    #[test]
    fn parses_order_events() {
        let p: KrakenHistoryPage<KrakenPublicOrderElement> =
            parse_page(reqwest::StatusCode::OK, &fixture("market_orders.json")).unwrap();
        assert!(matches!(
            p.elements[0].event,
            KrakenPublicOrderEvent::OrderPlaced(_)
        ));
        assert!(matches!(
            p.elements[1].event,
            KrakenPublicOrderEvent::OrderUpdated(_)
        ));
        assert!(matches!(
            p.elements[2].event,
            KrakenPublicOrderEvent::OrderCancelled(_)
        ));

        let text = fixture("market_orders.json").replacen("\"reduceOnly\":", "\"reduceOnlyX\":", 1);
        assert!(parse_page::<KrakenPublicOrderElement>(reqwest::StatusCode::OK, &text).is_err());
    }

    #[test]
    fn parses_mark_price_events() {
        let p: KrakenHistoryPage<KrakenMarkPriceElement> =
            parse_page(reqwest::StatusCode::OK, &fixture("market_price.json")).unwrap();
        let KrakenMarkPriceEvent::MarkPriceChanged(c) = &p.elements[0].event;
        assert!(!c.price.is_empty());

        let text = fixture("market_price.json").replacen("\"price\":", "\"priceX\":", 1);
        assert!(parse_page::<KrakenMarkPriceElement>(reqwest::StatusCode::OK, &text).is_err());
    }

    #[test]
    fn not_found_is_error() {
        // GET /market/NOPE/executions, 2026-11-16.
        let text = r#"{"status":"not_found","reason":"Resource was not found."}"#;
        let err =
            parse_page::<KrakenMarkPriceElement>(reqwest::StatusCode::NOT_FOUND, text).unwrap_err();
        assert!(err.to_string().contains("not_found"));
    }

    #[tokio::test]
    async fn test_get_market_history() {
        let api = MarketHistoryApi::new_with_client(Arc::new(RateLimitedClient::new(10, 1)));
        let q = KrakenHistoryQuery {
            count: Some(2),
            ..Default::default()
        };
        let p = api
            .get_executions("PF_XBTUSD", &q)
            .await
            .expect("executions");
        assert_eq!(p.elements.len(), 2);
        api.get_order_events("PF_XBTUSD", &q).await.expect("orders");
        let first = api
            .get_mark_price_events(
                "PF_XBTUSD",
                &KrakenHistoryQuery {
                    sort: Some(KrakenSortOrder::Asc),
                    count: Some(1),
                    ..Default::default()
                },
            )
            .await
            .expect("price");
        let next = api
            .get_mark_price_events(
                "PF_XBTUSD",
                &KrakenHistoryQuery {
                    sort: Some(KrakenSortOrder::Asc),
                    count: Some(1),
                    continuation_token: first.continuation_token.as_deref(),
                    ..Default::default()
                },
            )
            .await
            .expect("price page 2");
        assert!(next.elements[0].timestamp >= first.elements[0].timestamp);
    }
}
