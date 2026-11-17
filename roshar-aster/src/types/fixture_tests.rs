//! Parse tests over live captures in `tests/fixtures/` (REST and wss://fstream.asterdex.com,
//! captured 2026-11-16).

use super::*;

macro_rules! fixture {
    ($name:literal) => {
        include_str!(concat!("../../tests/fixtures/", $name))
    };
}

/// Removes `key` from the first object found at `pointer` and returns the JSON text.
fn without(json: &str, pointer: &str, key: &str) -> String {
    let mut v: serde_json::Value = serde_json::from_str(json).unwrap();
    v.pointer_mut(pointer)
        .and_then(|o| o.as_object_mut())
        .unwrap_or_else(|| panic!("no object at {pointer}"))
        .remove(key)
        .unwrap_or_else(|| panic!("no key {key} at {pointer}"));
    v.to_string()
}

#[test]
fn server_time() {
    let t: AsterServerTime = serde_json::from_str(fixture!("server_time.json")).unwrap();
    assert!(t.server_time > 0);
    assert!(serde_json::from_str::<AsterServerTime>("{}").is_err());
}

#[test]
fn exchange_info() {
    let raw = fixture!("exchange_info.json");
    let info: AsterExchangeInfo = serde_json::from_str(raw).unwrap();
    assert_eq!(info.futures_type, "U_MARGINED");
    assert_eq!(info.rate_limits[0].limit, 2400);
    assert_eq!(info.assets[0].asset_type, "crypto");
    assert_eq!(info.symbols.len(), 3);
    let s = &info.symbols[0];
    assert_eq!(s.filters.len(), 7);
    assert!(s
        .filters
        .iter()
        .any(|f| matches!(f, AsterSymbolFilter::PriceFilter { .. })));
    assert!(s.filters.iter().any(|f| matches!(
        f,
        AsterSymbolFilter::PercentPrice { multiplier_decimal, .. } if multiplier_decimal == "4"
    )));
    // A symbol that is not trading yet has an empty contract type.
    assert_eq!(info.symbols[2].contract_type, "");

    let err = serde_json::from_str::<AsterExchangeInfo>(&without(raw, "/symbols/0", "pair"));
    assert!(err.is_err());
    let err = serde_json::from_str::<AsterExchangeInfo>(&without(
        raw,
        "/symbols/0/filters/0",
        "tickSize",
    ));
    assert!(err.is_err());
    let unknown = raw.replacen("\"MIN_NOTIONAL\"", "\"NEW_FILTER\"", 1);
    assert!(serde_json::from_str::<AsterExchangeInfo>(&unknown).is_err());
    let null_auto = raw.replacen(
        "\"autoAssetExchange\": \"0\"",
        "\"autoAssetExchange\": null",
        1,
    );
    let info: AsterExchangeInfo = serde_json::from_str(&null_auto).unwrap();
    assert_eq!(info.assets[0].auto_asset_exchange, None);
    assert!(serde_json::from_str::<AsterExchangeInfo>(&without(
        raw,
        "/assets/0",
        "autoAssetExchange"
    ))
    .is_err());
}

#[test]
fn trades() {
    for raw in [fixture!("trades.json"), fixture!("historical_trades.json")] {
        let t: Vec<AsterTrade> = serde_json::from_str(raw).unwrap();
        assert_eq!(t.len(), 3);
        assert_eq!(t[0].quote_qty, "64.6");
        assert!(serde_json::from_str::<Vec<AsterTrade>>(&without(raw, "/0", "quoteQty")).is_err());
    }
}

#[test]
fn agg_trades() {
    let raw = fixture!("agg_trades.json");
    let t: Vec<AsterAggTrade> = serde_json::from_str(raw).unwrap();
    assert_eq!(t[1].first_trade_id, 150074921);
    assert_eq!(t[1].last_trade_id, 150074923);
    assert!(t[1].is_buyer_maker);
    assert!(serde_json::from_str::<Vec<AsterAggTrade>>(&without(raw, "/0", "m")).is_err());
}

#[test]
fn price_klines() {
    for raw in [
        fixture!("index_price_klines.json"),
        fixture!("mark_price_klines.json"),
    ] {
        let k: Vec<AsterPriceKline> = serde_json::from_str(raw).unwrap();
        assert_eq!(k.len(), 2);
        assert!(k[0].data_points > 0);
        assert_eq!(k[0].close_time - k[0].open_time, 59_999);
    }
    let short = r#"[[1794827400000,"1","2","0.5","1.5","0",1794827459999,"0"]]"#;
    let err = serde_json::from_str::<Vec<AsterPriceKline>>(short).unwrap_err();
    assert!(err.to_string().contains("missing index 8"), "{err}");
}

#[test]
fn premium_index_single() {
    let raw = fixture!("premium_index_single.json");
    let m: AsterMarkPrice = serde_json::from_str(raw).unwrap();
    assert_eq!(m.symbol, "BTCUSDT");
    assert!(serde_json::from_str::<AsterMarkPrice>(&without(raw, "", "indexPrice")).is_err());
}

#[test]
fn funding_info() {
    let raw = fixture!("funding_info.json");
    let f: Vec<AsterFundingInfo> = serde_json::from_str(raw).unwrap();
    assert_eq!(f[0].funding_interval_hours, 8);
    assert_eq!(f[0].funding_fee_cap, 0.02);
    assert_eq!(f[0].funding_fee_floor, -0.02);
    assert!(serde_json::from_str::<Vec<AsterFundingInfo>>(&without(
        raw,
        "/0",
        "fundingIntervalHours"
    ))
    .is_err());
}

#[test]
fn ticker_24hr() {
    let raw = fixture!("ticker_24hr_single.json");
    let t: AsterTicker24hr = serde_json::from_str(raw).unwrap();
    assert_eq!(t.count, 52039);
    assert_eq!(t.last_qty, "0.029");
    let all: Vec<AsterTicker24hr> = serde_json::from_str(fixture!("ticker_24hr_all.json")).unwrap();
    assert_eq!(all.len(), 2);
    assert!(serde_json::from_str::<AsterTicker24hr>(&without(raw, "", "lastId")).is_err());
}

#[test]
fn price_ticker() {
    let raw = fixture!("ticker_price_single.json");
    let t: AsterPriceTicker = serde_json::from_str(raw).unwrap();
    assert_eq!(t.price, "64607.9");
    let all: Vec<AsterPriceTicker> =
        serde_json::from_str(fixture!("ticker_price_all.json")).unwrap();
    assert_eq!(all.len(), 3);
    assert!(serde_json::from_str::<AsterPriceTicker>(&without(raw, "", "time")).is_err());
}

#[test]
fn book_ticker() {
    let raw = fixture!("book_ticker_single.json");
    let t: AsterBookTicker = serde_json::from_str(raw).unwrap();
    assert_eq!(t.last_update_id, 657235081941);
    let all: Vec<AsterBookTicker> = serde_json::from_str(fixture!("book_ticker_all.json")).unwrap();
    assert_eq!(all.len(), 2);
    assert!(serde_json::from_str::<AsterBookTicker>(&without(raw, "", "askQty")).is_err());
}

#[test]
fn index_references() {
    let raw = fixture!("index_references.json");
    let r: AsterIndexReferences = serde_json::from_str(raw).unwrap();
    assert_eq!(r.references[0].exchange, "binance");
    assert!(
        serde_json::from_str::<AsterIndexReferences>(&without(raw, "/references/0", "weight"))
            .is_err()
    );
}

#[test]
fn remaining_openable_notional_and_open_interest() {
    let raw = fixture!("remaining_openable_notional.json");
    let r: AsterRemainingOpenableNotional = serde_json::from_str(raw).unwrap();
    assert_eq!(r.remaining_openable_notional_value, "616971195.18731691084");
    assert!(serde_json::from_str::<AsterRemainingOpenableNotional>("{}").is_err());

    let raw = fixture!("open_interest.json");
    let oi: AsterOpenInterest = serde_json::from_str(raw).unwrap();
    assert_eq!(oi.open_interest, "6036.927");
    assert!(serde_json::from_str::<AsterOpenInterest>(&without(raw, "", "openInterest")).is_err());
}

#[test]
fn api_error() {
    let e: AsterApiError = serde_json::from_str(fixture!("error_invalid_symbol.json")).unwrap();
    assert_eq!(e.code, -1121);
    assert_eq!(e.msg, "Invalid symbol.");
}

// ── WebSocket ────────────────────────────────────────────────────────────────

fn ws(raw: &str) -> AsterWsMessage {
    AsterWsMessage::parse(raw).unwrap_or_else(|e| panic!("{e}: {raw}"))
}

#[test]
fn ws_responses() {
    assert!(matches!(
        ws(fixture!("ws_sub_ack.json")),
        AsterWsMessage::Response {
            id: 1,
            result: AsterWsResult::Null
        }
    ));
    match ws(fixture!("ws_list_subscriptions.json")) {
        AsterWsMessage::Response {
            id: 2,
            result: AsterWsResult::Subscriptions(s),
        } => assert!(s.contains(&"!markPrice@arr".to_string())),
        other => panic!("{other:?}"),
    }
    assert!(matches!(
        ws(fixture!("ws_get_property.json")),
        AsterWsMessage::Response {
            id: 3,
            result: AsterWsResult::Property(false)
        }
    ));
    match ws(fixture!("ws_error.json")) {
        AsterWsMessage::Error { id, code, msg } => {
            assert_eq!(id, Some(5));
            assert_eq!(code, 2);
            assert!(msg.starts_with("Invalid request"));
        }
        other => panic!("{other:?}"),
    }
    assert!(AsterWsMessage::parse(r#"{"result":null}"#).is_err());
    assert!(AsterWsMessage::parse(r#"{"id":1,"error":{"code":2}}"#).is_err());
}

#[test]
fn ws_depth() {
    for (raw, levels) in [
        (fixture!("ws_depth5.json"), 5),
        (fixture!("ws_depth20.json"), 20),
    ] {
        match ws(raw) {
            AsterWsMessage::Depth(d) => {
                assert_eq!(d.bids.len(), levels);
                assert_eq!(d.asks.len(), levels);
            }
            other => panic!("{other:?}"),
        }
        assert!(AsterWsMessage::parse(&without(raw, "", "pu")).is_err());
    }
}

#[test]
fn ws_mark_price() {
    let raw = fixture!("ws_mark_price_1s.json");
    assert!(matches!(ws(raw), AsterWsMessage::MarkPrice(m) if m.symbol == "BTCUSDT"));
    let raw = fixture!("ws_mark_price_arr.json");
    match ws(raw) {
        AsterWsMessage::MarkPriceArr(v) => assert_eq!(v.len(), 2),
        other => panic!("{other:?}"),
    }
    assert!(AsterWsMessage::parse(&without(raw, "/1", "r")).is_err());
}

#[test]
fn ws_tickers() {
    let raw = fixture!("ws_mini_ticker.json");
    assert!(matches!(ws(raw), AsterWsMessage::MiniTicker(t) if t.symbol == "BTCUSDT"));
    assert!(AsterWsMessage::parse(&without(raw, "", "q")).is_err());
    match ws(fixture!("ws_mini_ticker_arr.json")) {
        AsterWsMessage::MiniTickerArr(v) => assert_eq!(v.len(), 2),
        other => panic!("{other:?}"),
    }

    let raw = fixture!("ws_ticker.json");
    match ws(raw) {
        AsterWsMessage::Ticker(t) => assert!(t.num_trades > 0),
        other => panic!("{other:?}"),
    }
    assert!(AsterWsMessage::parse(&without(raw, "", "n")).is_err());
    match ws(fixture!("ws_ticker_arr.json")) {
        AsterWsMessage::TickerArr(v) => assert_eq!(v.len(), 2),
        other => panic!("{other:?}"),
    }
}

#[test]
fn ws_book_ticker() {
    let raw = fixture!("ws_book_ticker.json");
    assert!(matches!(ws(raw), AsterWsMessage::BookTicker(b) if b.update_id > 0));
    assert!(AsterWsMessage::parse(&without(raw, "", "u")).is_err());
}

#[test]
fn ws_combined() {
    match ws(fixture!("ws_combined_book_ticker.json")) {
        AsterWsMessage::Combined { stream, data } => {
            assert_eq!(stream, "btcusdt@bookTicker");
            assert!(matches!(*data, AsterWsMessage::BookTicker(_)));
        }
        other => panic!("{other:?}"),
    }
    let raw = fixture!("ws_combined_agg_trade.json");
    match ws(raw) {
        AsterWsMessage::Combined { data, .. } => {
            assert!(matches!(*data, AsterWsMessage::AggTrade(_)))
        }
        other => panic!("{other:?}"),
    }
    assert!(AsterWsMessage::parse(&without(raw, "/data", "a")).is_err());
    assert!(AsterWsMessage::parse(r#"{"stream":"x"}"#).is_err());
}

#[test]
fn ws_unknown_event_is_error() {
    assert!(AsterWsMessage::parse(r#"{"e":"somethingNew","E":1}"#).is_err());
    assert!(AsterWsMessage::parse("[]").is_err());
}

#[test]
fn ws_force_order() {
    let raw = fixture!("ws_force_order.json");
    match ws(raw) {
        AsterWsMessage::ForceOrder(f) => {
            assert_eq!(f.order.symbol, "BTCUSDT");
            assert_eq!(f.order.avg_price, "64690.2");
            assert_eq!(f.order.status, "FILLED");
        }
        other => panic!("{other:?}"),
    }
    assert!(AsterWsMessage::parse(&without(raw, "/o", "ap")).is_err());
    // `!forceOrder@arr` sends single objects, not arrays.
    for raw in [
        fixture!("ws_combined_force_order.json"),
        fixture!("ws_combined_all_force_order.json"),
    ] {
        match ws(raw) {
            AsterWsMessage::Combined { data, .. } => {
                assert!(matches!(*data, AsterWsMessage::ForceOrder(_)))
            }
            other => panic!("{other:?}"),
        }
    }
}
