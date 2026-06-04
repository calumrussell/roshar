use crate::http::RateLimitedClient;
use anyhow::{anyhow, Context, Result};
use ed25519_dalek::{Signer, SigningKey};
use roshar_types::pacifica::{
    PacificaOpenOrder, PacificaOpenOrdersResponse, PacificaPosition, PacificaPositionsResponse,
};
use serde_json::{json, Value};
use std::time::{SystemTime, UNIX_EPOCH};

const MAINNET_BASE_URL: &str = "https://api.pacifica.fi/api/v1";
const TESTNET_BASE_URL: &str = "https://test-api.pacifica.fi/api/v1";
const DEFAULT_EXPIRY_WINDOW_MS: i64 = 5_000;

pub struct TradingApi {
    client: RateLimitedClient,
    signing_key: SigningKey,
    agent_wallet: String,
    account: String,
    base_url: &'static str,
}

impl TradingApi {
    pub fn new(
        requests_per_second: u32,
        private_key_b58: &str,
        account: String,
        is_mainnet: bool,
    ) -> Result<Self> {
        let keypair_bytes = bs58::decode(private_key_b58)
            .into_vec()
            .context("Failed to decode Pacifica private key from base58")?;
        if keypair_bytes.len() != 64 {
            return Err(anyhow!(
                "Invalid Solana keypair: expected 64 bytes, got {}",
                keypair_bytes.len()
            ));
        }
        let seed: [u8; 32] = keypair_bytes[..32]
            .try_into()
            .context("Failed to extract ed25519 seed from keypair bytes")?;
        let signing_key = SigningKey::from_bytes(&seed);
        let agent_wallet = bs58::encode(signing_key.verifying_key().to_bytes()).into_string();
        Ok(Self {
            client: RateLimitedClient::new(requests_per_second, 1),
            signing_key,
            agent_wallet,
            account,
            base_url: if is_mainnet {
                MAINNET_BASE_URL
            } else {
                TESTNET_BASE_URL
            },
        })
    }

    pub fn account(&self) -> &str {
        &self.account
    }

    fn now_ms() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("System time before UNIX epoch")
            .as_millis() as i64
    }

    // Signs the payload and returns (timestamp_ms, base58_signature).
    // Message format mirrors the Pacifica Python SDK: sort_json_keys({type, timestamp, expiry_window, data: payload})
    // serde_json uses BTreeMap by default (no preserve_order feature), so JSON keys are sorted alphabetically.
    fn sign(&self, type_str: &str, payload: &Value) -> Result<(i64, String)> {
        let timestamp = Self::now_ms();
        let message = json!({
            "data": payload,
            "expiry_window": DEFAULT_EXPIRY_WINDOW_MS,
            "timestamp": timestamp,
            "type": type_str,
        });
        let message_str =
            serde_json::to_string(&message).context("Failed to serialize signing message")?;
        let signature = self.signing_key.sign(message_str.as_bytes());
        let signature_b58 = bs58::encode(signature.to_bytes()).into_string();
        Ok((timestamp, signature_b58))
    }

    fn build_request(&self, type_str: &str, payload: Value) -> Result<Value> {
        let (timestamp, signature) = self.sign(type_str, &payload)?;
        let mut request = json!({
            "account": &self.account,
            "agent_wallet": &self.agent_wallet,
            "expiry_window": DEFAULT_EXPIRY_WINDOW_MS,
            "signature": signature,
            "timestamp": timestamp,
        });
        if let (Some(req_map), Some(pay_map)) =
            (request.as_object_mut(), payload.as_object())
        {
            for (k, v) in pay_map {
                req_map.insert(k.clone(), v.clone());
            }
        }
        Ok(request)
    }

    pub async fn create_limit_order(
        &self,
        symbol: &str,
        price: f64,
        amount: f64,
        is_buy: bool,
        tif: &str,
        reduce_only: bool,
        client_order_id: Option<String>,
    ) -> Result<i64, Box<dyn std::error::Error + Send + Sync>> {
        let side = if is_buy { "bid" } else { "ask" };
        let mut payload = json!({
            "amount": amount.to_string(),
            "price": price.to_string(),
            "reduce_only": reduce_only,
            "side": side,
            "symbol": symbol,
            "tif": tif,
        });
        if let Some(coid) = client_order_id {
            payload["client_order_id"] = Value::String(coid);
        }

        let request = self
            .build_request("create_order", payload)
            .map_err(|e| format!("Failed to build create_order request: {e}"))?;

        let url = format!("{}/orders/create", self.base_url);
        let response = self
            .client
            .post(&url)
            .await
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("Pacifica create_limit_order failed ({status}): {body}").into());
        }

        let text = response.text().await?;
        let parsed: Value = serde_json::from_str(&text)
            .map_err(|e| format!("Failed to parse create_limit_order response: {e}"))?;
        parsed["data"]["order_id"]
            .as_i64()
            .ok_or_else(|| format!("No order_id in create_limit_order response: {text}").into())
    }

    pub async fn cancel_order(
        &self,
        symbol: &str,
        order_id: i64,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let payload = json!({
            "order_id": order_id,
            "symbol": symbol,
        });
        let request = self
            .build_request("cancel_order", payload)
            .map_err(|e| format!("Failed to build cancel_order request: {e}"))?;

        let url = format!("{}/orders/cancel", self.base_url);
        let response = self
            .client
            .post(&url)
            .await
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("Pacifica cancel_order failed ({status}): {body}").into());
        }

        Ok(())
    }

    pub async fn edit_order(
        &self,
        symbol: &str,
        order_id: i64,
        price: f64,
        amount: f64,
    ) -> Result<i64, Box<dyn std::error::Error + Send + Sync>> {
        let payload = json!({
            "amount": amount.to_string(),
            "order_id": order_id,
            "price": price.to_string(),
            "symbol": symbol,
        });
        let request = self
            .build_request("edit_order", payload)
            .map_err(|e| format!("Failed to build edit_order request: {e}"))?;

        let url = format!("{}/orders/edit", self.base_url);
        let response = self
            .client
            .post(&url)
            .await
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("Pacifica edit_order failed ({status}): {body}").into());
        }

        let text = response.text().await?;
        let parsed: Value = serde_json::from_str(&text)
            .map_err(|e| format!("Failed to parse edit_order response: {e}"))?;
        parsed["data"]["order_id"]
            .as_i64()
            .ok_or_else(|| format!("No order_id in edit_order response: {text}").into())
    }

    pub async fn get_open_orders(
        &self,
    ) -> Result<Vec<PacificaOpenOrder>, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/orders?account={}", self.base_url, self.account);
        let response = self.client.get(&url).await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("Pacifica get_open_orders failed ({status}): {body}").into());
        }

        let text = response.text().await?;
        let resp: PacificaOpenOrdersResponse = serde_json::from_str(&text)
            .map_err(|e| format!("Failed to parse open orders response: {e}"))?;
        Ok(resp.data)
    }

    pub async fn get_positions(
        &self,
    ) -> Result<Vec<PacificaPosition>, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/positions?account={}", self.base_url, self.account);
        let response = self.client.get(&url).await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("Pacifica get_positions failed ({status}): {body}").into());
        }

        let text = response.text().await?;
        let resp: PacificaPositionsResponse = serde_json::from_str(&text)
            .map_err(|e| format!("Failed to parse positions response: {e}"))?;
        Ok(resp.data)
    }
}
