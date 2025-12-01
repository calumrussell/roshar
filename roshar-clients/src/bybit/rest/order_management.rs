use serde::{Deserialize, Serialize};

use super::auth::AuthApi;

// ByBit Order Request Structures
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ByBitCreateOrderRequest {
    pub category: String,   // "spot", "linear", "inverse", "option"
    pub symbol: String,     // Symbol name, like BTCUSDT, uppercase only
    pub side: String,       // "Buy" or "Sell"
    pub order_type: String, // "Market" or "Limit"
    pub qty: String,        // Order quantity
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price: Option<String>, // Order price (required for limit orders)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order_link_id: Option<String>, // Unique user-set order ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_in_force: Option<String>, // "GTC", "IOC", "FOK", "PostOnly"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reduce_only: Option<bool>, // Reduce only order
    #[serde(skip_serializing_if = "Option::is_none")]
    pub close_on_trigger: Option<bool>, // Close on trigger order
}

// ByBit Order Response Structures
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ByBitCreateOrderResponse {
    pub ret_code: i32,
    pub ret_msg: String,
    pub result: ByBitOrderResult,
    pub ret_ext_info: serde_json::Value,
    pub time: u64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ByBitOrderResult {
    pub order_id: String,
    pub order_link_id: String,
}

// Helper functions for creating orders
impl ByBitCreateOrderRequest {
    pub fn new_market_order(category: &str, symbol: &str, side: &str, qty: &str) -> Self {
        Self {
            category: category.to_string(),
            symbol: symbol.to_string(),
            side: side.to_string(),
            order_type: "Market".to_string(),
            qty: qty.to_string(),
            price: None,
            order_link_id: None,
            time_in_force: None,
            reduce_only: None,
            close_on_trigger: None,
        }
    }

    pub fn new_limit_order(
        category: &str,
        symbol: &str,
        side: &str,
        qty: &str,
        price: &str,
    ) -> Self {
        Self {
            category: category.to_string(),
            symbol: symbol.to_string(),
            side: side.to_string(),
            order_type: "Limit".to_string(),
            qty: qty.to_string(),
            price: Some(price.to_string()),
            order_link_id: None,
            time_in_force: Some("GTC".to_string()),
            reduce_only: None,
            close_on_trigger: None,
        }
    }
}

/// ByBit Order Management API
pub struct OrderManagementApi;

impl OrderManagementApi {
    pub async fn create_order(
        request: &ByBitCreateOrderRequest,
    ) -> Result<ByBitCreateOrderResponse, Box<dyn std::error::Error + Send + Sync>> {
        let base_url = AuthApi::get_base_url();
        let endpoint = "/v5/order/create";
        let url = format!("{base_url}{endpoint}");

        // Serialize the request to JSON
        let json_body = serde_json::to_string(request)?;

        // Create authentication headers
        let headers = AuthApi::create_headers_post(&json_body)?;

        // Build the request
        let client = crate::http::get_http_client();
        let mut request_builder = client.post(&url).body(json_body);

        // Add headers
        for (key, value) in headers {
            request_builder = request_builder.header(key, value);
        }

        // Send the request
        let response = request_builder.send().await?;

        if !response.status().is_success() {
            return Err(format!(
                "ByBit API request failed with status: {}",
                response.status()
            )
            .into());
        }

        let response_text = response.text().await?;
        let order_response: ByBitCreateOrderResponse = serde_json::from_str(&response_text)
            .map_err(|e| format!("Failed to parse ByBit response: {e}"))?;

        if order_response.ret_code != 0 {
            return Err(format!("ByBit API error: {}", order_response.ret_msg).into());
        }

        Ok(order_response)
    }
}
