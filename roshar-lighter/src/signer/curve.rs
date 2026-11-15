//! ECgFp5 (Pornin's curve over GF(p^5), prime order n ~ 2^319) and Schnorr signatures over it.
//!
//! Ported from github.com/elliottech/poseidon_crypto v0.0.15 (`curve/ecgfp5`, `signature/schnorr`).
//! Points use the (x, u) = (x, x/y) fractional coordinates and the complete addition formulas of
//! the Go code; scalar multiplication is a plain double-and-add (the encoded result does not depend
//! on the algorithm). Scalars mod n use `num-bigint`.

use num_bigint::BigUint;
use num_traits::Zero;
use std::sync::OnceLock;

use super::field::{Fp5, F};
use super::poseidon2::hash_to_quintic_extension;

const B1: u64 = 263;

fn b() -> Fp5 {
    Fp5([F::ZERO, F(B1), F::ZERO, F::ZERO, F::ZERO])
}

fn b_mul2() -> Fp5 {
    Fp5([F::ZERO, F(2 * B1), F::ZERO, F::ZERO, F::ZERO])
}

fn b_mul4() -> Fp5 {
    Fp5([F::ZERO, F(4 * B1), F::ZERO, F::ZERO, F::ZERO])
}

const FOUR: Fp5 = Fp5([F(4), F::ZERO, F::ZERO, F::ZERO, F::ZERO]);

#[derive(Clone, Copy, Debug)]
pub(crate) struct Point {
    x: Fp5,
    z: Fp5,
    u: Fp5,
    t: Fp5,
}

impl Point {
    pub(crate) const NEUTRAL: Point = Point {
        x: Fp5::ZERO,
        z: Fp5::ONE,
        u: Fp5::ZERO,
        t: Fp5::ONE,
    };

    pub(crate) fn generator() -> Point {
        Point {
            x: Fp5::from_u64s([
                12883135586176881569,
                4356519642755055268,
                5248930565894896907,
                2165973894480315022,
                2448410071095648785,
            ]),
            z: Fp5::ONE,
            u: Fp5::ONE,
            t: FOUR,
        }
    }

    /// The point's canonical Fp5 encoding w = y/x = t/u (0 for the neutral point).
    pub(crate) fn encode(&self) -> Fp5 {
        self.t.mul(self.u.inverse_or_zero())
    }

    pub(crate) fn add(&self, rhs: &Point) -> Point {
        let (x1, z1, u1, t1_) = (self.x, self.z, self.u, self.t);
        let (x2, z2, u2, t2_) = (rhs.x, rhs.z, rhs.u, rhs.t);
        let t1 = x1.mul(x2);
        let t2 = z1.mul(z2);
        let t3 = u1.mul(u2);
        let t4 = t1_.mul(t2_);
        let t5 = x1.add(z1).mul(x2.add(z2)).sub(t1.add(t2));
        let t6 = u1.add(t1_).mul(u2.add(t2_)).sub(t3.add(t4));
        let t7 = t1.add(t2.mul(b()));
        let t8 = t4.mul(t7);
        let t9 = t3.mul(t5.mul(b_mul2()).add(t7.double()));
        let t10 = t4.add(t3.double()).mul(t5.add(t7));
        Point {
            x: t10.sub(t8).mul(b()),
            z: t8.sub(t9),
            u: t6.mul(t2.mul(b()).sub(t1)),
            t: t8.add(t9),
        }
    }

    pub(crate) fn double(&self) -> Point {
        let (x, z, u, t) = (self.x, self.z, self.u, self.t);
        let t1 = z.mul(t);
        let t2 = t1.mul(t);
        let x1 = t2.square();
        let z1 = t1.mul(u);
        let t3 = u.square();
        let w1 = t2.sub(t3.mul(x.add(z).double()));
        let t4 = z1.square();
        let z_new = w1.square();
        Point {
            x: t4.mul(b_mul4()),
            z: z_new,
            u: w1.add(z1).square().sub(t4.add(z_new)),
            t: x1.double().sub(t4.mul(FOUR).add(z_new)),
        }
    }

    /// self * k, MSB-first double-and-add over all 320 bits (always computes the addition).
    pub(crate) fn mul(&self, k: &Scalar) -> Point {
        let limbs = k.limbs();
        let mut acc = Point::NEUTRAL;
        for i in (0..320).rev() {
            acc = acc.double();
            let sum = acc.add(self);
            if (limbs[i / 64] >> (i % 64)) & 1 == 1 {
                acc = sum;
            }
        }
        acc
    }
}

fn order() -> &'static BigUint {
    static N: OnceLock<BigUint> = OnceLock::new();
    N.get_or_init(|| {
        BigUint::parse_bytes(
            b"1067993516717146951041484916571792702745057740581727230159139685185762082554198619328292418486241",
            10,
        )
        .unwrap()
    })
}

/// Scalar mod n, always reduced.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Scalar(BigUint);

impl Scalar {
    /// 40 little-endian bytes, reduced mod n (Go `ScalarElementFromLittleEndianBytes`).
    pub(crate) fn from_le_bytes(b: &[u8; 40]) -> Scalar {
        Scalar(BigUint::from_bytes_le(b) % order())
    }

    pub(crate) fn to_le_bytes(&self) -> [u8; 40] {
        let v = self.0.to_bytes_le();
        let mut out = [0u8; 40];
        out[..v.len()].copy_from_slice(&v);
        out
    }

    /// Five canonical Fp5 limbs as a 320-bit integer, reduced mod n (Go `FromGfp5`).
    pub(crate) fn from_fp5(e: &Fp5) -> Scalar {
        Scalar::from_le_bytes(&e.to_le_bytes())
    }

    fn limbs(&self) -> [u64; 5] {
        let b = self.to_le_bytes();
        let mut l = [0u64; 5];
        for (i, x) in l.iter_mut().enumerate() {
            *x = u64::from_le_bytes(b[i * 8..(i + 1) * 8].try_into().unwrap());
        }
        l
    }

    pub(crate) fn is_canonical_le(b: &[u8; 40]) -> bool {
        BigUint::from_bytes_le(b) < *order()
    }

    pub(crate) fn is_zero(&self) -> bool {
        self.0.is_zero()
    }

    #[cfg(test)]
    pub(crate) fn add(&self, o: &Scalar) -> Scalar {
        Scalar((&self.0 + &o.0) % order())
    }

    pub(crate) fn sub(&self, o: &Scalar) -> Scalar {
        Scalar((&self.0 + order() - &o.0) % order())
    }

    pub(crate) fn mul(&self, o: &Scalar) -> Scalar {
        Scalar((&self.0 * &o.0) % order())
    }

    /// Uniform in [0, n) by rejection sampling 319-bit values.
    pub(crate) fn random() -> Scalar {
        loop {
            let mut b = [0u8; 40];
            getrandom::getrandom(&mut b).expect("OS random number generator failed");
            b[39] &= 0x7f;
            if Scalar::is_canonical_le(&b) {
                return Scalar(BigUint::from_bytes_le(&b));
            }
        }
    }
}

/// Public key: encoding of sk * G.
pub(crate) fn public_key(sk: &Scalar) -> Fp5 {
    Point::generator().mul(sk).encode()
}

/// Schnorr signature with an explicit nonce k: r = k*G, e = H(r || m), s = k - e*sk.
/// Returns s (40 LE bytes) || e (40 LE bytes), as Go `Signature.ToBytes`.
pub(crate) fn sign_with_k(hashed_msg: &Fp5, sk: &Scalar, k: &Scalar) -> [u8; 80] {
    let r = Point::generator().mul(k).encode();
    let mut pre = Vec::with_capacity(10);
    pre.extend_from_slice(&r.0);
    pre.extend_from_slice(&hashed_msg.0);
    let e = Scalar::from_fp5(&hash_to_quintic_extension(&pre));
    let s = k.sub(&e.mul(sk));
    let mut out = [0u8; 80];
    out[..40].copy_from_slice(&s.to_le_bytes());
    out[40..].copy_from_slice(&e.to_le_bytes());
    out
}

pub(crate) fn sign(hashed_msg: &Fp5, sk: &Scalar) -> [u8; 80] {
    sign_with_k(hashed_msg, sk, &Scalar::random())
}

#[cfg(test)]
pub(crate) fn split_sig(sig: &[u8; 80]) -> (Scalar, Scalar) {
    let s: [u8; 40] = sig[..40].try_into().unwrap();
    let e: [u8; 40] = sig[40..].try_into().unwrap();
    (Scalar::from_le_bytes(&s), Scalar::from_le_bytes(&e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn group_law_basics() {
        let g = Point::generator();
        let two = Scalar::from_le_bytes(&{
            let mut b = [0u8; 40];
            b[0] = 2;
            b
        });
        assert_eq!(g.double().encode(), g.add(&g).encode());
        assert_eq!(g.mul(&two).encode(), g.double().encode());
        assert_eq!(g.add(&Point::NEUTRAL).encode(), g.encode());
        // n * G is the neutral point (encodes to 0).
        let n = Scalar(order().clone() - 1u32);
        assert_eq!(g.mul(&n).add(&g).encode(), Fp5::ZERO);
    }
}
