use super::{parse_data, ApiResult};
use crate::signer::SignedAddOrderlyKey;
use crate::types::{OrderlyAccountLookup, OrderlyAddKeyResult, OrderlyKeyLookup};
use crate::wire::PathQuery;
use roshar_http::RateLimitedClient;

/// Unsigned account endpoints: look up an account id and its Orderly keys, and register a new
/// Orderly key with a wallet signature.
pub struct AccountApi {
    client: RateLimitedClient,
    base_url: String,
}

impl AccountApi {
    pub fn new(client: RateLimitedClient, base_url: &str) -> Self {
        Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
        }
    }

    /// `GET /v1/get_account?address=..&broker_id=..[&chain_type=EVM|SOL]` — the account id of a
    /// wallet under a builder. Unregistered wallets return error -1607.
    pub async fn get_account(
        &self,
        address: &str,
        broker_id: &str,
        chain_type: Option<&str>,
    ) -> ApiResult<OrderlyAccountLookup> {
        let path = PathQuery::new("/v1/get_account")
            .param("address", address)?
            .param("broker_id", broker_id)?
            .opt("chain_type", chain_type)?
            .finish();
        let url = format!("{}{path}", self.base_url);
        let response = self.client.get(&url).await?;
        let status = response.status().as_u16();
        let body = response.text().await?;
        Ok(parse_data(status, &body, "account")?)
    }

    /// `GET /v1/get_orderly_key?account_id=..&orderly_key=..` — scope and expiry of a key.
    pub async fn get_orderly_key(
        &self,
        account_id: &str,
        orderly_key: &str,
    ) -> ApiResult<OrderlyKeyLookup> {
        let path = PathQuery::new("/v1/get_orderly_key")
            .param("account_id", account_id)?
            .param("orderly_key", orderly_key)?
            .finish();
        let url = format!("{}{path}", self.base_url);
        let response = self.client.get(&url).await?;
        let status = response.status().as_u16();
        let body = response.text().await?;
        Ok(parse_data(status, &body, "orderly key")?)
    }

    /// `POST /v1/orderly_key` with a body from `WalletSigner::sign_add_orderly_key`.
    pub async fn add_orderly_key(
        &self,
        signed: &SignedAddOrderlyKey,
    ) -> ApiResult<OrderlyAddKeyResult> {
        let url = format!("{}/v1/orderly_key", self.base_url);
        let response = self
            .client
            .post(&url)
            .await
            .header("Content-Type", "application/json")
            .body(signed.body.clone())
            .send()
            .await?;
        let status = response.status().as_u16();
        let body = response.text().await?;
        Ok(parse_data(status, &body, "add orderly key")?)
    }
}
