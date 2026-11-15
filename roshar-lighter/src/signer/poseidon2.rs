//! Poseidon2 over Goldilocks, width 12, rate 8, x^7 S-box, 8 full and 22 partial rounds.
//!
//! Ported from github.com/elliottech/poseidon_crypto v0.0.15. Lighter uses two Go packages,
//! `poseidon2_goldilocks_plonky2` (transaction hashes, Schnorr challenges) and `poseidon2_goldilocks`
//! (auth tokens). They implement the same permutation with the same constants, so one
//! implementation serves both.

use super::field::{Fp5, F};
use super::poseidon2_constants::{DIAG, EXTERNAL, INTERNAL};

const WIDTH: usize = 12;
const RATE: usize = 8;

fn sbox(x: F) -> F {
    let x2 = x.square();
    let x3 = x2.mul(x);
    x3.square().mul(x)
}

fn external_linear_layer(s: &mut [F; WIDTH]) {
    // Each 4-block by M4 = [[2,3,1,1],[1,2,3,1],[1,1,2,3],[3,1,1,2]].
    for i in (0..WIDTH).step_by(4) {
        let (a, b, c, d) = (s[i], s[i + 1], s[i + 2], s[i + 3]);
        let t01 = a.add(b);
        let t23 = c.add(d);
        let t0123 = t01.add(t23);
        s[i] = t0123.add(t01).add(b);
        s[i + 1] = t0123.add(b).add(c).add(c);
        s[i + 2] = t0123.add(t23).add(d);
        s[i + 3] = t0123.add(d).add(a).add(a);
    }
    let mut sums = [F::ZERO; 4];
    for (k, sum) in sums.iter_mut().enumerate() {
        *sum = s[k].add(s[k + 4]).add(s[k + 8]);
    }
    for i in 0..WIDTH {
        s[i] = s[i].add(sums[i % 4]);
    }
}

fn internal_linear_layer(s: &mut [F; WIDTH]) {
    let sum = s.iter().fold(F::ZERO, |acc, x| acc.add(*x));
    for i in 0..WIDTH {
        s[i] = s[i].mul(F(DIAG[i])).add(sum);
    }
}

fn full_rounds(s: &mut [F; WIDTH], start: usize) {
    for rc in &EXTERNAL[start..start + 4] {
        for i in 0..WIDTH {
            s[i] = sbox(s[i].add(F(rc[i])));
        }
        external_linear_layer(s);
    }
}

pub(crate) fn permute(s: &mut [F; WIDTH]) {
    external_linear_layer(s);
    full_rounds(s, 0);
    for ic in INTERNAL {
        s[0] = sbox(s[0].add(F(ic)));
        internal_linear_layer(s);
    }
    full_rounds(s, 4);
}

/// `HashNToMNoPad(input, 5)` read as an Fp5 element (`HashToQuinticExtension` in Go).
/// Absorbs by overwriting the first 8 lanes per chunk, no padding.
pub(crate) fn hash_to_quintic_extension(input: &[F]) -> Fp5 {
    let mut state = [F::ZERO; WIDTH];
    for chunk in input.chunks(RATE) {
        state[..chunk.len()].copy_from_slice(chunk);
        permute(&mut state);
    }
    Fp5([state[0], state[1], state[2], state[3], state[4]])
}

/// Bytes to field elements as in Go `goldilocks.ArrayFromCanonicalLittleEndianBytes`: 8-byte
/// little-endian chunks, the last one zero-padded. Errors on a chunk >= p.
pub(crate) fn bytes_to_elements(b: &[u8]) -> Result<Vec<F>, String> {
    b.chunks(8)
        .map(|c| {
            let mut buf = [0u8; 8];
            buf[..c.len()].copy_from_slice(c);
            let v = u64::from_le_bytes(buf);
            if v >= super::field::P {
                Err(format!("non-canonical field element {v}"))
            } else {
                Ok(F(v))
            }
        })
        .collect()
}
