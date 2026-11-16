//! Order payloads for `POST /v3/order`: the zkLink `Contract` message, its signature, `limitFee`, and
//! the form fields, as `apexomni`'s `HttpPrivateSign.create_order_v3` builds them.
//!
//! The library does not choose client ids, expirations, fee rates or prices: callers pass them in.
//! `expiration_ms` is not part of the signed message; the SDK sends `now + 28 days`.

use crate::zk::ZkSigner;
use anyhow::{anyhow, Context, Result};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::{Decimal, RoundingStrategy};
use sha2::{Digest, Sha256};
use std::str::FromStr;

const MAX_U32: u64 = u32::MAX as u64;
const MAX_U64: u128 = u64::MAX as u128;
/// Digits Python's default decimal context keeps; the SDK's `Decimal` arithmetic rounds beyond it.
const DECIMAL_PRECISION: usize = 28;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApexSide {
    Buy,
    Sell,
}

impl ApexSide {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Buy => "BUY",
            Self::Sell => "SELL",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApexOrderType {
    Limit,
    Market,
    StopLimit,
    StopMarket,
    TakeProfitLimit,
    TakeProfitMarket,
}

impl ApexOrderType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Limit => "LIMIT",
            Self::Market => "MARKET",
            Self::StopLimit => "STOP_LIMIT",
            Self::StopMarket => "STOP_MARKET",
            Self::TakeProfitLimit => "TAKE_PROFIT_LIMIT",
            Self::TakeProfitMarket => "TAKE_PROFIT_MARKET",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApexTimeInForce {
    GoodTilCancel,
    FillOrKill,
    ImmediateOrCancel,
    /// Post-only (ALO).
    PostOnly,
}

impl ApexTimeInForce {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::GoodTilCancel => "GOOD_TIL_CANCEL",
            Self::FillOrKill => "FILL_OR_KILL",
            Self::ImmediateOrCancel => "IMMEDIATE_OR_CANCEL",
            Self::PostOnly => "POST_ONLY",
        }
    }
}

/// Account- and market-level inputs to the zk signature.
#[derive(Debug, Clone, Copy)]
pub struct ApexOrderContext<'a> {
    /// `GET /v3/account` `id` (decimal string).
    pub account_id: &'a str,
    /// The contract's `l2PairId` from `GET /v3/symbols`.
    pub l2_pair_id: u16,
    /// `decimals` of the contract's settle asset (`contractConfig.assets[token == settleAssetId]`).
    pub settle_decimals: u32,
    /// `contractAccount.takerFeeRate` from `GET /v3/account`, e.g. `"0.0005"`.
    pub taker_fee_rate: &'a str,
    /// `contractAccount.makerFeeRate` from `GET /v3/account`.
    pub maker_fee_rate: &'a str,
}

/// One order as the caller wants it. `size` and `price` are sent verbatim.
#[derive(Debug, Clone, Copy)]
pub struct ApexOrderRequest<'a> {
    pub symbol: &'a str,
    pub side: ApexSide,
    pub order_type: ApexOrderType,
    pub size: &'a str,
    /// Required for every type; market orders need a worst acceptable price
    /// (`GET /v3/get-worst-price`).
    pub price: &'a str,
    pub time_in_force: ApexTimeInForce,
    pub reduce_only: bool,
    /// Unique per order; also the source of the signed `nonce` / `slotId`.
    pub client_id: &'a str,
    /// Absolute expiry, unix ms.
    pub expiration_ms: u64,
    pub trigger_price: Option<&'a str>,
    /// e.g. `INDEX`, `MARKET`.
    pub trigger_price_type: Option<&'a str>,
    pub trailing_percent: Option<&'a str>,
    /// `isPositionTpsl`: a TP/SL order on the whole position.
    pub is_position_tpsl: bool,
}

/// zkLink `Contract` (contract order) message: 36 bytes, signed with musig.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContractOrder {
    pub account_id: u32,
    pub sub_account_id: u8,
    /// Only the low 16 bits are signed.
    pub slot_id: u32,
    /// Only the low 24 bits are signed.
    pub nonce: u32,
    /// Only the low 8 bits are signed.
    pub pair_id: u16,
    /// Base size scaled by `10^settle_decimals`, packed as a 40-bit decimal float.
    pub size: u128,
    /// Price scaled by `10^settle_decimals`, 120 bits.
    pub price: u128,
    pub is_long: bool,
    /// Basis points (fee rate * 10000, rounded up); 100 = 1%.
    pub taker_fee_rate: u8,
    pub maker_fee_rate: u8,
    pub has_subsidy: bool,
}

impl ContractOrder {
    /// `Contract::MSG_TYPE`.
    pub const MSG_TYPE: u8 = 0xfe;
    pub const BYTES: usize = 36;

    /// `Contract::get_bytes`.
    pub fn to_bytes(&self) -> Result<[u8; Self::BYTES]> {
        if self.price >> 120 != 0 {
            return Err(anyhow!("scaled price {} does not fit 120 bits", self.price));
        }
        let mut out = [0u8; Self::BYTES];
        out[0] = Self::MSG_TYPE;
        out[1..5].copy_from_slice(&self.account_id.to_be_bytes());
        out[5] = self.sub_account_id;
        out[6..8].copy_from_slice(&(self.slot_id as u16).to_be_bytes());
        out[8..11].copy_from_slice(&self.nonce.to_be_bytes()[1..]);
        out[11] = self.pair_id as u8;
        out[12] = self.is_long as u8;
        out[13..18].copy_from_slice(&pack_token_amount(self.size)?);
        out[18..33].copy_from_slice(&self.price.to_be_bytes()[1..]);
        out[33] = self.maker_fee_rate;
        out[34] = self.taker_fee_rate;
        out[35] = self.has_subsidy as u8;
        Ok(out)
    }
}

/// zkLink `pack_token_amount`: largest `mantissa * 10^exponent <= amount` with a 35-bit mantissa
/// and 5-bit exponent, as `mantissa << 5 | exponent`, 5 bytes big-endian.
pub fn pack_token_amount(amount: u128) -> Result<[u8; 5]> {
    const MANTISSA_BITS: u32 = 35;
    const EXPONENT_BITS: u32 = 5;
    let max_mantissa: u128 = (1 << MANTISSA_BITS) - 1;
    let max_exponent = 10u128.saturating_pow((1 << EXPONENT_BITS) - 1);
    if amount > max_mantissa.saturating_mul(max_exponent) {
        return Err(anyhow!("amount {amount} is too large to pack"));
    }
    let mut exponent: u128 = 0;
    let mut power: u128 = 1;
    while amount > max_mantissa * power {
        power *= 10;
        exponent += 1;
    }
    let (exponent, mantissa) = if exponent == 0 {
        (0, amount)
    } else {
        let mantissa = amount / power;
        let below = amount - mantissa * power;
        let with_max_mantissa = amount - max_mantissa * power / 10;
        if below < with_max_mantissa {
            (exponent, mantissa)
        } else {
            (exponent - 1, max_mantissa)
        }
    };
    let packed = (mantissa << EXPONENT_BITS) | exponent;
    let mut out = [0u8; 5];
    out.copy_from_slice(&packed.to_be_bytes()[11..]);
    Ok(out)
}

/// The signed ids the exchange derives from `clientId` and the account id:
/// `n = sha256(client_id)`, `nonce = n mod (2^32 - 1)`, `slot = (n mod (2^64 - 1)) / (2^32 - 1)` and
/// `account = account_id mod (2^32 - 1)`.
///
/// The SDK computes `slot` with float division then truncates; this reproduces that rounding, so
/// for about one client id in four million `slot` is one above the exact quotient.
pub fn contract_ids(client_id: &str, account_id: &str) -> Result<(u32, u32, u32)> {
    let digest = Sha256::digest(client_id.as_bytes());
    let (mut mod_u64, mut mod_u32) = (0u128, 0u128);
    for byte in digest {
        mod_u64 = (mod_u64 * 256 + byte as u128) % MAX_U64;
        mod_u32 = (mod_u32 * 256 + byte as u128) % MAX_U32 as u128;
    }
    let slot = python_float_quotient(mod_u64 as u64);
    let slot = u32::try_from(slot)
        .map_err(|_| anyhow!("slot id {slot} for client id {client_id} exceeds u32"))?;
    let nonce = mod_u32 as u32;

    let account: u128 = account_id
        .parse()
        .with_context(|| format!("account id {account_id:?} is not a decimal integer"))?;
    let account = (account % MAX_U32 as u128) as u32;
    Ok((account, slot, nonce))
}

/// `int(a / (2^32 - 1))` with Python's correctly rounded float division.
fn python_float_quotient(a: u64) -> u64 {
    let q = a / MAX_U32;
    let rem = a % MAX_U32;
    if q == 0 {
        return 0;
    }
    // q < 2^33, so the double nearest a/b has spacing 2^(e-52) around q: it rounds up to q + 1 when
    // q + 1 - a/b <= 2^(e-53), ties going to the even q + 1.
    let e = 63 - q.leading_zeros();
    if ((MAX_U32 - rem) as u128) << (53 - e) <= MAX_U32 as u128 {
        q + 1
    } else {
        q
    }
}

/// `value * 10^decimals`, truncated, for a plain decimal string (`"65000.5"`).
pub fn scale_decimal(value: &str, decimals: u32) -> Result<u128> {
    let (int_part, frac_part) = value.split_once('.').unwrap_or((value, ""));
    let digits_ok = |s: &str| s.bytes().all(|b| b.is_ascii_digit());
    if (int_part.is_empty() && frac_part.is_empty())
        || !digits_ok(int_part)
        || !digits_ok(frac_part)
    {
        return Err(anyhow!("{value:?} is not a plain decimal number"));
    }
    let significant = format!("{int_part}{frac_part}");
    if significant.trim_start_matches('0').len() > DECIMAL_PRECISION {
        return Err(anyhow!(
            "{value:?} has more than {DECIMAL_PRECISION} significant digits"
        ));
    }
    let decimals = decimals as usize;
    let overflow = || anyhow!("{value:?} scaled by 10^{decimals} overflows u128");
    let mut out: u128 = 0;
    let frac = frac_part
        .bytes()
        .chain(std::iter::repeat(b'0'))
        .take(decimals);
    for b in int_part.bytes().chain(frac) {
        out = out
            .checked_mul(10)
            .and_then(|v| v.checked_add((b - b'0') as u128))
            .ok_or_else(overflow)?;
    }
    // The SDK's `quantize(Decimal(0))` raises past the 28-digit context precision.
    if out >= 10u128.pow(DECIMAL_PRECISION as u32) {
        return Err(anyhow!(
            "{value:?} scaled by 10^{decimals} has more than {DECIMAL_PRECISION} digits"
        ));
    }
    Ok(out)
}

/// Fee rate in basis points, rounded up: `ceil(rate * 10000)`.
pub fn fee_rate_bps(rate: &str) -> Result<u8> {
    let rate =
        Decimal::from_str(rate).with_context(|| format!("fee rate {rate:?} is not a decimal"))?;
    let bps =
        (rate * Decimal::from(10_000)).round_dp_with_strategy(0, RoundingStrategy::AwayFromZero);
    bps.to_u8()
        .ok_or_else(|| anyhow!("fee rate {rate} is {bps} bps; the signed field holds 0..=255"))
}

/// `limitFee`: `size * price * taker_fee_rate`, rounded up to 6 decimals.
pub fn limit_fee(size: &str, price: &str, taker_fee_rate: &str) -> Result<String> {
    let parse = |name: &str, v: &str| {
        Decimal::from_str(v).with_context(|| format!("{name} {v:?} is not a decimal"))
    };
    let rate = parse("taker fee rate", taker_fee_rate)?;
    let fee = parse("size", size)?
        .checked_mul(parse("price", price)?)
        .and_then(|cost| cost.checked_mul(rate))
        .ok_or_else(|| anyhow!("limit fee overflows: {size} * {price} * {taker_fee_rate}"))?;
    let mut fee = fee.round_dp_with_strategy(6, RoundingStrategy::AwayFromZero);
    fee.rescale(6);
    Ok(fee.to_string())
}

/// A signed order ready for `TradingApi::create_order`.
#[derive(Debug, Clone)]
pub struct ApexSignedOrder {
    /// Form fields in the SDK's order. `TradingApi` signs and sends exactly these.
    pub form: Vec<(&'static str, String)>,
    pub contract: ContractOrder,
    /// zk signature, `0x`-prefixed hex.
    pub signature: String,
    pub limit_fee: String,
}

fn python_bool(v: bool) -> String {
    // The SDK posts Python's `str(bool)`.
    let s = if v { "True" } else { "False" };
    s.to_string()
}

/// Builds and signs an order as `create_order_v3` does (without attached open TP/SL legs).
pub fn sign_order(
    signer: &ZkSigner,
    ctx: &ApexOrderContext<'_>,
    req: &ApexOrderRequest<'_>,
) -> Result<ApexSignedOrder> {
    let (account_id, slot_id, nonce) = contract_ids(req.client_id, ctx.account_id)?;
    let contract = ContractOrder {
        account_id,
        sub_account_id: 0,
        slot_id,
        nonce,
        pair_id: ctx.l2_pair_id,
        size: scale_decimal(req.size, ctx.settle_decimals)?,
        price: scale_decimal(req.price, ctx.settle_decimals)?,
        is_long: req.side == ApexSide::Buy,
        taker_fee_rate: fee_rate_bps(ctx.taker_fee_rate)?,
        maker_fee_rate: fee_rate_bps(ctx.maker_fee_rate)?,
        has_subsidy: false,
    };
    let signature = signer.sign_musig_hex(&contract.to_bytes()?)?;
    let limit_fee = limit_fee(req.size, req.price, ctx.taker_fee_rate)?;

    let mut form: Vec<(&'static str, String)> = vec![
        ("symbol", req.symbol.to_string()),
        ("side", req.side.as_str().to_string()),
        ("type", req.order_type.as_str().to_string()),
        ("timeInForce", req.time_in_force.as_str().to_string()),
        ("size", req.size.to_string()),
        ("price", req.price.to_string()),
        ("limitFee", limit_fee.clone()),
        ("expiration", req.expiration_ms.to_string()),
    ];
    if let Some(v) = req.trigger_price {
        form.push(("triggerPrice", v.to_string()));
    }
    if let Some(v) = req.trigger_price_type {
        form.push(("triggerPriceType", v.to_string()));
    }
    if let Some(v) = req.trailing_percent {
        form.push(("trailingPercent", v.to_string()));
    }
    form.extend([
        ("clientId", req.client_id.to_string()),
        ("signature", signature.clone()),
        ("reduceOnly", python_bool(req.reduce_only)),
        ("isPositionTpsl", python_bool(req.is_position_tpsl)),
        ("isOpenTpslOrder", python_bool(false)),
        ("isSetOpenSl", python_bool(false)),
        ("isSetOpenTp", python_bool(false)),
    ]);

    Ok(ApexSignedOrder {
        form,
        contract,
        signature,
        limit_fee,
    })
}
