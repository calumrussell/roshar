//! Parity with `edgex-python-sdk` 2.0.1. `tests/vectors.json` was produced by
//! `tests/gen_vectors.py` with a throwaway key and no network. Never regenerate it to make a
//! change pass.
//!
//! EIP-712 signing is RFC 6979 deterministic on both sides, so typed-data digests, `l2Signature`,
//! request bodies and HMAC headers are compared byte for byte.

use roshar_edgex::signer::*;
use roshar_edgex::types::*;
use roshar_edgex::{private_ws_config, SignedRequest, TradingApi, EDGEX_API_URL};
use roshar_http::RateLimitedClient;
use serde_json::Value;
use std::collections::BTreeMap;

fn load() -> Value {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/vectors.json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn s(v: &Value) -> &str {
    v.as_str().unwrap_or_else(|| panic!("not a string: {v}"))
}

fn wire<T: serde::de::DeserializeOwned>(v: &Value) -> T {
    serde_json::from_value(v.clone()).unwrap_or_else(|e| panic!("{v}: {e}"))
}

fn credentials(v: &Value) -> ApiCredentials {
    ApiCredentials::new(
        s(&v["api_key"]).to_string(),
        s(&v["api_passphrase"]).to_string(),
        s(&v["api_secret"]).to_string(),
    )
}

fn account_id(v: &Value) -> u64 {
    s(&v["account_id"]).parse().unwrap()
}

fn api(v: &Value) -> TradingApi {
    assert_eq!(s(&v["base_url"]), EDGEX_API_URL);
    TradingApi::new(
        RateLimitedClient::new(1, 1),
        EDGEX_API_URL,
        credentials(v),
        account_id(v),
    )
}

fn domain(v: &Value) -> L2Domain {
    let g = &v["metadata"]["global"];
    L2Domain::new(
        parse_chain_id(s(&g["nativeChainId"])).unwrap(),
        s(&g["contractAddress"]),
    )
    .unwrap()
}

fn header_map(h: &[(String, String)]) -> BTreeMap<String, String> {
    h.iter().cloned().collect()
}

fn expected_headers(h: &Value) -> BTreeMap<String, String> {
    h.as_object()
        .unwrap()
        .iter()
        .map(|(k, v)| (k.clone(), s(v).to_string()))
        .collect()
}

fn timestamp(req: &Value) -> u64 {
    s(&req["headers"][HEADER_TIMESTAMP]).parse().unwrap()
}

/// Checks a signed request against the SDK's recorded request: URL path, query or JSON body, and
/// all four auth headers.
fn assert_request(ours: &SignedRequest, sdk: &Value) {
    assert_eq!(ours.method, s(&sdk["method"]));
    let (path, query) = ours.url.split_once('?').unwrap_or((ours.url.as_str(), ""));
    assert_eq!(path, s(&sdk["url"]));
    match ours.method {
        "GET" => {
            let ours_q: BTreeMap<&str, &str> = query
                .split('&')
                .filter(|p| !p.is_empty())
                .map(|p| p.split_once('=').unwrap())
                .collect();
            let sdk_q: BTreeMap<&str, &str> = sdk["params"]
                .as_object()
                .unwrap()
                .iter()
                .map(|(k, v)| (k.as_str(), s(v)))
                .collect();
            assert_eq!(ours_q, sdk_q, "query of {}", ours.url);
        }
        _ => {
            let body: Value = serde_json::from_str(ours.body.as_ref().unwrap()).unwrap();
            assert_eq!(body, sdk["json"], "body of {}", ours.url);
        }
    }
    assert_eq!(
        header_map(&ours.headers),
        expected_headers(&sdk["headers"]),
        "headers of {} (signed payload {:?})",
        ours.url,
        ours.sign_payload
    );
}

#[test]
fn signer_address() {
    let v = load();
    let signer = L2Signer::from_hex(s(&v["trading_private_key"])).unwrap();
    assert_eq!(signer.address(), s(&v["signer"]));
}

#[test]
fn nonce_from_client_order_id() {
    let v = load();
    for c in v["calc_nonce"].as_array().unwrap() {
        let nonce: u64 = s(&c["nonce"]).parse().unwrap();
        assert_eq!(calc_nonce(s(&c["client_order_id"])), nonce, "{c}");
    }
}

#[test]
fn market_order_price_bounds() {
    let v = load();
    let contracts = v["metadata"]["contractList"].as_array().unwrap();
    for o in v["orders"].as_array().unwrap() {
        let input = &o["input"];
        let order_type: EdgexOrderType = wire(&input["type"]);
        if !matches!(
            order_type,
            EdgexOrderType::Market | EdgexOrderType::StopMarket | EdgexOrderType::TakeProfitMarket
        ) {
            continue;
        }
        let cid = s(&input["contract_id"]);
        let contract = contracts
            .iter()
            .find(|c| s(&c["contractId"]) == cid)
            .unwrap();
        let side: EdgexSide = wire(&input["side"]);
        let oracle = s(&v["oracle_prices"][cid]);
        assert_eq!(
            market_order_l2_price(side, oracle, s(&contract["tickSize"])).unwrap(),
            s(&o["l2_price"]),
            "{input}"
        );
    }
}

#[test]
fn create_order_bodies_and_signatures() {
    let v = load();
    let signer = L2Signer::from_hex(s(&v["trading_private_key"])).unwrap();
    let domain = domain(&v);
    let api = api(&v);
    let account = account_id(&v);
    let contracts = v["metadata"]["contractList"].as_array().unwrap();
    let coins = v["metadata"]["coinList"].as_array().unwrap();

    for o in v["orders"].as_array().unwrap() {
        let req = &o["request"];
        let body = &req["json"];
        let contract = contracts
            .iter()
            .find(|c| c["contractId"] == body["contractId"])
            .unwrap();
        let quote = coins
            .iter()
            .find(|c| c["coinId"] == contract["quoteCoinId"])
            .unwrap();

        let trigger_price = s(&body["triggerPrice"]);
        let trigger = (!trigger_price.is_empty()).then(|| {
            (
                trigger_price,
                wire::<EdgexTriggerPriceType>(&body["triggerPriceType"]),
            )
        });
        let order = NewOrder {
            contract_id: s(&body["contractId"]),
            side: wire(&body["side"]),
            order_type: wire(&body["type"]),
            time_in_force: wire(&body["timeInForce"]),
            price: s(&body["price"]),
            size: s(&body["size"]),
            client_order_id: s(&body["clientOrderId"]),
            expire_time_ms: s(&body["expireTime"]).parse().unwrap(),
            reduce_only: body["reduceOnly"].as_bool().unwrap(),
            trigger,
        };
        let l2 = OrderL2Inputs {
            l2_price: s(&o["l2_price"]),
            fee_rate: s(&o["fee_rate"]),
            contract_resolution: s(&contract["resolution"]),
            quote_resolution: s(&quote["resolution"]),
            quote_coin_id: s(&contract["quoteCoinId"]),
        };

        // Typed-data message and digest.
        let typed = &o["typed_data"];
        let msg = &typed["message"];
        let amounts = l2_order_amounts(
            order.size,
            l2.l2_price,
            l2.fee_rate,
            l2.contract_resolution,
            l2.quote_resolution,
        )
        .unwrap();
        assert_eq!(
            amounts.amount_synthetic.to_string(),
            s(&msg["amountSynthetic"])
        );
        assert_eq!(
            amounts.amount_collateral.to_string(),
            s(&msg["amountCollateral"])
        );
        assert_eq!(amounts.amount_fee.to_string(), s(&msg["amountFee"]));
        let params = LimitOrderParams {
            nonce: s(&msg["base"]["nonce"]).parse().unwrap(),
            account_id: s(&msg["base"]["accountId"]).parse().unwrap(),
            expiration_timestamp: s(&msg["base"]["expirationTimestamp"]).parse().unwrap(),
            amount_synthetic: amounts.amount_synthetic,
            amount_collateral: amounts.amount_collateral,
            amount_fee: amounts.amount_fee,
            asset_id_synthetic: s(&msg["assetIdSynthetic"]).parse().unwrap(),
            asset_id_collateral: s(&msg["assetIdCollateral"]).parse().unwrap(),
            is_buying_synthetic: msg["isBuyingSynthetic"].as_bool().unwrap(),
        };
        assert_eq!(s(&msg["base"]["signer"]), signer.address());
        assert_eq!(
            format!(
                "0x{}",
                hex::encode(signer.limit_order_digest(&domain, &params))
            ),
            s(&typed["digest"]),
            "digest of {}",
            order.client_order_id
        );

        // Full body (including l2Signature) and HMAC headers.
        let ours = signer
            .create_order_body(&domain, account, &order, &l2)
            .unwrap();
        assert_eq!(
            serde_json::to_value(&ours).unwrap(),
            *body,
            "{}",
            order.client_order_id
        );
        assert_eq!(ours.l2_signature, s(&typed["signature"]));
        let signed = api
            .signed_post(TradingApi::CREATE_ORDER, &ours, timestamp(req))
            .unwrap();
        assert_request(&signed, req);
    }
}

#[test]
fn set_margin_mode() {
    let v = load();
    let signer = L2Signer::from_hex(s(&v["trading_private_key"])).unwrap();
    let domain = domain(&v);
    let api = api(&v);
    for m in v["margin_mode"].as_array().unwrap() {
        let req = &m["request"];
        let body = &req["json"];
        let mode = match s(&m["input"]["margin_mode"]) {
            "0" => EdgexMarginMode::Cross,
            "1" => EdgexMarginMode::Isolated,
            other => panic!("margin mode {other}"),
        };
        let nonce = calc_nonce(s(&m["input"]["client_order_id"]));
        let params = MarginModeParams {
            account_id: account_id(&v),
            asset_id: s(&m["input"]["contract_id"]).parse().unwrap(),
            margin_mode: mode,
            nonce,
        };
        assert_eq!(
            format!(
                "0x{}",
                hex::encode(signer.margin_mode_digest(&domain, &params))
            ),
            s(&m["typed_data"]["digest"])
        );
        let ours = signer
            .set_margin_mode_body(
                &domain,
                account_id(&v),
                s(&m["input"]["contract_id"]),
                mode,
                nonce,
                s(&body["l2ExpireTime"]).parse().unwrap(),
            )
            .unwrap();
        assert_eq!(serde_json::to_value(&ours).unwrap(), *body);
        let signed = api
            .signed_post(TradingApi::SET_MARGIN_MODE, &ours, timestamp(req))
            .unwrap();
        assert_request(&signed, req);
    }
}

#[test]
fn private_requests() {
    let v = load();
    let api = api(&v);
    let account = s(&v["account_id"]).to_string();
    let acct = || ("accountId", account.clone());

    for r in v["requests"].as_array().unwrap() {
        let req = &r["request"];
        let ts = timestamp(req);
        let label = s(&r["label"]);
        let signed = match label {
            "cancel_by_id" => api
                .signed_post(
                    TradingApi::CANCEL_ORDER_BY_ID,
                    &api.cancel_order_by_id_body(&["816966711830904982"]),
                    ts,
                )
                .unwrap(),
            "cancel_by_client_id" => api
                .signed_post(
                    TradingApi::CANCEL_ORDER_BY_CLIENT_ORDER_ID,
                    &api.cancel_order_by_client_order_id_body(&["a-1", "b-2"]),
                    ts,
                )
                .unwrap(),
            "cancel_all_contract" => {
                let f = CancelAllFilter {
                    contract_ids: Some(vec!["30000001".into()]),
                    ..Default::default()
                };
                api.signed_post(TradingApi::CANCEL_ALL_ORDER, &api.cancel_all_body(&f), ts)
                    .unwrap()
            }
            "cancel_all_filters" => {
                let f = CancelAllFilter {
                    coin_ids: Some(vec![]),
                    contract_ids: Some(vec!["30000001".into(), "30000002".into()]),
                    order_types: Some(vec![EdgexOrderType::Limit]),
                    order_statuses: Some(vec![
                        EdgexOrderStatus::Open,
                        EdgexOrderStatus::Untriggered,
                    ]),
                    is_position_tpsl: Some(vec![false]),
                };
                api.signed_post(TradingApi::CANCEL_ALL_ORDER, &api.cancel_all_body(&f), ts)
                    .unwrap()
            }
            "cancel_all_empty" => api
                .signed_post(
                    TradingApi::CANCEL_ALL_ORDER,
                    &api.cancel_all_body(&CancelAllFilter::default()),
                    ts,
                )
                .unwrap(),
            "active_orders" => {
                let q = OrderPageQuery {
                    size: Some(50),
                    contract_ids: vec!["30000001".into(), "30000002".into()],
                    statuses: vec![EdgexOrderStatus::Open],
                    is_position_tpsl: Some(false),
                    ..Default::default()
                };
                api.signed_get(
                    TradingApi::GET_ACTIVE_ORDER_PAGE,
                    &api.order_page_params(&q, false),
                    ts,
                )
            }
            "active_orders_plain" => api.signed_get(
                TradingApi::GET_ACTIVE_ORDER_PAGE,
                &api.order_page_params(&OrderPageQuery::default(), false),
                ts,
            ),
            "history_orders" => {
                let q = OrderPageQuery {
                    size: Some(20),
                    offset_data: Some("0880A08A97B532".into()),
                    types: vec![EdgexOrderType::Limit, EdgexOrderType::Market],
                    ..Default::default()
                };
                api.signed_get(
                    TradingApi::GET_HISTORY_ORDER_PAGE,
                    &api.order_page_params(&q, true),
                    ts,
                )
            }
            "fills" => {
                let q = FillPageQuery {
                    size: Some(100),
                    order_ids: vec!["816966711830904982".into()],
                    start_created_time_inclusive: Some(1794700000000),
                    ..Default::default()
                };
                api.signed_get(TradingApi::GET_FILL_PAGE, &api.fill_page_params(&q), ts)
            }
            "orders_by_id" => api.signed_get(
                TradingApi::GET_ORDER_BY_ID,
                &[acct(), ("orderIdList", "1,2".to_string())],
                ts,
            ),
            "orders_by_client_id" => api.signed_get(
                TradingApi::GET_ORDER_BY_CLIENT_ORDER_ID,
                &[acct(), ("clientOrderIdList", "a-1".to_string())],
                ts,
            ),
            "max_order_size" => api
                .signed_post(
                    TradingApi::GET_MAX_CREATE_ORDER_SIZE,
                    &api.max_order_size_body("30000001", Some("65000.0")),
                    ts,
                )
                .unwrap(),
            "account_asset" => api.signed_get(TradingApi::GET_ACCOUNT_ASSET, &[acct()], ts),
            "account_by_id" => api.signed_get(TradingApi::GET_ACCOUNT_BY_ID, &[acct()], ts),
            "positions_by_contract" => api.signed_get(
                TradingApi::GET_POSITION_BY_CONTRACT_ID,
                &[acct(), ("contractIdList", "30000001,30000002".to_string())],
                ts,
            ),
            "collateral_by_coin" => api.signed_get(
                TradingApi::GET_COLLATERAL_BY_COIN_ID,
                &[acct(), ("coinIdList", "1000".to_string())],
                ts,
            ),
            "update_leverage" => api
                .signed_post(
                    TradingApi::UPDATE_LEVERAGE_SETTING,
                    &api.update_leverage_body("30000001", "20"),
                    ts,
                )
                .unwrap(),
            other => panic!("unhandled vector {other}"),
        };
        assert_request(&signed, req);
    }
}

#[test]
fn private_websocket_handshake() {
    let v = load();
    let ws = &v["private_ws"];
    let ts: u64 = s(&ws["headers"][HEADER_TIMESTAMP]).parse().unwrap();
    let config = private_ws_config(&credentials(&v), account_id(&v), ts);
    assert_eq!(config.url, s(&ws["url"]));
    assert_eq!(
        header_map(&config.headers),
        expected_headers(&ws["headers"])
    );
}
