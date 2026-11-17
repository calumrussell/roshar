//! Parity with the official SDK: `tests/vectors.json` was produced by paradex-py
//! (`tests/gen_vectors.py`). Never regenerate it to make a change pass.
//!
//! paradex-py signs with an RFC 6979 nonce (seed 32), so message hashes, signatures and request
//! bodies must match byte for byte.

use roshar_paradex::exchange::{Instruction, ModifyOrder, NewOrder, OrderKind, Stp};
use roshar_paradex::signer::{
    auth_message_hash, felt_from_hex, modify_order_message_hash, order_message_hash, quantize,
    verify_signature, OrderSide, ParadexSigner, Signature,
};
use serde_json::{Map, Value};

const VECTORS: &str = include_str!("vectors.json");

fn vectors() -> Value {
    serde_json::from_str(VECTORS).unwrap()
}

fn s(v: &Value) -> &str {
    v.as_str().unwrap()
}

fn u(v: &Value) -> u64 {
    v.as_u64().unwrap()
}

fn side(v: &Value) -> OrderSide {
    serde_json::from_value(v.clone()).unwrap()
}

fn check_signature(
    signer: &ParadexSigner,
    case: &Value,
    hash: &roshar_paradex::signer::Felt,
    sig: &Signature,
) {
    assert_eq!(*hash, felt_from_hex(s(&case["message_hash"])).unwrap());
    assert_eq!(sig.r.to_string(), s(&case["r"]));
    assert_eq!(sig.s.to_string(), s(&case["s"]));
    assert_eq!(sig.flattened(), s(&case["signature"]));
    assert!(verify_signature(&signer.public_key(), hash, sig).unwrap());
}

/// The SDK sends unset optional fields as `null` (and an unset client id as `""`); the Rust
/// requests leave them out.
fn without_unset(body: &Value) -> Value {
    let m: Map<String, Value> = body
        .as_object()
        .unwrap()
        .iter()
        .filter(|(k, v)| !v.is_null() && (k.as_str() != "client_id" || v.as_str() != Some("")))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    Value::Object(m)
}

#[test]
fn public_key_matches_sdk() {
    let v = vectors();
    for name in v["chains"].as_object().unwrap().keys() {
        let signer = ParadexSigner::from_hex(s(&v["private_key"]), s(&v["account"]), name).unwrap();
        assert_eq!(signer.public_key_hex(), s(&v["public_key"]));
        assert_eq!(signer.account_hex(), s(&v["account"]));
        assert_eq!(
            signer.chain_id(),
            felt_from_hex(s(&v["chains"][name]["chain_id"])).unwrap()
        );
    }
}

#[test]
fn auth_matches_sdk() {
    let v = vectors();
    for (name, c) in v["chains"].as_object().unwrap() {
        let signer = ParadexSigner::from_hex(s(&v["private_key"]), s(&v["account"]), name).unwrap();
        for case in c["auth"].as_array().unwrap() {
            let (ts, exp) = (u(&case["timestamp"]), u(&case["expiration"]));
            let hash = signer.auth_message_hash(ts, exp);
            assert_eq!(
                hash,
                auth_message_hash(signer.chain_id(), signer.account(), ts, exp)
            );
            check_signature(&signer, case, &hash, &signer.sign_auth(ts, exp).unwrap());
        }
    }
}

#[test]
fn orders_match_sdk() {
    let v = vectors();
    let mut n = 0;
    for (name, c) in v["chains"].as_object().unwrap() {
        let signer = ParadexSigner::from_hex(s(&v["private_key"]), s(&v["account"]), name).unwrap();
        for case in c["orders"].as_array().unwrap() {
            let kind = match s(&case["type"]) {
                "MARKET" => OrderKind::Market,
                "LIMIT" => OrderKind::Limit {
                    price: s(&case["price"]),
                },
                t => panic!("type {t}"),
            };
            let order = NewOrder {
                market: s(&case["market"]),
                side: side(&case["side"]),
                kind,
                size: s(&case["size"]),
                instruction: serde_json::from_value::<Instruction>(case["instruction"].clone())
                    .unwrap(),
                client_id: Some(s(&case["client_id"])).filter(|c| !c.is_empty()),
                reduce_only: case["reduce_only"].as_bool().unwrap(),
                stp: serde_json::from_value::<Option<Stp>>(case["stp"].clone()).unwrap(),
                recv_window_ms: case["recv_window"].as_u64(),
                signature_timestamp_ms: u(&case["signature_timestamp"]),
            };

            assert_eq!(
                quantize(order.size).unwrap(),
                s(&case["chain_size"]),
                "{case}"
            );
            if let OrderKind::Limit { price } = order.kind {
                assert_eq!(quantize(price).unwrap(), s(&case["chain_price"]), "{case}");
            } else {
                assert_eq!(s(&case["chain_price"]), "0");
            }

            let hash = signer.order_message_hash(&order.message()).unwrap();
            assert_eq!(
                hash,
                order_message_hash(signer.chain_id(), signer.account(), &order.message()).unwrap()
            );
            check_signature(
                &signer,
                case,
                &hash,
                &signer.sign_order(&order.message()).unwrap(),
            );

            let req = order.sign(&signer).unwrap();
            assert_eq!(req.signature(), s(&case["signature"]));
            // paradex-py writes str(Decimal) into the body (e.g. "1.9E-8"); the crate sends the
            // caller's decimal string verbatim. The signed (quantized) values are checked above,
            // so compare price/size against the inputs and everything else byte for byte.
            let mut got = serde_json::to_value(&req).unwrap();
            let mut want = without_unset(&case["body"]);
            for field in ["price", "size"] {
                let g = got.as_object_mut().unwrap().remove(field);
                want.as_object_mut().unwrap().remove(field);
                if let Some(g) = g {
                    assert_eq!(g, case[field], "{field} in {case}");
                }
            }
            assert_eq!(got, want, "{case}");
            n += 1;
        }
    }
    assert!(n > 0);
}

#[test]
fn modifies_match_sdk() {
    let v = vectors();
    for (name, c) in v["chains"].as_object().unwrap() {
        let signer = ParadexSigner::from_hex(s(&v["private_key"]), s(&v["account"]), name).unwrap();
        for case in c["modifies"].as_array().unwrap() {
            let m = ModifyOrder {
                order_id: s(&case["id"]),
                market: s(&case["market"]),
                side: side(&case["side"]),
                size: s(&case["size"]),
                price: s(&case["price"]),
                signature_timestamp_ms: u(&case["signature_timestamp"]),
            };
            let hash = signer
                .modify_order_message_hash(&m.message(), m.order_id)
                .unwrap();
            assert_eq!(
                hash,
                modify_order_message_hash(
                    signer.chain_id(),
                    signer.account(),
                    &m.message(),
                    m.order_id
                )
                .unwrap()
            );
            check_signature(
                &signer,
                case,
                &hash,
                &signer.sign_modify_order(&m.message(), m.order_id).unwrap(),
            );

            // The documented ModifyOrderRequest fields, each equal to what the SDK sends.
            let ours = serde_json::to_value(m.sign(&signer).unwrap()).unwrap();
            let sdk = case["body"].as_object().unwrap();
            let ours = ours.as_object().unwrap();
            let mut keys: Vec<&str> = ours.keys().map(String::as_str).collect();
            keys.sort_unstable();
            assert_eq!(
                keys,
                [
                    "id",
                    "market",
                    "price",
                    "side",
                    "signature",
                    "signature_timestamp",
                    "size",
                    "type"
                ]
            );
            for (k, val) in ours {
                assert_eq!(Some(val), sdk.get(k), "{k}");
            }
        }
    }
}

#[test]
fn market_orders_reject_a_price_and_limits_need_one() {
    let v = vectors();
    let signer = ParadexSigner::from_hex(
        s(&v["private_key"]),
        s(&v["account"]),
        roshar_paradex::signer::MAINNET_CHAIN_NAME,
    )
    .unwrap();
    let mut msg = NewOrder {
        market: "BTC-USD-PERP",
        side: OrderSide::Buy,
        kind: OrderKind::Market,
        size: "1",
        instruction: Instruction::Gtc,
        client_id: None,
        reduce_only: false,
        stp: None,
        recv_window_ms: None,
        signature_timestamp_ms: 1,
    }
    .message();
    msg.price = Some("1");
    assert!(signer.order_message_hash(&msg).is_err());
    msg.order_type = roshar_paradex::signer::OrderType::Limit;
    msg.price = None;
    assert!(signer.order_message_hash(&msg).is_err());
}
