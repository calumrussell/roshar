use super::{parse, BoxError, Query};
use crate::types::{
    CancelLimitOrderResponse, CancelTpslResponse, CollateralLimits, CreateLimitOrderRequest,
    CreateLimitOrderResponse, CreateTpslRequest, CreateTpslResponse, DataList,
    DecreasePositionRequest, DecreasePositionResponse, ExecuteRequest, ExecuteResponse,
    IncreasePositionRequest, IncreasePositionResponse, LimitOrder, Position, SerializedTxBatch,
    Side, Trade, TradeAction, TradesQuery, UpdateLimitOrderRequest, UpdateLimitOrderResponse,
    UpdateTpslRequest, UpdateTpslResponse,
};
use reqwest::Method;
use roshar_http::RateLimitedClient;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::json;

/// Wallet reads, transaction building and submission.
///
/// The Perps API has no API keys or sessions: reads are keyed by wallet address, and every
/// write returns an unsigned Solana transaction (`serializedTxBase64`) for the wallet to sign
/// with [`crate::signer::Keypair`] and submit with [`TradingApi::execute`].
pub struct TradingApi {
    client: RateLimitedClient,
    base_url: String,
}

impl TradingApi {
    pub fn new(client: RateLimitedClient, base_url: &str) -> Self {
        Self {
            client,
            base_url: base_url.to_string(),
        }
    }

    async fn get<T: DeserializeOwned>(&self, url: &str, what: &str) -> Result<T, BoxError> {
        parse(self.client.get(url).await?, what).await
    }

    async fn send<B: Serialize + ?Sized, T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        body: &B,
    ) -> Result<T, BoxError> {
        let url = format!("{}/{path}", self.base_url);
        let response = self
            .client
            .request(method, &url)
            .await
            .json(body)
            .send()
            .await?;
        parse(response, path).await
    }

    // -----------------------------------------------------------------------------------------
    // Reads
    // -----------------------------------------------------------------------------------------

    /// `GET /positions?walletAddress=[&includeClosedPositions=]`.
    pub async fn positions(
        &self,
        wallet_address: &str,
        include_closed_positions: Option<bool>,
    ) -> Result<DataList<Position>, BoxError> {
        let url = Query::new(&self.base_url, "positions")
            .param("walletAddress", wallet_address)
            .opt("includeClosedPositions", include_closed_positions)
            .url;
        self.get(&url, "positions").await
    }

    /// `GET /orders/limit?walletAddress=`: open limit orders. (The OpenAPI document lists the
    /// address as a body field; the API and the official CLI use the query string.)
    pub async fn limit_orders(
        &self,
        wallet_address: &str,
    ) -> Result<DataList<LimitOrder>, BoxError> {
        let url = Query::new(&self.base_url, "orders/limit")
            .param("walletAddress", wallet_address)
            .url;
        self.get(&url, "orders/limit").await
    }

    /// `GET /orders/expired?walletAddress=`: unsigned transactions closing expired limit orders.
    pub async fn expired_orders(
        &self,
        wallet_address: &str,
    ) -> Result<SerializedTxBatch, BoxError> {
        let url = Query::new(&self.base_url, "orders/expired")
            .param("walletAddress", wallet_address)
            .url;
        self.get(&url, "orders/expired").await
    }

    /// `GET /trades`: trade history (fills, trigger executions, liquidations).
    pub async fn trades(&self, query: &TradesQuery<'_>) -> Result<DataList<Trade>, BoxError> {
        let url = Query::new(&self.base_url, "trades")
            .opt("walletAddress", query.wallet_address)
            .opt("txid", query.txid)
            .opt("action", query.action.map(trade_action_str))
            .opt("mint", query.mint)
            .opt("side", query.side.map(side_str))
            .opt("createdAtAfter", query.created_at_after)
            .opt("createdAtBefore", query.created_at_before)
            .opt("start", query.start)
            .opt("end", query.end)
            .url;
        self.get(&url, "trades").await
    }

    /// `GET /positions/collateral-limits?inputMint=&positionPubkey=`.
    pub async fn collateral_limits(
        &self,
        input_mint: &str,
        position_pubkey: &str,
    ) -> Result<CollateralLimits, BoxError> {
        let url = Query::new(&self.base_url, "positions/collateral-limits")
            .param("inputMint", input_mint)
            .param("positionPubkey", position_pubkey)
            .url;
        self.get(&url, "positions/collateral-limits").await
    }

    // -----------------------------------------------------------------------------------------
    // Transaction building
    // -----------------------------------------------------------------------------------------

    /// `POST /positions/increase`: market open/increase or collateral deposit.
    /// Submit with [`crate::types::ExecuteAction::IncreasePosition`].
    pub async fn increase_position(
        &self,
        request: &IncreasePositionRequest,
    ) -> Result<IncreasePositionResponse, BoxError> {
        self.send(Method::POST, "positions/increase", request).await
    }

    /// `POST /positions/decrease`: market reduce/close or collateral withdrawal.
    /// Submit with [`crate::types::ExecuteAction::DecreasePosition`].
    pub async fn decrease_position(
        &self,
        request: &DecreasePositionRequest,
    ) -> Result<DecreasePositionResponse, BoxError> {
        self.send(Method::POST, "positions/decrease", request).await
    }

    /// `POST /positions/close-all`: one unsigned close transaction per open position.
    pub async fn close_all(&self, wallet_address: &str) -> Result<SerializedTxBatch, BoxError> {
        let body = json!({ "walletAddress": wallet_address });
        self.send(Method::POST, "positions/close-all", &body).await
    }

    /// `POST /orders/limit`. Submit with [`crate::types::ExecuteAction::CreateLimitOrder`].
    pub async fn create_limit_order(
        &self,
        request: &CreateLimitOrderRequest,
    ) -> Result<CreateLimitOrderResponse, BoxError> {
        self.send(Method::POST, "orders/limit", request).await
    }

    /// `PATCH /orders/limit`: change the trigger price.
    /// Submit with [`crate::types::ExecuteAction::UpdateLimitOrder`].
    pub async fn update_limit_order(
        &self,
        request: &UpdateLimitOrderRequest,
    ) -> Result<UpdateLimitOrderResponse, BoxError> {
        self.send(Method::PATCH, "orders/limit", request).await
    }

    /// `DELETE /orders/limit`. Submit with [`crate::types::ExecuteAction::CancelLimitOrder`].
    pub async fn cancel_limit_order(
        &self,
        position_request_pubkey: &str,
    ) -> Result<CancelLimitOrderResponse, BoxError> {
        let body = json!({ "positionRequestPubkey": position_request_pubkey });
        self.send(Method::DELETE, "orders/limit", &body).await
    }

    /// `POST /tpsl`. Submit with [`crate::types::ExecuteAction::CreateTpsl`].
    pub async fn create_tpsl(
        &self,
        request: &CreateTpslRequest,
    ) -> Result<CreateTpslResponse, BoxError> {
        self.send(Method::POST, "tpsl", request).await
    }

    /// `PATCH /tpsl`. Submit with [`crate::types::ExecuteAction::UpdateTpsl`].
    pub async fn update_tpsl(
        &self,
        request: &UpdateTpslRequest,
    ) -> Result<UpdateTpslResponse, BoxError> {
        self.send(Method::PATCH, "tpsl", request).await
    }

    /// `DELETE /tpsl`. Submit with [`crate::types::ExecuteAction::CancelTpsl`].
    pub async fn cancel_tpsl(
        &self,
        position_request_pubkey: &str,
    ) -> Result<CancelTpslResponse, BoxError> {
        let body = json!({ "positionRequestPubkey": position_request_pubkey });
        self.send(Method::DELETE, "tpsl", &body).await
    }

    // -----------------------------------------------------------------------------------------
    // Submission
    // -----------------------------------------------------------------------------------------

    /// `POST /transaction/execute`: submit a signed transaction. The API adds the keeper's
    /// signature where `requireKeeperSignature` is true, and simulates first: a failing
    /// simulation is a 422 `simulation_error`.
    pub async fn execute(&self, request: &ExecuteRequest) -> Result<ExecuteResponse, BoxError> {
        self.send(Method::POST, "transaction/execute", request)
            .await
    }
}

fn trade_action_str(action: TradeAction) -> &'static str {
    match action {
        TradeAction::Increase => "Increase",
        TradeAction::Decrease => "Decrease",
    }
}

fn side_str(side: Side) -> &'static str {
    match side {
        Side::Long => "long",
        Side::Short => "short",
    }
}
