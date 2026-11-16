//! CLOB fixtures: trimmed (fewer rows / book levels, values unedited) live responses from
//! `https://clob.polymarket.com` captured on 2026-11-16.

use roshar_polymarket::types::{
    ClobBatchPricesHistory, ClobBook, ClobError, ClobFeeRate, ClobLastTradePrice,
    ClobLastTradePriceItem, ClobLiveActivityMarket, ClobMarket, ClobMarketByToken, ClobMarketInfo,
    ClobMarketsPage, ClobMidpoint, ClobNegRisk, ClobPrice, ClobPricesHistory, ClobSide,
    ClobSimplifiedMarket, ClobSpread, ClobTickSize, ClobTokenPrices, ClobTokenValues,
    CLOB_END_CURSOR,
};
use serde_json::Value;

fn without(fixture: &str, pointer: &str, key: &str) -> String {
    let mut v: Value = serde_json::from_str(fixture).unwrap();
    v.pointer_mut(pointer)
        .unwrap()
        .as_object_mut()
        .unwrap()
        .remove(key)
        .unwrap();
    v.to_string()
}

const MARKETS: &str = include_str!("fixtures/clob_markets.json");
const BOOK: &str = include_str!("fixtures/clob_book.json");
const INFO_FULL: &str = include_str!("fixtures/clob_market_info_full.json");

#[test]
fn parses_markets_pages() {
    let page: ClobMarketsPage<ClobMarket> = serde_json::from_str(MARKETS).unwrap();
    assert_eq!(page.data.len(), 3);
    assert_ne!(page.next_cursor, CLOB_END_CURSOR);
    assert!(page.data[0].accepting_order_timestamp.is_none() && page.data[0].tags.is_none());
    assert!(page.data[1].end_date_iso.is_none());
    assert!(page.data[2].rewards.rates.is_some() && page.data[2].neg_risk);

    let end: ClobMarketsPage<ClobMarket> =
        serde_json::from_str(include_str!("fixtures/clob_markets_end.json")).unwrap();
    assert_eq!(end.next_cursor, CLOB_END_CURSOR);
    assert!(end.data.is_empty());

    let one: ClobMarket = serde_json::from_str(include_str!("fixtures/clob_market.json")).unwrap();
    assert_eq!(one.tokens.len(), 2);

    let simple: ClobMarketsPage<ClobSimplifiedMarket> =
        serde_json::from_str(include_str!("fixtures/clob_simplified_markets.json")).unwrap();
    assert!(simple.data[0].rewards.rates.is_none());
    assert!(simple.data[1].rewards.rates.is_some());
}

#[test]
fn markets_are_strict() {
    // Nullable keys must still be present.
    let missing = without(MARKETS, "/data/0", "tags");
    assert!(serde_json::from_str::<ClobMarketsPage<ClobMarket>>(&missing).is_err());
    let missing = without(MARKETS, "/data/0", "neg_risk");
    assert!(serde_json::from_str::<ClobMarketsPage<ClobMarket>>(&missing).is_err());
}

#[test]
fn parses_market_info_and_lookups() {
    let full: ClobMarketInfo = serde_json::from_str(INFO_FULL).unwrap();
    assert!(full.fee_details.is_some() && full.taker_base_fee.is_some());
    let min: ClobMarketInfo =
        serde_json::from_str(include_str!("fixtures/clob_market_info_min.json")).unwrap();
    assert!(min.min_order_size.is_none() && min.fee_details.is_none());
    assert!(serde_json::from_str::<ClobMarketInfo>(&without(INFO_FULL, "", "mts")).is_err());

    let by_token: ClobMarketByToken =
        serde_json::from_str(include_str!("fixtures/clob_market_by_token.json")).unwrap();
    assert!(by_token.condition_id.starts_with("0x"));

    let _: ClobLiveActivityMarket =
        serde_json::from_str(include_str!("fixtures/clob_live_activity_market.json")).unwrap();
    let many: Vec<ClobLiveActivityMarket> =
        serde_json::from_str(include_str!("fixtures/clob_live_activity_markets.json")).unwrap();
    assert_eq!(many.len(), 2);
}

#[test]
fn parses_books() {
    let book: ClobBook = serde_json::from_str(BOOK).unwrap();
    assert!(book.tick_size > 0.0);
    let books: Vec<ClobBook> =
        serde_json::from_str(include_str!("fixtures/clob_books.json")).unwrap();
    assert!(books[0].last_trade_price.is_some());
    // `"last_trade_price": ""` on a token that never traded.
    assert!(books[1].last_trade_price.is_none());

    assert!(serde_json::from_str::<ClobBook>(&without(BOOK, "", "last_trade_price")).is_err());
    assert!(serde_json::from_str::<ClobBook>(&without(BOOK, "/bids/0", "size")).is_err());
}

#[test]
fn parses_prices() {
    let p: ClobPrice = serde_json::from_str(include_str!("fixtures/clob_price.json")).unwrap();
    assert!(p.price > 0.0);
    let prices: ClobTokenPrices =
        serde_json::from_str(include_str!("fixtures/clob_prices.json")).unwrap();
    assert!(prices
        .values()
        .all(|sides| sides.contains_key(&ClobSide::Buy) || sides.contains_key(&ClobSide::Sell)));

    let m: ClobMidpoint =
        serde_json::from_str(include_str!("fixtures/clob_midpoint.json")).unwrap();
    assert!(m.mid > 0.0);
    let mids: ClobTokenValues =
        serde_json::from_str(include_str!("fixtures/clob_midpoints.json")).unwrap();
    assert_eq!(mids.len(), 2);
    assert!(serde_json::from_str::<ClobMidpoint>(r#"{"mid_price":"0.5"}"#).is_err());

    let s: ClobSpread = serde_json::from_str(include_str!("fixtures/clob_spread.json")).unwrap();
    assert!(s.spread >= 0.0);
    let spreads: ClobTokenValues =
        serde_json::from_str(include_str!("fixtures/clob_spreads.json")).unwrap();
    assert_eq!(spreads.len(), 2);

    let l: ClobLastTradePrice =
        serde_json::from_str(include_str!("fixtures/clob_last_trade_price.json")).unwrap();
    assert!(l.side.is_some());
    let u: ClobLastTradePrice =
        serde_json::from_str(include_str!("fixtures/clob_last_trade_price_untraded.json")).unwrap();
    assert_eq!((u.price, u.side), (0.5, None));
    let items: Vec<ClobLastTradePriceItem> =
        serde_json::from_str(include_str!("fixtures/clob_last_trades_prices.json")).unwrap();
    assert_eq!(items.len(), 2);
}

#[test]
fn parses_token_parameters() {
    let t: ClobTickSize =
        serde_json::from_str(include_str!("fixtures/clob_tick_size.json")).unwrap();
    assert!(t.minimum_tick_size > 0.0);
    let n: ClobNegRisk = serde_json::from_str(include_str!("fixtures/clob_neg_risk.json")).unwrap();
    assert!(n.neg_risk);
    let f: ClobFeeRate = serde_json::from_str(include_str!("fixtures/clob_fee_rate.json")).unwrap();
    assert!(f.base_fee >= 0);
    let time: u64 = serde_json::from_str(include_str!("fixtures/clob_time.json")).unwrap();
    assert!(time > 1_700_000_000);
}

#[test]
fn parses_price_history() {
    let h: ClobPricesHistory =
        serde_json::from_str(include_str!("fixtures/clob_prices_history.json")).unwrap();
    assert_eq!(h.history.len(), 3);
    let b: ClobBatchPricesHistory =
        serde_json::from_str(include_str!("fixtures/clob_batch_prices_history.json")).unwrap();
    assert_eq!(b.history.len(), 2);
    assert!(serde_json::from_str::<ClobPricesHistory>(r#"{"history":[{"t":1}]}"#).is_err());
}

#[test]
fn parses_error_body() {
    let e: ClobError = serde_json::from_str(include_str!("fixtures/clob_error.json")).unwrap();
    assert_eq!(e.error, "No orderbook exists for the requested token id");
}
