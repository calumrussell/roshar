//! Golden vectors for the signing path and response decoders.
//!
//! Recorded from saoirse's hl_exchange (the live Vin code) before it moved here;
//! every case is recomputed and compared byte for byte. Never regenerate it to make a change pass.
use roshar_hyperliquid::{
    decode_status, float_to_wire, to_tick, Action, OrderRef, OrderReq, Signer, Tif,
};
use serde_json::{json, Value};

const KEY: &str = "0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const VAULT: &str = "0x1111111111111111111111111111111111111111";

fn cloid(n: u8) -> [u8; 16] {
    let mut c = [0u8; 16];
    c[15] = n;
    c[0] = 0x80;
    c
}

fn order(
    asset: u32,
    is_buy: bool,
    px: f64,
    sz: f64,
    tif: Tif,
    reduce_only: bool,
    cl: Option<u8>,
) -> OrderReq {
    OrderReq {
        asset,
        is_buy,
        px,
        sz,
        reduce_only,
        tif,
        cloid: cl.map(cloid),
    }
}

fn actions() -> Vec<(&'static str, Action)> {
    vec![
        (
            "order_alo_na",
            Action::Order {
                orders: vec![order(0, true, 0.004361, 4600.0, Tif::Alo, false, None)],
                priority: None,
            },
        ),
        (
            "order_multi_priority",
            Action::Order {
                orders: vec![
                    order(3, false, 187.45, 0.12, Tif::Ioc, true, Some(7)),
                    order(10000, true, 1.2345, 10.0, Tif::Gtc, false, Some(9)),
                ],
                priority: Some(10_000),
            },
        ),
        (
            "batch_modify",
            Action::BatchModify {
                modifies: vec![
                    (
                        OrderRef::Oid(77738308),
                        order(0, true, 0.00436, 4600.0, Tif::Alo, false, None),
                    ),
                    (
                        OrderRef::Cloid(cloid(3)),
                        order(0, false, 0.004365, 4600.0, Tif::Alo, true, Some(3)),
                    ),
                ],
            },
        ),
        (
            "modify_always_place",
            Action::Modify {
                oid: OrderRef::Oid(5),
                order: order(1, true, 3000.5, 0.01, Tif::Gtc, false, None),
                always_place: true,
            },
        ),
        (
            "modify_cloid",
            Action::Modify {
                oid: OrderRef::Cloid(cloid(4)),
                order: order(1, false, 3001.0, 0.01, Tif::Alo, false, Some(4)),
                always_place: false,
            },
        ),
        (
            "cancel",
            Action::Cancel {
                cancels: vec![(200, 7), (0, 77738308)],
                fast: false,
            },
        ),
        (
            "cancel_fast",
            Action::Cancel {
                cancels: vec![(200, 7)],
                fast: true,
            },
        ),
        (
            "cancel_by_cloid",
            Action::CancelByCloid {
                cancels: vec![(0, cloid(1))],
                fast: false,
            },
        ),
        (
            "cancel_by_cloid_fast",
            Action::CancelByCloid {
                cancels: vec![(0, cloid(2)), (5, cloid(3))],
                fast: true,
            },
        ),
    ]
}

fn compute() -> Value {
    let mut signing = serde_json::Map::new();
    for (vault, mainnet) in [
        (None, true),
        (Some(VAULT), true),
        (None, false),
        (Some(VAULT), false),
    ] {
        let s = Signer::from_hex(KEY, vault, mainnet).unwrap();
        for (name, a) in actions() {
            let nonce = 1_700_000_000_123;
            let key = format!(
                "{name}/{}/{}",
                if vault.is_some() { "vault" } else { "novault" },
                if mainnet { "mainnet" } else { "testnet" }
            );
            signing.insert(
                key,
                json!({
                    "action": Signer::action_json(&a),
                    "connection_id": hex::encode(s.connection_id(&a, nonce).unwrap()),
                    "signed": s.sign(&a, nonce).unwrap(),
                }),
            );
        }
    }
    let floats: Vec<Value> = [
        0.0,
        1.0,
        0.1,
        0.004361,
        187.45,
        100000.0,
        123456.7,
        1e-8,
        0.30000000000000004,
        4600.0,
        2.5e-5,
    ]
    .iter()
    .map(|x| json!([x, float_to_wire(*x)]))
    .collect();
    let ticks: Vec<Value> = [
        (0.0043605, 0u32),
        (187.456789, 2),
        (3000.55, 4),
        (101234.6, 5),
        (1.23456, 1),
        (0.000123456, 0),
    ]
    .iter()
    .flat_map(|(px, d)| [false, true].map(|up| json!([px, d, up, to_tick(*px, *d, up)])))
    .collect();
    let payloads = [
        json!({"status":"ok","response":{"type":"order","data":{"statuses":[{"resting":{"oid":77738308}}]}}}),
        json!({"status":"ok","response":{"type":"order","data":{"statuses":[{"filled":{"totalSz":"0.02","avgPx":"1891.4","oid":77747314}}]}}}),
        json!({"status":"ok","response":{"type":"order","data":{"statuses":[{"error":"Order could not immediately match against any resting orders."}]}}}),
        json!({"status":"ok","response":{"type":"cancel","data":{"statuses":["success"]}}}),
        json!({"status":"ok","response":{"type":"default"}}),
        json!({"status":"err","response":"User or API Wallet 0xabc does not exist."}),
        json!({"foo": 1}),
    ];
    let decoded: Vec<Value> = payloads
        .iter()
        .map(|p| json!([p, format!("{:?}", decode_status(p))]))
        .collect();
    json!({"signing": signing, "float_to_wire": floats, "to_tick": ticks, "decode_status": decoded})
}

#[test]
fn golden() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden.json");
    let got = compute();
    let want: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    assert_eq!(got, want);
}
