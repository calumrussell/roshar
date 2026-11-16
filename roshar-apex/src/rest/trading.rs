use super::{parse_reply, ApiResult, Query};
use crate::auth::ApexApiCredentials;
use crate::order::ApexSignedOrder;
use crate::types::{
    ApexAccount, ApexAccountBalance, ApexAck, ApexFundingPayments, ApexHistoryQuery, ApexOrder,
    ApexOrderPage, ApexWorstPrice,
};
use roshar_http::RateLimitedClient;
use serde::de::DeserializeOwned;

/// Private REST endpoints, authenticated with the API key headers (`APEX-API-KEY`,
/// `APEX-PASSPHRASE`, `APEX-TIMESTAMP`, `APEX-SIGNATURE`).
///
/// Every call takes `timestamp_ms`, the request time the exchange checks (`err APEX-TIMESTAMP`,
/// code 20002, when missing). The library does not read the clock.
pub struct TradingApi {
    client: RateLimitedClient,
    base_url: &'static str,
    credentials: ApexApiCredentials,
}

/// Path prefix the signature covers: `base_url` ends in `/api`, signed paths start with `/api`.
const SIGNED_PREFIX: &str = "/api";

impl TradingApi {
    /// `base_url` is `APEX_OMNI_REST_URL` or `APEX_OMNI_TESTNET_REST_URL`. Pass a clone of the
    /// client the market API uses: the 600 requests / 60 s limit is per IP.
    pub fn new(
        client: RateLimitedClient,
        base_url: &'static str,
        credentials: ApexApiCredentials,
    ) -> Self {
        Self {
            client,
            base_url,
            credentials,
        }
    }

    pub fn credentials(&self) -> &ApexApiCredentials {
        &self.credentials
    }

    fn headers(
        &self,
        builder: reqwest::RequestBuilder,
        timestamp_ms: u64,
        signature: String,
    ) -> reqwest::RequestBuilder {
        builder
            .header("APEX-SIGNATURE", signature)
            .header("APEX-API-KEY", &self.credentials.key)
            .header("APEX-TIMESTAMP", timestamp_ms.to_string())
            .header("APEX-PASSPHRASE", &self.credentials.passphrase)
    }

    async fn get<T: DeserializeOwned>(
        &self,
        path_and_query: &str,
        timestamp_ms: u64,
        what: &str,
    ) -> ApiResult<T> {
        let signed_path = format!("{SIGNED_PREFIX}{path_and_query}");
        let signature = self
            .credentials
            .sign_request(timestamp_ms, "GET", &signed_path, &[]);
        let url = format!("{}{path_and_query}", self.base_url);
        let builder = self.client.request(reqwest::Method::GET, &url).await;
        let response = self
            .headers(builder, timestamp_ms, signature)
            .send()
            .await?;
        parse_reply(response, what).await
    }

    async fn post<T: DeserializeOwned>(
        &self,
        path: &str,
        form: &[(&str, String)],
        timestamp_ms: u64,
        what: &str,
    ) -> ApiResult<T> {
        let signed_path = format!("{SIGNED_PREFIX}{path}");
        let signature = self
            .credentials
            .sign_request(timestamp_ms, "POST", &signed_path, form);
        let url = format!("{}{path}", self.base_url);
        let builder = self.client.post(&url).await.form(form);
        let response = self
            .headers(builder, timestamp_ms, signature)
            .send()
            .await?;
        parse_reply(response, what).await
    }

    // ── Orders ───────────────────────────────────────────────────────────────

    /// `POST /v3/order` with a payload from `order::sign_order`.
    pub async fn create_order(
        &self,
        order: &ApexSignedOrder,
        timestamp_ms: u64,
    ) -> ApiResult<ApexOrder> {
        self.post("/v3/order", &order.form, timestamp_ms, "create order")
            .await
    }

    /// `POST /v3/delete-order`: cancel by exchange order id. Returns the order id.
    pub async fn cancel_order(&self, id: &str, timestamp_ms: u64) -> ApiResult<String> {
        self.post(
            "/v3/delete-order",
            &[("id", id.to_string())],
            timestamp_ms,
            "cancel order",
        )
        .await
    }

    /// `POST /v3/delete-client-order-id`: cancel by client id. Returns the order id.
    pub async fn cancel_order_by_client_id(
        &self,
        client_id: &str,
        timestamp_ms: u64,
    ) -> ApiResult<String> {
        self.post(
            "/v3/delete-client-order-id",
            &[("id", client_id.to_string())],
            timestamp_ms,
            "cancel order by client id",
        )
        .await
    }

    /// `POST /v3/delete-open-orders`. `symbols` is a comma-separated list (`BTC-USDT,ETH-USDT`);
    /// `None` cancels every open order. `is_conditional` / `is_active` restrict the cancel to
    /// conditional or active orders.
    pub async fn cancel_all_orders(
        &self,
        symbols: Option<&str>,
        is_conditional: Option<bool>,
        is_active: Option<bool>,
        timestamp_ms: u64,
    ) -> ApiResult<()> {
        let mut form: Vec<(&str, String)> = Vec::new();
        if let Some(s) = symbols {
            form.push(("symbol", s.to_string()));
        }
        if let Some(v) = is_conditional {
            form.push(("isConditional", v.to_string()));
        }
        if let Some(v) = is_active {
            form.push(("isActive", v.to_string()));
        }
        let _: ApexAck = self
            .post(
                "/v3/delete-open-orders",
                &form,
                timestamp_ms,
                "cancel all orders",
            )
            .await?;
        Ok(())
    }

    /// `GET /v3/open-orders[?symbols=..]`, `symbols` comma-separated.
    pub async fn get_open_orders(
        &self,
        symbols: Option<&str>,
        timestamp_ms: u64,
    ) -> ApiResult<Vec<ApexOrder>> {
        let path = Query::new("/v3/open-orders").opt("symbols", symbols).path;
        self.get(&path, timestamp_ms, "open orders").await
    }

    /// `GET /v3/history-orders`.
    pub async fn get_order_history(
        &self,
        query: &ApexHistoryQuery<'_>,
        timestamp_ms: u64,
    ) -> ApiResult<ApexOrderPage> {
        let path = Query::new("/v3/history-orders")
            .opt("symbol", query.symbol)
            .opt("status", query.status)
            .opt("side", query.side)
            .opt("type", query.order_type)
            .opt("limit", query.limit)
            .opt("beginTimeInclusive", query.begin_time_inclusive)
            .opt("endTimeExclusive", query.end_time_exclusive)
            .opt("page", query.page)
            .opt("orderType", query.order_category)
            .path;
        self.get(&path, timestamp_ms, "order history").await
    }

    /// `GET /v3/order?id=..`.
    pub async fn get_order(&self, id: &str, timestamp_ms: u64) -> ApiResult<ApexOrder> {
        let path = Query::new("/v3/order").param("id", id).path;
        self.get(&path, timestamp_ms, "order").await
    }

    /// `GET /v3/order-by-client-order-id?id=..`.
    pub async fn get_order_by_client_id(
        &self,
        client_id: &str,
        timestamp_ms: u64,
    ) -> ApiResult<ApexOrder> {
        let path = Query::new("/v3/order-by-client-order-id")
            .param("id", client_id)
            .path;
        self.get(&path, timestamp_ms, "order by client id").await
    }

    /// `GET /v3/fills`: trade history.
    pub async fn get_fills(
        &self,
        query: &ApexHistoryQuery<'_>,
        timestamp_ms: u64,
    ) -> ApiResult<ApexOrderPage> {
        let path = Query::new("/v3/fills")
            .opt("symbol", query.symbol)
            .opt("side", query.side)
            .opt("limit", query.limit)
            .opt("beginTimeInclusive", query.begin_time_inclusive)
            .opt("endTimeExclusive", query.end_time_exclusive)
            .opt("page", query.page)
            .opt("orderType", query.order_category)
            .path;
        self.get(&path, timestamp_ms, "fills").await
    }

    /// `GET /v3/get-worst-price?symbol=..&size=..&side=..`: the price a market order of `size`
    /// would need. `symbol` is `BTC-USDT` form, `side` `BUY` / `SELL`.
    pub async fn get_worst_price(
        &self,
        symbol: &str,
        size: &str,
        side: &str,
        timestamp_ms: u64,
    ) -> ApiResult<ApexWorstPrice> {
        let path = Query::new("/v3/get-worst-price")
            .param("symbol", symbol)
            .param("size", size)
            .param("side", side)
            .path;
        self.get(&path, timestamp_ms, "worst price").await
    }

    // ── Account ──────────────────────────────────────────────────────────────

    /// `GET /v3/account`: account id, fee rates, contract wallets and positions.
    pub async fn get_account(&self, timestamp_ms: u64) -> ApiResult<ApexAccount> {
        self.get("/v3/account", timestamp_ms, "account").await
    }

    /// `GET /v3/account-balance`: equity, available balance and margin.
    pub async fn get_account_balance(&self, timestamp_ms: u64) -> ApiResult<ApexAccountBalance> {
        self.get("/v3/account-balance", timestamp_ms, "account balance")
            .await
    }

    /// `GET /v3/funding`: the account's funding payments.
    pub async fn get_funding_payments(
        &self,
        query: &ApexHistoryQuery<'_>,
        timestamp_ms: u64,
    ) -> ApiResult<ApexFundingPayments> {
        let path = Query::new("/v3/funding")
            .opt("symbol", query.symbol)
            .opt("limit", query.limit)
            .opt("page", query.page)
            .opt("beginTimeInclusive", query.begin_time_inclusive)
            .opt("endTimeExclusive", query.end_time_exclusive)
            .opt("side", query.side)
            .opt("status", query.status)
            .path;
        self.get(&path, timestamp_ms, "funding payments").await
    }

    /// `POST /v3/set-initial-margin-rate`: leverage for `symbol` (`initial_margin_rate` is
    /// 1 / leverage, e.g. `"0.05"`).
    pub async fn set_initial_margin_rate(
        &self,
        symbol: &str,
        initial_margin_rate: &str,
        timestamp_ms: u64,
    ) -> ApiResult<()> {
        let _: ApexAck = self
            .post(
                "/v3/set-initial-margin-rate",
                &[
                    ("symbol", symbol.to_string()),
                    ("initialMarginRate", initial_margin_rate.to_string()),
                ],
                timestamp_ms,
                "set initial margin rate",
            )
            .await?;
        Ok(())
    }
}
