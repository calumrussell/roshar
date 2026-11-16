//! edgeX V2 signing.
//!
//! Two independent schemes, both ported from `edgex-python-sdk` 2.0.1:
//!
//! - **Request auth (HMAC-SHA256).** Every private REST call and the private websocket handshake
//!   carry `X-edgeX-Api-Key`, `X-edgeX-Passphrase`, `X-edgeX-Timestamp` and `X-edgeX-Signature`,
//!   where the signature is `hex(HMAC-SHA256(key = base64(api_secret), timestamp + METHOD + path +
//!   payload))`. For GET the payload is the query sorted by key (`k=v&...`); for POST it is the
//!   JSON body flattened by [`canonical_value`].
//! - **L2 signatures (EIP-712).** Orders and margin-mode changes are signed with the account's
//!   trading ("signer") key over the `EdgeX` / `1` domain with the chain id and verifying contract
//!   from `getMetaData().global`. Signatures are `r || s || v` hex without `0x`, `v` = 27/28.
//!
//! Nothing here reads the clock or the environment: timestamps, expiries and client order ids are
//! inputs.

use crate::types::{
    CreateOrderBody, EdgexGlobal, EdgexMarginMode, EdgexOrderType, EdgexSide, EdgexTimeInForce,
    EdgexTriggerPriceType, SetMarginModeBody,
};
use anyhow::{anyhow, bail, Context, Result};
use base64::Engine as _;
use hmac::{Hmac, Mac};
use k256::ecdsa::SigningKey;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde_json::Value;
use sha2::Sha256;
use sha3::{Digest, Keccak256};
use std::str::FromStr;

/// `l2ExpireTime - expireTime` used by both official SDKs. The order docs require
/// `l2ExpireTime >= expireTime + 8 days`.
pub const L1_EXPIRE_OFFSET_MS: u64 = 8 * 24 * 60 * 60 * 1000;

pub const HEADER_API_KEY: &str = "X-edgeX-Api-Key";
pub const HEADER_PASSPHRASE: &str = "X-edgeX-Passphrase";
pub const HEADER_SIGNATURE: &str = "X-edgeX-Signature";
pub const HEADER_TIMESTAMP: &str = "X-edgeX-Timestamp";

// ── HMAC request auth ────────────────────────────────────────────────────────

/// API key, passphrase and secret from API Management → Perps V2 → SDK Signer.
#[derive(Clone)]
pub struct ApiCredentials {
    api_key: String,
    api_passphrase: String,
    api_secret: String,
}

impl std::fmt::Debug for ApiCredentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ApiCredentials")
            .field("api_key", &self.api_key)
            .finish_non_exhaustive()
    }
}

impl ApiCredentials {
    pub fn new(api_key: String, api_passphrase: String, api_secret: String) -> Self {
        Self {
            api_key,
            api_passphrase,
            api_secret,
        }
    }

    pub fn api_key(&self) -> &str {
        &self.api_key
    }

    /// `hex(HMAC-SHA256(base64(api_secret), timestamp + method + request_uri + payload))`.
    pub fn signature(
        &self,
        timestamp_ms: u64,
        method: &str,
        request_uri: &str,
        payload: &str,
    ) -> String {
        let key = base64::engine::general_purpose::STANDARD.encode(self.api_secret.as_bytes());
        let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(key.as_bytes())
            .expect("HMAC accepts keys of any length");
        mac.update(timestamp_ms.to_string().as_bytes());
        mac.update(method.as_bytes());
        mac.update(request_uri.as_bytes());
        mac.update(payload.as_bytes());
        hex::encode(mac.finalize().into_bytes())
    }

    /// The four `X-edgeX-*` auth headers.
    pub fn headers(
        &self,
        timestamp_ms: u64,
        method: &str,
        request_uri: &str,
        payload: &str,
    ) -> Vec<(String, String)> {
        vec![
            (HEADER_API_KEY.to_string(), self.api_key.clone()),
            (HEADER_PASSPHRASE.to_string(), self.api_passphrase.clone()),
            (
                HEADER_SIGNATURE.to_string(),
                self.signature(timestamp_ms, method, request_uri, payload),
            ),
            (HEADER_TIMESTAMP.to_string(), timestamp_ms.to_string()),
        ]
    }
}

/// The SDK's `get_value`: the string a POST body is signed as. Objects become `k=v` pairs sorted by
/// key and joined with `&`; arrays join their items with `&`; `null` is empty; booleans are
/// `true`/`false`. Numbers print as serde_json prints them (the SDK only sends strings, booleans,
/// arrays and nulls).
pub fn canonical_value(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::Array(items) => items
            .iter()
            .map(canonical_value)
            .collect::<Vec<_>>()
            .join("&"),
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            keys.iter()
                .map(|k| format!("{k}={}", canonical_value(&map[k.as_str()])))
                .collect::<Vec<_>>()
                .join("&")
        }
    }
}

/// The string a GET query is signed as: `k=v` pairs sorted by key, joined with `&`, values raw.
pub fn canonical_query(params: &[(&str, String)]) -> String {
    let mut sorted: Vec<&(&str, String)> = params.iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(b.0).then_with(|| a.1.cmp(&b.1)));
    sorted
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("&")
}

// ── L2 amounts and nonce ─────────────────────────────────────────────────────

/// `int(sha256(client_order_id).hexdigest()[:8], 16)`: the first 32 bits of the SHA-256 of the
/// client order id. The order docs require `l2Nonce` to be derived this way.
pub fn calc_nonce(client_order_id: &str) -> u64 {
    let h = Sha256::digest(client_order_id.as_bytes());
    u32::from_be_bytes([h[0], h[1], h[2], h[3]]) as u64
}

/// Parses a metadata `resolution` (`"10000000000"` on contracts, `"0xf4240"` on USDC).
pub fn parse_resolution(resolution: &str) -> Result<Decimal> {
    let r = resolution.trim();
    if r.is_empty() {
        bail!("resolution is empty");
    }
    if let Some(hex) = r.strip_prefix("0x").or_else(|| r.strip_prefix("0X")) {
        let v = u128::from_str_radix(hex, 16)
            .with_context(|| format!("invalid hex resolution {resolution}"))?;
        let v = i128::try_from(v).map_err(|_| anyhow!("resolution {resolution} too large"))?;
        return Decimal::try_from_i128_with_scale(v, 0)
            .map_err(|e| anyhow!("resolution {resolution} too large: {e}"));
    }
    Decimal::from_str(r).with_context(|| format!("invalid resolution {resolution}"))
}

/// The L2 quantities the SDK derives for an order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct L2OrderAmounts {
    /// `l2Value` = `l2_price * size`, formatted as Python formats the `Decimal`.
    pub l2_value: String,
    /// `l2LimitFee` = `ceil(l2Value * fee_rate)` (a whole number of quote units).
    pub l2_limit_fee: String,
    /// `int(size * contract_resolution)`.
    pub amount_synthetic: i128,
    /// `int(l2Value * quote_resolution)`.
    pub amount_collateral: i128,
    /// `int(l2LimitFee * quote_resolution)`.
    pub amount_fee: u128,
}

fn dec(s: &str, what: &str) -> Result<Decimal> {
    Decimal::from_str(s.trim()).with_context(|| format!("invalid {what}: {s}"))
}

fn mul(a: Decimal, b: Decimal, what: &str) -> Result<Decimal> {
    a.checked_mul(b)
        .ok_or_else(|| anyhow!("{what} overflows a 96-bit decimal"))
}

/// Python's `str(Decimal)` for a value with a non-positive exponent: plain notation unless the
/// adjusted exponent is below -6, then scientific (`1.5E-7`).
fn py_decimal_str(d: &Decimal) -> String {
    let scale = d.scale() as i64;
    let mantissa = d.mantissa();
    let digits = mantissa.unsigned_abs().to_string();
    let adjusted = -scale + digits.len() as i64 - 1;
    if adjusted >= -6 {
        return d.to_string();
    }
    let sign = if mantissa < 0 { "-" } else { "" };
    let (head, tail) = digits.split_at(1);
    if tail.is_empty() {
        format!("{sign}{head}E{adjusted}")
    } else {
        format!("{sign}{head}.{tail}E{adjusted}")
    }
}

/// The SDK's order arithmetic. `l2_price` is the price bound the signature commits to: the limit
/// price for limit orders; for market orders the SDK uses `ceil(oracle * 10 / tick) * tick` when
/// buying and `tick_size` when selling. `fee_rate` is the maximum fee rate the order may pay; the
/// SDK uses `max(defaultTakerFeeRate, defaultMakerFeeRate)` from the contract metadata.
pub fn l2_order_amounts(
    size: &str,
    l2_price: &str,
    fee_rate: &str,
    contract_resolution: &str,
    quote_resolution: &str,
) -> Result<L2OrderAmounts> {
    let size = dec(size, "size")?;
    let l2_price = dec(l2_price, "l2 price")?;
    let fee_rate = dec(fee_rate, "fee rate")?;
    let contract_res = parse_resolution(contract_resolution)?;
    let quote_res = parse_resolution(quote_resolution)?;

    let l2_value = mul(l2_price, size, "l2 value")?;
    let mut limit_fee = mul(l2_value, fee_rate, "limit fee")?.ceil();
    limit_fee.rescale(0);

    let to_i128 = |d: Decimal, what: &str| -> Result<i128> {
        d.trunc()
            .to_i128()
            .ok_or_else(|| anyhow!("{what} out of range"))
    };
    let amount_synthetic = to_i128(
        mul(size, contract_res, "amountSynthetic")?,
        "amountSynthetic",
    )?;
    let amount_collateral = to_i128(
        mul(l2_value, quote_res, "amountCollateral")?,
        "amountCollateral",
    )?;
    let amount_fee = to_i128(mul(limit_fee, quote_res, "amountFee")?, "amountFee")?;
    let amount_fee =
        u128::try_from(amount_fee).map_err(|_| anyhow!("amountFee is negative: {amount_fee}"))?;

    Ok(L2OrderAmounts {
        l2_value: py_decimal_str(&l2_value),
        l2_limit_fee: limit_fee.to_string(),
        amount_synthetic,
        amount_collateral,
        amount_fee,
    })
}

/// The L2 price bound the SDKs sign for MARKET / STOP_MARKET / TAKE_PROFIT_MARKET orders (the
/// body `price` is `"0"`): buy → `ceil(oracle_price * 10 / tick_size) * tick_size`, sell →
/// `tick_size`. `oracle_price` is `getTicker().oraclePrice`, `tick_size` the contract's.
pub fn market_order_l2_price(
    side: EdgexSide,
    oracle_price: &str,
    tick_size: &str,
) -> Result<String> {
    let tick = dec(tick_size, "tick size")?;
    if tick <= Decimal::ZERO {
        bail!("tick size must be positive: {tick_size}");
    }
    match side {
        EdgexSide::Sell => Ok(tick.to_string()),
        EdgexSide::Buy => {
            let oracle = dec(oracle_price, "oracle price")?;
            if oracle <= Decimal::ZERO {
                bail!("oracle price must be positive: {oracle_price}");
            }
            let steps = mul(oracle, Decimal::TEN, "oracle bound")?
                .checked_div(tick)
                .ok_or_else(|| anyhow!("oracle bound overflows"))?
                .ceil();
            Ok(mul(steps, tick, "oracle bound")?.to_string())
        }
    }
}

// ── EIP-712 ──────────────────────────────────────────────────────────────────

const DOMAIN_TYPE: &[u8] =
    b"EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)";
const LIMIT_ORDER_TYPE: &[u8] = b"LimitOrderParams(OrderBase base,int256 amountSynthetic,int256 amountCollateral,uint256 amountFee,uint64 assetIdSynthetic,uint64 assetIdCollateral,bool isBuyingSynthetic,bytes32 parentOrderHash,uint256 subOrderIndex)OrderBase(uint256 nonce,address signer,uint64 accountId,uint256 expirationTimestamp)";
const ORDER_BASE_TYPE: &[u8] =
    b"OrderBase(uint256 nonce,address signer,uint64 accountId,uint256 expirationTimestamp)";
const MARGIN_MODE_TYPE: &[u8] = b"SetMarginPreferenceParams(uint64 accountId,uint64 assetId,uint8 marginMode,uint256 nonce,address signer)";

pub const DOMAIN_NAME: &str = "EdgeX";
pub const DOMAIN_VERSION: &str = "1";

fn keccak(data: &[u8]) -> [u8; 32] {
    Keccak256::digest(data).into()
}

fn word_u128(v: u128) -> [u8; 32] {
    let mut w = [0u8; 32];
    w[16..].copy_from_slice(&v.to_be_bytes());
    w
}

fn word_i128(v: i128) -> [u8; 32] {
    let mut w = if v < 0 { [0xffu8; 32] } else { [0u8; 32] };
    w[16..].copy_from_slice(&v.to_be_bytes());
    w
}

fn word_address(a: &[u8; 20]) -> [u8; 32] {
    let mut w = [0u8; 32];
    w[12..].copy_from_slice(a);
    w
}

fn parse_address(s: &str) -> Result<[u8; 20]> {
    let h = s.trim();
    let h = h
        .strip_prefix("0x")
        .or_else(|| h.strip_prefix("0X"))
        .unwrap_or(h);
    let bytes = hex::decode(h).with_context(|| format!("invalid address {s}"))?;
    bytes
        .try_into()
        .map_err(|_| anyhow!("address {s} is not 20 bytes"))
}

/// EIP-55 mixed-case form of an address.
pub fn checksum_address(addr: &[u8; 20]) -> String {
    let lower = hex::encode(addr);
    let hash = keccak(lower.as_bytes());
    let mut out = String::with_capacity(42);
    out.push_str("0x");
    for (i, c) in lower.chars().enumerate() {
        let shift = if i % 2 == 0 { 4 } else { 0 };
        let nibble = (hash[i / 2] >> shift) & 0x0f;
        if c.is_ascii_alphabetic() && nibble >= 8 {
            out.push(c.to_ascii_uppercase());
        } else {
            out.push(c);
        }
    }
    out
}

/// Parses a chain id as the SDK does: `0x`-prefixed hex or decimal.
pub fn parse_chain_id(s: &str) -> Result<u64> {
    let s = s.trim();
    match s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        Some(hex) => u64::from_str_radix(hex, 16).with_context(|| format!("invalid chain id {s}")),
        None => s.parse().with_context(|| format!("invalid chain id {s}")),
    }
}

/// The `EdgeX` / `1` EIP-712 domain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct L2Domain {
    pub chain_id: u64,
    pub verifying_contract: [u8; 20],
    separator: [u8; 32],
}

impl L2Domain {
    pub fn new(chain_id: u64, verifying_contract: &str) -> Result<Self> {
        let verifying_contract = parse_address(verifying_contract)?;
        let mut buf = Vec::with_capacity(5 * 32);
        buf.extend_from_slice(&keccak(DOMAIN_TYPE));
        buf.extend_from_slice(&keccak(DOMAIN_NAME.as_bytes()));
        buf.extend_from_slice(&keccak(DOMAIN_VERSION.as_bytes()));
        buf.extend_from_slice(&word_u128(chain_id as u128));
        buf.extend_from_slice(&word_address(&verifying_contract));
        Ok(Self {
            chain_id,
            verifying_contract,
            separator: keccak(&buf),
        })
    }

    /// Domain from `getMetaData().global`: chain id `nativeChainId` (falling back to `chainId` when
    /// empty, as the SDKs do) and verifying contract `contractAddress`.
    pub fn from_global(global: &EdgexGlobal) -> Result<Self> {
        let chain = if global.native_chain_id.trim().is_empty() {
            global.chain_id.as_str()
        } else {
            global.native_chain_id.as_str()
        };
        if global.contract_address.trim().is_empty() {
            bail!("metadata global.contractAddress is empty");
        }
        Self::new(parse_chain_id(chain)?, &global.contract_address)
    }

    pub fn separator(&self) -> [u8; 32] {
        self.separator
    }

    fn digest(&self, struct_hash: &[u8; 32]) -> [u8; 32] {
        let mut m = [0u8; 66];
        m[0] = 0x19;
        m[1] = 0x01;
        m[2..34].copy_from_slice(&self.separator);
        m[34..].copy_from_slice(struct_hash);
        keccak(&m)
    }
}

/// `LimitOrderParams` typed data. `parentOrderHash` is always zero and `subOrderIndex` always 0:
/// `createOrder` has no fields for them and both SDKs sign them this way.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LimitOrderParams {
    pub nonce: u64,
    pub account_id: u64,
    /// Seconds: `expireTime / 1000`.
    pub expiration_timestamp: u64,
    pub amount_synthetic: i128,
    pub amount_collateral: i128,
    pub amount_fee: u128,
    /// The contract id.
    pub asset_id_synthetic: u64,
    /// The quote coin id.
    pub asset_id_collateral: u64,
    pub is_buying_synthetic: bool,
}

/// `SetMarginPreferenceParams` typed data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarginModeParams {
    pub account_id: u64,
    /// The contract id.
    pub asset_id: u64,
    pub margin_mode: EdgexMarginMode,
    pub nonce: u64,
}

/// An order as the caller wants it; [`L2Signer::create_order_body`] adds the L2 fields.
#[derive(Debug, Clone)]
pub struct NewOrder<'a> {
    pub contract_id: &'a str,
    pub side: EdgexSide,
    pub order_type: EdgexOrderType,
    pub time_in_force: EdgexTimeInForce,
    /// Body `price`. The docs require `"0"` for market order types.
    pub price: &'a str,
    pub size: &'a str,
    /// Idempotency key; `l2Nonce` is derived from it.
    pub client_order_id: &'a str,
    /// Body `expireTime` (ms). The signature commits to `expire_time_ms / 1000` and `l2ExpireTime`
    /// is `expire_time_ms + L1_EXPIRE_OFFSET_MS`.
    pub expire_time_ms: u64,
    pub reduce_only: bool,
    /// `triggerPrice` and `triggerPriceType` for the conditional order types.
    pub trigger: Option<(&'a str, EdgexTriggerPriceType)>,
}

/// Inputs to the L2 arithmetic, see [`l2_order_amounts`].
#[derive(Debug, Clone)]
pub struct OrderL2Inputs<'a> {
    pub l2_price: &'a str,
    pub fee_rate: &'a str,
    /// Contract `resolution` from metadata.
    pub contract_resolution: &'a str,
    /// Quote coin `resolution` from metadata.
    pub quote_resolution: &'a str,
    /// Contract `quoteCoinId`.
    pub quote_coin_id: &'a str,
}

/// The trading ("signer") key: a secp256k1 key whose address is registered for the account.
#[derive(Clone)]
pub struct L2Signer {
    key: SigningKey,
    address: [u8; 20],
}

impl std::fmt::Debug for L2Signer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("L2Signer")
            .field("address", &self.address())
            .finish_non_exhaustive()
    }
}

impl L2Signer {
    /// 32-byte hex private key, with or without `0x`.
    pub fn from_hex(private_key: &str) -> Result<Self> {
        let pk = private_key.trim();
        let pk = pk.strip_prefix("0x").unwrap_or(pk);
        let bytes: [u8; 32] = hex::decode(pk)
            .context("trading private key: invalid hex")?
            .try_into()
            .map_err(|_| anyhow!("trading private key: expected 32 bytes"))?;
        let key = SigningKey::from_bytes((&bytes).into())
            .context("trading private key: not a valid secp256k1 scalar")?;
        let point = key.verifying_key().to_encoded_point(false);
        let hash = keccak(&point.as_bytes()[1..]);
        let mut address = [0u8; 20];
        address.copy_from_slice(&hash[12..]);
        Ok(Self { key, address })
    }

    /// EIP-55 address of the key (the `signer` the exchange checks).
    pub fn address(&self) -> String {
        checksum_address(&self.address)
    }

    pub fn limit_order_digest(&self, domain: &L2Domain, p: &LimitOrderParams) -> [u8; 32] {
        let mut base = Vec::with_capacity(5 * 32);
        base.extend_from_slice(&keccak(ORDER_BASE_TYPE));
        base.extend_from_slice(&word_u128(p.nonce as u128));
        base.extend_from_slice(&word_address(&self.address));
        base.extend_from_slice(&word_u128(p.account_id as u128));
        base.extend_from_slice(&word_u128(p.expiration_timestamp as u128));
        let base_hash = keccak(&base);

        let mut s = Vec::with_capacity(10 * 32);
        s.extend_from_slice(&keccak(LIMIT_ORDER_TYPE));
        s.extend_from_slice(&base_hash);
        s.extend_from_slice(&word_i128(p.amount_synthetic));
        s.extend_from_slice(&word_i128(p.amount_collateral));
        s.extend_from_slice(&word_u128(p.amount_fee));
        s.extend_from_slice(&word_u128(p.asset_id_synthetic as u128));
        s.extend_from_slice(&word_u128(p.asset_id_collateral as u128));
        s.extend_from_slice(&word_u128(p.is_buying_synthetic as u128));
        s.extend_from_slice(&[0u8; 32]); // parentOrderHash
        s.extend_from_slice(&[0u8; 32]); // subOrderIndex
        domain.digest(&keccak(&s))
    }

    pub fn margin_mode_digest(&self, domain: &L2Domain, p: &MarginModeParams) -> [u8; 32] {
        let mut s = Vec::with_capacity(6 * 32);
        s.extend_from_slice(&keccak(MARGIN_MODE_TYPE));
        s.extend_from_slice(&word_u128(p.account_id as u128));
        s.extend_from_slice(&word_u128(p.asset_id as u128));
        s.extend_from_slice(&word_u128(p.margin_mode.as_u8() as u128));
        s.extend_from_slice(&word_u128(p.nonce as u128));
        s.extend_from_slice(&word_address(&self.address));
        domain.digest(&keccak(&s))
    }

    /// `r || s || v` (v = 27/28) as 130 hex chars without `0x`. RFC 6979 nonces, low-s, so the
    /// result equals eth-account's.
    pub fn sign_digest(&self, digest: &[u8; 32]) -> Result<String> {
        let (sig, recid) = self
            .key
            .sign_prehash_recoverable(digest)
            .context("signing failed")?;
        let mut out = [0u8; 65];
        out[..64].copy_from_slice(&sig.to_bytes());
        out[64] = 27 + recid.to_byte();
        if out[64] > 28 {
            bail!("unexpected recovery id {}", recid.to_byte());
        }
        Ok(hex::encode(out))
    }

    pub fn sign_limit_order(&self, domain: &L2Domain, p: &LimitOrderParams) -> Result<String> {
        self.sign_digest(&self.limit_order_digest(domain, p))
    }

    pub fn sign_margin_mode(&self, domain: &L2Domain, p: &MarginModeParams) -> Result<String> {
        self.sign_digest(&self.margin_mode_digest(domain, p))
    }

    /// The typed-data message and the full `createOrder` body, as the SDK builds them.
    pub fn create_order_body(
        &self,
        domain: &L2Domain,
        account_id: u64,
        order: &NewOrder<'_>,
        l2: &OrderL2Inputs<'_>,
    ) -> Result<CreateOrderBody> {
        let amounts = l2_order_amounts(
            order.size,
            l2.l2_price,
            l2.fee_rate,
            l2.contract_resolution,
            l2.quote_resolution,
        )?;
        let nonce = calc_nonce(order.client_order_id);
        let params = LimitOrderParams {
            nonce,
            account_id,
            expiration_timestamp: order.expire_time_ms / 1000,
            amount_synthetic: amounts.amount_synthetic,
            amount_collateral: amounts.amount_collateral,
            amount_fee: amounts.amount_fee,
            asset_id_synthetic: order
                .contract_id
                .parse()
                .with_context(|| format!("invalid contract id {}", order.contract_id))?,
            asset_id_collateral: l2
                .quote_coin_id
                .parse()
                .with_context(|| format!("invalid quote coin id {}", l2.quote_coin_id))?,
            is_buying_synthetic: order.side == EdgexSide::Buy,
        };
        let l2_expire_time = order
            .expire_time_ms
            .checked_add(L1_EXPIRE_OFFSET_MS)
            .ok_or_else(|| anyhow!("expire time overflows"))?;
        let (trigger_price, trigger_price_type) = match order.trigger {
            Some((price, kind)) => (price.to_string(), kind.as_str().to_string()),
            None => (String::new(), String::new()),
        };
        Ok(CreateOrderBody {
            account_id: account_id.to_string(),
            contract_id: order.contract_id.to_string(),
            price: order.price.to_string(),
            size: order.size.to_string(),
            order_type: order.order_type,
            side: order.side,
            time_in_force: order.time_in_force,
            client_order_id: order.client_order_id.to_string(),
            expire_time: order.expire_time_ms.to_string(),
            l2_nonce: nonce.to_string(),
            l2_signature: self.sign_limit_order(domain, &params)?,
            l2_expire_time: l2_expire_time.to_string(),
            l2_value: amounts.l2_value,
            l2_size: order.size.to_string(),
            l2_limit_fee: amounts.l2_limit_fee,
            reduce_only: order.reduce_only,
            trigger_price,
            trigger_price_type,
            source_key: String::new(),
            is_position_tpsl: false,
            open_tpsl_parent_order_id: String::new(),
            is_set_open_tp: false,
            open_tp: None,
            is_set_open_sl: false,
            open_sl: None,
            extra_type: String::new(),
            extra_data_json: String::new(),
        })
    }

    /// The `setMarginMode` body. `nonce` is any unique value (the SDK uses `calc_nonce` of a fresh
    /// client id); `l2_expire_time_ms` is not signed (the Go SDK sends the next full hour + 14
    /// days).
    pub fn set_margin_mode_body(
        &self,
        domain: &L2Domain,
        account_id: u64,
        contract_id: &str,
        margin_mode: EdgexMarginMode,
        nonce: u64,
        l2_expire_time_ms: u64,
    ) -> Result<SetMarginModeBody> {
        let params = MarginModeParams {
            account_id,
            asset_id: contract_id
                .parse()
                .with_context(|| format!("invalid contract id {contract_id}"))?,
            margin_mode,
            nonce,
        };
        Ok(SetMarginModeBody {
            account_id: account_id.to_string(),
            contract_id: contract_id.to_string(),
            margin_mode: margin_mode.as_u8().to_string(),
            l2_nonce: nonce.to_string(),
            l2_expire_time: l2_expire_time_ms.to_string(),
            signer: self.address(),
            l2_signature: self.sign_margin_mode(domain, &params)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn python_decimal_formatting() {
        let d = |s: &str| Decimal::from_str(s).unwrap();
        assert_eq!(py_decimal_str(&d("64.9795")), "64.9795");
        assert_eq!(py_decimal_str(&d("0.0000")), "0.0000");
        assert_eq!(py_decimal_str(&d("0.000001")), "0.000001");
        assert_eq!(py_decimal_str(&d("0.0000001")), "1E-7");
        assert_eq!(py_decimal_str(&d("0.00000015")), "1.5E-7");
        assert_eq!(py_decimal_str(&d("-0.00000015")), "-1.5E-7");
    }

    #[test]
    fn canonical_value_matches_sdk_rules() {
        let v = serde_json::json!({"b": [true, "x"], "a": null, "c": {"z": "1", "y": []}});
        assert_eq!(canonical_value(&v), "a=&b=true&x&c=y=&z=1");
    }

    #[test]
    fn resolution_forms() {
        assert_eq!(
            parse_resolution("0xf4240").unwrap(),
            Decimal::from(1_000_000)
        );
        assert_eq!(
            parse_resolution("10000000000").unwrap(),
            Decimal::from(10_000_000_000u64)
        );
        assert!(parse_resolution("").is_err());
    }
}
