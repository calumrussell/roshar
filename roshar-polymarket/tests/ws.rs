//! Market websocket fixtures: live frames from
//! `wss://ws-subscriptions-clob.polymarket.com/ws/market` captured on 2026-11-16 with
//! `custom_feature_enabled: true`. Book levels are trimmed; other values are unedited.

use roshar_polymarket::types::{MarketWsEvent, MarketWsMessage, MarketWsSide};
use serde_json::Value;

fn event(text: &str) -> MarketWsEvent {
    match MarketWsMessage::parse(text).unwrap() {
        MarketWsMessage::Event(e) => e,
        other => panic!("expected a single event, got {other:?}"),
    }
}

fn without(text: &str, key: &str) -> String {
    let mut v: Value = serde_json::from_str(text).unwrap();
    v.as_object_mut().unwrap().remove(key).unwrap();
    v.to_string()
}

const SNAPSHOT: &str = include_str!("fixtures/ws_market_subscribe_snapshot.json");
const BOOK: &str = include_str!("fixtures/ws_market_book.json");
const PRICE_CHANGE: &str = include_str!("fixtures/ws_market_price_change.json");
const LAST_TRADE: &str = include_str!("fixtures/ws_market_last_trade_price.json");
const TICK: &str = include_str!("fixtures/ws_market_tick_size_change.json");
const BBA: &str = include_str!("fixtures/ws_market_best_bid_ask.json");
const NEW_MARKET: &str = include_str!("fixtures/ws_market_new_market.json");
const RESOLVED: &str = include_str!("fixtures/ws_market_market_resolved.json");

#[test]
fn parses_subscription_snapshot_batch() {
    let MarketWsMessage::Batch(books) = MarketWsMessage::parse(SNAPSHOT).unwrap() else {
        panic!("expected a batch");
    };
    assert_eq!(books.len(), 2);
    let MarketWsEvent::Book(traded) = &books[0] else {
        panic!("expected book");
    };
    assert!(traded.tick_size.is_some());
    assert!(traded.last_trade_price.is_some());
    let MarketWsEvent::Book(untraded) = &books[1] else {
        panic!("expected book");
    };
    // `"last_trade_price": ""` before the first trade.
    assert!(untraded.last_trade_price.is_none());
}

#[test]
fn parses_post_trade_book() {
    let MarketWsEvent::Book(book) = event(BOOK) else {
        panic!("expected book");
    };
    assert!(book.tick_size.is_none() && book.last_trade_price.is_none());
    assert!(book.timestamp > 1_700_000_000_000);
    assert!(serde_json::from_str::<MarketWsEvent>(&without(BOOK, "hash")).is_err());
}

#[test]
fn parses_price_change() {
    let MarketWsEvent::PriceChange(pc) = event(PRICE_CHANGE) else {
        panic!("expected price_change");
    };
    assert!(!pc.price_changes.is_empty());
    let mut v: Value = serde_json::from_str(PRICE_CHANGE).unwrap();
    v["price_changes"][0]
        .as_object_mut()
        .unwrap()
        .remove("best_bid");
    assert!(serde_json::from_str::<MarketWsEvent>(&v.to_string()).is_err());
}

#[test]
fn parses_trade_tick_and_bba() {
    let MarketWsEvent::LastTradePrice(t) = event(LAST_TRADE) else {
        panic!("expected last_trade_price");
    };
    assert!(matches!(t.side, MarketWsSide::Buy | MarketWsSide::Sell));
    assert!(t.price > 0.0 && t.size > 0.0);
    assert!(serde_json::from_str::<MarketWsEvent>(&without(LAST_TRADE, "size")).is_err());

    let MarketWsEvent::TickSizeChange(tick) = event(TICK) else {
        panic!("expected tick_size_change");
    };
    assert_eq!((tick.old_tick_size, tick.new_tick_size), (0.01, 0.001));

    let MarketWsEvent::BestBidAsk(bba) = event(BBA) else {
        panic!("expected best_bid_ask");
    };
    assert!(bba.best_ask >= bba.best_bid);
    assert!(serde_json::from_str::<MarketWsEvent>(&without(BBA, "spread")).is_err());
}

#[test]
fn parses_lifecycle_events() {
    let MarketWsEvent::NewMarket(m) = event(NEW_MARKET) else {
        panic!("expected new_market");
    };
    assert_eq!(m.assets_ids.len(), m.outcomes.len());
    assert!(m.fee_schedule.is_some());
    assert!(m.event_message.is_some());
    assert!(serde_json::from_str::<MarketWsEvent>(&without(NEW_MARKET, "slug")).is_err());

    let MarketWsEvent::MarketResolved(r) = event(RESOLVED) else {
        panic!("expected market_resolved");
    };
    // Live frames send `"event_message": null`.
    assert!(r.event_message.is_none());
    assert!(r.assets_ids.contains(&r.winning_asset_id));
    assert!(serde_json::from_str::<MarketWsEvent>(&without(RESOLVED, "winning_outcome")).is_err());
}
