//! Goldilocks field (p = 2^64 - 2^32 + 1) and its quintic extension GF(p^5) = GF(p)[x]/(x^5 - 3).
//!
//! Ported from github.com/elliottech/poseidon_crypto v0.0.15 (`field/goldilocks`,
//! `field/goldilocks_quintic_extension`). Elements are always kept canonical (< p); the Go code
//! keeps some values non-canonical between operations, which gives the same residues.

pub(crate) const P: u64 = 0xffff_ffff_0000_0001;
const EPSILON: u64 = 0xffff_ffff; // 2^64 mod p

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct F(pub(crate) u64);

impl F {
    pub(crate) const ZERO: F = F(0);
    pub(crate) const ONE: F = F(1);

    /// Reduces any u64 (the Go code casts signed fields straight to `GoldilocksField`).
    pub(crate) fn new(x: u64) -> F {
        F(if x >= P { x - P } else { x })
    }

    pub(crate) fn add(self, o: F) -> F {
        let (s, over) = self.0.overflowing_add(o.0);
        // s + 2^64 = s + EPSILON (mod p)
        let (s, over2) = s.overflowing_add(if over { EPSILON } else { 0 });
        let s = if over2 { s + EPSILON } else { s };
        F::new(s)
    }

    pub(crate) fn sub(self, o: F) -> F {
        let (d, borrow) = self.0.overflowing_sub(o.0);
        F::new(if borrow { d.wrapping_add(P) } else { d })
    }

    pub(crate) fn mul(self, o: F) -> F {
        F(reduce128((self.0 as u128) * (o.0 as u128)))
    }

    pub(crate) fn square(self) -> F {
        self.mul(self)
    }

    pub(crate) fn pow(self, mut e: u64) -> F {
        let mut base = self;
        let mut acc = F::ONE;
        while e > 0 {
            if e & 1 == 1 {
                acc = acc.mul(base);
            }
            base = base.square();
            e >>= 1;
        }
        acc
    }

    pub(crate) fn inverse_or_zero(self) -> F {
        // x^(p-2); 0 maps to 0.
        self.pow(P - 2)
    }
}

fn reduce128(x: u128) -> u64 {
    let lo = x as u64;
    let hi = (x >> 64) as u64;
    let hi_hi = hi >> 32;
    let hi_lo = hi & EPSILON;
    let (mut t0, borrow) = lo.overflowing_sub(hi_hi);
    if borrow {
        t0 = t0.wrapping_sub(EPSILON);
    }
    let t1 = hi_lo * EPSILON;
    let (s, carry) = t0.overflowing_add(t1);
    let s = s.wrapping_add(if carry { EPSILON } else { 0 });
    if s >= P {
        s - P
    } else {
        s
    }
}

/// Element of GF(p^5), little-endian coefficients.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Fp5(pub(crate) [F; 5]);

const W: F = F(3);
const DTH_ROOT: F = F(1041288259238279555);

impl Fp5 {
    pub(crate) const ZERO: Fp5 = Fp5([F::ZERO; 5]);
    pub(crate) const ONE: Fp5 = Fp5([F::ONE, F::ZERO, F::ZERO, F::ZERO, F::ZERO]);

    pub(crate) fn from_u64s(v: [u64; 5]) -> Fp5 {
        Fp5([
            F::new(v[0]),
            F::new(v[1]),
            F::new(v[2]),
            F::new(v[3]),
            F::new(v[4]),
        ])
    }

    pub(crate) fn is_zero(&self) -> bool {
        self.0.iter().all(|f| f.0 == 0)
    }

    /// 40 bytes, five canonical little-endian u64 limbs.
    pub(crate) fn to_le_bytes(self) -> [u8; 40] {
        let mut out = [0u8; 40];
        for (i, f) in self.0.iter().enumerate() {
            out[i * 8..(i + 1) * 8].copy_from_slice(&f.0.to_le_bytes());
        }
        out
    }

    /// Inverse of `to_le_bytes`. Like the Go code, limbs >= p are accepted and reduced.
    #[cfg(test)]
    pub(crate) fn from_le_bytes(b: &[u8; 40]) -> Fp5 {
        let mut v = [0u64; 5];
        for (i, limb) in v.iter_mut().enumerate() {
            *limb = u64::from_le_bytes(b[i * 8..(i + 1) * 8].try_into().unwrap());
        }
        Fp5::from_u64s(v)
    }

    pub(crate) fn add(self, o: Fp5) -> Fp5 {
        let mut r = [F::ZERO; 5];
        for (i, x) in r.iter_mut().enumerate() {
            *x = self.0[i].add(o.0[i]);
        }
        Fp5(r)
    }

    pub(crate) fn sub(self, o: Fp5) -> Fp5 {
        let mut r = [F::ZERO; 5];
        for (i, x) in r.iter_mut().enumerate() {
            *x = self.0[i].sub(o.0[i]);
        }
        Fp5(r)
    }

    pub(crate) fn double(self) -> Fp5 {
        self.add(self)
    }

    pub(crate) fn mul(self, o: Fp5) -> Fp5 {
        let a = &self.0;
        let b = &o.0;
        // c_k = sum_{i+j=k} a_i b_j + 3 * sum_{i+j=k+5} a_i b_j
        let mut c = [F::ZERO; 5];
        for (i, ai) in a.iter().enumerate() {
            for (j, bj) in b.iter().enumerate() {
                let prod = ai.mul(*bj);
                let k = i + j;
                if k < 5 {
                    c[k] = c[k].add(prod);
                } else {
                    c[k - 5] = c[k - 5].add(W.mul(prod));
                }
            }
        }
        Fp5(c)
    }

    pub(crate) fn square(self) -> Fp5 {
        self.mul(self)
    }

    fn scalar_mul(self, s: F) -> Fp5 {
        Fp5(self.0.map(|x| x.mul(s)))
    }

    fn repeated_frobenius(self, count: usize) -> Fp5 {
        let count = count % 5;
        if count == 0 {
            return self;
        }
        let mut z0 = DTH_ROOT;
        for _ in 1..count {
            z0 = DTH_ROOT.mul(z0);
        }
        let mut res = [F::ZERO; 5];
        let mut z = F::ONE;
        for (i, r) in res.iter_mut().enumerate() {
            *r = self.0[i].mul(z);
            z = z.mul(z0);
        }
        Fp5(res)
    }

    pub(crate) fn inverse_or_zero(self) -> Fp5 {
        if self.is_zero() {
            return Fp5::ZERO;
        }
        let d = self.repeated_frobenius(1);
        let e = d.mul(d.repeated_frobenius(1));
        let f = e.mul(e.repeated_frobenius(2));
        // g = (self * f)[0], which lies in the base field.
        let g = self.mul(f).0[0];
        f.scalar_mul(g.inverse_or_zero())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_field_matches_u128_arithmetic() {
        let vals = [
            0u64,
            1,
            2,
            P - 1,
            P - 2,
            EPSILON,
            1 << 63,
            0x1234_5678_9abc_def0 % P,
            P / 3,
        ];
        for &a in &vals {
            for &b in &vals {
                let (fa, fb) = (F::new(a), F::new(b));
                let p = P as u128;
                assert_eq!(fa.add(fb).0 as u128, (a as u128 + b as u128) % p);
                assert_eq!(fa.mul(fb).0 as u128, (a as u128 * b as u128) % p);
                assert_eq!(fa.sub(fb).0 as u128, (a as u128 + p - b as u128) % p);
            }
        }
        assert_eq!(F::new(u64::MAX).0, u64::MAX - P);
    }

    #[test]
    fn inverses() {
        let a = Fp5::from_u64s([5, 7, 11, 13, P - 1]);
        assert_eq!(a.mul(a.inverse_or_zero()), Fp5::ONE);
        let x = F(123456789);
        assert_eq!(x.mul(x.inverse_or_zero()), F::ONE);
    }
}
