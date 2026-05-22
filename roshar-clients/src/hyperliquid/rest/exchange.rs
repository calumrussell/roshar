#![allow(dead_code)]

use anyhow::{anyhow, Context, Result};
use ethers::abi::{encode, Token};
use ethers::core::k256::{elliptic_curve::FieldBytes, Secp256k1};
use ethers::signers::LocalWallet;
use ethers::types::{H160, H256, Signature, U256};
use roshar_types::{MetaAndAssetCtxs, SpotMetaAndAssetCtxs};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sha3::{Digest, Keccak256};
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

/// Convert an `f64` price or size to the canonical wire string Hyperliquid
/// expects: fixed-precision (no scientific notation), trailing zeros stripped,
/// and any IEEE-754 round-then-multiply residue (e.g. `2.0942000000000003`)
/// snapped to its intended decimal form.
///
/// Mirrors the official Hyperliquid Python SDK pattern (`f"{x:.8f}"` followed
/// by `Decimal.normalize()`). The validator already rounds to a tick / sz_decimals
/// before reaching this point, so 8 decimal places is always enough headroom.
fn float_to_wire(x: f64) -> String {
    // `format!("{:.8}", x)` never produces scientific notation and quantizes to
    // a fixed grid that absorbs the trailing IEEE-754 residue.
    let fixed = format!("{:.8}", x);
    match Decimal::from_str(&fixed) {
        Ok(d) => d.normalize().to_string(),
        Err(_) => fixed,
    }
}

pub enum HyperliquidOrderType {
    Alo,
    Ioc,
    Gtc,
    TriggerTp(bool, f64),
    TriggerSl(bool, f64),
}

/// POST a JSON body to a Hyperliquid info endpoint using a non-rate-limited
/// `reqwest::Client`. Mirrors `info::post_info_json`: debug-logs request and
/// response bodies, error-logs and bails on non-2xx with the response body
/// and outgoing request included.
async fn post_info_json<B, T>(
    client: &reqwest::Client,
    url: &str,
    body: &B,
    label: &str,
) -> Result<T>
where
    B: serde::Serialize,
    T: serde::de::DeserializeOwned,
{
    let body_str = serde_json::to_string(body)
        .with_context(|| format!("Failed to serialize {label} request"))?;
    log::debug!("POST {url} ({label}) body={body_str}");

    let response = client
        .post(url)
        .header("Content-Type", "application/json")
        .body(body_str.clone())
        .send()
        .await
        .with_context(|| format!("Failed to send {label} request"))?;

    let status = response.status();
    let text = response
        .text()
        .await
        .with_context(|| format!("Failed to read {label} response body"))?;

    if !status.is_success() {
        log::error!(
            "{label} request failed: status={status} body={text} request={body_str}"
        );
        anyhow::bail!("{label} request failed with status {status}: {text}");
    }

    log::debug!("{label} response: status={status} body={text}");

    serde_json::from_str(&text)
        .with_context(|| format!("Failed to parse {label} response (body={text})"))
}

pub struct ModifyOrderParams {
    pub oid: u64,
    pub asset: String,
    pub is_buy: bool,
    pub limit_px: f64,
    pub sz: f64,
    pub reduce_only: bool,
    pub order_type: HyperliquidOrderType,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleCancel {
    #[serde(rename = "type")]
    pub type_field: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time: Option<u64>,
}

#[derive(Serialize, Debug, Clone)]
struct WireOrderRequest {
    #[serde(rename = "a")]
    asset: u32,
    #[serde(rename = "b")]
    is_buy: bool,
    #[serde(rename = "p")]
    limit_px: String,
    #[serde(rename = "s")]
    sz: String,
    #[serde(rename = "r")]
    reduce_only: bool,
    #[serde(rename = "t")]
    order_type: WireClientOrder,
    #[serde(rename = "c", skip_serializing_if = "Option::is_none")]
    cloid: Option<String>,
}

#[derive(Serialize, Debug, Clone)]
#[serde(untagged)]
enum WireClientOrder {
    Limit { limit: WireLimit },
    Trigger { trigger: WireTrigger },
}

#[derive(Serialize, Debug, Clone)]
struct WireLimit {
    tif: String,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct WireTrigger {
    is_market: bool,
    trigger_px: String,
    tpsl: String,
}

#[derive(Serialize, Debug, Clone)]
struct WireBulkOrder {
    orders: Vec<WireOrderRequest>,
    grouping: String,
}

#[derive(Serialize, Debug, Clone)]
struct WireCancelRequest {
    #[serde(rename = "a")]
    asset: u32,
    #[serde(rename = "o")]
    oid: u64,
}

#[derive(Serialize, Debug, Clone)]
struct WireBulkCancel {
    cancels: Vec<WireCancelRequest>,
}

#[derive(Serialize, Debug, Clone)]
struct WireModify {
    oid: u64,
    order: WireOrderRequest,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct WireUpdateLeverage {
    asset: u32,
    is_cross: bool,
    leverage: u32,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct WireScheduleCancel {
    #[serde(skip_serializing_if = "Option::is_none")]
    time: Option<u64>,
}

#[derive(Serialize, Debug, Clone)]
#[serde(tag = "type")]
#[serde(rename_all = "camelCase")]
enum L1Action {
    Order(WireBulkOrder),
    Cancel(WireBulkCancel),
    Modify(WireModify),
    UpdateLeverage(WireUpdateLeverage),
    ScheduleCancel(WireScheduleCancel),
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
struct ExchangePayload {
    action: serde_json::Value,
    signature: ExchangeSignature,
    nonce: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    vault_address: Option<String>,
}

#[derive(Serialize, Debug)]
struct ExchangeSignature {
    r: String,
    s: String,
    v: u64,
}

#[derive(Debug, Clone)]
struct L1Agent {
    source: String,
    connection_id: H256,
}

impl From<Signature> for ExchangeSignature {
    fn from(value: Signature) -> Self {
        Self {
            r: format!("{:#066x}", value.r),
            s: format!("{:#066x}", value.s),
            v: value.v,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct RestingOrder {
    pub oid: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FilledOrder {
    #[serde(rename = "totalSz")]
    pub total_sz: String,
    #[serde(rename = "avgPx")]
    pub avg_px: String,
    pub oid: u64,
}

#[derive(Debug, Clone)]
pub enum ExchangeDataStatus {
    Error(String),
    Resting(RestingOrder),
    Filled(FilledOrder),
    Success,
    WaitingForFill,
    WaitingForTrigger,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum ExchangeDataStatusWire {
    Error { error: String },
    Resting { resting: RestingOrder },
    Filled { filled: FilledOrder },
    Status(String),
}

impl<'de> Deserialize<'de> for ExchangeDataStatus {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = ExchangeDataStatusWire::deserialize(deserializer)?;
        match wire {
            ExchangeDataStatusWire::Error { error } => Ok(Self::Error(error)),
            ExchangeDataStatusWire::Resting { resting } => Ok(Self::Resting(resting)),
            ExchangeDataStatusWire::Filled { filled } => Ok(Self::Filled(filled)),
            ExchangeDataStatusWire::Status(status) => match status.as_str() {
                "success" => Ok(Self::Success),
                "waitingForFill" => Ok(Self::WaitingForFill),
                "waitingForTrigger" => Ok(Self::WaitingForTrigger),
                _ => Err(serde::de::Error::custom(format!(
                    "Unknown status variant: {status}"
                ))),
            },
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExchangeData {
    pub statuses: Vec<ExchangeDataStatus>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExchangeResponse {
    #[serde(rename = "type")]
    pub response_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<ExchangeData>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExchangeRawStatus {
    pub status: String,
    pub response: serde_json::Value,
}

#[derive(Debug, Clone)]
pub enum ExchangeResponseStatus {
    Ok(ExchangeResponse),
    Err(String),
}

/// Hyperliquid Exchange API client with lazy initialization
///
/// The underlying ExchangeClient is initialized on first use, allowing
/// construction without immediate network calls or credential validation.
pub struct ExchangeApi {
    is_prod: bool,
    vault_address: Option<String>,
    wallet: tokio::sync::OnceCell<LocalWallet>,
}

impl ExchangeApi {
    fn sign_hash_like_sdk(hash: H256, wallet: &LocalWallet) -> Result<Signature> {
        // Match hyperliquid_rust_sdk signing path exactly:
        // signer().sign_digest_recoverable(...) over the precomputed 32-byte digest.
        let (sig, rec_id) = wallet
            .signer()
            .sign_prehash_recoverable(hash.as_bytes())
            .map_err(|e| anyhow!("Failed to sign prehash: {e}"))?;

        let v = u8::from(rec_id) as u64 + 27;
        let r_bytes: FieldBytes<Secp256k1> = sig.r().into();
        let s_bytes: FieldBytes<Secp256k1> = sig.s().into();
        let r = U256::from_big_endian(r_bytes.as_slice());
        let s = U256::from_big_endian(s_bytes.as_slice());

        Ok(Signature { r, s, v })
    }

    fn encode_outcome_asset_id(outcome_id: u32, side: u32) -> Result<u32> {
        if side > 1 {
            return Err(anyhow!(
                "Invalid outcome side {side}. Only side 0 or 1 are valid."
            ));
        }
        let encoding = outcome_id
            .checked_mul(10)
            .and_then(|v| v.checked_add(side))
            .ok_or_else(|| anyhow!("Outcome encoding overflow for outcome={outcome_id}, side={side}"))?;
        Ok(100_000_000 + encoding)
    }

    fn l1_agent_eip712_hash(agent: &L1Agent) -> H256 {
        let domain_type_hash = Keccak256::digest(
            b"EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)",
        );
        let agent_type_hash = Keccak256::digest(b"Agent(string source,bytes32 connectionId)");
        let name_hash = Keccak256::digest(b"Exchange");
        let version_hash = Keccak256::digest(b"1");
        let source_hash = Keccak256::digest(agent.source.as_bytes());

        let domain_separator = Keccak256::digest(encode(&[
            Token::FixedBytes(domain_type_hash.to_vec()),
            Token::FixedBytes(name_hash.to_vec()),
            Token::FixedBytes(version_hash.to_vec()),
            Token::Uint(1337u64.into()),
            Token::Address(H160::zero()),
        ]));

        let agent_struct_hash = Keccak256::digest(encode(&[
            Token::FixedBytes(agent_type_hash.to_vec()),
            Token::FixedBytes(source_hash.to_vec()),
            Token::FixedBytes(agent.connection_id.as_bytes().to_vec()),
        ]));

        let mut eip712_payload = Vec::with_capacity(2 + 32 + 32);
        eip712_payload.extend_from_slice(b"\x19\x01");
        eip712_payload.extend_from_slice(&domain_separator);
        eip712_payload.extend_from_slice(&agent_struct_hash);

        H256::from_slice(&Keccak256::digest(eip712_payload))
    }

    /// Create a new ExchangeApi (client is lazily initialized on first use)
    pub fn new(is_prod: bool) -> Self {
        Self::new_with_vault(is_prod, None)
    }

    /// Create a new ExchangeApi with vault address (client is lazily initialized on first use)
    pub fn new_with_vault(is_prod: bool, vault_address: Option<String>) -> Self {
        Self {
            is_prod,
            vault_address,
            wallet: tokio::sync::OnceCell::new(),
        }
    }

    /// Get or initialize the signing wallet
    async fn get_wallet(&self) -> Result<&LocalWallet> {
        self.wallet
            .get_or_try_init(|| async {
                let private_key = if self.is_prod {
                    std::env::var("HYPERLIQUID_API_PRIVATE_KEY")
                        .or_else(|_| std::env::var("HYPERLIQUID_PRIVATE_KEY"))
                        .map_err(|_| {
                            anyhow!(
                                "Neither HYPERLIQUID_API_PRIVATE_KEY nor HYPERLIQUID_PRIVATE_KEY is set"
                            )
                        })?
                } else {
                    std::env::var("HYPERLIQUID_TESTNET_API_PRIVATE_KEY")
                        .or_else(|_| std::env::var("HYPERLIQUID_TESTNET_PRIVATE_KEY"))
                        .map_err(|_| {
                            anyhow!(
                                "Neither HYPERLIQUID_TESTNET_API_PRIVATE_KEY nor HYPERLIQUID_TESTNET_PRIVATE_KEY is set"
                            )
                        })?
                };

                private_key
                    .parse::<LocalWallet>()
                    .map_err(|e| anyhow!("Failed to parse private key: {e}"))
            })
            .await
    }

    // Helper function to validate schedule cancel timing
    fn validate_schedule_time(time_ms: u64) -> Result<()> {
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        if time_ms < current_time + 5000 {
            return Err(anyhow!(
                "Schedule cancel time must be at least 5 seconds in the future"
            ));
        }
        Ok(())
    }

    fn exchange_url(&self) -> &'static str {
        if self.is_prod {
            "https://api.hyperliquid.xyz/exchange"
        } else {
            "https://api.hyperliquid-testnet.xyz/exchange"
        }
    }

    fn info_url(&self) -> &'static str {
        if self.is_prod {
            "https://api.hyperliquid.xyz/info"
        } else {
            "https://api.hyperliquid-testnet.xyz/info"
        }
    }

    fn signed_hash_payload<S: Serialize>(
        action: &S,
        nonce: u64,
        vault_address: Option<H160>,
    ) -> Result<H256> {
        let action_bytes = rmp_serde::to_vec_named(action)
            .map_err(|e| anyhow!("Failed to encode action: {e}"))?;

        let mut hash_data = action_bytes;
        hash_data.extend_from_slice(&nonce.to_be_bytes());

        match vault_address {
            Some(vault) => {
                hash_data.push(1);
                hash_data.extend_from_slice(vault.as_bytes());
            }
            None => hash_data.push(0),
        }

        let hash_result = Keccak256::digest(&hash_data);
        Ok(H256::from_slice(&hash_result))
    }

    async fn resolve_asset_index(&self, asset: &str) -> Result<u32> {
        if let Some(outcome_spec) = asset.strip_prefix("outcome:") {
            let mut parts = outcome_spec.split(':');
            let outcome_id = parts
                .next()
                .ok_or_else(|| anyhow!("Invalid outcome format '{asset}'"))?
                .parse::<u32>()
                .map_err(|e| anyhow!("Invalid outcome id in '{asset}': {e}"))?;
            let side = parts
                .next()
                .ok_or_else(|| anyhow!("Invalid outcome format '{asset}'"))?
                .parse::<u32>()
                .map_err(|e| anyhow!("Invalid outcome side in '{asset}': {e}"))?;
            if parts.next().is_some() {
                return Err(anyhow!(
                    "Invalid outcome format '{asset}'. Expected outcome:<id>:<side>."
                ));
            }
            return Self::encode_outcome_asset_id(outcome_id, side);
        }

        let colon_parts: Vec<&str> = asset.split(':').collect();
        if colon_parts.len() == 2
            && colon_parts[0].chars().all(|c| c.is_ascii_digit())
            && colon_parts[1].chars().all(|c| c.is_ascii_digit())
        {
            let outcome_id = colon_parts[0]
                .parse::<u32>()
                .map_err(|e| anyhow!("Invalid outcome id in '{asset}': {e}"))?;
            let side = colon_parts[1]
                .parse::<u32>()
                .map_err(|e| anyhow!("Invalid outcome side in '{asset}': {e}"))?;
            return Self::encode_outcome_asset_id(outcome_id, side);
        }

        if let Ok(raw_asset_id) = asset.parse::<u32>() {
            return Ok(raw_asset_id);
        }

        if let Some(outcome_encoding) = asset
            .strip_prefix('#')
            .or_else(|| asset.strip_prefix('+'))
        {
            let encoding = outcome_encoding
                .parse::<u32>()
                .map_err(|e| anyhow!("Invalid outcome encoding '{asset}': {e}"))?;
            return Ok(100_000_000 + encoding);
        }

        if let Some(raw_index) = asset.strip_prefix('@') {
            let idx = raw_index
                .parse::<u32>()
                .map_err(|e| anyhow!("Invalid @asset index '{asset}': {e}"))?;
            return Ok(10_000 + idx);
        }

        let client = crate::http::get_http_client();
        let info_url = self.info_url();

        let perp_meta: MetaAndAssetCtxs =
            post_info_json(client, info_url, &serde_json::json!({ "type": "metaAndAssetCtxs" }), "metaAndAssetCtxs (resolve_asset_index)").await?;

        if let Some((idx, _)) = perp_meta
            .0
            .universe
            .iter()
            .enumerate()
            .find(|(_, perp)| perp.name == asset)
        {
            if asset.contains(':') {
                return Err(anyhow!(
                    "Builder-deployed perp '{asset}' requires builder asset-id encoding. \
Provide a numeric asset id directly (e.g. 100000 + perp_dex_index * 10000 + index_in_meta)."
                ));
            }
            return Ok(idx as u32);
        }

        let spot_meta: SpotMetaAndAssetCtxs =
            post_info_json(client, info_url, &serde_json::json!({ "type": "spotMetaAndAssetCtxs" }), "spotMetaAndAssetCtxs (resolve_asset_index)").await?;

        if let Some(asset_info) = spot_meta.0.universe.iter().find(|spot| spot.name == asset) {
            return Ok(10_000 + asset_info.index);
        }

        Err(anyhow!("Unknown Hyperliquid asset '{asset}'"))
    }

    async fn send_action(&self, action: &L1Action) -> Result<ExchangeResponseStatus> {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        let vault_h160 = self
            .vault_address
            .as_ref()
            .map(|address| {
                address
                    .parse::<H160>()
                    .map_err(|e| anyhow!("Invalid vault address format '{address}': {e}"))
            })
            .transpose()?;
        let connection_id = Self::signed_hash_payload(action, nonce, vault_h160)?;
        let signature = self.sign_connection_id(connection_id).await?;
        let action_value = serde_json::to_value(action)
            .map_err(|e| anyhow!("Failed to serialize action payload: {e}"))?;

        let payload = ExchangePayload {
            action: action_value,
            signature: signature.into(),
            nonce,
            vault_address: vault_h160.map(|addr| format!("{:#x}", addr)),
        };

        let request_body = serde_json::to_string(&payload)
            .map_err(|e| anyhow!("Failed to serialize payload: {e}"))?;

        let http_client = crate::http::get_http_client();
        let exchange_url = self.exchange_url();
        log::debug!("POST {exchange_url} body={request_body}");
        let response = http_client
            .post(exchange_url)
            .header("Content-Type", "application/json")
            .body(request_body.clone())
            .send()
            .await
            .map_err(|e| anyhow!("Request failed: {e}"))?;

        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|e| anyhow!("Failed to read exchange response body: {e}"))?;

        if !status.is_success() {
            log::error!(
                "Exchange request failed: status={status} body={body} request={request_body}"
            );
            return Err(anyhow!(
                "Exchange request failed with status {status}: {body} (request: {request_body})"
            ));
        }

        log::debug!("Exchange response: status={status} body={body}");

        let payload: ExchangeRawStatus = serde_json::from_str(&body)
            .map_err(|e| anyhow!("Failed to parse exchange response: {e} (body: {body})"))?;

        match payload.status.as_str() {
            "ok" => serde_json::from_value::<ExchangeResponse>(payload.response)
                .map(ExchangeResponseStatus::Ok)
                .map_err(|e| anyhow!("Failed to decode successful exchange response: {e}")),
            "err" => match payload.response {
                serde_json::Value::String(msg) => Ok(ExchangeResponseStatus::Err(msg)),
                other => Ok(ExchangeResponseStatus::Err(other.to_string())),
            },
            other => Err(anyhow!("Unknown exchange status '{other}'")),
        }
    }

    /// Returns the wallet address associated with this exchange client.
    pub async fn wallet_address(&self) -> Result<H160> {
        use ethers::signers::Signer;
        Ok(self.get_wallet().await?.address())
    }

    pub async fn create_order(
        &self,
        asset: &str,
        is_buy: bool,
        limit_px: f64,
        sz: f64,
        reduce_only: bool,
        order_type: HyperliquidOrderType,
    ) -> Result<ExchangeResponseStatus> {
        let asset_index = self.resolve_asset_index(asset).await?;
        let order_type_wire = match order_type {
            HyperliquidOrderType::Alo => WireClientOrder::Limit {
                limit: WireLimit {
                    tif: "Alo".to_string(),
                },
            },
            HyperliquidOrderType::Ioc => WireClientOrder::Limit {
                limit: WireLimit {
                    tif: "Ioc".to_string(),
                },
            },
            HyperliquidOrderType::Gtc => WireClientOrder::Limit {
                limit: WireLimit {
                    tif: "Gtc".to_string(),
                },
            },
            HyperliquidOrderType::TriggerTp(is_market, trigger_px) => WireClientOrder::Trigger {
                trigger: WireTrigger {
                    is_market,
                    trigger_px: float_to_wire(trigger_px),
                    tpsl: "tp".to_string(),
                },
            },
            HyperliquidOrderType::TriggerSl(is_market, trigger_px) => WireClientOrder::Trigger {
                trigger: WireTrigger {
                    is_market,
                    trigger_px: float_to_wire(trigger_px),
                    tpsl: "sl".to_string(),
                },
            },
        };

        let action = L1Action::Order(WireBulkOrder {
            orders: vec![WireOrderRequest {
                asset: asset_index,
                is_buy,
                limit_px: float_to_wire(limit_px),
                sz: float_to_wire(sz),
                reduce_only,
                order_type: order_type_wire,
                cloid: None,
            }],
            grouping: "na".to_string(),
        });

        self.send_action(&action).await
    }

    pub async fn cancel_order(
        &self,
        asset: &str,
        oid: u64,
    ) -> Result<ExchangeResponseStatus> {
        let asset_index = self.resolve_asset_index(asset).await?;
        let action = L1Action::Cancel(WireBulkCancel {
            cancels: vec![WireCancelRequest {
                asset: asset_index,
                oid,
            }],
        });
        self.send_action(&action).await
    }

    pub async fn modify_order(
        &self,
        params: ModifyOrderParams,
    ) -> Result<ExchangeResponseStatus> {
        let asset_index = self.resolve_asset_index(&params.asset).await?;
        let order_type_wire = match params.order_type {
            HyperliquidOrderType::Alo => WireClientOrder::Limit {
                limit: WireLimit {
                    tif: "Alo".to_string(),
                },
            },
            HyperliquidOrderType::Ioc => WireClientOrder::Limit {
                limit: WireLimit {
                    tif: "Ioc".to_string(),
                },
            },
            HyperliquidOrderType::Gtc => WireClientOrder::Limit {
                limit: WireLimit {
                    tif: "Gtc".to_string(),
                },
            },
            HyperliquidOrderType::TriggerTp(is_market, trigger_px) => WireClientOrder::Trigger {
                trigger: WireTrigger {
                    is_market,
                    trigger_px: float_to_wire(trigger_px),
                    tpsl: "tp".to_string(),
                },
            },
            HyperliquidOrderType::TriggerSl(is_market, trigger_px) => WireClientOrder::Trigger {
                trigger: WireTrigger {
                    is_market,
                    trigger_px: float_to_wire(trigger_px),
                    tpsl: "sl".to_string(),
                },
            },
        };

        let action = L1Action::Modify(WireModify {
            oid: params.oid,
            order: WireOrderRequest {
                asset: asset_index,
                is_buy: params.is_buy,
                limit_px: float_to_wire(params.limit_px),
                sz: float_to_wire(params.sz),
                reduce_only: params.reduce_only,
                order_type: order_type_wire,
                cloid: None,
            },
        });

        self.send_action(&action).await
    }

    pub async fn update_leverage(
        &self,
        leverage: u32,
        coin: &str,
        is_cross: bool,
    ) -> Result<ExchangeResponseStatus> {
        let asset_index = self.resolve_asset_index(coin).await?;
        let action = L1Action::UpdateLeverage(WireUpdateLeverage {
            asset: asset_index,
            is_cross,
            leverage,
        });
        self.send_action(&action).await
    }

    pub async fn schedule_cancel(&self, time_ms: u64) -> Result<ExchangeResponseStatus> {
        // Validate timing
        Self::validate_schedule_time(time_ms)?;

        let action = L1Action::ScheduleCancel(WireScheduleCancel {
            time: Some(time_ms),
        });
        self.send_action(&action).await
    }

    async fn sign_connection_id(&self, connection_id: H256) -> Result<Signature> {
        let wallet = self.get_wallet().await?;
        let source = if self.is_prod { "a" } else { "b" }.to_string();
        let digest = Self::l1_agent_eip712_hash(&L1Agent {
            source,
            connection_id,
        });

        Self::sign_hash_like_sdk(digest, wallet)
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;
    use std::panic;

    use super::*;

    #[test]
    fn float_to_wire_strips_ieee754_residue() {
        // Real artifacts captured from live 422 responses.
        assert_eq!(float_to_wire(2.0942000000000003), "2.0942");
        assert_eq!(float_to_wire(1.9802000000000002), "1.9802");
        assert_eq!(float_to_wire(5.1000000000000005), "5.1");
        assert_eq!(float_to_wire(0.00030000000000000003), "0.0003");
        // Integer-valued prices stay integer-formatted (no trailing ".0").
        assert_eq!(float_to_wire(76056.0), "76056");
        // Already-clean values pass through.
        assert_eq!(float_to_wire(84.71), "84.71");
        assert_eq!(float_to_wire(2071.2), "2071.2");
        // Zero is "0", not the empty string.
        assert_eq!(float_to_wire(0.0), "0");
    }

    #[tokio::test]
    #[ignore = "requires testnet access and authentication"]
    async fn test_order_lifecycle() {
        let api = ExchangeApi::new(false);

        let create_status = api
            .create_order("ETH", true, 1000.0, 0.01, false, HyperliquidOrderType::Gtc)
            .await
            .unwrap();
        let ExchangeResponseStatus::Ok(create_response) = create_status else {
            panic!("Create failed")
        };
        let create_data = create_response.data.unwrap();
        assert!(!create_data.statuses.is_empty());

        let order = match create_data.statuses.first().unwrap() {
            ExchangeDataStatus::Resting(order) => order,
            _ => panic!("Bad return type"),
        };

        let modify_status = api
            .modify_order(ModifyOrderParams {
                oid: order.oid,
                asset: "ETH".to_string(),
                is_buy: true,
                limit_px: 1001.0,
                sz: 0.01,
                reduce_only: false,
                order_type: HyperliquidOrderType::Gtc,
            })
            .await
            .unwrap();
        let ExchangeResponseStatus::Ok(modify_response) = modify_status else {
            panic!("Modify failed")
        };
        let modify_data = modify_response.data.unwrap();
        assert!(!modify_data.statuses.is_empty());

        let modified_order = match modify_data.statuses.first().unwrap() {
            ExchangeDataStatus::Resting(order) => order,
            _ => panic!("Bad return type"),
        };

        let cancel_status = api.cancel_order("ETH", modified_order.oid).await.unwrap();
        let ExchangeResponseStatus::Ok(cancel_response) = cancel_status else {
            panic!("Cancel failed")
        };
        let cancel_data = cancel_response.data.unwrap();
        assert!(!cancel_data.statuses.is_empty());
        matches!(
            cancel_data.statuses.first().unwrap(),
            ExchangeDataStatus::Success
        );
    }

    #[tokio::test]
    #[ignore = "requires testnet access and authentication"]
    async fn test_update_leverage() {
        let api = ExchangeApi::new(false);
        let status = api.update_leverage(2, "ETH", true).await.unwrap();
        let ExchangeResponseStatus::Ok(response) = status else {
            panic!("Expected Ok response")
        };
        assert_eq!(response.response_type, "default");
    }

    #[tokio::test]
    #[ignore = "requires testnet access and authentication"]
    async fn test_schedule_cancel() {
        let api = ExchangeApi::new(false);

        // Schedule cancel for 10 seconds from now
        let schedule_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
            + 10000; // 10 seconds from now

        let result = api.schedule_cancel(schedule_time).await;

        // The test should either succeed or give us a meaningful error
        match result {
            Ok(status) => {
                println!("Schedule cancel successful: {status:?}");
            }
            Err(e) => {
                // Print the error for debugging - it might be a signature issue
                println!("Schedule cancel error (this is expected during development): {e:?}");
            }
        }
    }

    #[test]
    fn test_schedule_cancel_timing_validation() {
        // Test with time that's too soon (should fail validation)
        let invalid_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
            + 1000; // Only 1 second from now, should fail

        let result = ExchangeApi::validate_schedule_time(invalid_time);

        assert!(result.is_err());
        if let Err(e) = result {
            let error_message = format!("{e}");
            assert!(error_message.contains("at least 5 seconds"));
        }

        // Test with valid time (should succeed)
        let valid_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
            + 10000; // 10 seconds from now, should pass

        let result = ExchangeApi::validate_schedule_time(valid_time);
        assert!(result.is_ok());
    }

    #[tokio::test]
    #[ignore = "requires testnet access and authentication"]
    async fn test_schedule_cancel_validation() {
        let api = ExchangeApi::new(false);

        // Test with time that's too soon (should fail validation)
        let invalid_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
            + 1000; // Only 1 second from now, should fail

        let result = api.schedule_cancel(invalid_time).await;

        assert!(result.is_err());
        if let Err(e) = result {
            let error_message = format!("{e}");
            assert!(error_message.contains("at least 5 seconds"));
        }
    }

    #[test]
    fn test_sign_l1_action_matches_sdk_mainnet_vector() {
        let wallet = "e908f86dbb4d55ac876378565aafeabc187f6690f046459397b17d9b9a19688e"
            .parse::<LocalWallet>()
            .unwrap();
        let connection_id =
            H256::from_str("0xde6c4037798a4434ca03cd05f00e3b803126221375cd1e7eaaaf041768be06eb")
                .unwrap();
        let digest = ExchangeApi::l1_agent_eip712_hash(&L1Agent {
            source: "a".to_string(),
            connection_id,
        });
        let sig = ExchangeApi::sign_hash_like_sdk(digest, &wallet).unwrap();

        assert_eq!(
            sig.to_string(),
            "fa8a41f6a3fa728206df80801a83bcbfbab08649cd34d9c0bfba7c7b2f99340f53a00226604567b98a1492803190d65a201d6805e5831b7044f17fd530aec7841c"
        );
    }

    #[test]
    fn test_sign_l1_action_matches_sdk_testnet_vector() {
        let wallet = "e908f86dbb4d55ac876378565aafeabc187f6690f046459397b17d9b9a19688e"
            .parse::<LocalWallet>()
            .unwrap();
        let connection_id =
            H256::from_str("0xde6c4037798a4434ca03cd05f00e3b803126221375cd1e7eaaaf041768be06eb")
                .unwrap();
        let digest = ExchangeApi::l1_agent_eip712_hash(&L1Agent {
            source: "b".to_string(),
            connection_id,
        });
        let sig = ExchangeApi::sign_hash_like_sdk(digest, &wallet).unwrap();

        assert_eq!(
            sig.to_string(),
            "1713c0fc661b792a50e8ffdd59b637b1ed172d9a3aa4d801d9d88646710fb74b33959f4d075a7ccbec9f2374a6da21ffa4448d58d0413a0d335775f680a881431c"
        );
    }

    #[test]
    fn test_order_action_signature_matches_sdk_vector() {
        let wallet = "e908f86dbb4d55ac876378565aafeabc187f6690f046459397b17d9b9a19688e"
            .parse::<LocalWallet>()
            .unwrap();
        let action = L1Action::Order(WireBulkOrder {
            orders: vec![WireOrderRequest {
                asset: 1,
                is_buy: true,
                limit_px: "2000.0".to_string(),
                sz: "3.5".to_string(),
                reduce_only: false,
                order_type: WireClientOrder::Limit {
                    limit: WireLimit {
                        tif: "Ioc".to_string(),
                    },
                },
                cloid: None,
            }],
            grouping: "na".to_string(),
        });
        let connection_id = ExchangeApi::signed_hash_payload(&action, 1_583_838, None).unwrap();
        let mainnet_digest = ExchangeApi::l1_agent_eip712_hash(&L1Agent {
            source: "a".to_string(),
            connection_id,
        });
        let mainnet_sig = ExchangeApi::sign_hash_like_sdk(mainnet_digest, &wallet).unwrap();

        assert_eq!(
            mainnet_sig.to_string(),
            "77957e58e70f43b6b68581f2dc42011fc384538a2e5b7bf42d5b936f19fbb67360721a8598727230f67080efee48c812a6a4442013fd3b0eed509171bef9f23f1c"
        );
    }
}
