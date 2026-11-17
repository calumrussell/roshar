//! EIP-712 signing of Nado executes, byte-identical to the official Python SDK
//! (`nado-protocol` 2.0.0, `eth_account` RFC 6979 signatures).
//!
//! Domain: `{name: "Nado", version: "0.0.1", chainId, verifyingContract}`. The verifying contract
//! is `address(product_id)` for `Order` and the endpoint contract (from the `contracts` query)
//! for every other struct. Signatures are `r || s || v` with `v` in {27, 28}, hex with `0x`.
//!
//! The signing key is either the subaccount owner's wallet or the subaccount's linked signer;
//! the payloads are the same either way.

use anyhow::{anyhow, bail, Context, Result};
use k256::ecdsa::SigningKey;
use serde_json::{json, Map, Value};
use sha3::{Digest, Keccak256};

const DOMAIN_TYPE: &[u8] =
    b"EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)";
const ORDER_TYPE: &[u8] =
    b"Order(bytes32 sender,int128 priceX18,int128 amount,uint64 expiration,uint64 nonce,uint128 appendix)";
const CANCELLATION_TYPE: &[u8] =
    b"Cancellation(bytes32 sender,uint32[] productIds,bytes32[] digests,uint64 nonce)";
const CANCELLATION_PRODUCTS_TYPE: &[u8] =
    b"CancellationProducts(bytes32 sender,uint32[] productIds,uint64 nonce)";
const LINK_SIGNER_TYPE: &[u8] = b"LinkSigner(bytes32 sender,bytes32 signer,uint64 nonce)";
const STREAM_AUTH_TYPE: &[u8] = b"StreamAuthentication(bytes32 sender,uint64 expiration)";

/// Order appendix layout version (bits 7..0).
pub const APPENDIX_VERSION: u8 = 1;

fn keccak(data: &[u8]) -> [u8; 32] {
    Keccak256::digest(data).into()
}

fn word_u64(v: u64) -> [u8; 32] {
    let mut w = [0u8; 32];
    w[24..].copy_from_slice(&v.to_be_bytes());
    w
}

fn word_u128(v: u128) -> [u8; 32] {
    let mut w = [0u8; 32];
    w[16..].copy_from_slice(&v.to_be_bytes());
    w
}

/// Two's complement, sign-extended to 256 bits.
fn word_i128(v: i128) -> [u8; 32] {
    let mut w = [if v < 0 { 0xff } else { 0 }; 32];
    w[16..].copy_from_slice(&v.to_be_bytes());
    w
}

fn hex0x(bytes: &[u8]) -> String {
    format!("0x{}", hex::encode(bytes))
}

fn parse_hex_bytes<const N: usize>(s: &str, what: &str) -> Result<[u8; N]> {
    let s = s.trim();
    let s = s
        .strip_prefix("0x")
        .or_else(|| s.strip_prefix("0X"))
        .unwrap_or(s);
    let v = hex::decode(s).map_err(|_| anyhow!("{what}: invalid hex"))?;
    v.try_into()
        .map_err(|_| anyhow!("{what}: expected {N} bytes"))
}

/// `address(product_id)`: the `place_order` verifying contract.
pub fn order_verifying_contract(product_id: u32) -> [u8; 20] {
    let mut a = [0u8; 20];
    a[16..].copy_from_slice(&product_id.to_be_bytes());
    a
}

fn domain_separator(chain_id: u64, verifying_contract: &[u8; 20]) -> [u8; 32] {
    let mut buf = [0u8; 160];
    buf[..32].copy_from_slice(&keccak(DOMAIN_TYPE));
    buf[32..64].copy_from_slice(&keccak(b"Nado"));
    buf[64..96].copy_from_slice(&keccak(b"0.0.1"));
    buf[96..128].copy_from_slice(&word_u64(chain_id));
    buf[140..].copy_from_slice(verifying_contract);
    keccak(&buf)
}

fn typed_digest(domain: &[u8; 32], struct_hash: &[u8; 32]) -> [u8; 32] {
    let mut m = [0u8; 66];
    m[0] = 0x19;
    m[1] = 0x01;
    m[2..34].copy_from_slice(domain);
    m[34..].copy_from_slice(struct_hash);
    keccak(&m)
}

/// Order / cancellation nonce: `recv_time_ms << 20 | random`, where the engine ignores the
/// request after `recv_time_ms` and `random` (low 20 bits) separates otherwise identical orders.
/// Cancellations are only accepted while `now < recv_time <= now + 100 s`.
pub fn order_nonce(recv_time_ms: u64, random: u32) -> Result<u64> {
    if recv_time_ms >= 1 << 44 {
        bail!("recv_time_ms {recv_time_ms} does not fit 44 bits");
    }
    if random >= 1 << 20 {
        bail!("random {random} does not fit 20 bits");
    }
    Ok((recv_time_ms << 20) | random as u64)
}

/// A subaccount (`sender`): 20-byte owner address followed by a 12-byte name.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Subaccount([u8; 32]);

impl std::fmt::Debug for Subaccount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Subaccount({})", self.to_hex())
    }
}

impl Subaccount {
    /// Owner address (hex, `0x` optional) plus a name of at most 12 bytes, zero-padded (the
    /// default subaccount is named `default`).
    pub fn new(address: &str, name: &str) -> Result<Self> {
        let addr: [u8; 20] = parse_hex_bytes(address, "subaccount address")?;
        let name = name.as_bytes();
        if name.len() > 12 {
            bail!("subaccount name is {} bytes; at most 12", name.len());
        }
        let mut b = [0u8; 32];
        b[..20].copy_from_slice(&addr);
        b[20..20 + name.len()].copy_from_slice(name);
        Ok(Self(b))
    }

    /// A full bytes32 (hex, `0x` optional).
    pub fn from_hex(s: &str) -> Result<Self> {
        Ok(Self(parse_hex_bytes(s, "subaccount")?))
    }

    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Lower-case `0x` + 64 hex digits, as sent on the wire.
    pub fn to_hex(&self) -> String {
        hex0x(&self.0)
    }

    /// Lower-case `0x` owner address.
    pub fn address_hex(&self) -> String {
        hex0x(&self.0[..20])
    }
}

/// Parses a 32-byte order digest (hex, `0x` optional).
pub fn parse_digest(s: &str) -> Result<[u8; 32]> {
    parse_hex_bytes(s, "digest")
}

/// Appendix order type (bits 10..9).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderType {
    Default = 0,
    Ioc = 1,
    Fok = 2,
    PostOnly = 3,
}

/// Appendix trigger type (bits 13..12). Non-`None` values are for the trigger service.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerType {
    None = 0,
    Price = 1,
    Twap = 2,
    TwapCustomAmounts = 3,
}

/// The 128-bit order appendix:
/// `value (127..64) | builder id (63..48) | builder fee rate (47..38) | reserved (37..14) |
/// trigger (13..12) | reduce only (11) | order type (10..9) | isolated (8) | version (7..0)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Appendix {
    pub order_type: OrderType,
    pub isolated: bool,
    pub reduce_only: bool,
    pub trigger: TriggerType,
    /// Builder id; 0 with fee rate 0 means no builder.
    pub builder_id: u16,
    /// 10 bits, units of 0.1 bp.
    pub builder_fee_rate: u16,
    /// Isolated orders: margin (x6) moved into the isolated subaccount on the first match.
    /// TWAP triggers: `times << 32 | slippage_x6`. Otherwise 0.
    pub value: u64,
}

impl Appendix {
    /// Cross margin, not reduce-only, no trigger, no builder.
    pub fn new(order_type: OrderType) -> Self {
        Self {
            order_type,
            isolated: false,
            reduce_only: false,
            trigger: TriggerType::None,
            builder_id: 0,
            builder_fee_rate: 0,
            value: 0,
        }
    }

    /// Packs the appendix, rejecting what the SDK's `build_appendix` rejects.
    pub fn encode(&self) -> Result<u128> {
        let twap = matches!(
            self.trigger,
            TriggerType::Twap | TriggerType::TwapCustomAmounts
        );
        if self.isolated && twap {
            bail!("an order cannot be both isolated and a TWAP order");
        }
        if self.value != 0 && !self.isolated && !twap {
            bail!("appendix value is only used by isolated and TWAP orders");
        }
        if self.builder_fee_rate >= 1 << 10 {
            bail!(
                "builder fee rate {} does not fit 10 bits",
                self.builder_fee_rate
            );
        }
        Ok(APPENDIX_VERSION as u128
            | (self.isolated as u128) << 8
            | (self.order_type as u128) << 9
            | (self.reduce_only as u128) << 11
            | (self.trigger as u128) << 12
            | (self.builder_fee_rate as u128) << 38
            | (self.builder_id as u128) << 48
            | (self.value as u128) << 64)
    }

    /// Unpacks an appendix (e.g. from an order reply). Rejects other versions and set reserved
    /// bits.
    pub fn decode(v: u128) -> Result<Self> {
        let version = (v & 0xff) as u8;
        if version != APPENDIX_VERSION {
            bail!("unsupported appendix version {version}");
        }
        if (v >> 14) & ((1 << 24) - 1) != 0 {
            bail!("appendix reserved bits are set: {v}");
        }
        let order_type = match (v >> 9) & 3 {
            0 => OrderType::Default,
            1 => OrderType::Ioc,
            2 => OrderType::Fok,
            _ => OrderType::PostOnly,
        };
        let trigger = match (v >> 12) & 3 {
            0 => TriggerType::None,
            1 => TriggerType::Price,
            2 => TriggerType::Twap,
            _ => TriggerType::TwapCustomAmounts,
        };
        Ok(Self {
            order_type,
            isolated: (v >> 8) & 1 == 1,
            reduce_only: (v >> 11) & 1 == 1,
            trigger,
            builder_id: ((v >> 48) & 0xffff) as u16,
            builder_fee_rate: ((v >> 38) & 0x3ff) as u16,
            value: (v >> 64) as u64,
        })
    }

    /// TWAP `value`: `times << 32 | slippage_x6`.
    pub fn twap_value(times: u32, slippage_x6: u32) -> u64 {
        (times as u64) << 32 | slippage_x6 as u64
    }
}

/// The signed `Order` struct. Prices and amounts are x18; `amount > 0` buys, `< 0` sells.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Order {
    pub sender: Subaccount,
    pub price_x18: i128,
    pub amount: i128,
    /// Unix seconds after which the order is cancelled.
    pub expiration: u64,
    /// See [`order_nonce`].
    pub nonce: u64,
    /// See [`Appendix::encode`].
    pub appendix: u128,
}

/// A `place_order` request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaceOrder {
    pub product_id: u32,
    pub order: Order,
    /// Client id echoed in `fill` / `order_update` events. Not part of the digest.
    pub id: Option<u64>,
    /// `false` rejects the order if it would borrow. Server default `true`.
    pub spot_leverage: Option<bool>,
}

/// A `cancel_orders` request. `product_ids[i]` is the product of `digests[i]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CancelOrders {
    pub sender: Subaccount,
    pub product_ids: Vec<u32>,
    pub digests: Vec<[u8; 32]>,
    /// See [`order_nonce`].
    pub nonce: u64,
    /// Only cancel if the order's remaining size equals `|required_unfilled_amount|` (x18).
    /// Single-order cancels only. Not signed.
    pub required_unfilled_amount: Option<i128>,
    /// Client id echoed in the reply. Not signed.
    pub id: Option<u64>,
}

/// A `cancel_product_orders` request: cancels every order on `product_ids`, or on every product
/// when empty.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CancelProductOrders {
    pub sender: Subaccount,
    pub product_ids: Vec<u32>,
    /// See [`order_nonce`].
    pub nonce: u64,
    /// Client id echoed in the reply. Not signed.
    pub id: Option<u64>,
}

/// A `link_signer` request. `signer`'s first 20 bytes are the linked address (all zero revokes);
/// `nonce` is the `tx_nonce` from the `nonces` query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkSigner {
    pub sender: Subaccount,
    pub signer: Subaccount,
    pub nonce: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecuteKind {
    PlaceOrder,
    PlaceOrders,
    CancelOrders,
    CancelProductOrders,
    CancelAndPlace,
    LinkSigner,
}

/// A signed execute, ready for `POST /execute` or the gateway websocket.
#[derive(Debug, Clone)]
pub struct SignedExecute {
    pub kind: ExecuteKind,
    /// EIP-712 digests of the signed structs, `0x` hex, in payload order. For orders these are
    /// the order digests the exchange uses as order ids. `CancelAndPlace`: `[cancellation,
    /// order]`.
    pub digests: Vec<String>,
    /// The request body.
    pub payload: Value,
}

impl SignedExecute {
    /// The request body as a JSON string (gateway websocket frame).
    pub fn to_json(&self) -> String {
        self.payload.to_string()
    }
}

pub struct Signer {
    key: SigningKey,
    address: String,
    chain_id: u64,
    /// Domain separator with the endpoint contract as verifying contract.
    endpoint_domain: [u8; 32],
    order_type: [u8; 32],
    cancellation_type: [u8; 32],
    cancellation_products_type: [u8; 32],
    link_signer_type: [u8; 32],
    stream_auth_type: [u8; 32],
}

impl std::fmt::Debug for Signer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never print the key.
        f.debug_struct("Signer")
            .field("address", &self.address)
            .field("chain_id", &self.chain_id)
            .finish_non_exhaustive()
    }
}

impl Signer {
    /// `chain_id` and `endpoint_addr` come from the `contracts` query (mainnet: 57073 and
    /// `0x05ec92d78ed421f3d3ada77ffde167106565974e` on 2026-11-15).
    pub fn from_hex(private_key_hex: &str, chain_id: u64, endpoint_addr: &str) -> Result<Self> {
        let kb: [u8; 32] = parse_hex_bytes(private_key_hex, "private key")?;
        let key = SigningKey::from_bytes((&kb).into())
            .context("private key: not a valid secp256k1 scalar")?;
        let endpoint: [u8; 20] = parse_hex_bytes(endpoint_addr, "endpoint address")?;
        let pt = key.verifying_key().to_encoded_point(false);
        let address = hex0x(&keccak(&pt.as_bytes()[1..])[12..]);
        Ok(Self {
            key,
            address,
            chain_id,
            endpoint_domain: domain_separator(chain_id, &endpoint),
            order_type: keccak(ORDER_TYPE),
            cancellation_type: keccak(CANCELLATION_TYPE),
            cancellation_products_type: keccak(CANCELLATION_PRODUCTS_TYPE),
            link_signer_type: keccak(LINK_SIGNER_TYPE),
            stream_auth_type: keccak(STREAM_AUTH_TYPE),
        })
    }

    /// Lower-case `0x` address of the signing key.
    pub fn address(&self) -> &str {
        &self.address
    }

    pub fn chain_id(&self) -> u64 {
        self.chain_id
    }

    fn sign_digest(&self, digest: &[u8; 32]) -> Result<String> {
        let (sig, recid) = self
            .key
            .sign_prehash_recoverable(digest)
            .context("signing failed")?;
        let mut out = [0u8; 65];
        out[..64].copy_from_slice(&sig.to_bytes());
        out[64] = 27 + recid.to_byte();
        Ok(hex0x(&out))
    }

    /// EIP-712 digest of `order` on `product_id` (the order's id on the exchange).
    pub fn order_digest(&self, product_id: u32, order: &Order) -> [u8; 32] {
        let mut buf = [0u8; 224];
        buf[..32].copy_from_slice(&self.order_type);
        buf[32..64].copy_from_slice(order.sender.as_bytes());
        buf[64..96].copy_from_slice(&word_i128(order.price_x18));
        buf[96..128].copy_from_slice(&word_i128(order.amount));
        buf[128..160].copy_from_slice(&word_u64(order.expiration));
        buf[160..192].copy_from_slice(&word_u64(order.nonce));
        buf[192..].copy_from_slice(&word_u128(order.appendix));
        let domain = domain_separator(self.chain_id, &order_verifying_contract(product_id));
        typed_digest(&domain, &keccak(&buf))
    }

    /// EIP-712 digest of a `Cancellation`.
    pub fn cancellation_digest(&self, c: &CancelOrders) -> [u8; 32] {
        let mut ids = Vec::with_capacity(c.product_ids.len() * 32);
        for id in &c.product_ids {
            ids.extend_from_slice(&word_u64(*id as u64));
        }
        let mut digests = Vec::with_capacity(c.digests.len() * 32);
        for d in &c.digests {
            digests.extend_from_slice(d);
        }
        let mut buf = [0u8; 160];
        buf[..32].copy_from_slice(&self.cancellation_type);
        buf[32..64].copy_from_slice(c.sender.as_bytes());
        buf[64..96].copy_from_slice(&keccak(&ids));
        buf[96..128].copy_from_slice(&keccak(&digests));
        buf[128..].copy_from_slice(&word_u64(c.nonce));
        typed_digest(&self.endpoint_domain, &keccak(&buf))
    }

    /// EIP-712 digest of a `CancellationProducts`.
    pub fn cancellation_products_digest(&self, c: &CancelProductOrders) -> [u8; 32] {
        let mut ids = Vec::with_capacity(c.product_ids.len() * 32);
        for id in &c.product_ids {
            ids.extend_from_slice(&word_u64(*id as u64));
        }
        let mut buf = [0u8; 128];
        buf[..32].copy_from_slice(&self.cancellation_products_type);
        buf[32..64].copy_from_slice(c.sender.as_bytes());
        buf[64..96].copy_from_slice(&keccak(&ids));
        buf[96..].copy_from_slice(&word_u64(c.nonce));
        typed_digest(&self.endpoint_domain, &keccak(&buf))
    }

    /// EIP-712 digest of a `LinkSigner`.
    pub fn link_signer_digest(&self, l: &LinkSigner) -> [u8; 32] {
        let mut buf = [0u8; 128];
        buf[..32].copy_from_slice(&self.link_signer_type);
        buf[32..64].copy_from_slice(l.sender.as_bytes());
        buf[64..96].copy_from_slice(l.signer.as_bytes());
        buf[96..].copy_from_slice(&word_u64(l.nonce));
        typed_digest(&self.endpoint_domain, &keccak(&buf))
    }

    /// EIP-712 digest of a `StreamAuthentication`.
    pub fn stream_auth_digest(&self, sender: &Subaccount, expiration_ms: u64) -> [u8; 32] {
        let mut buf = [0u8; 96];
        buf[..32].copy_from_slice(&self.stream_auth_type);
        buf[32..64].copy_from_slice(sender.as_bytes());
        buf[64..].copy_from_slice(&word_u64(expiration_ms));
        typed_digest(&self.endpoint_domain, &keccak(&buf))
    }

    /// The inner `place_order` object and the order digest.
    fn place_order_object(&self, p: &PlaceOrder) -> Result<(Value, String)> {
        let digest = self.order_digest(p.product_id, &p.order);
        let o = &p.order;
        let mut obj = Map::new();
        obj.insert("product_id".into(), json!(p.product_id));
        obj.insert(
            "order".into(),
            json!({
                "sender": o.sender.to_hex(),
                "priceX18": o.price_x18.to_string(),
                "amount": o.amount.to_string(),
                "expiration": o.expiration.to_string(),
                "nonce": o.nonce.to_string(),
                "appendix": o.appendix.to_string(),
            }),
        );
        obj.insert("signature".into(), json!(self.sign_digest(&digest)?));
        if let Some(id) = p.id {
            obj.insert("id".into(), json!(id));
        }
        if let Some(lev) = p.spot_leverage {
            obj.insert("spot_leverage".into(), json!(lev));
        }
        Ok((Value::Object(obj), hex0x(&digest)))
    }

    /// `{"place_order": {"product_id", "order", "signature", "id"?, "spot_leverage"?}}`.
    pub fn sign_place_order(&self, p: &PlaceOrder) -> Result<SignedExecute> {
        let (obj, digest) = self.place_order_object(p)?;
        Ok(SignedExecute {
            kind: ExecuteKind::PlaceOrder,
            digests: vec![digest],
            payload: json!({ "place_order": obj }),
        })
    }

    /// `{"place_orders": {"orders": [..], "stop_on_failure"?}}`: each order signed as in
    /// `place_order`.
    pub fn sign_place_orders(
        &self,
        orders: &[PlaceOrder],
        stop_on_failure: Option<bool>,
    ) -> Result<SignedExecute> {
        let mut objs = Vec::with_capacity(orders.len());
        let mut digests = Vec::with_capacity(orders.len());
        for p in orders {
            let (obj, digest) = self.place_order_object(p)?;
            objs.push(obj);
            digests.push(digest);
        }
        let mut inner = json!({ "orders": objs });
        if let Some(stop) = stop_on_failure {
            inner["stop_on_failure"] = json!(stop);
        }
        Ok(SignedExecute {
            kind: ExecuteKind::PlaceOrders,
            digests,
            payload: json!({ "place_orders": inner }),
        })
    }

    /// The cancellation `tx`, its signature and digest.
    fn cancel_tx(&self, c: &CancelOrders) -> Result<(Value, String, String)> {
        if c.product_ids.len() != c.digests.len() {
            bail!(
                "cancel_orders: {} product ids for {} digests",
                c.product_ids.len(),
                c.digests.len()
            );
        }
        let digest = self.cancellation_digest(c);
        let digests: Vec<String> = c.digests.iter().map(|d| hex0x(d)).collect();
        let tx = json!({
            "sender": c.sender.to_hex(),
            "productIds": c.product_ids,
            "digests": digests,
            "nonce": c.nonce.to_string(),
        });
        Ok((tx, self.sign_digest(&digest)?, hex0x(&digest)))
    }

    /// `{"cancel_orders": {"tx", "signature", "required_unfilled_amount"?, "id"?}}`.
    pub fn sign_cancel_orders(&self, c: &CancelOrders) -> Result<SignedExecute> {
        let (tx, signature, digest) = self.cancel_tx(c)?;
        let mut inner = json!({ "tx": tx, "signature": signature });
        if let Some(amount) = c.required_unfilled_amount {
            inner["required_unfilled_amount"] = json!(amount.to_string());
        }
        if let Some(id) = c.id {
            inner["id"] = json!(id);
        }
        Ok(SignedExecute {
            kind: ExecuteKind::CancelOrders,
            digests: vec![digest],
            payload: json!({ "cancel_orders": inner }),
        })
    }

    /// `{"cancel_product_orders": {"tx", "signature", "id"?}}`. Empty `product_ids` cancels on
    /// every product.
    pub fn sign_cancel_product_orders(&self, c: &CancelProductOrders) -> Result<SignedExecute> {
        let digest = self.cancellation_products_digest(c);
        let mut inner = json!({
            "tx": {
                "sender": c.sender.to_hex(),
                "productIds": c.product_ids,
                "nonce": c.nonce.to_string(),
            },
            "signature": self.sign_digest(&digest)?,
        });
        if let Some(id) = c.id {
            inner["id"] = json!(id);
        }
        Ok(SignedExecute {
            kind: ExecuteKind::CancelProductOrders,
            digests: vec![hex0x(&digest)],
            payload: json!({ "cancel_product_orders": inner }),
        })
    }

    /// `{"cancel_and_place": {"cancel_tx", "cancel_signature", "place_order",
    /// "required_unfilled_amount"?, "place_requires_unfilled"?}}`. `cancel.required_unfilled_amount`
    /// moves to the top level; a cancel-leg `id` is not supported (put it on `place`).
    pub fn sign_cancel_and_place(
        &self,
        cancel: &CancelOrders,
        place: &PlaceOrder,
        place_requires_unfilled: Option<bool>,
    ) -> Result<SignedExecute> {
        if cancel.id.is_some() {
            bail!("cancel_and_place takes its id on the place_order leg");
        }
        let (tx, cancel_signature, cancel_digest) = self.cancel_tx(cancel)?;
        let (place_obj, place_digest) = self.place_order_object(place)?;
        let mut inner = json!({
            "cancel_tx": tx,
            "cancel_signature": cancel_signature,
            "place_order": place_obj,
        });
        if let Some(amount) = cancel.required_unfilled_amount {
            inner["required_unfilled_amount"] = json!(amount.to_string());
        }
        if let Some(b) = place_requires_unfilled {
            inner["place_requires_unfilled"] = json!(b);
        }
        Ok(SignedExecute {
            kind: ExecuteKind::CancelAndPlace,
            digests: vec![cancel_digest, place_digest],
            payload: json!({ "cancel_and_place": inner }),
        })
    }

    /// `{"link_signer": {"tx", "signature"}}`. Must be signed by the subaccount owner.
    pub fn sign_link_signer(&self, l: &LinkSigner) -> Result<SignedExecute> {
        let digest = self.link_signer_digest(l);
        Ok(SignedExecute {
            kind: ExecuteKind::LinkSigner,
            digests: vec![hex0x(&digest)],
            payload: json!({
                "link_signer": {
                    "tx": {
                        "sender": l.sender.to_hex(),
                        "signer": l.signer.to_hex(),
                        "nonce": l.nonce.to_string(),
                    },
                    "signature": self.sign_digest(&digest)?,
                }
            }),
        })
    }

    /// Subscriptions `authenticate` frame. `expiration_ms` must be in `(now, now + 100 s]`.
    /// Authenticates the whole wallet of `sender` for the life of the connection.
    pub fn stream_auth_message(
        &self,
        sender: &Subaccount,
        expiration_ms: u64,
        id: u64,
    ) -> Result<String> {
        let digest = self.stream_auth_digest(sender, expiration_ms);
        Ok(json!({
            "method": "authenticate",
            "id": id,
            "tx": {
                "sender": sender.to_hex(),
                "expiration": expiration_ms.to_string(),
            },
            "signature": self.sign_digest(&digest)?,
        })
        .to_string())
    }
}
