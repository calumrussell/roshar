//! Signing parity with the official Python SDK (`nado-protocol` 2.0.0). `vectors.json` was
//! produced by `gen_vectors.py` with a throwaway random key and no network. ECDSA signatures are
//! RFC 6979 (deterministic) on both sides, so payloads, digests and signatures must match exactly.

use roshar_nado::signer::*;
use serde_json::Value;

const VECTORS: &str = include_str!("vectors.json");

fn vectors() -> Value {
    serde_json::from_str(VECTORS).expect("vectors.json parses")
}

fn s(v: &Value) -> &str {
    v.as_str()
        .unwrap_or_else(|| panic!("expected string, got {v}"))
}

fn u64_of(v: &Value) -> u64 {
    s(v).parse().unwrap()
}

fn signer_for(v: &Value, case: &Value) -> Signer {
    let net = &v["networks"][s(&case["network"])];
    Signer::from_hex(
        s(&v["private_key"]),
        net["chain_id"].as_u64().unwrap(),
        s(&net["endpoint_addr"]),
    )
    .unwrap()
}

fn place_order_of(c: &Value) -> PlaceOrder {
    PlaceOrder {
        product_id: c["product_id"].as_u64().unwrap() as u32,
        order: Order {
            sender: Subaccount::from_hex(s(&c["sender"])).unwrap(),
            price_x18: s(&c["price_x18"]).parse().unwrap(),
            amount: s(&c["amount"]).parse().unwrap(),
            expiration: u64_of(&c["expiration"]),
            nonce: u64_of(&c["nonce"]),
            appendix: s(&c["appendix"]).parse().unwrap(),
        },
        id: c["id"].as_u64(),
        spot_leverage: c.get("spot_leverage").and_then(Value::as_bool),
    }
}

fn cancel_of(c: &Value) -> CancelOrders {
    CancelOrders {
        sender: Subaccount::from_hex(s(&c["sender"])).unwrap(),
        product_ids: c["product_ids"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p.as_u64().unwrap() as u32)
            .collect(),
        digests: c["digests"]
            .as_array()
            .unwrap()
            .iter()
            .map(|d| parse_digest(s(d)).unwrap())
            .collect(),
        nonce: u64_of(&c["nonce"]),
        required_unfilled_amount: None,
        id: None,
    }
}

fn digest_hex(d: [u8; 32]) -> String {
    format!("0x{}", hex::encode(d))
}

#[test]
fn address() {
    let v = vectors();
    for net in ["mainnet", "testnet"] {
        let case = serde_json::json!({ "network": net });
        assert_eq!(signer_for(&v, &case).address(), s(&v["address"]));
    }
}

#[test]
fn subaccounts() {
    let v = vectors();
    for c in v["subaccounts"].as_array().unwrap() {
        let sub = Subaccount::new(s(&c["address"]), s(&c["name"])).unwrap();
        assert_eq!(sub.to_hex(), s(&c["bytes32"]));
        assert_eq!(sub.address_hex(), s(&c["address"]));
    }
    assert!(Subaccount::new(s(&v["address"]), "thirteen_char").is_err());
}

#[test]
fn appendix() {
    let v = vectors();
    for c in v["appendix"].as_array().unwrap() {
        let order_type = match c["order_type"].as_u64().unwrap() {
            0 => OrderType::Default,
            1 => OrderType::Ioc,
            2 => OrderType::Fok,
            _ => OrderType::PostOnly,
        };
        let trigger = match c["trigger_type"].as_u64().unwrap() {
            0 => TriggerType::None,
            1 => TriggerType::Price,
            2 => TriggerType::Twap,
            _ => TriggerType::TwapCustomAmounts,
        };
        let a = Appendix {
            order_type,
            isolated: c["isolated"].as_bool().unwrap(),
            reduce_only: c["reduce_only"].as_bool().unwrap(),
            trigger,
            builder_id: c["builder_id"].as_u64().unwrap_or(0) as u16,
            builder_fee_rate: c["builder_fee_rate"].as_u64().unwrap_or(0) as u16,
            value: c["isolated_margin"].as_u64().unwrap_or(0),
        };
        let expected: u128 = s(&c["appendix"]).parse().unwrap();
        assert_eq!(a.encode().unwrap(), expected, "{c}");
        assert_eq!(Appendix::decode(expected).unwrap(), a, "{c}");
    }
}

#[test]
fn appendix_rejects() {
    let mut a = Appendix::new(OrderType::Default);
    a.value = 1;
    assert!(a.encode().is_err());
    let mut a = Appendix::new(OrderType::Default);
    a.isolated = true;
    a.trigger = TriggerType::Twap;
    assert!(a.encode().is_err());
    let mut a = Appendix::new(OrderType::Default);
    a.builder_fee_rate = 1024;
    assert!(a.encode().is_err());
}

#[test]
fn place_order() {
    let v = vectors();
    for c in v["place_order"].as_array().unwrap() {
        let signer = signer_for(&v, c);
        let p = place_order_of(c);
        assert_eq!(
            digest_hex(signer.order_digest(p.product_id, &p.order)),
            s(&c["digest"])
        );
        let signed = signer.sign_place_order(&p).unwrap();
        assert_eq!(signed.payload, c["request"], "{c}");
        assert_eq!(signed.digests, vec![s(&c["digest"]).to_string()]);
    }
}

#[test]
fn place_orders() {
    let v = vectors();
    for c in v["place_orders"].as_array().unwrap() {
        let signer = signer_for(&v, c);
        let orders: Vec<PlaceOrder> = c["orders"]
            .as_array()
            .unwrap()
            .iter()
            .map(place_order_of)
            .collect();
        let signed = signer
            .sign_place_orders(&orders, c["stop_on_failure"].as_bool())
            .unwrap();
        assert_eq!(signed.payload, c["request"]);
        let digests: Vec<String> = c["orders"]
            .as_array()
            .unwrap()
            .iter()
            .map(|o| s(&o["digest"]).to_string())
            .collect();
        assert_eq!(signed.digests, digests);
    }
}

#[test]
fn cancel_orders() {
    let v = vectors();
    for c in v["cancel_orders"].as_array().unwrap() {
        let signer = signer_for(&v, c);
        let cancel = cancel_of(c);
        assert_eq!(
            digest_hex(signer.cancellation_digest(&cancel)),
            s(&c["digest"])
        );
        let signed = signer.sign_cancel_orders(&cancel).unwrap();
        assert_eq!(signed.payload, c["request"], "{c}");
    }
}

#[test]
fn cancel_orders_length_mismatch() {
    let v = vectors();
    let c = &v["cancel_orders"][0];
    let mut cancel = cancel_of(c);
    cancel.product_ids.pop();
    assert!(signer_for(&v, c).sign_cancel_orders(&cancel).is_err());
}

#[test]
fn cancel_product_orders() {
    let v = vectors();
    for c in v["cancel_product_orders"].as_array().unwrap() {
        let signer = signer_for(&v, c);
        let cancel = CancelProductOrders {
            sender: Subaccount::from_hex(s(&c["sender"])).unwrap(),
            product_ids: c["product_ids"]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| p.as_u64().unwrap() as u32)
                .collect(),
            nonce: u64_of(&c["nonce"]),
            id: None,
        };
        assert_eq!(
            digest_hex(signer.cancellation_products_digest(&cancel)),
            s(&c["digest"])
        );
        let signed = signer.sign_cancel_product_orders(&cancel).unwrap();
        assert_eq!(signed.payload, c["request"], "{c}");
    }
}

#[test]
fn cancel_and_place() {
    let v = vectors();
    for c in v["cancel_and_place"].as_array().unwrap() {
        let signer = signer_for(&v, c);
        let cancel = cancel_of(&c["cancel"]);
        let place = place_order_of(&c["place"]);
        let signed = signer.sign_cancel_and_place(&cancel, &place, None).unwrap();
        assert_eq!(signed.payload, c["request"]);
        assert_eq!(
            signed.digests,
            vec![
                s(&c["cancel"]["digest"]).to_string(),
                s(&c["place"]["digest"]).to_string()
            ]
        );
    }
}

#[test]
fn link_signer() {
    let v = vectors();
    for c in v["link_signer"].as_array().unwrap() {
        let signer = signer_for(&v, c);
        let l = LinkSigner {
            sender: Subaccount::from_hex(s(&c["sender"])).unwrap(),
            signer: Subaccount::from_hex(s(&c["signer"])).unwrap(),
            nonce: u64_of(&c["nonce"]),
        };
        assert_eq!(digest_hex(signer.link_signer_digest(&l)), s(&c["digest"]));
        let signed = signer.sign_link_signer(&l).unwrap();
        assert_eq!(signed.payload, c["request"], "{c}");
    }
}

#[test]
fn stream_auth() {
    let v = vectors();
    for c in v["stream_auth"].as_array().unwrap() {
        let signer = signer_for(&v, c);
        let sender = Subaccount::from_hex(s(&c["sender"])).unwrap();
        let expiration = u64_of(&c["expiration"]);
        assert_eq!(
            digest_hex(signer.stream_auth_digest(&sender, expiration)),
            s(&c["digest"])
        );
        let msg = signer
            .stream_auth_message(&sender, expiration, c["id"].as_u64().unwrap())
            .unwrap();
        let msg: Value = serde_json::from_str(&msg).unwrap();
        assert_eq!(msg, c["message"], "{c}");
    }
}

#[test]
fn nonce_and_verifying_contract() {
    assert_eq!(
        order_nonce(1794780000000, 17).unwrap(),
        (1794780000000u64 << 20) + 17
    );
    assert!(order_nonce(1 << 44, 0).is_err());
    assert!(order_nonce(0, 1 << 20).is_err());
    let mut expected = [0u8; 20];
    expected[19] = 18;
    assert_eq!(order_verifying_contract(18), expected);
}
