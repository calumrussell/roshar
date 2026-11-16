//! Signing of the Solana transactions the Perps API returns.
//!
//! The API builds the whole transaction (`serializedTxBase64`): instructions, blockhash, fee
//! payer, and any signatures of its own (e.g. an ephemeral request account, already filled in).
//! The wallet adds its ed25519 signature over the message bytes in its own signature slot and
//! leaves the other slots as they are; the keeper fills its slot at `POST /transaction/execute`.
//! This matches `partiallySignTransactionWithSigners` in `@solana/kit`, which the official
//! Jupiter CLI uses.
//!
//! Wire format: `compact-u16 n` + `n * 64` signature bytes + message. The message starts with
//! `0x80 | version` for versioned messages (only v0 is accepted) and then the header
//! `[num_required_signatures, num_readonly_signed, num_readonly_unsigned]`, `compact-u16 k` and
//! `k * 32` account keys. Signer `i` is account key `i` for `i < num_required_signatures`.

use anyhow::{anyhow, bail, Context, Result};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;
use ed25519_dalek::{Signer as _, SigningKey};

/// A Solana wallet key.
pub struct Keypair {
    signing_key: SigningKey,
}

impl Keypair {
    /// From the 32-byte ed25519 secret seed.
    pub fn from_seed(seed: &[u8; 32]) -> Self {
        Self {
            signing_key: SigningKey::from_bytes(seed),
        }
    }

    /// From the 64-byte Solana keypair (`secret seed || public key`), as in a `solana-keygen`
    /// JSON file. The public half must match the secret.
    pub fn from_keypair_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != 64 {
            bail!("Solana keypair must be 64 bytes, got {}", bytes.len());
        }
        let seed: [u8; 32] = bytes[..32].try_into().expect("length checked");
        let keypair = Self::from_seed(&seed);
        if keypair.pubkey()[..] != bytes[32..] {
            bail!("Solana keypair: public half does not match the secret key");
        }
        Ok(keypair)
    }

    /// From the base58 64-byte keypair wallets export.
    pub fn from_base58(keypair_b58: &str) -> Result<Self> {
        let bytes = bs58::decode(keypair_b58)
            .into_vec()
            .context("Solana keypair is not valid base58")?;
        Self::from_keypair_bytes(&bytes)
    }

    pub fn pubkey(&self) -> [u8; 32] {
        self.signing_key.verifying_key().to_bytes()
    }

    /// The wallet address (base58 public key): `walletAddress` in API requests.
    pub fn address(&self) -> String {
        bs58::encode(self.pubkey()).into_string()
    }

    /// Signs a wire transaction in this key's signature slot. Fails if the key is not one of
    /// the message's required signers or the bytes are not a well-formed transaction.
    pub fn sign_transaction(&self, wire: &[u8]) -> Result<SignedTransaction> {
        let layout = parse_layout(wire)?;
        let pubkey = self.pubkey();
        let slot = layout
            .signer_keys
            .iter()
            .position(|key| *key == pubkey)
            .ok_or_else(|| {
                anyhow!(
                    "{} is not a required signer of this transaction",
                    self.address()
                )
            })?;
        let signature = self
            .signing_key
            .sign(&wire[layout.message_offset..])
            .to_bytes();
        let mut signed = wire.to_vec();
        let at = layout.signatures_offset + 64 * slot;
        signed[at..at + 64].copy_from_slice(&signature);
        Ok(SignedTransaction {
            wire: signed,
            signature,
        })
    }

    /// [`Self::sign_transaction`] on the API's `serializedTxBase64`.
    pub fn sign_transaction_base64(&self, serialized_tx_base64: &str) -> Result<SignedTransaction> {
        let wire = BASE64
            .decode(serialized_tx_base64)
            .context("serializedTxBase64 is not valid base64")?;
        self.sign_transaction(&wire)
    }
}

/// A transaction carrying this wallet's signature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedTransaction {
    /// Wire bytes with the signature in place.
    pub wire: Vec<u8>,
    /// This wallet's signature over the message.
    pub signature: [u8; 64],
}

impl SignedTransaction {
    /// `serializedTxBase64` for `POST /transaction/execute`.
    pub fn to_base64(&self) -> String {
        BASE64.encode(&self.wire)
    }

    /// This wallet's signature, base58. It is the transaction id when the wallet is the fee
    /// payer (signer 0), which it is for every Perps API transaction.
    pub fn signature_base58(&self) -> String {
        bs58::encode(self.signature).into_string()
    }
}

struct Layout {
    signatures_offset: usize,
    message_offset: usize,
    signer_keys: Vec<[u8; 32]>,
}

/// Solana's compact-u16 (1 to 3 bytes, 7 bits each, little-endian).
fn read_compact_u16(bytes: &[u8], offset: usize) -> Result<(usize, usize)> {
    let mut value = 0usize;
    for i in 0..3 {
        let byte = *bytes
            .get(offset + i)
            .ok_or_else(|| anyhow!("transaction truncated in a compact-u16"))?;
        value |= ((byte & 0x7f) as usize) << (7 * i);
        if byte & 0x80 == 0 {
            if value > u16::MAX as usize {
                bail!("compact-u16 out of range");
            }
            return Ok((value, offset + i + 1));
        }
    }
    bail!("compact-u16 longer than 3 bytes")
}

fn parse_layout(wire: &[u8]) -> Result<Layout> {
    let (num_signatures, signatures_offset) = read_compact_u16(wire, 0)?;
    let message_offset = signatures_offset + 64 * num_signatures;
    let message = wire
        .get(message_offset..)
        .filter(|m| !m.is_empty())
        .ok_or_else(|| anyhow!("transaction truncated before the message"))?;

    let mut at = 0;
    if message[0] & 0x80 != 0 {
        let version = message[0] & 0x7f;
        if version != 0 {
            bail!("unsupported transaction message version {version}");
        }
        at = 1;
    }
    let header = message
        .get(at..at + 3)
        .ok_or_else(|| anyhow!("transaction message truncated in the header"))?;
    let num_required_signatures = header[0] as usize;
    if num_required_signatures != num_signatures {
        bail!(
            "transaction has {num_signatures} signature slots but its message requires {num_required_signatures}"
        );
    }
    let (num_keys, keys_offset) = read_compact_u16(message, at + 3)?;
    if num_keys < num_required_signatures {
        bail!("transaction message has fewer account keys than required signers");
    }
    let keys = message
        .get(keys_offset..keys_offset + 32 * num_keys)
        .ok_or_else(|| anyhow!("transaction message truncated in the account keys"))?;
    let signer_keys: Vec<[u8; 32]> = keys
        .chunks_exact(32)
        .take(num_required_signatures)
        .map(|k| k.try_into().expect("32-byte chunk"))
        .collect();
    Ok(Layout {
        signatures_offset,
        message_offset,
        signer_keys,
    })
}
