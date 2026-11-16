//! Rescue over BN254 `Fr` as franklin-crypto's `Bn256RescueParams::new_checked_2_into_1`: state
//! width 3 (rate 2, capacity 1), 22 rounds of each S-box (`x^(1/5)` then `x^5`), sponge state
//! specialised with the input length, padding with ones.

use super::constants::{RESCUE_ALPHA_INV, RESCUE_MDS, RESCUE_ROUND_CONSTANTS};
use super::field::{limbs_to_le, Fr};
use std::sync::LazyLock;

const WIDTH: usize = 3;
const RATE: usize = 2;
const ROUNDS: usize = 22;
/// `Fr::CAPACITY`: bits packed into one field element by `multipack::compute_multipacking`.
const FR_CAPACITY: usize = 253;

struct Params {
    round_constants: [Fr; RESCUE_ROUND_CONSTANTS.len()],
    mds: [Fr; WIDTH * WIDTH],
}

static PARAMS: LazyLock<Params> = LazyLock::new(|| Params {
    round_constants: std::array::from_fn(|i| {
        Fr::from_canonical(RESCUE_ROUND_CONSTANTS[i]).expect("round constant is canonical")
    }),
    mds: std::array::from_fn(|i| {
        Fr::from_canonical(RESCUE_MDS[i]).expect("mds entry is canonical")
    }),
});

/// `rescue_mimc`.
fn permute(state: &mut [Fr; WIDTH]) {
    let p = &*PARAMS;
    for (s, c) in state.iter_mut().zip(&p.round_constants[0..WIDTH]) {
        *s = s.add(*c);
    }
    for round in 0..2 * ROUNDS {
        if round % 2 == 0 {
            for s in state.iter_mut() {
                *s = s.pow(&RESCUE_ALPHA_INV);
            }
        } else {
            for s in state.iter_mut() {
                let sq = s.square();
                *s = s.mul(sq.square());
            }
        }
        let rc = &p.round_constants[WIDTH * (round + 1)..WIDTH * (round + 2)];
        let mut next = [Fr::ZERO; WIDTH];
        for (row, out) in next.iter_mut().enumerate() {
            let mut acc = rc[row];
            for col in 0..WIDTH {
                acc = acc.add(state[col].mul(p.mds[row * WIDTH + col]));
            }
            *out = acc;
        }
        *state = next;
    }
}

/// Absorbs `inputs` into a fresh sponge specialised with `inputs.len()` and returns the rate part
/// of the final state: `[0]` is `rescue_hash`'s output, `[1]` the second squeeze.
fn sponge(inputs: &[Fr]) -> [Fr; RATE] {
    let mut state = [Fr::ZERO; WIDTH];
    state[WIDTH - 1] = Fr::from_u64(inputs.len() as u64);
    for chunk in inputs.chunks(RATE) {
        for i in 0..RATE {
            let value = chunk.get(i).copied().unwrap_or_else(Fr::one);
            state[i] = state[i].add(value);
        }
        permute(&mut state);
    }
    [state[0], state[1]]
}

/// `multipack::compute_multipacking` over the big-endian bits of `bytes`, zero-padded to
/// `N * 253` bits: each element takes the next 253 bits, first bit least significant.
fn multipack<const N: usize>(bytes: &[u8]) -> [Fr; N] {
    debug_assert!(bytes.len() * 8 <= N * FR_CAPACITY);
    std::array::from_fn(|n| {
        let mut limbs = [0u64; 4];
        for j in 0..FR_CAPACITY {
            let bit = n * FR_CAPACITY + j;
            let byte = bit / 8;
            if byte < bytes.len() && (bytes[byte] >> (7 - bit % 8)) & 1 == 1 {
                limbs[j / 64] |= 1 << (j % 64);
            }
        }
        Fr::from_canonical(limbs).expect("253-bit value is below the modulus")
    })
}

/// Largest message `rescue_hash_tx_msg` accepts: 736 bits.
pub const MAX_TX_MSG_LEN: usize = 92;

/// zkLink `rescue_hash_tx_msg`: the message's bits zero-padded to 736, packed into 3 elements and
/// hashed; the output is the hash's 32-byte little-endian encoding.
pub(crate) fn hash_tx_msg(msg: &[u8]) -> [u8; 32] {
    debug_assert!(msg.len() <= MAX_TX_MSG_LEN);
    let inputs = multipack::<3>(msg);
    limbs_to_le(&sponge(&inputs)[0].to_canonical())
}

/// franklin-crypto `rescue_hash_to_scalar(&[], pk_x || r_x, msg)`: the musig challenge. The low 125
/// bits of the first two squeezes, concatenated, as an `Fs` integer (always below 2^250).
pub(crate) fn challenge(pk_x: Fr, r_x: Fr, msg: &[u8; 32]) -> [u64; 4] {
    let mut bytes = [0u8; 96];
    bytes[0..32].copy_from_slice(&limbs_to_le(&pk_x.to_canonical()));
    bytes[32..64].copy_from_slice(&limbs_to_le(&r_x.to_canonical()));
    bytes[64..96].copy_from_slice(msg);
    let inputs = multipack::<4>(&bytes);
    let [s0, s1] = sponge(&inputs);
    let lo = low_125(s0.to_canonical());
    let hi = low_125(s1.to_canonical());
    // lo + hi << 125
    [
        lo[0],
        lo[1] | (hi[0] << 61),
        (hi[0] >> 3) | (hi[1] << 61),
        hi[1] >> 3,
    ]
}

fn low_125(limbs: [u64; 4]) -> [u64; 2] {
    [limbs[0], limbs[1] & ((1u64 << 61) - 1)]
}

/// zkLink `PackedPublicKey::public_key_hash`: low 160 bits of `rescue_hash([x, y])`, big-endian.
pub(crate) fn public_key_hash(x: Fr, y: Fr) -> [u8; 20] {
    let le = limbs_to_le(&sponge(&[x, y])[0].to_canonical());
    let mut out = [0u8; 20];
    out.copy_from_slice(&le[..20]);
    out.reverse();
    out
}
