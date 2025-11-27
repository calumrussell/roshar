#![allow(clippy::too_many_arguments)]

use reqwest::Client;
use serde::{Deserialize, Serialize};

use super::auth::AuthApi;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KrakenResponse<T> {
    pub result: String,
    #[serde(flatten)]
    pub data: T,
    #[serde(rename = "serverTime")]
    pub server_time: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KrakenOrderRequest {
    #[serde(rename = "orderType")]
    pub order_type: String, // "lmt", "mkt", "stp", "take"
    pub symbol: String,
    pub side: String, // "buy" or "sell"
    pub size: String,
    #[serde(rename = "limitPrice")]
    pub limit_price: String,
    #[serde(rename = "cliOrdId", skip_serializing_if = "Option::is_none")]
    pub cli_ord_id: Option<String>, // Client order ID - optional
    #[serde(rename = "stopPrice", skip_serializing_if = "Option::is_none")]
    pub stop_price: Option<String>,
    #[serde(rename = "reduceOnly", skip_serializing_if = "Option::is_none")]
    pub reduce_only: Option<bool>,
    #[serde(rename = "timeInForce", skip_serializing_if = "Option::is_none")]
    pub time_in_force: Option<String>, // "gtc", "ioc", "fok"
    #[serde(rename = "postOnly", skip_serializing_if = "Option::is_none")]
    pub post_only: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KrakenModifyOrderRequest {
    #[serde(rename = "orderId")]
    pub order_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<f64>,
    #[serde(rename = "limitPrice", skip_serializing_if = "Option::is_none")]
    pub limit_price: Option<f64>,
    #[serde(rename = "stopPrice", skip_serializing_if = "Option::is_none")]
    pub stop_price: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KrakenCancelOrderRequest {
    #[serde(rename = "order_id", skip_serializing_if = "Option::is_none")]
    pub order_id: Option<String>,
    #[serde(rename = "cliOrdId", skip_serializing_if = "Option::is_none")]
    pub cli_ord_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KrakenDeadMansSwitchRequest {
    pub timeout: u32, // timeout in seconds
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KrakenOrderData {
    #[serde(rename = "sendStatus")]
    pub send_status: KrakenSendStatus,
}

pub type KrakenOrderResponse = KrakenResponse<KrakenOrderData>;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KrakenOperationStatus<T> {
    #[serde(rename = "order_id", alias = "orderId")]
    pub order_id: String,
    pub status: String,
    #[serde(rename = "receivedTime")]
    pub received_time: String,
    #[serde(rename = "orderEvents")]
    pub order_events: Vec<T>,
}

pub type KrakenSendStatus = KrakenOperationStatus<KrakenOrderEvent>;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KrakenOrderEvent {
    pub order: KrakenOrder,
    #[serde(rename = "reducedQuantity")]
    pub reduced_quantity: Option<f64>,
    #[serde(rename = "type")]
    pub event_type: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KrakenCancelData {
    #[serde(rename = "cancelStatus")]
    pub cancel_status: KrakenCancelStatus,
}

pub type KrakenCancelResponse = KrakenResponse<KrakenCancelData>;

pub type KrakenCancelStatus = KrakenOperationStatus<KrakenCancelEvent>;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KrakenCancelEvent {
    pub uid: String,
    pub order: KrakenOrder,
    #[serde(rename = "type")]
    pub event_type: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KrakenOpenOrdersData {
    #[serde(rename = "openOrders")]
    pub open_orders: Vec<KrakenOrder>,
}

pub type KrakenOpenOrdersResponse = KrakenResponse<KrakenOpenOrdersData>;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KrakenOrder {
    #[serde(rename = "orderId", alias = "order_id")]
    pub order_id: String,
    #[serde(rename = "cliOrdId", skip_serializing_if = "Option::is_none")]
    pub cli_ord_id: Option<String>,
    #[serde(
        rename = "type",
        alias = "orderType",
        skip_serializing_if = "Option::is_none"
    )]
    pub order_type: Option<String>,
    pub symbol: String,
    pub side: String,
    #[serde(alias = "quantity", alias = "unfilledSize")]
    pub size: f64,
    #[serde(alias = "filled", alias = "filledSize")]
    pub filled_size: f64,
    #[serde(rename = "limitPrice")]
    pub limit_price: f64,
    #[serde(rename = "reduceOnly")]
    pub reduce_only: bool,
    #[serde(alias = "timestamp", alias = "receivedTime")]
    pub received_time: String,
    #[serde(rename = "lastUpdateTimestamp", alias = "lastUpdateTime")]
    pub last_update_time: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KrakenOrderStatusItem {
    pub order: KrakenOrder,
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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KrakenModifyData {
    #[serde(rename = "editStatus")]
    pub edit_status: KrakenEditStatus,
}

pub type KrakenModifyResponse = KrakenResponse<KrakenModifyData>;

pub type KrakenEditStatus = KrakenOperationStatus<KrakenEditEvent>;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KrakenEditEvent {
    pub old: KrakenOrder,
    pub new: KrakenOrder,
    #[serde(rename = "reducedQuantity")]
    pub reduced_quantity: Option<f64>,
    #[serde(rename = "type")]
    pub event_type: String,
}

pub struct OrderManagementApi;

impl OrderManagementApi {
    async fn make_request<T, R>(
        client: &Client,
        base_url: &String,
        endpoint: &str,
        request_data: &T,
    ) -> Result<R, Box<dyn std::error::Error>>
    where
        T: Serialize,
        R: for<'de> Deserialize<'de>,
    {
        let post_data = serde_urlencoded::to_string(request_data)?;
        let headers = AuthApi::create_headers(endpoint, &post_data).unwrap();

        let mut request = client.post(format!("{base_url}{endpoint}"));

        for (key, value) in headers {
            request = request.header(key, value);
        }

        request = request
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(post_data);

        let response = request.send().await?;
        let text = response.text().await?;

        let parsed: R = serde_json::from_str(&text)?;
        Ok(parsed)
    }

    async fn make_get_request<R>(
        client: &Client,
        base_url: &String,
        endpoint: &str,
    ) -> Result<R, Box<dyn std::error::Error>>
    where
        R: for<'de> Deserialize<'de>,
    {
        let headers = AuthApi::create_headers(endpoint, "").unwrap();

        let mut request = client.get(format!("{base_url}{endpoint}"));

        for (key, value) in headers {
            request = request.header(key, value);
        }

        let response = request.send().await?;
        let text = response.text().await?;

        let parsed: R = serde_json::from_str(&text)?;
        Ok(parsed)
    }

    pub async fn create_order(
        client: &Client,
        base_url: &String,
        symbol: &str,
        side: &str,
        order_type: &str,
        size: &str,
        limit_price: &str,
        cli_ord_id: Option<&str>,
        stop_price: Option<String>,
        reduce_only: Option<bool>,
        time_in_force: Option<&str>,
        post_only: Option<bool>,
    ) -> Result<KrakenOrderResponse, Box<dyn std::error::Error>> {
        let order_request = KrakenOrderRequest {
            order_type: order_type.to_string(),
            symbol: symbol.to_string(),
            side: side.to_string(),
            size: size.to_string(),
            limit_price: limit_price.to_string(),
            cli_ord_id: cli_ord_id.map(|s| s.to_string()),
            stop_price,
            reduce_only,
            time_in_force: time_in_force.map(|t| t.to_string()),
            post_only,
        };

        Self::make_request(
            client,
            base_url,
            "/derivatives/api/v3/sendorder",
            &order_request,
        )
        .await
    }

    pub async fn cancel_order(
        client: &Client,
        base_url: &String,
        order_id: Option<&str>,
        cli_ord_id: Option<&str>,
    ) -> Result<KrakenCancelResponse, Box<dyn std::error::Error>> {
        if order_id.is_none() && cli_ord_id.is_none() {
            return Err("Either order_id or cli_ord_id must be provided".into());
        }

        let cancel_request = KrakenCancelOrderRequest {
            order_id: order_id.map(|s| s.to_string()),
            cli_ord_id: None, // Let's try with just order_id first
        };

        Self::make_request(
            client,
            base_url,
            "/derivatives/api/v3/cancelorder",
            &cancel_request,
        )
        .await
    }

    pub async fn modify_order(
        client: &Client,
        base_url: &String,
        order_id: &str,
        size: Option<f64>,
        limit_price: Option<f64>,
        stop_price: Option<f64>,
    ) -> Result<KrakenModifyResponse, Box<dyn std::error::Error>> {
        let modify_request = KrakenModifyOrderRequest {
            order_id: order_id.to_string(),
            size,
            limit_price,
            stop_price,
        };

        Self::make_request(
            client,
            base_url,
            "/derivatives/api/v3/editorder",
            &modify_request,
        )
        .await
    }

    pub async fn get_open_orders(
        client: &Client,
        base_url: &String,
    ) -> Result<KrakenOpenOrdersResponse, Box<dyn std::error::Error>> {
        Self::make_get_request(client, base_url, "/derivatives/api/v3/openorders").await
    }

    pub async fn get_order_status(
        client: &Client,
        base_url: &String,
        order_ids: Option<Vec<String>>,
        cli_ord_ids: Option<Vec<String>>,
    ) -> Result<KrakenOrderStatusResponse, Box<dyn std::error::Error>> {
        if order_ids.is_none() && cli_ord_ids.is_none() {
            return Err("Either order_ids or cli_ord_ids must be provided".into());
        }

        // Build form data manually for array handling
        let mut form_data = Vec::new();

        if let Some(ids) = order_ids {
            for id in ids {
                form_data.push(("orderIds".to_string(), id));
            }
        }

        if let Some(ids) = cli_ord_ids {
            for id in ids {
                form_data.push(("cliOrdIds".to_string(), id));
            }
        }

        let post_data = serde_urlencoded::to_string(&form_data)?;
        let endpoint = "/derivatives/api/v3/orders/status";
        let headers = AuthApi::create_headers(endpoint, &post_data).unwrap();

        let mut request = client.post(format!("{base_url}{endpoint}"));

        for (key, value) in headers {
            request = request.header(key, value);
        }

        request = request
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(post_data);

        let response = request.send().await?;
        let text = response.text().await?;

        let parsed: KrakenOrderStatusResponse = serde_json::from_str(&text)?;
        Ok(parsed)
    }

    pub async fn dead_mans_switch(
        client: &Client,
        base_url: &String,
        timeout: u32,
    ) -> Result<KrakenDeadMansSwitchResponse, Box<dyn std::error::Error>> {
        let dms_request = KrakenDeadMansSwitchRequest { timeout };

        Self::make_request(
            client,
            base_url,
            "/derivatives/api/v3/cancelallordersafter",
            &dms_request,
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use std::{thread::sleep, time::Duration};

    use super::OrderManagementApi;
    use reqwest::Client;

    fn get_test_client_and_url() -> (Client, String) {
        (Client::new(), "https://demo-futures.kraken.com".to_string())
    }

    #[tokio::test]
    #[ignore]
    async fn test_create_order() -> Result<(), Box<dyn std::error::Error>> {
        let (client, base_url) = get_test_client_and_url();

        sleep(Duration::from_millis(100));

        let order_result = OrderManagementApi::create_order(
            &client,
            &base_url,
            "PF_XBTUSD",
            "buy",
            "post",
            "0.0001",
            "50000",
            None,
            None,
            None,
            Some("gtc"),
            None,
        )
        .await?;

        let open_orders = OrderManagementApi::get_open_orders(&client, &base_url).await?;
        assert_eq!(open_orders.result, "success");

        let modify_result = OrderManagementApi::modify_order(
            &client,
            &base_url,
            &order_result.data.send_status.order_id,
            Some(0.0002),
            Some(29000.0),
            None,
        )
        .await?;
        assert_eq!(modify_result.result, "success");

        let status_result = OrderManagementApi::get_order_status(
            &client,
            &base_url,
            Some(vec![order_result.data.send_status.order_id.clone()]),
            None,
        )
        .await?;
        assert_eq!(status_result.result, "success");

        let cancel_result = OrderManagementApi::cancel_order(
            &client,
            &base_url,
            Some(&order_result.data.send_status.order_id),
            None,
        )
        .await?;
        assert_eq!(cancel_result.result, "success");

        assert_eq!(order_result.result, "success");
        Ok(())
    }

    #[tokio::test]
    #[ignore]
    async fn test_dead_mans_switch() -> Result<(), Box<dyn std::error::Error>> {
        let (client, base_url) = get_test_client_and_url();

        sleep(Duration::from_millis(220));

        let dms_result = OrderManagementApi::dead_mans_switch(&client, &base_url, 60).await?;
        assert_eq!(dms_result.result, "success");

        let cancel_dms = OrderManagementApi::dead_mans_switch(&client, &base_url, 0).await?;
        assert_eq!(cancel_dms.result, "success");
        Ok(())
    }
}
