//! Parity with Bullet's official Rust SDK.
//!
//! `vectors.json` was produced by `tests/gen_vectors` (bullet-rust-sdk 0.0.48,
//! bullet-exchange-interface 0.16.3) with a throwaway random key and no network. ed25519 is
//! deterministic, so signing bytes, signatures and wire transactions must match byte for byte.
//! Never regenerate the vectors to make a change pass.

use std::str::FromStr;

use roshar_bullet::signer::{
    decode_address, parse_chain_hash, verify, AmendOrderArgs, CancelOrderArgs, NewOrderArgs,
    OrderIdKind, OrderType, PendingTpslPair, Side, Signer, Tpsl, TpslPair, TriggerPriceCondition,
    TxDetails, Uniqueness, UnsignedTransaction, UserAction,
};
use roshar_bullet::ws_tx::{tx_request, BulletTxMethod};
use rust_decimal::Decimal;
use serde_json::Value;

const VECTORS: &str = include_str!("vectors.json");

fn dec(v: &Value) -> Decimal {
    Decimal::from_str(v.as_str().unwrap()).unwrap()
}

fn opt_u8(v: &Value) -> Option<u8> {
    v.as_u64().map(|x| u8::try_from(x).unwrap())
}

fn side(v: &Value) -> Side {
    match v.as_str().unwrap() {
        "bid" => Side::Bid,
        "ask" => Side::Ask,
        s => panic!("side {s}"),
    }
}

fn order_type(v: &Value) -> OrderType {
    match v.as_str().unwrap() {
        "limit" => OrderType::Limit,
        "post_only" => OrderType::PostOnly,
        "fill_or_kill" => OrderType::FillOrKill,
        "immediate_or_cancel" => OrderType::ImmediateOrCancel,
        "post_only_slide" => OrderType::PostOnlySlide,
        "post_only_front" => OrderType::PostOnlyFront,
        s => panic!("order type {s}"),
    }
}

fn tpsl(v: &Value) -> Option<Tpsl> {
    if v.is_null() {
        return None;
    }
    Some(Tpsl {
        order_price: dec(&v["order_price"]),
        trigger_price: dec(&v["trigger_price"]),
        price_condition: match v["price_condition"].as_str().unwrap() {
            "mark" => TriggerPriceCondition::Mark,
            "oracle" => TriggerPriceCondition::Oracle,
            "last_trade" => TriggerPriceCondition::LastTrade,
            s => panic!("price condition {s}"),
        },
        order_type: order_type(&v["order_type"]),
    })
}

fn new_order(v: &Value) -> NewOrderArgs {
    let p = &v["pending_tpsl_pair"];
    NewOrderArgs {
        price: dec(&v["price"]),
        size: dec(&v["size"]),
        side: side(&v["side"]),
        order_type: order_type(&v["order_type"]),
        reduce_only: v["reduce_only"].as_bool().unwrap(),
        client_order_id: v["client_order_id"].as_u64(),
        pending_tpsl_pair: (!p.is_null()).then(|| PendingTpslPair {
            tpsl_pair: TpslPair {
                tp: tpsl(&p["tp"]),
                sl: tpsl(&p["sl"]),
            },
            dynamic_size: p["dynamic_size"].as_bool().unwrap(),
        }),
    }
}

fn cancel(v: &Value) -> CancelOrderArgs {
    CancelOrderArgs {
        order_id: v["order_id"].as_u64(),
        client_order_id: v["client_order_id"].as_u64(),
    }
}

fn market(v: &Value) -> u16 {
    u16::try_from(v["market_id"].as_u64().unwrap()).unwrap()
}

fn action(a: &Value) -> UserAction {
    let sub = opt_u8(&a["sub_account_index"]);
    let list = |k: &str| a[k].as_array().unwrap().clone();
    match a["type"].as_str().unwrap() {
        "place_orders" => UserAction::PlaceOrders {
            market_id: market(a),
            orders: list("orders").iter().map(new_order).collect(),
            replace: a["replace"].as_bool().unwrap(),
            sub_account_index: sub,
        },
        "amend_orders" => UserAction::AmendOrders {
            market_id: market(a),
            orders: list("orders")
                .iter()
                .map(|o| AmendOrderArgs {
                    cancel: cancel(&o["cancel"]),
                    place: new_order(&o["place"]),
                })
                .collect(),
            sub_account_index: sub,
        },
        "cancel_orders" => UserAction::CancelOrders {
            market_id: market(a),
            orders: list("orders").iter().map(cancel).collect(),
            sub_account_index: sub,
        },
        "cancel_market_orders" => UserAction::CancelMarketOrders {
            market_id: market(a),
            sub_account_index: sub,
        },
        "cancel_all_orders" => UserAction::CancelAllOrders {
            sub_account_index: sub,
        },
        "cancel_and_place_orders" => UserAction::CancelAndPlaceOrders {
            market_id: market(a),
            cancels: list("cancels")
                .iter()
                .map(|c| match (c["server"].as_u64(), c["client"].as_u64()) {
                    (Some(id), None) => OrderIdKind::Server(id),
                    (None, Some(id)) => OrderIdKind::Client(id),
                    _ => panic!("cancel kind {c}"),
                })
                .collect(),
            places: list("places").iter().map(new_order).collect(),
            expiry_timestamp: a["expiry_timestamp"].as_i64(),
            sub_account_index: sub,
        },
        "update_max_leverage" => UserAction::UpdateMaxLeverage {
            market_id: market(a),
            max_leverage: u16::try_from(a["max_leverage"].as_u64().unwrap()).unwrap(),
            sub_account_index: sub,
        },
        t => panic!("action {t}"),
    }
}

fn unsigned(case: &Value) -> UnsignedTransaction {
    let u = &case["uniqueness"];
    let n = u["value"].as_u64().unwrap();
    let uniqueness = match u["kind"].as_str().unwrap() {
        "nonce" => Uniqueness::Nonce(n),
        "generation" => Uniqueness::Generation(n),
        "window" => Uniqueness::Window(n),
        k => panic!("uniqueness {k}"),
    };
    let d = &case["details"];
    UnsignedTransaction {
        action: action(&case["action"]),
        uniqueness,
        details: TxDetails {
            max_priority_fee_bips: d["max_priority_fee_bips"].as_u64().unwrap(),
            max_fee: d["max_fee"].as_str().unwrap().parse().unwrap(),
            gas_limit: d["gas_limit"]
                .as_array()
                .map(|g| [g[0].as_u64().unwrap(), g[1].as_u64().unwrap()]),
        },
        address_override: case["address_override"]
            .as_str()
            .map(|a| decode_address(a).unwrap()),
    }
}

#[test]
fn key_matches_sdk() {
    let doc: Value = serde_json::from_str(VECTORS).unwrap();
    let signer = Signer::from_hex(doc["secret_key"].as_str().unwrap()).unwrap();
    assert_eq!(
        hex::encode(signer.public_key()),
        doc["public_key"].as_str().unwrap()
    );
    assert_eq!(signer.address(), doc["address"].as_str().unwrap());

    let mut keypair = hex::decode(doc["secret_key"].as_str().unwrap()).unwrap();
    keypair.extend(signer.public_key());
    let from_keypair = Signer::from_keypair_bytes(&keypair).unwrap();
    assert_eq!(from_keypair.public_key(), signer.public_key());
    keypair[63] ^= 1;
    assert!(Signer::from_keypair_bytes(&keypair).is_err());
}

#[test]
fn transactions_match_sdk() {
    let doc: Value = serde_json::from_str(VECTORS).unwrap();
    let signer = Signer::from_hex(doc["secret_key"].as_str().unwrap()).unwrap();
    let cases = doc["cases"].as_array().unwrap();
    assert!(!cases.is_empty());
    for case in cases {
        let name = case["name"].as_str().unwrap();
        let chain_hash = parse_chain_hash(case["chain_hash"].as_str().unwrap()).unwrap();
        let tx = unsigned(case);

        let signing_bytes = tx.signing_bytes(&chain_hash).unwrap();
        assert_eq!(
            hex::encode(&signing_bytes),
            case["signing_bytes"].as_str().unwrap(),
            "{name}: signing bytes"
        );

        let signed = signer.sign(&tx, &chain_hash).unwrap();
        assert_eq!(
            hex::encode(signed.signature),
            case["signature"].as_str().unwrap(),
            "{name}: signature"
        );
        assert_eq!(
            signed.to_base64(),
            case["tx_base64"].as_str().unwrap(),
            "{name}: wire transaction"
        );
        verify(&tx, &chain_hash, &signed.pub_key, &signed.signature).unwrap();

        let ws: Value = serde_json::from_str(&tx_request(
            Some(7),
            BulletTxMethod::for_action(&tx.action),
            &signed,
        ))
        .unwrap();
        assert_eq!(ws["params"]["tx"], case["tx_base64"], "{name}: ws request");
        assert_eq!(ws["id"], 7);
    }
}

#[test]
fn negative_decimal_is_rejected() {
    let tx = UnsignedTransaction {
        action: UserAction::PlaceOrders {
            market_id: 1,
            orders: vec![NewOrderArgs {
                price: Decimal::from_str("-1").unwrap(),
                size: Decimal::ONE,
                side: Side::Bid,
                order_type: OrderType::Limit,
                reduce_only: false,
                client_order_id: None,
                pending_tpsl_pair: None,
            }],
            replace: false,
            sub_account_index: None,
        },
        uniqueness: Uniqueness::Window(1),
        details: TxDetails {
            max_priority_fee_bips: 0,
            max_fee: 1,
            gas_limit: None,
        },
        address_override: None,
    };
    assert!(tx.signing_bytes(&[0u8; 32]).is_err());
}
