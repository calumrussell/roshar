//! Offline parsing of live captures from tradingapi.bullet.xyz (2026-11-15, UK IP).

use roshar_bullet::types::ws::{
    list_subscriptions, ping, subscribe, unsubscribe, BulletDepthLevels, BulletKlineInterval,
    BulletTopic, BulletWsMessage,
};
use roshar_bullet::types::*;
use serde::de::DeserializeOwned;
use serde_json::Value;

fn parse<T: DeserializeOwned>(json: &str) -> T {
    serde_json::from_str(json).unwrap()
}

#[test]
fn market_data() {
    let t: BulletServerTime = parse(include_str!("fixtures/time.json"));
    assert_eq!(t.server_time, 1_794_779_980_948_634);

    let info: BulletExchangeInfo = parse(include_str!("fixtures/exchange_info.json"));
    assert_eq!(
        info.chain_hash.as_deref(),
        Some("0xb08897f45bad670afdeb0f9a37f37f6d460903cc9a77a2cf86339416ac0b1b4d")
    );
    assert_eq!(info.chain_info.as_ref().unwrap().chain_id, 4321);
    assert!(info.rate_limits.is_empty());
    let btc = info.symbols.iter().find(|s| s.symbol == "BTC-USD").unwrap();
    assert_eq!(btc.market_id, 1);
    assert!(btc
        .filters
        .iter()
        .any(|f| matches!(f, BulletFilter::LotSize { .. })));
    assert!(info.assets.iter().any(|a| a.token_id.is_none()));
    assert!(info.assets.iter().any(|a| a.user_deposit_limit.is_some()));

    let book: BulletOrderBook = parse(include_str!("fixtures/depth.json"));
    assert_eq!(book.last_update_id, 22_075_423);
    assert_eq!(book.bids[0], ("64921".to_string(), "0.0029".to_string()));
    assert_eq!(book.asks.len(), 5);

    let trades: Vec<BulletTrade> = parse(include_str!("fixtures/trades.json"));
    assert_eq!(trades.len(), 5);
    assert!(trades[0].is_buyer_maker);

    let tickers: Vec<BulletTicker24hr> = parse(include_str!("fixtures/ticker_24hr.json"));
    assert!(!tickers.is_empty());
    let prices: Vec<BulletPriceTicker> = parse(include_str!("fixtures/ticker_price.json"));
    assert!(!prices.is_empty());

    let premium: Vec<BulletPremiumIndex> = parse(include_str!("fixtures/premium_index.json"));
    assert_eq!(premium[0].symbol, "SOL-USD");
    let funding: Vec<BulletFundingRate> = parse(include_str!("fixtures/funding_rate.json"));
    assert_eq!(funding[0].funding_time, 1_794_772_800_000_000);
    let info: Vec<BulletFundingInfo> = parse(include_str!("fixtures/funding_info.json"));
    assert!(info.iter().all(|i| i.funding_interval_hours == 1));
    let oi: Vec<BulletOpenInterest> = parse(include_str!("fixtures/open_interest.json"));
    assert!(!oi.is_empty());
    let cfg: Vec<BulletSymbolConfig> = parse(include_str!("fixtures/symbol_config.json"));
    assert!(!cfg.is_empty());
    let brackets: Vec<BulletLeverageBracket> =
        parse(include_str!("fixtures/leverage_bracket.json"));
    assert_eq!(brackets[0].brackets.len(), 5);
    assert_eq!(brackets[0].notional_coef, None);
}

/// Parses `json`, then checks that removing `field` from the object at `parent` fails.
fn strict<T: DeserializeOwned>(json: &str, parent: &str, field: &str) -> T {
    let parsed = parse(json);
    let mut v: Value = serde_json::from_str(json).unwrap();
    v.pointer_mut(parent)
        .and_then(Value::as_object_mut)
        .unwrap()
        .remove(field)
        .unwrap();
    assert!(
        serde_json::from_value::<T>(v).is_err(),
        "missing {parent}/{field} must fail"
    );
    parsed
}

#[test]
fn market_config_and_feeds() {
    let oracle: Vec<BulletOraclePrice> = strict(
        include_str!("fixtures/oracle_prices.json"),
        "/0",
        "oraclePrice",
    );
    assert_eq!(oracle[0].asset, "USDC");
    assert_eq!(oracle[0].asset_id, 0);
    assert!(oracle.iter().all(|o| o.last_update_timestamp.is_some()));

    let insurance: BulletInsuranceBalance = strict(
        include_str!("fixtures/insurance_balance.json"),
        "/assets/0",
        "marginBalance",
    );
    assert!(insurance.symbols.iter().any(|s| s == "BTC-USD"));
    assert_eq!(insurance.assets[0].asset, "USDC");

    let pools: Vec<BulletBorrowLendPool> = strict(
        include_str!("fixtures/borrow_lend_pools.json"),
        "/0/rateParams",
        "max_borrow_rate",
    );
    assert_eq!(pools[0].asset, "USDC");
    assert_eq!(pools[0].rate_params.optimal_utilization_rate, "0.5");
    assert_eq!(pools[0].interest_fee_tenth_bps, 100);

    let risk: BulletRiskEngineConfig = strict(
        include_str!("fixtures/risk_engine_config.json"),
        "/V1/pricing_engine/V1",
        "perp_pricing",
    );
    let BulletRiskEngineConfig::V1(risk) = risk;
    let BulletMarginConfig::V0(margin) = &risk.margin_config;
    assert_eq!(
        margin.perp_leverage_tables[&0].tiers["500000"].max_leverage,
        10
    );
    assert_eq!(margin.borrow_lend_risk_configs[&0].asset_weight, "1");
    let BulletPricingEngine::V1(pricing) = &risk.pricing_engine;
    assert_eq!(pricing.pricing_frequency_seconds, 60);
    assert_eq!(pricing.perp_pricing[&1].market_id, 1);
    assert!(pricing
        .rwa_pricing_config
        .values()
        .all(|c| c.reference_price.is_none()));
    let mut unknown: Value =
        serde_json::from_str(include_str!("fixtures/risk_engine_config.json")).unwrap();
    let inner = unknown.as_object_mut().unwrap().remove("V1").unwrap();
    unknown.as_object_mut().unwrap().insert("V2".into(), inner);
    assert!(serde_json::from_value::<BulletRiskEngineConfig>(unknown).is_err());

    let cmc: Vec<BulletCmcContract> = strict(
        include_str!("fixtures/cmc_contracts.json"),
        "/0",
        "USD_volume",
    );
    assert_eq!(cmc[0].ticker_id, "SOL-PERP");
    let cg: Vec<BulletCoinGeckoContract> = strict(
        include_str!("fixtures/coingecko_contracts.json"),
        "/0",
        "funding_interval_minutes",
    );
    assert_eq!(cg[0].funding_interval_minutes, 60);
    for json in [
        include_str!("fixtures/cmc_orderbook.json"),
        include_str!("fixtures/coingecko_orderbook.json"),
    ] {
        let book: BulletListingOrderBook = strict(json, "", "timestamp");
        assert_eq!(book.ticker_id, "BTC-PERP");
        assert_eq!(book.bids.len(), 5);
        assert_eq!(book.asks.len(), 5);
    }
    let _: BulletRiskEngineConfig = strict(
        include_str!("fixtures/risk_engine_config.json"),
        "/V1/pricing_engine/V1/rwa_pricing_config/20000",
        "reference_price",
    );
}

#[test]
fn account_data() {
    let account: BulletAccount = parse(include_str!("fixtures/account.json"));
    assert_eq!(account.margin_discount, "NONE");
    assert!(account.max_adverse_move.is_some());
    assert!(!account.positions.is_empty());

    let balances: Vec<BulletBalance> = parse(include_str!("fixtures/balance.json"));
    assert_eq!(balances[0].asset, "USDC");
    let positions: Vec<BulletPosition> = parse(include_str!("fixtures/position_risk.json"));
    assert!(positions.iter().any(|p| p.position_amt.starts_with('-')));

    let orders: Vec<BulletOpenOrder> = parse(include_str!("fixtures/open_orders.json"));
    assert_eq!(orders[0].order_id, 7_250_149_508);
    assert_eq!(
        orders[0].client_order_id.as_deref(),
        Some("1794300365740858276")
    );
    assert_eq!(orders[0].stop_price, None);
    assert!(orders.iter().all(|o| o.pending_tpsl.is_none()));

    let all: BulletPage<BulletHistoricalOrder> = parse(include_str!("fixtures/all_orders.json"));
    assert_eq!(all.data[0].order_type, "POST_ONLY_SLIDE");
    assert!(all.next_cursor.is_some());

    let fills: BulletPage<BulletUserTrade> = parse(include_str!("fixtures/user_trades.json"));
    assert_eq!(
        fills.data[0].client_order_id,
        Some(1_794_300_365_737_696_436)
    );
    let funding: BulletPage<BulletFundingPayment> =
        parse(include_str!("fixtures/user_funding_payments.json"));
    assert_eq!(funding.data[0].market_id, 20003);

    let fee: BulletCommissionRate = parse(include_str!("fixtures/commission_rate.json"));
    assert_eq!(fee.symbol, "BTC-USD");

    let last_page: BulletPage<BulletUserTrade> = parse(r#"{"data":[]}"#);
    assert_eq!(last_page.next_cursor, None);
}

#[test]
fn error_envelopes() {
    let e: BulletApiError = parse(include_str!("fixtures/error_submit_borsh.json"));
    assert_eq!(e.status, 400);
    assert!(e.message.contains("borsh"));
    let e: BulletApiError = parse(include_str!("fixtures/error_submit_bad_signature.json"));
    assert_eq!(e.status, 401);
    let e: BulletApiError = parse(include_str!("fixtures/error_submit_unknown_account.json"));
    assert_eq!(e.details.unwrap()["code"], "not_found");
    let e: BulletApiError = parse(include_str!("fixtures/error_account_not_found.json"));
    assert!(e.error_id.is_some());
    let e: BulletApiError = parse(include_str!("fixtures/error_not_delegate.json"));
    assert!(e.message.contains("not a delegate"));
}

#[test]
fn submit_response() {
    // Shape from the OpenAPI `SubmitTxResponse` (no order was placed to capture one).
    let r: BulletSubmitTxResponse = parse(
        r#"{"id":"0xabc","status":"processed","receipt":{"result":"successful"},"tx_number":5,
            "events":[{"key":"k","module":{"name":"exchange"},"number":1,"type":"OrderPlaced","value":{}}]}"#,
    );
    assert_eq!(r.status, BulletTxStatus::Processed);
    assert_eq!(r.receipt.unwrap().result, BulletTxResult::Successful);
    let r: BulletSubmitTxResponse = parse(r#"{"id":"0xabc","status":"submitted"}"#);
    assert!(r.events.is_none());
}

fn ws(json: &str) -> BulletWsMessage {
    BulletWsMessage::parse(json).unwrap()
}

#[test]
fn ws_messages() {
    assert!(
        matches!(ws(include_str!("fixtures/ws_status.json")), BulletWsMessage::Status(s) if s.status == "connected")
    );
    assert!(
        matches!(ws(include_str!("fixtures/ws_subscribe.json")), BulletWsMessage::Subscribe(a) if a.id == Some(1))
    );
    assert!(matches!(
        ws(include_str!("fixtures/ws_unsubscribe.json")),
        BulletWsMessage::Unsubscribe(_)
    ));
    assert!(
        matches!(ws(include_str!("fixtures/ws_pong.json")), BulletWsMessage::Pong(p) if p.id == Some(2))
    );
    match ws(include_str!("fixtures/ws_list_subscriptions.json")) {
        BulletWsMessage::ListSubscriptions(l) => {
            assert!(l.result.contains(&"markPrices".to_string()))
        }
        m => panic!("{m:?}"),
    }
    match ws(include_str!("fixtures/ws_error_topic.json")) {
        BulletWsMessage::Error(e) => {
            assert_eq!(e.error.code, -1004);
            assert_eq!(e.error.param.as_deref(), Some("bogus"));
        }
        m => panic!("{m:?}"),
    }
    match ws(include_str!("fixtures/ws_error_tx.json")) {
        BulletWsMessage::Error(e) => assert_eq!(e.error.code, -1008),
        m => panic!("{m:?}"),
    }
    match ws(include_str!("fixtures/ws_depth_snapshot.json")) {
        BulletWsMessage::Depth(d) => {
            assert_eq!(d.message_type, "s");
            assert_eq!(d.prev_update_id, 0);
            assert_eq!(d.bids.len(), 5);
        }
        m => panic!("{m:?}"),
    }
    match ws(include_str!("fixtures/ws_depth_update.json")) {
        BulletWsMessage::Depth(d) => assert_eq!(d.message_type, "u"),
        m => panic!("{m:?}"),
    }
    assert!(matches!(
        ws(include_str!("fixtures/ws_book_ticker.json")),
        BulletWsMessage::BookTicker(_)
    ));
    match ws(include_str!("fixtures/ws_mark_price.json")) {
        BulletWsMessage::MarkPrice(m) => {
            assert!(m.mark_event_number.is_some());
            assert_eq!(m.next_funding_time, None);
        }
        m => panic!("{m:?}"),
    }
    match ws(include_str!("fixtures/ws_market_status.json")) {
        BulletWsMessage::MarketStatus(s) => {
            assert_eq!(s.message_type, "s");
            assert!(s.markets.iter().all(|m| m.change.is_none()));
        }
        m => panic!("{m:?}"),
    }
    match ws(include_str!("fixtures/ws_agg_trade.json")) {
        BulletWsMessage::AggTrade(t) => assert_eq!(t.symbol, "ETH-USD"),
        m => panic!("{m:?}"),
    }
    match ws(include_str!("fixtures/ws_order_new.json")) {
        BulletWsMessage::OrderUpdate(u) => {
            assert_eq!(u.order.execution_type, "NEW");
            assert_eq!(u.order.time_in_force.as_deref(), Some("GTX"));
            assert_eq!(u.order.trade_id, None);
        }
        m => panic!("{m:?}"),
    }
    match ws(include_str!("fixtures/ws_order_trade.json")) {
        BulletWsMessage::OrderUpdate(u) => {
            assert_eq!(u.order.execution_type, "TRADE");
            assert_eq!(u.order.trade_id, Some(26_895_599));
            assert_eq!(u.order.is_maker, Some(true));
            assert_eq!(u.order.fill_type.as_deref(), Some("o"));
        }
        m => panic!("{m:?}"),
    }
    match ws(include_str!("fixtures/ws_order_canceled.json")) {
        BulletWsMessage::OrderUpdate(u) => assert_eq!(u.order.side, None),
        m => panic!("{m:?}"),
    }
}

#[test]
fn ws_documented_messages() {
    // Examples from the WebSocket spec (no live capture: no liquidation or own order occurred).
    match ws(
        r#"{"E":1706745600000000,"e":"User/PlaceOrders","id":10,"results":{"client_order_ids":[1],"order_ids":[1],"status":"processed","tx_id":"0xabc123def456"}}"#,
    ) {
        BulletWsMessage::OrderResult(r) => {
            assert_eq!(r.results.status, BulletTxStatus::Processed);
            assert_eq!(r.results.order_ids, vec![1]);
        }
        m => panic!("{m:?}"),
    }
    match ws(
        r#"{"E":1706745600000000,"error":{"code":-2010,"msg":"new order rejected: insufficient margin"},"id":12}"#,
    ) {
        BulletWsMessage::OrderError(e) => assert_eq!(e.error.code, -2010),
        m => panic!("{m:?}"),
    }
    match ws(
        r#"{"E":1706745600000000,"e":"liquidation","o":{"S":"SELL","T":1706745600000000,"X":"FILLED","ap":"49000.00","f":"IOC","l":"1.0","o":"LIMIT","oi":100001,"p":"49000.00","s":"BTC-USD","th":"0xabc123def456","ti":200001,"ua":"0xuser123"}}"#,
    ) {
        BulletWsMessage::Liquidation(l) => assert_eq!(l.order.trade_id, 200_001),
        m => panic!("{m:?}"),
    }
    assert!(BulletWsMessage::parse(r#"{"e":"somethingNew","E":1}"#).is_err());
    assert!(BulletWsMessage::parse(r#"{"e":"depthUpdate","E":1}"#).is_err());
}

#[test]
fn ws_requests() {
    let sub: Value = serde_json::from_str(&subscribe(
        Some(1),
        &[
            BulletTopic::Depth("BTC-USD", BulletDepthLevels::L5),
            BulletTopic::Depth("BTC-USD", BulletDepthLevels::Default),
            BulletTopic::AggTrade("BTC-USD"),
            BulletTopic::BookTicker("ETH-USD"),
            BulletTopic::MarkPrice("SOL-USD"),
            BulletTopic::AllMarkPrices,
            BulletTopic::Kline("BTC-USD", BulletKlineInterval::M1),
            BulletTopic::MarketStatus,
            BulletTopic::UserOrders("CuHSLVyC4U3QGKjU1vdzDi1MhQcZbyu1v4bCjcd2tNhc"),
        ],
    ))
    .unwrap();
    assert_eq!(
        sub,
        serde_json::json!({
            "id": 1,
            "method": "subscribe",
            "params": [
                "BTC-USD@depth5", "BTC-USD@depth", "BTC-USD@aggTrade", "ETH-USD@bookTicker",
                "SOL-USD@markPrice", "markPrices", "BTC-USD@kline_1m", "marketStatus",
                "CuHSLVyC4U3QGKjU1vdzDi1MhQcZbyu1v4bCjcd2tNhc@user.orders"
            ]
        })
    );
    let unsub: Value =
        serde_json::from_str(&unsubscribe(None, &[BulletTopic::AggTrade("BTC-USD")])).unwrap();
    assert_eq!(
        unsub,
        serde_json::json!({ "method": "unsubscribe", "params": ["BTC-USD@aggTrade"] })
    );
    assert_eq!(
        serde_json::from_str::<Value>(&ping(Some(4))).unwrap(),
        serde_json::json!({ "id": 4, "method": "ping" })
    );
    assert_eq!(
        serde_json::from_str::<Value>(&list_subscriptions(None)).unwrap(),
        serde_json::json!({ "method": "list_subscriptions" })
    );
}
