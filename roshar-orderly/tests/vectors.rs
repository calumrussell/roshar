//! Parity with the official Python SDK (`tests/gen_vectors.py` → `tests/vectors.json`).
//! ed25519 and RFC 6979 ECDSA are deterministic, so every path, body and signature must match
//! byte for byte. Never regenerate the vectors to make a change pass.

use roshar_orderly::signer::{account_id, request_message};
use roshar_orderly::{
    AddOrderlyKey, OrderlyEditOrderRequest, OrderlyFundingFeeQuery, OrderlyKey, OrderlyMarginMode,
    OrderlyOrderRequest, OrderlyOrderType, OrderlyOrdersQuery, OrderlySide, OrderlyTradesQuery,
    PreparedRequest, WalletSigner,
};
use serde_json::Value;

fn vectors() -> Value {
    serde_json::from_str(include_str!("vectors.json")).expect("vectors.json")
}

fn opt_str(v: &Value, k: &str) -> Option<String> {
    v.get(k).and_then(Value::as_str).map(str::to_string)
}

fn opt_u64(v: &Value, k: &str) -> Option<u64> {
    v.get(k).and_then(Value::as_u64)
}

fn opt_u32(v: &Value, k: &str) -> Option<u32> {
    opt_u64(v, k).map(|n| u32::try_from(n).expect("u32"))
}

fn req_str<'a>(v: &'a Value, k: &str) -> &'a str {
    v[k].as_str().unwrap_or_else(|| panic!("missing {k}"))
}

fn order_type(s: &str) -> OrderlyOrderType {
    match s {
        "LIMIT" => OrderlyOrderType::Limit,
        "MARKET" => OrderlyOrderType::Market,
        "IOC" => OrderlyOrderType::Ioc,
        "FOK" => OrderlyOrderType::Fok,
        "POST_ONLY" => OrderlyOrderType::PostOnly,
        "ASK" => OrderlyOrderType::Ask,
        "BID" => OrderlyOrderType::Bid,
        other => panic!("order type {other}"),
    }
}

fn side(s: &str) -> OrderlySide {
    match s {
        "BUY" => OrderlySide::Buy,
        "SELL" => OrderlySide::Sell,
        other => panic!("side {other}"),
    }
}

fn margin_mode(s: &str) -> OrderlyMarginMode {
    match s {
        "CROSS" => OrderlyMarginMode::Cross,
        "ISOLATED" => OrderlyMarginMode::Isolated,
        other => panic!("margin mode {other}"),
    }
}

fn order(i: &Value) -> OrderlyOrderRequest {
    let mut o = OrderlyOrderRequest::new(
        req_str(i, "symbol"),
        order_type(req_str(i, "order_type")),
        side(req_str(i, "side")),
    );
    o.client_order_id = opt_str(i, "client_order_id");
    o.order_price = opt_str(i, "order_price");
    o.order_quantity = opt_str(i, "order_quantity");
    o.order_amount = opt_str(i, "order_amount");
    o.reduce_only = i.get("reduce_only").and_then(Value::as_bool);
    o.visible_quantity = opt_str(i, "visible_quantity");
    o.order_tag = opt_str(i, "order_tag");
    o.slippage = opt_str(i, "slippage");
    o.level = opt_u64(i, "level").map(|l| u8::try_from(l).expect("level"));
    o.post_only_adjust = i.get("post_only_adjust").and_then(Value::as_bool);
    o.margin_mode = opt_str(i, "margin_mode").map(|m| margin_mode(&m));
    o
}

fn build(name: &str, i: &Value) -> PreparedRequest {
    let r = match name {
        n if n.starts_with("create_") => PreparedRequest::create_order(&order(i)),
        "batch_create" => {
            let orders: Vec<_> = i["orders"].as_array().unwrap().iter().map(order).collect();
            PreparedRequest::batch_create_orders(&orders)
        }
        "edit_order" | "edit_order_cloid_reduce_only" => {
            PreparedRequest::edit_order(&OrderlyEditOrderRequest {
                order_id: i["order_id"].as_u64().unwrap(),
                symbol: req_str(i, "symbol").to_string(),
                order_type: order_type(req_str(i, "order_type")),
                side: side(req_str(i, "side")),
                client_order_id: opt_str(i, "client_order_id"),
                order_price: opt_str(i, "order_price"),
                order_quantity: opt_str(i, "order_quantity"),
                order_amount: opt_str(i, "order_amount"),
                reduce_only: i.get("reduce_only").and_then(Value::as_bool),
                visible_quantity: opt_str(i, "visible_quantity"),
            })
        }
        "cancel_order" => {
            PreparedRequest::cancel_order(i["order_id"].as_u64().unwrap(), req_str(i, "symbol"))
        }
        "cancel_order_by_client_order_id" => PreparedRequest::cancel_order_by_client_order_id(
            req_str(i, "client_order_id"),
            req_str(i, "symbol"),
        ),
        "cancel_all_symbol" | "cancel_all" => {
            PreparedRequest::cancel_all_orders(i["symbol"].as_str())
        }
        "batch_cancel" => {
            let ids: Vec<u64> = i["order_ids"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap())
                .collect();
            PreparedRequest::batch_cancel_orders(&ids)
        }
        "batch_cancel_by_client_order_id" => {
            let ids: Vec<&str> = i["client_order_ids"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap())
                .collect();
            PreparedRequest::batch_cancel_orders_by_client_order_id(&ids)
        }
        "cancel_all_after" => {
            PreparedRequest::cancel_all_after(i["trigger_after"].as_u64().unwrap())
        }
        "get_orders" | "get_orders_time_range" | "get_orders_all" => {
            PreparedRequest::get_orders(&OrderlyOrdersQuery {
                symbol: i.get("symbol").and_then(Value::as_str),
                order_type: i.get("order_type").and_then(Value::as_str),
                side: i.get("side").and_then(Value::as_str),
                status: i.get("status").and_then(Value::as_str),
                start_t: opt_u64(i, "start_t"),
                end_t: opt_u64(i, "end_t"),
                page: opt_u32(i, "page"),
                size: opt_u32(i, "size"),
                order_tag: i.get("order_tag").and_then(Value::as_str),
                sort_by: i.get("sort_by").and_then(Value::as_str),
                product_type: None,
            })
        }
        "get_order" => PreparedRequest::get_order(i["order_id"].as_u64().unwrap()),
        "get_order_by_client_order_id" => {
            PreparedRequest::get_order_by_client_order_id(req_str(i, "client_order_id"))
        }
        "get_order_trades" => PreparedRequest::get_order_trades(i["order_id"].as_u64().unwrap()),
        "get_trades" => PreparedRequest::get_trades(&OrderlyTradesQuery {
            symbol: i.get("symbol").and_then(Value::as_str),
            start_t: opt_u64(i, "start_t"),
            end_t: opt_u64(i, "end_t"),
            page: opt_u32(i, "page"),
            size: opt_u32(i, "size"),
            product_type: None,
        }),
        "get_trade" => PreparedRequest::get_trade(i["trade_id"].as_u64().unwrap()),
        "get_positions" => PreparedRequest::get_positions(None),
        "get_position" => PreparedRequest::get_position(req_str(i, "symbol")),
        "get_holding" => PreparedRequest::get_holding(None),
        "get_client_info" => PreparedRequest::get_client_info(),
        "get_account_info" => PreparedRequest::get_account_info(),
        "get_funding_fee_history" => {
            PreparedRequest::get_funding_fee_history(&OrderlyFundingFeeQuery {
                symbol: i.get("symbol").and_then(Value::as_str),
                start_t: opt_u64(i, "start_t"),
                end_t: opt_u64(i, "end_t"),
                page: opt_u32(i, "page"),
                size: opt_u32(i, "size"),
            })
        }
        "get_key_info" => {
            PreparedRequest::get_key_info(i.get("key_status").and_then(Value::as_str))
        }
        "get_orderbook" | "get_orderbook_default_depth" => {
            PreparedRequest::get_orderbook(req_str(i, "symbol"), opt_u32(i, "max_level"))
        }
        "get_kline" | "get_kline_no_limit" => PreparedRequest::get_kline(
            req_str(i, "symbol"),
            req_str(i, "type"),
            opt_u32(i, "limit"),
        ),
        other => panic!("no builder for vector {other}"),
    };
    r.unwrap_or_else(|e| panic!("{name}: {e}"))
}

#[test]
fn keys_and_account_id_match_sdk() {
    let v = vectors();
    let key = OrderlyKey::from_secret(v["orderly_secret"].as_str().unwrap()).unwrap();
    assert_eq!(key.public_key(), v["orderly_key"].as_str().unwrap());
    let wallet = WalletSigner::from_hex(v["wallet_private_key"].as_str().unwrap()).unwrap();
    assert_eq!(wallet.address(), v["wallet_address"].as_str().unwrap());
    assert_eq!(
        account_id(
            v["wallet_address"].as_str().unwrap(),
            v["broker_id"].as_str().unwrap()
        )
        .unwrap(),
        v["account_id"].as_str().unwrap()
    );
}

#[test]
fn requests_match_sdk() {
    let v = vectors();
    let key = OrderlyKey::from_secret(v["orderly_secret"].as_str().unwrap()).unwrap();
    let account = v["account_id"].as_str().unwrap();
    let requests = v["requests"].as_array().unwrap();
    assert!(requests.len() >= 30);
    for r in requests {
        let name = r["name"].as_str().unwrap();
        let ts = r["timestamp"].as_u64().unwrap();
        let req = build(name, &r["inputs"]);
        assert_eq!(
            req.method.as_str(),
            r["method"].as_str().unwrap(),
            "{name}: method"
        );
        assert_eq!(req.path, r["path"].as_str().unwrap(), "{name}: path");
        assert_eq!(req.body.as_deref(), r["body"].as_str(), "{name}: body");
        assert_eq!(
            request_message(ts, req.method.as_str(), &req.path, req.body.as_deref()),
            r["message"].as_str().unwrap(),
            "{name}: message"
        );
        assert_eq!(
            key.sign_request(ts, &req),
            r["signature"].as_str().unwrap(),
            "{name}: signature"
        );
        let headers = key.auth_headers(account, ts, &req);
        assert_eq!(headers.timestamp, ts.to_string());
        assert_eq!(headers.key, v["orderly_key"].as_str().unwrap());
        assert_eq!(headers.account_id, account);
        assert_eq!(headers.signature, r["signature"].as_str().unwrap());
    }
}

#[test]
fn ws_auth_matches_sdk() {
    let v = vectors();
    let key = OrderlyKey::from_secret(v["orderly_secret"].as_str().unwrap()).unwrap();
    let w = &v["ws_auth"];
    let msg = key.ws_auth_message(w["id"].as_str().unwrap(), w["timestamp"].as_u64().unwrap());
    assert_eq!(msg, w["message"].as_str().unwrap());
    assert_eq!(
        key.sign(w["timestamp"].as_u64().unwrap().to_string().as_bytes()),
        w["sign"].as_str().unwrap()
    );
}

#[test]
fn add_orderly_key_matches_sdk() {
    let v = vectors();
    let wallet = WalletSigner::from_hex(v["wallet_private_key"].as_str().unwrap()).unwrap();
    let cases = v["add_orderly_key"].as_array().unwrap();
    assert!(!cases.is_empty());
    for c in cases {
        let msg = AddOrderlyKey {
            broker_id: c["broker_id"].as_str().unwrap(),
            chain_id: c["chain_id"].as_u64().unwrap(),
            orderly_key: c["orderly_key"].as_str().unwrap(),
            scope: c["scope"].as_str().unwrap(),
            timestamp: c["timestamp"].as_u64().unwrap(),
            expiration: c["expiration"].as_u64().unwrap(),
        };
        let signed = wallet.sign_add_orderly_key(&msg).unwrap();
        assert_eq!(signed.signature, c["signature"].as_str().unwrap());
        assert_eq!(signed.body, c["body"].as_str().unwrap());
        assert_eq!(wallet.address(), c["user_address"].as_str().unwrap());
    }
}

#[test]
fn secret_forms() {
    let v = vectors();
    let secret = v["orderly_secret"].as_str().unwrap();
    let bare = secret.strip_prefix("ed25519:").unwrap();
    let a = OrderlyKey::from_secret(secret).unwrap();
    let b = OrderlyKey::from_secret(bare).unwrap();
    assert_eq!(a.public_key(), b.public_key());
    assert!(OrderlyKey::from_secret("ed25519:abc").is_err());
}

#[test]
fn rejects_bad_numbers_and_url_text() {
    let mut o =
        OrderlyOrderRequest::new("PERP_BTC_USDC", OrderlyOrderType::Limit, OrderlySide::Buy);
    o.order_price = Some("1,000".to_string());
    assert!(PreparedRequest::create_order(&o).is_err());
    assert!(PreparedRequest::cancel_order_by_client_order_id("a&b", "PERP_BTC_USDC").is_err());
    assert!(PreparedRequest::get_position("PERP/BTC").is_err());
    assert!(PreparedRequest::batch_cancel_orders(&[]).is_err());
}
