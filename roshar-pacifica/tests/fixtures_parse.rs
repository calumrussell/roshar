//! Parses live captures from 2026-11-16 (mainnet) under `tests/fixtures/`.

use roshar_pacifica::types::{
    PacificaKline, PacificaResponse, PacificaSpotAsset, PacificaTrade, PacificaWsError,
    PacificaWsEvent, PacificaWsMessage,
};
use roshar_pacifica::PacificaInflater;

fn fixture(name: &str) -> String {
    std::fs::read_to_string(format!(
        "{}/tests/fixtures/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

fn fixture_bytes(name: &str) -> Vec<u8> {
    std::fs::read(format!(
        "{}/tests/fixtures/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

fn event(name: &str) -> PacificaWsEvent {
    match PacificaWsMessage::parse(&fixture(name)).unwrap() {
        PacificaWsMessage::Event(e) => e,
        other => panic!("{name}: {other:?}"),
    }
}

/// Removes the first occurrence of `key` (depth-first) from a JSON document.
fn without(text: &str, key: &str) -> String {
    let mut v: serde_json::Value = serde_json::from_str(text).unwrap();
    fn strip(v: &mut serde_json::Value, key: &str) -> bool {
        match v {
            serde_json::Value::Object(m) => {
                if m.remove(key).is_some() {
                    return true;
                }
                m.values_mut().any(|c| strip(c, key))
            }
            serde_json::Value::Array(a) => a.iter_mut().any(|c| strip(c, key)),
            _ => false,
        }
    }
    assert!(strip(&mut v, key), "{key} not found");
    v.to_string()
}

#[test]
fn ws_subscription_messages() {
    use roshar_pacifica::types::PacificaWssMessage as M;
    assert_eq!(
        M::bbo("BTC").to_json(),
        r#"{"method":"subscribe","params":{"source":"bbo","symbol":"BTC"}}"#
    );
    assert_eq!(
        M::mark_price_candles_unsub("BTC", "1m").to_json(),
        r#"{"method":"unsubscribe","params":{"source":"mark_price_candle","symbol":"BTC","interval":"1m"}}"#
    );
    assert_eq!(
        M::candles_unsub("BTC", "1m").to_json(),
        r#"{"method":"unsubscribe","params":{"source":"candle","symbol":"BTC","interval":"1m"}}"#
    );
    assert_eq!(
        M::prices_unsub().to_json(),
        r#"{"method":"unsubscribe","params":{"source":"prices"}}"#
    );
}

#[test]
fn rest_mark_price_klines() {
    let r: PacificaResponse<Vec<PacificaKline>> =
        serde_json::from_str(&fixture("rest_kline_mark.json")).unwrap();
    let candles = r.data.unwrap();
    assert_eq!(candles.len(), 3);
    assert_eq!(candles[0].symbol, "BTC");
    assert_eq!(candles[0].interval, "1m");
    assert_eq!(candles[0].open, "64732.926528");
    assert_eq!(candles[0].trades, 44);
    let bad = without(&fixture("rest_kline_mark.json"), "c");
    assert!(serde_json::from_str::<PacificaResponse<Vec<PacificaKline>>>(&bad).is_err());
}

#[test]
fn rest_spot_assets() {
    let r: PacificaResponse<Vec<PacificaSpotAsset>> =
        serde_json::from_str(&fixture("rest_spot_assets.json")).unwrap();
    let assets = r.data.unwrap();
    assert_eq!(assets[0].symbol, "SOL");
    assert_eq!(assets[0].ltv_ratio, "0.8");
    assert!(assets[0].active && assets[0].collateral_enabled);
    let bad = without(&fixture("rest_spot_assets.json"), "ltv_ratio");
    assert!(serde_json::from_str::<PacificaResponse<Vec<PacificaSpotAsset>>>(&bad).is_err());
}

#[test]
fn rest_trades_all_markets() {
    let r: PacificaResponse<Vec<PacificaTrade>> =
        serde_json::from_str(&fixture("rest_trades_all.json")).unwrap();
    assert_eq!(r.data.unwrap().len(), 2);
}

#[test]
fn ws_acks_and_pong() {
    match event("ws_subscribe.json") {
        PacificaWsEvent::Subscribe(p) => {
            assert_eq!(p.source, "bbo");
            assert_eq!(p.symbol.as_deref(), Some("BTC"));
        }
        e => panic!("{e:?}"),
    }
    match event("ws_unsubscribe.json") {
        PacificaWsEvent::Unsubscribe(p) => {
            assert_eq!(p.source, "candle");
            assert_eq!(p.interval.as_deref(), Some("1m"));
        }
        e => panic!("{e:?}"),
    }
    assert!(matches!(event("ws_pong.json"), PacificaWsEvent::Pong));
}

#[test]
fn ws_errors() {
    match PacificaWsMessage::parse(&fixture("ws_error_message.json")).unwrap() {
        PacificaWsMessage::Error(PacificaWsError::Message { error }) => {
            assert_eq!(error, "Symbol not found")
        }
        m => panic!("{m:?}"),
    }
    match PacificaWsMessage::parse(&fixture("ws_error_code.json")).unwrap() {
        PacificaWsMessage::Error(PacificaWsError::Coded { code, err, .. }) => {
            assert_eq!(code, 400);
            assert_eq!(err, "Invalid subscription parameters.");
        }
        m => panic!("{m:?}"),
    }
    assert!(PacificaWsMessage::parse(r#"{"foo":1}"#).is_err());
    assert!(PacificaWsMessage::parse(r#"{"channel":"nope","data":{}}"#).is_err());
}

#[test]
fn ws_book() {
    match event("ws_book.json") {
        PacificaWsEvent::Book(b) => {
            assert_eq!(b.symbol, "BTC");
            assert_eq!(b.bids().len(), 10);
            assert_eq!(b.asks().len(), 10);
            assert!(b.li > 0);
        }
        e => panic!("{e:?}"),
    }
    let bad = without(&fixture("ws_book.json"), "li");
    assert!(PacificaWsMessage::parse(&bad).is_err());
}

#[test]
fn ws_bbo() {
    match event("ws_bbo.json") {
        PacificaWsEvent::Bbo(b) => {
            assert_eq!(b.symbol, "BTC");
            assert_eq!(b.bid_price, "64701");
            assert_eq!(b.ask_amount, "3.58957");
        }
        e => panic!("{e:?}"),
    }
    let bad = without(&fixture("ws_bbo.json"), "B");
    assert!(PacificaWsMessage::parse(&bad).is_err());
}

#[test]
fn ws_trades() {
    match event("ws_trades.json") {
        PacificaWsEvent::Trades(t) => {
            assert_eq!(t[0].symbol, "BTC");
            assert_eq!(t[0].side, "open_short");
            assert_eq!(t[0].cause, "normal");
            assert_eq!(t[0].instrument_type, 0);
        }
        e => panic!("{e:?}"),
    }
    let bad = without(&fixture("ws_trades.json"), "tc");
    assert!(PacificaWsMessage::parse(&bad).is_err());
}

#[test]
fn ws_candles() {
    match event("ws_candle.json") {
        PacificaWsEvent::Candle(c) => assert_eq!(c.interval, "1m"),
        e => panic!("{e:?}"),
    }
    match event("ws_mark_price_candle.json") {
        PacificaWsEvent::MarkPriceCandle(c) => assert_eq!(c.symbol, "BTC"),
        e => panic!("{e:?}"),
    }
    let bad = without(&fixture("ws_mark_price_candle.json"), "n");
    assert!(PacificaWsMessage::parse(&bad).is_err());
}

#[test]
fn ws_prices() {
    match event("ws_prices.json") {
        PacificaWsEvent::Prices(p) => {
            assert_eq!(p.len(), 2);
            assert_eq!(p[0].symbol, "WIF");
        }
        e => panic!("{e:?}"),
    }
    let bad = without(&fixture("ws_prices.json"), "mark");
    assert!(PacificaWsMessage::parse(&bad).is_err());
}

#[test]
fn ws_deflate_frames_share_state() {
    let mut inflater = PacificaInflater::new();
    let first = inflater
        .decompress(&fixture_bytes("ws_deflate_bbo_1.bin"))
        .unwrap();
    let second = inflater
        .decompress(&fixture_bytes("ws_deflate_bbo_2.bin"))
        .unwrap();
    for text in [&first, &second] {
        assert!(matches!(
            PacificaWsMessage::parse(text).unwrap(),
            PacificaWsMessage::Event(PacificaWsEvent::Bbo(_))
        ));
    }
    // The second frame refers back to the first: a fresh decoder cannot read it alone.
    let mut fresh = PacificaInflater::new();
    assert!(fresh
        .decompress(&fixture_bytes("ws_deflate_bbo_2.bin"))
        .map(|t| t != second)
        .unwrap_or(true));
}
