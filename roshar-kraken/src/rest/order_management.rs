#![allow(clippy::too_many_arguments)]

use roshar_http::RateLimitedClient;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use super::auth::{AuthApi, KrakenCredentials};
use super::parse_response;

const BASE_URL: &str = "https://futures.kraken.com";

/// Envelope of every `/derivatives/api/v3` success reply.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KrakenResponse<T> {
    pub result: String,
    #[serde(flatten)]
    pub data: T,
    #[serde(rename = "serverTime")]
    pub server_time: String,
}

/// Parameters of `POST /sendorder`. Numbers are decimal strings, sent as given.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KrakenOrderRequest {
    /// `lmt`, `post`, `ioc`, `mkt`, `stp`, `take_profit`, `trailing_stop` or `fok`.
    #[serde(rename = "orderType")]
    pub order_type: String,
    pub symbol: String,
    /// `buy` or `sell`.
    pub side: String,
    pub size: String,
    #[serde(rename = "limitPrice", skip_serializing_if = "Option::is_none")]
    pub limit_price: Option<String>,
    #[serde(rename = "stopPrice", skip_serializing_if = "Option::is_none")]
    pub stop_price: Option<String>,
    #[serde(rename = "cliOrdId", skip_serializing_if = "Option::is_none")]
    pub cli_ord_id: Option<String>,
    /// `mark`, `index` or `last`, for trigger orders.
    #[serde(rename = "triggerSignal", skip_serializing_if = "Option::is_none")]
    pub trigger_signal: Option<String>,
    #[serde(rename = "reduceOnly", skip_serializing_if = "Option::is_none")]
    pub reduce_only: Option<bool>,
}

/// Parameters of `POST /editorder`. One of `orderId` / `cliOrdId` is required.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KrakenModifyOrderRequest {
    #[serde(rename = "orderId", skip_serializing_if = "Option::is_none")]
    pub order_id: Option<String>,
    #[serde(rename = "cliOrdId", skip_serializing_if = "Option::is_none")]
    pub cli_ord_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<String>,
    #[serde(rename = "limitPrice", skip_serializing_if = "Option::is_none")]
    pub limit_price: Option<String>,
    #[serde(rename = "stopPrice", skip_serializing_if = "Option::is_none")]
    pub stop_price: Option<String>,
}

/// Parameters of `POST /cancelorder`. One of `order_id` / `cliOrdId` is required.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KrakenCancelOrderRequest {
    #[serde(rename = "order_id", skip_serializing_if = "Option::is_none")]
    pub order_id: Option<String>,
    #[serde(rename = "cliOrdId", skip_serializing_if = "Option::is_none")]
    pub cli_ord_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KrakenDeadMansSwitchRequest {
    /// Seconds; 0 disables the switch.
    pub timeout: u32,
}

/// Order as it appears in order events (`order`, `old`, `new`, `orderPriorExecution`, ...).
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KrakenOrder {
    #[serde(rename = "orderId")]
    pub order_id: String,
    #[serde(rename = "cliOrdId")]
    pub cli_ord_id: Option<String>,
    #[serde(rename = "type")]
    pub order_type: String,
    pub symbol: String,
    pub side: String,
    pub quantity: f64,
    pub filled: f64,
    #[serde(rename = "limitPrice")]
    pub limit_price: f64,
    #[serde(rename = "reduceOnly")]
    pub reduce_only: bool,
    pub timestamp: String,
    #[serde(rename = "lastUpdateTimestamp")]
    pub last_update_timestamp: String,
    #[serde(rename = "algoId")]
    pub algo_id: Option<String>,
}

/// Trigger order as it appears in order events (`orderTrigger`).
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KrakenOrderTrigger {
    pub uid: String,
    #[serde(rename = "clientId")]
    pub client_id: Option<String>,
    #[serde(rename = "type")]
    pub order_type: String,
    pub symbol: String,
    pub side: String,
    pub quantity: Option<f64>,
    #[serde(rename = "limitPrice")]
    pub limit_price: Option<f64>,
    #[serde(rename = "triggerPrice")]
    pub trigger_price: Option<f64>,
    /// `trigger_above` or `trigger_below`.
    #[serde(rename = "triggerSide")]
    pub trigger_side: Option<String>,
    /// `mark_price`, `last_price` or `spot_price`.
    #[serde(rename = "triggerSignal")]
    pub trigger_signal: Option<String>,
    #[serde(rename = "reduceOnly")]
    pub reduce_only: bool,
    pub timestamp: String,
    #[serde(rename = "lastUpdateTimestamp")]
    pub last_update_timestamp: String,
    #[serde(rename = "startTime")]
    pub start_time: Option<String>,
}

/// One entry of `orderEvents` in send/edit/cancel replies, keyed by `type`. Events about a
/// trigger order carry `order_trigger` instead of `order`.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type")]
pub enum KrakenOrderEvent {
    #[serde(rename = "PLACE")]
    Place {
        order: Option<KrakenOrder>,
        #[serde(rename = "orderTrigger")]
        order_trigger: Option<KrakenOrderTrigger>,
        #[serde(rename = "reducedQuantity")]
        reduced_quantity: Option<f64>,
    },
    #[serde(rename = "CANCEL")]
    Cancel {
        uid: String,
        order: Option<KrakenOrder>,
        #[serde(rename = "orderTrigger")]
        order_trigger: Option<KrakenOrderTrigger>,
    },
    #[serde(rename = "EDIT")]
    Edit {
        old: KrakenOrder,
        new: KrakenOrder,
        #[serde(rename = "reducedQuantity")]
        reduced_quantity: Option<f64>,
    },
    #[serde(rename = "REJECT")]
    Reject {
        uid: String,
        order: Option<KrakenOrder>,
        #[serde(rename = "orderTrigger")]
        order_trigger: Option<KrakenOrderTrigger>,
        reason: String,
    },
    #[serde(rename = "EXECUTION")]
    Execution {
        #[serde(rename = "executionId")]
        execution_id: String,
        price: f64,
        amount: f64,
        #[serde(rename = "orderPriorEdit")]
        order_prior_edit: Option<KrakenOrder>,
        #[serde(rename = "orderPriorExecution")]
        order_prior_execution: KrakenOrder,
        #[serde(rename = "takerReducedQuantity")]
        taker_reduced_quantity: Option<f64>,
    },
}

/// `sendStatus`. `status` other than `placed`/`partiallyFilled`/`filled` means the order was
/// not accepted; `order_id` and `orderEvents` may then be absent.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KrakenSendStatus {
    pub order_id: Option<String>,
    #[serde(rename = "cliOrdId")]
    pub cli_ord_id: Option<String>,
    pub status: String,
    #[serde(rename = "receivedTime")]
    pub received_time: Option<String>,
    #[serde(rename = "orderEvents")]
    pub order_events: Option<Vec<KrakenOrderEvent>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KrakenOrderData {
    #[serde(rename = "sendStatus")]
    pub send_status: KrakenSendStatus,
}

pub type KrakenOrderResponse = KrakenResponse<KrakenOrderData>;

/// `cancelStatus`. `status` is `cancelled`, `filled` or `notFound`.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KrakenCancelStatus {
    pub order_id: Option<String>,
    #[serde(rename = "cliOrdId")]
    pub cli_ord_id: Option<String>,
    pub status: String,
    #[serde(rename = "receivedTime")]
    pub received_time: Option<String>,
    #[serde(rename = "orderEvents")]
    pub order_events: Option<Vec<KrakenOrderEvent>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KrakenCancelData {
    #[serde(rename = "cancelStatus")]
    pub cancel_status: KrakenCancelStatus,
}

pub type KrakenCancelResponse = KrakenResponse<KrakenCancelData>;

/// `editStatus`.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KrakenEditStatus {
    #[serde(rename = "orderId")]
    pub order_id: Option<String>,
    #[serde(rename = "cliOrdId")]
    pub cli_ord_id: Option<String>,
    pub status: String,
    #[serde(rename = "receivedTime")]
    pub received_time: Option<String>,
    #[serde(rename = "orderEvents")]
    pub order_events: Vec<KrakenOrderEvent>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KrakenModifyData {
    #[serde(rename = "editStatus")]
    pub edit_status: KrakenEditStatus,
}

pub type KrakenModifyResponse = KrakenResponse<KrakenModifyData>;

/// One entry of `GET /openorders`.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KrakenOpenOrder {
    pub order_id: String,
    #[serde(rename = "cliOrdId")]
    pub cli_ord_id: Option<String>,
    /// `untouched` or `partiallyFilled`.
    pub status: String,
    pub side: String,
    /// `lmt`, `stop` or `take_profit`.
    #[serde(rename = "orderType")]
    pub order_type: String,
    pub symbol: String,
    #[serde(rename = "limitPrice")]
    pub limit_price: Option<f64>,
    #[serde(rename = "stopPrice")]
    pub stop_price: Option<f64>,
    #[serde(rename = "filledSize")]
    pub filled_size: f64,
    #[serde(rename = "unfilledSize")]
    pub unfilled_size: Option<f64>,
    #[serde(rename = "reduceOnly")]
    pub reduce_only: bool,
    /// `mark`, `last` or `spot`.
    #[serde(rename = "triggerSignal")]
    pub trigger_signal: Option<String>,
    #[serde(rename = "lastUpdateTime")]
    pub last_update_time: String,
    #[serde(rename = "receivedTime")]
    pub received_time: String,
    #[serde(rename = "algoId")]
    pub algo_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KrakenOpenOrdersData {
    #[serde(rename = "openOrders")]
    pub open_orders: Vec<KrakenOpenOrder>,
}

pub type KrakenOpenOrdersResponse = KrakenResponse<KrakenOpenOrdersData>;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KrakenTrailingStopOptions {
    #[serde(rename = "maxDeviation")]
    pub max_deviation: f64,
    /// `PERCENT` or `QUOTE_CURRENCY`.
    pub unit: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KrakenPriceTriggerOptions {
    #[serde(rename = "triggerPrice")]
    pub trigger_price: f64,
    /// `TRIGGER_ABOVE` or `TRIGGER_BELOW`.
    #[serde(rename = "triggerSide")]
    pub trigger_side: String,
    /// `MARK_PRICE`, `LAST_PRICE` or `SPOT_PRICE`.
    #[serde(rename = "triggerSignal")]
    pub trigger_signal: String,
    #[serde(rename = "trailingStopOptions")]
    pub trailing_stop_options: Option<KrakenTrailingStopOptions>,
}

/// `order` of a `POST /orders/status` entry.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KrakenOrderStatusOrder {
    /// `ORDER` or `TRIGGER_ORDER`.
    #[serde(rename = "type")]
    pub order_type: String,
    #[serde(rename = "orderId")]
    pub order_id: String,
    #[serde(rename = "cliOrdId")]
    pub cli_ord_id: Option<String>,
    pub symbol: String,
    pub side: String,
    pub quantity: Option<f64>,
    pub filled: Option<f64>,
    #[serde(rename = "limitPrice")]
    pub limit_price: Option<f64>,
    #[serde(rename = "reduceOnly")]
    pub reduce_only: bool,
    pub timestamp: String,
    #[serde(rename = "lastUpdateTimestamp")]
    pub last_update_timestamp: String,
    #[serde(rename = "algoId")]
    pub algo_id: Option<String>,
    #[serde(rename = "priceTriggerOptions")]
    pub price_trigger_options: Option<KrakenPriceTriggerOptions>,
    #[serde(rename = "triggerTime")]
    pub trigger_time: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KrakenOrderStatusItem {
    pub order: KrakenOrderStatusOrder,
    /// `ENTERED_BOOK`, `FULLY_EXECUTED`, `REJECTED`, `CANCELLED`, `TRIGGER_PLACED` or
    /// `TRIGGER_ACTIVATION_FAILURE`.
    pub status: String,
    #[serde(rename = "updateReason")]
    pub update_reason: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KrakenOrderStatusData {
    pub orders: Vec<KrakenOrderStatusItem>,
}

pub type KrakenOrderStatusResponse = KrakenResponse<KrakenOrderStatusData>;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KrakenDeadMansSwitchData {
    pub status: KrakenDeadMansSwitchStatus,
}

pub type KrakenDeadMansSwitchResponse = KrakenResponse<KrakenDeadMansSwitchData>;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KrakenDeadMansSwitchStatus {
    #[serde(rename = "currentTime")]
    pub current_time: String,
    #[serde(rename = "triggerTime")]
    pub trigger_time: String,
}

/// Kraken Futures order management (`/derivatives/api/v3`).
pub struct OrderManagementApi {
    client: Arc<RateLimitedClient>,
    credentials: KrakenCredentials,
}

impl OrderManagementApi {
    pub fn new_with_client(client: Arc<RateLimitedClient>, credentials: KrakenCredentials) -> Self {
        Self {
            client,
            credentials,
        }
    }

    /// Signed POST with the parameters form-encoded in the body.
    async fn make_request<T, R>(
        &self,
        endpoint: &str,
        request_data: &T,
    ) -> Result<R, Box<dyn std::error::Error>>
    where
        T: Serialize + ?Sized,
        R: for<'de> Deserialize<'de>,
    {
        let post_data = serde_urlencoded::to_string(request_data)?;
        let headers = AuthApi::create_headers(&self.credentials, endpoint, &post_data)?;

        let mut request = self.client.post(&format!("{BASE_URL}{endpoint}")).await;

        for (key, value) in headers {
            request = request.header(key, value);
        }

        request = request
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(post_data);

        let text = request.send().await?.text().await?;
        parse_response(&text)
    }

    async fn make_get_request<R>(&self, endpoint: &str) -> Result<R, Box<dyn std::error::Error>>
    where
        R: for<'de> Deserialize<'de>,
    {
        let headers = AuthApi::create_headers(&self.credentials, endpoint, "")?;

        let mut request = self
            .client
            .request(reqwest::Method::GET, &format!("{BASE_URL}{endpoint}"))
            .await;

        for (key, value) in headers {
            request = request.header(key, value);
        }

        let text = request.send().await?.text().await?;
        parse_response(&text)
    }

    /// `POST /sendorder`. Post-only is `order_type = "post"`; IOC and FOK are `ioc` / `fok`.
    pub async fn create_order(
        &self,
        symbol: &str,
        side: &str,
        order_type: &str,
        size: &str,
        limit_price: Option<&str>,
        stop_price: Option<&str>,
        cli_ord_id: Option<&str>,
        trigger_signal: Option<&str>,
        reduce_only: Option<bool>,
    ) -> Result<KrakenOrderResponse, Box<dyn std::error::Error>> {
        let order_request = KrakenOrderRequest {
            order_type: order_type.to_string(),
            symbol: symbol.to_string(),
            side: side.to_string(),
            size: size.to_string(),
            limit_price: limit_price.map(str::to_string),
            stop_price: stop_price.map(str::to_string),
            cli_ord_id: cli_ord_id.map(str::to_string),
            trigger_signal: trigger_signal.map(str::to_string),
            reduce_only,
        };

        self.make_request("/derivatives/api/v3/sendorder", &order_request)
            .await
    }

    /// `POST /cancelorder`, by exchange order id or client order id.
    pub async fn cancel_order(
        &self,
        order_id: Option<&str>,
        cli_ord_id: Option<&str>,
    ) -> Result<KrakenCancelResponse, Box<dyn std::error::Error>> {
        if order_id.is_none() && cli_ord_id.is_none() {
            return Err("Either order_id or cli_ord_id must be provided".into());
        }

        let cancel_request = KrakenCancelOrderRequest {
            order_id: order_id.map(str::to_string),
            cli_ord_id: cli_ord_id.map(str::to_string),
        };

        self.make_request("/derivatives/api/v3/cancelorder", &cancel_request)
            .await
    }

    /// `POST /editorder`, by exchange order id or client order id. Numbers are decimal strings.
    pub async fn modify_order(
        &self,
        order_id: Option<&str>,
        cli_ord_id: Option<&str>,
        size: Option<&str>,
        limit_price: Option<&str>,
        stop_price: Option<&str>,
    ) -> Result<KrakenModifyResponse, Box<dyn std::error::Error>> {
        if order_id.is_none() && cli_ord_id.is_none() {
            return Err("Either order_id or cli_ord_id must be provided".into());
        }

        let modify_request = KrakenModifyOrderRequest {
            order_id: order_id.map(str::to_string),
            cli_ord_id: cli_ord_id.map(str::to_string),
            size: size.map(str::to_string),
            limit_price: limit_price.map(str::to_string),
            stop_price: stop_price.map(str::to_string),
        };

        self.make_request("/derivatives/api/v3/editorder", &modify_request)
            .await
    }

    pub async fn get_open_orders(
        &self,
    ) -> Result<KrakenOpenOrdersResponse, Box<dyn std::error::Error>> {
        self.make_get_request("/derivatives/api/v3/openorders")
            .await
    }

    /// `POST /orders/status`: orders open now or closed in the last 5 seconds.
    pub async fn get_order_status(
        &self,
        order_ids: Option<Vec<String>>,
        cli_ord_ids: Option<Vec<String>>,
    ) -> Result<KrakenOrderStatusResponse, Box<dyn std::error::Error>> {
        if order_ids.is_none() && cli_ord_ids.is_none() {
            return Err("Either order_ids or cli_ord_ids must be provided".into());
        }

        // Arrays are sent as repeated keys.
        let mut form_data: Vec<(&str, String)> = Vec::new();
        for id in order_ids.into_iter().flatten() {
            form_data.push(("orderIds", id));
        }
        for id in cli_ord_ids.into_iter().flatten() {
            form_data.push(("cliOrdIds", id));
        }

        self.make_request("/derivatives/api/v3/orders/status", &form_data)
            .await
    }

    /// `POST /cancelallordersafter`. `timeout` 0 disables the switch.
    pub async fn dead_mans_switch(
        &self,
        timeout: u32,
    ) -> Result<KrakenDeadMansSwitchResponse, Box<dyn std::error::Error>> {
        let dms_request = KrakenDeadMansSwitchRequest { timeout };

        self.make_request("/derivatives/api/v3/cancelallordersafter", &dms_request)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Response examples from the official OpenAPI spec,
    // https://docs.kraken.com/openapi/futures-rest.yaml (components/examples).

    #[test]
    fn parses_placed_order() {
        let text = r#"{"result":"success","sendStatus":{"order_id":"179f9af8-e45e-469d-b3e9-2fd4675cb7d0","status":"placed","receivedTime":"2019-09-05T16:33:50.734Z","orderEvents":[{"type":"PLACE","order":{"orderId":"179f9af8-e45e-469d-b3e9-2fd4675cb7d0","cliOrdId":null,"type":"lmt","symbol":"PI_XBTUSD","side":"buy","quantity":10000,"filled":0,"limitPrice":9400,"reduceOnly":false,"timestamp":"2019-09-05T16:33:50.734Z","lastUpdateTimestamp":"2019-09-05T16:33:50.734Z"},"reducedQuantity":null}]},"serverTime":"2019-09-05T16:33:50.734Z"}"#;
        let r: KrakenOrderResponse = parse_response(text).unwrap();
        let events = r.data.send_status.order_events.unwrap();
        assert!(
            matches!(&events[0], KrakenOrderEvent::Place { order: Some(o), .. } if o.limit_price == 9400.0)
        );
    }

    #[test]
    fn parses_executed_order() {
        let text = r#"{"result":"success","sendStatus":{"order_id":"61ca5732-3478-42fe-8362-abbfd9465294","status":"placed","receivedTime":"2019-12-11T17:17:33.888Z","orderEvents":[{"type":"EXECUTION","executionId":"e1ec9f63-2338-4c44-b40a-43486c6732d7","price":7244.5,"amount":10,"orderPriorEdit":null,"orderPriorExecution":{"orderId":"61ca5732-3478-42fe-8362-abbfd9465294","cliOrdId":null,"type":"lmt","symbol":"PI_XBTUSD","side":"buy","quantity":10,"filled":0,"limitPrice":7500,"reduceOnly":false,"timestamp":"2019-12-11T17:17:33.888Z","lastUpdateTimestamp":"2019-12-11T17:17:33.888Z"},"takerReducedQuantity":null}]},"serverTime":"2019-12-11T17:17:33.888Z"}"#;
        let r: KrakenOrderResponse = parse_response(text).unwrap();
        let events = r.data.send_status.order_events.unwrap();
        assert!(
            matches!(&events[0], KrakenOrderEvent::Execution { price, .. } if *price == 7244.5)
        );
    }

    #[test]
    fn parses_rejected_order() {
        let text = r#"{"result":"success","sendStatus":{"order_id":"614a5298-0071-450f-83c6-0617ce8c6bc4","status":"iocWouldNotExecute","receivedTime":"2019-09-05T16:32:54.076Z","orderEvents":[{"type":"REJECT","uid":"614a5298-0071-450f-83c6-0617ce8c6bc4","reason":"IOC_WOULD_NOT_EXECUTE","order":{"orderId":"614a5298-0071-450f-83c6-0617ce8c6bc4","cliOrdId":null,"type":"lmt","symbol":"PI_XBTUSD","side":"buy","quantity":10000,"filled":0,"limitPrice":9400,"reduceOnly":true,"timestamp":"2019-09-05T16:32:54.076Z","lastUpdateTimestamp":"2019-09-05T16:32:54.076Z"}}]},"serverTime":"2019-09-05T16:32:54.077Z"}"#;
        let r: KrakenOrderResponse = parse_response(text).unwrap();
        assert_eq!(r.data.send_status.status, "iocWouldNotExecute");
    }

    #[test]
    fn parses_unaccepted_order_without_id() {
        // Documented in https://docs.kraken.com/exchange/guides/futures/rest#returns
        let text = r#"{"result":"success","serverTime":"2016-02-25T09:45:53.818Z","sendStatus":{"receivedTime":"2016-02-25T09:45:53.601Z","status":"insufficientAvailableFunds"}}"#;
        let r: KrakenOrderResponse = parse_response(text).unwrap();
        assert!(r.data.send_status.order_id.is_none());
    }

    #[test]
    fn parses_edit() {
        let text = r#"{"result":"success","editStatus":{"status":"edited","orderId":"022774bc-2c4a-4f26-9317-436c8d85746d","receivedTime":"2019-09-05T16:47:47.521Z","orderEvents":[{"type":"EDIT","old":{"orderId":"022774bc-2c4a-4f26-9317-436c8d85746d","cliOrdId":null,"type":"lmt","symbol":"PI_XBTUSD","side":"buy","quantity":1000,"filled":0,"limitPrice":9400,"reduceOnly":false,"timestamp":"2019-09-05T16:41:35.173Z","lastUpdateTimestamp":"2019-09-05T16:41:35.173Z"},"new":{"orderId":"022774bc-2c4a-4f26-9317-436c8d85746d","cliOrdId":null,"type":"lmt","symbol":"PI_XBTUSD","side":"buy","quantity":1501,"filled":0,"limitPrice":7200,"reduceOnly":false,"timestamp":"2019-09-05T16:41:35.173Z","lastUpdateTimestamp":"2019-09-05T16:47:47.519Z"},"reducedQuantity":null}]},"serverTime":"2019-09-05T16:47:47.521Z"}"#;
        let r: KrakenModifyResponse = parse_response(text).unwrap();
        assert_eq!(
            r.data.edit_status.order_id.as_deref(),
            Some("022774bc-2c4a-4f26-9317-436c8d85746d")
        );
    }

    #[test]
    fn parses_cancel() {
        let text = r#"{"result":"success","cancelStatus":{"status":"cancelled","order_id":"cb4e34f6-4eb3-4d4b-9724-4c3035b99d47","receivedTime":"2020-07-22T13:26:20.806Z","orderEvents":[{"type":"CANCEL","uid":"cb4e34f6-4eb3-4d4b-9724-4c3035b99d47","order":{"orderId":"cb4e34f6-4eb3-4d4b-9724-4c3035b99d47","cliOrdId":"1234568","type":"lmt","symbol":"PI_XBTUSD","side":"buy","quantity":5500,"filled":0,"limitPrice":8000,"reduceOnly":false,"timestamp":"2020-07-22T13:25:56.366Z","lastUpdateTimestamp":"2020-07-22T13:25:56.366Z"}}]},"serverTime":"2020-07-22T13:26:20.806Z"}"#;
        let r: KrakenCancelResponse = parse_response(text).unwrap();
        assert_eq!(r.data.cancel_status.status, "cancelled");
    }

    #[test]
    fn parses_open_orders() {
        let text = r#"{"result":"success","openOrders":[{"order_id":"59302619-41d2-4f0b-941f-7e7914760ad3","symbol":"PI_XBTUSD","side":"sell","orderType":"lmt","limitPrice":10640,"unfilledSize":304,"receivedTime":"2019-09-05T17:01:17.410Z","status":"untouched","filledSize":0,"reduceOnly":true,"lastUpdateTime":"2019-09-05T17:01:17.410Z"}],"serverTime":"2019-09-05T17:08:18.138Z"}"#;
        let r: KrakenOpenOrdersResponse = parse_response(text).unwrap();
        assert_eq!(r.data.open_orders[0].unfilled_size, Some(304.0));
    }

    #[test]
    fn parses_dead_mans_switch() {
        let text = r#"{"result":"success","status":{"currentTime":"2018-06-19T16:51:23.839Z","triggerTime":"2018-06-19T16:52:23.839Z"},"serverTime":"2018-06-19T16:51:23.839Z"}"#;
        let r: KrakenDeadMansSwitchResponse = parse_response(text).unwrap();
        assert_eq!(r.data.status.trigger_time, "2018-06-19T16:52:23.839Z");
    }

    fn create_test_client() -> Arc<RateLimitedClient> {
        Arc::new(RateLimitedClient::new(10, 1))
    }

    // Live tests only: the library itself never reads the environment.
    fn test_credentials() -> KrakenCredentials {
        KrakenCredentials::new(
            std::env::var("KRAKEN_PUBLIC_KEY").expect("KRAKEN_PUBLIC_KEY"),
            std::env::var("KRAKEN_PRIVATE_KEY").expect("KRAKEN_PRIVATE_KEY"),
        )
    }

    #[tokio::test]
    #[ignore]
    async fn test_create_order() -> Result<(), Box<dyn std::error::Error>> {
        let api = OrderManagementApi::new_with_client(create_test_client(), test_credentials());

        let order_result = api
            .create_order(
                "PF_XBTUSD",
                "buy",
                "post",
                "0.0001",
                Some("50000"),
                None,
                None,
                None,
                None,
            )
            .await?;
        assert_eq!(order_result.data.send_status.status, "placed");
        let order_id = order_result
            .data
            .send_status
            .order_id
            .clone()
            .expect("placed order has an id");

        let open_orders = api.get_open_orders().await?;
        assert_eq!(open_orders.result, "success");

        let modify_result = api
            .modify_order(Some(&order_id), None, Some("0.0002"), Some("49000"), None)
            .await?;
        assert_eq!(modify_result.result, "success");

        let status_result = api
            .get_order_status(Some(vec![order_id.clone()]), None)
            .await?;
        assert_eq!(status_result.result, "success");

        let cancel_result = api.cancel_order(Some(&order_id), None).await?;
        assert_eq!(cancel_result.data.cancel_status.status, "cancelled");
        Ok(())
    }

    #[tokio::test]
    #[ignore]
    async fn test_dead_mans_switch() -> Result<(), Box<dyn std::error::Error>> {
        let api = OrderManagementApi::new_with_client(create_test_client(), test_credentials());

        let dms_result = api.dead_mans_switch(60).await?;
        assert_eq!(dms_result.result, "success");

        let cancel_dms = api.dead_mans_switch(0).await?;
        assert_eq!(cancel_dms.result, "success");
        Ok(())
    }
}
