//! Signing parity with `@gmx-io/sdk` (`signPreparedOrder` / `PrivateKeySigner`, viem hashing).
//! tests/vectors.json is produced by tests/gen_vectors.mjs; never regenerate it to make a
//! change pass.

use roshar_gmx::signer::{
    hash_typed_data, validate_order_typed_data, GmxSigner, ARBITRUM_RELAY_ROUTERS,
    AVALANCHE_RELAY_ROUTERS,
};
use roshar_gmx::types::{GmxPreparedOrder, GmxTypedData};
use serde_json::Value;

const VECTORS: &str = include_str!("vectors.json");

fn fixture(name: &str) -> &'static str {
    match name {
        "prepare_increase_limit.json" => include_str!("fixtures/prepare_increase_limit.json"),
        "prepare_increase_market.json" => include_str!("fixtures/prepare_increase_market.json"),
        "prepare_decrease_market.json" => include_str!("fixtures/prepare_decrease_market.json"),
        "prepare_edit.json" => include_str!("fixtures/prepare_edit.json"),
        "prepare_cancel.json" => include_str!("fixtures/prepare_cancel.json"),
        "prepare_cancel_all.json" => include_str!("fixtures/prepare_cancel_all.json"),
        other => panic!("no fixture {other}"),
    }
}

fn hex32(b: &[u8; 32]) -> String {
    format!("0x{}", hex::encode(b))
}

fn routers(chain_id: u64) -> &'static [&'static str] {
    match chain_id {
        42161 => ARBITRUM_RELAY_ROUTERS,
        43114 => AVALANCHE_RELAY_ROUTERS,
        other => panic!("no routers for {other}"),
    }
}

fn vectors() -> Value {
    serde_json::from_str(VECTORS).unwrap()
}

fn signer(v: &Value) -> GmxSigner {
    let s = GmxSigner::from_hex(v["privateKey"].as_str().unwrap()).unwrap();
    assert_eq!(s.address(), v["address"].as_str().unwrap());
    s
}

#[test]
fn prepared_orders_match_sdk() {
    let v = vectors();
    let signer = signer(&v);
    let orders = v["orders"].as_array().unwrap();
    assert_eq!(orders.len(), 6);
    for o in orders {
        let name = o["fixture"].as_str().unwrap();
        let prepared: GmxPreparedOrder = serde_json::from_str(fixture(name)).unwrap();
        let td = &prepared.express().unwrap().typed_data;

        let h = hash_typed_data(td).unwrap();
        assert_eq!(h.primary_type, o["primaryType"].as_str().unwrap(), "{name}");
        assert_eq!(
            hex32(&h.domain_separator),
            o["domainSeparator"].as_str().unwrap(),
            "{name}"
        );
        assert_eq!(
            hex32(&h.struct_hash),
            o["structHash"].as_str().unwrap(),
            "{name}"
        );
        assert_eq!(hex32(&h.digest), o["digest"].as_str().unwrap(), "{name}");

        let chain_id = o["chainId"].as_u64().unwrap();
        let sig = signer
            .sign_prepared_order(
                &prepared,
                chain_id,
                o["account"].as_str(),
                routers(chain_id),
            )
            .unwrap();
        assert_eq!(sig, o["signature"].as_str().unwrap(), "{name}");
    }
}

#[test]
fn validation_matches_sdk() {
    let v = vectors();
    let signer = signer(&v);
    for c in v["validation"].as_array().unwrap() {
        let name = c["name"].as_str().unwrap();
        let td: GmxTypedData = serde_json::from_value(c["typedData"].clone()).unwrap();
        let chain_id = c["chainId"].as_u64().unwrap();
        let got = validate_order_typed_data(
            &td,
            chain_id,
            &signer.address(),
            c["account"].as_str(),
            routers(chain_id),
        );
        assert_eq!(got.is_ok(), c["ok"].as_bool().unwrap(), "{name}: {got:?}");
    }
}

#[test]
fn synthetic_typed_data_matches_viem() {
    let v = vectors();
    let signer = signer(&v);
    let s = &v["synthetic"];
    let td: GmxTypedData = serde_json::from_value(s["typedData"].clone()).unwrap();
    let h = hash_typed_data(&td).unwrap();
    assert_eq!(h.primary_type, s["primaryType"].as_str().unwrap());
    assert_eq!(
        hex32(&h.domain_separator),
        s["domainSeparator"].as_str().unwrap()
    );
    assert_eq!(hex32(&h.struct_hash), s["structHash"].as_str().unwrap());
    assert_eq!(hex32(&h.digest), s["digest"].as_str().unwrap());
    assert_eq!(
        signer.sign_typed_data(&td).unwrap(),
        s["signature"].as_str().unwrap()
    );
}

#[test]
fn rejects_malformed_values() {
    let v = vectors();
    let base: GmxTypedData = serde_json::from_value(v["synthetic"]["typedData"].clone()).unwrap();
    let cases: [(&str, Value); 6] = [
        ("count", Value::from(256)),
        ("small", Value::from(-129)),
        ("tag", Value::from("0x0102")),
        ("flags", serde_json::json!([true])),
        ("nonce", Value::from("1.5")),
        ("delta", Value::from(1.5)),
    ];
    for (field, bad) in cases {
        let mut td = base.clone();
        td.message[field] = bad;
        assert!(hash_typed_data(&td).is_err(), "{field}");
    }
    let mut td = base.clone();
    td.message["from"]["wallet"] = Value::from("0xcd2a3d9F938E13CD947Ec05AbC7FE734Df8DD826");
    assert!(hash_typed_data(&td).is_err(), "bad checksum");
    let mut td = base;
    td.message.as_object_mut().unwrap().remove("contents");
    assert!(hash_typed_data(&td).is_err(), "missing field");
}
