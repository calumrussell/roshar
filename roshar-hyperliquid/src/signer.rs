//! Hyperliquid L1 action signing, byte-identical to the official Python SDK
//! (`sign_l1_action` without `expiresAfter`).
//!
//! connection id = keccak256(msgpack(action) || nonce_be_u64 || (0x00 | 0x01 || vault[20]))
//! signature     = secp256k1 over the EIP-712 digest of `Agent(string source,bytes32 connectionId)`
//!                 in domain {name "Exchange", version "1", chainId 1337, verifyingContract 0x0}.

use anyhow::{anyhow, bail, Context};
use k256::ecdsa::SigningKey;
use crate::types::exchange::{
    Action, Cloid, Grouping, OrderRef, OrderReq, OrderType, Tif, Tpsl, TypedOrderReq,
};
use serde_json::{json, Value};
use sha3::{Digest, Keccak256};

fn keccak(data: &[u8]) -> [u8; 32] {
    Keccak256::digest(data).into()
}

/// SDK `float_to_wire`: `f"{x:.8f}"`, then normalized (trailing zeros and point stripped, "-0" -> "0").
pub fn float_to_wire(x: f64) -> String {
    let mut s = format!("{x:.8}");
    if s.contains('.') {
        let trimmed = s.trim_end_matches('0').trim_end_matches('.').len();
        s.truncate(trimmed);
    }
    if s == "-0" {
        s.truncate(0);
        s.push('0');
    }
    s
}

/// Lowercase `0x` + 32 hex digits, without heap allocation.
fn cloid_buf(c: &Cloid) -> [u8; 34] {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = [0u8; 34];
    out[0] = b'0';
    out[1] = b'x';
    for (i, b) in c.iter().enumerate() {
        out[2 + 2 * i] = HEX[(b >> 4) as usize];
        out[3 + 2 * i] = HEX[(b & 0xf) as usize];
    }
    out
}

fn cloid_str(buf: &[u8; 34]) -> &str {
    // Only ASCII hex digits are written.
    std::str::from_utf8(buf).expect("ascii")
}

fn tif_str(t: Tif) -> &'static str {
    match t {
        Tif::Alo => "Alo",
        Tif::Ioc => "Ioc",
        Tif::Gtc => "Gtc",
    }
}

/// An order in wire form (prices/sizes already stringified), shared by the JSON and msgpack encoders.
struct WireOrder {
    a: u32,
    b: bool,
    p: String,
    s: String,
    r: bool,
    tif: Tif,
    c: Option<[u8; 34]>,
}

impl WireOrder {
    fn new(o: &OrderReq) -> Self {
        WireOrder {
            a: o.asset,
            b: o.is_buy,
            p: float_to_wire(o.px),
            s: float_to_wire(o.sz),
            r: o.reduce_only,
            tif: o.tif,
            c: o.cloid.as_ref().map(cloid_buf),
        }
    }

    fn json(&self) -> Value {
        let mut m = serde_json::Map::new();
        m.insert("a".into(), json!(self.a));
        m.insert("b".into(), json!(self.b));
        m.insert("p".into(), json!(self.p));
        m.insert("s".into(), json!(self.s));
        m.insert("r".into(), json!(self.r));
        m.insert("t".into(), json!({"limit": {"tif": tif_str(self.tif)}}));
        if let Some(c) = &self.c {
            m.insert("c".into(), json!(cloid_str(c)));
        }
        Value::Object(m)
    }

    fn pack(&self, w: &mut Vec<u8>) {
        mp_map(w, if self.c.is_some() { 7 } else { 6 });
        mp_str(w, "a");
        mp_uint(w, self.a as u64);
        mp_str(w, "b");
        mp_bool(w, self.b);
        mp_str(w, "p");
        mp_str(w, &self.p);
        mp_str(w, "s");
        mp_str(w, &self.s);
        mp_str(w, "r");
        mp_bool(w, self.r);
        mp_str(w, "t");
        mp_map(w, 1);
        mp_str(w, "limit");
        mp_map(w, 1);
        mp_str(w, "tif");
        mp_str(w, tif_str(self.tif));
        if let Some(c) = &self.c {
            mp_str(w, "c");
            mp_str(w, cloid_str(c));
        }
    }
}

fn tpsl_str(t: Tpsl) -> &'static str {
    match t {
        Tpsl::Tp => "tp",
        Tpsl::Sl => "sl",
    }
}

fn grouping_str(g: Grouping) -> &'static str {
    match g {
        Grouping::Na => "na",
        Grouping::NormalTpsl => "normalTpsl",
        Grouping::PositionTpsl => "positionTpsl",
    }
}

/// Order type in wire form. Trigger keys are in the SDK's `order_type_to_wire` order:
/// `isMarket`, `triggerPx`, `tpsl`.
enum WireType {
    Limit(Tif),
    Trigger {
        is_market: bool,
        trigger_px: String,
        tpsl: Tpsl,
    },
}

/// A `TypedOrderReq` in wire form. Kept apart from `WireOrder` so the limit-only path is untouched.
struct WireTypedOrder {
    a: u32,
    b: bool,
    p: String,
    s: String,
    r: bool,
    t: WireType,
    c: Option<[u8; 34]>,
}

impl WireTypedOrder {
    fn new(o: &TypedOrderReq) -> Self {
        WireTypedOrder {
            a: o.asset,
            b: o.is_buy,
            p: float_to_wire(o.px),
            s: float_to_wire(o.sz),
            r: o.reduce_only,
            t: match o.order_type {
                OrderType::Limit(tif) => WireType::Limit(tif),
                OrderType::Trigger {
                    is_market,
                    trigger_px,
                    tpsl,
                } => WireType::Trigger {
                    is_market,
                    trigger_px: float_to_wire(trigger_px),
                    tpsl,
                },
            },
            c: o.cloid.as_ref().map(cloid_buf),
        }
    }

    fn json(&self) -> Value {
        let mut m = serde_json::Map::new();
        m.insert("a".into(), json!(self.a));
        m.insert("b".into(), json!(self.b));
        m.insert("p".into(), json!(self.p));
        m.insert("s".into(), json!(self.s));
        m.insert("r".into(), json!(self.r));
        let t = match &self.t {
            WireType::Limit(tif) => json!({"limit": {"tif": tif_str(*tif)}}),
            WireType::Trigger {
                is_market,
                trigger_px,
                tpsl,
            } => json!({"trigger": {
                "isMarket": is_market,
                "triggerPx": trigger_px,
                "tpsl": tpsl_str(*tpsl),
            }}),
        };
        m.insert("t".into(), t);
        if let Some(c) = &self.c {
            m.insert("c".into(), json!(cloid_str(c)));
        }
        Value::Object(m)
    }

    fn pack(&self, w: &mut Vec<u8>) {
        mp_map(w, if self.c.is_some() { 7 } else { 6 });
        mp_str(w, "a");
        mp_uint(w, self.a as u64);
        mp_str(w, "b");
        mp_bool(w, self.b);
        mp_str(w, "p");
        mp_str(w, &self.p);
        mp_str(w, "s");
        mp_str(w, &self.s);
        mp_str(w, "r");
        mp_bool(w, self.r);
        mp_str(w, "t");
        mp_map(w, 1);
        match &self.t {
            WireType::Limit(tif) => {
                mp_str(w, "limit");
                mp_map(w, 1);
                mp_str(w, "tif");
                mp_str(w, tif_str(*tif));
            }
            WireType::Trigger {
                is_market,
                trigger_px,
                tpsl,
            } => {
                mp_str(w, "trigger");
                mp_map(w, 3);
                mp_str(w, "isMarket");
                mp_bool(w, *is_market);
                mp_str(w, "triggerPx");
                mp_str(w, trigger_px);
                mp_str(w, "tpsl");
                mp_str(w, tpsl_str(*tpsl));
            }
        }
        if let Some(c) = &self.c {
            mp_str(w, "c");
            mp_str(w, cloid_str(c));
        }
    }
}

// msgpack primitives matching Python `msgpack.packb` (smallest int width, str types). Writes to a Vec
// cannot fail.
fn mp_map(w: &mut Vec<u8>, n: u32) {
    rmp::encode::write_map_len(w, n).expect("vec write");
}
fn mp_arr(w: &mut Vec<u8>, n: u32) {
    rmp::encode::write_array_len(w, n).expect("vec write");
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

/// Action in wire form; built once per `sign` and encoded both as msgpack (for the hash) and JSON.
enum Wire {
    Order {
        orders: Vec<WireOrder>,
        priority: Option<u64>,
    },
    BatchModify {
        modifies: Vec<(WireRef, WireOrder)>,
    },
    Modify {
        oid: WireRef,
        order: WireOrder,
        always_place: bool,
    },
    Cancel {
        cancels: Vec<(u32, u64)>,
        fast: bool,
    },
    CancelByCloid {
        cancels: Vec<(u32, [u8; 34])>,
        fast: bool,
    },
    OrderTyped {
        orders: Vec<WireTypedOrder>,
        grouping: Grouping,
    },
    UpdateLeverage {
        asset: u32,
        is_cross: bool,
        leverage: u32,
    },
}

enum WireRef {
    Oid(u64),
    Cloid([u8; 34]),
}

fn wire_ref(r: &OrderRef) -> WireRef {
    match r {
        OrderRef::Oid(oid) => WireRef::Oid(*oid),
        OrderRef::Cloid(c) => WireRef::Cloid(cloid_buf(c)),
    }
}

impl WireRef {
    fn json(&self) -> Value {
        match self {
            WireRef::Oid(n) => json!(n),
            WireRef::Cloid(c) => json!(cloid_str(c)),
        }
    }

    fn pack(&self, w: &mut Vec<u8>) {
        match self {
            WireRef::Oid(n) => mp_uint(w, *n),
            WireRef::Cloid(c) => mp_str(w, cloid_str(c)),
        }
    }
}

impl Wire {
    fn new(action: &Action) -> Self {
        match action {
            Action::Order { orders, priority } => Wire::Order {
                orders: orders.iter().map(WireOrder::new).collect(),
                priority: *priority,
            },
            Action::BatchModify { modifies } => Wire::BatchModify {
                modifies: modifies
                    .iter()
                    .map(|(r, o)| (wire_ref(r), WireOrder::new(o)))
                    .collect(),
            },
            Action::Modify {
                oid,
                order,
                always_place,
            } => Wire::Modify {
                oid: wire_ref(oid),
                order: WireOrder::new(order),
                always_place: *always_place,
            },
            Action::Cancel { cancels, fast } => Wire::Cancel {
                cancels: cancels.clone(),
                fast: *fast,
            },
            Action::CancelByCloid { cancels, fast } => Wire::CancelByCloid {
                cancels: cancels.iter().map(|(a, c)| (*a, cloid_buf(c))).collect(),
                fast: *fast,
            },
            Action::OrderTyped { orders, grouping } => Wire::OrderTyped {
                orders: orders.iter().map(WireTypedOrder::new).collect(),
                grouping: *grouping,
            },
            Action::UpdateLeverage {
                asset,
                is_cross,
                leverage,
            } => Wire::UpdateLeverage {
                asset: *asset,
                is_cross: *is_cross,
                leverage: *leverage,
            },
        }
    }

    fn json(&self) -> Value {
        let mut m = serde_json::Map::new();
        match self {
            Wire::Order { orders, priority } => {
                m.insert("type".into(), json!("order"));
                m.insert(
                    "orders".into(),
                    Value::Array(orders.iter().map(WireOrder::json).collect()),
                );
                let g = match priority {
                    Some(p) => json!({ "p": p }),
                    None => json!("na"),
                };
                m.insert("grouping".into(), g);
            }
            Wire::BatchModify { modifies } => {
                m.insert("type".into(), json!("batchModify"));
                let v = modifies
                    .iter()
                    .map(|(r, o)| json!({"oid": r.json(), "order": o.json()}))
                    .collect();
                m.insert("modifies".into(), Value::Array(v));
            }
            Wire::Modify {
                oid,
                order,
                always_place,
            } => {
                m.insert("type".into(), json!("modify"));
                m.insert("oid".into(), oid.json());
                m.insert("order".into(), order.json());
                if *always_place {
                    m.insert("a".into(), json!(true));
                }
            }
            Wire::Cancel { cancels, fast } => {
                m.insert("type".into(), json!("cancel"));
                let v = cancels
                    .iter()
                    .map(|(a, o)| json!({"a": a, "o": o}))
                    .collect();
                m.insert("cancels".into(), Value::Array(v));
                if *fast {
                    m.insert("f".into(), json!(true));
                }
            }
            Wire::CancelByCloid { cancels, fast } => {
                m.insert("type".into(), json!("cancelByCloid"));
                let v = cancels
                    .iter()
                    .map(|(a, c)| json!({"asset": a, "cloid": cloid_str(c)}))
                    .collect();
                m.insert("cancels".into(), Value::Array(v));
                if *fast {
                    m.insert("f".into(), json!(true));
                }
            }
            Wire::OrderTyped { orders, grouping } => {
                m.insert("type".into(), json!("order"));
                m.insert(
                    "orders".into(),
                    Value::Array(orders.iter().map(WireTypedOrder::json).collect()),
                );
                m.insert("grouping".into(), json!(grouping_str(*grouping)));
            }
            Wire::UpdateLeverage {
                asset,
                is_cross,
                leverage,
            } => {
                m.insert("type".into(), json!("updateLeverage"));
                m.insert("asset".into(), json!(asset));
                m.insert("isCross".into(), json!(is_cross));
                m.insert("leverage".into(), json!(leverage));
            }
        }
        Value::Object(m)
    }

    fn pack(&self, w: &mut Vec<u8>) {
        match self {
            Wire::Order { orders, priority } => {
                mp_map(w, 3);
                mp_str(w, "type");
                mp_str(w, "order");
                mp_str(w, "orders");
                mp_arr(w, orders.len() as u32);
                for o in orders {
                    o.pack(w);
                }
                mp_str(w, "grouping");
                match priority {
                    Some(p) => {
                        mp_map(w, 1);
                        mp_str(w, "p");
                        mp_uint(w, *p);
                    }
                    None => mp_str(w, "na"),
                }
            }
            Wire::BatchModify { modifies } => {
                mp_map(w, 2);
                mp_str(w, "type");
                mp_str(w, "batchModify");
                mp_str(w, "modifies");
                mp_arr(w, modifies.len() as u32);
                for (r, o) in modifies {
                    mp_map(w, 2);
                    mp_str(w, "oid");
                    r.pack(w);
                    mp_str(w, "order");
                    o.pack(w);
                }
            }
            Wire::Modify {
                oid,
                order,
                always_place,
            } => {
                mp_map(w, if *always_place { 4 } else { 3 });
                mp_str(w, "type");
                mp_str(w, "modify");
                mp_str(w, "oid");
                oid.pack(w);
                mp_str(w, "order");
                order.pack(w);
                if *always_place {
                    mp_str(w, "a");
                    mp_bool(w, true);
                }
            }
            Wire::Cancel { cancels, fast } => {
                mp_map(w, if *fast { 3 } else { 2 });
                mp_str(w, "type");
                mp_str(w, "cancel");
                mp_str(w, "cancels");
                mp_arr(w, cancels.len() as u32);
                for (a, o) in cancels {
                    mp_map(w, 2);
                    mp_str(w, "a");
                    mp_uint(w, *a as u64);
                    mp_str(w, "o");
                    mp_uint(w, *o);
                }
                if *fast {
                    mp_str(w, "f");
                    mp_bool(w, true);
                }
            }
            Wire::CancelByCloid { cancels, fast } => {
                mp_map(w, if *fast { 3 } else { 2 });
                mp_str(w, "type");
                mp_str(w, "cancelByCloid");
                mp_str(w, "cancels");
                mp_arr(w, cancels.len() as u32);
                for (a, c) in cancels {
                    mp_map(w, 2);
                    mp_str(w, "asset");
                    mp_uint(w, *a as u64);
                    mp_str(w, "cloid");
                    mp_str(w, cloid_str(c));
                }
                if *fast {
                    mp_str(w, "f");
                    mp_bool(w, true);
                }
            }
            Wire::OrderTyped { orders, grouping } => {
                mp_map(w, 3);
                mp_str(w, "type");
                mp_str(w, "order");
                mp_str(w, "orders");
                mp_arr(w, orders.len() as u32);
                for o in orders {
                    o.pack(w);
                }
                mp_str(w, "grouping");
                mp_str(w, grouping_str(*grouping));
            }
            Wire::UpdateLeverage {
                asset,
                is_cross,
                leverage,
            } => {
                mp_map(w, 4);
                mp_str(w, "type");
                mp_str(w, "updateLeverage");
                mp_str(w, "asset");
                mp_uint(w, *asset as u64);
                mp_str(w, "isCross");
                mp_bool(w, *is_cross);
                mp_str(w, "leverage");
                mp_uint(w, *leverage as u64);
            }
        }
    }
}

pub struct Signer {
    key: SigningKey,
    vault: Option<[u8; 20]>,
    vault_hex: Option<String>,
    /// keccak256 of the EIP-712 domain struct (constant).
    domain_sep: [u8; 32],
    /// keccak256("Agent(string source,bytes32 connectionId)").
    agent_type: [u8; 32],
    /// keccak256("a") mainnet / keccak256("b") testnet.
    source_hash: [u8; 32],
}

impl std::fmt::Debug for Signer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never print the key.
        f.debug_struct("Signer")
            .field("vault", &self.vault_hex)
            .finish_non_exhaustive()
    }
}

fn parse_hex_bytes<const N: usize>(s: &str, what: &str) -> anyhow::Result<[u8; N]> {
    let s = s.strip_prefix("0x").unwrap_or(s);
    let v = hex::decode(s).map_err(|_| anyhow!("{what}: invalid hex"))?;
    v.try_into()
        .map_err(|_| anyhow!("{what}: expected {N} bytes"))
}

impl Signer {
    pub fn from_hex(
        private_key_hex: &str,
        vault: Option<&str>,
        mainnet: bool,
    ) -> anyhow::Result<Signer> {
        let kb: [u8; 32] = parse_hex_bytes(private_key_hex.trim(), "private key")?;
        let key = SigningKey::from_bytes((&kb).into())
            .context("private key: not a valid secp256k1 scalar")?;
        let vault = match vault {
            Some(v) => Some(parse_hex_bytes::<20>(v.trim(), "vault address")?),
            None => None,
        };
        let vault_hex = vault.map(|v| format!("0x{}", hex::encode(v)));

        let mut dom = Vec::with_capacity(5 * 32);
        dom.extend_from_slice(&keccak(
            b"EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)",
        ));
        dom.extend_from_slice(&keccak(b"Exchange"));
        dom.extend_from_slice(&keccak(b"1"));
        let mut chain = [0u8; 32];
        chain[24..].copy_from_slice(&1337u64.to_be_bytes());
        dom.extend_from_slice(&chain);
        dom.extend_from_slice(&[0u8; 32]);

        Ok(Signer {
            key,
            vault,
            vault_hex,
            domain_sep: keccak(&dom),
            agent_type: keccak(b"Agent(string source,bytes32 connectionId)"),
            source_hash: keccak(if mainnet { b"a" } else { b"b" }),
        })
    }

    /// Lower-cased 0x address of the signing key (the API wallet).
    pub fn address(&self) -> String {
        let pt = self.key.verifying_key().to_encoded_point(false);
        let h = keccak(&pt.as_bytes()[1..]);
        format!("0x{}", hex::encode(&h[12..]))
    }

    /// The SDK's JSON form of `action` (what is sent as `payload.action`).
    pub fn action_json(action: &Action) -> Value {
        Wire::new(action).json()
    }

    fn connection_id_wire(&self, wire: &Wire, nonce: u64) -> [u8; 32] {
        let mut buf = Vec::with_capacity(256);
        wire.pack(&mut buf);
        buf.extend_from_slice(&nonce.to_be_bytes());
        match &self.vault {
            None => buf.push(0),
            Some(v) => {
                buf.push(1);
                buf.extend_from_slice(v);
            }
        }
        keccak(&buf)
    }

    pub fn connection_id(&self, action: &Action, nonce: u64) -> anyhow::Result<[u8; 32]> {
        Ok(self.connection_id_wire(&Wire::new(action), nonce))
    }

    fn digest(&self, connection_id: &[u8; 32]) -> [u8; 32] {
        let mut st = [0u8; 96];
        st[..32].copy_from_slice(&self.agent_type);
        st[32..64].copy_from_slice(&self.source_hash);
        st[64..].copy_from_slice(connection_id);
        let struct_hash = keccak(&st);
        let mut m = [0u8; 66];
        m[0] = 0x19;
        m[1] = 0x01;
        m[2..34].copy_from_slice(&self.domain_sep);
        m[34..].copy_from_slice(&struct_hash);
        keccak(&m)
    }

    /// Full `/exchange` payload: `{"action", "nonce", "signature": {r, s, v}, "vaultAddress"}`.
    pub fn sign(&self, action: &Action, nonce: u64) -> anyhow::Result<Value> {
        let wire = Wire::new(action);
        let cid = self.connection_id_wire(&wire, nonce);
        let digest = self.digest(&cid);
        let (sig, recid) = self
            .key
            .sign_prehash_recoverable(&digest)
            .context("signing failed")?;
        let bytes = sig.to_bytes();
        let v = 27 + recid.to_byte() as u64;
        if v > 28 {
            bail!("unexpected recovery id {v}");
        }
        Ok(json!({
            "action": wire.json(),
            "nonce": nonce,
            "signature": {
                "r": format!("0x{}", hex::encode(&bytes[..32])),
                "s": format!("0x{}", hex::encode(&bytes[32..])),
                "v": v,
            },
            "vaultAddress": self.vault_hex,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn float_wire() {
        assert_eq!(float_to_wire(3500.0), "3500");
        assert_eq!(float_to_wire(0.004201), "0.004201");
        assert_eq!(float_to_wire(187.45), "187.45");
        assert_eq!(float_to_wire(0.12), "0.12");
        assert_eq!(float_to_wire(-0.0), "0");
        assert_eq!(float_to_wire(0.0), "0");
        assert_eq!(float_to_wire(10.0), "10");
    }
}
