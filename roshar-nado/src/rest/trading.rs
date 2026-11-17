use super::{ApiResult, Transport};
use crate::signer::{ExecuteKind, SignedExecute};
use crate::types::{
    NadoArchiveOrders, NadoArchiveOrdersQuery, NadoCancelledOrders, NadoExecuteReply, NadoFeeRates,
    NadoInterestAndFunding, NadoInterestAndFundingQuery, NadoLinkedSigner, NadoMatches,
    NadoMatchesQuery, NadoMaxOrderSize, NadoMultiProductOrders, NadoNonces, NadoOrder,
    NadoPlaceOrdersItem, NadoPlacedOrder, NadoPlacedOrders, NadoSubaccountInfo,
    NadoSubaccountOrders,
};
use crate::NadoNetwork;
use roshar_http::RateLimitedClient;
use serde::de::DeserializeOwned;
use serde_json::{json, Value};

/// Executes (signed with [`crate::signer::Signer`]) and the unsigned account reads a trader
/// needs: nonces, balances / positions, open orders, fee rates, fills and funding payments.
///
/// Holds no key: account queries are public on Nado and take the subaccount as a parameter.
pub struct TradingApi {
    gateway: Transport,
    archive: Transport,
    executes: Transport,
}

/// `max_order_size` parameters.
#[derive(Debug, Clone)]
pub struct NadoMaxOrderSizeQuery<'a> {
    pub sender: &'a str,
    pub product_id: u32,
    pub price_x18: i128,
    /// `true` for the max bid (`long`), `false` for the max ask (`short`).
    pub long: bool,
    pub spot_leverage: Option<bool>,
    pub reduce_only: Option<bool>,
    pub isolated: Option<bool>,
    pub avg_price_x18: Option<i128>,
}

impl TradingApi {
    /// `query_requests_per_second` covers gateway queries and archive queries (2400 weight / min
    /// per IP each). `execute_requests_per_second` covers executes (600 weight / min per wallet;
    /// a leveraged order or a single-digest cancel weighs 1).
    pub fn new(
        network: NadoNetwork,
        query_requests_per_second: u32,
        execute_requests_per_second: u32,
    ) -> Self {
        Self {
            gateway: Transport::new(
                RateLimitedClient::new(query_requests_per_second, 1),
                network,
            ),
            archive: Transport::new(
                RateLimitedClient::new(query_requests_per_second, 1),
                network,
            ),
            executes: Transport::new(
                RateLimitedClient::new(execute_requests_per_second, 1),
                network,
            ),
        }
    }

    pub fn network(&self) -> NadoNetwork {
        self.gateway.network()
    }

    // ── Executes (`POST /gateway/v1/execute`) ───────────────────────────────────────────────

    async fn execute_checked<T: DeserializeOwned>(
        &self,
        signed: &SignedExecute,
        expected: ExecuteKind,
        what: &str,
    ) -> ApiResult<NadoExecuteReply<T>> {
        if signed.kind != expected {
            return Err(format!("{what} given a {:?} execute", signed.kind).into());
        }
        self.executes.execute(&signed.payload, what).await
    }

    fn require_data<T>(reply: NadoExecuteReply<T>, what: &str) -> ApiResult<T> {
        reply
            .data
            .ok_or_else(|| format!("Nado {what} reply has no data").into())
    }

    /// `place_order`; returns the order digest.
    pub async fn place_order(&self, signed: &SignedExecute) -> ApiResult<NadoPlacedOrder> {
        let reply = self
            .execute_checked(signed, ExecuteKind::PlaceOrder, "place_order")
            .await?;
        Self::require_data(reply, "place_order")
    }

    /// `place_orders`; one result (digest or error) per order, in request order.
    pub async fn place_orders(
        &self,
        signed: &SignedExecute,
    ) -> ApiResult<Vec<NadoPlaceOrdersItem>> {
        let reply = self
            .execute_checked::<NadoPlacedOrders>(signed, ExecuteKind::PlaceOrders, "place_orders")
            .await?;
        Ok(Self::require_data(reply, "place_orders")?.place_orders)
    }

    /// `cancel_orders`; returns the cancelled orders.
    pub async fn cancel_orders(&self, signed: &SignedExecute) -> ApiResult<Vec<NadoOrder>> {
        let reply = self
            .execute_checked::<NadoCancelledOrders>(
                signed,
                ExecuteKind::CancelOrders,
                "cancel_orders",
            )
            .await?;
        Ok(Self::require_data(reply, "cancel_orders")?.cancelled_orders)
    }

    /// `cancel_product_orders` (cancel-all); returns the cancelled orders.
    pub async fn cancel_product_orders(&self, signed: &SignedExecute) -> ApiResult<Vec<NadoOrder>> {
        let reply = self
            .execute_checked::<NadoCancelledOrders>(
                signed,
                ExecuteKind::CancelProductOrders,
                "cancel_product_orders",
            )
            .await?;
        Ok(Self::require_data(reply, "cancel_product_orders")?.cancelled_orders)
    }

    /// `cancel_and_place`; returns the new order's digest.
    pub async fn cancel_and_place(&self, signed: &SignedExecute) -> ApiResult<NadoPlacedOrder> {
        let reply = self
            .execute_checked(signed, ExecuteKind::CancelAndPlace, "cancel_and_place")
            .await?;
        Self::require_data(reply, "cancel_and_place")
    }

    /// `link_signer`. The reply carries no data.
    pub async fn link_signer(&self, signed: &SignedExecute) -> ApiResult<()> {
        self.execute_checked::<Value>(signed, ExecuteKind::LinkSigner, "link_signer")
            .await?;
        Ok(())
    }

    // ── Account queries (`POST /gateway/v1/query`) ──────────────────────────────────────────

    /// `nonces` for a wallet address (20 bytes hex).
    pub async fn nonces(&self, address: &str) -> ApiResult<NadoNonces> {
        self.gateway
            .query(&json!({"type": "nonces", "address": address}), "nonces")
            .await
    }

    /// `subaccount_info`: balances, positions and health of a bytes32 subaccount.
    pub async fn subaccount_info(&self, subaccount: &str) -> ApiResult<NadoSubaccountInfo> {
        self.gateway
            .query(
                &json!({"type": "subaccount_info", "subaccount": subaccount}),
                "subaccount_info",
            )
            .await
    }

    /// `order`: one open order by digest.
    pub async fn order(&self, product_id: u32, digest: &str) -> ApiResult<NadoOrder> {
        self.gateway
            .query(
                &json!({"type": "order", "product_id": product_id, "digest": digest}),
                "order",
            )
            .await
    }

    /// `subaccount_orders`: open orders on one product.
    pub async fn subaccount_orders(
        &self,
        sender: &str,
        product_id: u32,
    ) -> ApiResult<NadoSubaccountOrders> {
        self.gateway
            .query(
                &json!({"type": "subaccount_orders", "sender": sender, "product_id": product_id}),
                "subaccount_orders",
            )
            .await
    }

    /// `orders`: open orders on several products.
    pub async fn orders(
        &self,
        sender: &str,
        product_ids: &[u32],
    ) -> ApiResult<NadoMultiProductOrders> {
        self.gateway
            .query(
                &json!({"type": "orders", "sender": sender, "product_ids": product_ids}),
                "orders",
            )
            .await
    }

    /// `fee_rates` of a subaccount.
    pub async fn fee_rates(&self, sender: &str) -> ApiResult<NadoFeeRates> {
        self.gateway
            .query(&json!({"type": "fee_rates", "sender": sender}), "fee_rates")
            .await
    }

    /// `linked_signer` of a subaccount.
    pub async fn linked_signer(&self, subaccount: &str) -> ApiResult<NadoLinkedSigner> {
        self.gateway
            .query(
                &json!({"type": "linked_signer", "subaccount": subaccount}),
                "linked_signer",
            )
            .await
    }

    /// `max_order_size`. Booleans and prices are sent as strings, as documented.
    pub async fn max_order_size(
        &self,
        q: &NadoMaxOrderSizeQuery<'_>,
    ) -> ApiResult<NadoMaxOrderSize> {
        let mut body = json!({
            "type": "max_order_size",
            "sender": q.sender,
            "product_id": q.product_id,
            "price_x18": q.price_x18.to_string(),
            "direction": if q.long { "long" } else { "short" },
        });
        if let Some(b) = q.spot_leverage {
            body["spot_leverage"] = json!(b.to_string());
        }
        if let Some(b) = q.reduce_only {
            body["reduce_only"] = json!(b.to_string());
        }
        if let Some(b) = q.isolated {
            body["isolated"] = json!(b.to_string());
        }
        if let Some(p) = q.avg_price_x18 {
            body["avg_price_x18"] = json!(p.to_string());
        }
        self.gateway.query(&body, "max_order_size").await
    }

    // ── Archive (`POST /archive/v1`) ────────────────────────────────────────────────────────

    /// `matches`: fills with fees and balance changes, newest first.
    pub async fn matches(&self, query: &NadoMatchesQuery) -> ApiResult<NadoMatches> {
        self.archive
            .archive(&json!({"matches": query}), "matches")
            .await
    }

    /// `orders`: matched historical orders, newest first.
    pub async fn archive_orders(
        &self,
        query: &NadoArchiveOrdersQuery,
    ) -> ApiResult<NadoArchiveOrders> {
        self.archive
            .archive(&json!({"orders": query}), "orders")
            .await
    }

    /// `interest_and_funding`: interest (spot) and funding (perp) payments.
    pub async fn interest_and_funding(
        &self,
        query: &NadoInterestAndFundingQuery,
    ) -> ApiResult<NadoInterestAndFunding> {
        self.archive
            .archive(
                &json!({"interest_and_funding": query}),
                "interest_and_funding",
            )
            .await
    }
}
