//! Parity with the official ApeX Omni Python SDK (`apexomni`, whose zkLink signer is a native
//! library). `vectors.json` was produced by `gen_vectors.py` with throwaway keys and no network.
//! zkLink musig signatures are deterministic, so every byte is compared.

use roshar_apex::auth::ApexApiCredentials;
use roshar_apex::order::{
    contract_ids, sign_order, ApexOrderContext, ApexOrderRequest, ApexOrderType, ApexSide,
    ApexTimeInForce,
};
use roshar_apex::zk::ZkSigner;
use serde_json::Value;

const VECTORS: &str = include_str!("vectors.json");

fn vectors() -> Value {
    serde_json::from_str(VECTORS).expect("vectors.json parses")
}

fn s(v: &Value) -> &str {
    v.as_str().expect("string")
}

fn credentials(v: &Value) -> ApexApiCredentials {
    let c = &v["credentials"];
    ApexApiCredentials {
        key: s(&c["key"]).to_string(),
        secret: s(&c["secret"]).to_string(),
        passphrase: s(&c["passphrase"]).to_string(),
    }
}

#[test]
fn keys_match_sdk() {
    let v = vectors();
    for key in v["keys"].as_array().unwrap() {
        let signer = ZkSigner::from_seeds_hex(s(&key["seeds"])).unwrap();
        assert_eq!(signer.l2_key_hex(), s(&key["l2_key"]));
        assert_eq!(
            format!("0x{}", hex::encode(signer.public_key_hash())),
            s(&key["pub_key_hash"])
        );
    }
}

#[test]
fn musig_matches_sdk() {
    let v = vectors();
    for case in v["musig"].as_array().unwrap() {
        let signer = ZkSigner::from_seeds_hex(s(&case["seeds"])).unwrap();
        let msg = hex::decode(s(&case["message"])).unwrap();
        assert_eq!(
            signer.sign_musig_hex(&msg).unwrap(),
            s(&case["signature"]),
            "message {}",
            s(&case["message"])
        );
    }
}

#[test]
fn rejects_oversized_message_and_short_seeds() {
    let signer = ZkSigner::from_seeds(&[7u8; 65]).unwrap();
    assert!(signer.sign_musig(&[0u8; 93]).is_err());
    assert!(ZkSigner::from_seeds(&[7u8; 31]).is_err());
}

fn side(v: &str) -> ApexSide {
    match v {
        "BUY" => ApexSide::Buy,
        "SELL" => ApexSide::Sell,
        other => panic!("side {other}"),
    }
}

fn order_type(v: &str) -> ApexOrderType {
    match v {
        "LIMIT" => ApexOrderType::Limit,
        "MARKET" => ApexOrderType::Market,
        "STOP_LIMIT" => ApexOrderType::StopLimit,
        "STOP_MARKET" => ApexOrderType::StopMarket,
        "TAKE_PROFIT_LIMIT" => ApexOrderType::TakeProfitLimit,
        "TAKE_PROFIT_MARKET" => ApexOrderType::TakeProfitMarket,
        other => panic!("type {other}"),
    }
}

fn tif(v: &str) -> ApexTimeInForce {
    match v {
        "GOOD_TIL_CANCEL" => ApexTimeInForce::GoodTilCancel,
        "FILL_OR_KILL" => ApexTimeInForce::FillOrKill,
        "IMMEDIATE_OR_CANCEL" => ApexTimeInForce::ImmediateOrCancel,
        "POST_ONLY" => ApexTimeInForce::PostOnly,
        other => panic!("timeInForce {other}"),
    }
}

#[test]
fn orders_match_sdk() {
    let v = vectors();
    let creds = credentials(&v);
    for case in v["orders"].as_array().unwrap() {
        let name = s(&case["name"]);
        let inputs = &case["inputs"];
        let signer = ZkSigner::from_seeds_hex(s(&case["seeds"])).unwrap();
        let ctx = ApexOrderContext {
            account_id: s(&case["account_id"]),
            l2_pair_id: s(&case["l2_pair_id"]).parse().unwrap(),
            settle_decimals: case["settle_decimals"].as_u64().unwrap() as u32,
            taker_fee_rate: s(&case["taker_fee_rate"]),
            maker_fee_rate: s(&case["maker_fee_rate"]),
        };
        let req = ApexOrderRequest {
            symbol: s(&inputs["symbol"]),
            side: side(s(&inputs["side"])),
            order_type: order_type(s(&inputs["type"])),
            size: s(&inputs["size"]),
            price: s(&inputs["price"]),
            time_in_force: tif(s(&inputs["timeInForce"])),
            reduce_only: inputs["reduceOnly"].as_bool().unwrap_or(false),
            client_id: s(&inputs["clientId"]),
            expiration_ms: case["expiration"].as_u64().unwrap(),
            trigger_price: inputs["triggerPrice"].as_str(),
            trigger_price_type: inputs["triggerPriceType"].as_str(),
            trailing_percent: inputs["trailingPercent"].as_str(),
            is_position_tpsl: false,
        };
        let signed = sign_order(&signer, &ctx, &req).unwrap();

        let b = &case["builder"];
        let c = &signed.contract;
        assert_eq!(
            c.account_id as u64,
            b["account_id"].as_u64().unwrap(),
            "{name}"
        );
        assert_eq!(c.slot_id as u64, b["slot_id"].as_u64().unwrap(), "{name}");
        assert_eq!(c.nonce as u64, b["nonce"].as_u64().unwrap(), "{name}");
        assert_eq!(c.pair_id as u64, b["pair_id"].as_u64().unwrap(), "{name}");
        assert_eq!(c.size.to_string(), s(&b["size"]), "{name}");
        assert_eq!(c.price.to_string(), s(&b["price"]), "{name}");
        assert_eq!(c.is_long, b["direction"].as_bool().unwrap(), "{name}");
        assert_eq!(
            c.taker_fee_rate as u64,
            b["taker_fee_rate"].as_u64().unwrap(),
            "{name}"
        );
        assert_eq!(
            c.maker_fee_rate as u64,
            b["maker_fee_rate"].as_u64().unwrap(),
            "{name}"
        );
        assert_eq!(
            hex::encode(c.to_bytes().unwrap()),
            s(&case["contract_bytes"]),
            "{name}"
        );
        assert_eq!(signed.signature, s(&case["signature"]), "{name}");
        assert_eq!(signed.limit_fee, s(&case["limit_fee"]), "{name}");

        let mut ours: Vec<(String, String)> = signed
            .form
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect();
        let mut theirs: Vec<(String, String)> = case["form"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(k, v)| (k.clone(), s(v).to_string()))
            .collect();
        ours.sort();
        theirs.sort();
        assert_eq!(ours, theirs, "{name}");

        let hmac = &case["hmac"];
        assert_eq!(
            creds.sign_request(
                hmac["timestamp"].as_u64().unwrap(),
                "POST",
                s(&case["endpoint"]),
                &signed.form
            ),
            s(&hmac["signature"]),
            "{name}"
        );
    }
}

#[test]
fn slot_id_reproduces_sdk_float_rounding() {
    // sha256("1970091") lands where the SDK's float division rounds up; the exact quotient is
    // 1793084397.
    let (_, slot, _) = contract_ids("1970091", "1").unwrap();
    assert_eq!(slot, 1793084398);
}

#[test]
fn request_hmac_matches_sdk() {
    let v = vectors();
    let creds = credentials(&v);
    for case in v["hmac"].as_array().unwrap() {
        let form: Vec<(&str, String)> = case["data"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(k, v)| (k.as_str(), s(v).to_string()))
            .collect();
        assert_eq!(
            creds.sign_request(
                case["timestamp"].as_u64().unwrap(),
                s(&case["method"]),
                s(&case["path"]),
                &form
            ),
            s(&case["signature"]),
            "{} {}",
            s(&case["method"]),
            s(&case["path"])
        );
    }
}

#[test]
fn ws_login_matches_sdk() {
    let v = vectors();
    let creds = credentials(&v);
    let login = &v["ws_login"];
    let ts = login["timestamp"].as_u64().unwrap();
    let msg: Value =
        serde_json::from_str(&creds.ws_login_message(ts, &["ws_zk_accounts_v3"])).unwrap();
    assert_eq!(msg["op"], "login");
    let inner: Value = serde_json::from_str(s(&msg["args"][0])).unwrap();
    assert_eq!(inner["type"], "login");
    assert_eq!(inner["requestPath"], login["request_path"]);
    assert_eq!(inner["httpMethod"], "GET");
    assert_eq!(inner["apiKey"], v["credentials"]["key"]);
    assert_eq!(inner["passphrase"], v["credentials"]["passphrase"]);
    assert_eq!(inner["timestamp"].as_u64().unwrap(), ts);
    assert_eq!(inner["topics"][0], "ws_zk_accounts_v3");
    assert_eq!(inner["signature"], login["signature"]);
}
