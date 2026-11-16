use super::{read_json, ApiResult, Query};
use crate::signer::{SignedCommand, SignedCreateProxy};
use crate::types::{
    AccountFills, AccountFundingHistory, Ack, AutoCancelResult, AutoCancelStatus, Balance,
    CancelResult, CreateProxyResult, Credentials, FillsSort, InstrumentConfig, LeverageResult,
    ModifyResult, Order, OrderAck, Portfolio,
};
use roshar_http::RateLimitedClient;
use serde::de::DeserializeOwned;

/// Proxy address and the secret returned by `createProxy`. Sent as `POLYMARKET-PROXY` /
/// `POLYMARKET-SECRET` on private reads, and in the WebSocket `auth` frame.
#[derive(Clone)]
pub struct ProxyCredentials {
    pub proxy: String,
    pub secret: String,
}

impl std::fmt::Debug for ProxyCredentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never print the secret.
        f.debug_struct("ProxyCredentials")
            .field("proxy", &self.proxy)
            .finish_non_exhaustive()
    }
}

/// Signed trading routes (`/v1/trade/*`, `/v1/account/proxy`) and authenticated account reads
/// (`/v1/account/*`). Signed routes need no headers; the signature identifies the proxy.
pub struct TradingApi {
    client: RateLimitedClient,
    base_url: String,
}

impl TradingApi {
    /// Pass a clone of the market client's `RateLimitedClient`: the IP budget is shared.
    pub fn new(client: RateLimitedClient, base_url: &str) -> Self {
        Self {
            client,
            base_url: base_url.to_string(),
        }
    }

    async fn send_body<T: DeserializeOwned>(
        &self,
        method: &str,
        path: &str,
        body: String,
        what: &str,
    ) -> ApiResult<T> {
        let method = reqwest::Method::from_bytes(method.as_bytes())?;
        let url = format!("{}{}", self.base_url, path);
        let response = self
            .client
            .request(method, &url)
            .await
            .header("content-type", "application/json")
            .body(body)
            .send()
            .await?;
        read_json(response, what).await
    }

    async fn send_command<T: DeserializeOwned>(
        &self,
        command: &SignedCommand,
        allowed: &[&str],
    ) -> ApiResult<T> {
        let op = command.op();
        if !allowed.contains(&op.type_name()) {
            return Err(format!(
                "{} cannot be sent here (expected {})",
                op.type_name(),
                allowed.join(" or ")
            )
            .into());
        }
        let (method, path) = op.rest_route();
        self.send_body(method, path, command.rest_body(), op.type_name())
            .await
    }

    /// `POST /v1/trade/orders`. One ack per order, in request order.
    pub async fn create_orders(&self, command: &SignedCommand) -> ApiResult<Vec<OrderAck>> {
        self.send_command(command, &["createOrders"]).await
    }

    /// `DELETE /v1/trade/orders` (`Op::CancelOrders`) or `DELETE /v1/trade/orders-coid`
    /// (`Op::CancelOrdersCoid`). One result per id.
    pub async fn cancel_orders(&self, command: &SignedCommand) -> ApiResult<Vec<CancelResult>> {
        self.send_command(command, &["cancelOrders", "cancelOrdersCOID"])
            .await
    }

    /// `PATCH /v1/trade/orders` (`Op::ModifyOrders`) or `PATCH /v1/trade/orders-coid`
    /// (`Op::ModifyOrdersCoid`). Only resting standalone GTC orders can be modified.
    pub async fn modify_orders(&self, command: &SignedCommand) -> ApiResult<Vec<ModifyResult>> {
        self.send_command(command, &["modifyOrders", "modifyOrdersCOID"])
            .await
    }

    /// `DELETE /v1/trade/orders/all`.
    pub async fn cancel_all(&self, command: &SignedCommand) -> ApiResult<Ack> {
        self.send_command(command, &["cancelAll"]).await
    }

    /// `PATCH /v1/trade/auto-cancel`: arm (time at least 5 s ahead) or clear (time 0).
    pub async fn set_auto_cancel(&self, command: &SignedCommand) -> ApiResult<AutoCancelResult> {
        self.send_command(command, &["autoCancel"]).await
    }

    /// `PATCH /v1/trade/leverage`.
    pub async fn update_leverage(&self, command: &SignedCommand) -> ApiResult<LeverageResult> {
        self.send_command(command, &["updateLeverage"]).await
    }

    /// `PATCH /v1/trade/leverage/batch`: sequential, not atomic; one result per instrument.
    pub async fn update_leverages(
        &self,
        command: &SignedCommand,
    ) -> ApiResult<Vec<LeverageResult>> {
        self.send_command(command, &["updateLeverages"]).await
    }

    /// `PATCH /v1/trade/margin`.
    pub async fn update_margin(&self, command: &SignedCommand) -> ApiResult<Ack> {
        self.send_command(command, &["updateMargin"]).await
    }

    /// `DELETE /v1/account/proxy` (owner-signed `Op::DeleteProxy`).
    pub async fn delete_proxy(&self, command: &SignedCommand) -> ApiResult<Ack> {
        self.send_command(command, &["deleteProxy"]).await
    }

    /// `POST /v1/account/proxy` (owner-signed). Store the returned secret with the proxy key.
    pub async fn create_proxy(&self, request: &SignedCreateProxy) -> ApiResult<CreateProxyResult> {
        self.send_body(
            "POST",
            "/v1/account/proxy",
            request.rest_body(),
            "createProxy",
        )
        .await
    }

    async fn get_auth<T: DeserializeOwned>(
        &self,
        credentials: &ProxyCredentials,
        url: String,
        what: &str,
    ) -> ApiResult<T> {
        let response = self
            .client
            .request(reqwest::Method::GET, &url)
            .await
            .header("POLYMARKET-PROXY", &credentials.proxy)
            .header("POLYMARKET-SECRET", &credentials.secret)
            .send()
            .await?;
        read_json(response, what).await
    }

    fn query(&self, path: &str) -> Query {
        Query::new(&self.base_url, path)
    }

    /// `GET /v1/account/credentials`: owner address and registered proxies.
    pub async fn get_credentials(&self, credentials: &ProxyCredentials) -> ApiResult<Credentials> {
        let url = self.query("/v1/account/credentials").url();
        self.get_auth(credentials, url, "credentials").await
    }

    /// `GET /v1/account/orders`: latest known state of matching orders, at most 100.
    pub async fn get_orders(
        &self,
        credentials: &ProxyCredentials,
        order_id: Option<u64>,
        client_order_id: Option<&str>,
        instrument_id: Option<u32>,
        start_timestamp: Option<i64>,
        end_timestamp: Option<i64>,
    ) -> ApiResult<Vec<Order>> {
        let url = self
            .query("/v1/account/orders")
            .opt("order_id", order_id)
            .opt("client_order_id", client_order_id)
            .opt("instrument_id", instrument_id)
            .opt("start_timestamp", start_timestamp)
            .opt("end_timestamp", end_timestamp)
            .url();
        self.get_auth(credentials, url, "orders").await
    }

    /// `GET /v1/account/open-orders[?instrument_id=]`.
    pub async fn get_open_orders(
        &self,
        credentials: &ProxyCredentials,
        instrument_id: Option<u32>,
    ) -> ApiResult<Vec<Order>> {
        let url = self
            .query("/v1/account/open-orders")
            .opt("instrument_id", instrument_id)
            .url();
        self.get_auth(credentials, url, "open orders").await
    }

    /// `GET /v1/account/balances`.
    pub async fn get_balances(&self, credentials: &ProxyCredentials) -> ApiResult<Vec<Balance>> {
        let url = self.query("/v1/account/balances").url();
        self.get_auth(credentials, url, "balances").await
    }

    /// `GET /v1/account/portfolio`: positions, margin and withdrawable balance.
    pub async fn get_portfolio(&self, credentials: &ProxyCredentials) -> ApiResult<Portfolio> {
        let url = self.query("/v1/account/portfolio").url();
        self.get_auth(credentials, url, "portfolio").await
    }

    /// `GET /v1/account/fills`, at most 100 per page; page with the returned `cursor`.
    pub async fn get_fills(
        &self,
        credentials: &ProxyCredentials,
        instrument_id: Option<u32>,
        start_timestamp: Option<i64>,
        end_timestamp: Option<i64>,
        cursor: Option<&str>,
        sort: Option<FillsSort>,
    ) -> ApiResult<AccountFills> {
        let url = self
            .query("/v1/account/fills")
            .opt("instrument_id", instrument_id)
            .opt("start_timestamp", start_timestamp)
            .opt("end_timestamp", end_timestamp)
            .opt("cursor", cursor)
            .opt("sort", sort.map(FillsSort::as_str))
            .url();
        self.get_auth(credentials, url, "fills").await
    }

    /// `GET /v1/account/funding`: funding payments, at most 100.
    pub async fn get_account_funding(
        &self,
        credentials: &ProxyCredentials,
        instrument_id: Option<u32>,
        start_timestamp: Option<i64>,
        end_timestamp: Option<i64>,
    ) -> ApiResult<AccountFundingHistory> {
        let url = self
            .query("/v1/account/funding")
            .opt("instrument_id", instrument_id)
            .opt("start_timestamp", start_timestamp)
            .opt("end_timestamp", end_timestamp)
            .url();
        self.get_auth(credentials, url, "account funding").await
    }

    /// `GET /v1/account/config[?instrument_id=]`: resolved leverage and margin mode.
    pub async fn get_config(
        &self,
        credentials: &ProxyCredentials,
        instrument_id: Option<u32>,
    ) -> ApiResult<Vec<InstrumentConfig>> {
        let url = self
            .query("/v1/account/config")
            .opt("instrument_id", instrument_id)
            .url();
        self.get_auth(credentials, url, "instrument config").await
    }

    /// `GET /v1/account/auto-cancel`.
    pub async fn get_auto_cancel(
        &self,
        credentials: &ProxyCredentials,
    ) -> ApiResult<AutoCancelStatus> {
        let url = self.query("/v1/account/auto-cancel").url();
        self.get_auth(credentials, url, "auto-cancel status").await
    }
}
