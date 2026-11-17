//! zkLink L2 signing, as used by ApeX Omni order payloads.
//!
//! A port of `zklink_sdk`'s `ZkLinkSigner` (zkLinkProtocol/zklink_sdk, branch `apex-devnet`, the
//! build the `apexomni` Python SDK ships as `libzklink_sdk`) and the franklin-crypto pieces it calls
//! (zkLinkProtocol/franklin-crypto `3bafa7f3`): musig over Alt-BabyJubjub with a Rescue challenge.
//!
//! - Key: `sk = sha256(sha256(seeds))`, re-hashed until it is below the subgroup order.
//! - Public key (`l2Key`): `sk * G`, packed as `y` little-endian with the sign of `x` in the top bit.
//! - Signing `m` (at most 92 bytes): `h = rescue_hash_tx_msg(m)`; `k` = RFC6979-style HMAC-SHA256
//!   nonce from `sk` and `sha256(h)`; `R = k * G`; `e = rescue_challenge(pk.x || R.x || h)`;
//!   `s = k + e * sk`. The signature is `pack(R) || s` (little-endian), 64 bytes.
//!
//! Signatures are deterministic, so `tests/vectors.rs` compares them byte for byte with the SDK.

mod constants;
mod curve;
mod field;
mod rescue;

use anyhow::{anyhow, Context, Result};
use curve::{pack_point, Point};
use field::{geq, limbs_from_be, limbs_to_be, limbs_to_le, Fr, Fs};
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};

pub use rescue::MAX_TX_MSG_LEN;

type HmacSha256 = Hmac<Sha256>;

/// An ApeX Omni zk key, derived from the account's `seeds`.
pub struct ZkSigner {
    /// Canonical scalar, below the subgroup order.
    secret: [u64; 4],
    pk_x: Fr,
    pk_y: Fr,
}

impl std::fmt::Debug for ZkSigner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ZkSigner({})", self.l2_key_hex())
    }
}

impl ZkSigner {
    /// `ZkLinkSigner::new_from_seed`. `seeds` are the bytes ApeX calls `seeds` (the onboarding
    /// signature); the exchange requires at least 32.
    pub fn from_seeds(seeds: &[u8]) -> Result<Self> {
        if seeds.len() < 32 {
            return Err(anyhow!(
                "zk seeds must be at least 32 bytes, got {}",
                seeds.len()
            ));
        }
        let mut effective: [u8; 32] = Sha256::digest(seeds).into();
        let secret = loop {
            let raw: [u8; 32] = Sha256::digest(effective).into();
            let limbs = limbs_from_be(&raw);
            if !geq(&limbs, &Fs::MODULUS) {
                break limbs;
            }
            effective = raw;
        };
        let (pk_x, pk_y) = Point::generator().mul(&secret).to_affine();
        Ok(Self { secret, pk_x, pk_y })
    }

    /// `seeds` as hex, with or without `0x` (the form the SDK and web app export).
    pub fn from_seeds_hex(seeds: &str) -> Result<Self> {
        let raw = seeds.strip_prefix("0x").unwrap_or(seeds);
        let bytes = hex::decode(raw).context("zk seeds are not valid hex")?;
        Self::from_seeds(&bytes)
    }

    /// Packed public key, the account's `l2Key`.
    pub fn l2_key(&self) -> [u8; 32] {
        pack_point(self.pk_x, self.pk_y)
    }

    /// `l2Key` as `0x`-prefixed lowercase hex, as the API reports it.
    pub fn l2_key_hex(&self) -> String {
        format!("0x{}", hex::encode(self.l2_key()))
    }

    /// `get_public_key_hash`: the 20-byte Rescue hash of the public key.
    pub fn public_key_hash(&self) -> [u8; 20] {
        rescue::public_key_hash(self.pk_x, self.pk_y)
    }

    /// `ZkLinkSigner::sign_musig`. `msg` is at most [`MAX_TX_MSG_LEN`] bytes.
    pub fn sign_musig(&self, msg: &[u8]) -> Result<[u8; 64]> {
        if msg.len() > MAX_TX_MSG_LEN {
            return Err(anyhow!(
                "zk message is {} bytes; at most {MAX_TX_MSG_LEN} can be signed",
                msg.len()
            ));
        }
        let hashed = rescue::hash_tx_msg(msg);
        let nonce = deterministic_nonce(&self.secret, &hashed);
        let (r_x, r_y) = Point::generator().mul(&nonce).to_affine();
        let e = rescue::challenge(self.pk_x, r_x, &hashed);

        let to_fs =
            |limbs: [u64; 4]| Fs::from_canonical(limbs).expect("scalar below subgroup order");
        let s = to_fs(e)
            .mul(to_fs(self.secret))
            .add(to_fs(nonce))
            .to_canonical();

        let mut out = [0u8; 64];
        out[..32].copy_from_slice(&pack_point(r_x, r_y));
        out[32..].copy_from_slice(&limbs_to_le(&s));
        Ok(out)
    }

    /// [`Self::sign_musig`] as `0x`-prefixed hex, the form order payloads carry.
    pub fn sign_musig_hex(&self, msg: &[u8]) -> Result<String> {
        Ok(format!("0x{}", hex::encode(self.sign_musig(msg)?)))
    }
}

fn hmac(key: &[u8; 32], parts: &[&[u8]]) -> [u8; 32] {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC accepts any key length");
    for part in parts {
        mac.update(part);
    }
    mac.finalize().into_bytes().into()
}

/// franklin-crypto `Seed::deterministic_seed`, including its retry step: a rejected candidate
/// re-keys with `HMAC(key, v || v || 0x00)` and keeps `v`.
fn deterministic_nonce(secret: &[u64; 4], msg: &[u8; 32]) -> [u64; 4] {
    let h1: [u8; 32] = Sha256::digest(msg).into();
    let sk = limbs_to_be(secret);
    let mut v = [1u8; 32];
    let mut key = [0u8; 32];

    key = hmac(&key, &[&v, &[0x00], &sk, &h1]);
    v = hmac(&key, &[&v]);
    key = hmac(&key, &[&v, &[0x01], &sk, &h1]);
    v = hmac(&key, &[&v]);

    loop {
        let candidate = hmac(&key, &[&v]);
        let limbs = limbs_from_be(&candidate);
        if !geq(&limbs, &Fs::MODULUS) {
            return limbs;
        }
        key = hmac(&key, &[&v, &v, &[0x00]]);
    }
}
