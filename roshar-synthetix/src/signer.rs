//! EIP-712 signing for the Synthetix offchain trading API, byte-identical to the official Python
//! SDK (`synthetix-sdk` 0.7.0, `synthetix/signing.py`).
//!
//! digest    = keccak256(0x19 0x01 || domainSeparator || hashStruct(message))
//! domain    = {name "Synthetix", version "1", chainId 1, verifyingContract 0x0}
//! signature = secp256k1 RFC 6979 over the digest, low-s, `v` in {27, 28}
//!
//! Each `sign_*` call returns a [`SignedRequest`]: the `POST /v1/trade` body
//! `{"signature", "nonce", "expiresAfter", "params"}` with `params` in the SDK's key order, and the
//! same request flattened for the trade websocket (`method: "post"`).
//! Nonces and expiries are always passed in; nothing here reads the clock.

use anyhow::{anyhow, bail, Context, Result};
use k256::ecdsa::SigningKey;
use serde::{Serialize, Serializer};
use sha3::{Digest, Keccak256};
use std::sync::LazyLock;

use crate::types::trading::SubAccountQuery;

pub const DOMAIN_NAME: &str = "Synthetix";
pub const DOMAIN_VERSION: &str = "1";
/// EIP-712 `chainId`. Ethereum mainnet; the docs and the SDK use it for every request.
pub const DOMAIN_CHAIN_ID: u64 = 1;

const EIP712_DOMAIN_TYPE: &str =
    "EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)";
const ORDER_TYPE: &str = "Order(string symbol,string side,string orderType,string price,string triggerPrice,string quantity,bool reduceOnly,bool isTriggerMarket,string clientOrderId,bool closePosition)";
const PLACE_ORDERS_TYPE: &str = "PlaceOrders(uint256 subAccountId,Order[] orders,string grouping,uint256 nonce,uint256 expiresAfter)Order(string symbol,string side,string orderType,string price,string triggerPrice,string quantity,bool reduceOnly,bool isTriggerMarket,string clientOrderId,bool closePosition)";
const CANCEL_ORDERS_TYPE: &str =
    "CancelOrders(uint256 subAccountId,uint256[] orderIds,uint256 nonce,uint256 expiresAfter)";
const CANCEL_ORDERS_BY_CLOID_TYPE: &str = "CancelOrdersByCloid(uint256 subAccountId,string[] clientOrderIds,uint256 nonce,uint256 expiresAfter)";
const CANCEL_ALL_ORDERS_TYPE: &str =
    "CancelAllOrders(uint256 subAccountId,string[] symbols,uint256 nonce,uint256 expiresAfter)";
const MODIFY_ORDER_TYPE: &str = "ModifyOrder(uint256 subAccountId,uint256 orderId,string price,string quantity,string triggerPrice,uint256 nonce,uint256 expiresAfter)";
const MODIFY_ORDER_BY_CLOID_TYPE: &str = "ModifyOrderByCloid(uint256 subAccountId,string clientOrderId,string price,string quantity,string triggerPrice,uint256 nonce,uint256 expiresAfter)";
const UPDATE_LEVERAGE_TYPE: &str = "UpdateLeverage(uint256 subAccountId,string symbol,string leverage,uint256 nonce,uint256 expiresAfter)";
const SCHEDULE_CANCEL_TYPE: &str =
    "ScheduleCancel(uint256 subAccountId,uint256 timeoutSeconds,uint256 nonce,uint256 expiresAfter)";
const SUB_ACCOUNT_ACTION_TYPE: &str =
    "SubAccountAction(uint256 subAccountId,string action,uint256 nonce,uint256 expiresAfter)";
const ADD_DELEGATED_SIGNER_TYPE: &str = "AddDelegatedSigner(address delegateAddress,uint256 subAccountId,uint256 nonce,uint256 expiresAfter,uint256 expiresAt,string[] permissions)";
const REMOVE_DELEGATED_SIGNER_TYPE: &str = "RemoveDelegatedSigner(address delegateAddress,uint256 subAccountId,uint256 nonce,uint256 expiresAfter)";
const REMOVE_ALL_DELEGATED_SIGNERS_TYPE: &str =
    "RemoveAllDelegatedSigners(uint256 subAccountId,uint256 nonce,uint256 expiresAfter)";
const AUTH_MESSAGE_TYPE: &str = "AuthMessage(uint256 subAccountId,uint256 timestamp,string action)";

/// The `types` / `primaryType` / `domain` part of the websocket auth payload, as the SDK builds it
/// (`json.dumps` key order, compact separators). The `message` object is appended per call.
const WS_AUTH_PAYLOAD_PREFIX: &str = r#"{"types":{"EIP712Domain":[{"name":"name","type":"string"},{"name":"version","type":"string"},{"name":"chainId","type":"uint256"},{"name":"verifyingContract","type":"address"}],"AuthMessage":[{"name":"subAccountId","type":"uint256"},{"name":"timestamp","type":"uint256"},{"name":"action","type":"string"}]},"primaryType":"AuthMessage","domain":{"name":"Synthetix","version":"1","chainId":1,"verifyingContract":"0x0000000000000000000000000000000000000000"},"message":"#;
const WS_AUTH_ACTION: &str = "websocket_auth";

fn keccak(data: &[u8]) -> [u8; 32] {
    Keccak256::digest(data).into()
}

/// keccak256 of every type string plus the domain separator, computed once.
struct TypeHashes {
    domain_separator: [u8; 32],
    order: [u8; 32],
    place_orders: [u8; 32],
    cancel_orders: [u8; 32],
    cancel_orders_by_cloid: [u8; 32],
    cancel_all_orders: [u8; 32],
    modify_order: [u8; 32],
    modify_order_by_cloid: [u8; 32],
    update_leverage: [u8; 32],
    schedule_cancel: [u8; 32],
    sub_account_action: [u8; 32],
    add_delegated_signer: [u8; 32],
    remove_delegated_signer: [u8; 32],
    remove_all_delegated_signers: [u8; 32],
    auth_message: [u8; 32],
}

static HASHES: LazyLock<TypeHashes> = LazyLock::new(|| {
    let mut dom = Encoder::new(keccak(EIP712_DOMAIN_TYPE.as_bytes()));
    dom.string(DOMAIN_NAME);
    dom.string(DOMAIN_VERSION);
    dom.uint(DOMAIN_CHAIN_ID);
    dom.word([0u8; 32]); // verifyingContract = address(0)
    TypeHashes {
        domain_separator: dom.finish(),
        order: keccak(ORDER_TYPE.as_bytes()),
        place_orders: keccak(PLACE_ORDERS_TYPE.as_bytes()),
        cancel_orders: keccak(CANCEL_ORDERS_TYPE.as_bytes()),
        cancel_orders_by_cloid: keccak(CANCEL_ORDERS_BY_CLOID_TYPE.as_bytes()),
        cancel_all_orders: keccak(CANCEL_ALL_ORDERS_TYPE.as_bytes()),
        modify_order: keccak(MODIFY_ORDER_TYPE.as_bytes()),
        modify_order_by_cloid: keccak(MODIFY_ORDER_BY_CLOID_TYPE.as_bytes()),
        update_leverage: keccak(UPDATE_LEVERAGE_TYPE.as_bytes()),
        schedule_cancel: keccak(SCHEDULE_CANCEL_TYPE.as_bytes()),
        sub_account_action: keccak(SUB_ACCOUNT_ACTION_TYPE.as_bytes()),
        add_delegated_signer: keccak(ADD_DELEGATED_SIGNER_TYPE.as_bytes()),
        remove_delegated_signer: keccak(REMOVE_DELEGATED_SIGNER_TYPE.as_bytes()),
        remove_all_delegated_signers: keccak(REMOVE_ALL_DELEGATED_SIGNERS_TYPE.as_bytes()),
        auth_message: keccak(AUTH_MESSAGE_TYPE.as_bytes()),
    }
});

fn hashes() -> &'static TypeHashes {
    &HASHES
}

/// `encodeData` builder: type hash followed by 32-byte words.
struct Encoder(Vec<u8>);

impl Encoder {
    fn new(type_hash: [u8; 32]) -> Self {
        let mut v = Vec::with_capacity(32 * 8);
        v.extend_from_slice(&type_hash);
        Encoder(v)
    }

    fn word(&mut self, w: [u8; 32]) {
        self.0.extend_from_slice(&w);
    }

    fn uint(&mut self, v: u64) {
        self.word(uint_word(v));
    }

    fn boolean(&mut self, b: bool) {
        self.uint(b as u64);
    }

    fn string(&mut self, s: &str) {
        self.word(keccak(s.as_bytes()));
    }

    fn address(&mut self, a: &[u8; 20]) {
        let mut w = [0u8; 32];
        w[12..].copy_from_slice(a);
        self.word(w);
    }

    fn string_array(&mut self, items: &[&str]) {
        let mut buf = Vec::with_capacity(items.len() * 32);
        for s in items {
            buf.extend_from_slice(&keccak(s.as_bytes()));
        }
        self.word(keccak(&buf));
    }

    fn uint_array(&mut self, items: &[u64]) {
        let mut buf = Vec::with_capacity(items.len() * 32);
        for v in items {
            buf.extend_from_slice(&uint_word(*v));
        }
        self.word(keccak(&buf));
    }

    fn finish(self) -> [u8; 32] {
        keccak(&self.0)
    }
}

fn uint_word(v: u64) -> [u8; 32] {
    let mut w = [0u8; 32];
    w[24..].copy_from_slice(&v.to_be_bytes());
    w
}

fn parse_hex<const N: usize>(s: &str, what: &str) -> Result<[u8; N]> {
    let s = s.trim();
    let s = s
        .strip_prefix("0x")
        .or_else(|| s.strip_prefix("0X"))
        .unwrap_or(s);
    let v = hex::decode(s).map_err(|_| anyhow!("{what}: invalid hex"))?;
    v.try_into()
        .map_err(|_| anyhow!("{what}: expected {N} bytes"))
}

/// EIP-55 checksummed `0x` address, as `eth_account` reports it (and as the SDK sends it in
/// `params.walletAddress`).
fn checksum_address(addr: &[u8; 20]) -> String {
    let lower = hex::encode(addr);
    let h = keccak(lower.as_bytes());
    let mut out = String::with_capacity(42);
    out.push_str("0x");
    for (i, c) in lower.chars().enumerate() {
        let nibble = (h[i / 2] >> if i % 2 == 0 { 4 } else { 0 }) & 0xf;
        if c.is_ascii_alphabetic() && nibble >= 8 {
            out.push(c.to_ascii_uppercase());
        } else {
            out.push(c);
        }
    }
    out
}

// ── Order model ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Side {
    Buy,
    Sell,
}

/// `orderType` values accepted by `placeOrders`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum OrderType {
    LimitGtc,
    LimitIoc,
    LimitAlo,
    LimitGtd,
    Market,
    TriggerSl,
    TriggerTp,
    Twap,
}

/// `grouping` values accepted by `placeOrders`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Grouping {
    Na,
    NormalTpsl,
    PositionTpsl,
    Twap,
}

/// `triggerPriceType` for `triggerSl` / `triggerTp` orders (unsigned request field).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TriggerPriceType {
    Mark,
    Last,
}

impl Side {
    pub fn as_str(self) -> &'static str {
        match self {
            Side::Buy => "buy",
            Side::Sell => "sell",
        }
    }
}

impl OrderType {
    pub fn as_str(self) -> &'static str {
        match self {
            OrderType::LimitGtc => "limitGtc",
            OrderType::LimitIoc => "limitIoc",
            OrderType::LimitAlo => "limitAlo",
            OrderType::LimitGtd => "limitGtd",
            OrderType::Market => "market",
            OrderType::TriggerSl => "triggerSl",
            OrderType::TriggerTp => "triggerTp",
            OrderType::Twap => "twap",
        }
    }
}

impl Grouping {
    pub fn as_str(self) -> &'static str {
        match self {
            Grouping::Na => "na",
            Grouping::NormalTpsl => "normalTpsl",
            Grouping::PositionTpsl => "positionTpsl",
            Grouping::Twap => "twap",
        }
    }
}

/// TWAP-only request fields (not signed).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Twap {
    pub duration_seconds: u64,
    pub interval_seconds: Option<u64>,
    pub interval_variance_bps: Option<u64>,
    pub size_variance_bps: Option<u64>,
}

/// One `placeOrders` item. The first ten fields are signed (the EIP-712 `Order` type); the rest
/// are request-only. Empty strings mean "not applicable", exactly as on the wire: `price` is `""`
/// for market orders, `trigger_price` is `""` for non-trigger orders, `client_order_id` is `""`
/// when unset (the SDK always sends the key).
#[derive(Debug, Clone, Copy)]
pub struct Order<'a> {
    pub symbol: &'a str,
    pub side: Side,
    pub order_type: OrderType,
    pub price: &'a str,
    pub trigger_price: &'a str,
    pub quantity: &'a str,
    pub reduce_only: bool,
    pub is_trigger_market: bool,
    pub client_order_id: &'a str,
    pub close_position: bool,
    /// Sent as `postOnly: true` only when set (SDK behaviour). `limitGtc` / `limitGtd` only.
    pub post_only: bool,
    /// `limitGtd` expiry, Unix seconds (10 s to 24 h ahead).
    pub expires_at: Option<u64>,
    pub trigger_price_type: Option<TriggerPriceType>,
    pub twap: Option<Twap>,
}

/// New values for `modifyOrder`. `""` leaves a field unchanged; at least one must be set.
#[derive(Debug, Clone, Copy, Default)]
pub struct OrderChange<'a> {
    pub price: &'a str,
    pub quantity: &'a str,
    pub trigger_price: &'a str,
}

/// Replay protection for a signed request. `nonce` must be positive and increasing per signer.
/// `expires_after` is a Unix-ms deadline (the SDK uses `nonce + 60_000`); `0` means none.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReqOpts {
    pub nonce: u64,
    pub expires_after: u64,
}

// ── Signed output ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Signature {
    pub v: u8,
    pub r: [u8; 32],
    pub s: [u8; 32],
}

impl Signature {
    /// `{"v":27,"r":"0x…","s":"0x…"}`: the SDK's `format_signature` (r/s zero-padded to 64 hex).
    pub fn to_json(&self) -> String {
        format!(
            r#"{{"v":{},"r":"0x{}","s":"0x{}"}}"#,
            self.v,
            hex::encode(self.r),
            hex::encode(self.s)
        )
    }

    /// `0x` + r || s || v (65 bytes), the websocket auth signature form.
    pub fn to_hex(&self) -> String {
        let mut b = [0u8; 65];
        b[..32].copy_from_slice(&self.r);
        b[32..64].copy_from_slice(&self.s);
        b[64] = self.v;
        format!("0x{}", hex::encode(b))
    }
}

/// A signed `/v1/trade` action.
#[derive(Debug, Clone)]
pub struct SignedRequest {
    action: &'static str,
    params: String,
    nonce: u64,
    expires_after: u64,
    signature: Signature,
    digest: [u8; 32],
}

impl SignedRequest {
    /// The `params.action` value, e.g. `"placeOrders"`.
    pub fn action(&self) -> &'static str {
        self.action
    }

    /// The `params` object as compact JSON.
    pub fn params_json(&self) -> &str {
        &self.params
    }

    pub fn nonce(&self) -> u64 {
        self.nonce
    }

    pub fn expires_after(&self) -> u64 {
        self.expires_after
    }

    pub fn signature(&self) -> &Signature {
        &self.signature
    }

    /// The EIP-712 digest that was signed.
    pub fn digest(&self) -> &[u8; 32] {
        &self.digest
    }

    /// `POST /v1/trade` body: `{"signature","nonce","expiresAfter","params"}`.
    pub fn rest_body(&self) -> String {
        format!(
            r#"{{"signature":{},"nonce":{},"expiresAfter":{},"params":{}}}"#,
            self.signature.to_json(),
            self.nonce,
            self.expires_after,
            self.params
        )
    }

    /// Trade-websocket frame: `{"id","method":"post","params":{<params…>,"signature","nonce","expiresAfter"}}`.
    /// `id` is the caller's request id, echoed back as `id` / `requestId`.
    pub fn ws_post(&self, id: &str) -> String {
        // `params` is always a non-empty JSON object (it holds at least `action`).
        let inner = &self.params[1..self.params.len() - 1];
        format!(
            r#"{{"id":{},"method":"post","params":{{{},"signature":{},"nonce":{},"expiresAfter":{}}}}}"#,
            json_str(id),
            inner,
            self.signature.to_json(),
            self.nonce,
            self.expires_after
        )
    }
}

/// Signed websocket auth payload for `/v1/ws/trade`.
#[derive(Debug, Clone)]
pub struct WsAuth {
    /// JSON-stringified EIP-712 typed data (`params.message`).
    pub message: String,
    /// `0x` + r || s || v.
    pub signature: String,
    pub digest: [u8; 32],
}

impl WsAuth {
    /// `{"id","method":"auth","params":{"message","signature"}}`.
    pub fn ws_message(&self, id: &str) -> String {
        format!(
            r#"{{"id":{},"method":"auth","params":{{"message":{},"signature":{}}}}}"#,
            json_str(id),
            json_str(&self.message),
            json_str(&self.signature)
        )
    }
}

pub(crate) fn json_str(s: &str) -> String {
    serde_json::to_string(s).expect("string serialization cannot fail")
}

// ── Wire params (SDK key order) ─────────────────────────────────────────────

fn ser_u64_str<S: Serializer>(v: &u64, s: S) -> std::result::Result<S::Ok, S::Error> {
    s.collect_str(v)
}

fn ser_u64_slice_str<S: Serializer>(v: &&[u64], s: S) -> std::result::Result<S::Ok, S::Error> {
    use serde::ser::SerializeSeq;
    let mut seq = s.serialize_seq(Some(v.len()))?;
    for id in v.iter() {
        seq.serialize_element(&id.to_string())?;
    }
    seq.end()
}

fn is_empty_str(s: &&str) -> bool {
    s.is_empty()
}

fn is_empty_slice(s: &&[&str]) -> bool {
    s.is_empty()
}

fn is_false(b: &bool) -> bool {
    !*b
}

/// `{action, subaccountId, walletAddress, ...extra}`: the SDK's `_make_request` params.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Params<'a, X: Serialize> {
    action: &'a str,
    #[serde(serialize_with = "ser_u64_str")]
    subaccount_id: u64,
    wallet_address: &'a str,
    #[serde(flatten)]
    extra: X,
}

#[derive(Serialize)]
struct NoExtra {}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WireOrder<'a> {
    symbol: &'a str,
    side: Side,
    order_type: OrderType,
    price: &'a str,
    trigger_price: &'a str,
    quantity: &'a str,
    reduce_only: bool,
    is_trigger_market: bool,
    client_order_id: &'a str,
    close_position: bool,
    #[serde(skip_serializing_if = "is_false")]
    post_only: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    expires_at: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    duration_seconds: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    interval_seconds: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    interval_variance_bps: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    size_variance_bps: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    trigger_price_type: Option<TriggerPriceType>,
}

impl<'a> From<&Order<'a>> for WireOrder<'a> {
    fn from(o: &Order<'a>) -> Self {
        WireOrder {
            symbol: o.symbol,
            side: o.side,
            order_type: o.order_type,
            price: o.price,
            trigger_price: o.trigger_price,
            quantity: o.quantity,
            reduce_only: o.reduce_only,
            is_trigger_market: o.is_trigger_market,
            client_order_id: o.client_order_id,
            close_position: o.close_position,
            post_only: o.post_only,
            expires_at: o.expires_at,
            duration_seconds: o.twap.map(|t| t.duration_seconds),
            interval_seconds: o.twap.and_then(|t| t.interval_seconds),
            interval_variance_bps: o.twap.and_then(|t| t.interval_variance_bps),
            size_variance_bps: o.twap.and_then(|t| t.size_variance_bps),
            trigger_price_type: o.trigger_price_type,
        }
    }
}

#[derive(Serialize)]
struct PlaceOrdersExtra<'a> {
    orders: Vec<WireOrder<'a>>,
    grouping: Grouping,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CancelOrdersExtra<'a> {
    #[serde(serialize_with = "ser_u64_slice_str")]
    order_ids: &'a [u64],
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CancelByCloidExtra<'a> {
    client_order_ids: &'a [&'a str],
}

#[derive(Serialize)]
struct CancelAllExtra<'a> {
    #[serde(skip_serializing_if = "is_empty_slice")]
    symbols: &'a [&'a str],
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ModifyExtra<'a> {
    #[serde(serialize_with = "ser_u64_str")]
    order_id: u64,
    #[serde(skip_serializing_if = "is_empty_str")]
    price: &'a str,
    #[serde(skip_serializing_if = "is_empty_str")]
    quantity: &'a str,
    #[serde(skip_serializing_if = "is_empty_str")]
    trigger_price: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ModifyByCloidExtra<'a> {
    client_order_id: &'a str,
    #[serde(skip_serializing_if = "is_empty_str")]
    price: &'a str,
    #[serde(skip_serializing_if = "is_empty_str")]
    quantity: &'a str,
    #[serde(skip_serializing_if = "is_empty_str")]
    trigger_price: &'a str,
}

#[derive(Serialize)]
struct LeverageExtra<'a> {
    symbol: &'a str,
    leverage: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ScheduleCancelExtra {
    timeout_seconds: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AddDelegateExtra<'a> {
    expires_at: u64,
    #[serde(skip_serializing_if = "is_empty_slice")]
    permissions: &'a [&'a str],
}

// ── Signer ──────────────────────────────────────────────────────────────────

pub struct Signer {
    key: SigningKey,
    address: String,
}

impl std::fmt::Debug for Signer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never print the key.
        f.debug_struct("Signer")
            .field("address", &self.address)
            .finish_non_exhaustive()
    }
}

impl Signer {
    /// 32-byte secp256k1 private key in hex, with or without `0x`. This is the subaccount owner's
    /// wallet key or a delegated signer's key.
    pub fn from_hex(private_key_hex: &str) -> Result<Self> {
        let kb: [u8; 32] = parse_hex(private_key_hex, "private key")?;
        let key = SigningKey::from_bytes((&kb).into())
            .context("private key: not a valid secp256k1 scalar")?;
        let pt = key.verifying_key().to_encoded_point(false);
        let h = keccak(&pt.as_bytes()[1..]);
        let mut addr = [0u8; 20];
        addr.copy_from_slice(&h[12..]);
        Ok(Signer {
            key,
            address: checksum_address(&addr),
        })
    }

    /// EIP-55 checksummed address of the signing key.
    pub fn address(&self) -> &str {
        &self.address
    }

    fn sign_digest(&self, struct_hash: [u8; 32]) -> Result<([u8; 32], Signature)> {
        let mut m = [0u8; 66];
        m[0] = 0x19;
        m[1] = 0x01;
        m[2..34].copy_from_slice(&hashes().domain_separator);
        m[34..].copy_from_slice(&struct_hash);
        let digest = keccak(&m);
        let (sig, recid) = self
            .key
            .sign_prehash_recoverable(&digest)
            .context("signing failed")?;
        let bytes = sig.to_bytes();
        let v = 27 + recid.to_byte();
        if v > 28 {
            bail!("unexpected recovery id {v}");
        }
        let mut r = [0u8; 32];
        let mut s = [0u8; 32];
        r.copy_from_slice(&bytes[..32]);
        s.copy_from_slice(&bytes[32..]);
        Ok((digest, Signature { v, r, s }))
    }

    fn finish<X: Serialize>(
        &self,
        action: &'static str,
        subaccount_id: u64,
        wallet_address: &str,
        extra: X,
        struct_hash: [u8; 32],
        opts: ReqOpts,
    ) -> Result<SignedRequest> {
        let params = serde_json::to_string(&Params {
            action,
            subaccount_id,
            wallet_address,
            extra,
        })
        .context("serializing params")?;
        let (digest, signature) = self.sign_digest(struct_hash)?;
        Ok(SignedRequest {
            action,
            params,
            nonce: opts.nonce,
            expires_after: opts.expires_after,
            signature,
            digest,
        })
    }

    /// `placeOrders`. Signs `PlaceOrders{subAccountId, orders, grouping, nonce, expiresAfter}`.
    pub fn sign_place_orders(
        &self,
        subaccount_id: u64,
        orders: &[Order<'_>],
        grouping: Grouping,
        opts: ReqOpts,
    ) -> Result<SignedRequest> {
        let h = hashes();
        let mut order_hashes = Vec::with_capacity(orders.len() * 32);
        for o in orders {
            let mut e = Encoder::new(h.order);
            e.string(o.symbol);
            e.string(o.side.as_str());
            e.string(o.order_type.as_str());
            e.string(o.price);
            e.string(o.trigger_price);
            e.string(o.quantity);
            e.boolean(o.reduce_only);
            e.boolean(o.is_trigger_market);
            e.string(o.client_order_id);
            e.boolean(o.close_position);
            order_hashes.extend_from_slice(&e.finish());
        }
        let mut e = Encoder::new(h.place_orders);
        e.uint(subaccount_id);
        e.word(keccak(&order_hashes));
        e.string(grouping.as_str());
        e.uint(opts.nonce);
        e.uint(opts.expires_after);
        let extra = PlaceOrdersExtra {
            orders: orders.iter().map(WireOrder::from).collect(),
            grouping,
        };
        self.finish(
            "placeOrders",
            subaccount_id,
            &self.address,
            extra,
            e.finish(),
            opts,
        )
    }

    /// `cancelOrders` by venue order id. Signs `CancelOrders{…, orderIds uint256[]}`; the ids go
    /// on the wire as strings.
    pub fn sign_cancel_orders(
        &self,
        subaccount_id: u64,
        order_ids: &[u64],
        opts: ReqOpts,
    ) -> Result<SignedRequest> {
        let mut e = Encoder::new(hashes().cancel_orders);
        e.uint(subaccount_id);
        e.uint_array(order_ids);
        e.uint(opts.nonce);
        e.uint(opts.expires_after);
        self.finish(
            "cancelOrders",
            subaccount_id,
            &self.address,
            CancelOrdersExtra { order_ids },
            e.finish(),
            opts,
        )
    }

    /// `cancelOrders` by client order id. Signs `CancelOrdersByCloid`.
    pub fn sign_cancel_orders_by_cloid(
        &self,
        subaccount_id: u64,
        client_order_ids: &[&str],
        opts: ReqOpts,
    ) -> Result<SignedRequest> {
        let mut e = Encoder::new(hashes().cancel_orders_by_cloid);
        e.uint(subaccount_id);
        e.string_array(client_order_ids);
        e.uint(opts.nonce);
        e.uint(opts.expires_after);
        self.finish(
            "cancelOrders",
            subaccount_id,
            &self.address,
            CancelByCloidExtra { client_order_ids },
            e.finish(),
            opts,
        )
    }

    /// `cancelAllOrders`. `["*"]` cancels every market; the exchange rejects `[]` and `"*"`
    /// mixed with symbols. An empty slice omits `symbols` from `params` (SDK behaviour).
    pub fn sign_cancel_all_orders(
        &self,
        subaccount_id: u64,
        symbols: &[&str],
        opts: ReqOpts,
    ) -> Result<SignedRequest> {
        let mut e = Encoder::new(hashes().cancel_all_orders);
        e.uint(subaccount_id);
        e.string_array(symbols);
        e.uint(opts.nonce);
        e.uint(opts.expires_after);
        self.finish(
            "cancelAllOrders",
            subaccount_id,
            &self.address,
            CancelAllExtra { symbols },
            e.finish(),
            opts,
        )
    }

    /// `modifyOrder` by venue order id. Unset (`""`) fields are signed as `""` and left out of
    /// `params`.
    pub fn sign_modify_order(
        &self,
        subaccount_id: u64,
        order_id: u64,
        change: &OrderChange<'_>,
        opts: ReqOpts,
    ) -> Result<SignedRequest> {
        let mut e = Encoder::new(hashes().modify_order);
        e.uint(subaccount_id);
        e.uint(order_id);
        e.string(change.price);
        e.string(change.quantity);
        e.string(change.trigger_price);
        e.uint(opts.nonce);
        e.uint(opts.expires_after);
        let extra = ModifyExtra {
            order_id,
            price: change.price,
            quantity: change.quantity,
            trigger_price: change.trigger_price,
        };
        self.finish(
            "modifyOrder",
            subaccount_id,
            &self.address,
            extra,
            e.finish(),
            opts,
        )
    }

    /// `modifyOrder` by client order id. Signs `ModifyOrderByCloid`; the wire action stays
    /// `modifyOrder`.
    pub fn sign_modify_order_by_cloid(
        &self,
        subaccount_id: u64,
        client_order_id: &str,
        change: &OrderChange<'_>,
        opts: ReqOpts,
    ) -> Result<SignedRequest> {
        let mut e = Encoder::new(hashes().modify_order_by_cloid);
        e.uint(subaccount_id);
        e.string(client_order_id);
        e.string(change.price);
        e.string(change.quantity);
        e.string(change.trigger_price);
        e.uint(opts.nonce);
        e.uint(opts.expires_after);
        let extra = ModifyByCloidExtra {
            client_order_id,
            price: change.price,
            quantity: change.quantity,
            trigger_price: change.trigger_price,
        };
        self.finish(
            "modifyOrder",
            subaccount_id,
            &self.address,
            extra,
            e.finish(),
            opts,
        )
    }

    /// `updateLeverage`. `leverage` is a decimal string, e.g. `"10"`.
    pub fn sign_update_leverage(
        &self,
        subaccount_id: u64,
        symbol: &str,
        leverage: &str,
        opts: ReqOpts,
    ) -> Result<SignedRequest> {
        let mut e = Encoder::new(hashes().update_leverage);
        e.uint(subaccount_id);
        e.string(symbol);
        e.string(leverage);
        e.uint(opts.nonce);
        e.uint(opts.expires_after);
        self.finish(
            "updateLeverage",
            subaccount_id,
            &self.address,
            LeverageExtra { symbol, leverage },
            e.finish(),
            opts,
        )
    }

    /// `scheduleCancel` (dead-man switch). `timeout_seconds` 5..=86400 arms it, 0 disarms it.
    pub fn sign_schedule_cancel(
        &self,
        subaccount_id: u64,
        timeout_seconds: u64,
        opts: ReqOpts,
    ) -> Result<SignedRequest> {
        let mut e = Encoder::new(hashes().schedule_cancel);
        e.uint(subaccount_id);
        e.uint(timeout_seconds);
        e.uint(opts.nonce);
        e.uint(opts.expires_after);
        self.finish(
            "scheduleCancel",
            subaccount_id,
            &self.address,
            ScheduleCancelExtra { timeout_seconds },
            e.finish(),
            opts,
        )
    }

    /// A read-only subaccount query (`getOpenOrders`, `getPositions`, …). Signs
    /// `SubAccountAction{subAccountId, action, nonce, expiresAfter}`, the form the SDK uses.
    pub fn sign_query<Q: SubAccountQuery>(
        &self,
        subaccount_id: u64,
        query: &Q,
        opts: ReqOpts,
    ) -> Result<SignedRequest> {
        let mut e = Encoder::new(hashes().sub_account_action);
        e.uint(subaccount_id);
        e.string(Q::ACTION);
        e.uint(opts.nonce);
        e.uint(opts.expires_after);
        self.finish(
            Q::ACTION,
            subaccount_id,
            &self.address,
            query,
            e.finish(),
            opts,
        )
    }

    /// `addDelegatedSigner`. `delegate` is the delegate's address (sent as `params.walletAddress`,
    /// signed as `delegateAddress`). `permissions` is `["session"]` or `["delegate"]`;
    /// `expires_at` is Unix ms, `0` for no expiry.
    pub fn sign_add_delegated_signer(
        &self,
        subaccount_id: u64,
        delegate: &str,
        permissions: &[&str],
        expires_at: u64,
        opts: ReqOpts,
    ) -> Result<SignedRequest> {
        let addr: [u8; 20] = parse_hex(delegate, "delegate address")?;
        let mut e = Encoder::new(hashes().add_delegated_signer);
        e.address(&addr);
        e.uint(subaccount_id);
        e.uint(opts.nonce);
        e.uint(opts.expires_after);
        e.uint(expires_at);
        e.string_array(permissions);
        self.finish(
            "addDelegatedSigner",
            subaccount_id,
            delegate,
            AddDelegateExtra {
                expires_at,
                permissions,
            },
            e.finish(),
            opts,
        )
    }

    /// `removeDelegatedSigner`.
    pub fn sign_remove_delegated_signer(
        &self,
        subaccount_id: u64,
        delegate: &str,
        opts: ReqOpts,
    ) -> Result<SignedRequest> {
        let addr: [u8; 20] = parse_hex(delegate, "delegate address")?;
        let mut e = Encoder::new(hashes().remove_delegated_signer);
        e.address(&addr);
        e.uint(subaccount_id);
        e.uint(opts.nonce);
        e.uint(opts.expires_after);
        self.finish(
            "removeDelegatedSigner",
            subaccount_id,
            delegate,
            NoExtra {},
            e.finish(),
            opts,
        )
    }

    /// `removeAllDelegatedSigners`.
    pub fn sign_remove_all_delegated_signers(
        &self,
        subaccount_id: u64,
        opts: ReqOpts,
    ) -> Result<SignedRequest> {
        let mut e = Encoder::new(hashes().remove_all_delegated_signers);
        e.uint(subaccount_id);
        e.uint(opts.nonce);
        e.uint(opts.expires_after);
        self.finish(
            "removeAllDelegatedSigners",
            subaccount_id,
            &self.address,
            NoExtra {},
            e.finish(),
            opts,
        )
    }

    /// Websocket auth for `/v1/ws/trade`. Signs `AuthMessage{subAccountId, timestamp,
    /// "websocket_auth"}`. The docs ask for Unix seconds within ±60 s of server time; the SDK
    /// sends Unix ms. The value is signed as given.
    pub fn sign_ws_auth(&self, subaccount_id: u64, timestamp: u64) -> Result<WsAuth> {
        let mut e = Encoder::new(hashes().auth_message);
        e.uint(subaccount_id);
        e.uint(timestamp);
        e.string(WS_AUTH_ACTION);
        let (digest, sig) = self.sign_digest(e.finish())?;
        let message = format!(
            r#"{WS_AUTH_PAYLOAD_PREFIX}{{"subAccountId":"0x{subaccount_id:x}","timestamp":"0x{timestamp:x}","action":"{WS_AUTH_ACTION}"}}}}"#
        );
        Ok(WsAuth {
            message,
            signature: sig.to_hex(),
            digest,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checksum_matches_eip55_examples() {
        // From the EIP-55 specification.
        for a in [
            "0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed",
            "0xfB6916095ca1df60bB79Ce92cE3Ea74c37c5d359",
            "0xdbF03B407c01E7cD3CBea99509d93f8DDDC8C6FB",
            "0xD1220A0cf47c7B9Be7A2E6BA89F429762e7b9aDb",
        ] {
            let b: [u8; 20] = parse_hex(a, "a").unwrap();
            assert_eq!(checksum_address(&b), a);
        }
    }

    #[test]
    fn ws_post_flattens_params() {
        let req = SignedRequest {
            action: "cancelAllOrders",
            params: r#"{"action":"cancelAllOrders","subaccountId":"1"}"#.to_string(),
            nonce: 5,
            expires_after: 6,
            signature: Signature {
                v: 27,
                r: [0; 32],
                s: [1; 32],
            },
            digest: [0; 32],
        };
        let v: serde_json::Value = serde_json::from_str(&req.ws_post("a\"b")).unwrap();
        assert_eq!(v["id"], "a\"b");
        assert_eq!(v["method"], "post");
        assert_eq!(v["params"]["action"], "cancelAllOrders");
        assert_eq!(v["params"]["nonce"], 5);
        assert_eq!(v["params"]["expiresAfter"], 6);
        assert_eq!(v["params"]["signature"]["v"], 27);
    }
}
