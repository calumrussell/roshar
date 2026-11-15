//! Native Lighter transaction signing (no FFI).
//!
//! Lighter L2 transactions are hashed with Poseidon2 over the Goldilocks field into a GF(p^5)
//! element and signed with Schnorr over the ECgFp5 curve. This module reproduces the official Go
//! signer (github.com/elliottech/lighter-go, as shipped in lighter-sdk 1.1.8, with
//! github.com/elliottech/poseidon_crypto v0.0.15). `tests/vectors.json` holds transactions signed
//! by that library; the tests below reproduce them byte for byte (the Schnorr nonce is random, so
//! the tests recover it from each recorded signature and re-sign with it).
//!
//! Nothing here reads the environment, the clock or the network: the caller passes the API key,
//! account index, API key index, chain id, nonce and expiry.

mod curve;
mod field;
mod poseidon2;
mod poseidon2_constants;

use anyhow::{bail, ensure, Context, Result};
use base64::Engine;

use curve::Scalar;
use field::{Fp5, F};

/// `chain_id` for mainnet (`mainnet.zklighter.elliot.ai`).
pub const MAINNET_CHAIN_ID: u32 = 304;
/// `chain_id` for testnet (`testnet.zklighter.elliot.ai`).
pub const TESTNET_CHAIN_ID: u32 = 300;
/// The Go signer's default transaction lifetime (10 minutes less 1 second): `ExpiredAt = now_ms + this`.
pub const DEFAULT_TX_LIFETIME_MS: i64 = 599_000;

pub const TX_TYPE_CREATE_ORDER: u8 = 14;
pub const TX_TYPE_CANCEL_ORDER: u8 = 15;
pub const TX_TYPE_CANCEL_ALL_ORDERS: u8 = 16;
pub const TX_TYPE_MODIFY_ORDER: u8 = 17;

const NIL_MARKET_INDEX: i64 = 255;
const MAX_ACCOUNT_INDEX: i64 = (1 << 48) - 2;
const MAX_API_KEY_INDEX: u8 = 254;
const MAX_MARKET_INDEX: i64 = (1 << 48) - 1;
const MAX_CLIENT_ORDER_INDEX: i64 = (1 << 48) - 1;
const MIN_ORDER_INDEX: i64 = MAX_CLIENT_ORDER_INDEX + 1;
const MAX_ORDER_INDEX: i64 = (1 << 60) - 1;
const MAX_BASE_AMOUNT: i64 = (1 << 48) - 1;
const MAX_TIMESTAMP: i64 = (1 << 48) - 1;
const ATTR_CANCEL_ALL_MARKET_INDEX: u8 = 5;
const ATTRIBUTES_PER_TX: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum OrderType {
    Limit = 0,
    Market = 1,
    StopLoss = 2,
    StopLossLimit = 3,
    TakeProfit = 4,
    TakeProfitLimit = 5,
    Twap = 6,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum TimeInForce {
    ImmediateOrCancel = 0,
    GoodTillTime = 1,
    PostOnly = 2,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum CancelAllTif {
    /// Cancel now. `time` must be 0.
    Immediate = 0,
    /// Dead-man's switch: cancel everything at `time` (ms). Cannot be limited to one market.
    Scheduled = 1,
    /// Abort a scheduled cancel. `time` must be 0.
    Abort = 2,
}

/// `L2CreateOrder` (tx type 14). Prices and sizes are integers in the market's ticks/units.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreateOrder {
    pub market_index: i64,
    /// 1..2^48-1, or 0 for none.
    pub client_order_index: i64,
    pub base_amount: i64,
    pub price: u32,
    pub is_ask: bool,
    pub order_type: OrderType,
    pub time_in_force: TimeInForce,
    pub reduce_only: bool,
    /// 0 = none.
    pub trigger_price: u32,
    /// Absolute expiry in ms. Must be 0 for IOC orders and non-zero for resting limit orders.
    /// The Python SDK's `-1` default means `now_ms + 28 days`; pass that value explicitly.
    pub order_expiry: i64,
}

/// `L2CancelOrder` (tx type 15). `index` is the client order index or the exchange order index.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CancelOrder {
    pub market_index: i64,
    pub index: i64,
}

/// `L2CancelAllOrders` (tx type 16).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CancelAllOrders {
    pub time_in_force: CancelAllTif,
    /// ms timestamp for `Scheduled`, else 0.
    pub time: i64,
    /// Limit to one market (only with `Immediate`). `None` cancels on every market.
    pub market_index: Option<i64>,
}

/// `L2ModifyOrder` (tx type 17).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModifyOrder {
    pub market_index: i64,
    pub index: i64,
    pub base_amount: i64,
    pub price: u32,
    pub trigger_price: u32,
}

/// Per-transaction inputs the Go SDK takes from its nonce manager and clock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TxOpts {
    /// Next nonce of this API key (`GET /api/v1/nextNonce`, then +1 per signed tx).
    pub nonce: i64,
    /// ms timestamp after which the sequencer rejects the tx; usually `now_ms + DEFAULT_TX_LIFETIME_MS`.
    pub expired_at_ms: i64,
}

/// A signed transaction, as the Go library returns it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SignedTx {
    pub tx_type: u8,
    /// JSON `tx_info` for `sendTx` (`/api/v1/sendTx` form field or WS `jsonapi/sendtx`).
    pub tx_info: String,
    /// Hex of the signed message hash (40 bytes); the exchange reports the same `tx_hash`.
    pub tx_hash: String,
}

/// A Lighter API key (the Schnorr private key), parsed from the 40-byte hex the SDKs produce.
#[derive(Clone)]
pub struct ApiKey {
    sk: Scalar,
}

impl std::fmt::Debug for ApiKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ApiKey({})", self.public_key_hex())
    }
}

impl ApiKey {
    /// `0x`-prefixed or bare hex of 40 bytes (little-endian scalar), as in `LIGHTER_API_PRIVATE_KEY`.
    pub fn from_hex(private_key: &str) -> Result<ApiKey> {
        let s = private_key.strip_prefix("0x").unwrap_or(private_key);
        let b = hex::decode(s).context("API private key is not hex")?;
        ensure!(
            b.len() == 40,
            "API private key must be 40 bytes, got {}",
            b.len()
        );
        let sk = Scalar::from_le_bytes(b.as_slice().try_into().unwrap());
        ensure!(!sk.is_zero(), "API private key is zero");
        Ok(ApiKey { sk })
    }

    /// `0x` + hex of the 40-byte public key, as Lighter's `/api/v1/apikeys` reports it.
    pub fn public_key_hex(&self) -> String {
        format!(
            "0x{}",
            hex::encode(curve::public_key(&self.sk).to_le_bytes())
        )
    }
}

/// Signs transactions for one (account, API key) pair on one chain.
#[derive(Clone, Debug)]
pub struct Signer {
    key: ApiKey,
    account_index: i64,
    api_key_index: u8,
    chain_id: u32,
}

impl Signer {
    pub fn new(
        key: ApiKey,
        account_index: i64,
        api_key_index: u8,
        chain_id: u32,
    ) -> Result<Signer> {
        ensure!(
            account_index > 0 && account_index <= MAX_ACCOUNT_INDEX,
            "invalid account index {account_index}"
        );
        ensure!(
            api_key_index <= MAX_API_KEY_INDEX,
            "invalid API key index {api_key_index}"
        );
        Ok(Signer {
            key,
            account_index,
            api_key_index,
            chain_id,
        })
    }

    pub fn account_index(&self) -> i64 {
        self.account_index
    }

    pub fn api_key_index(&self) -> u8 {
        self.api_key_index
    }

    pub fn chain_id(&self) -> u32 {
        self.chain_id
    }

    pub fn public_key_hex(&self) -> String {
        self.key.public_key_hex()
    }

    pub fn sign_create_order(&self, o: &CreateOrder, opts: TxOpts) -> Result<SignedTx> {
        self.create_order(o, opts, None)
    }

    pub fn sign_cancel_order(&self, c: &CancelOrder, opts: TxOpts) -> Result<SignedTx> {
        self.cancel_order(c, opts, None)
    }

    pub fn sign_cancel_all_orders(&self, c: &CancelAllOrders, opts: TxOpts) -> Result<SignedTx> {
        self.cancel_all_orders(c, opts, None)
    }

    pub fn sign_modify_order(&self, m: &ModifyOrder, opts: TxOpts) -> Result<SignedTx> {
        self.modify_order(m, opts, None)
    }

    /// Auth token for private WS channels and authenticated REST reads:
    /// `"{deadline}:{account_index}:{api_key_index}:{hex signature}"`, valid until `deadline_unix_s`
    /// (seconds). Lighter caps the lifetime at 8 hours.
    pub fn auth_token(&self, deadline_unix_s: i64) -> Result<String> {
        self.auth_token_with(deadline_unix_s, None)
    }

    fn auth_token_with(&self, deadline_unix_s: i64, k: Option<&Scalar>) -> Result<String> {
        let message = format!(
            "{}:{}:{}",
            deadline_unix_s, self.account_index, self.api_key_index
        );
        let elems = poseidon2::bytes_to_elements(message.as_bytes()).map_err(anyhow::Error::msg)?;
        let h = poseidon2::hash_to_quintic_extension(&elems);
        Ok(format!("{message}:{}", hex::encode(self.sign_hash(&h, k))))
    }

    fn sign_hash(&self, h: &Fp5, k: Option<&Scalar>) -> [u8; 80] {
        match k {
            Some(k) => curve::sign_with_k(h, &self.key.sk, k),
            None => curve::sign(h, &self.key.sk),
        }
    }

    fn check_common(&self, opts: TxOpts) -> Result<()> {
        ensure!(opts.nonce >= 0, "nonce too low");
        ensure!(
            (0..=MAX_TIMESTAMP).contains(&opts.expired_at_ms),
            "invalid ExpiredAt {}",
            opts.expired_at_ms
        );
        Ok(())
    }

    fn header(&self, tx_type: u8, opts: TxOpts) -> Vec<F> {
        vec![
            F::new(self.chain_id as u64),
            F::new(tx_type as u64),
            F::new(opts.nonce as u64),
            F::new(opts.expired_at_ms as u64),
            F::new(self.account_index as u64),
            F::new(self.api_key_index as u64),
        ]
    }

    /// Hash, sign and assemble `tx_info`. `body` is the JSON between the key indices and
    /// `ExpiredAt`, starting with a comma.
    fn finish(
        &self,
        tx_type: u8,
        elems: Vec<F>,
        attrs: &[(u8, i64)],
        body: String,
        opts: TxOpts,
        k: Option<&Scalar>,
    ) -> SignedTx {
        let h = aggregate_attributes(poseidon2::hash_to_quintic_extension(&elems), attrs);
        let sig = self.sign_hash(&h, k);
        let attrs_json = if attrs.is_empty() {
            "null".to_string()
        } else {
            let mut sorted = attrs.to_vec();
            sorted.sort();
            let parts: Vec<String> = sorted.iter().map(|(t, v)| format!("\"{t}\":{v}")).collect();
            format!("{{{}}}", parts.join(","))
        };
        let tx_info = format!(
            "{{\"AccountIndex\":{},\"ApiKeyIndex\":{}{},\"ExpiredAt\":{},\"Nonce\":{},\"Sig\":\"{}\",\"L2TxAttributes\":{}}}",
            self.account_index,
            self.api_key_index,
            body,
            opts.expired_at_ms,
            opts.nonce,
            base64::engine::general_purpose::STANDARD.encode(sig),
            attrs_json
        );
        SignedTx {
            tx_type,
            tx_info,
            tx_hash: hex::encode(h.to_le_bytes()),
        }
    }

    fn create_order(&self, o: &CreateOrder, opts: TxOpts, k: Option<&Scalar>) -> Result<SignedTx> {
        self.check_common(opts)?;
        validate_create_order(o)?;
        let mut elems = self.header(TX_TYPE_CREATE_ORDER, opts);
        elems.extend(
            [
                o.market_index as u64,
                o.client_order_index as u64,
                o.base_amount as u64,
                o.price as u64,
                o.is_ask as u64,
                o.order_type as u64,
                o.time_in_force as u64,
                o.reduce_only as u64,
                o.trigger_price as u64,
                o.order_expiry as u64,
            ]
            .map(F::new),
        );
        let body = format!(
            ",\"MarketIndex\":{},\"ClientOrderIndex\":{},\"BaseAmount\":{},\"Price\":{},\"IsAsk\":{},\"Type\":{},\"TimeInForce\":{},\"ReduceOnly\":{},\"TriggerPrice\":{},\"OrderExpiry\":{}",
            o.market_index,
            o.client_order_index,
            o.base_amount,
            o.price,
            o.is_ask as u8,
            o.order_type as u8,
            o.time_in_force as u8,
            o.reduce_only as u8,
            o.trigger_price,
            o.order_expiry
        );
        Ok(self.finish(TX_TYPE_CREATE_ORDER, elems, &[], body, opts, k))
    }

    fn cancel_order(&self, c: &CancelOrder, opts: TxOpts, k: Option<&Scalar>) -> Result<SignedTx> {
        self.check_common(opts)?;
        check_market(c.market_index)?;
        check_order_index(c.index)?;
        let mut elems = self.header(TX_TYPE_CANCEL_ORDER, opts);
        elems.extend([F::new(c.market_index as u64), F::new(c.index as u64)]);
        let body = format!(",\"MarketIndex\":{},\"Index\":{}", c.market_index, c.index);
        Ok(self.finish(TX_TYPE_CANCEL_ORDER, elems, &[], body, opts, k))
    }

    fn cancel_all_orders(
        &self,
        c: &CancelAllOrders,
        opts: TxOpts,
        k: Option<&Scalar>,
    ) -> Result<SignedTx> {
        self.check_common(opts)?;
        let attrs: Vec<(u8, i64)> = match c.market_index {
            None | Some(NIL_MARKET_INDEX) => vec![],
            Some(m) => {
                ensure!(
                    (0..=MAX_MARKET_INDEX).contains(&m),
                    "invalid cancel-all market index {m}"
                );
                ensure!(
                    c.time_in_force == CancelAllTif::Immediate,
                    "cancel-all for one market can't be scheduled"
                );
                vec![(ATTR_CANCEL_ALL_MARKET_INDEX, m)]
            }
        };
        match c.time_in_force {
            CancelAllTif::Immediate | CancelAllTif::Abort => {
                ensure!(c.time == 0, "cancel-all time must be 0")
            }
            CancelAllTif::Scheduled => ensure!(c.time >= 1, "scheduled cancel-all needs a time"),
        }
        let mut elems = self.header(TX_TYPE_CANCEL_ALL_ORDERS, opts);
        elems.extend([F::new(c.time_in_force as u64), F::new(c.time as u64)]);
        let body = format!(
            ",\"TimeInForce\":{},\"Time\":{}",
            c.time_in_force as u8, c.time
        );
        Ok(self.finish(TX_TYPE_CANCEL_ALL_ORDERS, elems, &attrs, body, opts, k))
    }

    fn modify_order(&self, m: &ModifyOrder, opts: TxOpts, k: Option<&Scalar>) -> Result<SignedTx> {
        self.check_common(opts)?;
        check_market(m.market_index)?;
        check_order_index(m.index)?;
        ensure!(
            (0..=MAX_BASE_AMOUNT).contains(&m.base_amount),
            "invalid base amount {}",
            m.base_amount
        );
        ensure!(m.price >= 1, "price too low");
        let mut elems = self.header(TX_TYPE_MODIFY_ORDER, opts);
        elems.extend(
            [
                m.market_index as u64,
                m.index as u64,
                m.base_amount as u64,
                m.price as u64,
                m.trigger_price as u64,
            ]
            .map(F::new),
        );
        let body = format!(
            ",\"MarketIndex\":{},\"Index\":{},\"BaseAmount\":{},\"Price\":{},\"TriggerPrice\":{}",
            m.market_index, m.index, m.base_amount, m.price, m.trigger_price
        );
        Ok(self.finish(TX_TYPE_MODIFY_ORDER, elems, &[], body, opts, k))
    }
}

/// Go `L2TxAttributes.AggregateTxHash`: with no attributes the tx hash is used as is; otherwise
/// H(tx hash || H(type/value pairs sorted by type, zero-padded to 4 pairs)).
fn aggregate_attributes(tx_hash: Fp5, attrs: &[(u8, i64)]) -> Fp5 {
    if attrs.is_empty() {
        return tx_hash;
    }
    let mut sorted = attrs.to_vec();
    sorted.sort();
    let mut elems = Vec::with_capacity(2 * ATTRIBUTES_PER_TX);
    for i in 0..ATTRIBUTES_PER_TX {
        let (t, v) = sorted.get(i).copied().unwrap_or((0, 0));
        elems.push(F::new(t as u64));
        elems.push(F::new(v as u64));
    }
    let attrs_hash = poseidon2::hash_to_quintic_extension(&elems);
    let mut combined = tx_hash.0.to_vec();
    combined.extend_from_slice(&attrs_hash.0);
    poseidon2::hash_to_quintic_extension(&combined)
}

fn check_market(m: i64) -> Result<()> {
    if m < 0 || m == NIL_MARKET_INDEX || m > MAX_MARKET_INDEX {
        bail!("invalid market index {m}");
    }
    Ok(())
}

fn check_order_index(i: i64) -> Result<()> {
    ensure!(i >= 1, "order index too low");
    ensure!(
        i <= MAX_CLIENT_ORDER_INDEX || (MIN_ORDER_INDEX..=MAX_ORDER_INDEX).contains(&i),
        "order index too high"
    );
    Ok(())
}

/// The Go `L2CreateOrderTxInfo.Validate` checks.
fn validate_create_order(o: &CreateOrder) -> Result<()> {
    use OrderType::*;
    use TimeInForce::*;
    check_market(o.market_index)?;
    ensure!(
        o.client_order_index == 0 || (1..=MAX_CLIENT_ORDER_INDEX).contains(&o.client_order_index),
        "invalid client order index {}",
        o.client_order_index
    );
    ensure!(o.reduce_only || o.base_amount != 0, "base amount too low");
    ensure!(
        (0..=MAX_BASE_AMOUNT).contains(&o.base_amount),
        "invalid base amount {}",
        o.base_amount
    );
    ensure!(o.price >= 1, "price too low");
    ensure!(
        o.order_expiry >= 0,
        "invalid order expiry {}",
        o.order_expiry
    );
    let nil_expiry = o.order_expiry == 0;
    let nil_trigger = o.trigger_price == 0;
    match o.order_type {
        Market => ensure!(
            o.time_in_force == ImmediateOrCancel && nil_expiry && nil_trigger,
            "market orders are IOC with no expiry or trigger"
        ),
        Limit => {
            ensure!(nil_trigger, "limit orders take no trigger price");
            ensure!(
                (o.time_in_force == ImmediateOrCancel) == nil_expiry,
                "IOC limit orders need expiry 0, resting ones a non-zero expiry"
            );
        }
        StopLoss | TakeProfit => ensure!(
            o.time_in_force == ImmediateOrCancel && !nil_trigger && !nil_expiry,
            "invalid stop-loss/take-profit order"
        ),
        StopLossLimit | TakeProfitLimit => ensure!(
            !nil_trigger && !nil_expiry,
            "invalid stop/take-profit limit order"
        ),
        Twap => ensure!(
            o.time_in_force == GoodTillTime && nil_trigger && !nil_expiry,
            "invalid TWAP order"
        ),
    }
    Ok(())
}

#[cfg(test)]
mod tests;
