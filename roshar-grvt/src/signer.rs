//! GRVT EIP-712 signing, byte-identical to GRVT's Python signers.
//!
//! Domain: `EIP712Domain(string name,string version,uint256 chainId)` with name `GRVT Exchange`,
//! version `0` and the GRVT L2 chain id (325 mainnet, 326 testnet). No verifying contract.
//!
//! - Orders: `Order(uint64 subAccountID,bool isMarket,uint8 timeInForce,bool postOnly,bool
//!   reduceOnly,OrderLeg[] legs,uint32 nonce,int64 expiration)` with `OrderLeg(uint256 assetID,
//!   uint64 contractSize,uint64 limitPrice,bool isBuyingContract)`. Orders with a builder use
//!   `OrderWithBuilderFee`, which adds `address builder,uint32 builderFee` before `nonce`
//!   (`grvt-pysdk` `grvt_raw_signing.sign_order`).
//! - Wallet login: `WalletLogin(address signer,uint32 nonce,int64 expiration)`
//!   (`builder-examples/python/wallet_login.py`).
//!
//! Signatures are deterministic (RFC 6979, low-s), as with `eth_account`, so they match the SDK
//! byte for byte.

use crate::types::{
    GrvtOrderLegRequest, GrvtOrderMetadataRequest, GrvtOrderRequest, GrvtSignatureRequest,
    GrvtTimeInForce, GrvtWalletLoginRequest,
};
use anyhow::{anyhow, bail, Context, Result};
use k256::ecdsa::SigningKey;
use sha3::{Digest, Keccak256};

/// GRVT L2 chain id used in the EIP-712 domain on mainnet.
pub const MAINNET_CHAIN_ID: u64 = 325;
/// GRVT L2 chain id used in the EIP-712 domain on testnet.
pub const TESTNET_CHAIN_ID: u64 = 326;
/// Limit prices are signed as `price * 10^9`.
pub const PRICE_DECIMALS: u32 = 9;
/// Builder fees (percent) are signed as `fee * 10^4`.
pub const BUILDER_FEE_DECIMALS: u32 = 4;

const DOMAIN_TYPE: &[u8] = b"EIP712Domain(string name,string version,uint256 chainId)";
const ORDER_LEG_TYPE: &[u8] =
    b"OrderLeg(uint256 assetID,uint64 contractSize,uint64 limitPrice,bool isBuyingContract)";
// EIP-712 `encodeType` appends referenced struct types, so both order types end in `OrderLeg(..)`.
const ORDER_TYPE: &[u8] = b"Order(uint64 subAccountID,bool isMarket,uint8 timeInForce,bool postOnly,bool reduceOnly,OrderLeg[] legs,uint32 nonce,int64 expiration)OrderLeg(uint256 assetID,uint64 contractSize,uint64 limitPrice,bool isBuyingContract)";
const ORDER_WITH_BUILDER_FEE_TYPE: &[u8] = b"OrderWithBuilderFee(uint64 subAccountID,bool isMarket,uint8 timeInForce,bool postOnly,bool reduceOnly,OrderLeg[] legs,address builder,uint32 builderFee,uint32 nonce,int64 expiration)OrderLeg(uint256 assetID,uint64 contractSize,uint64 limitPrice,bool isBuyingContract)";
const WALLET_LOGIN_TYPE: &[u8] = b"WalletLogin(address signer,uint32 nonce,int64 expiration)";

fn keccak(data: &[u8]) -> [u8; 32] {
    Keccak256::digest(data).into()
}

/// keccak256 of concatenated 32-byte words (EIP-712 `encodeData`).
fn keccak_concat(words: &[&[u8; 32]]) -> [u8; 32] {
    let mut h = Keccak256::new();
    for w in words {
        h.update(w.as_slice());
    }
    h.finalize().into()
}

fn word_u64(v: u64) -> [u8; 32] {
    let mut w = [0u8; 32];
    w[24..].copy_from_slice(&v.to_be_bytes());
    w
}

fn word_i64(v: i64) -> [u8; 32] {
    let mut w = if v < 0 { [0xffu8; 32] } else { [0u8; 32] };
    w[24..].copy_from_slice(&v.to_be_bytes());
    w
}

fn word_bool(v: bool) -> [u8; 32] {
    word_u64(v as u64)
}

fn word_address(a: &[u8; 20]) -> [u8; 32] {
    let mut w = [0u8; 32];
    w[12..].copy_from_slice(a);
    w
}

fn parse_hex(s: &str, what: &str) -> Result<Vec<u8>> {
    let s = s
        .strip_prefix("0x")
        .or_else(|| s.strip_prefix("0X"))
        .unwrap_or(s);
    if s.is_empty() {
        bail!("{what}: empty hex");
    }
    if s.len() % 2 == 1 {
        // `0x30501`-style odd-length hashes are valid integers; left-pad one nibble.
        let mut padded = String::with_capacity(s.len() + 1);
        padded.push('0');
        padded.push_str(s);
        return hex::decode(&padded).map_err(|_| anyhow!("{what}: invalid hex"));
    }
    hex::decode(s).map_err(|_| anyhow!("{what}: invalid hex"))
}

/// `uint256` word from an instrument hash such as `0x030501`.
fn word_asset_id(instrument_hash: &str) -> Result<[u8; 32]> {
    let bytes = parse_hex(instrument_hash, "instrument_hash")?;
    if bytes.len() > 32 {
        bail!("instrument_hash: more than 32 bytes");
    }
    let mut w = [0u8; 32];
    w[32 - bytes.len()..].copy_from_slice(&bytes);
    Ok(w)
}

/// Parses a 20-byte address (any case; the checksum is not enforced, as in `eth_account`).
pub fn parse_address(s: &str) -> Result<[u8; 20]> {
    let bytes = parse_hex(s, "address")?;
    bytes
        .try_into()
        .map_err(|_| anyhow!("address: expected 20 bytes"))
}

/// EIP-55 checksummed `0x` address.
pub fn checksum_address(a: &[u8; 20]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut lower = [0u8; 40];
    for (i, b) in a.iter().enumerate() {
        lower[2 * i] = HEX[(b >> 4) as usize];
        lower[2 * i + 1] = HEX[(b & 0xf) as usize];
    }
    let h = keccak(&lower);
    let mut out = String::with_capacity(42);
    out.push_str("0x");
    for (i, c) in lower.iter().enumerate() {
        let shift = if i % 2 == 0 { 4 } else { 0 };
        let nibble = (h[i / 2] >> shift) & 0xf;
        if c.is_ascii_alphabetic() && nibble >= 8 {
            out.push(c.to_ascii_uppercase() as char);
        } else {
            out.push(*c as char);
        }
    }
    out
}

/// `int(Decimal(value) * 10**decimals)` for a plain non-negative decimal string, as the SDK
/// computes contract sizes, limit prices and builder fees. The SDK truncates digits beyond
/// `decimals`; this rejects them instead, since the exchange checks the signed value against the
/// unsigned string and would reject the order.
pub fn scale_decimal(value: &str, decimals: u32) -> Result<u64> {
    let (int_part, frac_part) = match value.split_once('.') {
        Some((i, f)) => (i, f),
        None => (value, ""),
    };
    if int_part.is_empty() && frac_part.is_empty() {
        bail!("decimal {value:?}: empty");
    }
    if !int_part.bytes().all(|b| b.is_ascii_digit())
        || !frac_part.bytes().all(|b| b.is_ascii_digit())
    {
        bail!("decimal {value:?}: expected plain non-negative digits");
    }
    let d = decimals as usize;
    if frac_part.len() > d && frac_part[d..].bytes().any(|b| b != b'0') {
        bail!("decimal {value:?}: more than {decimals} fractional digits");
    }
    let mut acc: u128 = 0;
    let digits = int_part
        .bytes()
        .chain(frac_part.bytes().chain(std::iter::repeat(b'0')).take(d));
    for b in digits {
        acc = acc
            .checked_mul(10)
            .and_then(|a| a.checked_add((b - b'0') as u128))
            .ok_or_else(|| anyhow!("decimal {value:?}: overflow"))?;
    }
    u64::try_from(acc).map_err(|_| anyhow!("decimal {value:?} * 10^{decimals} exceeds u64"))
}

/// The value signed as `uint8 timeInForce` (the SDK's `SignTimeInForce`).
/// `RETAIL_PRICE_IMPROVEMENT` has no signing value in the SDK and is rejected.
pub fn sign_time_in_force(tif: GrvtTimeInForce) -> Result<u8> {
    Ok(match tif {
        GrvtTimeInForce::GoodTillTime => 1,
        GrvtTimeInForce::AllOrNone => 2,
        GrvtTimeInForce::ImmediateOrCancel => 3,
        GrvtTimeInForce::FillOrKill => 4,
        GrvtTimeInForce::RetailPriceImprovement => {
            bail!("RETAIL_PRICE_IMPROVEMENT has no signing value in grvt-pysdk")
        }
    })
}

/// One order leg. `instrument_hash` and `base_decimals` come from the instrument definition
/// (`GrvtInstrument`); `size` and `limit_price` are sent as given and signed scaled by
/// `10^base_decimals` and `10^9`.
#[derive(Debug, Clone)]
pub struct NewOrderLeg {
    pub instrument: String,
    pub instrument_hash: String,
    pub base_decimals: u32,
    pub size: String,
    /// `"0"` for market orders (except on `STABLE_PERP`, where it carries the slippage bound).
    pub limit_price: String,
    pub is_buying_asset: bool,
}

/// Builder attribution, signed via `OrderWithBuilderFee`.
#[derive(Debug, Clone)]
pub struct OrderBuilder {
    /// Builder main account address. Sent as given; signed as an `address`.
    pub address: String,
    /// Fee in percent (`"0.001"` = 0.001%), signed as `fee * 10^4`.
    pub fee: String,
}

/// An order to sign. Nonce and expiration are chosen by the caller.
#[derive(Debug, Clone)]
pub struct NewOrder {
    pub sub_account_id: u64,
    pub is_market: bool,
    pub time_in_force: GrvtTimeInForce,
    pub post_only: bool,
    pub reduce_only: bool,
    /// Sorted as the exchange requires (instrument/underlying/quote/expiry/strike).
    pub legs: Vec<NewOrderLeg>,
    /// Unsigned metadata. GRVT asks API clients to draw it from `[2^63, 2^64 - 1]`.
    pub client_order_id: u64,
    pub builder: Option<OrderBuilder>,
    /// Signature deconflicting key (uint32).
    pub nonce: u32,
    /// Signature expiry in unix nanoseconds, at most 30 days ahead.
    pub expiration_ns: i64,
}

/// `domainSeparator` for `chain_id`.
pub fn domain_separator(chain_id: u64) -> [u8; 32] {
    keccak_concat(&[
        &keccak(DOMAIN_TYPE),
        &keccak(b"GRVT Exchange"),
        &keccak(b"0"),
        &word_u64(chain_id),
    ])
}

fn legs_hash(legs: &[NewOrderLeg]) -> Result<[u8; 32]> {
    let leg_type = keccak(ORDER_LEG_TYPE);
    let mut h = Keccak256::new();
    for leg in legs {
        let size = scale_decimal(&leg.size, leg.base_decimals)
            .with_context(|| format!("leg {} size", leg.instrument))?;
        let price = scale_decimal(&leg.limit_price, PRICE_DECIMALS)
            .with_context(|| format!("leg {} limit_price", leg.instrument))?;
        h.update(keccak_concat(&[
            &leg_type,
            &word_asset_id(&leg.instrument_hash)?,
            &word_u64(size),
            &word_u64(price),
            &word_bool(leg.is_buying_asset),
        ]));
    }
    Ok(h.finalize().into())
}

/// EIP-712 `hashStruct` of the order (`Order` or `OrderWithBuilderFee`).
pub fn order_struct_hash(order: &NewOrder) -> Result<[u8; 32]> {
    if order.legs.is_empty() {
        bail!("order has no legs");
    }
    let legs = legs_hash(&order.legs)?;
    let tif = sign_time_in_force(order.time_in_force)?;
    let head: [[u8; 32]; 5] = [
        word_u64(order.sub_account_id),
        word_bool(order.is_market),
        word_u64(tif as u64),
        word_bool(order.post_only),
        word_bool(order.reduce_only),
    ];
    let nonce = word_u64(order.nonce as u64);
    let expiration = word_i64(order.expiration_ns);
    Ok(match &order.builder {
        None => keccak_concat(&[
            &keccak(ORDER_TYPE),
            &head[0],
            &head[1],
            &head[2],
            &head[3],
            &head[4],
            &legs,
            &nonce,
            &expiration,
        ]),
        Some(b) => {
            let builder = parse_address(&b.address).context("builder")?;
            let fee = scale_decimal(&b.fee, BUILDER_FEE_DECIMALS).context("builder fee")?;
            let fee = u32::try_from(fee).map_err(|_| anyhow!("builder fee exceeds uint32"))?;
            keccak_concat(&[
                &keccak(ORDER_WITH_BUILDER_FEE_TYPE),
                &head[0],
                &head[1],
                &head[2],
                &head[3],
                &head[4],
                &legs,
                &word_address(&builder),
                &word_u64(fee as u64),
                &nonce,
                &expiration,
            ])
        }
    })
}

/// EIP-712 `hashStruct` of `WalletLogin`.
pub fn wallet_login_struct_hash(signer: &[u8; 20], nonce: u32, expiration_ns: i64) -> [u8; 32] {
    keccak_concat(&[
        &keccak(WALLET_LOGIN_TYPE),
        &word_address(signer),
        &word_u64(nonce as u64),
        &word_i64(expiration_ns),
    ])
}

/// `keccak256(0x1901 || domainSeparator || structHash)`.
pub fn typed_data_digest(domain_separator: &[u8; 32], struct_hash: &[u8; 32]) -> [u8; 32] {
    let mut m = [0u8; 66];
    m[0] = 0x19;
    m[1] = 0x01;
    m[2..34].copy_from_slice(domain_separator);
    m[34..].copy_from_slice(struct_hash);
    keccak(&m)
}

/// A secp256k1 signing key bound to one GRVT chain id.
pub struct Signer {
    key: SigningKey,
    address: [u8; 20],
    address_str: String,
    chain_id: u64,
    domain_separator: [u8; 32],
}

/// `(r, s, v)` of an Ethereum signature, hex-encoded as the SDK sends them.
struct Rsv {
    r: String,
    s: String,
    v: u8,
}

impl Signer {
    /// `private_key_hex`: 32 bytes, with or without `0x`. `chain_id`: [`MAINNET_CHAIN_ID`] or
    /// [`TESTNET_CHAIN_ID`].
    pub fn from_hex(private_key_hex: &str, chain_id: u64) -> Result<Signer> {
        let bytes = parse_hex(private_key_hex.trim(), "private key")?;
        let kb: [u8; 32] = bytes
            .try_into()
            .map_err(|_| anyhow!("private key: expected 32 bytes"))?;
        let key = SigningKey::from_bytes((&kb).into())
            .context("private key: not a valid secp256k1 scalar")?;
        let pt = key.verifying_key().to_encoded_point(false);
        let h = keccak(&pt.as_bytes()[1..]);
        let mut address = [0u8; 20];
        address.copy_from_slice(&h[12..]);
        Ok(Signer {
            key,
            address,
            address_str: checksum_address(&address),
            chain_id,
            domain_separator: domain_separator(chain_id),
        })
    }

    /// EIP-55 checksummed signer address (what the SDK sends as `signature.signer`).
    pub fn address(&self) -> &str {
        &self.address_str
    }

    pub fn chain_id(&self) -> u64 {
        self.chain_id
    }

    pub fn domain_separator(&self) -> &[u8; 32] {
        &self.domain_separator
    }

    fn sign_digest(&self, digest: &[u8; 32]) -> Result<Rsv> {
        let (sig, recid) = self
            .key
            .sign_prehash_recoverable(digest)
            .context("signing failed")?;
        let bytes = sig.to_bytes();
        let v = 27 + recid.to_byte();
        if v > 28 {
            bail!("unexpected recovery id {v}");
        }
        Ok(Rsv {
            r: format!("0x{}", hex::encode(&bytes[..32])),
            s: format!("0x{}", hex::encode(&bytes[32..])),
            v,
        })
    }

    /// The EIP-712 digest that [`Signer::sign_order`] signs.
    pub fn order_digest(&self, order: &NewOrder) -> Result<[u8; 32]> {
        Ok(typed_data_digest(
            &self.domain_separator,
            &order_struct_hash(order)?,
        ))
    }

    /// Signs `order` and returns the `order` object for `create_order` / `bulk_orders`.
    /// As in the SDK, `signature.chain_id` is not sent (the exchange falls back to its own).
    pub fn sign_order(&self, order: &NewOrder) -> Result<GrvtOrderRequest> {
        let rsv = self.sign_digest(&self.order_digest(order)?)?;
        Ok(GrvtOrderRequest {
            sub_account_id: order.sub_account_id.to_string(),
            time_in_force: order.time_in_force,
            legs: order
                .legs
                .iter()
                .map(|l| GrvtOrderLegRequest {
                    instrument: l.instrument.clone(),
                    size: l.size.clone(),
                    is_buying_asset: l.is_buying_asset,
                    limit_price: l.limit_price.clone(),
                })
                .collect(),
            signature: GrvtSignatureRequest {
                signer: self.address_str.clone(),
                r: rsv.r,
                s: rsv.s,
                v: rsv.v,
                expiration: order.expiration_ns.to_string(),
                nonce: order.nonce,
                chain_id: None,
            },
            metadata: GrvtOrderMetadataRequest {
                client_order_id: order.client_order_id.to_string(),
            },
            is_market: order.is_market,
            post_only: order.post_only,
            reduce_only: order.reduce_only,
            builder: order.builder.as_ref().map(|b| b.address.clone()),
            builder_fee: order.builder.as_ref().map(|b| b.fee.clone()),
        })
    }

    /// The EIP-712 digest that [`Signer::sign_wallet_login`] signs.
    pub fn wallet_login_digest(&self, nonce: u32, expiration_ns: i64) -> [u8; 32] {
        typed_data_digest(
            &self.domain_separator,
            &wallet_login_struct_hash(&self.address, nonce, expiration_ns),
        )
    }

    /// Body for `POST /auth/wallet/login`. The exchange requires `expiration_ns` in the future and
    /// at most 5 minutes ahead, and each `(address, nonce)` pair once.
    pub fn sign_wallet_login(
        &self,
        nonce: u32,
        expiration_ns: i64,
    ) -> Result<GrvtWalletLoginRequest> {
        let rsv = self.sign_digest(&self.wallet_login_digest(nonce, expiration_ns))?;
        Ok(GrvtWalletLoginRequest {
            address: self.address_str.clone(),
            signature: GrvtSignatureRequest {
                signer: self.address_str.clone(),
                r: rsv.r,
                s: rsv.s,
                v: rsv.v,
                expiration: expiration_ns.to_string(),
                nonce,
                chain_id: Some(self.chain_id.to_string()),
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scale() {
        assert_eq!(scale_decimal("0.001", 9).unwrap(), 1_000_000);
        assert_eq!(scale_decimal("65038.1", 9).unwrap(), 65_038_100_000_000);
        assert_eq!(scale_decimal("10", 6).unwrap(), 10_000_000);
        assert_eq!(scale_decimal("0", 9).unwrap(), 0);
        assert_eq!(scale_decimal("1.50000000000", 9).unwrap(), 1_500_000_000);
        assert_eq!(scale_decimal(".5", 1).unwrap(), 5);
        assert!(scale_decimal("0.0000000001", 9).is_err());
        assert!(scale_decimal("-1", 9).is_err());
        assert!(scale_decimal("1e3", 9).is_err());
        assert!(scale_decimal("", 9).is_err());
        assert!(scale_decimal("18446744073.709551616", 9).is_err());
    }

    #[test]
    fn signed_words() {
        assert_eq!(word_i64(-1), [0xff; 32]);
        assert_eq!(word_i64(1)[31], 1);
        assert_eq!(word_asset_id("0x030501").unwrap()[29..], [0x03, 0x05, 0x01]);
        assert_eq!(word_asset_id("0x30501").unwrap()[29..], [0x03, 0x05, 0x01]);
    }

    #[test]
    fn eip55() {
        // EIP-55 test vector.
        let a = parse_address("0x5aaeb6053f3e94c9b9a09f33669435e7ef1beaed").unwrap();
        assert_eq!(
            checksum_address(&a),
            "0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed"
        );
    }
}
