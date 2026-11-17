//! Polymarket Perps request signing.
//!
//! Every trading operation is an EIP-712 `Op(bytes32 data,uint64 salt,uint64 ts)` signed with the
//! proxy key, where `data` is `keccak256(msgpack(compact op))`. The compact op is the SDK's
//! positional array form (`["createOrders", [[iid, buy, p, qty, tif, po, ro, c, gtd]]]`) with every
//! absent entry removed. Registering a proxy is an EIP-712
//! `CreateProxy(address addr,uint64 exp,uint64 salt,uint64 ts)` signed by the account owner; revoking
//! it is an `Op` (`["deleteProxy", [proxy]]`) signed by the owner. The domain is
//! `{name: "Polymarket", version: "1", chainId}` with no verifying contract.
//!
//! The caller supplies `salt`, `ts` and `exp`; nothing here reads the clock or a random source.

use anyhow::{anyhow, bail, Context, Result};
use k256::ecdsa::SigningKey;
use serde::Serialize;
use sha3::{Digest, Keccak256};

use crate::types::TimeInForce;

/// Chain id of the production EIP-712 domain (`chain_id` in `GET /v1/info/exchange`).
pub const POLYGON_CHAIN_ID: u64 = 137;

/// The SDK holds every integer it msgpack-encodes as a JS number. Larger values would be encoded
/// as float64 by the SDK, so they cannot be signed with parity.
const MAX_SAFE_INTEGER: u64 = (1 << 53) - 1;

const DOMAIN_TYPE: &[u8] = b"EIP712Domain(string name,string version,uint256 chainId)";
const OP_TYPE: &[u8] = b"Op(bytes32 data,uint64 salt,uint64 ts)";
const CREATE_PROXY_TYPE: &[u8] = b"CreateProxy(address addr,uint64 exp,uint64 salt,uint64 ts)";

fn keccak(data: &[u8]) -> [u8; 32] {
    Keccak256::digest(data).into()
}

fn u256(v: u64) -> [u8; 32] {
    let mut w = [0u8; 32];
    w[24..].copy_from_slice(&v.to_be_bytes());
    w
}

fn parse_address(s: &str) -> Result<[u8; 20]> {
    let hex_part = s
        .strip_prefix("0x")
        .ok_or_else(|| anyhow!("address {s:?}: missing 0x prefix"))?;
    if hex_part.len() != 40 {
        bail!("address {s:?}: expected 40 hex characters");
    }
    let mut out = [0u8; 20];
    hex::decode_to_slice(hex_part, &mut out).map_err(|_| anyhow!("address {s:?}: invalid hex"))?;
    Ok(out)
}

/// EIP-55 mixed-case form of a 20-byte address.
fn checksum_address(addr: &[u8; 20]) -> String {
    let lower = hex::encode(addr);
    let hash = keccak(lower.as_bytes());
    let mut out = String::with_capacity(42);
    out.push_str("0x");
    for (i, c) in lower.chars().enumerate() {
        let nibble = (hash[i / 2] >> (if i % 2 == 0 { 4 } else { 0 })) & 0x0f;
        if c.is_ascii_alphabetic() && nibble >= 8 {
            out.push(c.to_ascii_uppercase());
        } else {
            out.push(c);
        }
    }
    out
}

fn check_safe(value: u64, what: &str) -> Result<()> {
    if value > MAX_SAFE_INTEGER {
        bail!("{what} {value} exceeds 2^53 - 1");
    }
    Ok(())
}

fn check_coid(coid: &str) -> Result<()> {
    if coid.len() != 32 || !coid.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
        bail!("client order id {coid:?}: expected 32 lowercase hex characters");
    }
    Ok(())
}

fn check_decimal(value: &str, what: &str) -> Result<()> {
    if value.is_empty() {
        bail!("{what}: empty decimal string");
    }
    Ok(())
}

fn mp_arr(w: &mut Vec<u8>, n: usize) {
    rmp::encode::write_array_len(w, n as u32).expect("vec write");
}
fn mp_str(w: &mut Vec<u8>, s: &str) {
    rmp::encode::write_str(w, s).expect("vec write");
}
fn mp_uint(w: &mut Vec<u8>, n: u64) {
    rmp::encode::write_uint(w, n).expect("vec write");
}
fn mp_bool(w: &mut Vec<u8>, b: bool) {
    rmp::encode::write_bool(w, b).expect("vec write");
}

/// One order in a `createOrders` operation (`POST /v1/trade/orders`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreateOrder {
    /// `iid`.
    pub instrument_id: u32,
    /// `buy`: `true` buys, `false` sells.
    pub buy: bool,
    /// `p`: limit price as a decimal string. Required for `gtc`/`gtd`; omit for a market-style
    /// `ioc`/`fok` order.
    pub price: Option<String>,
    /// `qty`: number of contracts as a decimal string.
    pub quantity: String,
    /// `tif`.
    pub tif: TimeInForce,
    /// `po`: post-only. Only valid with `gtc`/`gtd`.
    pub post_only: bool,
    /// `ro`: reduce-only.
    pub reduce_only: bool,
    /// `c`: client order id, 32 lowercase hex characters.
    pub client_order_id: Option<String>,
    /// `gtd_expiry`: order expiry in Unix ms. Required with `gtd`, rejected otherwise.
    pub gtd_expiry: Option<u64>,
}

/// One entry of a `modifyOrders` operation (`PATCH /v1/trade/orders`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModifyOrder {
    /// `oid`.
    pub order_id: u64,
    /// `p`: new limit price.
    pub price: String,
    /// `qty`: new total quantity, including any quantity already filled.
    pub quantity: String,
}

/// One entry of a `modifyOrdersCOID` operation (`PATCH /v1/trade/orders-coid`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModifyOrderCoid {
    /// `coid`.
    pub client_order_id: String,
    /// `p`: new limit price.
    pub price: String,
    /// `qty`: new total quantity, including any quantity already filled.
    pub quantity: String,
}

/// Leverage and margin mode for one instrument (`updateLeverage` / `updateLeverages`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LeverageUpdate {
    /// `iid`.
    pub instrument_id: u32,
    /// `lev`.
    pub leverage: u32,
    /// `cross`: `true` for cross margin, `false` for isolated.
    pub cross: bool,
}

/// A signed operation. Each variant maps to one REST route and one WebSocket `post` op.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Op {
    /// `POST /v1/trade/orders`.
    CreateOrders(Vec<CreateOrder>),
    /// `DELETE /v1/trade/orders`, by exchange order id.
    CancelOrders(Vec<u64>),
    /// `DELETE /v1/trade/orders-coid`, by client order id.
    CancelOrdersCoid(Vec<String>),
    /// `PATCH /v1/trade/orders`, by exchange order id.
    ModifyOrders(Vec<ModifyOrder>),
    /// `PATCH /v1/trade/orders-coid`, by client order id.
    ModifyOrdersCoid(Vec<ModifyOrderCoid>),
    /// `DELETE /v1/trade/orders/all`; `None` cancels on every instrument.
    CancelAll { instrument_id: Option<u32> },
    /// `PATCH /v1/trade/auto-cancel`: cancel-all time in Unix ms, `0` to disarm.
    AutoCancel { time: u64 },
    /// `PATCH /v1/trade/leverage`.
    UpdateLeverage(LeverageUpdate),
    /// `PATCH /v1/trade/leverage/batch` (1..=100 unique instruments).
    UpdateLeverages(Vec<LeverageUpdate>),
    /// `PATCH /v1/trade/margin`: signed isolated-margin adjustment in the quote asset.
    UpdateMargin { instrument_id: u32, amount: String },
    /// `DELETE /v1/account/proxy`. Signed by the account owner, not the proxy. The address is
    /// hashed exactly as given.
    DeleteProxy { proxy: String },
}

impl Op {
    /// `op.type` on the wire.
    pub fn type_name(&self) -> &'static str {
        match self {
            Op::CreateOrders(_) => "createOrders",
            Op::CancelOrders(_) => "cancelOrders",
            Op::CancelOrdersCoid(_) => "cancelOrdersCOID",
            Op::ModifyOrders(_) => "modifyOrders",
            Op::ModifyOrdersCoid(_) => "modifyOrdersCOID",
            Op::CancelAll { .. } => "cancelAll",
            Op::AutoCancel { .. } => "autoCancel",
            Op::UpdateLeverage(_) => "updateLeverage",
            Op::UpdateLeverages(_) => "updateLeverages",
            Op::UpdateMargin { .. } => "updateMargin",
            Op::DeleteProxy { .. } => "deleteProxy",
        }
    }

    /// HTTP method and path of the REST route.
    pub fn rest_route(&self) -> (&'static str, &'static str) {
        match self {
            Op::CreateOrders(_) => ("POST", "/v1/trade/orders"),
            Op::CancelOrders(_) => ("DELETE", "/v1/trade/orders"),
            Op::CancelOrdersCoid(_) => ("DELETE", "/v1/trade/orders-coid"),
            Op::ModifyOrders(_) => ("PATCH", "/v1/trade/orders"),
            Op::ModifyOrdersCoid(_) => ("PATCH", "/v1/trade/orders-coid"),
            Op::CancelAll { .. } => ("DELETE", "/v1/trade/orders/all"),
            Op::AutoCancel { .. } => ("PATCH", "/v1/trade/auto-cancel"),
            Op::UpdateLeverage(_) => ("PATCH", "/v1/trade/leverage"),
            Op::UpdateLeverages(_) => ("PATCH", "/v1/trade/leverage/batch"),
            Op::UpdateMargin { .. } => ("PATCH", "/v1/trade/margin"),
            Op::DeleteProxy { .. } => ("DELETE", "/v1/account/proxy"),
        }
    }

    /// Whether the documented request carries the optional `exp` command deadline.
    pub fn accepts_exp(&self) -> bool {
        matches!(
            self,
            Op::CreateOrders(_)
                | Op::CancelOrders(_)
                | Op::CancelOrdersCoid(_)
                | Op::ModifyOrders(_)
                | Op::ModifyOrdersCoid(_)
                | Op::CancelAll { .. }
        )
    }

    /// Rejects what the SDK's input schemas reject.
    pub fn validate(&self) -> Result<()> {
        match self {
            Op::CreateOrders(orders) => {
                if orders.is_empty() {
                    bail!("createOrders: at least one order is required");
                }
                for o in orders {
                    check_decimal(&o.quantity, "qty")?;
                    if let Some(p) = &o.price {
                        check_decimal(p, "p")?;
                    }
                    match o.tif {
                        TimeInForce::Gtc | TimeInForce::Gtd if o.price.is_none() => {
                            bail!("createOrders: gtc/gtd orders need a price")
                        }
                        TimeInForce::Ioc | TimeInForce::Fok if o.post_only => {
                            bail!("createOrders: post-only is only valid with gtc/gtd")
                        }
                        _ => {}
                    }
                    match (o.tif, o.gtd_expiry) {
                        (TimeInForce::Gtd, Some(e)) => check_safe(e, "gtd_expiry")?,
                        (TimeInForce::Gtd, None) => bail!("createOrders: gtd needs gtd_expiry"),
                        (_, Some(_)) => bail!("createOrders: gtd_expiry is only valid with gtd"),
                        (_, None) => {}
                    }
                    if let Some(c) = &o.client_order_id {
                        check_coid(c)?;
                    }
                }
            }
            Op::CancelOrders(ids) => {
                if ids.is_empty() {
                    bail!("cancelOrders: at least one order id is required");
                }
                for &id in ids {
                    check_safe(id, "order id")?;
                }
            }
            Op::CancelOrdersCoid(ids) => {
                if ids.is_empty() {
                    bail!("cancelOrdersCOID: at least one client order id is required");
                }
                for c in ids {
                    check_coid(c)?;
                }
            }
            Op::ModifyOrders(items) => {
                if items.is_empty() {
                    bail!("modifyOrders: at least one order is required");
                }
                for m in items {
                    check_safe(m.order_id, "order id")?;
                    check_decimal(&m.price, "p")?;
                    check_decimal(&m.quantity, "qty")?;
                }
            }
            Op::ModifyOrdersCoid(items) => {
                if items.is_empty() {
                    bail!("modifyOrdersCOID: at least one order is required");
                }
                for m in items {
                    check_coid(&m.client_order_id)?;
                    check_decimal(&m.price, "p")?;
                    check_decimal(&m.quantity, "qty")?;
                }
            }
            Op::CancelAll { .. } => {}
            Op::AutoCancel { time } => check_safe(*time, "auto-cancel time")?,
            Op::UpdateLeverage(u) => {
                if u.leverage == 0 {
                    bail!("updateLeverage: leverage must be positive");
                }
            }
            Op::UpdateLeverages(updates) => {
                if updates.is_empty() || updates.len() > 100 {
                    bail!("updateLeverages: 1 to 100 updates are required");
                }
                for (i, u) in updates.iter().enumerate() {
                    if u.leverage == 0 {
                        bail!("updateLeverages: leverage must be positive");
                    }
                    if updates[..i]
                        .iter()
                        .any(|p| p.instrument_id == u.instrument_id)
                    {
                        bail!("updateLeverages: duplicate instrument {}", u.instrument_id);
                    }
                }
            }
            Op::UpdateMargin { amount, .. } => check_decimal(amount, "amt")?,
            Op::DeleteProxy { proxy } => {
                parse_address(proxy)?;
            }
        }
        Ok(())
    }

    /// MessagePack encoding of the compact op (absent entries removed), as hashed into `data`.
    pub fn compact_msgpack(&self) -> Vec<u8> {
        let mut w = Vec::with_capacity(64);
        mp_arr(&mut w, 2);
        mp_str(&mut w, self.type_name());
        match self {
            Op::CreateOrders(orders) => {
                mp_arr(&mut w, orders.len());
                for o in orders {
                    let len = 5
                        + o.price.is_some() as usize
                        + o.reduce_only as usize
                        + o.client_order_id.is_some() as usize
                        + o.gtd_expiry.is_some() as usize;
                    mp_arr(&mut w, len);
                    mp_uint(&mut w, o.instrument_id as u64);
                    mp_bool(&mut w, o.buy);
                    if let Some(p) = &o.price {
                        mp_str(&mut w, p);
                    }
                    mp_str(&mut w, &o.quantity);
                    mp_str(&mut w, o.tif.as_str());
                    mp_bool(&mut w, o.post_only);
                    if o.reduce_only {
                        mp_bool(&mut w, true);
                    }
                    if let Some(c) = &o.client_order_id {
                        mp_str(&mut w, c);
                    }
                    if let Some(e) = o.gtd_expiry {
                        mp_uint(&mut w, e);
                    }
                }
            }
            Op::CancelOrders(ids) => {
                mp_arr(&mut w, ids.len());
                for &id in ids {
                    mp_uint(&mut w, id);
                }
            }
            Op::CancelOrdersCoid(ids) => {
                mp_arr(&mut w, ids.len());
                for c in ids {
                    mp_str(&mut w, c);
                }
            }
            Op::ModifyOrders(items) => {
                mp_arr(&mut w, items.len());
                for m in items {
                    mp_arr(&mut w, 3);
                    mp_uint(&mut w, m.order_id);
                    mp_str(&mut w, &m.price);
                    mp_str(&mut w, &m.quantity);
                }
            }
            Op::ModifyOrdersCoid(items) => {
                mp_arr(&mut w, items.len());
                for m in items {
                    mp_arr(&mut w, 3);
                    mp_str(&mut w, &m.client_order_id);
                    mp_str(&mut w, &m.price);
                    mp_str(&mut w, &m.quantity);
                }
            }
            Op::CancelAll { instrument_id } => match instrument_id {
                Some(iid) => {
                    mp_arr(&mut w, 1);
                    mp_uint(&mut w, *iid as u64);
                }
                None => mp_arr(&mut w, 0),
            },
            Op::AutoCancel { time } => {
                mp_arr(&mut w, 1);
                mp_uint(&mut w, *time);
            }
            Op::UpdateLeverage(u) => pack_leverage(&mut w, u),
            Op::UpdateLeverages(updates) => {
                mp_arr(&mut w, updates.len());
                for u in updates {
                    pack_leverage(&mut w, u);
                }
            }
            Op::UpdateMargin {
                instrument_id,
                amount,
            } => {
                mp_arr(&mut w, 2);
                mp_uint(&mut w, *instrument_id as u64);
                mp_str(&mut w, amount);
            }
            Op::DeleteProxy { proxy } => {
                mp_arr(&mut w, 1);
                mp_str(&mut w, proxy);
            }
        }
        w
    }

    /// `data` field of the `Op` typed data: `keccak256(compact_msgpack())`.
    pub fn hash(&self) -> [u8; 32] {
        keccak(&self.compact_msgpack())
    }

    /// The structured `op` JSON object sent in the request body.
    pub fn body_json(&self) -> String {
        serde_json::to_string(&OpBody(self)).expect("op body serializes")
    }
}

fn pack_leverage(w: &mut Vec<u8>, u: &LeverageUpdate) {
    mp_arr(w, 3);
    mp_uint(w, u.instrument_id as u64);
    mp_uint(w, u.leverage as u64);
    mp_bool(w, u.cross);
}

fn is_false(b: &bool) -> bool {
    !*b
}

/// `{"type", "args"}` with the field order the SDK sends.
#[derive(Serialize)]
struct TypeArgs<A> {
    #[serde(rename = "type")]
    ty: &'static str,
    args: A,
}

/// `createOrders` puts `args` first.
#[derive(Serialize)]
struct ArgsType<A> {
    args: A,
    #[serde(rename = "type")]
    ty: &'static str,
}

#[derive(Serialize)]
struct OrderBody<'a> {
    iid: u32,
    buy: bool,
    po: bool,
    qty: &'a str,
    tif: TimeInForce,
    #[serde(skip_serializing_if = "is_false")]
    ro: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    p: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    c: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    gtd_expiry: Option<u64>,
}

#[derive(Serialize)]
struct ModifyBody<'a> {
    oid: u64,
    p: &'a str,
    qty: &'a str,
}

#[derive(Serialize)]
struct ModifyCoidBody<'a> {
    coid: &'a str,
    p: &'a str,
    qty: &'a str,
}

#[derive(Serialize)]
struct CancelAllArgs {
    #[serde(skip_serializing_if = "Option::is_none")]
    iid: Option<u32>,
}

#[derive(Serialize)]
struct TimeArgs {
    time: u64,
}

#[derive(Serialize)]
struct LeverageBody {
    cross: bool,
    iid: u32,
    lev: u32,
}

impl From<&LeverageUpdate> for LeverageBody {
    fn from(u: &LeverageUpdate) -> Self {
        LeverageBody {
            cross: u.cross,
            iid: u.instrument_id,
            lev: u.leverage,
        }
    }
}

#[derive(Serialize)]
struct MarginBody<'a> {
    amt: &'a str,
    iid: u32,
}

#[derive(Serialize)]
struct ProxyArg<'a> {
    proxy: &'a str,
}

struct OpBody<'a>(&'a Op);

impl Serialize for OpBody<'_> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let op = self.0;
        let ty = op.type_name();
        match op {
            Op::CreateOrders(orders) => ArgsType {
                args: orders
                    .iter()
                    .map(|o| OrderBody {
                        iid: o.instrument_id,
                        buy: o.buy,
                        po: o.post_only,
                        qty: &o.quantity,
                        tif: o.tif,
                        ro: o.reduce_only,
                        p: o.price.as_deref(),
                        c: o.client_order_id.as_deref(),
                        gtd_expiry: o.gtd_expiry,
                    })
                    .collect::<Vec<_>>(),
                ty,
            }
            .serialize(s),
            Op::CancelOrders(ids) => TypeArgs { ty, args: ids }.serialize(s),
            Op::CancelOrdersCoid(ids) => TypeArgs { ty, args: ids }.serialize(s),
            Op::ModifyOrders(items) => TypeArgs {
                ty,
                args: items
                    .iter()
                    .map(|m| ModifyBody {
                        oid: m.order_id,
                        p: &m.price,
                        qty: &m.quantity,
                    })
                    .collect::<Vec<_>>(),
            }
            .serialize(s),
            Op::ModifyOrdersCoid(items) => TypeArgs {
                ty,
                args: items
                    .iter()
                    .map(|m| ModifyCoidBody {
                        coid: &m.client_order_id,
                        p: &m.price,
                        qty: &m.quantity,
                    })
                    .collect::<Vec<_>>(),
            }
            .serialize(s),
            Op::CancelAll { instrument_id } => TypeArgs {
                ty,
                args: CancelAllArgs {
                    iid: *instrument_id,
                },
            }
            .serialize(s),
            Op::AutoCancel { time } => TypeArgs {
                ty,
                args: TimeArgs { time: *time },
            }
            .serialize(s),
            Op::UpdateLeverage(u) => TypeArgs {
                ty,
                args: LeverageBody::from(u),
            }
            .serialize(s),
            Op::UpdateLeverages(updates) => TypeArgs {
                ty,
                args: updates.iter().map(LeverageBody::from).collect::<Vec<_>>(),
            }
            .serialize(s),
            Op::UpdateMargin {
                instrument_id,
                amount,
            } => TypeArgs {
                ty,
                args: MarginBody {
                    amt: amount,
                    iid: *instrument_id,
                },
            }
            .serialize(s),
            Op::DeleteProxy { proxy } => TypeArgs {
                ty,
                args: ProxyArg { proxy },
            }
            .serialize(s),
        }
    }
}

/// A signed operation, ready for its REST route or the WebSocket.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SignedCommand {
    op: Op,
    salt: u64,
    ts: u64,
    exp: Option<u64>,
    signature: String,
}

#[derive(Serialize)]
struct CommandBody<'a> {
    salt: u64,
    sig: &'a str,
    ts: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    exp: Option<u64>,
    op: OpBody<'a>,
}

#[derive(Serialize)]
struct OwnerOpBody<'a> {
    op: OpBody<'a>,
    salt: u64,
    sig: &'a str,
    ts: u64,
}

#[derive(Serialize)]
struct WsPost<'a> {
    salt: u64,
    sig: &'a str,
    ts: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    exp: Option<u64>,
    id: u64,
    op: OpBody<'a>,
    req: &'static str,
}

impl SignedCommand {
    pub fn op(&self) -> &Op {
        &self.op
    }

    pub fn salt(&self) -> u64 {
        self.salt
    }

    pub fn ts(&self) -> u64 {
        self.ts
    }

    pub fn exp(&self) -> Option<u64> {
        self.exp
    }

    /// `0x` + r ‖ s ‖ v (v = 27/28).
    pub fn signature(&self) -> &str {
        &self.signature
    }

    /// JSON body for the op's REST route (`Op::rest_route`).
    pub fn rest_body(&self) -> String {
        let out = match &self.op {
            Op::DeleteProxy { .. } => serde_json::to_string(&OwnerOpBody {
                op: OpBody(&self.op),
                salt: self.salt,
                sig: &self.signature,
                ts: self.ts,
            }),
            _ => serde_json::to_string(&CommandBody {
                salt: self.salt,
                sig: &self.signature,
                ts: self.ts,
                exp: self.exp,
                op: OpBody(&self.op),
            }),
        };
        out.expect("command body serializes")
    }

    /// WebSocket `post` frame (`{"salt","sig","ts","exp"?,"id","op","req":"post"}`). `id`
    /// correlates the response. `deleteProxy` has no WebSocket form.
    pub fn ws_frame(&self, id: u64) -> Result<String> {
        if matches!(self.op, Op::DeleteProxy { .. }) {
            bail!("deleteProxy is REST only");
        }
        Ok(serde_json::to_string(&WsPost {
            salt: self.salt,
            sig: &self.signature,
            ts: self.ts,
            exp: self.exp,
            id,
            op: OpBody(&self.op),
            req: "post",
        })?)
    }
}

/// A signed `createProxy` request (`POST /v1/account/proxy`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SignedCreateProxy {
    owner: String,
    proxy: String,
    expiry: u64,
    salt: u64,
    ts: u64,
    label: Option<String>,
    signature: String,
}

#[derive(Serialize)]
struct CreateProxyArgs<'a> {
    expiry: u64,
    owner: &'a str,
    proxy: &'a str,
}

#[derive(Serialize)]
struct CreateProxyBody<'a> {
    op: ArgsType<CreateProxyArgs<'a>>,
    salt: u64,
    sig: &'a str,
    ts: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    label: Option<&'a str>,
}

impl SignedCreateProxy {
    pub fn signature(&self) -> &str {
        &self.signature
    }

    pub fn rest_body(&self) -> String {
        serde_json::to_string(&CreateProxyBody {
            op: ArgsType {
                args: CreateProxyArgs {
                    expiry: self.expiry,
                    owner: &self.owner,
                    proxy: &self.proxy,
                },
                ty: "createProxy",
            },
            salt: self.salt,
            sig: &self.signature,
            ts: self.ts,
            label: self.label.as_deref(),
        })
        .expect("createProxy body serializes")
    }
}

/// A secp256k1 key with the Polymarket EIP-712 domain for one chain. Use the proxy key for
/// trading ops; use the account owner's key for `sign_create_proxy` and `Op::DeleteProxy`.
pub struct Signer {
    key: SigningKey,
    address: [u8; 20],
    domain_separator: [u8; 32],
}

impl std::fmt::Debug for Signer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never print the key.
        f.debug_struct("Signer")
            .field("address", &self.address())
            .finish_non_exhaustive()
    }
}

impl Signer {
    /// `private_key_hex`: 32 bytes, with or without `0x`. `chain_id`: the exchange's
    /// `chain_id` (`POLYGON_CHAIN_ID` in production).
    pub fn from_hex(private_key_hex: &str, chain_id: u64) -> Result<Self> {
        let s = private_key_hex.trim();
        let s = s.strip_prefix("0x").unwrap_or(s);
        let mut kb = [0u8; 32];
        hex::decode_to_slice(s, &mut kb)
            .map_err(|_| anyhow!("private key: expected 32 hex bytes"))?;
        let key = SigningKey::from_bytes((&kb).into())
            .context("private key: not a valid secp256k1 scalar")?;
        let point = key.verifying_key().to_encoded_point(false);
        let hash = keccak(&point.as_bytes()[1..]);
        let mut address = [0u8; 20];
        address.copy_from_slice(&hash[12..]);

        let mut dom = [0u8; 128];
        dom[..32].copy_from_slice(&keccak(DOMAIN_TYPE));
        dom[32..64].copy_from_slice(&keccak(b"Polymarket"));
        dom[64..96].copy_from_slice(&keccak(b"1"));
        dom[96..].copy_from_slice(&u256(chain_id));

        Ok(Signer {
            key,
            address,
            domain_separator: keccak(&dom),
        })
    }

    /// EIP-55 checksummed address of the key.
    pub fn address(&self) -> String {
        checksum_address(&self.address)
    }

    fn typed_digest(&self, struct_hash: &[u8; 32]) -> [u8; 32] {
        let mut m = [0u8; 66];
        m[0] = 0x19;
        m[1] = 0x01;
        m[2..34].copy_from_slice(&self.domain_separator);
        m[34..].copy_from_slice(struct_hash);
        keccak(&m)
    }

    fn sign_digest(&self, digest: &[u8; 32]) -> Result<String> {
        let (sig, recid) = self
            .key
            .sign_prehash_recoverable(digest)
            .context("signing failed")?;
        let mut out = [0u8; 65];
        out[..64].copy_from_slice(&sig.to_bytes());
        out[64] = 27 + recid.to_byte();
        Ok(format!("0x{}", hex::encode(out)))
    }

    /// EIP-712 digest of `Op(data, salt, ts)` for `op`.
    pub fn op_digest(&self, op: &Op, salt: u64, ts: u64) -> [u8; 32] {
        let mut st = [0u8; 128];
        st[..32].copy_from_slice(&keccak(OP_TYPE));
        st[32..64].copy_from_slice(&op.hash());
        st[64..96].copy_from_slice(&u256(salt));
        st[96..].copy_from_slice(&u256(ts));
        self.typed_digest(&keccak(&st))
    }

    /// Signs `op`. `salt` is the per-request random integer, `ts` the current Unix ms and `exp`
    /// the optional command-processing deadline (Unix ms; only for ops where
    /// `Op::accepts_exp`). `exp` is sent in the body but is not part of the signed data.
    pub fn sign_op(&self, op: Op, exp: Option<u64>, salt: u64, ts: u64) -> Result<SignedCommand> {
        op.validate()?;
        if exp.is_some() && !op.accepts_exp() {
            bail!("{} does not take exp", op.type_name());
        }
        let signature = self.sign_digest(&self.op_digest(&op, salt, ts))?;
        Ok(SignedCommand {
            op,
            salt,
            ts,
            exp,
            signature,
        })
    }

    /// EIP-712 digest of `CreateProxy(addr, exp, salt, ts)`.
    pub fn create_proxy_digest(
        &self,
        proxy: &str,
        expiry: u64,
        salt: u64,
        ts: u64,
    ) -> Result<[u8; 32]> {
        let addr = parse_address(proxy)?;
        let mut st = [0u8; 160];
        st[..32].copy_from_slice(&keccak(CREATE_PROXY_TYPE));
        st[44..64].copy_from_slice(&addr);
        st[64..96].copy_from_slice(&u256(expiry));
        st[96..128].copy_from_slice(&u256(salt));
        st[128..].copy_from_slice(&u256(ts));
        Ok(self.typed_digest(&keccak(&st)))
    }

    /// Owner-signed registration of `proxy` (the proxy key's address) until `expiry` (Unix ms).
    /// The body's `owner` is this signer's checksummed address and `proxy` is sent as given.
    pub fn sign_create_proxy(
        &self,
        proxy: &str,
        expiry: u64,
        salt: u64,
        ts: u64,
        label: Option<&str>,
    ) -> Result<SignedCreateProxy> {
        let digest = self.create_proxy_digest(proxy, expiry, salt, ts)?;
        Ok(SignedCreateProxy {
            owner: self.address(),
            proxy: proxy.to_string(),
            expiry,
            salt,
            ts,
            label: label.map(str::to_string),
            signature: self.sign_digest(&digest)?,
        })
    }
}
