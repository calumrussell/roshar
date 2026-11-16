//! Bullet transaction encoding and ed25519 signing.
//!
//! A transaction is the borsh encoding of the rollup's `Transaction::V0`. The signer signs the
//! borsh-encoded `TransactionSigningPayload::V0`: a version byte `0`, the unsigned transaction
//! (call, uniqueness, details, address override) and the 32-byte chain hash. This is the format
//! of the accounts hard fork (changelog 2026-09-28); the pre-fork encoding (`chain_id` in the
//! details, no version byte) is not produced.
//!
//! Types and discriminants mirror `bullet-exchange-interface` 0.16.3 (the crate the official
//! SDK signs with) and `GET /rollup/schema`. Only the `User` actions a trading account needs are
//! modelled. Bytes are checked against vectors from the official SDK (`tests/vectors.rs`).

use anyhow::{anyhow, bail, Result};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use ed25519_dalek::{Signer as _, SigningKey, VerifyingKey};
use rust_decimal::Decimal;

/// The chain hash a transaction is bound to (`chainHash` of `GET /fapi/v1/exchangeInfo`, or
/// `chain_hash` of `GET /rollup/schema`).
pub type ChainHash = [u8; 32];

/// A 32-byte account address (an ed25519 public key; base58 on the API).
pub type Address = [u8; 32];

/// Version byte of `TransactionSigningPayload::V0` and `Transaction::V0`.
const V0: u8 = 0;
/// `RuntimeCall::Exchange`.
const RUNTIME_CALL_EXCHANGE: u8 = 7;
/// `CallMessage::User`.
const CALL_MESSAGE_USER: u8 = 0;

/// Parses a `0x`-prefixed (or bare) 64-character hex chain hash.
pub fn parse_chain_hash(hex_str: &str) -> Result<ChainHash> {
    let raw = hex_str.strip_prefix("0x").unwrap_or(hex_str);
    let mut out = [0u8; 32];
    hex::decode_to_slice(raw, &mut out)
        .map_err(|e| anyhow!("invalid chain hash {hex_str}: {e}"))?;
    Ok(out)
}

/// `TxDetails.chain_hash_fragment`: the first eight bytes of the chain hash, little-endian.
pub fn chain_hash_fragment(chain_hash: &ChainHash) -> u64 {
    let mut b = [0u8; 8];
    b.copy_from_slice(&chain_hash[..8]);
    u64::from_le_bytes(b)
}

/// Decodes a base58 account address.
pub fn decode_address(address: &str) -> Result<Address> {
    let bytes = bs58::decode(address)
        .into_vec()
        .map_err(|e| anyhow!("invalid base58 address {address}: {e}"))?;
    bytes
        .try_into()
        .map_err(|v: Vec<u8>| anyhow!("address {address} is {} bytes, expected 32", v.len()))
}

/// Encodes a 32-byte address as base58.
pub fn encode_address(address: &Address) -> String {
    bs58::encode(address).into_string()
}

/// `UniquenessData`: replay protection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Uniqueness {
    /// Must be exactly the credential's current nonce + 1.
    Nonce(u64),
    Generation(u64),
    /// Any value unique per key within the rollup's window (the docs suggest millisecond time).
    Window(u64),
}

/// `TxDetails` without `chain_hash_fragment`, which is derived from the chain hash at signing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TxDetails {
    pub max_priority_fee_bips: u64,
    pub max_fee: u128,
    /// `[compute, storage]`; `None` defaults to the block limit.
    pub gas_limit: Option<[u64; 2]>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Bid,
    Ask,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderType {
    Limit,
    PostOnly,
    FillOrKill,
    ImmediateOrCancel,
    PostOnlySlide,
    PostOnlyFront,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerPriceCondition {
    Mark,
    Oracle,
    LastTrade,
}

/// One take-profit or stop-loss leg (`Tpsl`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tpsl {
    pub order_price: Decimal,
    pub trigger_price: Decimal,
    pub price_condition: TriggerPriceCondition,
    pub order_type: OrderType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TpslPair {
    pub tp: Option<Tpsl>,
    pub sl: Option<Tpsl>,
}

/// TP/SL reserved on a new order (`PendingTpslPair`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingTpslPair {
    pub tpsl_pair: TpslPair,
    /// The legs track the position size at trigger time.
    pub dynamic_size: bool,
}

/// `NewOrderArgs`. Prices and sizes are encoded with their scale as given (`50000.0` and `50000`
/// are different bytes), and must not be negative.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewOrderArgs {
    pub price: Decimal,
    pub size: Decimal,
    pub side: Side,
    pub order_type: OrderType,
    pub reduce_only: bool,
    pub client_order_id: Option<u64>,
    pub pending_tpsl_pair: Option<PendingTpslPair>,
}

/// `CancelOrderArgs`: by exchange order id, client order id, or both.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CancelOrderArgs {
    pub order_id: Option<u64>,
    pub client_order_id: Option<u64>,
}

/// `AmendOrderArgs`: cancel one order and place another, atomically.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AmendOrderArgs {
    pub cancel: CancelOrderArgs,
    pub place: NewOrderArgs,
}

/// `OrderIdKind`, used by `CancelAndPlaceOrders`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderIdKind {
    Server(u64),
    Client(u64),
}

/// The `UserAction` variants covered by this crate. `sub_account_index: None` acts on the main
/// account (the delegator's, for a delegate key).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UserAction {
    /// `UpdateMaxLeverage` (10).
    UpdateMaxLeverage {
        market_id: u16,
        max_leverage: u16,
        sub_account_index: Option<u8>,
    },
    /// `PlaceOrders` (20). `replace: true` cancels every order on the market first, atomically.
    PlaceOrders {
        market_id: u16,
        orders: Vec<NewOrderArgs>,
        replace: bool,
        sub_account_index: Option<u8>,
    },
    /// `AmendOrders` (21).
    AmendOrders {
        market_id: u16,
        orders: Vec<AmendOrderArgs>,
        sub_account_index: Option<u8>,
    },
    /// `CancelOrders` (22).
    CancelOrders {
        market_id: u16,
        orders: Vec<CancelOrderArgs>,
        sub_account_index: Option<u8>,
    },
    /// `CancelMarketOrders` (23): every order on one market.
    CancelMarketOrders {
        market_id: u16,
        sub_account_index: Option<u8>,
    },
    /// `CancelAllOrders` (29): every order on every market.
    CancelAllOrders { sub_account_index: Option<u8> },
    /// `CancelAndPlaceOrders` (30). Cancels of orders no longer resting are skipped.
    /// `expiry_timestamp` (unix µs, venue clock) stops the placed orders filling after it.
    CancelAndPlaceOrders {
        market_id: u16,
        cancels: Vec<OrderIdKind>,
        places: Vec<NewOrderArgs>,
        expiry_timestamp: Option<i64>,
        sub_account_index: Option<u8>,
    },
}

impl UserAction {
    /// `CallMessage::msg_type`, e.g. `"User/PlaceOrders"` (also the `e` of WS order results).
    pub fn msg_type(&self) -> &'static str {
        match self {
            Self::UpdateMaxLeverage { .. } => "User/UpdateMaxLeverage",
            Self::PlaceOrders { .. } => "User/PlaceOrders",
            Self::AmendOrders { .. } => "User/AmendOrders",
            Self::CancelOrders { .. } => "User/CancelOrders",
            Self::CancelMarketOrders { .. } => "User/CancelMarketOrders",
            Self::CancelAllOrders { .. } => "User/CancelAllOrders",
            Self::CancelAndPlaceOrders { .. } => "User/CancelAndPlaceOrders",
        }
    }

    fn encode(&self, w: &mut Writer) -> Result<()> {
        match self {
            Self::UpdateMaxLeverage {
                market_id,
                max_leverage,
                sub_account_index,
            } => {
                w.u8(10);
                w.u16(*market_id);
                w.u16(*max_leverage);
                w.opt_u8(*sub_account_index);
            }
            Self::PlaceOrders {
                market_id,
                orders,
                replace,
                sub_account_index,
            } => {
                w.u8(20);
                w.u16(*market_id);
                w.len(orders.len())?;
                for o in orders {
                    w.new_order(o)?;
                }
                w.bool(*replace);
                w.opt_u8(*sub_account_index);
            }
            Self::AmendOrders {
                market_id,
                orders,
                sub_account_index,
            } => {
                w.u8(21);
                w.u16(*market_id);
                w.len(orders.len())?;
                for a in orders {
                    w.cancel(&a.cancel);
                    w.new_order(&a.place)?;
                }
                w.opt_u8(*sub_account_index);
            }
            Self::CancelOrders {
                market_id,
                orders,
                sub_account_index,
            } => {
                w.u8(22);
                w.u16(*market_id);
                w.len(orders.len())?;
                for c in orders {
                    w.cancel(c);
                }
                w.opt_u8(*sub_account_index);
            }
            Self::CancelMarketOrders {
                market_id,
                sub_account_index,
            } => {
                w.u8(23);
                w.u16(*market_id);
                w.opt_u8(*sub_account_index);
            }
            Self::CancelAllOrders { sub_account_index } => {
                w.u8(29);
                w.opt_u8(*sub_account_index);
            }
            Self::CancelAndPlaceOrders {
                market_id,
                cancels,
                places,
                expiry_timestamp,
                sub_account_index,
            } => {
                w.u8(30);
                w.u16(*market_id);
                w.len(cancels.len())?;
                for c in cancels {
                    match c {
                        OrderIdKind::Server(id) => {
                            w.u8(0);
                            w.u64(*id);
                        }
                        OrderIdKind::Client(id) => {
                            w.u8(1);
                            w.u64(*id);
                        }
                    }
                }
                w.len(places.len())?;
                for o in places {
                    w.new_order(o)?;
                }
                match expiry_timestamp {
                    Some(t) => {
                        w.u8(1);
                        w.i64(*t);
                    }
                    None => w.u8(0),
                }
                w.opt_u8(*sub_account_index);
            }
        }
        Ok(())
    }
}

/// `UnsignedTransaction` with an exchange `User` call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsignedTransaction {
    pub action: UserAction,
    pub uniqueness: Uniqueness,
    pub details: TxDetails,
    /// Account to execute as. `None` executes as the signer's account (the delegator's, for a
    /// registered delegate key). `Some` is admitted only if that account authorised the key.
    pub address_override: Option<Address>,
}

impl UnsignedTransaction {
    /// Borsh `UnsignedTransaction`, with `chain_hash_fragment` taken from `chain_hash`.
    fn encode(&self, w: &mut Writer, chain_hash: &ChainHash) -> Result<()> {
        w.u8(RUNTIME_CALL_EXCHANGE);
        w.u8(CALL_MESSAGE_USER);
        self.action.encode(w)?;
        let (tag, value) = match self.uniqueness {
            Uniqueness::Nonce(v) => (0, v),
            Uniqueness::Generation(v) => (1, v),
            Uniqueness::Window(v) => (2, v),
        };
        w.u8(tag);
        w.u64(value);
        w.u64(self.details.max_priority_fee_bips);
        w.u128(self.details.max_fee);
        match self.details.gas_limit {
            Some([compute, storage]) => {
                w.u8(1);
                w.u64(compute);
                w.u64(storage);
            }
            None => w.u8(0),
        }
        w.u64(chain_hash_fragment(chain_hash));
        match &self.address_override {
            Some(a) => {
                w.u8(1);
                w.bytes(a);
            }
            None => w.u8(0),
        }
        Ok(())
    }

    /// The bytes that are signed: borsh `TransactionSigningPayload::V0`.
    pub fn signing_bytes(&self, chain_hash: &ChainHash) -> Result<Vec<u8>> {
        let mut w = Writer::default();
        w.u8(V0);
        self.encode(&mut w, chain_hash)?;
        w.bytes(chain_hash);
        Ok(w.0)
    }
}

/// A signed transaction: borsh `Transaction::V0` (`signature`, `pub_key`, unsigned transaction).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedTransaction {
    pub bytes: Vec<u8>,
    pub signature: [u8; 64],
    pub pub_key: [u8; 32],
}

impl SignedTransaction {
    /// Base64 of the wire bytes: the `body` of `POST /tx/submit` and the `tx` of WS order methods.
    pub fn to_base64(&self) -> String {
        BASE64.encode(&self.bytes)
    }
}

/// An ed25519 signing key: the account's own key or a registered delegate key.
pub struct Signer {
    key: SigningKey,
}

impl Signer {
    /// From the 32-byte ed25519 secret key.
    pub fn from_secret_bytes(secret: &[u8; 32]) -> Self {
        Self {
            key: SigningKey::from_bytes(secret),
        }
    }

    /// From a hex 32-byte secret key, with or without `0x`.
    pub fn from_hex(secret_hex: &str) -> Result<Self> {
        let raw = secret_hex.strip_prefix("0x").unwrap_or(secret_hex);
        let mut secret = [0u8; 32];
        hex::decode_to_slice(raw, &mut secret)
            .map_err(|e| anyhow!("invalid secret key hex: {e}"))?;
        Ok(Self::from_secret_bytes(&secret))
    }

    /// From a 64-byte Solana keypair (`solana-keygen` JSON array contents, or the decoded base58
    /// export): secret key then public key. The public half must match the secret.
    pub fn from_keypair_bytes(keypair: &[u8]) -> Result<Self> {
        let keypair: &[u8; 64] = keypair
            .try_into()
            .map_err(|_| anyhow!("keypair is {} bytes, expected 64", keypair.len()))?;
        let key =
            SigningKey::from_keypair_bytes(keypair).map_err(|e| anyhow!("invalid keypair: {e}"))?;
        Ok(Self { key })
    }

    pub fn public_key(&self) -> [u8; 32] {
        self.key.verifying_key().to_bytes()
    }

    /// Base58 address of the key (for a delegate, the delegate address, not the account's).
    pub fn address(&self) -> String {
        encode_address(&self.public_key())
    }

    /// Signs `tx` for `chain_hash` and returns the wire transaction.
    pub fn sign(
        &self,
        tx: &UnsignedTransaction,
        chain_hash: &ChainHash,
    ) -> Result<SignedTransaction> {
        let payload = tx.signing_bytes(chain_hash)?;
        let signature = self.key.sign(&payload).to_bytes();
        let pub_key = self.public_key();
        let mut w = Writer(Vec::with_capacity(payload.len() + 64));
        w.u8(V0);
        w.bytes(&signature);
        w.bytes(&pub_key);
        tx.encode(&mut w, chain_hash)?;
        Ok(SignedTransaction {
            bytes: w.0,
            signature,
            pub_key,
        })
    }
}

/// Verifies `signature` over `tx`'s signing bytes for `pub_key`.
pub fn verify(
    tx: &UnsignedTransaction,
    chain_hash: &ChainHash,
    pub_key: &[u8; 32],
    signature: &[u8; 64],
) -> Result<()> {
    let key = VerifyingKey::from_bytes(pub_key).map_err(|e| anyhow!("invalid public key: {e}"))?;
    let sig = ed25519_dalek::Signature::from_bytes(signature);
    key.verify_strict(&tx.signing_bytes(chain_hash)?, &sig)
        .map_err(|e| anyhow!("signature does not verify: {e}"))
}

/// Minimal borsh writer (little-endian integers, `u8` option tags, `u32` vector lengths).
#[derive(Default)]
struct Writer(Vec<u8>);

impl Writer {
    fn u8(&mut self, v: u8) {
        self.0.push(v);
    }
    fn bool(&mut self, v: bool) {
        self.0.push(v as u8);
    }
    fn u16(&mut self, v: u16) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn u32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn u64(&mut self, v: u64) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn i64(&mut self, v: i64) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn u128(&mut self, v: u128) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn bytes(&mut self, v: &[u8]) {
        self.0.extend_from_slice(v);
    }
    fn len(&mut self, n: usize) -> Result<()> {
        let n = u32::try_from(n).map_err(|_| anyhow!("vector of {n} items exceeds u32"))?;
        self.u32(n);
        Ok(())
    }
    fn opt_u8(&mut self, v: Option<u8>) {
        match v {
            Some(x) => {
                self.u8(1);
                self.u8(x);
            }
            None => self.u8(0),
        }
    }
    fn opt_u64(&mut self, v: Option<u64>) {
        match v {
            Some(x) => {
                self.u8(1);
                self.u64(x);
            }
            None => self.u8(0),
        }
    }
    /// `PositiveDecimal`: rust_decimal's `{flags, hi, lo, mid}` as four `u32`.
    fn decimal(&mut self, d: &Decimal) -> Result<()> {
        if d.is_sign_negative() {
            bail!("decimal {d} is negative; Bullet prices and sizes are PositiveDecimal");
        }
        let u = d.unpack();
        self.u32(u.scale << 16);
        self.u32(u.hi);
        self.u32(u.lo);
        self.u32(u.mid);
        Ok(())
    }
    fn side(&mut self, s: Side) {
        self.u8(match s {
            Side::Bid => 0,
            Side::Ask => 1,
        });
    }
    fn order_type(&mut self, t: OrderType) {
        self.u8(match t {
            OrderType::Limit => 0,
            OrderType::PostOnly => 1,
            OrderType::FillOrKill => 2,
            OrderType::ImmediateOrCancel => 3,
            OrderType::PostOnlySlide => 4,
            OrderType::PostOnlyFront => 5,
        });
    }
    fn tpsl(&mut self, t: &Option<Tpsl>) -> Result<()> {
        match t {
            Some(t) => {
                self.u8(1);
                self.decimal(&t.order_price)?;
                self.decimal(&t.trigger_price)?;
                self.u8(match t.price_condition {
                    TriggerPriceCondition::Mark => 0,
                    TriggerPriceCondition::Oracle => 1,
                    TriggerPriceCondition::LastTrade => 2,
                });
                self.order_type(t.order_type);
            }
            None => self.u8(0),
        }
        Ok(())
    }
    fn new_order(&mut self, o: &NewOrderArgs) -> Result<()> {
        self.decimal(&o.price)?;
        self.decimal(&o.size)?;
        self.side(o.side);
        self.order_type(o.order_type);
        self.bool(o.reduce_only);
        self.opt_u64(o.client_order_id);
        match &o.pending_tpsl_pair {
            Some(p) => {
                self.u8(1);
                self.tpsl(&p.tpsl_pair.tp)?;
                self.tpsl(&p.tpsl_pair.sl)?;
                self.bool(p.dynamic_size);
            }
            None => self.u8(0),
        }
        Ok(())
    }
    fn cancel(&mut self, c: &CancelOrderArgs) {
        self.opt_u64(c.order_id);
        self.opt_u64(c.client_order_id);
    }
}
