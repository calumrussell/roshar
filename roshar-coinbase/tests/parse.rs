//! Offline parse tests over live captures in `tests/fixtures/` (2026-11-16).

use roshar_coinbase::types::*;
use serde_json::Value;

fn fixture(name: &str) -> String {
    std::fs::read_to_string(format!(
        "{}/tests/fixtures/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap_or_else(|e| panic!("read {name}: {e}"))
}

/// Remove the field at `path` (object keys / array indexes) from the JSON fixture.
fn without(name: &str, path: &[&str]) -> String {
    let mut v: Value = serde_json::from_str(&fixture(name)).unwrap();
    let (last, parents) = path.split_last().unwrap();
    let mut cur = &mut v;
    for p in parents {
        cur = match p.parse::<usize>() {
            Ok(i) => &mut cur[i],
            Err(_) => &mut cur[*p],
        };
    }
    assert!(
        cur.as_object_mut().unwrap().remove(*last).is_some(),
        "{name}: no field {path:?}"
    );
    v.to_string()
}

// ---------------------------------------------------------------- REST

#[test]
fn products_spot() {
    let r: CoinbaseProductsResponse = serde_json::from_str(&fixture("products_spot.json")).unwrap();
    assert_eq!(r.products.len(), 2);
    let p = &r.products[0];
    assert_eq!(p.product_type.as_deref(), Some("SPOT"));
    assert!(p.future_product_details.is_none());
    assert!(p.equity_product_details.is_none());
    assert!(p.fcm_trading_session_details.is_none());
    let pg = r.pagination.unwrap();
    assert!(!pg.has_next);
}

#[test]
fn products_by_ids_has_null_pagination() {
    let r: CoinbaseProductsResponse =
        serde_json::from_str(&fixture("products_by_ids.json")).unwrap();
    assert_eq!(r.products[0].product_id, "BTC-USD");
    assert!(r.pagination.is_none());
}

#[test]
fn products_future() {
    let r: CoinbaseProductsResponse =
        serde_json::from_str(&fixture("products_future.json")).unwrap();
    // FCM dated/nano perp, INTX perpetual, expired FCM without session details.
    assert_eq!(r.products.len(), 3);
    let fcm = &r.products[0];
    let fd = fcm.future_product_details.as_ref().unwrap();
    assert!(fd.contract_expiry.is_some());
    assert!(fd.intraday_margin_rate.is_some());
    assert!(fcm.fcm_trading_session_details.is_some());

    let intx = &r.products[1];
    assert_eq!(intx.product_venue.as_deref(), Some("INTX"));
    let fd = intx.future_product_details.as_ref().unwrap();
    assert_eq!(fd.contract_expiry_type, "PERPETUAL");
    assert!(fd.contract_expiry.is_none());
    assert!(fd.intraday_margin_rate.is_none());
    assert!(!fd.perpetual_details.max_leverage.is_empty());
    let s = intx.fcm_trading_session_details.as_ref().unwrap();
    assert!(s.open_time.is_none());

    assert!(r.products[2].fcm_trading_session_details.is_none());
}

#[test]
fn products_equity() {
    let r: CoinbaseProductsResponse =
        serde_json::from_str(&fixture("products_equity.json")).unwrap();
    let p = &r.products[0];
    assert_eq!(p.product_venue.as_deref(), Some("CCM"));
    let e = p.equity_product_details.as_ref().unwrap();
    assert!(!e.ticker.is_empty());
    assert!(!e.trading_day_info.trading_sessions.is_empty());
    assert_eq!(e.recent_trading_days.trading_days.len(), 1);
    assert!(e.trading_halted_start_time.is_none());
    let halted = r.products[1].equity_product_details.as_ref().unwrap();
    assert!(halted.trading_halted_start_time.is_some());
    assert!(r.pagination.unwrap().has_next);
}

#[test]
fn product_single() {
    let p: CoinbaseProduct = serde_json::from_str(&fixture("product_btc_usd.json")).unwrap();
    assert_eq!(p.product_id, "BTC-USD");
}

#[test]
fn product_strict() {
    let s = without("product_btc_usd.json", &["watched"]);
    assert!(serde_json::from_str::<CoinbaseProduct>(&s).is_err());
    let s = without(
        "products_future.json",
        &["products", "0", "future_product_details", "contract_size"],
    );
    assert!(serde_json::from_str::<CoinbaseProductsResponse>(&s).is_err());
    let s = without(
        "products_future.json",
        &[
            "products",
            "0",
            "fcm_trading_session_details",
            "session_state",
        ],
    );
    assert!(serde_json::from_str::<CoinbaseProductsResponse>(&s).is_err());
    let s = without(
        "products_equity.json",
        &[
            "products",
            "0",
            "equity_product_details",
            "equity_trading_flags",
        ],
    );
    assert!(serde_json::from_str::<CoinbaseProductsResponse>(&s).is_err());
    let s = without("products_spot.json", &["pagination", "next_cursor"]);
    assert!(serde_json::from_str::<CoinbaseProductsResponse>(&s).is_err());
    // Nullable fields are always sent: a missing key is an error, `null` is accepted.
    let s = without("product_btc_usd.json", &["new_at"]);
    assert!(serde_json::from_str::<CoinbaseProduct>(&s).is_err());
    let s = without("product_btc_usd.json", &["fcm_trading_session_details"]);
    assert!(serde_json::from_str::<CoinbaseProduct>(&s).is_err());
    let s = without("products_spot.json", &["pagination"]);
    assert!(serde_json::from_str::<CoinbaseProductsResponse>(&s).is_err());
    let s = without(
        "products_future.json",
        &["products", "1", "future_product_details", "contract_expiry"],
    );
    assert!(serde_json::from_str::<CoinbaseProductsResponse>(&s).is_err());
    let s = without(
        "products_equity.json",
        &[
            "products",
            "0",
            "equity_product_details",
            "trading_halted_start_time",
        ],
    );
    assert!(serde_json::from_str::<CoinbaseProductsResponse>(&s).is_err());
    // Detail objects are genuinely absent on other product types.
    let s = without(
        "products_future.json",
        &["products", "0", "future_product_details"],
    );
    let r: CoinbaseProductsResponse = serde_json::from_str(&s).unwrap();
    assert!(r.products[0].future_product_details.is_none());
}

#[test]
fn products_query_pairs() {
    assert!(CoinbaseProductsQuery::default().to_query().is_empty());
    let q = CoinbaseProductsQuery {
        limit: Some(2),
        product_type: Some(CoinbaseProductType::Future),
        product_ids: Some(vec!["A".into(), "B".into()]),
        contract_expiry_type: Some(CoinbaseContractExpiryType::Perpetual),
        cursor: Some("abc=".into()),
        ..Default::default()
    };
    assert_eq!(
        q.to_query(),
        vec![
            ("limit", "2".to_string()),
            ("product_type", "FUTURE".to_string()),
            ("product_ids", "A".to_string()),
            ("product_ids", "B".to_string()),
            ("contract_expiry_type", "PERPETUAL".to_string()),
            ("cursor", "abc=".to_string()),
        ]
    );
}

#[test]
fn product_book() {
    let r: CoinbaseProductBookResponse =
        serde_json::from_str(&fixture("product_book.json")).unwrap();
    assert_eq!(r.pricebook.bids.len(), 3);
    assert_eq!(r.pricebook.asks.len(), 3);
    let s = without("product_book.json", &["pricebook", "bids"]);
    assert!(serde_json::from_str::<CoinbaseProductBookResponse>(&s).is_err());
    let s = without("product_book.json", &["spread_bps"]);
    assert!(serde_json::from_str::<CoinbaseProductBookResponse>(&s).is_err());
}

#[test]
fn market_trades() {
    let r: CoinbaseMarketTradesResponse =
        serde_json::from_str(&fixture("market_trades.json")).unwrap();
    assert_eq!(r.trades.len(), 2);
    assert_eq!(r.trades[0].side, CoinbaseTradeSide::Buy);
    let s = without("market_trades.json", &["trades", "0", "side"]);
    assert!(serde_json::from_str::<CoinbaseMarketTradesResponse>(&s).is_err());
}

#[test]
fn candles() {
    let r: CoinbaseCandlesResponse = serde_json::from_str(&fixture("candles.json")).unwrap();
    assert_eq!(r.candles.len(), 2);
    let s = without("candles.json", &["candles", "0", "volume"]);
    assert!(serde_json::from_str::<CoinbaseCandlesResponse>(&s).is_err());
}

#[test]
fn server_time() {
    let r: CoinbaseServerTime = serde_json::from_str(&fixture("time.json")).unwrap();
    assert!(r.epoch_millis.starts_with(&r.epoch_seconds));
    let s = without("time.json", &["epochMillis"]);
    assert!(serde_json::from_str::<CoinbaseServerTime>(&s).is_err());
}

#[test]
fn error_response() {
    let r: CoinbaseErrorResponse = serde_json::from_str(&fixture("error_not_found.json")).unwrap();
    assert_eq!(r.error, "NOT_FOUND");
    assert_eq!(r.message, "Product NOPE-USD not supported");
}

// ---------------------------------------------------------------- WebSocket

fn ws(name: &str) -> CoinbaseWsChannelMessage {
    match CoinbaseWsMessage::parse(&fixture(name)).unwrap() {
        CoinbaseWsMessage::Channel(m) => m,
        other => panic!("{name}: {other:?}"),
    }
}

#[test]
fn ws_l2() {
    let CoinbaseWsChannelMessage::L2Data(m) = ws("ws_l2_snapshot.json") else {
        panic!()
    };
    let e = &m.events[0];
    assert_eq!(e.event_type, CoinbaseWsEventType::Snapshot);
    assert_eq!(e.updates[0].side, CoinbaseWsL2Side::Bid);
    assert_eq!(e.updates[3].side, CoinbaseWsL2Side::Offer);
    let CoinbaseWsChannelMessage::L2Data(m) = ws("ws_l2_update.json") else {
        panic!()
    };
    assert_eq!(m.events[0].event_type, CoinbaseWsEventType::Update);
}

#[test]
fn ws_market_trades() {
    for f in [
        "ws_market_trades_snapshot.json",
        "ws_market_trades_update.json",
    ] {
        let CoinbaseWsChannelMessage::MarketTrades(m) = ws(f) else {
            panic!()
        };
        assert!(!m.events[0].trades.is_empty());
    }
}

#[test]
fn ws_ticker() {
    for f in ["ws_ticker_snapshot.json", "ws_ticker_update.json"] {
        let CoinbaseWsChannelMessage::Ticker(m) = ws(f) else {
            panic!()
        };
        assert!(!m.events[0].tickers[0].best_bid.is_empty());
    }
    for f in [
        "ws_ticker_batch_snapshot.json",
        "ws_ticker_batch_update.json",
    ] {
        let CoinbaseWsChannelMessage::TickerBatch(m) = ws(f) else {
            panic!()
        };
        assert!(!m.events[0].tickers[0].price.is_empty());
    }
}

#[test]
fn ws_candles_status_heartbeats_subscriptions() {
    for f in ["ws_candles_snapshot.json", "ws_candles_update.json"] {
        let CoinbaseWsChannelMessage::Candles(m) = ws(f) else {
            panic!()
        };
        assert!(!m.events[0].candles.is_empty());
    }
    let CoinbaseWsChannelMessage::Status(m) = ws("ws_status_snapshot.json") else {
        panic!()
    };
    assert_eq!(m.events[0].products[0].status, "online");
    let CoinbaseWsChannelMessage::Heartbeats(m) = ws("ws_heartbeats.json") else {
        panic!()
    };
    assert!(m.events[0].heartbeat_counter > 0);
    let CoinbaseWsChannelMessage::Subscriptions(m) = ws("ws_subscriptions.json") else {
        panic!()
    };
    assert_eq!(
        m.events[0].subscriptions["ticker"],
        vec!["BTC-USD".to_string()]
    );
}

#[test]
fn ws_error() {
    match CoinbaseWsMessage::parse(&fixture("ws_error.json")).unwrap() {
        CoinbaseWsMessage::Error(e) => assert_eq!(e.message, "authentication failure"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn ws_strict() {
    let cases: &[(&str, &[&str])] = &[
        (
            "ws_l2_update.json",
            &["events", "0", "updates", "0", "new_quantity"],
        ),
        ("ws_l2_update.json", &["sequence_num"]),
        (
            "ws_market_trades_update.json",
            &["events", "0", "trades", "0", "side"],
        ),
        (
            "ws_ticker_update.json",
            &["events", "0", "tickers", "0", "best_ask"],
        ),
        (
            "ws_ticker_batch_update.json",
            &["events", "0", "tickers", "0", "price"],
        ),
        (
            "ws_candles_update.json",
            &["events", "0", "candles", "0", "close"],
        ),
        (
            "ws_status_snapshot.json",
            &["events", "0", "products", "0", "status"],
        ),
        ("ws_heartbeats.json", &["events", "0", "heartbeat_counter"]),
        ("ws_subscriptions.json", &["events", "0", "subscriptions"]),
        ("ws_error.json", &["message"]),
    ];
    for (f, path) in cases {
        assert!(
            CoinbaseWsMessage::parse(&without(f, path)).is_err(),
            "{f} without {path:?} parsed"
        );
    }
    assert!(CoinbaseWsMessage::parse(r#"{"channel":"bogus","events":[]}"#).is_err());
}
