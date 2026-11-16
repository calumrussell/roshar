//! Order transaction routes of the GMX API (`/v2/orders/txns/*`), the path `@gmx-io/sdk` 2.9.1
//! uses for express (gasless, relayed) and classic orders.
//!
//! Keep one order's prepare, submit and status calls on the host that served prepare: the
//! `.io` and `.ai` deployments do not share `requestId` records. Use one `OrderTxApi` per host.

use crate::rest::http::{post_json, ApiResult};
use crate::types::{
    GmxCancelOrderRequest, GmxEditOrderRequest, GmxOrderStatus, GmxPrepareOrderRequest,
    GmxPreparedOrder, GmxSubmitOrderRequest, GmxSubmitResponse,
};
use roshar_http::RateLimitedClient;
use serde::Serialize;

pub struct OrderTxApi {
    client: RateLimitedClient,
    base_url: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StatusRequest<'a> {
    request_id: &'a str,
}

impl OrderTxApi {
    /// `base_url` is a deployment root such as [`crate::GMX_ARBITRUM_API_URL`].
    pub fn new(base_url: &str, requests_per_second: u32) -> Self {
        Self {
            client: RateLimitedClient::new(requests_per_second, 1),
            base_url: base_url.trim_end_matches('/').to_string(),
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base_url)
    }

    /// `POST /v2/orders/txns/prepare`: builds an increase, decrease or swap order (market, limit,
    /// stop-market, take-profit, stop-loss, TWAP; optional TP/SL sidecars). A 400 carries
    /// `code` such as `INSUFFICIENT_LIQUIDITY`, `MARKET_NOT_FOUND`, `TOKEN_NOT_FOUND`,
    /// `NO_SWAP_PATH`, `POSITION_NOT_FOUND`, `INVALID_PARAMS`.
    pub async fn prepare_order(&self, req: &GmxPrepareOrderRequest) -> ApiResult<GmxPreparedOrder> {
        post_json(
            &self.client,
            &self.url("/v2/orders/txns/prepare"),
            req,
            "prepare order",
        )
        .await
    }

    /// `POST /v2/orders/txns/edit/prepare`: changes size, trigger price, acceptable price or
    /// auto-cancel of open orders (404 if an order is not the account's).
    pub async fn prepare_edit_order(
        &self,
        req: &GmxEditOrderRequest,
    ) -> ApiResult<GmxPreparedOrder> {
        post_json(
            &self.client,
            &self.url("/v2/orders/txns/edit/prepare"),
            req,
            "prepare edit",
        )
        .await
    }

    /// `POST /v2/orders/txns/cancel/prepare`: cancels the given orders, or every open order with
    /// `all` (400 `No open orders found for account` when there are none).
    pub async fn prepare_cancel_order(
        &self,
        req: &GmxCancelOrderRequest,
    ) -> ApiResult<GmxPreparedOrder> {
        post_json(
            &self.client,
            &self.url("/v2/orders/txns/cancel/prepare"),
            req,
            "prepare cancel",
        )
        .await
    }

    /// `POST /v2/orders/txns/submit`: hands a signed express order to GMX Relay. A rejected
    /// signature is a 400 (`The relay rejected the operation.`).
    pub async fn submit_order(&self, req: &GmxSubmitOrderRequest) -> ApiResult<GmxSubmitResponse> {
        post_json(
            &self.client,
            &self.url("/v2/orders/txns/submit"),
            req,
            "submit order",
        )
        .await
    }

    /// `POST /v2/orders/txns/status`: relay, creation and execution state of a request
    /// (404 `Transaction not found` for an unknown `request_id`).
    pub async fn order_status(&self, request_id: &str) -> ApiResult<GmxOrderStatus> {
        post_json(
            &self.client,
            &self.url("/v2/orders/txns/status"),
            &StatusRequest { request_id },
            "order status",
        )
        .await
    }
}
