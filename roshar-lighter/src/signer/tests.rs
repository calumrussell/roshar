//! Parity with the official signer: `tests/vectors.json` was produced by lighter-sdk's Go library
//! (`tests/gen_vectors.py`). Never regenerate it to make a change pass.
//!
//! The Schnorr nonce k is random, so for each recorded signature (s, e) we recover
//! k = s + e*sk and check that signing the same transaction with that k reproduces the whole
//! `tx_info` (signature included), `tx_hash` and `tx_type` exactly. That pins the message hash, the
//! curve arithmetic (r = k*G must hash to the recorded e) and the JSON layout.

use super::curve::{self, split_sig, Point, Scalar};
use super::*;
use serde_json::Value;

const VECTORS: &str = include_str!("../../tests/vectors.json");

fn vectors() -> Value {
    serde_json::from_str(VECTORS).unwrap()
}

fn signer(v: &Value, chain: &str) -> Signer {
    let key = ApiKey::from_hex(v["private_key"].as_str().unwrap()).unwrap();
    Signer::new(
        key,
        v["account_index"].as_i64().unwrap(),
        v["api_key_index"].as_u64().unwrap() as u8,
        chain.parse().unwrap(),
    )
    .unwrap()
}

fn info(t: &Value) -> Value {
    serde_json::from_str(t["tx_info"].as_str().unwrap()).unwrap()
}

fn recorded_sig(sig_b64: &str) -> [u8; 80] {
    base64::engine::general_purpose::STANDARD.decode(sig_b64).unwrap().try_into().unwrap()
}

/// k = s + e*sk for the recorded signature.
fn recover_k(s: &Signer, sig: &[u8; 80]) -> Scalar {
    let (sv, ev) = split_sig(sig);
    sv.add(&ev.mul(&s.key.sk))
}

fn opts(i: &Value) -> TxOpts {
    TxOpts { nonce: i["Nonce"].as_i64().unwrap(), expired_at_ms: i["ExpiredAt"].as_i64().unwrap() }
}

fn assert_same(got: &SignedTx, want: &Value) {
    assert_eq!(got.tx_type as u64, want["tx_type"].as_u64().unwrap());
    assert_eq!(got.tx_info, want["tx_info"].as_str().unwrap());
    assert_eq!(got.tx_hash, want["tx_hash"].as_str().unwrap());
}

/// Verifies with the public point sk*G: e == H(encode(s*G + e*PK) || m).
fn verifies(s: &Signer, msg_hash_hex: &str, sig: &[u8; 80]) -> bool {
    let h: [u8; 40] = hex::decode(msg_hash_hex).unwrap().try_into().unwrap();
    let h = Fp5::from_le_bytes(&h);
    let (sv, ev) = split_sig(sig);
    let g = Point::generator();
    let pk = g.mul(&s.key.sk);
    let r = g.mul(&sv).add(&pk.mul(&ev)).encode();
    let mut pre = r.0.to_vec();
    pre.extend_from_slice(&h.0);
    Scalar::from_fp5(&poseidon2::hash_to_quintic_extension(&pre)) == ev
}

fn chains(v: &Value) -> Vec<(String, Value)> {
    v["chains"].as_object().unwrap().iter().map(|(k, c)| (k.clone(), c.clone())).collect()
}

#[test]
fn public_key_matches_sdk() {
    let v = vectors();
    let key = ApiKey::from_hex(v["private_key"].as_str().unwrap()).unwrap();
    assert_eq!(key.public_key_hex(), v["public_key"].as_str().unwrap());
}

#[test]
fn create_order_vectors() {
    let v = vectors();
    let mut n = 0;
    for (chain, c) in chains(&v) {
        let s = signer(&v, &chain);
        for t in c["create_order"].as_array().unwrap() {
            let i = info(t);
            let o = CreateOrder {
                market_index: t["market_index"].as_i64().unwrap(),
                client_order_index: t["client_order_index"].as_i64().unwrap(),
                base_amount: t["base_amount"].as_i64().unwrap(),
                price: t["price"].as_u64().unwrap() as u32,
                is_ask: t["is_ask"].as_u64().unwrap() == 1,
                order_type: [OrderType::Limit, OrderType::Market][t["order_type"].as_u64().unwrap() as usize],
                time_in_force: [TimeInForce::ImmediateOrCancel, TimeInForce::GoodTillTime, TimeInForce::PostOnly]
                    [t["time_in_force"].as_u64().unwrap() as usize],
                reduce_only: t["reduce_only"].as_u64().unwrap() == 1,
                trigger_price: t["trigger_price"].as_u64().unwrap() as u32,
                // -1 was filled in by the library from its clock.
                order_expiry: i["OrderExpiry"].as_i64().unwrap(),
            };
            if t["order_expiry_arg"].as_i64().unwrap() != -1 {
                assert_eq!(o.order_expiry, t["order_expiry_arg"].as_i64().unwrap());
            }
            let sig = recorded_sig(i["Sig"].as_str().unwrap());
            let k = recover_k(&s, &sig);
            assert_same(&s.create_order(&o, opts(&i), Some(&k)).unwrap(), t);
            assert!(verifies(&s, t["tx_hash"].as_str().unwrap(), &sig));
            // A fresh random signature differs but verifies, and nothing else changes.
            let fresh = s.sign_create_order(&o, opts(&i)).unwrap();
            assert_eq!(fresh.tx_hash, t["tx_hash"].as_str().unwrap());
            let fi = info(&serde_json::json!({"tx_info": fresh.tx_info}));
            assert!(verifies(&s, &fresh.tx_hash, &recorded_sig(fi["Sig"].as_str().unwrap())));
            n += 1;
        }
    }
    assert_eq!(n, 20);
}

#[test]
fn cancel_order_vectors() {
    let v = vectors();
    for (chain, c) in chains(&v) {
        let s = signer(&v, &chain);
        for t in c["cancel_order"].as_array().unwrap() {
            let i = info(t);
            let o = CancelOrder { market_index: t["market_index"].as_i64().unwrap(), index: t["index"].as_i64().unwrap() };
            let k = recover_k(&s, &recorded_sig(i["Sig"].as_str().unwrap()));
            assert_same(&s.cancel_order(&o, opts(&i), Some(&k)).unwrap(), t);
        }
    }
}

#[test]
fn cancel_all_orders_vectors() {
    let v = vectors();
    for (chain, c) in chains(&v) {
        let s = signer(&v, &chain);
        for t in c["cancel_all_orders"].as_array().unwrap() {
            let i = info(t);
            let m = t["market_index"].as_i64().unwrap();
            let o = CancelAllOrders {
                time_in_force: [CancelAllTif::Immediate, CancelAllTif::Scheduled, CancelAllTif::Abort]
                    [t["time_in_force"].as_u64().unwrap() as usize],
                time: t["time"].as_i64().unwrap(),
                market_index: if m == 255 { None } else { Some(m) },
            };
            let k = recover_k(&s, &recorded_sig(i["Sig"].as_str().unwrap()));
            assert_same(&s.cancel_all_orders(&o, opts(&i), Some(&k)).unwrap(), t);
        }
    }
}

#[test]
fn modify_order_vectors() {
    let v = vectors();
    for (chain, c) in chains(&v) {
        let s = signer(&v, &chain);
        for t in c["modify_order"].as_array().unwrap() {
            let i = info(t);
            let o = ModifyOrder {
                market_index: t["market_index"].as_i64().unwrap(),
                index: t["index"].as_i64().unwrap(),
                base_amount: t["base_amount"].as_i64().unwrap(),
                price: t["price"].as_u64().unwrap() as u32,
                trigger_price: t["trigger_price"].as_u64().unwrap() as u32,
            };
            let k = recover_k(&s, &recorded_sig(i["Sig"].as_str().unwrap()));
            assert_same(&s.modify_order(&o, opts(&i), Some(&k)).unwrap(), t);
        }
    }
}

#[test]
fn auth_token_vectors() {
    let v = vectors();
    for (chain, c) in chains(&v) {
        let s = signer(&v, &chain);
        for t in c["auth_token"].as_array().unwrap() {
            let token = t["token"].as_str().unwrap();
            let sig_hex = token.rsplit(':').next().unwrap();
            let sig: [u8; 80] = hex::decode(sig_hex).unwrap().try_into().unwrap();
            let k = recover_k(&s, &sig);
            let got = s.auth_token_with(t["deadline"].as_i64().unwrap(), Some(&k)).unwrap();
            assert_eq!(got, token);
            let fresh = s.auth_token(t["deadline"].as_i64().unwrap()).unwrap();
            assert_eq!(fresh.rsplit_once(':').map(|x| x.0), token.rsplit_once(':').map(|x| x.0));
        }
    }
}

#[test]
fn rejects_what_the_go_signer_rejects() {
    let v = vectors();
    let s = signer(&v, "304");
    let o = TxOpts { nonce: 1, expired_at_ms: 1_800_000_000_000 };
    let scheduled_one_market =
        CancelAllOrders { time_in_force: CancelAllTif::Scheduled, time: 1_800_000_000_000, market_index: Some(2) };
    assert!(s.sign_cancel_all_orders(&scheduled_one_market, o).is_err());
    let post_only_no_expiry = CreateOrder {
        market_index: 1,
        client_order_index: 1,
        base_amount: 1,
        price: 1,
        is_ask: false,
        order_type: OrderType::Limit,
        time_in_force: TimeInForce::PostOnly,
        reduce_only: false,
        trigger_price: 0,
        order_expiry: 0,
    };
    assert!(s.sign_create_order(&post_only_no_expiry, o).is_err());
    assert!(s.sign_cancel_order(&CancelOrder { market_index: 255, index: 1 }, o).is_err());
    assert!(ApiKey::from_hex("0x1234").is_err());
    let _ = curve::public_key; // keep the import used in all cfgs
}
