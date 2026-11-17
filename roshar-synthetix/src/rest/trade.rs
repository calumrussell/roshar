use super::{read_envelope, ApiResult, SYNTHETIX_TRADE_URL};
use crate::signer::SignedRequest;
use crate::types::trading::{
    CancelAllResult, DelegatedSigner, ModifyOrderResult, OrderStatuses, RemovedAllDelegatedSigners,
    RemovedDelegatedSigner, ScheduleCancelResult, SubAccountQuery, UpdateLeverageResult,
};
use serde::de::DeserializeOwned;

/// Sends [`SignedRequest`]s to `POST /v1/trade`. Not rate-limited: trade actions draw on a
/// token-weighted per-subaccount budget (fee-tier dependent) and the shared per-IP budget, so
/// pacing is the caller's.
pub struct TradeApi {
    url: String,
    user_agent: String,
}

impl TradeApi {
    /// `url`: [`SYNTHETIX_TRADE_URL`](super::SYNTHETIX_TRADE_URL). `user_agent` is required by
    /// the exchange (HTTP 403 without one).
    pub fn new(url: &str, user_agent: &str) -> Self {
        Self {
            url: url.to_string(),
            user_agent: user_agent.to_string(),
        }
    }

    /// Mainnet [`SYNTHETIX_TRADE_URL`].
    pub fn mainnet(user_agent: &str) -> Self {
        Self::new(SYNTHETIX_TRADE_URL, user_agent)
    }

    /// Posts `req` and parses the envelope's `response` as `T`.
    pub async fn send<T: DeserializeOwned>(&self, req: &SignedRequest) -> ApiResult<T> {
        let response = roshar_http::get_http_client()
            .post(&self.url)
            .header("User-Agent", &self.user_agent)
            .header("Content-Type", "application/json")
            .body(req.rest_body())
            .send()
            .await?;
        read_envelope(response, req.action()).await
    }

    async fn send_checked<T: DeserializeOwned>(
        &self,
        req: &SignedRequest,
        action: &str,
    ) -> ApiResult<T> {
        if req.action() != action {
            return Err(format!("expected a signed {action} request, got {}", req.action()).into());
        }
        self.send(req).await
    }

    /// `placeOrders` (from [`Signer::sign_place_orders`](crate::signer::Signer::sign_place_orders)).
    /// Per-order rejections are `OrderStatus::Error` items, not an `Err`.
    pub async fn place_orders(&self, req: &SignedRequest) -> ApiResult<OrderStatuses> {
        self.send_checked(req, "placeOrders").await
    }

    /// `cancelOrders` by venue id or client order id.
    pub async fn cancel_orders(&self, req: &SignedRequest) -> ApiResult<OrderStatuses> {
        self.send_checked(req, "cancelOrders").await
    }

    /// `cancelAllOrders`.
    pub async fn cancel_all_orders(&self, req: &SignedRequest) -> ApiResult<Vec<CancelAllResult>> {
        self.send_checked(req, "cancelAllOrders").await
    }

    /// `modifyOrder` by venue id or client order id.
    pub async fn modify_order(&self, req: &SignedRequest) -> ApiResult<ModifyOrderResult> {
        self.send_checked(req, "modifyOrder").await
    }

    /// `updateLeverage`.
    pub async fn update_leverage(&self, req: &SignedRequest) -> ApiResult<UpdateLeverageResult> {
        self.send_checked(req, "updateLeverage").await
    }

    /// `scheduleCancel` (dead-man switch).
    pub async fn schedule_cancel(&self, req: &SignedRequest) -> ApiResult<ScheduleCancelResult> {
        self.send_checked(req, "scheduleCancel").await
    }

    /// A read-only query signed with [`Signer::sign_query::<Q>`](crate::signer::Signer::sign_query).
    pub async fn query<Q: SubAccountQuery>(&self, req: &SignedRequest) -> ApiResult<Q::Response> {
        self.send_checked(req, Q::ACTION).await
    }

    /// `addDelegatedSigner`.
    pub async fn add_delegated_signer(&self, req: &SignedRequest) -> ApiResult<DelegatedSigner> {
        self.send_checked(req, "addDelegatedSigner").await
    }

    /// `removeDelegatedSigner`.
    pub async fn remove_delegated_signer(
        &self,
        req: &SignedRequest,
    ) -> ApiResult<RemovedDelegatedSigner> {
        self.send_checked(req, "removeDelegatedSigner").await
    }

    /// `removeAllDelegatedSigners`.
    pub async fn remove_all_delegated_signers(
        &self,
        req: &SignedRequest,
    ) -> ApiResult<RemovedAllDelegatedSigners> {
        self.send_checked(req, "removeAllDelegatedSigners").await
    }
}
