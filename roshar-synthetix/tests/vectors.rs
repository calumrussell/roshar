//! Parity with the official Python SDK (`synthetix-sdk` 0.7.0). `tests/vectors.json` was produced
//! by `tests/gen_vectors.py` with a throwaway key. ECDSA here and in `eth_account` is RFC 6979
//! deterministic with low-s, so digests, signatures, REST bodies and websocket frames must match
//! byte for byte.

use roshar_synthetix::signer::{
    Grouping, Order, OrderChange, OrderType, ReqOpts, Side, SignedRequest, Signer,
    TriggerPriceType, Twap,
};
use roshar_synthetix::types::trading::{
    GetDelegatedSigners, GetFundingPayments, GetOpenOrders, GetOrderHistory, GetPositions,
    GetRateLimits, GetSubAccount, GetTrades,
};
use serde_json::Value;

fn load() -> Value {
    serde_json::from_str(include_str!("vectors.json")).expect("vectors.json")
}

fn hex32(b: &[u8; 32]) -> String {
    format!("0x{}", hex_encode(b))
}

fn hex_encode(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn s(v: &Value) -> &str {
    v.as_str().expect("string")
}

fn opt_u32(v: &Value) -> Option<u32> {
    v.as_u64().map(|x| u32::try_from(x).expect("u32"))
}

fn side(v: &Value) -> Side {
    match s(v) {
        "buy" => Side::Buy,
        "sell" => Side::Sell,
        other => panic!("side {other}"),
    }
}

fn order_type(v: &Value) -> OrderType {
    match s(v) {
        "limitGtc" => OrderType::LimitGtc,
        "limitIoc" => OrderType::LimitIoc,
        "limitAlo" => OrderType::LimitAlo,
        "limitGtd" => OrderType::LimitGtd,
        "market" => OrderType::Market,
        "triggerSl" => OrderType::TriggerSl,
        "triggerTp" => OrderType::TriggerTp,
        "twap" => OrderType::Twap,
        other => panic!("orderType {other}"),
    }
}

fn grouping(v: &Value) -> Grouping {
    match s(v) {
        "na" => Grouping::Na,
        "normalTpsl" => Grouping::NormalTpsl,
        "positionTpsl" => Grouping::PositionTpsl,
        "twap" => Grouping::Twap,
        other => panic!("grouping {other}"),
    }
}

fn order(o: &Value) -> Order<'_> {
    let twap = o["durationSeconds"].as_u64().map(|d| Twap {
        duration_seconds: d,
        interval_seconds: o["intervalSeconds"].as_u64(),
        interval_variance_bps: o["intervalVarianceBps"].as_u64(),
        size_variance_bps: o["sizeVarianceBps"].as_u64(),
    });
    Order {
        symbol: s(&o["symbol"]),
        side: side(&o["side"]),
        order_type: order_type(&o["orderType"]),
        price: s(&o["price"]),
        trigger_price: s(&o["triggerPrice"]),
        quantity: s(&o["quantity"]),
        reduce_only: o["reduceOnly"].as_bool().unwrap(),
        is_trigger_market: o["isTriggerMarket"].as_bool().unwrap(),
        client_order_id: s(&o["clientOrderId"]),
        close_position: o["closePosition"].as_bool().unwrap(),
        post_only: o["postOnly"].as_bool().unwrap_or(false),
        expires_at: o["expiresAt"].as_u64(),
        trigger_price_type: None,
        twap,
    }
}

fn strs(v: &Value) -> Vec<&str> {
    v.as_array().unwrap().iter().map(s).collect()
}

fn sign_query(signer: &Signer, sub: u64, inputs: &Value, opts: ReqOpts) -> SignedRequest {
    let p = &inputs["params"];
    match s(&inputs["action"]) {
        "getOpenOrders" => signer.sign_query(
            sub,
            &GetOpenOrders {
                symbol: p["symbol"].as_str(),
                client_order_id: p["clientOrderId"].as_str(),
                limit: opt_u32(&p["limit"]),
                offset: opt_u32(&p["offset"]),
            },
            opts,
        ),
        "getPositions" => signer.sign_query(
            sub,
            &GetPositions {
                status: p["status"].as_str(),
                symbol: p["symbol"].as_str(),
                limit: opt_u32(&p["limit"]),
                offset: opt_u32(&p["offset"]),
                ..Default::default()
            },
            opts,
        ),
        "getSubAccount" => signer.sign_query(sub, &GetSubAccount {}, opts),
        "getTrades" => signer.sign_query(
            sub,
            &GetTrades {
                symbol: p["symbol"].as_str(),
                order_id: p["orderId"].as_str(),
                limit: opt_u32(&p["limit"]),
                ..Default::default()
            },
            opts,
        ),
        "getOrderHistory" => {
            let status = strs(&p["status"]);
            signer.sign_query(
                sub,
                &GetOrderHistory {
                    status: Some(status.as_slice()),
                    start_time: p["startTime"].as_u64(),
                    end_time: p["endTime"].as_u64(),
                    limit: opt_u32(&p["limit"]),
                    ..Default::default()
                },
                opts,
            )
        }
        "getFundingPayments" => signer.sign_query(
            sub,
            &GetFundingPayments {
                symbol: p["symbol"].as_str(),
                start_time: p["startTime"].as_u64(),
                limit: opt_u32(&p["limit"]),
                ..Default::default()
            },
            opts,
        ),
        "getRateLimits" => signer.sign_query(sub, &GetRateLimits::default(), opts),
        "getDelegatedSigners" => signer.sign_query(sub, &GetDelegatedSigners {}, opts),
        other => panic!("query {other}"),
    }
    .unwrap()
}

#[test]
fn signed_requests_match_sdk() {
    let d = load();
    let signer = Signer::from_hex(s(&d["private_key"])).unwrap();
    assert_eq!(signer.address(), s(&d["address"]));

    let vectors = d["vectors"].as_array().unwrap();
    assert!(vectors.len() >= 30);
    for v in vectors {
        let name = s(&v["name"]);
        let sub = v["subaccount_id"].as_u64().unwrap();
        let opts = ReqOpts {
            nonce: v["nonce"].as_u64().unwrap(),
            expires_after: v["expires_after"].as_u64().unwrap(),
        };
        let i = &v["inputs"];
        let req = match s(&v["kind"]) {
            "place_orders" => {
                let orders: Vec<Order<'_>> =
                    i["orders"].as_array().unwrap().iter().map(order).collect();
                signer.sign_place_orders(sub, &orders, grouping(&i["grouping"]), opts)
            }
            "cancel_orders" => {
                let ids: Vec<u64> = i["order_ids"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|x| x.as_u64().unwrap())
                    .collect();
                signer.sign_cancel_orders(sub, &ids, opts)
            }
            "cancel_orders_by_cloid" => {
                signer.sign_cancel_orders_by_cloid(sub, &strs(&i["client_order_ids"]), opts)
            }
            "cancel_all_orders" => signer.sign_cancel_all_orders(sub, &strs(&i["symbols"]), opts),
            "modify_order" => signer.sign_modify_order(
                sub,
                i["order_id"].as_u64().unwrap(),
                &OrderChange {
                    price: s(&i["price"]),
                    quantity: s(&i["quantity"]),
                    trigger_price: s(&i["trigger_price"]),
                },
                opts,
            ),
            "modify_order_by_cloid" => signer.sign_modify_order_by_cloid(
                sub,
                s(&i["client_order_id"]),
                &OrderChange {
                    price: s(&i["price"]),
                    quantity: s(&i["quantity"]),
                    trigger_price: s(&i["trigger_price"]),
                },
                opts,
            ),
            "update_leverage" => {
                signer.sign_update_leverage(sub, s(&i["symbol"]), s(&i["leverage"]), opts)
            }
            "schedule_cancel" => {
                signer.sign_schedule_cancel(sub, i["timeout_seconds"].as_u64().unwrap(), opts)
            }
            "query" => Ok(sign_query(&signer, sub, i, opts)),
            "add_delegated_signer" => signer.sign_add_delegated_signer(
                sub,
                s(&i["delegate"]),
                &strs(&i["permissions"]),
                i["expires_at"].as_u64().unwrap(),
                opts,
            ),
            "remove_delegated_signer" => {
                signer.sign_remove_delegated_signer(sub, s(&i["delegate"]), opts)
            }
            "remove_all_delegated_signers" => signer.sign_remove_all_delegated_signers(sub, opts),
            other => panic!("unknown kind {other}"),
        }
        .unwrap_or_else(|e| panic!("{name}: {e}"));

        assert_eq!(hex32(req.digest()), s(&v["digest"]), "{name}: digest");
        assert_eq!(req.rest_body(), s(&v["body"]), "{name}: REST body");
        assert_eq!(req.ws_post(name), s(&v["ws_post"]), "{name}: ws post frame");
    }
}

#[test]
fn ws_auth_matches_sdk() {
    let d = load();
    let signer = Signer::from_hex(s(&d["private_key"])).unwrap();
    for v in d["ws_auth"].as_array().unwrap() {
        let name = s(&v["name"]);
        let auth = signer
            .sign_ws_auth(
                v["subaccount_id"].as_u64().unwrap(),
                v["timestamp"].as_u64().unwrap(),
            )
            .unwrap();
        assert_eq!(hex32(&auth.digest), s(&v["digest"]), "{name}: digest");
        assert_eq!(auth.message, s(&v["message"]), "{name}: message");
        assert_eq!(auth.signature, s(&v["signature"]), "{name}: signature");

        let frame: Value = serde_json::from_str(&auth.ws_message("auth-1")).unwrap();
        assert_eq!(frame["method"], "auth");
        assert_eq!(frame["params"]["message"], v["message"]);
        assert_eq!(frame["params"]["signature"], v["signature"]);
    }
}

#[test]
fn trigger_price_type_is_unsigned() {
    // `triggerPriceType` is a request-only field: it changes the body, not the signature.
    let d = load();
    let signer = Signer::from_hex(s(&d["private_key"])).unwrap();
    let base = Order {
        symbol: "BTC-USDT",
        side: Side::Sell,
        order_type: OrderType::TriggerSl,
        price: "",
        trigger_price: "60000",
        quantity: "0.1",
        reduce_only: true,
        is_trigger_market: true,
        client_order_id: "",
        close_position: false,
        post_only: false,
        expires_at: None,
        trigger_price_type: None,
        twap: None,
    };
    let opts = ReqOpts {
        nonce: 1,
        expires_after: 0,
    };
    let a = signer
        .sign_place_orders(1, &[base], Grouping::Na, opts)
        .unwrap();
    let b = signer
        .sign_place_orders(
            1,
            &[Order {
                trigger_price_type: Some(TriggerPriceType::Last),
                ..base
            }],
            Grouping::Na,
            opts,
        )
        .unwrap();
    assert_eq!(a.signature(), b.signature());
    assert!(b.params_json().contains(r#""triggerPriceType":"last""#));
    assert!(!a.params_json().contains("triggerPriceType"));
}
