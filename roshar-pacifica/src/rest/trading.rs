use super::{parse_data, parse_envelope, BoxError};
use crate::types::{PacificaOpenOrder, PacificaPosition};
use anyhow::{anyhow, Context, Result};
use ed25519_dalek::{Signer, SigningKey};
use roshar_http::RateLimitedClient;
use serde::Deserialize;
use serde_json::{json, Value};
use std::fmt::Write as _;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::wire::float_to_wire;

pub struct TradingApi {
    client: RateLimitedClient,
    signing_key: SigningKey,
    /// Set only when the signing key is an agent key, i.e. not the account's own key: the exchange
    /// rejects an `agent_wallet` equal to `account`.
    agent_wallet: Option<String>,
    account: String,
    expiry_window_ms: u64,
    base_url: &'static str,
}

#[derive(Deserialize)]
struct OrderIdData {
    order_id: i64,
}

impl TradingApi {
    /// `private_key_b58` is a base58 64-byte Solana keypair: the account's own key or an API agent
    /// key bound to `account`. `expiry_window_ms` is the signature lifetime sent with every request
    /// (the exchange's default when omitted is 30 000). Pass a clone of the `RateLimitedClient` the
    /// market API uses: Pacifica's credit quota is per IP / account.
    pub fn new(
        client: RateLimitedClient,
        private_key_b58: &str,
        account: String,
        expiry_window_ms: u64,
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
        if signing_key.verifying_key().as_bytes()[..] != keypair_bytes[32..] {
            return Err(anyhow!(
                "Invalid Solana keypair: public half does not match the secret key"
            ));
        }
        let signer = bs58::encode(signing_key.verifying_key().to_bytes()).into_string();
        let agent_wallet = (signer != account).then_some(signer);
        Ok(Self {
            client,
            signing_key,
            agent_wallet,
            account,
            expiry_window_ms,
            base_url: super::base_url(is_mainnet),
        })
    }

    pub fn account(&self) -> &str {
        &self.account
    }

    fn now_ms() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("System time before UNIX epoch")
            .as_millis() as u64
    }

    /// The signed message, as the Python SDK's `prepare_message` builds it: header and `data`
    /// with keys sorted at every level, compact separators, and `json.dumps`' default ASCII
    /// escaping.
    fn signing_message(
        type_str: &str,
        payload: &Value,
        timestamp: u64,
        expiry_window_ms: u64,
    ) -> Result<String> {
        let message = sort_keys(json!({
            "data": payload,
            "expiry_window": expiry_window_ms,
            "timestamp": timestamp,
            "type": type_str,
        }));
        let compact =
            serde_json::to_string(&message).context("Failed to serialize signing message")?;
        Ok(ascii_escape(&compact))
    }

    /// The request body for `payload` signed at `timestamp`: the signature fields, then the
    /// payload's own fields at the top level (not under `data`).
    fn build_request_at(&self, type_str: &str, payload: Value, timestamp: u64) -> Result<Value> {
        let message = Self::signing_message(type_str, &payload, timestamp, self.expiry_window_ms)?;
        let signature = self.signing_key.sign(message.as_bytes());
        let mut request = json!({
            "account": &self.account,
            "expiry_window": self.expiry_window_ms,
            "signature": bs58::encode(signature.to_bytes()).into_string(),
            "timestamp": timestamp,
        });
        let req_map = request
            .as_object_mut()
            .expect("request literal is a JSON object");
        if let Some(agent) = &self.agent_wallet {
            req_map.insert("agent_wallet".to_string(), Value::String(agent.clone()));
        }
        let Value::Object(pay_map) = payload else {
            return Err(anyhow!("Pacifica payload must be a JSON object"));
        };
        req_map.extend(pay_map);
        Ok(request)
    }

    async fn post_signed(
        &self,
        type_str: &str,
        path: &str,
        payload: Value,
    ) -> Result<(bool, String, String), BoxError> {
        let request = self
            .build_request_at(type_str, payload, Self::now_ms())
            .map_err(|e| format!("Failed to build {type_str} request: {e}"))?;
        let url = format!("{}/{path}", self.base_url);
        let response = self.client.post(&url).await.json(&request).send().await?;
        let status = response.status();
        let text = response.text().await?;
        Ok((status.is_success(), status.to_string(), text))
    }

    fn limit_order_payload(
        symbol: &str,
        price: f64,
        amount: f64,
        is_buy: bool,
        tif: &str,
        reduce_only: bool,
        client_order_id: Option<String>,
    ) -> Result<Value, BoxError> {
        let mut payload = json!({
            "amount": float_to_wire(amount)?,
            "price": float_to_wire(price)?,
            "reduce_only": reduce_only,
            "side": if is_buy { "bid" } else { "ask" },
            "symbol": symbol,
            "tif": tif,
        });
        if let Some(coid) = client_order_id {
            payload["client_order_id"] = Value::String(coid);
        }
        Ok(payload)
    }

    fn cancel_payload(symbol: &str, order_id: i64) -> Value {
        json!({
            "order_id": order_id,
            "symbol": symbol,
        })
    }

    fn edit_payload(
        symbol: &str,
        order_id: i64,
        price: f64,
        amount: f64,
    ) -> Result<Value, BoxError> {
        Ok(json!({
            "amount": float_to_wire(amount)?,
            "order_id": order_id,
            "price": float_to_wire(price)?,
            "symbol": symbol,
        }))
    }

    /// `POST /api/v1/orders/create` (`create_order`). `tif` is sent as given (`GTC`, `IOC`,
    /// `ALO`, `TOB`). Returns the exchange order id.
    pub async fn create_limit_order(
        &self,
        symbol: &str,
        price: f64,
        amount: f64,
        is_buy: bool,
        tif: &str,
        reduce_only: bool,
        client_order_id: Option<String>,
    ) -> Result<i64, BoxError> {
        let payload = Self::limit_order_payload(
            symbol,
            price,
            amount,
            is_buy,
            tif,
            reduce_only,
            client_order_id,
        )?;
        let (ok, status, text) = self
            .post_signed("create_order", "orders/create", payload)
            .await?;
        let data: OrderIdData = parse_data(ok, status, &text, "orders/create")?;
        Ok(data.order_id)
    }

    /// `POST /api/v1/orders/cancel` (`cancel_order`).
    pub async fn cancel_order(&self, symbol: &str, order_id: i64) -> Result<(), BoxError> {
        let payload = Self::cancel_payload(symbol, order_id);
        let (ok, status, text) = self
            .post_signed("cancel_order", "orders/cancel", payload)
            .await?;
        parse_envelope::<Value>(ok, status, &text, "orders/cancel")?;
        Ok(())
    }

    /// `POST /api/v1/orders/edit` (`edit_order`). The exchange cancels the order and places a new
    /// ALO order; returns the new order id.
    pub async fn edit_order(
        &self,
        symbol: &str,
        order_id: i64,
        price: f64,
        amount: f64,
    ) -> Result<i64, BoxError> {
        let payload = Self::edit_payload(symbol, order_id, price, amount)?;
        let (ok, status, text) = self
            .post_signed("edit_order", "orders/edit", payload)
            .await?;
        let data: OrderIdData = parse_data(ok, status, &text, "orders/edit")?;
        Ok(data.order_id)
    }

    /// `GET /api/v1/orders?account=...`
    pub async fn get_open_orders(&self) -> Result<Vec<PacificaOpenOrder>, BoxError> {
        let url = format!("{}/orders?account={}", self.base_url, self.account);
        let response = self.client.get(&url).await?;
        let status = response.status();
        let text = response.text().await?;
        parse_data(status.is_success(), status, &text, "orders")
    }

    /// `GET /api/v1/positions?account=...`
    pub async fn get_positions(&self) -> Result<Vec<PacificaPosition>, BoxError> {
        let url = format!("{}/positions?account={}", self.base_url, self.account);
        let response = self.client.get(&url).await?;
        let status = response.status();
        let text = response.text().await?;
        parse_data(status.is_success(), status, &text, "positions")
    }
}

/// Rebuilds every object with its keys in sorted order, as the SDK's `sort_json_keys` does. With
/// serde_json's default BTreeMap this is already the case; with the `preserve_order` feature
/// (which any crate in a build can switch on) it is not.
fn sort_keys(value: Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut entries: Vec<(String, Value)> = map.into_iter().collect();
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            Value::Object(
                entries
                    .into_iter()
                    .map(|(k, v)| (k, sort_keys(v)))
                    .collect(),
            )
        }
        Value::Array(items) => Value::Array(items.into_iter().map(sort_keys).collect()),
        other => other,
    }
}

/// Escapes every character outside printable ASCII (`' '..='~'`) as `\uXXXX` (UTF-16 code units),
/// matching Python's `json.dumps(ensure_ascii=True)`. serde_json already escapes control
/// characters the same way, and such characters can only appear inside JSON strings, so escaping
/// the serialized text is equivalent to escaping each string.
fn ascii_escape(json: &str) -> String {
    if json.bytes().all(|b| b < 0x7f) {
        return json.to_string();
    }
    let mut out = String::with_capacity(json.len() + 16);
    for c in json.chars() {
        if (c as u32) < 0x7f {
            out.push(c);
        } else {
            let mut units = [0u16; 2];
            for unit in c.encode_utf16(&mut units) {
                write!(out, "\\u{unit:04x}").expect("writing to a String cannot fail");
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    //! Parity with the official Python SDK: `tests/vectors.json` was produced by
    //! `tests/gen_vectors.py` with `common/utils.py::sign_message` and throwaway keys. Never
    //! regenerate the vectors to make a change pass.
    use super::*;

    const VECTORS: &str = include_str!("../../tests/vectors.json");

    fn api(private_key: &str, account: &str, expiry_window_ms: u64) -> TradingApi {
        TradingApi::new(
            RateLimitedClient::new(1, 1),
            private_key,
            account.to_string(),
            expiry_window_ms,
            true,
        )
        .expect("vector key is a valid keypair")
    }

    #[test]
    fn signing_matches_python_sdk() {
        let file: Value = serde_json::from_str(VECTORS).unwrap();
        let key = file["private_key_b58"].as_str().unwrap();
        let vectors = file["vectors"].as_array().unwrap();
        assert_eq!(vectors.len(), 5);
        for v in vectors {
            let name = v["name"].as_str().unwrap();
            let typ = v["type"].as_str().unwrap();
            let timestamp = v["timestamp"].as_u64().unwrap();
            let expiry = v["expiry_window"].as_u64().unwrap();
            let payload = v["payload"].clone();

            let message = TradingApi::signing_message(typ, &payload, timestamp, expiry).unwrap();
            assert_eq!(message, v["message"].as_str().unwrap(), "{name}: message");

            let api = api(key, v["account"].as_str().unwrap(), expiry);
            let request = api.build_request_at(typ, payload, timestamp).unwrap();
            assert_eq!(request["signature"], v["signature"], "{name}: signature");
            assert_eq!(request, v["request"], "{name}: request body");
        }
    }

    #[test]
    fn payload_builders_match_python_sdk() {
        let file: Value = serde_json::from_str(VECTORS).unwrap();
        let mut checked = 0;
        for v in file["vectors"].as_array().unwrap() {
            let name = v["name"].as_str().unwrap();
            let i = &v["inputs"];
            let built = match i["kind"].as_str() {
                Some("limit") => TradingApi::limit_order_payload(
                    i["symbol"].as_str().unwrap(),
                    i["price"].as_f64().unwrap(),
                    i["amount"].as_f64().unwrap(),
                    i["is_buy"].as_bool().unwrap(),
                    i["tif"].as_str().unwrap(),
                    i["reduce_only"].as_bool().unwrap(),
                    i["client_order_id"].as_str().map(str::to_string),
                )
                .unwrap(),
                Some("cancel") => TradingApi::cancel_payload(
                    i["symbol"].as_str().unwrap(),
                    i["order_id"].as_i64().unwrap(),
                ),
                Some("edit") => TradingApi::edit_payload(
                    i["symbol"].as_str().unwrap(),
                    i["order_id"].as_i64().unwrap(),
                    i["price"].as_f64().unwrap(),
                    i["amount"].as_f64().unwrap(),
                )
                .unwrap(),
                _ => continue,
            };
            assert_eq!(built, v["payload"], "{name}: payload");
            checked += 1;
        }
        assert_eq!(checked, 4);
    }

    #[test]
    fn main_key_sends_no_agent_wallet() {
        let file: Value = serde_json::from_str(VECTORS).unwrap();
        let key = file["private_key_b58"].as_str().unwrap();
        let own = api(key, file["public_key"].as_str().unwrap(), 5000);
        assert!(own.agent_wallet.is_none());
        let agent = api(key, "6ETnufiec2CxVWTS4u5Wiq33Zh5Y3Qm6Pkdpi375fuxP", 5000);
        assert_eq!(agent.agent_wallet.as_deref(), file["public_key"].as_str());
    }

    #[test]
    fn rejects_mismatched_keypair() {
        let file: Value = serde_json::from_str(VECTORS).unwrap();
        let mut bytes = bs58::decode(file["private_key_b58"].as_str().unwrap())
            .into_vec()
            .unwrap();
        bytes[63] ^= 1;
        let bad = bs58::encode(bytes).into_string();
        assert!(
            TradingApi::new(RateLimitedClient::new(1, 1), &bad, "x".into(), 5000, true).is_err()
        );
    }
}
