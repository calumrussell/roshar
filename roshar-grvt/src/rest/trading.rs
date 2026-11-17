//! Auth (`https://edge.grvt.io/auth/*`) and trading REST (`https://trades.grvt.io/full/v1/*`).
//!
//! Not rate-limited here: GRVT limits per main account and per endpoint class (create per
//! instrument, cancel, reads; see the README), so pacing is the caller's choice.

use super::{parse_reply, ApiResult, GrvtApiError};
use crate::types::{
    GrvtAck, GrvtBulkOrdersRequest, GrvtBulkOrdersResponse, GrvtCancelAllOrdersRequest,
    GrvtCancelOnDisconnectRequest, GrvtCancelOrderRequest, GrvtCreateOrderRequest, GrvtError,
    GrvtFill, GrvtFilteredRequest, GrvtFundingPayment, GrvtFundingPaymentHistoryRequest,
    GrvtGetOrderRequest, GrvtHistoryRequest, GrvtLoginFailure, GrvtLoginResponse, GrvtOrder,
    GrvtOrderRequest, GrvtPage, GrvtPosition, GrvtResult, GrvtSession, GrvtSubAccount,
    GrvtSubAccountRequest, GrvtWalletLoginRequest,
};
use serde::de::DeserializeOwned;
use serde::Serialize;

#[derive(Serialize)]
struct ApiKeyLogin<'a> {
    api_key: &'a str,
}

pub struct TradingApi {
    edge_url: String,
    trades_url: String,
}

impl TradingApi {
    /// `edge_url`: [`crate::GRVT_EDGE_URL`] / [`crate::GRVT_TESTNET_EDGE_URL`];
    /// `trades_url`: [`crate::GRVT_TRADES_URL`] / [`crate::GRVT_TESTNET_TRADES_URL`].
    pub fn new(edge_url: &str, trades_url: &str) -> Self {
        Self {
            edge_url: edge_url.trim_end_matches('/').to_string(),
            trades_url: trades_url.trim_end_matches('/').to_string(),
        }
    }

    async fn login(&self, path: &str, body: String, what: &str) -> ApiResult<GrvtSession> {
        let url = format!("{}/auth/{path}", self.edge_url);
        let response = roshar_http::get_http_client()
            .post(&url)
            .header("Content-Type", "application/json")
            .header("Cookie", "rm=true;")
            .body(body)
            .send()
            .await?;
        let status = response.status().as_u16();
        let mut cookie = None;
        let mut cookie_expires = None;
        for value in response.headers().get_all("set-cookie") {
            let Ok(value) = value.to_str() else { continue };
            let mut parts = value.split(';').map(str::trim);
            let Some(first) = parts.next() else { continue };
            if !first.starts_with("gravity=") {
                continue;
            }
            cookie = Some(first.to_string());
            cookie_expires = parts
                .find(|p| {
                    p.get(..8)
                        .is_some_and(|k| k.eq_ignore_ascii_case("expires="))
                })
                .map(|p| p[8..].to_string());
        }
        let account_id = response
            .headers()
            .get("x-grvt-account-id")
            .and_then(|v| v.to_str().ok())
            .map(str::to_string);
        let text = response.text().await?;
        if !(200..300).contains(&status) {
            let message = serde_json::from_str::<GrvtLoginFailure>(&text)
                .map(|f| f.error)
                .unwrap_or_else(|_| text.clone());
            return Err(Box::new(GrvtApiError {
                what: what.to_string(),
                http_status: status,
                body: serde_json::from_str::<GrvtError>(&text).ok(),
                raw: message,
            }));
        }
        let body: GrvtLoginResponse = serde_json::from_str(&text)
            .map_err(|e| format!("Failed to parse GRVT {what}: {e}: {text}"))?;
        Ok(GrvtSession {
            cookie: cookie.ok_or_else(|| format!("GRVT {what}: no gravity cookie in reply"))?,
            cookie_expires: cookie_expires
                .ok_or_else(|| format!("GRVT {what}: gravity cookie has no Expires"))?,
            account_id: account_id
                .ok_or_else(|| format!("GRVT {what}: no X-Grvt-Account-Id header in reply"))?,
            body,
        })
    }

    /// `POST /auth/api_key/login` with an API key provisioned in the GRVT UI.
    pub async fn login_api_key(&self, api_key: &str) -> ApiResult<GrvtSession> {
        let body = serde_json::to_string(&ApiKeyLogin { api_key })?;
        self.login("api_key/login", body, "api key login").await
    }

    /// `POST /auth/wallet/login` with a `Signer::sign_wallet_login` body.
    pub async fn login_wallet(&self, request: &GrvtWalletLoginRequest) -> ApiResult<GrvtSession> {
        let body = serde_json::to_string(request)?;
        self.login("wallet/login", body, "wallet login").await
    }

    async fn post<B: Serialize, T: DeserializeOwned>(
        &self,
        session: &GrvtSession,
        path: &str,
        body: &B,
        what: &str,
    ) -> ApiResult<T> {
        let url = format!("{}/full/{path}", self.trades_url);
        let response = roshar_http::get_http_client()
            .post(&url)
            .header("Content-Type", "application/json")
            .header("Cookie", session.cookie.as_str())
            .header("X-Grvt-Account-Id", session.account_id.as_str())
            .body(serde_json::to_string(body)?)
            .send()
            .await?;
        let status = response.status().as_u16();
        let text = response.text().await?;
        parse_reply(what, status, &text)
    }

    /// `full/v1/create_order`. Returns the order as accepted (state usually `PENDING`).
    pub async fn create_order(
        &self,
        session: &GrvtSession,
        order: &GrvtOrderRequest,
    ) -> ApiResult<GrvtOrder> {
        let body = GrvtCreateOrderRequest {
            order: order.clone(),
        };
        let r: GrvtResult<GrvtOrder> = self
            .post(session, "v1/create_order", &body, "create order")
            .await?;
        Ok(r.result)
    }

    /// `full/v1/cancel_order` (by `order_id` or `client_order_id`).
    pub async fn cancel_order(
        &self,
        session: &GrvtSession,
        request: &GrvtCancelOrderRequest,
    ) -> ApiResult<GrvtAck> {
        let r: GrvtResult<GrvtAck> = self
            .post(session, "v1/cancel_order", request, "cancel order")
            .await?;
        Ok(r.result)
    }

    /// `full/v1/cancel_all_orders`.
    pub async fn cancel_all_orders(
        &self,
        session: &GrvtSession,
        request: &GrvtCancelAllOrdersRequest,
    ) -> ApiResult<GrvtAck> {
        let r: GrvtResult<GrvtAck> = self
            .post(
                session,
                "v1/cancel_all_orders",
                request,
                "cancel all orders",
            )
            .await?;
        Ok(r.result)
    }

    /// `full/v2/bulk_orders`: bulk create, bulk cancel or cancel-replace (GRVT has no amend).
    pub async fn bulk_orders(
        &self,
        session: &GrvtSession,
        request: &GrvtBulkOrdersRequest,
    ) -> ApiResult<GrvtBulkOrdersResponse> {
        self.post(session, "v2/bulk_orders", request, "bulk orders")
            .await
    }

    /// `full/v1/cancel_on_disconnect`: arms (or with 0 disarms) the dead-man's switch.
    pub async fn cancel_on_disconnect(
        &self,
        session: &GrvtSession,
        request: &GrvtCancelOnDisconnectRequest,
    ) -> ApiResult<GrvtAck> {
        let r: GrvtResult<GrvtAck> = self
            .post(
                session,
                "v1/cancel_on_disconnect",
                request,
                "cancel on disconnect",
            )
            .await?;
        Ok(r.result)
    }

    /// `full/v1/order`.
    pub async fn get_order(
        &self,
        session: &GrvtSession,
        request: &GrvtGetOrderRequest,
    ) -> ApiResult<GrvtOrder> {
        let r: GrvtResult<GrvtOrder> = self.post(session, "v1/order", request, "get order").await?;
        Ok(r.result)
    }

    /// `full/v1/open_orders`.
    pub async fn open_orders(
        &self,
        session: &GrvtSession,
        request: &GrvtFilteredRequest,
    ) -> ApiResult<Vec<GrvtOrder>> {
        let r: GrvtResult<Vec<GrvtOrder>> = self
            .post(session, "v1/open_orders", request, "open orders")
            .await?;
        Ok(r.result)
    }

    /// `full/v1/order_history`.
    pub async fn order_history(
        &self,
        session: &GrvtSession,
        request: &GrvtHistoryRequest,
    ) -> ApiResult<GrvtPage<GrvtOrder>> {
        self.post(session, "v1/order_history", request, "order history")
            .await
    }

    /// `full/v1/fill_history`.
    pub async fn fill_history(
        &self,
        session: &GrvtSession,
        request: &GrvtHistoryRequest,
    ) -> ApiResult<GrvtPage<GrvtFill>> {
        self.post(session, "v1/fill_history", request, "fill history")
            .await
    }

    /// `full/v1/funding_payment_history`.
    pub async fn funding_payment_history(
        &self,
        session: &GrvtSession,
        request: &GrvtFundingPaymentHistoryRequest,
    ) -> ApiResult<GrvtPage<GrvtFundingPayment>> {
        self.post(
            session,
            "v1/funding_payment_history",
            request,
            "funding payment history",
        )
        .await
    }

    /// `full/v1/positions`.
    pub async fn positions(
        &self,
        session: &GrvtSession,
        request: &GrvtFilteredRequest,
    ) -> ApiResult<Vec<GrvtPosition>> {
        let r: GrvtResult<Vec<GrvtPosition>> = self
            .post(session, "v1/positions", request, "positions")
            .await?;
        Ok(r.result)
    }

    /// `full/v1/account_summary`: balances, margin and positions of one sub account.
    pub async fn account_summary(
        &self,
        session: &GrvtSession,
        sub_account_id: u64,
    ) -> ApiResult<GrvtSubAccount> {
        let body = GrvtSubAccountRequest {
            sub_account_id: sub_account_id.to_string(),
        };
        let r: GrvtResult<GrvtSubAccount> = self
            .post(session, "v1/account_summary", &body, "account summary")
            .await?;
        Ok(r.result)
    }
}
