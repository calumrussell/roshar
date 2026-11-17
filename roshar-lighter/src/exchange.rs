//! REST transaction submission and the account reads that go with it.
//!
//! `TxApi::new(LIGHTER_API_URL)` wraps `/api/v1/sendTx`, `/api/v1/sendTxBatch`, `/api/v1/nextNonce`,
//! `/api/v1/apikeys`, `/api/v1/account` and `/api/v1/orderBookDetails`. None of these need an auth
//! token: transactions carry their own signature. Responses are parsed strictly; a missing field
//! is an error. A non-200 `code` in the body is returned as `Err` with the exchange's message.

use anyhow::{bail, Context, Result};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use std::time::Duration;

use crate::signer::SignedTx;

pub const LIGHTER_API_URL: &str = "https://mainnet.zklighter.elliot.ai";
pub const LIGHTER_TESTNET_API_URL: &str = "https://testnet.zklighter.elliot.ai";

/// Reply to `sendTx` (`RespSendTx`). `volume_quota_remaining` and `remaining_send_tx` are optional
/// in the schema; `remaining_send_tx` is only returned for Premium/Plus accounts.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct SendTxResponse {
    pub code: i64,
    #[serde(default)]
    pub message: Option<String>,
    pub tx_hash: String,
    pub predicted_execution_time_ms: i64,
    pub volume_quota_remaining: Option<i64>,
    pub remaining_send_tx: Option<i64>,
}

/// Reply to `sendTxBatch` (`RespSendTxBatch`). Optional fields as for `SendTxResponse`.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct SendTxBatchResponse {
    pub code: i64,
    #[serde(default)]
    pub message: Option<String>,
    pub tx_hash: Vec<String>,
    pub predicted_execution_time_ms: i64,
    pub volume_quota_remaining: Option<i64>,
    pub remaining_send_tx: Option<i64>,
}

/// One registered API key (`/api/v1/apikeys`).
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct ApiKeyInfo {
    pub account_index: i64,
    pub api_key_index: u8,
    pub nonce: i64,
    pub public_key: String,
}

/// One position of an account (`/api/v1/account`). `position` is unsigned; `sign` is 1 (long) or -1.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct AccountPosition {
    pub market_id: i64,
    pub symbol: String,
    pub sign: i64,
    pub position: String,
    pub avg_entry_price: String,
    pub open_order_count: i64,
    pub pending_order_count: i64,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct Account {
    pub index: i64,
    pub collateral: String,
    pub available_balance: String,
    pub positions: Vec<AccountPosition>,
}

/// The order-entry fields of `/api/v1/orderBookDetails`. Prices are integers in units of
/// 10^-price_decimals, sizes in units of 10^-size_decimals.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct OrderBookDetail {
    pub market_id: i64,
    pub symbol: String,
    pub status: String,
    pub price_decimals: u32,
    pub size_decimals: u32,
    pub min_base_amount: String,
    pub min_quote_amount: String,
    pub taker_fee: String,
    pub maker_fee: String,
}

#[derive(Deserialize)]
struct Code {
    code: i64,
    #[serde(default)]
    message: Option<String>,
}

#[derive(Deserialize)]
struct NextNonce {
    nonce: i64,
}

#[derive(Deserialize)]
struct ApiKeys {
    api_keys: Vec<ApiKeyInfo>,
}

#[derive(Deserialize)]
struct Accounts {
    accounts: Vec<Account>,
}

/// Perp markets come back in `order_book_details`, spot markets in `spot_order_book_details`.
#[derive(Deserialize)]
struct Details {
    order_book_details: Vec<OrderBookDetail>,
    spot_order_book_details: Vec<OrderBookDetail>,
}

pub struct TxApi {
    http: reqwest::Client,
    base: String,
}

impl TxApi {
    /// `base_url` without the `/api/v1` suffix, e.g. `LIGHTER_API_URL`.
    pub fn new(base_url: &str) -> Result<TxApi> {
        Self::with_timeout(base_url, Duration::from_secs(10))
    }

    pub fn with_timeout(base_url: &str, timeout: Duration) -> Result<TxApi> {
        let http = reqwest::Client::builder().timeout(timeout).build()?;
        Ok(TxApi {
            http,
            base: base_url.trim_end_matches('/').to_string(),
        })
    }

    /// `POST /api/v1/sendTx` (form-encoded `tx_type`, `tx_info`).
    pub async fn send_tx(&self, tx: &SignedTx) -> Result<SendTxResponse> {
        let form = [
            ("tx_type", tx.tx_type.to_string()),
            ("tx_info", tx.tx_info.clone()),
        ];
        let r = self
            .http
            .post(format!("{}/api/v1/sendTx", self.base))
            .form(&form)
            .send()
            .await?;
        parse(r).await
    }

    /// `POST /api/v1/sendTxBatch`: up to 50 transactions in nonce order.
    pub async fn send_tx_batch(&self, txs: &[SignedTx]) -> Result<SendTxBatchResponse> {
        let (types, infos) = batch_fields(txs)?;
        let form = [("tx_types", types), ("tx_infos", infos)];
        let r = self
            .http
            .post(format!("{}/api/v1/sendTxBatch", self.base))
            .form(&form)
            .send()
            .await?;
        parse(r).await
    }

    /// `GET /api/v1/nextNonce`: the nonce the next transaction of this API key must use.
    pub async fn next_nonce(&self, account_index: i64, api_key_index: u8) -> Result<i64> {
        let q = [
            ("account_index", account_index.to_string()),
            ("api_key_index", api_key_index.to_string()),
        ];
        let r = self
            .http
            .get(format!("{}/api/v1/nextNonce", self.base))
            .query(&q)
            .send()
            .await?;
        Ok(parse::<NextNonce>(r).await?.nonce)
    }

    /// `GET /api/v1/apikeys` for one key. Compare `public_key` with `Signer::public_key_hex` to
    /// check the key is registered (what the Python SDK's `check_client` does).
    pub async fn api_key(&self, account_index: i64, api_key_index: u8) -> Result<ApiKeyInfo> {
        let q = [
            ("account_index", account_index.to_string()),
            ("api_key_index", api_key_index.to_string()),
        ];
        let r = self
            .http
            .get(format!("{}/api/v1/apikeys", self.base))
            .query(&q)
            .send()
            .await?;
        let keys = parse::<ApiKeys>(r).await?.api_keys;
        keys.into_iter()
            .find(|k| k.api_key_index == api_key_index)
            .with_context(|| {
                format!("api key {api_key_index} not registered on account {account_index}")
            })
    }

    /// `GET /api/v1/account?by=index`.
    pub async fn account(&self, account_index: i64) -> Result<Account> {
        let q = [
            ("by", "index".to_string()),
            ("value", account_index.to_string()),
        ];
        let r = self
            .http
            .get(format!("{}/api/v1/account", self.base))
            .query(&q)
            .send()
            .await?;
        let mut accounts = parse::<Accounts>(r).await?.accounts;
        if accounts.len() != 1 {
            bail!(
                "expected one account {account_index}, got {}",
                accounts.len()
            );
        }
        Ok(accounts.remove(0))
    }

    /// `GET /api/v1/orderBookDetails?market_id=`.
    pub async fn order_book_detail(&self, market_id: i64) -> Result<OrderBookDetail> {
        let q = [("market_id", market_id.to_string())];
        let r = self
            .http
            .get(format!("{}/api/v1/orderBookDetails", self.base))
            .query(&q)
            .send()
            .await?;
        let d = parse::<Details>(r).await?;
        d.order_book_details
            .into_iter()
            .chain(d.spot_order_book_details)
            .find(|x| x.market_id == market_id)
            .with_context(|| format!("market {market_id} not in orderBookDetails"))
    }

    /// `GET /api/v1/orderBookOrders?market_id=&limit=` as the raw body (status, bytes), for
    /// callers that record it verbatim. Non-200 statuses are returned, not turned into errors.
    pub async fn order_book_orders_raw(
        &self,
        market_id: i64,
        limit: u32,
    ) -> Result<(u16, bytes::Bytes)> {
        let q = [
            ("market_id", market_id.to_string()),
            ("limit", limit.to_string()),
        ];
        let r = self
            .http
            .get(format!("{}/api/v1/orderBookOrders", self.base))
            .query(&q)
            .send()
            .await?;
        let status = r.status().as_u16();
        Ok((status, r.bytes().await?))
    }
}

/// `tx_types` / `tx_infos` as `sendTxBatch` (REST form and WS `jsonapi/sendtxbatch`) expect them:
/// a JSON list of ints and a JSON list of `tx_info` strings, each serialised to a string.
pub fn batch_fields(txs: &[SignedTx]) -> Result<(String, String)> {
    if txs.is_empty() {
        bail!("empty batch");
    }
    let types: Vec<u8> = txs.iter().map(|t| t.tx_type).collect();
    let infos: Vec<&str> = txs.iter().map(|t| t.tx_info.as_str()).collect();
    Ok((
        serde_json::to_string(&types)?,
        serde_json::to_string(&infos)?,
    ))
}

async fn parse<T: DeserializeOwned>(r: reqwest::Response) -> Result<T> {
    let status = r.status();
    let body = r.text().await?;
    parse_body(status.as_u16(), &body)
}

fn parse_body<T: DeserializeOwned>(status: u16, body: &str) -> Result<T> {
    let code: Code =
        serde_json::from_str(body).with_context(|| format!("HTTP {status}: {body}"))?;
    if code.code != 200 {
        bail!(
            "lighter code {}: {} (HTTP {status})",
            code.code,
            code.message.unwrap_or_default()
        );
    }
    serde_json::from_str(body).with_context(|| format!("unexpected response: {body}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn send_tx_reply_and_errors() {
        let ok: SendTxResponse = parse_body(
            200,
            r#"{"code":200,"message":"{\"ratelimit\": \"didn't use volume quota\"}","tx_hash":"ab","predicted_execution_time_ms":1763200000000,"volume_quota_remaining":10}"#,
        )
        .unwrap();
        assert_eq!(ok.tx_hash, "ab");
        assert_eq!(ok.volume_quota_remaining, Some(10));
        let no_quota: SendTxResponse = parse_body(
            200,
            r#"{"code":200,"tx_hash":"ab","predicted_execution_time_ms":1763200000000}"#,
        )
        .unwrap();
        assert_eq!(no_quota.volume_quota_remaining, None);
        let err = parse_body::<SendTxResponse>(400, r#"{"code":21120,"message":"invalid nonce"}"#)
            .unwrap_err();
        assert!(err.to_string().contains("invalid nonce"));
        assert!(parse_body::<SendTxResponse>(200, r#"{"code":200}"#).is_err());
    }

    #[test]
    fn batch_fields_are_json_strings() {
        let t = SignedTx {
            tx_type: 14,
            tx_info: r#"{"a":1}"#.into(),
            tx_hash: "h".into(),
        };
        let (types, infos) = batch_fields(&[t.clone(), t]).unwrap();
        assert_eq!(types, "[14,14]");
        assert_eq!(infos, r#"["{\"a\":1}","{\"a\":1}"]"#);
    }
}
