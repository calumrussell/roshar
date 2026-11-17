//! Parity with the official TypeScript SDK (`tests/gen_vectors.mts`, output `tests/vectors.json`).
//! Signatures are RFC 6979 deterministic on both sides, so every byte must match.

use roshar_polymarket_perps::signer::{
    CreateOrder, LeverageUpdate, ModifyOrder, ModifyOrderCoid, Op, Signer,
};
use roshar_polymarket_perps::types::TimeInForce;
use serde_json::Value;

fn vectors() -> Value {
    serde_json::from_str(include_str!("vectors.json")).expect("vectors.json parses")
}

fn hex0x(bytes: &[u8]) -> String {
    format!("0x{}", hex::encode(bytes))
}

fn u(v: &Value) -> u64 {
    v.as_u64()
        .unwrap_or_else(|| panic!("expected u64, got {v}"))
}

fn s(v: &Value) -> String {
    v.as_str()
        .unwrap_or_else(|| panic!("expected string, got {v}"))
        .to_string()
}

fn opt_u(v: &Value) -> Option<u64> {
    if v.is_null() {
        None
    } else {
        Some(u(v))
    }
}

fn tif(v: &Value) -> TimeInForce {
    match v.as_str().unwrap() {
        "gtc" => TimeInForce::Gtc,
        "gtd" => TimeInForce::Gtd,
        "ioc" => TimeInForce::Ioc,
        "fok" => TimeInForce::Fok,
        other => panic!("unknown tif {other}"),
    }
}

/// Rebuilds the op from the SDK request recorded in `input`.
fn op_from_input(name: &str, input: &Value) -> Op {
    let kind = name.split('/').next().unwrap();
    match kind {
        "create_orders" => Op::CreateOrders(
            input["orders"]
                .as_array()
                .unwrap()
                .iter()
                .map(|o| CreateOrder {
                    instrument_id: u(&o["instrumentId"]) as u32,
                    buy: match o["side"].as_str().unwrap() {
                        "BUY" => true,
                        "SELL" => false,
                        other => panic!("unknown side {other}"),
                    },
                    price: o.get("price").map(s),
                    quantity: s(&o["quantity"]),
                    tif: tif(&o["timeInForce"]),
                    post_only: o.get("postOnly").is_some_and(|v| v.as_bool().unwrap()),
                    reduce_only: o.get("reduceOnly").is_some_and(|v| v.as_bool().unwrap()),
                    client_order_id: o.get("clientOrderId").map(s),
                    gtd_expiry: o.get("gtdExpiry").map(u),
                })
                .collect(),
        ),
        "cancel_orders" => Op::CancelOrders(
            input["order_ids"]
                .as_array()
                .unwrap()
                .iter()
                .map(u)
                .collect(),
        ),
        "cancel_orders_coid" => Op::CancelOrdersCoid(
            input["client_order_ids"]
                .as_array()
                .unwrap()
                .iter()
                .map(s)
                .collect(),
        ),
        "cancel_all" => Op::CancelAll {
            instrument_id: opt_u(&input["instrument_id"]).map(|v| v as u32),
        },
        "auto_cancel" => Op::AutoCancel {
            time: u(&input["time"]),
        },
        "update_leverage" => Op::UpdateLeverage(LeverageUpdate {
            instrument_id: u(&input["instrument_id"]) as u32,
            leverage: u(&input["leverage"]) as u32,
            cross: input["cross"].as_bool().unwrap(),
        }),
        "update_leverages" => Op::UpdateLeverages(
            input["updates"]
                .as_array()
                .unwrap()
                .iter()
                .map(|t| LeverageUpdate {
                    instrument_id: u(&t[0]) as u32,
                    leverage: u(&t[1]) as u32,
                    cross: t[2].as_bool().unwrap(),
                })
                .collect(),
        ),
        "update_margin" => Op::UpdateMargin {
            instrument_id: u(&input["instrument_id"]) as u32,
            amount: s(&input["amount"]),
        },
        "modify_orders" => Op::ModifyOrders(
            input["items"]
                .as_array()
                .unwrap()
                .iter()
                .map(|t| ModifyOrder {
                    order_id: u(&t[0]),
                    price: s(&t[1]),
                    quantity: s(&t[2]),
                })
                .collect(),
        ),
        "modify_orders_coid" => Op::ModifyOrdersCoid(
            input["items"]
                .as_array()
                .unwrap()
                .iter()
                .map(|t| ModifyOrderCoid {
                    client_order_id: s(&t[0]),
                    price: s(&t[1]),
                    quantity: s(&t[2]),
                })
                .collect(),
        ),
        other => panic!("unhandled vector kind {other}"),
    }
}

fn exp_of(input: &Value) -> Option<u64> {
    input.get("expires_at").and_then(opt_u)
}

#[test]
fn addresses_match_sdk() {
    let v = vectors();
    let proxy = Signer::from_hex(v["proxy_private_key"].as_str().unwrap(), 137).unwrap();
    assert_eq!(proxy.address(), v["proxy_address"].as_str().unwrap());
    let owner = Signer::from_hex(v["owner_private_key"].as_str().unwrap(), 137).unwrap();
    assert_eq!(owner.address(), v["owner_address"].as_str().unwrap());
}

#[test]
fn commands_match_sdk() {
    let v = vectors();
    let key = v["proxy_private_key"].as_str().unwrap();
    let commands = v["commands"].as_array().unwrap();
    assert!(commands.len() >= 20);
    for c in commands {
        let name = c["name"].as_str().unwrap();
        let signer = Signer::from_hex(key, u(&c["chain_id"])).unwrap();
        let op = op_from_input(name, &c["input"]);
        let (salt, ts) = (u(&c["salt"]), u(&c["ts"]));
        let exp = exp_of(&c["input"]);
        assert_eq!(exp, opt_u(&c["exp"]), "{name}: exp");

        assert_eq!(
            hex0x(&op.compact_msgpack()),
            c["msgpack"].as_str().unwrap(),
            "{name}: msgpack"
        );
        assert_eq!(
            hex0x(&op.hash()),
            c["data"].as_str().unwrap(),
            "{name}: data"
        );
        assert_eq!(
            hex0x(&signer.op_digest(&op, salt, ts)),
            c["digest"].as_str().unwrap(),
            "{name}: digest"
        );
        if let Some(path) = c.get("path").and_then(Value::as_str) {
            assert_eq!(op.rest_route().1, path, "{name}: path");
        }

        let cmd = signer.sign_op(op, exp, salt, ts).unwrap();
        assert_eq!(cmd.signature(), c["sig"].as_str().unwrap(), "{name}: sig");
        assert_eq!(
            cmd.rest_body(),
            c["rest_body"].as_str().unwrap(),
            "{name}: rest body"
        );
        assert_eq!(
            cmd.ws_frame(u(&c["ws_id"])).unwrap(),
            c["ws_frame"].as_str().unwrap(),
            "{name}: ws frame"
        );
    }
}

#[test]
fn owner_ops_match_sdk() {
    let v = vectors();
    let owner = Signer::from_hex(v["owner_private_key"].as_str().unwrap(), 137).unwrap();
    for c in v["owner_ops"].as_array().unwrap() {
        let name = c["name"].as_str().unwrap();
        let (salt, ts) = (u(&c["salt"]), u(&c["ts"]));
        assert_eq!(c["path"].as_str().unwrap(), "/v1/account/proxy");
        if name.starts_with("create_proxy") {
            let proxy = c["proxy"].as_str().unwrap();
            let expiry = u(&c["expiry"]);
            assert_eq!(
                hex0x(&owner.create_proxy_digest(proxy, expiry, salt, ts).unwrap()),
                c["digest"].as_str().unwrap(),
                "{name}: digest"
            );
            let req = owner
                .sign_create_proxy(proxy, expiry, salt, ts, c["label"].as_str())
                .unwrap();
            assert_eq!(c["owner"].as_str().unwrap(), owner.address());
            assert_eq!(req.signature(), c["sig"].as_str().unwrap(), "{name}: sig");
            assert_eq!(
                req.rest_body(),
                c["rest_body"].as_str().unwrap(),
                "{name}: body"
            );
        } else {
            assert_eq!(name, "delete_proxy");
            let op = Op::DeleteProxy {
                proxy: s(&c["proxy"]),
            };
            assert_eq!(hex0x(&op.compact_msgpack()), c["msgpack"].as_str().unwrap());
            assert_eq!(hex0x(&op.hash()), c["data"].as_str().unwrap());
            assert_eq!(
                hex0x(&owner.op_digest(&op, salt, ts)),
                c["digest"].as_str().unwrap()
            );
            let cmd = owner.sign_op(op, None, salt, ts).unwrap();
            assert_eq!(cmd.signature(), c["sig"].as_str().unwrap());
            assert_eq!(cmd.rest_body(), c["rest_body"].as_str().unwrap());
            assert!(cmd.ws_frame(1).is_err());
        }
    }
}

#[test]
fn rejects_what_the_sdk_rejects() {
    let signer = Signer::from_hex(&format!("{:064x}", 1), 137).unwrap();
    let order = CreateOrder {
        instrument_id: 1,
        buy: true,
        price: Some("1".into()),
        quantity: "1".into(),
        tif: TimeInForce::Gtc,
        post_only: false,
        reduce_only: false,
        client_order_id: None,
        gtd_expiry: None,
    };
    let bad = [
        Op::CreateOrders(vec![]),
        Op::CreateOrders(vec![CreateOrder {
            price: None,
            ..order.clone()
        }]),
        Op::CreateOrders(vec![CreateOrder {
            tif: TimeInForce::Ioc,
            post_only: true,
            ..order.clone()
        }]),
        Op::CreateOrders(vec![CreateOrder {
            tif: TimeInForce::Gtd,
            ..order.clone()
        }]),
        Op::CreateOrders(vec![CreateOrder {
            gtd_expiry: Some(1),
            ..order.clone()
        }]),
        Op::CreateOrders(vec![CreateOrder {
            client_order_id: Some("ABC".into()),
            ..order.clone()
        }]),
        Op::CancelOrders(vec![1 << 53]),
        Op::CancelOrdersCoid(vec!["7F9E4A2B6C8D0E1F1234567890ABCDEF".into()]),
        Op::UpdateLeverages(vec![
            LeverageUpdate {
                instrument_id: 1,
                leverage: 2,
                cross: true,
            },
            LeverageUpdate {
                instrument_id: 1,
                leverage: 3,
                cross: true,
            },
        ]),
        Op::DeleteProxy {
            proxy: "0x1234".into(),
        },
    ];
    for op in bad {
        assert!(
            signer.sign_op(op.clone(), None, 1, 1).is_err(),
            "{op:?} accepted"
        );
    }
    // `exp` only where the API documents it.
    assert!(signer
        .sign_op(Op::AutoCancel { time: 0 }, Some(1), 1, 1)
        .is_err());
    assert!(signer
        .sign_op(Op::CreateOrders(vec![order]), Some(1), 1, 1)
        .is_ok());
}
