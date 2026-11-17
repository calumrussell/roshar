//! Montgomery arithmetic over 4 x 64-bit limbs for the two prime fields the zkLink signer uses:
//! `Fr` (BN254 scalar field, curve coordinates and Rescue) and `Fs` (Alt-BabyJubjub subgroup order,
//! scalars). Values are always fully reduced, so equality on the Montgomery form is equality of
//! field elements.

use super::constants::{FR_INV, FR_MODULUS, FR_R2, FS_INV, FS_MODULUS, FS_R2};

#[inline(always)]
fn adc(a: u64, b: u64, carry: u64) -> (u64, u64) {
    let t = a as u128 + b as u128 + carry as u128;
    (t as u64, (t >> 64) as u64)
}

#[inline(always)]
fn sbb(a: u64, b: u64, borrow: u64) -> (u64, u64) {
    let t = (a as u128).wrapping_sub(b as u128 + borrow as u128);
    (t as u64, (t >> 127) as u64)
}

#[inline(always)]
fn mac(acc: u64, b: u64, c: u64, carry: u64) -> (u64, u64) {
    let t = acc as u128 + (b as u128) * (c as u128) + carry as u128;
    (t as u64, (t >> 64) as u64)
}

/// `a >= m`, both little-endian limbs.
#[inline(always)]
pub(crate) fn geq(a: &[u64; 4], m: &[u64; 4]) -> bool {
    for i in (0..4).rev() {
        if a[i] != m[i] {
            return a[i] > m[i];
        }
    }
    true
}

#[inline(always)]
fn sub_limbs(a: &[u64; 4], b: &[u64; 4]) -> ([u64; 4], u64) {
    let mut out = [0u64; 4];
    let mut borrow = 0;
    for i in 0..4 {
        let (r, br) = sbb(a[i], b[i], borrow);
        out[i] = r;
        borrow = br;
    }
    (out, borrow)
}

#[inline(always)]
fn add_mod(a: &[u64; 4], b: &[u64; 4], m: &[u64; 4]) -> [u64; 4] {
    let mut out = [0u64; 4];
    let mut carry = 0;
    for i in 0..4 {
        let (r, c) = adc(a[i], b[i], carry);
        out[i] = r;
        carry = c;
    }
    if carry != 0 || geq(&out, m) {
        sub_limbs(&out, m).0
    } else {
        out
    }
}

#[inline(always)]
fn sub_mod(a: &[u64; 4], b: &[u64; 4], m: &[u64; 4]) -> [u64; 4] {
    let (out, borrow) = sub_limbs(a, b);
    if borrow == 0 {
        return out;
    }
    let mut fixed = [0u64; 4];
    let mut carry = 0;
    for i in 0..4 {
        let (r, c) = adc(out[i], m[i], carry);
        fixed[i] = r;
        carry = c;
    }
    fixed
}

/// CIOS Montgomery multiplication: `a * b * 2^-256 mod m`. Both moduli are below 2^254, so one
/// conditional subtraction fully reduces the result.
#[inline(always)]
fn mont_mul(a: &[u64; 4], b: &[u64; 4], m: &[u64; 4], inv: u64) -> [u64; 4] {
    let mut t = [0u64; 6];
    for i in 0..4 {
        let mut c = 0;
        for j in 0..4 {
            let (lo, hi) = mac(t[j], a[j], b[i], c);
            t[j] = lo;
            c = hi;
        }
        let (lo, hi) = adc(t[4], c, 0);
        t[4] = lo;
        t[5] = hi;

        let k = t[0].wrapping_mul(inv);
        let (_, mut c) = mac(t[0], k, m[0], 0);
        for j in 1..4 {
            let (lo, hi) = mac(t[j], k, m[j], c);
            t[j - 1] = lo;
            c = hi;
        }
        let (lo, hi) = adc(t[4], c, 0);
        t[3] = lo;
        t[4] = t[5] + hi;
        t[5] = 0;
    }
    let r = [t[0], t[1], t[2], t[3]];
    if t[4] != 0 || geq(&r, m) {
        sub_limbs(&r, m).0
    } else {
        r
    }
}

macro_rules! mont_field {
    ($name:ident, $modulus:expr, $r2:expr, $inv:expr) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub(crate) struct $name([u64; 4]);

        // Both fields share one API; each uses a subset of it.
        #[allow(dead_code)]
        impl $name {
            pub(crate) const MODULUS: [u64; 4] = $modulus;
            pub(crate) const ZERO: Self = Self([0; 4]);

            pub(crate) fn one() -> Self {
                Self::from_u64(1)
            }

            /// Field element from a canonical little-endian integer; `None` if not below the modulus.
            pub(crate) fn from_canonical(limbs: [u64; 4]) -> Option<Self> {
                if geq(&limbs, &Self::MODULUS) {
                    return None;
                }
                Some(Self(mont_mul(&limbs, &$r2, &Self::MODULUS, $inv)))
            }

            pub(crate) fn from_u64(v: u64) -> Self {
                Self(mont_mul(&[v, 0, 0, 0], &$r2, &Self::MODULUS, $inv))
            }

            /// Canonical integer, little-endian limbs.
            pub(crate) fn to_canonical(self) -> [u64; 4] {
                mont_mul(&self.0, &[1, 0, 0, 0], &Self::MODULUS, $inv)
            }

            pub(crate) fn add(self, other: Self) -> Self {
                Self(add_mod(&self.0, &other.0, &Self::MODULUS))
            }

            pub(crate) fn sub(self, other: Self) -> Self {
                Self(sub_mod(&self.0, &other.0, &Self::MODULUS))
            }

            pub(crate) fn mul(self, other: Self) -> Self {
                Self(mont_mul(&self.0, &other.0, &Self::MODULUS, $inv))
            }

            pub(crate) fn square(self) -> Self {
                self.mul(self)
            }

            pub(crate) fn double(self) -> Self {
                self.add(self)
            }

            /// `self^exp`, `exp` a little-endian integer.
            pub(crate) fn pow(self, exp: &[u64; 4]) -> Self {
                let mut acc = Self::one();
                for i in (0..256).rev() {
                    acc = acc.square();
                    if (exp[i / 64] >> (i % 64)) & 1 == 1 {
                        acc = acc.mul(self);
                    }
                }
                acc
            }

            /// Multiplicative inverse (Fermat); the inverse of zero is zero.
            pub(crate) fn invert(self) -> Self {
                let m = Self::MODULUS;
                // Neither modulus has a low limb below 2.
                self.pow(&[m[0] - 2, m[1], m[2], m[3]])
            }
        }
    };
}

mont_field!(Fr, FR_MODULUS, FR_R2, FR_INV);
mont_field!(Fs, FS_MODULUS, FS_R2, FS_INV);

/// Little-endian limbs of a 32-byte big-endian integer.
pub(crate) fn limbs_from_be(bytes: &[u8; 32]) -> [u64; 4] {
    let mut out = [0u64; 4];
    for (i, limb) in out.iter_mut().enumerate() {
        let start = 32 - 8 * (i + 1);
        let mut word = [0u8; 8];
        word.copy_from_slice(&bytes[start..start + 8]);
        *limb = u64::from_be_bytes(word);
    }
    out
}

/// 32-byte little-endian encoding of little-endian limbs.
pub(crate) fn limbs_to_le(limbs: &[u64; 4]) -> [u8; 32] {
    let mut out = [0u8; 32];
    for (i, limb) in limbs.iter().enumerate() {
        out[8 * i..8 * i + 8].copy_from_slice(&limb.to_le_bytes());
    }
    out
}

/// 32-byte big-endian encoding of little-endian limbs.
pub(crate) fn limbs_to_be(limbs: &[u64; 4]) -> [u8; 32] {
    let mut out = limbs_to_le(limbs);
    out.reverse();
    out
}
