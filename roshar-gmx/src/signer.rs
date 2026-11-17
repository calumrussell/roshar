//! EIP-712 signing of GMX express orders, byte-identical to `@gmx-io/sdk` `signPreparedOrder`
//! (viem `signTypedData` with a private key).
//!
//! The GMX API builds the typed data (`Batch` over `GmxBaseGelatoRelayRouter` v1); this module
//! hashes whatever typed data it is given (EIP-712 `encodeType` / `encodeData`) and signs the
//! digest with secp256k1 (RFC 6979, low-s). [`validate_order_typed_data`] performs the checks the
//! SDK runs before signing.

use crate::types::{GmxEip712Domain, GmxPreparedOrder, GmxTypedData, GmxTypedField};
use anyhow::{anyhow, bail, Context, Result};
use k256::ecdsa::SigningKey;
use serde_json::Value;
use sha3::{Digest, Keccak256};
use std::collections::{BTreeMap, BTreeSet};

/// `domain.name` of every GMX relay router.
pub const GMX_RELAY_DOMAIN_NAME: &str = "GmxBaseGelatoRelayRouter";
/// `domain.version` of every GMX relay router.
pub const GMX_RELAY_DOMAIN_VERSION: &str = "1";

// Relay routers a prepared order may be bound to (`domain.verifyingContract`), from
// `@gmx-io/sdk` 2.9.1 `configs/contracts` (GelatoRelayRouter, SubaccountGelatoRelayRouter,
// MultichainClaimsRouter, MultichainGlvRouter, MultichainGmRouter, MultichainOrderRouter,
// MultichainSubaccountRouter, MultichainTransferRouter). The docs' addresses page still lists the
// previous GelatoRelayRouter, which the SDK no longer accepts.

pub const ARBITRUM_RELAY_ROUTERS: &[&str] = &[
    "0xE6c871230F758F03b39D4Fb5ea3fc7B49E9dAc64",
    "0xC3969579D6ec54879F078c36F6B364cDbd1B557B",
    "0x7b2b62a4604417d0340Ed3A837BA66ad3a5eeAd7",
    "0xf9B01d45B2c5022429c3f745e5A2E700B02cEBeb",
    "0xa334b4e3f92a91e927f061494A95d84B7F0D134d",
    "0x5cFdc00811d0697D18b080f4E4fEB092a3b18671",
    "0xa2d9D6B0904929A1CD43c3762931Eb19ca1b4215",
    "0xC993eF170859DAE0241a3c12B8186e456Fa1c1B0",
];

pub const AVALANCHE_RELAY_ROUTERS: &[&str] = &[
    "0xE9f7E49F7304D36Aae146B5Af7cE6f9421Dac2CF",
    "0x4fe335118106542DeD0748Eebbf276dD92F386e3",
    "0xd9e34687F82602780a277F891f9a33B9A1F9291e",
    "0x13BF6A53C2aBc86b080d3E6f500d7c209F73dc82",
    "0xC7fC0698774b4C704aCD5521368dA5BeB397289E",
    "0x1c9d9BF1286Ea617a4eFbCf4f0B2b4A160269229",
    "0x96bE3cbFfbCeb1020ddfE5689807b8C20c3659C0",
    "0xcD11f8277Faca1c906cB13e2447cDC9fed1d3B39",
];

pub const MEGAETH_RELAY_ROUTERS: &[&str] = &[
    "0xd34f5ce2a26c6B67892ef633877e381Aba4f66c5",
    "0x5B8c1B498CCDDCAad36B5C81e48D773Ad338145D",
    "0xDA1C23829B8c9c0F5F4de04B763D64BDcb62BCC4",
    "0x23E7bC6069A0336F96ea2b3EB93d6A86CE432E16",
    "0xBF877cB8E1e8B82e23Eb7c64CfD941a2a739f94a",
    "0x61F8cA02B89360b5860aa6840E5c043b319DF693",
    "0x9456C4E4D7b443D040318DD1225c15D49ec14562",
    "0xd5F5F52A580787898B1c5491f48568bC9fAa9753",
];

pub const ARBITRUM_SEPOLIA_RELAY_ROUTERS: &[&str] = &[
    "0x0384909F08B93EB7B8Bf7c1a0b0B1c2F50233aea",
    "0x842826C68d52Cd54b2C2Dee539C84d5249e02CA0",
    "0xc79199213ca050ad02B07394eD2aE95257C1FF31",
    "0xb86aC18dEB4C16364820C5D7BEf76496B391bfc5",
    "0x29155BFBFDe3f3a191ae6999C83cF96c7d3DD0F4",
    "0xf41C2ad38b78Ab4661093947F58179CAaCf84FB5",
    "0x43998d11d3C2703022d213A0888320C477d09C0d",
    "0xA51c25bE642429e6B6316075a22f11DA6a2e57E8",
];

const DOMAIN_TYPE: &str =
    "EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)";
const DOMAIN_FIELDS: [(&str, &str); 4] = [
    ("name", "string"),
    ("version", "string"),
    ("chainId", "uint256"),
    ("verifyingContract", "address"),
];

pub fn keccak256(data: &[u8]) -> [u8; 32] {
    Keccak256::digest(data).into()
}

/// Intermediate and final hashes of a typed-data message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedDataHashes {
    pub primary_type: String,
    pub domain_separator: [u8; 32],
    pub struct_hash: [u8; 32],
    /// `keccak256(0x1901 || domain_separator || struct_hash)`: the signed digest.
    pub digest: [u8; 32],
}

/// The struct type no other struct references. The SDK's signer takes the first key of the
/// JSON `types` object; for GMX payloads (`Batch` first) both rules agree.
pub fn primary_type(types: &BTreeMap<String, Vec<GmxTypedField>>) -> Result<&str> {
    let referenced: BTreeSet<&str> = types
        .values()
        .flatten()
        .map(|f| base_type(&f.kind))
        .collect();
    let mut roots = types
        .keys()
        .map(String::as_str)
        .filter(|t| *t != "EIP712Domain" && !referenced.contains(t));
    let root = roots
        .next()
        .ok_or_else(|| anyhow!("typed data has no primary type"))?;
    if let Some(other) = roots.next() {
        bail!("typed data has several root types ({root}, {other})");
    }
    Ok(root)
}

/// EIP-712 hashes of `td`.
pub fn hash_typed_data(td: &GmxTypedData) -> Result<TypedDataHashes> {
    if let Some(declared) = td.types.get("EIP712Domain") {
        let expected = DOMAIN_FIELDS.iter().map(|(name, kind)| GmxTypedField {
            name: name.to_string(),
            kind: kind.to_string(),
        });
        if !declared.iter().cloned().eq(expected) {
            bail!("EIP712Domain type does not match the domain fields");
        }
    }
    let primary = primary_type(&td.types)?;
    let domain_separator = domain_separator(&td.domain)?;
    let struct_hash = hash_struct(primary, &td.message, &td.types)?;
    let mut buf = [0u8; 66];
    buf[0] = 0x19;
    buf[1] = 0x01;
    buf[2..34].copy_from_slice(&domain_separator);
    buf[34..].copy_from_slice(&struct_hash);
    Ok(TypedDataHashes {
        primary_type: primary.to_string(),
        domain_separator,
        struct_hash,
        digest: keccak256(&buf),
    })
}

pub fn domain_separator(domain: &GmxEip712Domain) -> Result<[u8; 32]> {
    let mut buf = Vec::with_capacity(5 * 32);
    buf.extend_from_slice(&keccak256(DOMAIN_TYPE.as_bytes()));
    buf.extend_from_slice(&keccak256(domain.name.as_bytes()));
    buf.extend_from_slice(&keccak256(domain.version.as_bytes()));
    buf.extend_from_slice(&u64_word(domain.chain_id));
    buf.extend_from_slice(&address_word(&domain.verifying_contract)?);
    Ok(keccak256(&buf))
}

/// `encodeType(name)`: the struct, then its dependencies sorted by name.
pub fn encode_type(name: &str, types: &BTreeMap<String, Vec<GmxTypedField>>) -> Result<String> {
    let mut deps = BTreeSet::new();
    collect_deps(name, types, &mut deps)?;
    deps.remove(name);
    let mut out = String::new();
    for t in std::iter::once(name).chain(deps.iter().map(String::as_str)) {
        let fields = types
            .get(t)
            .ok_or_else(|| anyhow!("unknown struct type {t}"))?;
        out.push_str(t);
        out.push('(');
        for (i, f) in fields.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            out.push_str(&f.kind);
            out.push(' ');
            out.push_str(&f.name);
        }
        out.push(')');
    }
    Ok(out)
}

fn collect_deps(
    name: &str,
    types: &BTreeMap<String, Vec<GmxTypedField>>,
    deps: &mut BTreeSet<String>,
) -> Result<()> {
    let fields = types
        .get(name)
        .ok_or_else(|| anyhow!("unknown struct type {name}"))?;
    for f in fields {
        let base = base_type(&f.kind);
        if types.contains_key(base) && deps.insert(base.to_string()) {
            collect_deps(base, types, deps)?;
        }
    }
    Ok(())
}

/// `hashStruct(name, value)`.
pub fn hash_struct(
    name: &str,
    value: &Value,
    types: &BTreeMap<String, Vec<GmxTypedField>>,
) -> Result<[u8; 32]> {
    let fields = types
        .get(name)
        .ok_or_else(|| anyhow!("unknown struct type {name}"))?;
    let obj = value
        .as_object()
        .ok_or_else(|| anyhow!("{name}: expected an object"))?;
    let mut buf = Vec::with_capacity(32 * (fields.len() + 1));
    buf.extend_from_slice(&keccak256(encode_type(name, types)?.as_bytes()));
    for f in fields {
        let v = obj
            .get(&f.name)
            .ok_or_else(|| anyhow!("{name}.{}: missing", f.name))?;
        let word = encode_value(&f.kind, v, types).with_context(|| format!("{name}.{}", f.name))?;
        buf.extend_from_slice(&word);
    }
    Ok(keccak256(&buf))
}

fn base_type(kind: &str) -> &str {
    match kind.find('[') {
        Some(i) => &kind[..i],
        None => kind,
    }
}

fn encode_value(
    kind: &str,
    v: &Value,
    types: &BTreeMap<String, Vec<GmxTypedField>>,
) -> Result<[u8; 32]> {
    if let Some(inner) = kind.strip_suffix(']') {
        let open = inner
            .rfind('[')
            .ok_or_else(|| anyhow!("bad array type {kind}"))?;
        let (elem, len) = (&inner[..open], &inner[open + 1..]);
        let items = v.as_array().ok_or_else(|| anyhow!("expected an array"))?;
        if !len.is_empty() {
            let n: usize = len
                .parse()
                .map_err(|_| anyhow!("bad array length in {kind}"))?;
            if items.len() != n {
                bail!("expected {n} items, got {}", items.len());
            }
        }
        let mut buf = Vec::with_capacity(32 * items.len());
        for item in items {
            buf.extend_from_slice(&encode_value(elem, item, types)?);
        }
        return Ok(keccak256(&buf));
    }
    if types.contains_key(kind) {
        return hash_struct(kind, v, types);
    }
    match kind {
        "string" => Ok(keccak256(
            v.as_str()
                .ok_or_else(|| anyhow!("expected a string"))?
                .as_bytes(),
        )),
        "bytes" => Ok(keccak256(&hex_bytes(str_of(v)?)?)),
        "bool" => {
            let mut w = [0u8; 32];
            w[31] = v.as_bool().ok_or_else(|| anyhow!("expected a bool"))? as u8;
            Ok(w)
        }
        "address" => address_word(str_of(v)?),
        _ => {
            if let Some(n) = kind.strip_prefix("bytes") {
                let n = bits_param(n, 1, 32, 1, kind)?;
                let b = hex_bytes(str_of(v)?)?;
                if b.len() != n {
                    bail!("{kind}: expected {n} bytes, got {}", b.len());
                }
                let mut w = [0u8; 32];
                w[..n].copy_from_slice(&b);
                Ok(w)
            } else if let Some(n) = kind.strip_prefix("uint") {
                let bits = bits_param(n, 8, 256, 8, kind)?;
                let (neg, mag) = parse_int(v)?;
                if neg && mag != [0u8; 32] {
                    bail!("{kind}: negative value");
                }
                if bit_len(&mag) > bits {
                    bail!("{kind}: value out of range");
                }
                Ok(mag)
            } else if let Some(n) = kind.strip_prefix("int") {
                let bits = bits_param(n, 8, 256, 8, kind)?;
                let (neg, mag) = parse_int(v)?;
                if !neg {
                    if bit_len(&mag) > bits - 1 {
                        bail!("{kind}: value out of range");
                    }
                    Ok(mag)
                } else {
                    // |x| <= 2^(bits-1)
                    let len = bit_len(&mag);
                    if len > bits || (len == bits && !is_power_of_two(&mag)) {
                        bail!("{kind}: value out of range");
                    }
                    Ok(twos_complement(&mag))
                }
            } else {
                bail!("unsupported EIP-712 type {kind}")
            }
        }
    }
}

/// `uint256`/`bytes32` size suffix: empty means the default (256 bits / unsupported for bytes).
fn bits_param(s: &str, min: usize, max: usize, step: usize, kind: &str) -> Result<usize> {
    if s.is_empty() && max == 256 {
        return Ok(256);
    }
    let n: usize = s
        .parse()
        .map_err(|_| anyhow!("unsupported EIP-712 type {kind}"))?;
    if n < min || n > max || !n.is_multiple_of(step) {
        bail!("unsupported EIP-712 type {kind}");
    }
    Ok(n)
}

fn str_of(v: &Value) -> Result<&str> {
    v.as_str().ok_or_else(|| anyhow!("expected a hex string"))
}

fn hex_bytes(s: &str) -> Result<Vec<u8>> {
    let h = s
        .strip_prefix("0x")
        .ok_or_else(|| anyhow!("hex value {s:?} lacks 0x"))?;
    hex::decode(h).map_err(|_| anyhow!("invalid hex {s:?}"))
}

fn u64_word(x: u64) -> [u8; 32] {
    let mut w = [0u8; 32];
    w[24..].copy_from_slice(&x.to_be_bytes());
    w
}

fn address_word(s: &str) -> Result<[u8; 32]> {
    let a = parse_address(s)?;
    let mut w = [0u8; 32];
    w[12..].copy_from_slice(&a);
    Ok(w)
}

/// Parses a 0x address. Mixed-case input must carry a valid EIP-55 checksum (viem `isAddress`).
pub fn parse_address(s: &str) -> Result<[u8; 20]> {
    let h = s
        .strip_prefix("0x")
        .ok_or_else(|| anyhow!("address {s:?} lacks 0x"))?;
    if h.len() != 40 {
        bail!("address {s:?}: expected 20 bytes");
    }
    let bytes: [u8; 20] = hex::decode(h)
        .map_err(|_| anyhow!("address {s:?}: invalid hex"))?
        .try_into()
        .map_err(|_| anyhow!("address {s:?}: expected 20 bytes"))?;
    let has_upper = h.bytes().any(|c| c.is_ascii_uppercase());
    let has_lower = h.bytes().any(|c| c.is_ascii_lowercase());
    if has_upper && has_lower && checksum_address(&bytes) != s {
        bail!("address {s:?}: bad EIP-55 checksum");
    }
    Ok(bytes)
}

/// EIP-55 checksummed `0x` address.
pub fn checksum_address(a: &[u8; 20]) -> String {
    let lower = hex::encode(a);
    let hash = keccak256(lower.as_bytes());
    let mut out = String::with_capacity(42);
    out.push_str("0x");
    for (i, c) in lower.chars().enumerate() {
        let nibble = (hash[i / 2] >> if i % 2 == 0 { 4 } else { 0 }) & 0xf;
        if c.is_ascii_alphabetic() && nibble >= 8 {
            out.push(c.to_ascii_uppercase());
        } else {
            out.push(c);
        }
    }
    out
}

/// Integer from a decimal string, `0x` hex string or JSON integer, as (negative, |x| big-endian).
fn parse_int(v: &Value) -> Result<(bool, [u8; 32])> {
    match v {
        Value::Number(n) => {
            if let Some(u) = n.as_u64() {
                Ok((false, u64_word(u)))
            } else if let Some(i) = n.as_i64() {
                Ok((true, u64_word(i.unsigned_abs())))
            } else {
                bail!("expected an integer, got {n}")
            }
        }
        Value::String(s) => {
            let (neg, digits) = match s.strip_prefix('-') {
                Some(rest) => (true, rest),
                None => (false, s.as_str()),
            };
            let mag = if let Some(h) = digits.strip_prefix("0x") {
                parse_hex_u256(h)?
            } else {
                parse_dec_u256(digits)?
            };
            Ok((neg, mag))
        }
        other => bail!("expected an integer, got {other}"),
    }
}

fn parse_dec_u256(s: &str) -> Result<[u8; 32]> {
    if s.is_empty() || !s.bytes().all(|c| c.is_ascii_digit()) {
        bail!("invalid integer {s:?}");
    }
    let mut w = [0u8; 32];
    for c in s.bytes() {
        let mut carry = (c - b'0') as u32;
        for byte in w.iter_mut().rev() {
            let x = *byte as u32 * 10 + carry;
            *byte = x as u8;
            carry = x >> 8;
        }
        if carry != 0 {
            bail!("integer {s:?} exceeds 256 bits");
        }
    }
    Ok(w)
}

fn parse_hex_u256(h: &str) -> Result<[u8; 32]> {
    let h = h.trim_start_matches('0');
    if h.len() > 64 {
        bail!("integer 0x{h} exceeds 256 bits");
    }
    let padded = format!("{h:0>64}");
    let b = hex::decode(&padded).map_err(|_| anyhow!("invalid hex integer 0x{h}"))?;
    let mut w = [0u8; 32];
    w.copy_from_slice(&b);
    Ok(w)
}

fn bit_len(w: &[u8; 32]) -> usize {
    for (i, b) in w.iter().enumerate() {
        if *b != 0 {
            return (32 - i) * 8 - b.leading_zeros() as usize;
        }
    }
    0
}

fn is_power_of_two(w: &[u8; 32]) -> bool {
    w.iter().map(|b| b.count_ones()).sum::<u32>() == 1
}

fn twos_complement(mag: &[u8; 32]) -> [u8; 32] {
    let mut out = [0u8; 32];
    let mut carry = 1u16;
    for i in (0..32).rev() {
        let x = (!mag[i]) as u16 + carry;
        out[i] = x as u8;
        carry = x >> 8;
    }
    out
}

/// The checks `@gmx-io/sdk` (`validateOrderTypedData`) runs on prepared express typed data
/// before signing:
///
/// - domain name `GmxBaseGelatoRelayRouter`, version `1`, `chainId == chain_id`;
/// - `verifyingContract` is one of `relay_routers` (e.g. [`ARBITRUM_RELAY_ROUTERS`]);
/// - `types` is not empty;
/// - every `createOrderParamsList[].addresses.receiver` and `.cancellationReceiver` is the zero
///   address, `signer_address`, or `account_address` (the main account when a subaccount signs).
pub fn validate_order_typed_data(
    td: &GmxTypedData,
    chain_id: u64,
    signer_address: &str,
    account_address: Option<&str>,
    relay_routers: &[&str],
) -> Result<()> {
    let d = &td.domain;
    if d.name != GMX_RELAY_DOMAIN_NAME {
        bail!(
            "EIP-712 domain name {:?}, expected {GMX_RELAY_DOMAIN_NAME:?}",
            d.name
        );
    }
    if d.version != GMX_RELAY_DOMAIN_VERSION {
        bail!(
            "EIP-712 domain version {:?}, expected {GMX_RELAY_DOMAIN_VERSION:?}",
            d.version
        );
    }
    if d.chain_id != chain_id {
        bail!("EIP-712 domain chainId {}, expected {chain_id}", d.chain_id);
    }
    let verifying = parse_address(&d.verifying_contract)?;
    let mut known = false;
    for r in relay_routers {
        if parse_address(r)? == verifying {
            known = true;
            break;
        }
    }
    if !known {
        bail!(
            "EIP-712 verifyingContract {} is not a known relay router for chain {chain_id}",
            d.verifying_contract
        );
    }
    if td.types.is_empty() {
        bail!("EIP-712 types are empty");
    }

    let mut allowed = vec![parse_address(signer_address)?];
    if let Some(a) = account_address {
        allowed.push(parse_address(a)?);
    }
    if let Some(Value::Array(orders)) = td.message.get("createOrderParamsList") {
        for order in orders {
            let Some(addresses) = order.get("addresses") else {
                continue;
            };
            for field in ["receiver", "cancellationReceiver"] {
                let Some(v) = addresses.get(field) else {
                    continue;
                };
                // The SDK skips falsy values (absent or empty string).
                if v.is_null() || v.as_str() == Some("") {
                    continue;
                }
                let a = parse_address(str_of(v)?)?;
                if a != [0u8; 20] && !allowed.contains(&a) {
                    bail!("order {field} {v} is neither the signer nor the account");
                }
            }
        }
    }
    Ok(())
}

/// A secp256k1 key that signs GMX typed data: the account's key, or a subaccount key that the
/// account has authorised on-chain.
pub struct GmxSigner {
    key: SigningKey,
    address: [u8; 20],
}

impl GmxSigner {
    /// From a 32-byte hex private key, with or without `0x`.
    pub fn from_hex(private_key: &str) -> Result<Self> {
        let h = private_key.trim();
        let h = h.strip_prefix("0x").unwrap_or(h);
        let kb: [u8; 32] = hex::decode(h)
            .map_err(|_| anyhow!("private key: invalid hex"))?
            .try_into()
            .map_err(|_| anyhow!("private key: expected 32 bytes"))?;
        let key = SigningKey::from_bytes((&kb).into())
            .context("private key: not a valid secp256k1 scalar")?;
        let pt = key.verifying_key().to_encoded_point(false);
        let h = keccak256(&pt.as_bytes()[1..]);
        let mut address = [0u8; 20];
        address.copy_from_slice(&h[12..]);
        Ok(Self { key, address })
    }

    /// EIP-55 checksummed address of the key.
    pub fn address(&self) -> String {
        checksum_address(&self.address)
    }

    /// 65-byte `r || s || v` signature (`v` 27/28) of a digest, as lowercase `0x` hex.
    pub fn sign_digest(&self, digest: &[u8; 32]) -> Result<String> {
        let (sig, recid) = self
            .key
            .sign_prehash_recoverable(digest)
            .context("signing failed")?;
        let mut out = [0u8; 65];
        out[..64].copy_from_slice(&sig.to_bytes());
        out[64] = 27 + recid.to_byte();
        Ok(format!("0x{}", hex::encode(out)))
    }

    /// Signs typed data as viem `signTypedData` does. Does not validate it: see
    /// [`Self::sign_prepared_order`].
    pub fn sign_typed_data(&self, td: &GmxTypedData) -> Result<String> {
        self.sign_digest(&hash_typed_data(td)?.digest)
    }

    /// `signPreparedOrder`: validates a prepared express order with
    /// [`validate_order_typed_data`] and signs its typed data. `account_address` is the main
    /// account when this key is a subaccount (or, as in the SDK, any additional allowed receiver).
    pub fn sign_prepared_order(
        &self,
        prepared: &GmxPreparedOrder,
        chain_id: u64,
        account_address: Option<&str>,
        relay_routers: &[&str],
    ) -> Result<String> {
        let payload = prepared
            .express()
            .ok_or_else(|| anyhow!("classic transactions are signed and sent by the wallet"))?;
        let td = &payload.typed_data;
        validate_order_typed_data(
            td,
            chain_id,
            &self.address(),
            account_address,
            relay_routers,
        )?;
        self.sign_typed_data(td)
    }
}
