//! Stark-curve signing of Paradex auth requests and orders.
//!
//! Paradex signs off-chain messages as legacy (revision 0) Starknet typed data: every field is a
//! `felt`, struct hashes and the final message hash are Pedersen hash chains
//! (`h(h(h(0, a0), a1)..., n)`), type hashes are `starknet_keccak` of the encoded type, and
//!
//! `message_hash = H("StarkNet Message", H(StarkNetDomain), account, H(primary struct))`.
//!
//! The domain is `{name: "Paradex", chainId, version: "1"}` where `chainId` is the shortstring of
//! `starknet_chain_id` from `GET /v1/system/config`. Signatures are ECDSA over the Stark curve with
//! an RFC 6979 nonce seeded with 32, which is what the official SDK (paradex-py, through
//! starknet-crypto-py's `sign`) does, so signatures are deterministic. `tests/vectors.json` holds
//! hashes and signatures produced by paradex-py; `tests/vectors.rs` reproduces them exactly.
//!
//! Nothing here reads the environment, the clock or the network: keys, account, chain, timestamps
//! and expiries are passed in.

use anyhow::{anyhow, bail, ensure, Result};
use serde::{Deserialize, Serialize};
use sha3::{Digest, Keccak256};
use starknet_crypto::{get_public_key, rfc6979_generate_k, sign, verify, PedersenHasher};
use std::sync::LazyLock;

pub use starknet_crypto::Felt;

/// `starknet_chain_id` of `https://api.prod.paradex.trade/v1/system/config`.
pub const MAINNET_CHAIN_NAME: &str = "PRIVATE_SN_PARACLEAR_MAINNET";
/// `starknet_chain_id` of `https://api.testnet.paradex.trade/v1/system/config`.
pub const TESTNET_CHAIN_NAME: &str = "PRIVATE_SN_PARACLEAR_TESTNET";

/// Sizes and prices are signed as integers with this many decimals.
const QUANTUM_DECIMALS: i64 = 8;
/// Python's default `decimal` context precision, which `Decimal.scaleb` rounds to.
const DECIMAL_PRECISION: usize = 28;
/// Extra RFC 6979 entropy the SDK passes to `sign`.
const SIGN_SEED: u8 = 32;
/// The Stark field prime, big-endian.
const PRIME_BE: [u8; 32] = [
    0x08, 0, 0, 0, 0, 0, 0, 0x11, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0x01,
];

const DOMAIN_TYPE: &str = "StarkNetDomain(name:felt,chainId:felt,version:felt)";
const REQUEST_TYPE: &str =
    "Request(method:felt,path:felt,body:felt,timestamp:felt,expiration:felt)";
const ORDER_TYPE: &str =
    "Order(timestamp:felt,market:felt,side:felt,orderType:felt,size:felt,price:felt)";
const MODIFY_ORDER_TYPE: &str =
    "ModifyOrder(timestamp:felt,market:felt,side:felt,orderType:felt,size:felt,price:felt,id:felt)";

/// Order side. Serialises as `BUY` / `SELL`; signed as `1` / `2`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OrderSide {
    Buy,
    Sell,
}

impl OrderSide {
    pub fn as_str(self) -> &'static str {
        match self {
            OrderSide::Buy => "BUY",
            OrderSide::Sell => "SELL",
        }
    }

    fn chain_value(self) -> u8 {
        match self {
            OrderSide::Buy => 1,
            OrderSide::Sell => 2,
        }
    }
}

/// Order type as Paradex lists it. The name is signed as a shortstring.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OrderType {
    Market,
    Limit,
    StopLimit,
    StopMarket,
    TakeProfitLimit,
    TakeProfitMarket,
    StopLossMarket,
    StopLossLimit,
}

impl OrderType {
    pub fn as_str(self) -> &'static str {
        match self {
            OrderType::Market => "MARKET",
            OrderType::Limit => "LIMIT",
            OrderType::StopLimit => "STOP_LIMIT",
            OrderType::StopMarket => "STOP_MARKET",
            OrderType::TakeProfitLimit => "TAKE_PROFIT_LIMIT",
            OrderType::TakeProfitMarket => "TAKE_PROFIT_MARKET",
            OrderType::StopLossMarket => "STOP_LOSS_MARKET",
            OrderType::StopLossLimit => "STOP_LOSS_LIMIT",
        }
    }
}

/// The signed fields of an order (`Order` typed data; `ModifyOrder` adds the order id).
///
/// `size` and `price` are plain decimal strings (`digits[.digits]`). `price` must be `None` for
/// `MARKET` (signed as 0) and `Some` for every other type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OrderMessage<'a> {
    /// Signature timestamp, unix ms. Paradex uses it as the order nonce.
    pub timestamp_ms: u64,
    pub market: &'a str,
    pub side: OrderSide,
    pub order_type: OrderType,
    pub size: &'a str,
    pub price: Option<&'a str>,
}

/// Stark ECDSA signature.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Signature {
    pub r: Felt,
    pub s: Felt,
}

impl Signature {
    /// `["r","s"]` with decimal values: the form Paradex takes in `PARADEX-STARKNET-SIGNATURE` and
    /// in the order `signature` field (paradex-py `flatten_signature`).
    pub fn flattened(&self) -> String {
        format!("[\"{}\",\"{}\"]", self.r, self.s)
    }
}

/// A Paradex Stark key bound to an account and chain.
///
/// `account` is the Paradex (L2) account address. With a subkey, `private_key` is the subkey and
/// `account` the main account it is registered to.
pub struct ParadexSigner {
    private_key: Felt,
    public_key: Felt,
    account: Felt,
    chain_id: Felt,
    domain_hash: Felt,
}

impl ParadexSigner {
    pub fn new(private_key: Felt, account: Felt, chain_id: Felt) -> Result<ParadexSigner> {
        ensure!(private_key != Felt::ZERO, "private key is zero");
        Ok(ParadexSigner {
            private_key,
            public_key: get_public_key(&private_key),
            account,
            chain_id,
            domain_hash: domain_hash(chain_id),
        })
    }

    /// `private_key` and `account` as `0x` hex; `chain_name` as in `/v1/system/config`
    /// (`MAINNET_CHAIN_NAME`, `TESTNET_CHAIN_NAME`).
    pub fn from_hex(private_key: &str, account: &str, chain_name: &str) -> Result<ParadexSigner> {
        ParadexSigner::new(
            felt_from_hex(private_key)?,
            felt_from_hex(account)?,
            chain_id_from_name(chain_name)?,
        )
    }

    pub fn public_key(&self) -> Felt {
        self.public_key
    }

    /// The public key as `0x` lowercase hex without leading zeros (Python `hex()`).
    pub fn public_key_hex(&self) -> String {
        format!("{:#x}", self.public_key)
    }

    pub fn account(&self) -> Felt {
        self.account
    }

    /// The account address as `0x` lowercase hex without leading zeros (Python `hex()`), the form
    /// of the `PARADEX-STARKNET-ACCOUNT` header.
    pub fn account_hex(&self) -> String {
        format!("{:#x}", self.account)
    }

    pub fn chain_id(&self) -> Felt {
        self.chain_id
    }

    /// Hash of the `POST /v1/auth` request message.
    pub fn auth_message_hash(&self, timestamp_s: u64, expiry_s: u64) -> Felt {
        message_hash(
            self.domain_hash,
            self.account,
            auth_struct_hash(timestamp_s, expiry_s),
        )
    }

    pub fn order_message_hash(&self, order: &OrderMessage<'_>) -> Result<Felt> {
        Ok(message_hash(
            self.domain_hash,
            self.account,
            order_struct_hash(order, None)?,
        ))
    }

    pub fn modify_order_message_hash(
        &self,
        order: &OrderMessage<'_>,
        order_id: &str,
    ) -> Result<Felt> {
        Ok(message_hash(
            self.domain_hash,
            self.account,
            order_struct_hash(order, Some(order_id))?,
        ))
    }

    pub fn sign_hash(&self, hash: &Felt) -> Result<Signature> {
        sign_hash(&self.private_key, hash)
    }

    pub fn sign_auth(&self, timestamp_s: u64, expiry_s: u64) -> Result<Signature> {
        self.sign_hash(&self.auth_message_hash(timestamp_s, expiry_s))
    }

    pub fn sign_order(&self, order: &OrderMessage<'_>) -> Result<Signature> {
        self.sign_hash(&self.order_message_hash(order)?)
    }

    pub fn sign_modify_order(&self, order: &OrderMessage<'_>, order_id: &str) -> Result<Signature> {
        self.sign_hash(&self.modify_order_message_hash(order, order_id)?)
    }
}

/// Hash of the `POST /v1/auth` request message for `account` on `chain_id`.
pub fn auth_message_hash(chain_id: Felt, account: Felt, timestamp_s: u64, expiry_s: u64) -> Felt {
    message_hash(
        domain_hash(chain_id),
        account,
        auth_struct_hash(timestamp_s, expiry_s),
    )
}

/// Hash of the `Order` message for `account` on `chain_id`.
pub fn order_message_hash(chain_id: Felt, account: Felt, order: &OrderMessage<'_>) -> Result<Felt> {
    Ok(message_hash(
        domain_hash(chain_id),
        account,
        order_struct_hash(order, None)?,
    ))
}

/// Hash of the `ModifyOrder` message for `account` on `chain_id`.
pub fn modify_order_message_hash(
    chain_id: Felt,
    account: Felt,
    order: &OrderMessage<'_>,
    order_id: &str,
) -> Result<Felt> {
    Ok(message_hash(
        domain_hash(chain_id),
        account,
        order_struct_hash(order, Some(order_id))?,
    ))
}

/// ECDSA-sign `hash` with the RFC 6979 nonce paradex-py uses (seed 32).
pub fn sign_hash(private_key: &Felt, hash: &Felt) -> Result<Signature> {
    let k = rfc6979_generate_k(hash, private_key, Some(&Felt::from(SIGN_SEED)));
    let sig = sign(private_key, hash, &k).map_err(|e| anyhow!("stark sign: {e}"))?;
    Ok(Signature { r: sig.r, s: sig.s })
}

/// Check `signature` over `hash` against a Stark public key.
pub fn verify_signature(public_key: &Felt, hash: &Felt, signature: &Signature) -> Result<bool> {
    verify(public_key, hash, &signature.r, &signature.s).map_err(|e| anyhow!("stark verify: {e}"))
}

/// The Stark public key (x coordinate) of a private key.
pub fn public_key(private_key: &Felt) -> Result<Felt> {
    ensure!(*private_key != Felt::ZERO, "private key is zero");
    Ok(get_public_key(private_key))
}

/// Chain id felt from the `starknet_chain_id` name in `/v1/system/config` (its ASCII bytes as a
/// big-endian integer).
pub fn chain_id_from_name(name: &str) -> Result<Felt> {
    shortstring(name)
}

/// Cairo shortstring: at most 31 ASCII characters as a big-endian integer. `""` is 0.
pub fn shortstring(s: &str) -> Result<Felt> {
    ensure!(s.is_ascii(), "shortstring {s:?} is not ASCII");
    ensure!(
        s.len() <= 31,
        "shortstring {s:?} is longer than 31 characters"
    );
    let mut buf = [0u8; 32];
    buf[32 - s.len()..].copy_from_slice(s.as_bytes());
    Ok(Felt::from_bytes_be(&buf))
}

/// A typed-data `felt` field value as starknet-py's `parse_felt` reads a string: `0x` hex, else
/// decimal if every character is a digit, else a shortstring.
pub fn parse_felt(s: &str) -> Result<Felt> {
    if s.starts_with("0x") {
        felt_from_hex(s)
    } else if !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()) {
        felt_from_dec(s)
    } else {
        shortstring(s)
    }
}

/// `0x`-prefixed hex to a felt. Errors on values outside the field.
pub fn felt_from_hex(s: &str) -> Result<Felt> {
    let digits = s
        .strip_prefix("0x")
        .ok_or_else(|| anyhow!("{s:?}: expected 0x-prefixed hex"))?;
    ensure!(
        !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_hexdigit()),
        "{s:?}: invalid hex"
    );
    let digits = digits.trim_start_matches('0');
    ensure!(digits.len() <= 64, "{s:?}: larger than a felt");
    let mut buf = [0u8; 32];
    for (i, b) in digits.bytes().rev().enumerate() {
        let nibble = (b as char).to_digit(16).expect("checked hex digit") as u8;
        buf[31 - i / 2] |= if i % 2 == 0 { nibble } else { nibble << 4 };
    }
    felt_from_be(buf).map_err(|e| anyhow!("{s:?}: {e}"))
}

/// Decimal digits to a felt. Errors on values outside the field.
pub fn felt_from_dec(s: &str) -> Result<Felt> {
    ensure!(
        !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()),
        "{s:?}: invalid decimal integer"
    );
    let mut buf = [0u8; 32];
    for d in s.bytes() {
        let mut carry = u32::from(d - b'0');
        for byte in buf.iter_mut().rev() {
            let v = u32::from(*byte) * 10 + carry;
            *byte = (v & 0xff) as u8;
            carry = v >> 8;
        }
        ensure!(carry == 0, "{s:?}: larger than a felt");
    }
    felt_from_be(buf).map_err(|e| anyhow!("{s:?}: {e}"))
}

fn felt_from_be(buf: [u8; 32]) -> Result<Felt> {
    ensure!(buf < PRIME_BE, "not below the field prime");
    Ok(Felt::from_bytes_be(&buf))
}

/// `int(Decimal(value).scaleb(8))` as decimal digits: the value in units of 1e-8, truncated
/// toward zero. Like Python's default context, the scaled value is first rounded half-even to 28
/// significant digits, which only matters for inputs with more than 28 significant digits.
///
/// `value` must be `digits` or `digits.digits`.
pub fn quantize(value: &str) -> Result<String> {
    let (int_part, frac_part) = match value.split_once('.') {
        Some((i, f)) => (i, f),
        None => (value, ""),
    };
    ensure!(
        !int_part.is_empty()
            && int_part.bytes().all(|b| b.is_ascii_digit())
            && (!value.contains('.') || !frac_part.is_empty())
            && frac_part.bytes().all(|b| b.is_ascii_digit()),
        "{value:?}: expected a plain decimal (digits[.digits])"
    );
    let all: String = [int_part, frac_part].concat();
    let coeff = all.trim_start_matches('0');
    if coeff.is_empty() {
        return Ok("0".to_string());
    }
    let mut exp = QUANTUM_DECIMALS - frac_part.len() as i64;
    let mut digits: Vec<u8> = coeff.bytes().map(|b| b - b'0').collect();
    if digits.len() > DECIMAL_PRECISION {
        let rest = digits.split_off(DECIMAL_PRECISION);
        exp += rest.len() as i64;
        let round_up = match rest[0].cmp(&5) {
            std::cmp::Ordering::Greater => true,
            std::cmp::Ordering::Less => false,
            std::cmp::Ordering::Equal => {
                rest[1..].iter().any(|&d| d != 0) || digits[DECIMAL_PRECISION - 1] % 2 == 1
            }
        };
        if round_up {
            let mut i = digits.len();
            loop {
                if i == 0 {
                    digits.insert(0, 1);
                    break;
                }
                i -= 1;
                if digits[i] == 9 {
                    digits[i] = 0;
                } else {
                    digits[i] += 1;
                    break;
                }
            }
        }
    }
    let mut out: String = digits.iter().map(|&d| char::from(b'0' + d)).collect();
    if exp >= 0 {
        out.extend(std::iter::repeat_n('0', exp as usize));
    } else {
        let drop = (-exp) as usize;
        if drop >= out.len() {
            return Ok("0".to_string());
        }
        out.truncate(out.len() - drop);
    }
    let trimmed = out.trim_start_matches('0');
    Ok(if trimmed.is_empty() {
        "0".to_string()
    } else {
        trimmed.to_string()
    })
}

/// `get_selector_from_name`: keccak-256 truncated to 250 bits.
pub fn starknet_keccak(data: &[u8]) -> Felt {
    let mut digest = [0u8; 32];
    digest.copy_from_slice(&Keccak256::digest(data));
    digest[0] &= 0x03;
    Felt::from_bytes_be(&digest)
}

/// Type hashes of `StarkNetDomain`, `Request`, `Order`, `ModifyOrder`.
static TYPE_HASHES: LazyLock<[Felt; 4]> = LazyLock::new(|| {
    [
        starknet_keccak(DOMAIN_TYPE.as_bytes()),
        starknet_keccak(REQUEST_TYPE.as_bytes()),
        starknet_keccak(ORDER_TYPE.as_bytes()),
        starknet_keccak(MODIFY_ORDER_TYPE.as_bytes()),
    ]
});

fn hash_on_elements(elements: &[Felt]) -> Felt {
    let mut h = PedersenHasher::new();
    for e in elements {
        h.update(*e);
    }
    h.finalize()
}

fn const_shortstring(s: &str) -> Felt {
    shortstring(s).expect("constant shortstring")
}

fn domain_hash(chain_id: Felt) -> Felt {
    hash_on_elements(&[
        TYPE_HASHES[0],
        const_shortstring("Paradex"),
        chain_id,
        // "1" is numeric, so parse_felt reads it as the integer 1.
        Felt::ONE,
    ])
}

fn auth_struct_hash(timestamp_s: u64, expiry_s: u64) -> Felt {
    hash_on_elements(&[
        TYPE_HASHES[1],
        const_shortstring("POST"),
        const_shortstring("/v1/auth"),
        // body "" is the shortstring 0
        Felt::ZERO,
        Felt::from(timestamp_s),
        Felt::from(expiry_s),
    ])
}

fn order_struct_hash(order: &OrderMessage<'_>, order_id: Option<&str>) -> Result<Felt> {
    let price = match (order.order_type, order.price) {
        (OrderType::Market, None) => Felt::ZERO,
        (OrderType::Market, Some(_)) => bail!("MARKET orders are signed without a price"),
        (_, Some(p)) => felt_from_dec(&quantize(p)?)?,
        (t, None) => bail!("{} orders need a price", t.as_str()),
    };
    let mut fields = [Felt::ZERO; 8];
    fields[0] = TYPE_HASHES[if order_id.is_some() { 3 } else { 2 }];
    fields[1] = Felt::from(order.timestamp_ms);
    fields[2] = parse_felt(order.market)?;
    fields[3] = Felt::from(order.side.chain_value());
    fields[4] = parse_felt(order.order_type.as_str())?;
    fields[5] = felt_from_dec(&quantize(order.size)?)?;
    fields[6] = price;
    let n = match order_id {
        Some(id) => {
            fields[7] = parse_felt(id)?;
            8
        }
        None => 7,
    };
    Ok(hash_on_elements(&fields[..n]))
}

fn message_hash(domain_hash: Felt, account: Felt, struct_hash: Felt) -> Felt {
    hash_on_elements(&[
        const_shortstring("StarkNet Message"),
        domain_hash,
        account,
        struct_hash,
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quantize_truncates_and_rejects_non_plain_decimals() {
        assert_eq!(quantize("1").unwrap(), "100000000");
        assert_eq!(quantize("0.000000019").unwrap(), "1");
        assert_eq!(quantize("0.000000009").unwrap(), "0");
        assert_eq!(quantize("000.10").unwrap(), "10000000");
        assert_eq!(quantize("0").unwrap(), "0");
        for bad in ["", ".5", "1.", "-1", "1e3", " 1", "1.2.3", "+1"] {
            assert!(quantize(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn parse_felt_follows_starknet_py() {
        assert_eq!(parse_felt("").unwrap(), Felt::ZERO);
        assert_eq!(parse_felt("12").unwrap(), Felt::from(12u8));
        assert_eq!(parse_felt("0x0c").unwrap(), Felt::from(12u8));
        assert_eq!(parse_felt("A").unwrap(), Felt::from(0x41u8));
        assert!(
            parse_felt("0x0800000000000011000000000000000000000000000000000000000000000001")
                .is_err()
        );
        assert_eq!(
            parse_felt("0x0800000000000011000000000000000000000000000000000000000000000000")
                .unwrap(),
            felt_from_dec(
                "3618502788666131213697322783095070105623107215331596699973092056135872020480"
            )
            .unwrap()
        );
        assert!(felt_from_dec(
            "3618502788666131213697322783095070105623107215331596699973092056135872020481"
        )
        .is_err());
        assert!(shortstring("0123456789012345678901234567890123").is_err());
    }

    #[test]
    fn chain_ids() {
        assert_eq!(
            chain_id_from_name(MAINNET_CHAIN_NAME).unwrap(),
            felt_from_hex("0x505249564154455f534e5f50415241434c4541525f4d41494e4e4554").unwrap()
        );
    }
}
