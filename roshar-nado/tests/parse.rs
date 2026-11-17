//! Offline parse tests against live captures in `tests/fixtures` (2026-11-15/16, mainnet).

use roshar_nado::signer::{Appendix, OrderType};
use roshar_nado::types::*;
use roshar_nado::{
    parse_archive_reply, parse_edge_control_reply, parse_execute_reply, parse_query_reply,
    NadoApiError,
};

macro_rules! fixture {
    ($name:literal) => {
        include_str!(concat!("fixtures/", $name, ".json"))
    };
}

fn query<T: serde::de::DeserializeOwned>(text: &str) -> T {
    parse_query_reply(200, text, "test").expect("query reply parses")
}

fn archive<T: serde::de::DeserializeOwned>(text: &str) -> T {
    parse_archive_reply(200, text, "test").expect("archive reply parses")
}

// ── Gateway queries ─────────────────────────────────────────────────────────────────────────

#[test]
fn status() {
    let s: String = query(fixture!("status"));
    assert_eq!(s, "active");
}

#[test]
fn contracts() {
    let c: NadoContracts = query(fixture!("contracts"));
    assert_eq!(c.chain_id.get(), 57073);
    assert_eq!(
        c.endpoint_addr,
        "0x05ec92d78ed421f3d3ada77ffde167106565974e"
    );
}

#[test]
fn symbols() {
    let s: NadoSymbols = query(fixture!("symbols"));
    let btc = &s.symbols["BTC-PERP"];
    assert_eq!(btc.product_type, NadoProductType::Perp);
    assert_eq!(btc.product_id, 2);
    assert_eq!(btc.price_increment_x18.get(), 1_000_000_000_000_000_000);
    assert!(s.symbols["KBTC"].product_type == NadoProductType::Spot);
    assert!(s
        .symbols
        .values()
        .any(|v| v.max_open_interest_x18.is_none()));
    for status in [
        NadoTradingStatus::Live,
        NadoTradingStatus::NotTradable,
        NadoTradingStatus::ReduceOnly,
        NadoTradingStatus::SoftReduceOnly,
        NadoTradingStatus::PostOnly,
    ] {
        assert!(
            s.symbols.values().any(|v| v.trading_status == status),
            "{status:?}"
        );
    }
}

#[test]
fn all_products() {
    let p: NadoAllProducts = query(fixture!("all_products"));
    assert_eq!(p.spot_products[0].product_id, 0);
    assert_eq!(
        p.spot_products[0].oracle_price_x18.get(),
        1_000_000_000_000_000_000
    );
    assert!(!p.perp_products.is_empty());
    assert_eq!(p.perp_products[0].product_id, 2);
}

#[test]
fn market_liquidity() {
    let b: NadoMarketLiquidity = query(fixture!("market_liquidity"));
    assert_eq!(b.product_id, 2);
    assert_eq!(b.bids.len(), 5);
    assert_eq!(b.asks.len(), 5);
    assert!(b.bids[0][0] > b.bids[1][0]);
    assert!(b.asks[0][0] < b.asks[1][0]);
    assert!(b.bids[0][0] < b.asks[0][0]);
    assert!(b.timestamp.get() > 1_700_000_000_000_000_000);
}

#[test]
fn market_prices() {
    let p: NadoMarketPrice = query(fixture!("market_price"));
    assert_eq!(p.product_id, 2);
    assert!(p.bid_x18 < p.ask_x18);
    let ps: NadoMarketPrices = query(fixture!("market_prices"));
    assert_eq!(
        ps.market_prices
            .iter()
            .map(|p| p.product_id)
            .collect::<Vec<_>>(),
        [1, 2]
    );
}

#[test]
fn nonces() {
    let n: NadoNonces = query(fixture!("nonces"));
    assert_eq!(n.tx_nonce.get(), 3);
    assert!(n.order_nonce.get() >> 20 > 1_700_000_000_000);
}

#[test]
fn open_orders() {
    let o: NadoSubaccountOrders = query(fixture!("subaccount_orders"));
    assert_eq!(o.product_id, 2);
    let first = &o.orders[0];
    assert_eq!(first.order_type, NadoOrderType::PostOnly);
    assert_eq!(first.appendix.get(), 1537);
    assert!(first.id.is_some());
    let a = Appendix::decode(first.appendix.get()).unwrap();
    assert_eq!(a.order_type, OrderType::PostOnly);
    assert!(!a.reduce_only);

    let m: NadoMultiProductOrders = query(fixture!("orders"));
    assert_eq!(m.product_orders.len(), 2);
    assert_eq!(m.product_orders[0].product_id, 2);
}

#[test]
fn subaccount_info() {
    let i: NadoSubaccountInfo = query(fixture!("subaccount_info"));
    assert!(i.exists);
    assert_eq!(i.healths.len(), 3);
    assert_eq!(i.spot_balances[0].product_id, 0);
    let perp = &i.perp_balances[0];
    assert_eq!(perp.product_id, 2);
    assert!(perp.balance.amount.get() != 0);
    assert!(!i.spot_products.is_empty() && !i.perp_products.is_empty());
}

#[test]
fn fee_rates_linked_signer_max_size() {
    let f: NadoFeeRates = query(fixture!("fee_rates"));
    assert_eq!(f.taker_fee_rates_x18[0].get(), 0);
    assert_eq!(f.fee_tier, 6);
    let l: NadoLinkedSigner = query(fixture!("linked_signer"));
    assert_eq!(l.linked_signer.len(), 42);
    let m: NadoMaxOrderSize = query(fixture!("max_order_size"));
    assert!(m.max_order_size.get() > 0);
}

#[test]
fn query_failure_surfaces_code() {
    let err = parse_query_reply::<NadoOrder>(200, fixture!("order_missing"), "order").unwrap_err();
    let e = err.downcast_ref::<NadoApiError>().expect("NadoApiError");
    assert_eq!(e.error_code, Some(2020));
    assert_eq!(e.request, "query_order");
    assert!(e.error.contains("could not be found"));
}

#[test]
fn query_plain_text_error() {
    let text = "Failed to deserialize query string: missing field `depth`";
    let err = parse_query_reply::<NadoMarketLiquidity>(400, text, "market_liquidity").unwrap_err();
    let e = err.downcast_ref::<NadoApiError>().expect("NadoApiError");
    assert_eq!(e.http_status, 400);
    assert_eq!(e.error_code, None);
    assert_eq!(e.error, text);
}

// ── Executes ────────────────────────────────────────────────────────────────────────────────

#[test]
fn execute_failures_surface_code() {
    let err =
        parse_execute_reply::<NadoPlacedOrder>(200, fixture!("execute_bad_sig"), "place_order")
            .unwrap_err();
    let e = err.downcast_ref::<NadoApiError>().expect("NadoApiError");
    assert_eq!(e.error_code, Some(2024));
    assert_eq!(e.request, "execute_place_order");

    let err = parse_execute_reply::<NadoCancelledOrders>(
        200,
        fixture!("execute_recv_time_error"),
        "cancel_orders",
    )
    .unwrap_err();
    let e = err.downcast_ref::<NadoApiError>().expect("NadoApiError");
    assert_eq!(e.error_code, Some(2012));
    assert_eq!(e.request, "execute_cancel_orders");
}

#[test]
fn execute_success_shapes() {
    // Documented success bodies (no live order was placed).
    let placed = r#"{"status":"success","signature":"0xab","data":{"digest":"0x01"},"request_type":"execute_place_order","id":100}"#;
    let r = parse_execute_reply::<NadoPlacedOrder>(200, placed, "place_order").unwrap();
    assert_eq!(r.data.unwrap().digest, "0x01");
    assert_eq!(r.id, Some(100));

    let linked = r#"{"status":"success","signature":"0xab","request_type":"execute_link_signer"}"#;
    let r = parse_execute_reply::<serde_json::Value>(200, linked, "link_signer").unwrap();
    assert!(r.data.is_none());
}

// ── Archive ─────────────────────────────────────────────────────────────────────────────────

#[test]
fn candlesticks() {
    let c: NadoCandlesticks = archive(fixture!("candlesticks"));
    assert_eq!(c.candlesticks.len(), 2);
    assert!(c.candlesticks[0].timestamp > c.candlesticks[1].timestamp);
    assert_eq!(c.candlesticks[0].granularity, 60);
}

#[test]
fn funding() {
    let f: NadoFundingRate = archive(fixture!("funding_rate"));
    assert_eq!(f.product_id, 2);
    assert!(f.premium_x18.is_some());
    let fs: NadoFundingRates = archive(fixture!("funding_rates"));
    assert_eq!(fs["4"].product_id, 4);
    let h: NadoFundingRateHistory = archive(fixture!("funding_rate_history"));
    assert_eq!(h.funding_rates.len(), 3);
    assert!(h.funding_rates[0].timestamp < h.funding_rates[1].timestamp);
}

#[test]
fn funding_rate_without_premium() {
    let f: NadoFundingRate =
        archive(r#"{"product_id":44,"funding_rate_x18":"-5","update_time":"1794780088"}"#);
    assert!(f.premium_x18.is_none());
}

#[test]
fn prices() {
    let p: NadoPerpPrices = archive(fixture!("price"));
    assert_eq!(p.product_id, 2);
    let ps: NadoPerpPricesMap = archive(fixture!("perp_prices"));
    assert_eq!(ps.len(), 2);
    let o: NadoOraclePrices = archive(fixture!("oracle_price"));
    assert_eq!(o.prices.len(), 2);
}

#[test]
fn matches() {
    let m: NadoMatches = archive(fixture!("archive_matches"));
    assert_eq!(m.matches.len(), 3);
    assert_eq!(m.matches[0].submission_idx, m.txs[0].submission_idx);
    let first = &m.matches[0];
    assert!(first.margin.is_none());
    assert!(matches!(
        first.pre_balance.base,
        NadoProductBalance::Perp { product_id: 2, .. }
    ));
    assert!(first.pre_balance.quote.is_none());
    assert!(m.txs[0].tx.get("match_orders").is_some());

    let spot: NadoMatches = archive(fixture!("archive_matches_spot"));
    assert!(matches!(
        spot.matches[0].pre_balance.base,
        NadoProductBalance::Spot { product_id: 1, .. }
    ));
}

#[test]
fn archive_orders_and_payments() {
    let o: NadoArchiveOrders = archive(fixture!("archive_orders"));
    assert_eq!(o.orders.len(), 3);
    assert!(o.orders[0].closed_margin.is_none());
    let p: NadoInterestAndFunding = archive(fixture!("interest_and_funding"));
    assert_eq!(p.interest_payments[0].product_id, 0);
    assert_eq!(p.funding_payments[0].product_id, 2);
    assert!(p.next_idx.is_some());
}

#[test]
fn archive_error_surfaces_code() {
    let body = r#"{"error":"The provided 'product_ids' is invalid. Please verify input contains only valid products and no duplicates.","error_code":2060}"#;
    let err = parse_archive_reply::<NadoFundingRate>(400, body, "funding_rate").unwrap_err();
    let e = err.downcast_ref::<NadoApiError>().expect("NadoApiError");
    assert_eq!(e.error_code, Some(2060));
    assert_eq!(e.http_status, 400);
}

// ── V2 REST ─────────────────────────────────────────────────────────────────────────────────

#[test]
fn v2_gateway() {
    let a: Vec<NadoAsset> = archive(fixture!("v2_assets"));
    assert_eq!(a[0].product_id, 0);
    assert!(a[0].ticker_id.is_none() && a[0].maker_fee.is_none());
    assert!(a[1].ticker_id.is_some() && a[1].taker_fee.is_some());
    let p: Vec<NadoPair> = archive(fixture!("v2_pairs"));
    assert_eq!(p[0].ticker_id, "BTC-PERP_USDT0");
    let apr: Vec<NadoApr> = archive(fixture!("v2_apr"));
    assert_eq!(apr[0].symbol, "USDT0");
    let ob: NadoOrderbook = archive(fixture!("v2_orderbook"));
    assert_eq!(ob.product_id, 2);
    assert!(ob.bids[0][0] < ob.asks[0][0]);
}

#[test]
fn v2_archive() {
    let t: NadoTickers = archive(fixture!("v2_tickers"));
    assert_eq!(t.len(), 3);
    let c: NadoContractsSummary = archive(fixture!("v2_contracts"));
    assert!(c.values().all(|c| c.product_type == "perpetual"));
    let tr: Vec<NadoTrade> = archive(fixture!("v2_trades"));
    assert!(tr.iter().any(|t| t.trade_type == NadoTradeSide::Buy));
    assert!(tr.iter().any(|t| t.trade_type == NadoTradeSide::Sell));
}

#[test]
fn v2_symbols() {
    let s: NadoV2Symbols = archive(fixture!("v2_symbols"));
    assert!(s.values().any(|v| v.market_hours.is_some()));
    assert!(s.values().any(|v| v.exchange_rate_x18.is_some()));
    assert!(s.values().any(|v| v.boost_type.is_some()));
    assert!(s.values().any(|v| v.symbol.max_open_interest_x18.is_none()));
    assert!(s
        .values()
        .any(|v| v.price_feed.as_ref().unwrap().spot_index.is_none()));
    assert!(s
        .values()
        .any(|v| v.price_feed.as_ref().unwrap().oracle.weekend.is_some()));
}

// ── Subscriptions websocket ─────────────────────────────────────────────────────────────────

fn event(text: &str) -> NadoEvent {
    match NadoWsMessage::parse(text).expect("ws frame parses") {
        NadoWsMessage::Event(e) => e,
        other => panic!("expected event, got {other:?}"),
    }
}

fn response(text: &str) -> NadoWsResponse {
    match NadoWsMessage::parse(text).expect("ws frame parses") {
        NadoWsMessage::Response(r) => r,
        other => panic!("expected response, got {other:?}"),
    }
}

#[test]
fn ws_events() {
    assert!(
        matches!(event(fixture!("ws_book_depth")), NadoEvent::BookDepth(e) if e.product_id == 2)
    );
    assert!(
        matches!(event(fixture!("ws_best_bid_offer")), NadoEvent::BestBidOffer(e) if e.bid_price < e.ask_price)
    );
    assert!(
        matches!(event(fixture!("ws_trade")), NadoEvent::Trade(e) if e.taker_qty == e.maker_qty)
    );
    assert!(
        matches!(event(fixture!("ws_latest_candlestick")), NadoEvent::LatestCandlestick(e) if e.granularity == 60)
    );
    assert!(
        matches!(event(fixture!("ws_funding_rate")), NadoEvent::FundingRate(e) if e.product_id == 2)
    );
    assert!(
        matches!(event(fixture!("ws_fill")), NadoEvent::Fill(e) if e.id.is_some() && e.submission_idx.get() > 0)
    );
    assert!(
        matches!(event(fixture!("ws_position_change")), NadoEvent::PositionChange(e) if e.reason == "match_orders")
    );
    match event(fixture!("ws_all_bbo")) {
        NadoEvent::AllBbo(e) => {
            // No asks: the ask is i128::MAX.
            assert_eq!(e.bbos["44"].ask.get(), i128::MAX);
            assert_eq!(e.bbos["44"].bid.get(), 0);
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn ws_documented_events() {
    // From the Events page; not observed live in the capture window.
    let order_update = r#"{"type":"order_update","timestamp":"1695081920633151000","product_id":1,"digest":"0xf7","amount":"82000000000000000","reason":"filled","filled_qty":"18000000000000000","filled_price":"25000000000000000000000","id":100}"#;
    assert!(
        matches!(event(order_update), NadoEvent::OrderUpdate(e) if e.reason == NadoOrderUpdateReason::Filled && e.filled_qty.is_some())
    );
    let placed = r#"{"type":"order_update","timestamp":"1","product_id":1,"digest":"0xf7","amount":"8","reason":"placed"}"#;
    assert!(
        matches!(event(placed), NadoEvent::OrderUpdate(e) if e.filled_qty.is_none() && e.id.is_none())
    );
    let liq = r#"{"type":"liquidation","timestamp":"1","product_ids":[1],"liquidator":"0x1","liquidatee":"0x2","amount":"-1","price":"2"}"#;
    assert!(matches!(event(liq), NadoEvent::Liquidation(_)));
    let fp = r#"{"type":"funding_payment","timestamp":"1","product_id":2,"payment_amount":"-5","open_interest":"5","cumulative_funding_long_x18":"1","cumulative_funding_short_x18":"-1","dt":"3600000000000"}"#;
    assert!(matches!(event(fp), NadoEvent::FundingPayment(_)));
}

#[test]
fn ws_responses() {
    let ok = response(fixture!("ws_subscribe_ok"));
    assert!(ok.result.is_null() && ok.error.is_none());
    assert_eq!(ok.id, Some(1));
    let unauth = response(fixture!("ws_subscribe_error"));
    assert!(unauth.error.unwrap().contains("authentication"));
    let bad_sig = response(fixture!("ws_auth_error"));
    assert_eq!(bad_sig.error.as_deref(), Some("Invalid signature v"));
    assert!(response(fixture!("ws_list")).result.is_array());
    assert_eq!(response(fixture!("ws_pong")).result["method"], "pong");
}

#[test]
fn ws_request_frames() {
    let v = |s: String| serde_json::from_str::<serde_json::Value>(&s).unwrap();
    assert_eq!(
        v(NadoStream::Trade { product_id: 2 }.subscribe_message(Some(10))),
        serde_json::json!({"method":"subscribe","stream":{"type":"trade","product_id":2},"id":10})
    );
    assert_eq!(
        v(NadoStream::FundingRate { product_id: None }.subscribe_message(None)),
        serde_json::json!({"method":"subscribe","stream":{"type":"funding_rate","product_id":null}})
    );
    assert_eq!(
        v(NadoStream::LatestCandlestick {
            product_id: 1,
            granularity: 60
        }
        .unsubscribe_message(Some(3))),
        serde_json::json!({"method":"unsubscribe","stream":{"type":"latest_candlestick","product_id":1,"granularity":60},"id":3})
    );
    assert_eq!(
        v(NadoStream::AllBbo.subscribe_message(Some(1))),
        serde_json::json!({"method":"subscribe","stream":{"type":"all_bbo"},"id":1})
    );
    let sub = "0x7a5ec2748e9065794491a8d29dcf3f9edb8d7c43746573743000000000000000".to_string();
    assert_eq!(
        v(NadoStream::OrderUpdate {
            subaccount: sub.clone(),
            product_id: Some(1)
        }
        .subscribe_message(Some(10))),
        serde_json::json!({"method":"subscribe","stream":{"type":"order_update","subaccount":sub,"product_id":1},"id":10})
    );
    assert_eq!(
        v(ping_message(Some(10), Some(1780000000000))),
        serde_json::json!({"method":"ping","id":10,"client_time":"1780000000000"})
    );
    assert_eq!(
        v(list_message(Some(10))),
        serde_json::json!({"method":"list","id":10})
    );
    assert_eq!(v(time_message(None)), serde_json::json!({"method":"time"}));
}

#[test]
fn strict_numbers() {
    // x18 values must be strings holding integers.
    assert!(serde_json::from_str::<NadoMarketPrice>(
        r#"{"product_id":1,"bid_x18":1,"ask_x18":"2"}"#
    )
    .is_err());
    assert!(serde_json::from_str::<NadoMarketPrice>(
        r#"{"product_id":1,"bid_x18":"1.5","ask_x18":"2"}"#
    )
    .is_err());
    assert!(serde_json::from_str::<NadoMarketPrice>(r#"{"product_id":1,"ask_x18":"2"}"#).is_err());
}

// ── Gateway: edge_all_products, health_groups, insurance (2026-11-16) ──────────────────────

#[test]
fn edge_all_products_health_insurance() {
    let e: NadoEdgeAllProducts = query(fixture!("edge_all_products"));
    let mainnet = &e.edge_all_products["57073"];
    assert_eq!(mainnet.spot_products[0].product_id, 0);
    assert!(!mainnet.perp_products.is_empty());
    let h: NadoHealthGroups = query(fixture!("health_groups"));
    assert_eq!(h.health_groups[0], [1, 2]);
    let i: NadoInsurance = query(fixture!("insurance"));
    assert!(i.insurance.get() > 0);
}

#[test]
fn edge_cached_queries() {
    let p: NadoMarketPrices = query(fixture!("edge_cached_prices"));
    assert_eq!(p.market_prices.len(), 3);
    let s: NadoSymbols = query(fixture!("edge_cached_symbols"));
    assert!(s.symbols.contains_key("BTC-PERP"));
    let c: NadoContracts = query(fixture!("edge_cached_contracts"));
    assert_eq!(c.chain_id.get(), 57073);
    let st: String = query(fixture!("edge_cached_status"));
    assert_eq!(st, "active");
    let ap: NadoAllProducts = query(fixture!("edge_cached_all_products"));
    assert!(!ap.perp_products.is_empty());
    let eap: NadoEdgeAllProducts = query(fixture!("edge_cached_edge_all_products"));
    assert!(eap.edge_all_products.contains_key("57073"));
    let hg: NadoHealthGroups = query(fixture!("edge_cached_health_groups"));
    assert!(!hg.health_groups.is_empty());
    let b: NadoBboHistory = query(fixture!("edge_bbo_history"));
    assert_eq!((b.interval_ms, b.limit), (5000, 3));
    assert!(b.history.iter().all(|s| s.product_id == 2 && s.bid < s.ask));
}

#[test]
fn edge_query_failure_surfaces_message() {
    let err = parse_query_reply::<NadoBboHistory>(200, fixture!("edge_bbo_history_error"), "x")
        .unwrap_err();
    let e = err.downcast_ref::<NadoApiError>().expect("NadoApiError");
    assert_eq!(e.error, "interval_ms must be at least 500");
}

#[test]
fn edge_control_replies() {
    let p = parse_edge_control_reply(200, fixture!("edge_ping"), "ping").unwrap();
    assert_eq!(p.method, "pong");
    assert_eq!(p.id, Some(1));
    assert_eq!(p.client_time.map(|t| t.get()), Some(1780000000000));
    let t = parse_edge_control_reply(200, fixture!("edge_time"), "time").unwrap();
    assert_eq!(
        (t.method.as_str(), t.id, t.client_time),
        ("time", Some(2), None)
    );
    assert!(t.server_time.get() > 0);
    let fail = r#"{"status":"failure","error":"rate limited"}"#;
    let err = parse_edge_control_reply(200, fail, "time").unwrap_err();
    assert_eq!(
        err.downcast_ref::<NadoApiError>().unwrap().error,
        "rate limited"
    );
    // server_time is required.
    assert!(
        parse_edge_control_reply(200, r#"{"status":"success","method":"time"}"#, "time").is_err()
    );
}

#[test]
fn new_gateway_types_are_strict() {
    assert!(serde_json::from_str::<NadoInsurance>(r#"{}"#).is_err());
    assert!(serde_json::from_str::<NadoHealthGroups>(r#"{"health_groups":[[1]]}"#).is_err());
    assert!(
        serde_json::from_str::<NadoBboSample>(r#"{"product_id":2,"timestamp":"1","bid":"1"}"#)
            .is_err()
    );
    assert!(
        serde_json::from_str::<NadoEdgeAllProducts>(r#"{"edge_all_products":{"1":{}}}"#).is_err()
    );
}

// ── Archive: edge candles, product / oracle / market snapshots, feeds (2026-11-16) ─────────

#[test]
fn edge_candlesticks() {
    let c: NadoCandlesticks = archive(fixture!("edge_candlesticks"));
    assert_eq!(c.candlesticks.len(), 2);
    assert!(c.candlesticks[0].timestamp > c.candlesticks[1].timestamp);
}

#[test]
fn product_snapshots() {
    let h: NadoProductHistory = archive(fixture!("product_history"));
    assert_eq!(h.products[0].submission_idx, h.txs[0].submission_idx);
    assert!(matches!(&h.products[0].product, NadoProductState::Perp(p) if p.product_id == 2));
    assert!(h.txs[0].tx.get("update_price").is_some());
    let s: NadoProductSnapshots = archive(fixture!("product_snapshots"));
    assert!(matches!(s["1"].product, NadoProductState::Spot(_)));
    assert!(matches!(s["2"].product, NadoProductState::Perp(_)));
}

#[test]
fn matches_and_liquidations() {
    let m: NadoMatches = archive(fixture!("matches_and_liquidations"));
    assert!(m.txs[0].tx.get("liquidate_subaccount").is_some());
    assert!(m
        .matches
        .iter()
        .all(|e| e.submission_idx == m.txs[0].submission_idx && e.order.amount.get() == 0));
}

#[test]
fn market_and_oracle_snapshots() {
    let o: NadoOracleSnapshots = archive(fixture!("oracle_snapshots"));
    assert!(o.snapshots[0].oracle_prices.contains_key("2"));
    let m: NadoMarketSnapshots = archive(fixture!("market_snapshots"));
    let s = &m.snapshots[0];
    assert!(s.open_interests.contains_key("2") && !s.open_interests.contains_key("1"));
    assert!(s.funding_rates.contains_key("2"));
    let e: NadoEdgeMarketSnapshots = archive(fixture!("edge_market_snapshots"));
    assert_eq!(e.snapshots["57073"].len(), 1);
}

#[test]
fn archive_feeds() {
    let l: Vec<NadoLiquidatable> = archive(fixture!("liquidation_feed"));
    assert_eq!(l[0].subaccount.len(), 66);
    let q: NadoQuotePrice = archive(fixture!("quote_price"));
    assert!(q.price_x18.get() > 0);
    let b: NadoBacklog = archive(fixture!("backlog"));
    assert_eq!(b.backlog_eta_in_seconds, Some(88.0));
    assert_eq!(b.txs_per_second, Some(6.17));
    let null = r#"{"total_txs":"1","total_submissions":"1","backlog_size":"0","updated_at":"1","backlog_eta_in_seconds":null,"txs_per_second":null}"#;
    let b: NadoBacklog = serde_json::from_str(null).unwrap();
    assert!(b.txs_per_second.is_none());
}

#[test]
fn new_archive_types_are_strict() {
    // Nullable but required.
    assert!(serde_json::from_str::<NadoBacklog>(
        r#"{"total_txs":"1","total_submissions":"1","backlog_size":"0","updated_at":"1","backlog_eta_in_seconds":null}"#
    )
    .is_err());
    assert!(serde_json::from_str::<NadoQuotePrice>(r#"{"price_x18":0.99}"#).is_err());
    assert!(serde_json::from_str::<NadoLiquidatable>(r#"{"subaccount":"0x1"}"#).is_err());
    assert!(serde_json::from_str::<NadoOracleSnapshot>(r#"{"oracle_prices":{}}"#).is_err());
    let mut snap: serde_json::Value = serde_json::from_str(fixture!("market_snapshots")).unwrap();
    snap["snapshots"][0].as_object_mut().unwrap().remove("tvl");
    assert!(serde_json::from_value::<NadoMarketSnapshots>(snap).is_err());
    assert!(serde_json::from_str::<NadoProductSnapshot>(
        r#"{"product_id":1,"submission_idx":"1","product":{"future":{}}}"#
    )
    .is_err());
}

// ── subscribe_multi / unsubscribe_multi ─────────────────────────────────────────────────────

#[test]
fn ws_multi_frames() {
    let v = |s: String| serde_json::from_str::<serde_json::Value>(&s).unwrap();
    let streams = [
        NadoStream::Trade { product_id: 2 },
        NadoStream::BestBidOffer { product_id: 2 },
    ];
    assert_eq!(
        v(subscribe_multi_message(&streams, Some(1))),
        serde_json::json!({"method":"subscribe_multi","streams":[{"type":"trade","product_id":2},{"type":"best_bid_offer","product_id":2}],"id":1})
    );
    assert_eq!(
        v(unsubscribe_multi_message(&streams, None)),
        serde_json::json!({"method":"unsubscribe_multi","streams":[{"type":"trade","product_id":2},{"type":"best_bid_offer","product_id":2}]})
    );
    let r = response(fixture!("ws_multi_not_enabled"));
    assert!(r
        .error
        .unwrap()
        .starts_with("multi subscriptions not enabled"));
}
