//! Alt-BabyJubjub: the twisted Edwards curve `-x^2 + y^2 = 1 + d x^2 y^2` over BN254's scalar
//! field (franklin-crypto `alt_babyjubjub`). Points are kept in extended coordinates; the unified
//! addition law (`add-2008-hwcd-3`, a = -1) is complete on this curve.

use super::constants::{EDWARDS_D, GENERATOR_X, GENERATOR_Y};
use super::field::{limbs_to_le, Fr};

#[derive(Clone, Copy, Debug)]
pub(crate) struct Point {
    x: Fr,
    y: Fr,
    z: Fr,
    t: Fr,
}

impl Point {
    fn identity() -> Self {
        Self {
            x: Fr::ZERO,
            y: Fr::one(),
            z: Fr::one(),
            t: Fr::ZERO,
        }
    }

    /// `FixedGenerators::SpendingKeyGenerator`, the base point of zkLink keys and signatures.
    pub(crate) fn generator() -> Self {
        let x = Fr::from_canonical(GENERATOR_X).expect("generator x is canonical");
        let y = Fr::from_canonical(GENERATOR_Y).expect("generator y is canonical");
        Self {
            x,
            y,
            z: Fr::one(),
            t: x.mul(y),
        }
    }

    /// `2d`, hoisted out of the addition loop.
    fn d2() -> Fr {
        Fr::from_canonical(EDWARDS_D)
            .expect("edwards d is canonical")
            .double()
    }

    fn add(&self, other: &Self, d2: Fr) -> Self {
        let a = self.y.sub(self.x).mul(other.y.sub(other.x));
        let b = self.y.add(self.x).mul(other.y.add(other.x));
        let c = self.t.mul(d2).mul(other.t);
        let d = self.z.double().mul(other.z);
        let e = b.sub(a);
        let f = d.sub(c);
        let g = d.add(c);
        let h = b.add(a);
        Self {
            x: e.mul(f),
            y: g.mul(h),
            z: f.mul(g),
            t: e.mul(h),
        }
    }

    /// `scalar * self`, `scalar` a little-endian integer.
    pub(crate) fn mul(&self, scalar: &[u64; 4]) -> Self {
        let d2 = Self::d2();
        let mut acc = Self::identity();
        for i in (0..256).rev() {
            acc = acc.add(&acc, d2);
            if (scalar[i / 64] >> (i % 64)) & 1 == 1 {
                acc = acc.add(self, d2);
            }
        }
        acc
    }

    /// Affine `(x, y)`.
    pub(crate) fn to_affine(&self) -> (Fr, Fr) {
        let z_inv = self.z.invert();
        (self.x.mul(z_inv), self.y.mul(z_inv))
    }
}

/// franklin-crypto `Point::write`: `y` little-endian with the top bit set when `x` is odd.
pub(crate) fn pack_point(x: Fr, y: Fr) -> [u8; 32] {
    let mut out = limbs_to_le(&y.to_canonical());
    if x.to_canonical()[0] & 1 == 1 {
        out[31] |= 0x80;
    }
    out
}
