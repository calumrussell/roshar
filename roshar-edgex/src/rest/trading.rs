use super::{build_query, parse_data, parse_void, read_reply, BoxError};
use crate::signer::{canonical_query, canonical_value, ApiCredentials};
use crate::types::{
    CancelAllFilter, CreateOrderBody, EdgexAccount, EdgexAccountAsset, EdgexCancelResult,
    EdgexCollateral, EdgexCreateOrderResult, EdgexMaxOrderSize, EdgexOrder, EdgexOrderFill,
    EdgexPage, EdgexPosition, FillPageQuery, OrderPageQuery, SetMarginModeBody,
};
use roshar_http::RateLimitedClient;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{json, Map, Value};

type ApiResult<T> = Result<T, BoxError>;

/// A fully signed private request, ready to send. Built offline so it can be checked against the
/// SDK's vectors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedRequest {
    /// `GET` or `POST`.
    pub method: &'static str,
    pub url: String,
    /// The four `X-edgeX-*` headers (plus `Content-Type` for POST is added on send).
    pub headers: Vec<(String, String)>,
    /// JSON body for POST.
    pub body: Option<String>,
    /// The string the HMAC covers after `timestamp + method + path`.
    pub sign_payload: String,
}

/// Private REST for one account. Every call takes the request timestamp (ms) for the
/// `X-edgeX-Timestamp` header; the library never reads the clock.
pub struct TradingApi {
    client: RateLimitedClient,
    base_url: String,
    credentials: ApiCredentials,
    account_id: u64,
}

fn csv<T: AsRef<str>>(items: &[T]) -> String {
    items
        .iter()
        .map(|s| s.as_ref())
        .collect::<Vec<_>>()
        .join(",")
}

fn push<T: ToString>(params: &mut Vec<(&'static str, String)>, key: &'static str, v: Option<T>) {
    if let Some(v) = v {
        params.push((key, v.to_string()));
    }
}

fn push_list(params: &mut Vec<(&'static str, String)>, key: &'static str, items: Vec<String>) {
    if !items.is_empty() {
        params.push((key, items.join(",")));
    }
}

impl TradingApi {
    /// `base_url` is `EDGEX_API_URL` in production. Pass a clone of the market API's
    /// `RateLimitedClient` to share one budget.
    pub fn new(
        client: RateLimitedClient,
        base_url: &str,
        credentials: ApiCredentials,
        account_id: u64,
    ) -> Self {
        Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
            credentials,
            account_id,
        }
    }

    pub fn account_id(&self) -> u64 {
        self.account_id
    }

    fn account_param(&self) -> (&'static str, String) {
        ("accountId", self.account_id.to_string())
    }

    /// Signs a GET. The query is sent sorted, as signed.
    pub fn signed_get(
        &self,
        path: &str,
        params: &[(&str, String)],
        timestamp_ms: u64,
    ) -> SignedRequest {
        let mut sorted: Vec<(&str, String)> = params.to_vec();
        sorted.sort_by(|a, b| a.0.cmp(b.0).then_with(|| a.1.cmp(&b.1)));
        let payload = canonical_query(&sorted);
        let url = if sorted.is_empty() {
            format!("{}{path}", self.base_url)
        } else {
            format!("{}{path}?{}", self.base_url, build_query(&sorted))
        };
        SignedRequest {
            method: "GET",
            url,
            headers: self
                .credentials
                .headers(timestamp_ms, "GET", path, &payload),
            body: None,
            sign_payload: payload,
        }
    }

    /// Signs a POST with a JSON body.
    pub fn signed_post<B: Serialize>(
        &self,
        path: &str,
        body: &B,
        timestamp_ms: u64,
    ) -> Result<SignedRequest, BoxError> {
        let value = serde_json::to_value(body)?;
        let payload = canonical_value(&value);
        Ok(SignedRequest {
            method: "POST",
            url: format!("{}{path}", self.base_url),
            headers: self
                .credentials
                .headers(timestamp_ms, "POST", path, &payload),
            body: Some(serde_json::to_string(&value)?),
            sign_payload: payload,
        })
    }

    /// Sends a signed request and returns `(HTTP status, body)`.
    pub async fn send_raw(&self, req: &SignedRequest) -> ApiResult<(u16, String)> {
        let mut builder = match req.method {
            "GET" => self.client.request(reqwest::Method::GET, &req.url).await,
            _ => self.client.request(reqwest::Method::POST, &req.url).await,
        };
        for (k, v) in &req.headers {
            builder = builder.header(k.as_str(), v.as_str());
        }
        if let Some(body) = &req.body {
            builder = builder
                .header("Content-Type", "application/json")
                .body(body.clone());
        }
        read_reply(builder.send().await?).await
    }

    async fn get<T: DeserializeOwned>(
        &self,
        path: &str,
        params: &[(&str, String)],
        timestamp_ms: u64,
        what: &str,
    ) -> ApiResult<T> {
        let req = self.signed_get(path, params, timestamp_ms);
        let (status, text) = self.send_raw(&req).await?;
        parse_data(status, &text, what)
    }

    async fn post<B: Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
        timestamp_ms: u64,
        what: &str,
    ) -> ApiResult<T> {
        let req = self.signed_post(path, body, timestamp_ms)?;
        let (status, text) = self.send_raw(&req).await?;
        parse_data(status, &text, what)
    }

    // ── orders ──

    pub const CREATE_ORDER: &'static str = "/api/v2/private/order/createOrder";
    pub const CANCEL_ORDER_BY_ID: &'static str = "/api/v2/private/order/cancelOrderById";
    pub const CANCEL_ORDER_BY_CLIENT_ORDER_ID: &'static str =
        "/api/v2/private/order/cancelOrderByClientOrderId";
    pub const CANCEL_ALL_ORDER: &'static str = "/api/v2/private/order/cancelAllOrder";
    pub const GET_ACTIVE_ORDER_PAGE: &'static str = "/api/v2/private/order/getActiveOrderPage";
    pub const GET_HISTORY_ORDER_PAGE: &'static str = "/api/v2/private/order/getHistoryOrderPage";
    pub const GET_FILL_PAGE: &'static str =
        "/api/v2/private/order/getHistoryOrderFillTransactionPage";
    pub const GET_ORDER_BY_ID: &'static str = "/api/v2/private/order/getOrderById";
    pub const GET_ORDER_BY_CLIENT_ORDER_ID: &'static str =
        "/api/v2/private/order/getOrderByClientOrderId";
    pub const GET_MAX_CREATE_ORDER_SIZE: &'static str =
        "/api/v2/private/order/getMaxCreateOrderSize";
    pub const GET_ACCOUNT_ASSET: &'static str = "/api/v2/private/account/getAccountAsset";
    pub const GET_ACCOUNT_BY_ID: &'static str = "/api/v2/private/account/getAccountById";
    pub const GET_POSITION_BY_CONTRACT_ID: &'static str =
        "/api/v2/private/account/getPositionByContractId";
    pub const GET_COLLATERAL_BY_COIN_ID: &'static str =
        "/api/v2/private/account/getCollateralByCoinId";
    pub const UPDATE_LEVERAGE_SETTING: &'static str =
        "/api/v2/private/account/updateLeverageSetting";
    pub const SET_MARGIN_MODE: &'static str = "/api/v2/private/account/setMarginMode";

    /// `createOrder`. Build `body` with `L2Signer::create_order_body` for this account.
    pub async fn create_order(
        &self,
        body: &CreateOrderBody,
        timestamp_ms: u64,
    ) -> ApiResult<EdgexCreateOrderResult> {
        self.post(Self::CREATE_ORDER, body, timestamp_ms, "createOrder")
            .await
    }

    pub fn cancel_order_by_id_body(&self, order_ids: &[&str]) -> Value {
        json!({"accountId": self.account_id.to_string(), "orderIdList": order_ids})
    }

    pub async fn cancel_order_by_id(
        &self,
        order_ids: &[&str],
        timestamp_ms: u64,
    ) -> ApiResult<EdgexCancelResult> {
        let body = self.cancel_order_by_id_body(order_ids);
        self.post(
            Self::CANCEL_ORDER_BY_ID,
            &body,
            timestamp_ms,
            "cancelOrderById",
        )
        .await
    }

    pub fn cancel_order_by_client_order_id_body(&self, client_order_ids: &[&str]) -> Value {
        json!({"accountId": self.account_id.to_string(), "clientOrderIdList": client_order_ids})
    }

    pub async fn cancel_order_by_client_order_id(
        &self,
        client_order_ids: &[&str],
        timestamp_ms: u64,
    ) -> ApiResult<EdgexCancelResult> {
        let body = self.cancel_order_by_client_order_id_body(client_order_ids);
        self.post(
            Self::CANCEL_ORDER_BY_CLIENT_ORDER_ID,
            &body,
            timestamp_ms,
            "cancelOrderByClientOrderId",
        )
        .await
    }

    pub fn cancel_all_body(&self, f: &CancelAllFilter) -> Value {
        let mut m = Map::new();
        m.insert("accountId".into(), json!(self.account_id.to_string()));
        if let Some(v) = &f.coin_ids {
            m.insert("filterCoinIdList".into(), json!(v));
        }
        if let Some(v) = &f.contract_ids {
            m.insert("filterContractIdList".into(), json!(v));
        }
        if let Some(v) = &f.order_types {
            m.insert("filterOrderTypeList".into(), json!(v));
        }
        if let Some(v) = &f.order_statuses {
            m.insert("filterOrderStatusList".into(), json!(v));
        }
        if let Some(v) = &f.is_position_tpsl {
            m.insert("filterIsPositionTpsl".into(), json!(v));
        }
        Value::Object(m)
    }

    pub async fn cancel_all_orders(
        &self,
        filter: &CancelAllFilter,
        timestamp_ms: u64,
    ) -> ApiResult<EdgexCancelResult> {
        let body = self.cancel_all_body(filter);
        self.post(
            Self::CANCEL_ALL_ORDER,
            &body,
            timestamp_ms,
            "cancelAllOrder",
        )
        .await
    }

    /// Query params for the active/history order pages (without `filterOrderIdList` when
    /// `include_order_ids` is false).
    pub fn order_page_params(
        &self,
        q: &OrderPageQuery,
        include_order_ids: bool,
    ) -> Vec<(&'static str, String)> {
        let mut p = vec![self.account_param()];
        push(&mut p, "size", q.size);
        push(&mut p, "offsetData", q.offset_data.as_deref());
        push_list(&mut p, "filterCoinIdList", q.coin_ids.clone());
        push_list(&mut p, "filterContractIdList", q.contract_ids.clone());
        push_list(
            &mut p,
            "filterTypeList",
            q.types.iter().map(|t| t.as_str().to_string()).collect(),
        );
        push_list(
            &mut p,
            "filterStatusList",
            q.statuses.iter().map(|s| s.as_str().to_string()).collect(),
        );
        push(&mut p, "filterIsLiquidateList", q.is_liquidate);
        push(&mut p, "filterIsDeleverageList", q.is_deleverage);
        push(&mut p, "filterIsPositionTpslList", q.is_position_tpsl);
        push(
            &mut p,
            "filterStartCreatedTimeInclusive",
            q.start_created_time_inclusive,
        );
        push(
            &mut p,
            "filterEndCreatedTimeExclusive",
            q.end_created_time_exclusive,
        );
        if include_order_ids {
            push_list(&mut p, "filterOrderIdList", q.order_ids.clone());
        }
        p
    }

    /// Open orders.
    pub async fn get_active_orders(
        &self,
        q: &OrderPageQuery,
        timestamp_ms: u64,
    ) -> ApiResult<EdgexPage<EdgexOrder>> {
        let p = self.order_page_params(q, false);
        self.get(
            Self::GET_ACTIVE_ORDER_PAGE,
            &p,
            timestamp_ms,
            "getActiveOrderPage",
        )
        .await
    }

    pub async fn get_history_orders(
        &self,
        q: &OrderPageQuery,
        timestamp_ms: u64,
    ) -> ApiResult<EdgexPage<EdgexOrder>> {
        let p = self.order_page_params(q, true);
        self.get(
            Self::GET_HISTORY_ORDER_PAGE,
            &p,
            timestamp_ms,
            "getHistoryOrderPage",
        )
        .await
    }

    pub fn fill_page_params(&self, q: &FillPageQuery) -> Vec<(&'static str, String)> {
        let mut p = vec![self.account_param()];
        push(&mut p, "size", q.size);
        push(&mut p, "offsetData", q.offset_data.as_deref());
        push_list(&mut p, "filterCoinIdList", q.coin_ids.clone());
        push_list(&mut p, "filterContractIdList", q.contract_ids.clone());
        push_list(&mut p, "filterOrderIdList", q.order_ids.clone());
        push(&mut p, "filterIsLiquidateList", q.is_liquidate);
        push(&mut p, "filterIsDeleverageList", q.is_deleverage);
        push(&mut p, "filterIsPositionTpslList", q.is_position_tpsl);
        push(
            &mut p,
            "filterStartCreatedTimeInclusive",
            q.start_created_time_inclusive,
        );
        push(
            &mut p,
            "filterEndCreatedTimeExclusive",
            q.end_created_time_exclusive,
        );
        p
    }

    /// Fill history.
    pub async fn get_fills(
        &self,
        q: &FillPageQuery,
        timestamp_ms: u64,
    ) -> ApiResult<EdgexPage<EdgexOrderFill>> {
        let p = self.fill_page_params(q);
        self.get(
            Self::GET_FILL_PAGE,
            &p,
            timestamp_ms,
            "getHistoryOrderFillTransactionPage",
        )
        .await
    }

    pub async fn get_orders_by_id(
        &self,
        order_ids: &[&str],
        timestamp_ms: u64,
    ) -> ApiResult<Vec<EdgexOrder>> {
        let p = [self.account_param(), ("orderIdList", csv(order_ids))];
        self.get(Self::GET_ORDER_BY_ID, &p, timestamp_ms, "getOrderById")
            .await
    }

    pub async fn get_orders_by_client_order_id(
        &self,
        client_order_ids: &[&str],
        timestamp_ms: u64,
    ) -> ApiResult<Vec<EdgexOrder>> {
        let p = [
            self.account_param(),
            ("clientOrderIdList", csv(client_order_ids)),
        ];
        self.get(
            Self::GET_ORDER_BY_CLIENT_ORDER_ID,
            &p,
            timestamp_ms,
            "getOrderByClientOrderId",
        )
        .await
    }

    pub fn max_order_size_body(&self, contract_id: &str, price: Option<&str>) -> Value {
        let mut m = Map::new();
        m.insert("accountId".into(), json!(self.account_id.to_string()));
        m.insert("contractId".into(), json!(contract_id));
        if let Some(p) = price {
            m.insert("price".into(), json!(p));
        }
        Value::Object(m)
    }

    pub async fn get_max_order_size(
        &self,
        contract_id: &str,
        price: Option<&str>,
        timestamp_ms: u64,
    ) -> ApiResult<EdgexMaxOrderSize> {
        let body = self.max_order_size_body(contract_id, price);
        self.post(
            Self::GET_MAX_CREATE_ORDER_SIZE,
            &body,
            timestamp_ms,
            "getMaxCreateOrderSize",
        )
        .await
    }

    // ── account ──

    /// Balances, positions and per-position risk.
    pub async fn get_account_asset(&self, timestamp_ms: u64) -> ApiResult<EdgexAccountAsset> {
        let p = [self.account_param()];
        self.get(Self::GET_ACCOUNT_ASSET, &p, timestamp_ms, "getAccountAsset")
            .await
    }

    pub async fn get_account_by_id(&self, timestamp_ms: u64) -> ApiResult<EdgexAccount> {
        let p = [self.account_param()];
        self.get(Self::GET_ACCOUNT_BY_ID, &p, timestamp_ms, "getAccountById")
            .await
    }

    pub async fn get_positions_by_contract_id(
        &self,
        contract_ids: &[&str],
        timestamp_ms: u64,
    ) -> ApiResult<Vec<EdgexPosition>> {
        let p = [self.account_param(), ("contractIdList", csv(contract_ids))];
        self.get(
            Self::GET_POSITION_BY_CONTRACT_ID,
            &p,
            timestamp_ms,
            "getPositionByContractId",
        )
        .await
    }

    pub async fn get_collateral_by_coin_id(
        &self,
        coin_ids: &[&str],
        timestamp_ms: u64,
    ) -> ApiResult<Vec<EdgexCollateral>> {
        let p = [self.account_param(), ("coinIdList", csv(coin_ids))];
        self.get(
            Self::GET_COLLATERAL_BY_COIN_ID,
            &p,
            timestamp_ms,
            "getCollateralByCoinId",
        )
        .await
    }

    pub fn update_leverage_body(&self, contract_id: &str, leverage: &str) -> Value {
        json!({
            "accountId": self.account_id.to_string(),
            "contractId": contract_id,
            "leverage": leverage,
        })
    }

    /// `contract_id` `"0"` sets the account default.
    pub async fn update_leverage_setting(
        &self,
        contract_id: &str,
        leverage: &str,
        timestamp_ms: u64,
    ) -> ApiResult<()> {
        let body = self.update_leverage_body(contract_id, leverage);
        let req = self.signed_post(Self::UPDATE_LEVERAGE_SETTING, &body, timestamp_ms)?;
        let (status, text) = self.send_raw(&req).await?;
        parse_void(status, &text, "updateLeverageSetting")
    }

    /// Build `body` with `L2Signer::set_margin_mode_body`.
    pub async fn set_margin_mode(
        &self,
        body: &SetMarginModeBody,
        timestamp_ms: u64,
    ) -> ApiResult<()> {
        let req = self.signed_post(Self::SET_MARGIN_MODE, body, timestamp_ms)?;
        let (status, text) = self.send_raw(&req).await?;
        parse_void(status, &text, "setMarginMode")
    }
}
