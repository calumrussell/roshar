//! Signing vectors for roshar-bullet, produced by Bullet's official Rust SDK
//! (`bullet-rust-sdk` 0.0.48, `bullet-exchange-interface` 0.16.3).
//!
//! Every case is described once as JSON (the shape `tests/vectors.rs` reads), converted to the
//! SDK's own types here, then built and signed through `bullet_rust_sdk::UnsignedTransaction` and
//! `bullet_rust_sdk::Transaction` exactly as an SDK user would. The key is a fresh random throwaway.
//! The SDK client needs `/rollup/schema` and `/fapi/v1/exchangeInfo`; an in-process wiremock
//! server answers them (the SDK's own tests do the same), so nothing leaves the machine.

use std::str::FromStr;

use bullet_exchange_interface::address::Address;
use bullet_exchange_interface::message::{
    AmendOrderArgs, CallMessage, CancelOrderArgs, NewOrderArgs, PendingTpslPair, Tpsl, TpslPair,
    UserAction,
};
use bullet_exchange_interface::schema::Schema;
use bullet_exchange_interface::time::UnixTimestampMicros;
use bullet_exchange_interface::transaction::{Gas, Transaction as WireTransaction, UniquenessData};
use bullet_exchange_interface::types::{
    ClientOrderId, MarketId, OrderId, OrderIdKind, OrderType, Side, TriggerPriceCondition,
};
use bullet_rust_sdk::{Client, Keypair, PositiveDecimal, Transaction, UnsignedTransaction};
use serde_json::{Value, json};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// `chainHash` of `GET /fapi/v1/exchangeInfo` on 2026-11-15.
const MAINNET_CHAIN_HASH: &str =
    "0xb08897f45bad670afdeb0f9a37f37f6d460903cc9a77a2cf86339416ac0b1b4d";
const TESTNET_CHAIN_HASH: &str =
    "0x2967a8bd109de24b8687dafa9b4694232bb19f0586f0246241808b9aca32d772";

async fn offline_client(chain_hash: &str) -> (MockServer, Client) {
    let server = MockServer::start().await;
    let schema = Schema::of_single_type::<WireTransaction>().unwrap();
    Mock::given(method("GET"))
        .and(path("/rollup/schema"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "chain_hash": chain_hash,
            "schema": schema,
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/fapi/v1/exchangeInfo"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "assets": [], "rateLimits": [], "symbols": [],
            "globalConfig": {
                "maxOrdersPerUser": 0, "maxTriggerOrdersPerUser": 0,
                "maxTriggerOrdersToExecutePerMsg": 0, "minNotionalTwapValue": "0",
                "minNotionalTwapValuePerOrder": "0", "twapExecutionIntervalSeconds": 0
            },
            "globalVaultConfig": {
                "leaderMinimumHoldingPercentage": 0, "creationFeeUsdc": "0", "minDepositValue": "0"
            }
        })))
        .mount(&server)
        .await;
    let client = Client::builder().network(server.uri()).build().await.unwrap();
    assert_eq!(hex::encode(client.chain_hash()), chain_hash.trim_start_matches("0x"));
    (server, client)
}

fn dec(v: &Value) -> PositiveDecimal {
    PositiveDecimal::from_str(v.as_str().unwrap()).unwrap()
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
        client_order_id: v["client_order_id"].as_u64().map(ClientOrderId),
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
        order_id: v["order_id"].as_u64().map(OrderId),
        client_order_id: v["client_order_id"].as_u64().map(ClientOrderId),
    }
}

fn market(v: &Value) -> MarketId {
    MarketId(u16::try_from(v["market_id"].as_u64().unwrap()).unwrap())
}

fn call_message(a: &Value) -> CallMessage<Address> {
    let sub = opt_u8(&a["sub_account_index"]);
    let list = |k: &str| a[k].as_array().unwrap().iter();
    let action = match a["type"].as_str().unwrap() {
        "place_orders" => UserAction::PlaceOrders {
            market_id: market(a),
            orders: list("orders").map(new_order).collect(),
            replace: a["replace"].as_bool().unwrap(),
            sub_account_index: sub,
        },
        "amend_orders" => UserAction::AmendOrders {
            market_id: market(a),
            orders: list("orders")
                .map(|o| AmendOrderArgs {
                    cancel: cancel(&o["cancel"]),
                    place: new_order(&o["place"]),
                })
                .collect(),
            sub_account_index: sub,
        },
        "cancel_orders" => UserAction::CancelOrders {
            market_id: market(a),
            orders: list("orders").map(cancel).collect(),
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
                .map(|c| match (c["server"].as_u64(), c["client"].as_u64()) {
                    (Some(id), None) => OrderIdKind::Server(OrderId(id)),
                    (None, Some(id)) => OrderIdKind::Client(ClientOrderId(id)),
                    _ => panic!("cancel kind {c}"),
                })
                .collect(),
            places: list("places").map(new_order).collect(),
            expiry_timestamp: a["expiry_timestamp"].as_i64().map(UnixTimestampMicros),
            sub_account_index: sub,
        },
        "update_max_leverage" => UserAction::UpdateMaxLeverage {
            market_id: market(a),
            max_leverage: u16::try_from(a["max_leverage"].as_u64().unwrap()).unwrap(),
            sub_account_index: sub,
        },
        t => panic!("action {t}"),
    };
    CallMessage::User(action)
}

fn uniqueness(v: &Value) -> UniquenessData {
    let n = v["value"].as_u64().unwrap();
    match v["kind"].as_str().unwrap() {
        "nonce" => UniquenessData::Nonce(n),
        "generation" => UniquenessData::Generation(n),
        "window" => UniquenessData::Window(n),
        k => panic!("uniqueness {k}"),
    }
}

fn order(price: &str, size: &str, side: &str, ty: &str) -> Value {
    json!({
        "price": price, "size": size, "side": side, "order_type": ty, "reduce_only": false,
        "client_order_id": null, "pending_tpsl_pair": null
    })
}

fn details(max_fee: u128, prio: u64, gas: Option<[u64; 2]>) -> Value {
    json!({ "max_priority_fee_bips": prio, "max_fee": max_fee.to_string(), "gas_limit": gas })
}

fn cases() -> Vec<Value> {
    let default_details = details(1 << 48, 0, None);
    let mut with_cid = order("50000.0", "0.001", "bid", "limit");
    with_cid["client_order_id"] = json!(1u64);
    let mut ro_ask = order("3125.75", "1.5", "ask", "post_only");
    ro_ask["reduce_only"] = json!(true);
    ro_ask["client_order_id"] = json!(u64::MAX);
    let mut tpsl_order = order("64921", "0.0123", "bid", "immediate_or_cancel");
    tpsl_order["client_order_id"] = json!(1_794_300_365_740_858_276u64);
    tpsl_order["pending_tpsl_pair"] = json!({
        "tp": { "order_price": "70000", "trigger_price": "69999.5", "price_condition": "mark", "order_type": "limit" },
        "sl": { "order_price": "60000.00", "trigger_price": "60100", "price_condition": "last_trade", "order_type": "immediate_or_cancel" },
        "dynamic_size": true
    });
    let mut sl_only = order("2.8217", "354", "ask", "post_only_slide");
    sl_only["pending_tpsl_pair"] = json!({
        "tp": null,
        "sl": { "order_price": "3.1", "trigger_price": "3.05", "price_condition": "oracle", "order_type": "fill_or_kill" },
        "dynamic_size": false
    });
    vec![
        json!({
            "name": "place_limit_bid_client_id",
            "network": "mainnet",
            "uniqueness": { "kind": "window", "value": 1_794_780_000_000u64 },
            "details": default_details,
            "address_override": null,
            "action": { "type": "place_orders", "market_id": 1, "orders": [with_cid], "replace": false, "sub_account_index": null }
        }),
        json!({
            "name": "replace_two_orders_sub_account_nonce_gas",
            "network": "mainnet",
            "uniqueness": { "kind": "nonce", "value": 7u64 },
            "details": details(10_000_000_000, 5, Some([5_000_000, 5_000_000])),
            "address_override": null,
            "action": { "type": "place_orders", "market_id": 2, "orders": [ro_ask, order("3000", "2", "bid", "post_only_front")], "replace": true, "sub_account_index": 3 }
        }),
        json!({
            "name": "place_ioc_with_tpsl_testnet_generation",
            "network": "testnet",
            "uniqueness": { "kind": "generation", "value": 42u64 },
            "details": default_details,
            "address_override": null,
            "action": { "type": "place_orders", "market_id": 1, "orders": [tpsl_order], "replace": false, "sub_account_index": null }
        }),
        json!({
            "name": "place_fok_and_slide_sl_only",
            "network": "mainnet",
            "uniqueness": { "kind": "window", "value": u64::MAX },
            "details": details(u128::MAX, u64::MAX, Some([0, u64::MAX])),
            "address_override": null,
            "action": { "type": "place_orders", "market_id": 8, "orders": [order("123456789012.345678901234567", "0.00000001", "ask", "fill_or_kill"), sl_only], "replace": false, "sub_account_index": 255 }
        }),
        json!({
            "name": "place_with_address_override",
            "network": "mainnet",
            "uniqueness": { "kind": "window", "value": 1_794_780_000_001u64 },
            "details": default_details,
            "address_override": "CuHSLVyC4U3QGKjU1vdzDi1MhQcZbyu1v4bCjcd2tNhc",
            "action": { "type": "place_orders", "market_id": 20003, "orders": [order("139.27", "7.17", "bid", "post_only")], "replace": false, "sub_account_index": null }
        }),
        json!({
            "name": "place_empty_order_list",
            "network": "mainnet",
            "uniqueness": { "kind": "window", "value": 0u64 },
            "details": details(0, 0, None),
            "address_override": null,
            "action": { "type": "place_orders", "market_id": 0, "orders": [], "replace": true, "sub_account_index": 0 }
        }),
        json!({
            "name": "cancel_by_order_id_client_id_and_both",
            "network": "mainnet",
            "uniqueness": { "kind": "window", "value": 1_794_780_000_002u64 },
            "details": default_details,
            "address_override": null,
            "action": { "type": "cancel_orders", "market_id": 1, "orders": [
                { "order_id": 12345u64, "client_order_id": null },
                { "order_id": null, "client_order_id": 1u64 },
                { "order_id": 7_250_149_508u64, "client_order_id": 1_794_300_365_740_858_276u64 }
            ], "sub_account_index": null }
        }),
        json!({
            "name": "cancel_market_orders",
            "network": "mainnet",
            "uniqueness": { "kind": "window", "value": 1_794_780_000_003u64 },
            "details": default_details,
            "address_override": null,
            "action": { "type": "cancel_market_orders", "market_id": 1, "sub_account_index": null }
        }),
        json!({
            "name": "cancel_all_orders",
            "network": "mainnet",
            "uniqueness": { "kind": "window", "value": 1_794_780_000_004u64 },
            "details": default_details,
            "address_override": null,
            "action": { "type": "cancel_all_orders", "sub_account_index": null }
        }),
        json!({
            "name": "cancel_all_orders_sub_account_testnet",
            "network": "testnet",
            "uniqueness": { "kind": "nonce", "value": 1u64 },
            "details": details(1 << 48, 0, Some([500_000, 500_000])),
            "address_override": null,
            "action": { "type": "cancel_all_orders", "sub_account_index": 1 }
        }),
        json!({
            "name": "amend_order",
            "network": "mainnet",
            "uniqueness": { "kind": "window", "value": 1_794_780_000_005u64 },
            "details": default_details,
            "address_override": null,
            "action": { "type": "amend_orders", "market_id": 0, "orders": [
                { "cancel": { "order_id": 12345u64, "client_order_id": null }, "place": with_cid_amend() },
                { "cancel": { "order_id": null, "client_order_id": 9u64 }, "place": order("71.8", "0.10", "ask", "post_only") }
            ], "sub_account_index": null }
        }),
        json!({
            "name": "cancel_and_place_with_expiry",
            "network": "mainnet",
            "uniqueness": { "kind": "window", "value": 1_794_780_000_006u64 },
            "details": default_details,
            "address_override": null,
            "action": { "type": "cancel_and_place_orders", "market_id": 4,
                "cancels": [ { "server": 7_250_124_078u64 }, { "client": 1_794_300_365_737_696_436u64 } ],
                "places": [ order("67.632", "5.72", "ask", "post_only"), order("67.5", "5.72", "bid", "post_only") ],
                "expiry_timestamp": 1_794_780_060_000_000i64, "sub_account_index": null }
        }),
        json!({
            "name": "cancel_and_place_cancel_only_no_expiry",
            "network": "testnet",
            "uniqueness": { "kind": "window", "value": 1_794_780_000_007u64 },
            "details": default_details,
            "address_override": null,
            "action": { "type": "cancel_and_place_orders", "market_id": 4,
                "cancels": [ { "server": 1u64 } ], "places": [],
                "expiry_timestamp": null, "sub_account_index": 2 }
        }),
        json!({
            "name": "update_max_leverage",
            "network": "mainnet",
            "uniqueness": { "kind": "window", "value": 1_794_780_000_008u64 },
            "details": default_details,
            "address_override": null,
            "action": { "type": "update_max_leverage", "market_id": 1, "max_leverage": 20, "sub_account_index": null }
        }),
    ]
}

fn with_cid_amend() -> Value {
    let mut o = order("50500.0", "0.0015", "bid", "limit");
    o["client_order_id"] = json!(3u64);
    o
}

#[tokio::main]
async fn main() {
    let secret: [u8; 32] = rand::random();
    let keypair = Keypair::from_bytes(secret);
    let (_m, mainnet) = offline_client(MAINNET_CHAIN_HASH).await;
    let (_t, testnet) = offline_client(TESTNET_CHAIN_HASH).await;

    let mut out = Vec::new();
    for mut case in cases() {
        let client = match case["network"].as_str().unwrap() {
            "mainnet" => &mainnet,
            "testnet" => &testnet,
            n => panic!("network {n}"),
        };
        let d = &case["details"];
        let max_fee: u128 = d["max_fee"].as_str().unwrap().parse().unwrap();
        let prio = d["max_priority_fee_bips"].as_u64().unwrap();
        let gas = d["gas_limit"]
            .as_array()
            .map(|g| Gas([g[0].as_u64().unwrap(), g[1].as_u64().unwrap()]));
        let address_override = case["address_override"]
            .as_str()
            .map(|a| Address::from_str(a).unwrap());
        let call = call_message(&case["action"]);
        let uniq = uniqueness(&case["uniqueness"]);

        let unsigned = UnsignedTransaction::builder()
            .call_message(call.clone())
            .max_fee(max_fee)
            .priority_fee_bips(prio)
            .maybe_gas_limit(gas.clone())
            .uniqueness(uniq.clone())
            .maybe_address_override(address_override)
            .client(client)
            .build()
            .unwrap();
        let signing_bytes = unsigned.to_bytes().unwrap();

        let signed = Transaction::builder()
            .call_message(call)
            .max_fee(max_fee)
            .priority_fee_bips(prio)
            .maybe_gas_limit(gas)
            .uniqueness(uniq)
            .maybe_address_override(address_override)
            .signer(&keypair)
            .client(client)
            .build()
            .unwrap();
        let signature = match &signed {
            WireTransaction::V0(v0) => v0.signature,
            _ => unreachable!("only V0 exists"),
        };
        assert_eq!(signature.to_vec(), keypair.sign(&signing_bytes));

        case["chain_hash"] = json!(hex::encode(client.chain_hash()));
        case["signing_bytes"] = json!(hex::encode(&signing_bytes));
        case["signature"] = json!(hex::encode(signature));
        case["tx_base64"] = json!(Transaction::to_base64(&signed).unwrap());
        out.push(case);
    }

    let doc = json!({
        "generator": "tests/gen_vectors (bullet-rust-sdk 0.0.48, bullet-exchange-interface 0.16.3)",
        "secret_key": hex::encode(secret),
        "public_key": keypair.public_key_hex(),
        "address": keypair.address(),
        "cases": out,
    });
    println!("{}", serde_json::to_string_pretty(&doc).unwrap());
}
