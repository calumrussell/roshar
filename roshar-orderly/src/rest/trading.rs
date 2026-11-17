use super::{parse_data, ApiResult, Rows};
use crate::request::{
    Method, OrderlyEditOrderRequest, OrderlyFundingFeeQuery, OrderlyOrderRequest,
    OrderlyOrdersQuery, OrderlyTradesQuery, PreparedRequest,
};
use crate::signer::OrderlyKey;
use crate::types::trading::{
    OrderlyAccountOverview, OrderlyBatchOrderAck, OrderlyCancelAllAfterAck, OrderlyClientInfo,
    OrderlyFill, OrderlyFillRows, OrderlyFundingFee, OrderlyHoldings, OrderlyKeyInfoRows,
    OrderlyOrder, OrderlyOrderAck, OrderlyPosition, OrderlyPositions, OrderlyStatusAck,
};
use crate::types::{OrderlyKline, OrderlyOrderbook, OrderlyPage};
use roshar_http::RateLimitedClient;
use serde::de::DeserializeOwned;

/// Endpoints signed with an Orderly key. Every call takes `timestamp_ms`, the request time the
/// caller vouches for: Orderly rejects it if it is more than 300 s from server time. Order
/// placement needs the `trading` scope; reads need `read`.
pub struct TradingApi {
    client: RateLimitedClient,
    base_url: String,
    account_id: String,
    key: OrderlyKey,
}

impl TradingApi {
    /// `account_id` is the `0x…` Orderly account id (see `signer::account_id` or
    /// `AccountApi::get_account`). Pass a clone of a shared `RateLimitedClient` if other APIs use
    /// the same key: Orderly's limits are per key (and per IP for public endpoints).
    pub fn new(
        client: RateLimitedClient,
        base_url: &str,
        account_id: String,
        key: OrderlyKey,
    ) -> Self {
        Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
            account_id,
            key,
        }
    }

    pub fn account_id(&self) -> &str {
        &self.account_id
    }

    pub fn key(&self) -> &OrderlyKey {
        &self.key
    }

    /// Signs and sends `request`, returning the parsed `data`.
    pub async fn execute<T: DeserializeOwned>(
        &self,
        timestamp_ms: u64,
        request: &PreparedRequest,
        what: &str,
    ) -> ApiResult<T> {
        let headers = self
            .key
            .auth_headers(&self.account_id, timestamp_ms, request);
        let url = format!("{}{}", self.base_url, request.path);
        self.client.acquire().await;
        let inner = self.client.inner();
        let builder = match request.method {
            Method::Get => inner.get(&url),
            Method::Post => inner.post(&url),
            Method::Put => inner.put(&url),
            Method::Delete => inner.delete(&url),
        };
        let mut builder = builder
            .header("Content-Type", request.method.content_type())
            .header("orderly-timestamp", headers.timestamp)
            .header("orderly-account-id", headers.account_id)
            .header("orderly-key", headers.key)
            .header("orderly-signature", headers.signature);
        if let Some(body) = &request.body {
            builder = builder.body(body.clone());
        }
        let response = builder.send().await?;
        let status = response.status().as_u16();
        let body = response.text().await?;
        Ok(parse_data(status, &body, what)?)
    }

    // ── orders ──────────────────────────────────────────────────────────────

    /// `POST /v1/order`. The ack only means accepted; fills arrive on `executionreport`.
    pub async fn create_order(
        &self,
        timestamp_ms: u64,
        order: &OrderlyOrderRequest,
    ) -> ApiResult<OrderlyOrderAck> {
        let req = PreparedRequest::create_order(order)?;
        self.execute(timestamp_ms, &req, "create order").await
    }

    /// `POST /v1/batch-order` (at most 10 orders).
    pub async fn batch_create_orders(
        &self,
        timestamp_ms: u64,
        orders: &[OrderlyOrderRequest],
    ) -> ApiResult<OrderlyBatchOrderAck> {
        let req = PreparedRequest::batch_create_orders(orders)?;
        self.execute(timestamp_ms, &req, "batch create order").await
    }

    /// `PUT /v1/order` — change price and/or quantity.
    pub async fn edit_order(
        &self,
        timestamp_ms: u64,
        edit: &OrderlyEditOrderRequest,
    ) -> ApiResult<OrderlyStatusAck> {
        let req = PreparedRequest::edit_order(edit)?;
        self.execute(timestamp_ms, &req, "edit order").await
    }

    /// `DELETE /v1/order`
    pub async fn cancel_order(
        &self,
        timestamp_ms: u64,
        order_id: u64,
        symbol: &str,
    ) -> ApiResult<OrderlyStatusAck> {
        let req = PreparedRequest::cancel_order(order_id, symbol)?;
        self.execute(timestamp_ms, &req, "cancel order").await
    }

    /// `DELETE /v1/client/order`
    pub async fn cancel_order_by_client_order_id(
        &self,
        timestamp_ms: u64,
        client_order_id: &str,
        symbol: &str,
    ) -> ApiResult<OrderlyStatusAck> {
        let req = PreparedRequest::cancel_order_by_client_order_id(client_order_id, symbol)?;
        self.execute(timestamp_ms, &req, "cancel order by client order id")
            .await
    }

    /// `DELETE /v1/orders[?symbol=..]`
    pub async fn cancel_all_orders(
        &self,
        timestamp_ms: u64,
        symbol: Option<&str>,
    ) -> ApiResult<OrderlyStatusAck> {
        let req = PreparedRequest::cancel_all_orders(symbol)?;
        self.execute(timestamp_ms, &req, "cancel all orders").await
    }

    /// `DELETE /v1/batch-order` (at most 10 ids).
    pub async fn batch_cancel_orders(
        &self,
        timestamp_ms: u64,
        order_ids: &[u64],
    ) -> ApiResult<OrderlyStatusAck> {
        let req = PreparedRequest::batch_cancel_orders(order_ids)?;
        self.execute(timestamp_ms, &req, "batch cancel orders")
            .await
    }

    /// `DELETE /v1/client/batch-order` (at most 10 ids).
    pub async fn batch_cancel_orders_by_client_order_id(
        &self,
        timestamp_ms: u64,
        client_order_ids: &[&str],
    ) -> ApiResult<OrderlyStatusAck> {
        let req = PreparedRequest::batch_cancel_orders_by_client_order_id(client_order_ids)?;
        self.execute(timestamp_ms, &req, "batch cancel orders by client order id")
            .await
    }

    /// `POST /v1/order/cancel_all_after`
    pub async fn cancel_all_after(
        &self,
        timestamp_ms: u64,
        trigger_after_ms: u64,
    ) -> ApiResult<OrderlyCancelAllAfterAck> {
        let req = PreparedRequest::cancel_all_after(trigger_after_ms)?;
        self.execute(timestamp_ms, &req, "cancel all after").await
    }

    // ── order and fill reads ────────────────────────────────────────────────

    /// `GET /v1/orders` — `status: Some("INCOMPLETE")` lists open orders.
    pub async fn get_orders(
        &self,
        timestamp_ms: u64,
        query: &OrderlyOrdersQuery<'_>,
    ) -> ApiResult<OrderlyPage<OrderlyOrder>> {
        let req = PreparedRequest::get_orders(query)?;
        self.execute(timestamp_ms, &req, "orders").await
    }

    /// `GET /v1/order/{order_id}`
    pub async fn get_order(&self, timestamp_ms: u64, order_id: u64) -> ApiResult<OrderlyOrder> {
        let req = PreparedRequest::get_order(order_id)?;
        self.execute(timestamp_ms, &req, "order").await
    }

    /// `GET /v1/client/order/{client_order_id}`
    pub async fn get_order_by_client_order_id(
        &self,
        timestamp_ms: u64,
        client_order_id: &str,
    ) -> ApiResult<OrderlyOrder> {
        let req = PreparedRequest::get_order_by_client_order_id(client_order_id)?;
        self.execute(timestamp_ms, &req, "order by client order id")
            .await
    }

    /// `GET /v1/order/{order_id}/trades`
    pub async fn get_order_trades(
        &self,
        timestamp_ms: u64,
        order_id: u64,
    ) -> ApiResult<Vec<OrderlyFill>> {
        let req = PreparedRequest::get_order_trades(order_id)?;
        let rows: OrderlyFillRows = self.execute(timestamp_ms, &req, "order trades").await?;
        Ok(rows.rows)
    }

    /// `GET /v1/trades`
    pub async fn get_trades(
        &self,
        timestamp_ms: u64,
        query: &OrderlyTradesQuery<'_>,
    ) -> ApiResult<OrderlyPage<OrderlyFill>> {
        let req = PreparedRequest::get_trades(query)?;
        self.execute(timestamp_ms, &req, "trades").await
    }

    /// `GET /v1/trade/{trade_id}`
    pub async fn get_trade(&self, timestamp_ms: u64, trade_id: u64) -> ApiResult<OrderlyFill> {
        let req = PreparedRequest::get_trade(trade_id)?;
        self.execute(timestamp_ms, &req, "trade").await
    }

    // ── account ─────────────────────────────────────────────────────────────

    /// `GET /v1/positions`
    pub async fn get_positions(
        &self,
        timestamp_ms: u64,
        product_type: Option<&str>,
    ) -> ApiResult<OrderlyPositions> {
        let req = PreparedRequest::get_positions(product_type)?;
        self.execute(timestamp_ms, &req, "positions").await
    }

    /// `GET /v1/position/{symbol}`
    pub async fn get_position(
        &self,
        timestamp_ms: u64,
        symbol: &str,
    ) -> ApiResult<OrderlyPosition> {
        let req = PreparedRequest::get_position(symbol)?;
        self.execute(timestamp_ms, &req, "position").await
    }

    /// `GET /v1/client/holding` — balances per token.
    pub async fn get_holding(
        &self,
        timestamp_ms: u64,
        all: Option<bool>,
    ) -> ApiResult<OrderlyHoldings> {
        let req = PreparedRequest::get_holding(all)?;
        self.execute(timestamp_ms, &req, "holding").await
    }

    /// `GET /v1/client/info` — account settings and fee rates.
    pub async fn get_client_info(&self, timestamp_ms: u64) -> ApiResult<OrderlyClientInfo> {
        let req = PreparedRequest::get_client_info()?;
        self.execute(timestamp_ms, &req, "client info").await
    }

    /// `GET /v1/account_info` — collateral, margin ratios, account value.
    pub async fn get_account_info(&self, timestamp_ms: u64) -> ApiResult<OrderlyAccountOverview> {
        let req = PreparedRequest::get_account_info()?;
        self.execute(timestamp_ms, &req, "account info").await
    }

    /// `GET /v1/funding_fee/history`
    pub async fn get_funding_fee_history(
        &self,
        timestamp_ms: u64,
        query: &OrderlyFundingFeeQuery<'_>,
    ) -> ApiResult<OrderlyPage<OrderlyFundingFee>> {
        let req = PreparedRequest::get_funding_fee_history(query)?;
        self.execute(timestamp_ms, &req, "funding fee history")
            .await
    }

    /// `GET /v1/client/key_info`
    pub async fn get_key_info(
        &self,
        timestamp_ms: u64,
        key_status: Option<&str>,
    ) -> ApiResult<OrderlyKeyInfoRows> {
        let req = PreparedRequest::get_key_info(key_status)?;
        self.execute(timestamp_ms, &req, "key info").await
    }

    // ── market data that needs an Orderly key ───────────────────────────────

    /// `GET /v1/orderbook/{symbol}`
    pub async fn get_orderbook(
        &self,
        timestamp_ms: u64,
        symbol: &str,
        max_level: Option<u32>,
    ) -> ApiResult<OrderlyOrderbook> {
        let req = PreparedRequest::get_orderbook(symbol, max_level)?;
        self.execute(timestamp_ms, &req, "orderbook").await
    }

    /// `GET /v1/kline`
    pub async fn get_kline(
        &self,
        timestamp_ms: u64,
        symbol: &str,
        kline_type: &str,
        limit: Option<u32>,
    ) -> ApiResult<Vec<OrderlyKline>> {
        let req = PreparedRequest::get_kline(symbol, kline_type, limit)?;
        let rows: Rows<OrderlyKline> = self.execute(timestamp_ms, &req, "kline").await?;
        Ok(rows.rows)
    }
}
