//! Offline parsing of live public market-data captures (mainnet, 2026-11-16) in `tests/fixtures/`.
//! REST via curl; websocket via `wss://mainnet.zklighter.elliot.ai/stream?readonly=true`.

use roshar_lighter::types::*;
use serde::de::DeserializeOwned;
use serde_json::Value;

fn fixture(name: &str) -> String {
    std::fs::read_to_string(format!(
        "{}/tests/fixtures/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn parse<T: DeserializeOwned>(name: &str) -> T {
    serde_json::from_str(&fixture(name)).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn without(name: &str, at: &str, key: &str) -> Value {
    let mut v: Value = serde_json::from_str(&fixture(name)).unwrap();
    v.pointer_mut(at)
        .and_then(Value::as_object_mut)
        .unwrap_or_else(|| panic!("{name}: no object at {at}"))
        .remove(key)
        .unwrap_or_else(|| panic!("{name}: no {key} at {at}"));
    v
}

/// Removes `key` from the object at JSON pointer `at` and checks `T` then fails to parse.
fn assert_required<T: DeserializeOwned + std::fmt::Debug>(name: &str, at: &str, key: &str) {
    assert!(
        serde_json::from_value::<T>(without(name, at, key)).is_err(),
        "{name}: {at}/{key} should be required"
    );
}

/// Same for websocket frames through `LighterWsMessage::parse`.
fn assert_ws_required(name: &str, at: &str, key: &str) {
    let v = without(name, at, key);
    assert!(
        LighterWsMessage::parse(&v.to_string()).is_err(),
        "{name}: {at}/{key} should be required"
    );
}

fn ws(name: &str) -> LighterWsMessage {
    LighterWsMessage::parse(&fixture(name)).unwrap_or_else(|e| panic!("{name}: {e:#}"))
}

// ── REST ──────────────────────────────────────────────────────────────────────

#[test]
fn status() {
    let s: LighterStatus = parse("rest_status.json");
    assert_eq!(s.status, 200);
    assert_required::<LighterStatus>("rest_status.json", "", "timestamp");
}

#[test]
fn order_book_details_perp_and_spot() {
    let d: LighterOrderBookDetails = parse("rest_order_book_details.json");
    let p = &d.order_book_details[0];
    assert_eq!(p.common.market_type, "perp");
    assert!(p.open_interest > 0.0);
    let s = &d.spot_order_book_details[0];
    assert_eq!(s.common.market_type, "spot");
    assert!(s.open_interest_usd.is_none());
    let f = "rest_order_book_details.json";
    assert_required::<LighterOrderBookDetails>(f, "", "spot_order_book_details");
    assert_required::<LighterOrderBookDetails>(f, "/order_book_details/0", "mark_price");
    assert_required::<LighterOrderBookDetails>(f, "/order_book_details/0", "symbol");
    assert_required::<LighterOrderBookDetails>(f, "/order_book_details/0/market_config", "hidden");
    assert_required::<LighterOrderBookDetails>(f, "/spot_order_book_details/0", "daily_chart");
}

#[test]
fn candles() {
    let c: LighterCandles = parse("rest_candles.json");
    assert_eq!(c.r, "1h");
    assert!(c.c[0].quote_volume > 0.0 && c.c[0].unadjusted_close.is_none());
    assert_required::<LighterCandles>("rest_candles.json", "/c/0", "t");
    assert_required::<LighterCandles>("rest_candles.json", "/c/0", "o");

    let m: LighterMarkPriceCandles = parse("rest_mark_price_candles.json");
    assert!(m.c[0].sc > 0);
    assert_required::<LighterMarkPriceCandles>("rest_mark_price_candles.json", "/c/0", "sc");

    let p: LighterMarketPriceCharts = parse("rest_market_price_charts.json");
    assert_eq!(p.price_charts[0].prices.len(), 24);
    assert_required::<LighterMarketPriceCharts>(
        "rest_market_price_charts.json",
        "/price_charts/0",
        "prices",
    );
}

#[test]
fn fundings() {
    let f: LighterFundings = parse("rest_fundings.json");
    assert_eq!(f.fundings[0].direction, "short");
    assert_required::<LighterFundings>("rest_fundings.json", "/fundings/0", "rate");
}

#[test]
fn exchange_stats_assets_oracle_fee_tiers() {
    let x: LighterExchangeStats = parse("rest_exchange_stats.json");
    assert!(x.daily_usd_volume > 0.0);
    assert_required::<LighterExchangeStats>(
        "rest_exchange_stats.json",
        "/order_book_stats/0",
        "symbol",
    );

    let a: LighterAssetDetails = parse("rest_asset_details.json");
    assert!(!a.asset_details.is_empty());
    assert_required::<LighterAssetDetails>(
        "rest_asset_details.json",
        "/asset_details/0",
        "index_price",
    );

    let o: LighterPriceOracleInfo = parse("rest_price_oracle_info.json");
    assert!(o
        .markets
        .iter()
        .any(|m| m.index.underlying.is_some() || m.mark.underlying.is_some()));
    assert!(o.markets.iter().any(|m| m.index.underlying.is_none()));
    assert_required::<LighterPriceOracleInfo>(
        "rest_price_oracle_info.json",
        "/markets/0/mark",
        "source",
    );

    let t: LighterFeeTiers = parse("rest_fee_tiers.json");
    assert!(!t.bo_tiers.is_empty());
    assert_required::<LighterFeeTiers>("rest_fee_tiers.json", "/user_tiers/0", "taker_fee_ticks");
}

#[test]
fn trades_page() {
    let t: LighterTradesResponse = parse("rest_trades.json");
    assert!(t.next_cursor.is_some());
    assert!(t.trades[0].ask_id_str.is_some());
    assert_required::<LighterTradesResponse>("rest_trades.json", "/trades/0", "price");
}

#[test]
fn binary_options() {
    let e: LighterEvents = parse("rest_events.json");
    let b = &e.events[0].order_books[0];
    assert_eq!(b.market.market_type, "binary_options");
    assert_eq!(b.stats.market_id, b.market.market_id);
    assert!(e.next_cursor.is_some());
    assert_required::<LighterEvents>("rest_events.json", "/events/0/order_books/0", "stats");
    assert_required::<LighterEvents>(
        "rest_events.json",
        "/events/0/order_books/0/stats",
        "status",
    );
    assert_required::<LighterEvents>("rest_events.json", "/events/0", "start_at");

    let u: LighterUpDownCandles = parse("rest_up_down_candles.json");
    assert_eq!(u.candles[0].resolution, "30s");
    assert_required::<LighterUpDownCandles>("rest_up_down_candles.json", "/candles/0", "close");
}

// ── Websocket ─────────────────────────────────────────────────────────────────

#[test]
fn ws_control_frames() {
    assert!(matches!(
        ws("ws_connected.json"),
        LighterWsMessage::Connected(_)
    ));
    assert!(matches!(ws("ws_pong.json"), LighterWsMessage::Pong));
    assert!(
        matches!(ws("ws_unsubscribed.json"), LighterWsMessage::Unsubscribed(u) if u.channel == "height")
    );
    for f in ["ws_error_0.json", "ws_error_1.json", "ws_error_2.json"] {
        match ws(f) {
            LighterWsMessage::Error(e) => assert_eq!(e.code, 30005),
            m => panic!("{f}: {m:?}"),
        }
    }
    assert!(LighterWsMessage::parse(r#"{"type":"update/nope","channel":"nope:1"}"#).is_err());
}

#[test]
fn ws_order_book_ticker_trade_height() {
    match ws("ws_order_book_snapshot.json") {
        LighterWsMessage::OrderBook(e) => assert!(e.snapshot && !e.data.order_book.asks.is_empty()),
        m => panic!("{m:?}"),
    }
    match ws("ws_order_book_update.json") {
        LighterWsMessage::OrderBook(e) => {
            assert!(!e.snapshot && e.data.order_book.begin_nonce < e.data.order_book.nonce)
        }
        m => panic!("{m:?}"),
    }
    assert_ws_required("ws_order_book_update.json", "/order_book", "begin_nonce");
    match ws("ws_ticker.json") {
        LighterWsMessage::Ticker(e) => assert_eq!(e.data.ticker.s, "ETH"),
        m => panic!("{m:?}"),
    }
    assert_ws_required("ws_ticker.json", "/ticker", "a");
    // Snapshot of a never-updated market: no top-level `last_updated_at`, both sides empty.
    match ws("ws_ticker_empty_book.json") {
        LighterWsMessage::Ticker(e) => {
            assert!(e.snapshot && e.data.last_updated_at == 0);
            assert!(e.data.ticker.a.price.is_empty() && e.data.ticker.b.price.is_empty());
        }
        m => panic!("{m:?}"),
    }
    match ws("ws_trade_snapshot.json") {
        LighterWsMessage::Trade(e) => assert!(e.snapshot && !e.data.liquidation_trades.is_empty()),
        m => panic!("{m:?}"),
    }
    match ws("ws_trade_update.json") {
        LighterWsMessage::Trade(e) => assert!(!e.data.trades.is_empty()),
        m => panic!("{m:?}"),
    }
    assert_ws_required("ws_trade_update.json", "/trades/0", "size");
    match ws("ws_height.json") {
        LighterWsMessage::Height(e) => assert!(e.data.height > 0),
        m => panic!("{m:?}"),
    }
}

#[test]
fn ws_market_stats() {
    match ws("ws_market_stats.json") {
        LighterWsMessage::MarketStats(e) => assert_eq!(e.data.stats.market_id, 0),
        m => panic!("{m:?}"),
    }
    assert_ws_required("ws_market_stats.json", "/market_stats", "funding_rate");
    match ws("ws_market_stats_all.json") {
        LighterWsMessage::MarketStatsMap(e) => assert!(e.snapshot && e.data.stats.contains_key(&0)),
        m => panic!("{m:?}"),
    }
    assert_ws_required("ws_market_stats_all.json", "/market_stats/0", "mark_price");
    // Partial updates fail the strict parser and parse as patches with parse_partial.
    let partial = fixture("ws_market_stats_list_partial.json");
    assert!(LighterWsMessage::parse(&partial).is_err());
    match LighterWsMessage::parse_partial(&partial).unwrap() {
        LighterWsMessage::MarketStatsPatch(p) => {
            assert!(p.stats[&0].mark_price.is_some() && p.stats[&0].symbol.is_none())
        }
        m => panic!("{m:?}"),
    }
    // Snapshots stay full under parse_partial.
    assert!(matches!(
        LighterWsMessage::parse_partial(&fixture("ws_market_stats_all.json")).unwrap(),
        LighterWsMessage::MarketStatsMap(_)
    ));
}

#[test]
fn ws_spot_and_bo_stats() {
    match ws("ws_spot_market_stats.json") {
        LighterWsMessage::SpotMarketStats(e) => assert_eq!(e.data.stats.market_id, 2048),
        m => panic!("{m:?}"),
    }
    assert_ws_required(
        "ws_spot_market_stats.json",
        "/spot_market_stats",
        "index_price",
    );
    assert!(matches!(
        ws("ws_spot_market_stats_all.json"),
        LighterWsMessage::SpotMarketStatsMap(_)
    ));
    for f in [
        "ws_spot_market_stats_list_partial.json",
        "ws_spot_market_stats_list_partial_id_only.json",
    ] {
        assert!(matches!(
            LighterWsMessage::parse_partial(&fixture(f)).unwrap(),
            LighterWsMessage::SpotMarketStatsPatch(_)
        ));
    }
    match ws("ws_bo_market_stats.json") {
        LighterWsMessage::BoMarketStats(e) => assert_eq!(e.data.stats.market_id, 10408),
        m => panic!("{m:?}"),
    }
    assert_ws_required("ws_bo_market_stats.json", "/bo_market_stats", "outcome");
    match ws("ws_bo_market_stats_all.json") {
        LighterWsMessage::BoMarketStatsMap(e) => assert_eq!(e.data.stats.len(), 2),
        m => panic!("{m:?}"),
    }
}

#[test]
fn ws_candles() {
    match ws("ws_candle_snapshot.json") {
        LighterWsMessage::Candle(e) => assert!(e.snapshot && e.data.candles[0].i > 0),
        m => panic!("{m:?}"),
    }
    assert!(matches!(ws("ws_candle_update.json"), LighterWsMessage::Candle(e) if !e.snapshot));
    assert_ws_required("ws_candle_update.json", "/candles/0", "c");
    match ws("ws_mark_price_candle.json") {
        LighterWsMessage::MarkPriceCandle(e) => assert!(e.data.candles[0].sc > 0),
        m => panic!("{m:?}"),
    }
    assert_ws_required("ws_mark_price_candle.json", "/candles/0", "sc");
    match ws("ws_up_down_candle_snapshot.json") {
        LighterWsMessage::UpDownCandle(e) => assert_eq!(e.data.candles.len(), 2),
        m => panic!("{m:?}"),
    }
    assert!(matches!(
        ws("ws_up_down_candle_update.json"),
        LighterWsMessage::UpDownCandle(_)
    ));
    assert_ws_required("ws_up_down_candle_update.json", "/candles/0", "feed_id");
}

#[test]
fn subscribe_builders() {
    assert_eq!(
        LighterWssMessage::all_market_stats(Some(true)).to_json(),
        r#"{"type":"subscribe","channel":"market_stats/all","partial":true}"#
    );
    assert_eq!(
        LighterWssMessage::market_stats_list(&[0, 1], None).to_json(),
        r#"{"type":"subscribe","channel":"market_stats","market_ids":[0,1]}"#
    );
    assert_eq!(
        LighterWssMessage::market_stats_list_unsub(None).to_json(),
        r#"{"type":"unsubscribe","channel":"market_stats"}"#
    );
    assert_eq!(
        LighterWssMessage::candle(0, "1m")
            .into_unsubscribe()
            .to_json(),
        r#"{"type":"unsubscribe","channel":"candle/0/1m"}"#
    );
    assert_eq!(
        LighterWssMessage::up_down_candle("eth", "15m", "1s").channel,
        "up_down_candle/eth/15m/1s"
    );
    assert_eq!(LighterWssMessage::height().channel, "height");
    assert_eq!(
        LighterWssMessage::mark_price_candle(0, "1h").channel,
        "mark_price_candle/0/1h"
    );
    assert_eq!(
        LighterWssMessage::spot_market_stats(2048).channel,
        "spot_market_stats/2048"
    );
    assert_eq!(
        LighterWssMessage::all_bo_market_stats().channel,
        "bo_market_stats/all"
    );
}
