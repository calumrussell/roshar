use super::{Http, Query, RestResult};
use crate::signer::SignedTransaction;
use crate::types::{
    BulletAccount, BulletBalance, BulletCommissionRate, BulletDelegateOf, BulletFundingPayment,
    BulletHistoricalOrder, BulletHistoryQuery, BulletOpenOrder, BulletOrderRef, BulletPage,
    BulletPosition, BulletSubmitTxRequest, BulletSubmitTxResponse, BulletUserTrade,
};

/// Account reads and transaction submission.
///
/// Reads take the main account address (base58) and need no authentication; for a delegate key
/// pass the delegator's address. Writes are signed transactions (`crate::signer`).
#[derive(Clone)]
pub struct AccountApi {
    http: Http,
}

fn history(q: Query, h: &BulletHistoryQuery<'_>) -> Query {
    q.opt("address", h.address)
        .opt("symbol", h.symbol)
        .opt("startTime", h.start_time)
        .opt("endTime", h.end_time)
        .opt("limit", h.limit)
        .opt("cursor", h.cursor)
}

impl AccountApi {
    pub(crate) fn new(http: Http) -> Self {
        Self { http }
    }

    /// `GET /fapi/v3/account?address=..`.
    pub async fn get_account(&self, address: &str) -> RestResult<BulletAccount> {
        let url = self
            .http
            .query("/fapi/v3/account")
            .param("address", address)
            .url;
        self.http.get(&url, "account").await
    }

    /// `GET /fapi/v3/balance?address=..`.
    pub async fn get_balance(&self, address: &str) -> RestResult<Vec<BulletBalance>> {
        let url = self
            .http
            .query("/fapi/v3/balance")
            .param("address", address)
            .url;
        self.http.get(&url, "balance").await
    }

    /// `GET /fapi/v2/positionRisk?address=..[&symbol=..]`.
    pub async fn get_positions(
        &self,
        address: &str,
        symbol: Option<&str>,
    ) -> RestResult<Vec<BulletPosition>> {
        let url = self
            .http
            .query("/fapi/v2/positionRisk")
            .param("address", address)
            .opt("symbol", symbol)
            .url;
        self.http.get(&url, "positionRisk").await
    }

    /// `GET /fapi/v1/openOrders?address=..[&symbol=..]` (regular and TP/SL orders).
    pub async fn get_open_orders(
        &self,
        address: &str,
        symbol: Option<&str>,
    ) -> RestResult<Vec<BulletOpenOrder>> {
        let url = self
            .http
            .query("/fapi/v1/openOrders")
            .param("address", address)
            .opt("symbol", symbol)
            .url;
        self.http.get(&url, "openOrders").await
    }

    /// `GET /fapi/v1/openOrder?address=..&symbol=..&{orderId|clientOrderId|triggerOrderId}=..`.
    pub async fn get_open_order(
        &self,
        address: &str,
        symbol: &str,
        order: BulletOrderRef,
    ) -> RestResult<BulletOpenOrder> {
        let q = self
            .http
            .query("/fapi/v1/openOrder")
            .param("address", address)
            .param("symbol", symbol);
        let url = match order {
            BulletOrderRef::OrderId(id) => q.param("orderId", id),
            BulletOrderRef::ClientOrderId(id) => q.param("clientOrderId", id),
            BulletOrderRef::TriggerOrderId(id) => q.param("triggerOrderId", id),
        }
        .url;
        self.http.get(&url, "openOrder").await
    }

    /// `GET /fapi/v1/allOrders`: order history. `order_id` returns orders with id >= it.
    pub async fn get_all_orders(
        &self,
        query: &BulletHistoryQuery<'_>,
        order_id: Option<u64>,
    ) -> RestResult<BulletPage<BulletHistoricalOrder>> {
        let url = history(self.http.query("/fapi/v1/allOrders"), query)
            .opt("orderId", order_id)
            .url;
        self.http.get(&url, "allOrders").await
    }

    /// `GET /fapi/v1/userTrades`: fills. `is_degen` restricts to Degen or non-Degen fills.
    pub async fn get_user_trades(
        &self,
        query: &BulletHistoryQuery<'_>,
        is_degen: Option<bool>,
    ) -> RestResult<BulletPage<BulletUserTrade>> {
        let url = history(self.http.query("/fapi/v1/userTrades"), query)
            .opt("isDegen", is_degen)
            .url;
        self.http.get(&url, "userTrades").await
    }

    /// `GET /fapi/v1/userFundingPayments`.
    pub async fn get_funding_payments(
        &self,
        query: &BulletHistoryQuery<'_>,
    ) -> RestResult<BulletPage<BulletFundingPayment>> {
        let url = history(self.http.query("/fapi/v1/userFundingPayments"), query).url;
        self.http.get(&url, "userFundingPayments").await
    }

    /// `GET /fapi/v1/commissionRate?address=..&symbol=..`.
    pub async fn get_commission_rate(
        &self,
        address: &str,
        symbol: &str,
    ) -> RestResult<BulletCommissionRate> {
        let url = self
            .http
            .query("/fapi/v1/commissionRate")
            .param("address", address)
            .param("symbol", symbol)
            .url;
        self.http.get(&url, "commissionRate").await
    }

    /// `GET /api/v1/delegateOf?address=..`: the account a delegate key trades for (400 if the
    /// address is not a delegate).
    pub async fn get_delegate_of(&self, delegate_address: &str) -> RestResult<BulletDelegateOf> {
        let url = self
            .http
            .query("/api/v1/delegateOf")
            .param("address", delegate_address)
            .url;
        self.http.get(&url, "delegateOf").await
    }

    /// `POST /tx/submit` with `{"body": base64(tx)}`. A stale chain hash fails signature
    /// verification (401); a sequencer rate limit is a 503.
    pub async fn submit_tx(&self, tx: &SignedTransaction) -> RestResult<BulletSubmitTxResponse> {
        let body = serde_json::to_string(&BulletSubmitTxRequest {
            body: tx.to_base64(),
        })
        .expect("serializing a string field cannot fail");
        self.http.post_json("/tx/submit", body, "tx/submit").await
    }
}
