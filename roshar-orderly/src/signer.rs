//! Orderly signing.
//!
//! - Request auth: ed25519 "Orderly key". The signed message is
//!   `{timestamp_ms}{METHOD}{path?query}{body}`; the signature goes in `orderly-signature` as
//!   standard base64 (the official Python SDK's encoding).
//! - WebSocket auth: the same key signs just `{timestamp_ms}`.
//! - Adding an Orderly key: the wallet signs EIP-712 `AddOrderlyKey` in the off-chain domain
//!   `{name "Orderly", version "1", chainId, verifyingContract 0xCcCC…cccC}`.
//!
//! Nothing here reads the clock: every timestamp and expiration is passed in.

use crate::request::PreparedRequest;
use crate::wire::PyObject;
use anyhow::{anyhow, bail, Context, Result};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;
use ed25519_dalek::Signer as _;
use sha3::{Digest, Keccak256};

/// `verifyingContract` of Orderly's off-chain EIP-712 domain (`Registration`, `AddOrderlyKey`).
pub const OFF_CHAIN_VERIFYING_CONTRACT: &str = "0xCcCCccccCCCCcCCCCCCcCcCccCcCCCcCcccccccC";

const ED25519_PREFIX: &str = "ed25519:";

fn keccak(data: &[u8]) -> [u8; 32] {
    Keccak256::digest(data).into()
}

fn parse_hex<const N: usize>(s: &str, what: &str) -> Result<[u8; N]> {
    let s = s.trim();
    let s = s.strip_prefix("0x").unwrap_or(s);
    let v = hex::decode(s).map_err(|_| anyhow!("{what}: invalid hex"))?;
    v.try_into()
        .map_err(|_| anyhow!("{what}: expected {N} bytes"))
}

/// The message Orderly signs for a REST request.
pub fn request_message(timestamp_ms: u64, method: &str, path: &str, body: Option<&str>) -> String {
    let body = body.unwrap_or("");
    let mut m = String::with_capacity(20 + method.len() + path.len() + body.len());
    m.push_str(&timestamp_ms.to_string());
    m.push_str(method);
    m.push_str(path);
    m.push_str(body);
    m
}

/// Headers of a signed REST request, in Orderly's names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderlyAuthHeaders {
    /// `orderly-timestamp`
    pub timestamp: String,
    /// `orderly-account-id`
    pub account_id: String,
    /// `orderly-key`
    pub key: String,
    /// `orderly-signature`
    pub signature: String,
}

/// An Orderly access key (ed25519).
pub struct OrderlyKey {
    key: ed25519_dalek::SigningKey,
    public_key: String,
}

impl std::fmt::Debug for OrderlyKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never print the secret.
        f.debug_struct("OrderlyKey")
            .field("public_key", &self.public_key)
            .finish_non_exhaustive()
    }
}

impl OrderlyKey {
    /// Parses an Orderly secret: base58 of the 32-byte ed25519 seed, with or without the
    /// `ed25519:` prefix. A 64-byte base58 keypair (seed || public key) is also accepted; its
    /// public half must match the seed.
    pub fn from_secret(secret: &str) -> Result<Self> {
        let s = secret.trim();
        let s = s.strip_prefix(ED25519_PREFIX).unwrap_or(s);
        let bytes = bs58::decode(s)
            .into_vec()
            .context("Orderly secret: invalid base58")?;
        let seed: [u8; 32] = match bytes.len() {
            32 | 64 => bytes[..32].try_into().expect("length checked"),
            n => bail!("Orderly secret: expected 32 or 64 bytes, got {n}"),
        };
        let key = Self::from_seed(seed);
        if bytes.len() == 64 && key.key.verifying_key().as_bytes()[..] != bytes[32..] {
            bail!("Orderly secret: public half does not match the seed");
        }
        Ok(key)
    }

    pub fn from_seed(seed: [u8; 32]) -> Self {
        let key = ed25519_dalek::SigningKey::from_bytes(&seed);
        let public_key = format!(
            "{ED25519_PREFIX}{}",
            bs58::encode(key.verifying_key().as_bytes()).into_string()
        );
        Self { key, public_key }
    }

    /// The public key as Orderly writes it: `ed25519:<base58>`. This is the `orderly-key` header
    /// and the `orderlyKey` registered with `add_orderly_key`.
    pub fn public_key(&self) -> &str {
        &self.public_key
    }

    /// ed25519 signature of `message`, standard base64.
    pub fn sign(&self, message: &[u8]) -> String {
        BASE64.encode(self.key.sign(message).to_bytes())
    }

    /// `orderly-signature` for `request` sent at `timestamp_ms`.
    pub fn sign_request(&self, timestamp_ms: u64, request: &PreparedRequest) -> String {
        let message = request_message(
            timestamp_ms,
            request.method.as_str(),
            &request.path,
            request.body.as_deref(),
        );
        self.sign(message.as_bytes())
    }

    /// All four auth headers for `request` sent at `timestamp_ms` by `account_id`.
    pub fn auth_headers(
        &self,
        account_id: &str,
        timestamp_ms: u64,
        request: &PreparedRequest,
    ) -> OrderlyAuthHeaders {
        OrderlyAuthHeaders {
            timestamp: timestamp_ms.to_string(),
            account_id: account_id.to_string(),
            key: self.public_key.clone(),
            signature: self.sign_request(timestamp_ms, request),
        }
    }

    /// Private websocket `auth` frame, as the SDK sends it:
    /// `{"id": id, "event": "auth", "params": {"orderly_key", "sign", "timestamp"}}`, where `sign`
    /// signs the decimal `timestamp_ms` alone.
    pub fn ws_auth_message(&self, id: &str, timestamp_ms: u64) -> String {
        let sign = self.sign(timestamp_ms.to_string().as_bytes());
        let mut params = PyObject::new();
        params
            .str("orderly_key", &self.public_key)
            .str("sign", &sign)
            .uint("timestamp", timestamp_ms);
        let mut msg = PyObject::new();
        msg.str("id", id)
            .str("event", "auth")
            .raw("params", &params.finish());
        msg.finish()
    }
}

/// Orderly account id for `address` registered under `broker_id`:
/// `keccak256(abi.encode(address, keccak256(broker_id)))`, lowercase `0x` hex.
pub fn account_id(address: &str, broker_id: &str) -> Result<String> {
    let addr: [u8; 20] = parse_hex(address, "address")?;
    let mut buf = [0u8; 64];
    buf[12..32].copy_from_slice(&addr);
    buf[32..].copy_from_slice(&keccak(broker_id.as_bytes()));
    Ok(format!("0x{}", hex::encode(keccak(&buf))))
}

/// EIP-55 checksummed form of a 20-byte address.
fn checksum_address(addr: &[u8; 20]) -> String {
    let lower = hex::encode(addr);
    let hash = keccak(lower.as_bytes());
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

fn u256_word(v: u64) -> [u8; 32] {
    let mut w = [0u8; 32];
    w[24..].copy_from_slice(&v.to_be_bytes());
    w
}

/// The `AddOrderlyKey` EIP-712 message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddOrderlyKey<'a> {
    pub broker_id: &'a str,
    /// Chain the wallet is connected to; must be one Orderly supports.
    pub chain_id: u64,
    /// `ed25519:<base58>` public key being added (`OrderlyKey::public_key`).
    pub orderly_key: &'a str,
    /// Comma-separated scopes, e.g. `read,trading`. Scopes are independent.
    pub scope: &'a str,
    /// Unix ms.
    pub timestamp: u64,
    /// Unix ms; at most 365 days after `timestamp`.
    pub expiration: u64,
}

/// A wallet-signed `POST /v1/orderly_key` request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedAddOrderlyKey {
    /// `0x` + r || s || v (v = 27/28).
    pub signature: String,
    /// The JSON request body.
    pub body: String,
}

/// An EVM wallet (secp256k1) signing Orderly's EIP-712 messages.
pub struct WalletSigner {
    key: k256::ecdsa::SigningKey,
    address: [u8; 20],
}

impl std::fmt::Debug for WalletSigner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WalletSigner")
            .field("address", &self.address())
            .finish_non_exhaustive()
    }
}

impl WalletSigner {
    /// 32-byte hex private key, with or without `0x`.
    pub fn from_hex(private_key: &str) -> Result<Self> {
        let kb: [u8; 32] = parse_hex(private_key, "wallet private key")?;
        let key = k256::ecdsa::SigningKey::from_bytes((&kb).into())
            .context("wallet private key: not a valid secp256k1 scalar")?;
        let point = key.verifying_key().to_encoded_point(false);
        let hash = keccak(&point.as_bytes()[1..]);
        let mut address = [0u8; 20];
        address.copy_from_slice(&hash[12..]);
        Ok(Self { key, address })
    }

    /// EIP-55 checksummed address.
    pub fn address(&self) -> String {
        checksum_address(&self.address)
    }

    fn domain_separator(chain_id: u64) -> Result<[u8; 32]> {
        let verifying: [u8; 20] = parse_hex(OFF_CHAIN_VERIFYING_CONTRACT, "verifying contract")?;
        let mut d = [0u8; 160];
        d[..32].copy_from_slice(&keccak(
            b"EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)",
        ));
        d[32..64].copy_from_slice(&keccak(b"Orderly"));
        d[64..96].copy_from_slice(&keccak(b"1"));
        d[96..128].copy_from_slice(&u256_word(chain_id));
        d[140..].copy_from_slice(&verifying);
        Ok(keccak(&d))
    }

    /// EIP-712 digest of `msg`.
    pub fn add_orderly_key_digest(msg: &AddOrderlyKey<'_>) -> Result<[u8; 32]> {
        let mut s = [0u8; 224];
        s[..32].copy_from_slice(&keccak(
            b"AddOrderlyKey(string brokerId,uint256 chainId,string orderlyKey,string scope,uint64 timestamp,uint64 expiration)",
        ));
        s[32..64].copy_from_slice(&keccak(msg.broker_id.as_bytes()));
        s[64..96].copy_from_slice(&u256_word(msg.chain_id));
        s[96..128].copy_from_slice(&keccak(msg.orderly_key.as_bytes()));
        s[128..160].copy_from_slice(&keccak(msg.scope.as_bytes()));
        s[160..192].copy_from_slice(&u256_word(msg.timestamp));
        s[192..].copy_from_slice(&u256_word(msg.expiration));
        let struct_hash = keccak(&s);
        let mut m = [0u8; 66];
        m[0] = 0x19;
        m[1] = 0x01;
        m[2..34].copy_from_slice(&Self::domain_separator(msg.chain_id)?);
        m[34..].copy_from_slice(&struct_hash);
        Ok(keccak(&m))
    }

    /// Signs `msg` and builds the `POST /v1/orderly_key` body:
    /// `{"message": {...}, "signature": "0x…", "userAddress": "0x…"}`.
    pub fn sign_add_orderly_key(&self, msg: &AddOrderlyKey<'_>) -> Result<SignedAddOrderlyKey> {
        let digest = Self::add_orderly_key_digest(msg)?;
        let (sig, recid) = self
            .key
            .sign_prehash_recoverable(&digest)
            .context("EIP-712 signing failed")?;
        let mut bytes = [0u8; 65];
        bytes[..64].copy_from_slice(&sig.to_bytes());
        bytes[64] = 27 + recid.to_byte();
        if bytes[64] > 28 {
            bail!("unexpected recovery id {}", recid.to_byte());
        }
        let signature = format!("0x{}", hex::encode(bytes));

        let mut message = PyObject::new();
        message
            .str("brokerId", msg.broker_id)
            .uint("chainId", msg.chain_id)
            .str("orderlyKey", msg.orderly_key)
            .str("scope", msg.scope)
            .uint("timestamp", msg.timestamp)
            .uint("expiration", msg.expiration);
        let mut body = PyObject::new();
        body.raw("message", &message.finish())
            .str("signature", &signature)
            .str("userAddress", &self.address());
        Ok(SignedAddOrderlyKey {
            signature,
            body: body.finish(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checksum_matches_eip55_examples() {
        // From EIP-55.
        for a in [
            "0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed",
            "0xfB6916095ca1df60bB79Ce92cE3Ea74c37c5d359",
            "0xdbF03B407c01E7cD3CBea99509d93f8DDDC8C6FB",
        ] {
            let raw: [u8; 20] = parse_hex(a, "a").unwrap();
            assert_eq!(checksum_address(&raw), a);
        }
    }
}
