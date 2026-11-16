//! Offline parse tests over live websocket captures in `tests/fixtures/ws` (2026-11-16, UK IP).
//! `docs_*` fixtures are the documented examples, used where nothing was pushed live.

use roshar_bybit::{ByBitWsMessage, ByBitWsStream};

macro_rules! fixture {
    ($name:literal) => {
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/ws/",
            $name,
            ".json"
        ))
    };
}

fn parse(stream: ByBitWsStream, text: &str) -> ByBitWsMessage {
    ByBitWsMessage::parse(stream, text).unwrap_or_else(|e| panic!("{e}"))
}

/// Removes `key` from the object at `pointer` and checks parsing then fails.
fn assert_required(stream: ByBitWsStream, text: &str, pointer: &str, key: &str) {
    let mut v: serde_json::Value = serde_json::from_str(text).unwrap();
    v.pointer_mut(pointer)
        .and_then(|o| o.as_object_mut())
        .unwrap_or_else(|| panic!("no object at {pointer}"))
        .remove(key)
        .unwrap_or_else(|| panic!("no key {key} at {pointer}"));
    assert!(
        ByBitWsMessage::parse(stream, &v.to_string()).is_err(),
        "missing {pointer}/{key} must be an error"
    );
}

use ByBitWsStream::{Inverse, Linear, Option as Opt, Spot, Spread, SystemStatus};

#[test]
fn control_frames() {
    match parse(Linear, fixture!("ctrl_linear_subscribe_ok")) {
        ByBitWsMessage::Response(r) => assert!(r.success && r.op == "subscribe"),
        m => panic!("{m:?}"),
    }
    match parse(Linear, fixture!("ctrl_linear_subscribe_fail")) {
        ByBitWsMessage::Response(r) => {
            assert!(!r.success);
            assert!(r.ret_msg.contains("handler not found"));
        }
        m => panic!("{m:?}"),
    }
    match parse(Linear, fixture!("ctrl_linear_unsubscribe_ok")) {
        ByBitWsMessage::Response(r) => assert_eq!(r.op, "unsubscribe"),
        m => panic!("{m:?}"),
    }
    match parse(Linear, fixture!("ctrl_linear_pong")) {
        ByBitWsMessage::Response(r) => assert_eq!(r.ret_msg, "pong"),
        m => panic!("{m:?}"),
    }
    match parse(Spot, fixture!("ctrl_spot_subscribe_ok")) {
        ByBitWsMessage::Response(r) => assert_eq!(r.ret_msg, "subscribe"),
        m => panic!("{m:?}"),
    }
    match parse(Opt, fixture!("ctrl_option_command_resp")) {
        ByBitWsMessage::CommandResponse(r) => {
            assert!(r.data.fail_topics.is_empty());
            assert_eq!(r.data.success_topics.len(), 4);
        }
        m => panic!("{m:?}"),
    }
    match parse(SystemStatus, fixture!("ctrl_misc_command_resp")) {
        ByBitWsMessage::CommandResponse(r) => assert_eq!(r.data.success_topics, ["system.status"]),
        m => panic!("{m:?}"),
    }
    match parse(Opt, fixture!("ctrl_option_pong")) {
        ByBitWsMessage::Pong(p) => assert_eq!(p.args.len(), 1),
        m => panic!("{m:?}"),
    }
    assert_required(Linear, fixture!("ctrl_linear_subscribe_ok"), "", "conn_id");
    assert_required(
        Opt,
        fixture!("ctrl_option_command_resp"),
        "/data",
        "failTopics",
    );
}

#[test]
fn orderbooks() {
    for (stream, text) in [
        (Linear, fixture!("linear_orderbook_1_snapshot")),
        (Linear, fixture!("linear_orderbook_50_snapshot")),
        (Linear, fixture!("linear_orderbook_50_delta")),
        (Linear, fixture!("linear_orderbook_full_delta")),
        (Spot, fixture!("spot_orderbook_full_delta")),
        (Opt, fixture!("option_orderbook_25_snapshot")),
        (Opt, fixture!("option_orderbook_25_delta")),
        (Spread, fixture!("spread_orderbook_snapshot")),
        (Spread, fixture!("spread_orderbook_empty_snapshot")),
    ] {
        match parse(stream, text) {
            ByBitWsMessage::Orderbook(_) => {}
            m => panic!("{m:?}"),
        }
    }
    match parse(Opt, fixture!("option_orderbook_25_snapshot")) {
        ByBitWsMessage::Orderbook(m) => assert!(m.is_full_update() && m.id.is_some()),
        m => panic!("{m:?}"),
    }
    match parse(Linear, fixture!("linear_orderbook_full_delta")) {
        ByBitWsMessage::Orderbook(m) => assert!(!m.is_full_update()),
        m => panic!("{m:?}"),
    }
    for (stream, text) in [
        (Linear, fixture!("linear_orderbook_rpi_snapshot")),
        (Linear, fixture!("linear_orderbook_rpi_delta")),
        (Spot, fixture!("spot_orderbook_rpi_snapshot")),
    ] {
        match parse(stream, text) {
            ByBitWsMessage::OrderbookRpi(m) => assert!(!m.data.b.is_empty()),
            m => panic!("{m:?}"),
        }
    }
    assert_required(
        Linear,
        fixture!("linear_orderbook_50_delta"),
        "/data",
        "seq",
    );
    assert_required(Linear, fixture!("linear_orderbook_rpi_snapshot"), "", "cts");
}

#[test]
fn trades() {
    match parse(Linear, fixture!("linear_public_trade")) {
        ByBitWsMessage::Trades(m) => {
            let t = &m.data[0];
            assert!(t.tick_direction.is_some() && t.is_rpi_trade == Some(false));
            assert!(t.mark_price.is_none());
        }
        m => panic!("{m:?}"),
    }
    let _ = parse(Inverse, fixture!("inverse_public_trade"));
    match parse(Spot, fixture!("spot_public_trade")) {
        ByBitWsMessage::Trades(m) => assert!(m.data[0].tick_direction.is_none()),
        m => panic!("{m:?}"),
    }
    match parse(Opt, fixture!("option_public_trade")) {
        ByBitWsMessage::Trades(m) => {
            let t = &m.data[0];
            assert!(m.id.is_some());
            assert!(t.is_rpi_trade.is_none());
            assert!(t.mark_price.is_some() && t.index_price.is_some());
            assert!(t.mark_iv.is_some() && t.iv.is_some());
        }
        m => panic!("{m:?}"),
    }
    match parse(Spread, fixture!("docs_spread_public_trade")) {
        ByBitWsMessage::Trades(m) => assert!(m.data[0].is_block_trade.is_none()),
        m => panic!("{m:?}"),
    }
    assert_required(Linear, fixture!("linear_public_trade"), "/data/0", "p");
}

#[test]
fn tickers() {
    for text in [
        fixture!("linear_ticker_snapshot"),
        fixture!("linear_ticker_futures_snapshot"),
        fixture!("linear_ticker_delta"),
    ] {
        match parse(Linear, text) {
            ByBitWsMessage::ContractTicker(_) => {}
            m => panic!("{m:?}"),
        }
    }
    match parse(Linear, fixture!("linear_ticker_delta")) {
        ByBitWsMessage::ContractTicker(m) => {
            assert_eq!(m.snapshot_type, "delta");
            assert!(m.data.funding_rate.is_none());
        }
        m => panic!("{m:?}"),
    }
    match parse(Inverse, fixture!("inverse_ticker_snapshot")) {
        ByBitWsMessage::ContractTicker(m) => assert!(m.data.funding_rate.is_some()),
        m => panic!("{m:?}"),
    }
    match parse(Spot, fixture!("spot_ticker")) {
        ByBitWsMessage::SpotTicker(m) => assert!(m.data.usd_index_price.is_some()),
        m => panic!("{m:?}"),
    }
    match parse(Opt, fixture!("option_ticker")) {
        ByBitWsMessage::OptionTicker(m) => assert!(!m.data.mark_price_iv.is_empty()),
        m => panic!("{m:?}"),
    }
    for text in [
        fixture!("spread_ticker"),
        fixture!("spread_ticker_empty_book"),
    ] {
        match parse(Spread, text) {
            ByBitWsMessage::SpreadTicker(_) => {}
            m => panic!("{m:?}"),
        }
    }
    assert_required(Linear, fixture!("linear_ticker_delta"), "/data", "symbol");
    assert_required(Linear, fixture!("linear_ticker_delta"), "", "cs");
    assert_required(Spot, fixture!("spot_ticker"), "/data", "lastPrice");
    assert_required(Opt, fixture!("option_ticker"), "/data", "delta");
    assert_required(Spread, fixture!("spread_ticker"), "/data", "bidPrice");
    // An option ticker frame is not a spot ticker.
    assert!(ByBitWsMessage::parse(Spot, fixture!("option_ticker")).is_err());
}

#[test]
fn klines() {
    for (stream, text) in [
        (Linear, fixture!("linear_kline")),
        (Spot, fixture!("spot_kline")),
    ] {
        match parse(stream, text) {
            ByBitWsMessage::Kline(m) => assert_eq!(m.data[0].interval, "1"),
            m => panic!("{m:?}"),
        }
    }
    assert_required(Spot, fixture!("spot_kline"), "/data/0", "confirm");
}

#[test]
fn liquidation_insurance_adl_price_limit_system_status() {
    match parse(Linear, fixture!("linear_all_liquidation")) {
        ByBitWsMessage::Liquidation(m) => assert_eq!(m.data.len(), 2),
        m => panic!("{m:?}"),
    }
    assert_required(Linear, fixture!("linear_all_liquidation"), "/data/0", "p");

    for (stream, text) in [
        (Linear, fixture!("linear_insurance_snapshot")),
        (Inverse, fixture!("inverse_insurance_snapshot")),
    ] {
        match parse(stream, text) {
            ByBitWsMessage::InsurancePool(m) => assert!(!m.data.is_empty()),
            m => panic!("{m:?}"),
        }
    }
    assert_required(
        Inverse,
        fixture!("inverse_insurance_snapshot"),
        "/data/0",
        "updateTime",
    );

    for (stream, text) in [
        (Linear, fixture!("linear_adl_alert_snapshot")),
        (Linear, fixture!("linear_adl_alert_usdc_snapshot")),
        (Inverse, fixture!("inverse_adl_alert_delta")),
    ] {
        match parse(stream, text) {
            ByBitWsMessage::AdlAlert(m) => assert_eq!(m.data[0].mb, ""),
            m => panic!("{m:?}"),
        }
    }
    assert_required(
        Inverse,
        fixture!("inverse_adl_alert_delta"),
        "/data/0",
        "adl_sr",
    );

    for (stream, text) in [
        (Linear, fixture!("linear_price_limit")),
        (Spot, fixture!("spot_price_limit")),
    ] {
        match parse(stream, text) {
            ByBitWsMessage::PriceLimit(m) => assert!(!m.data.buy_lmt.is_empty()),
            m => panic!("{m:?}"),
        }
    }
    assert_required(Linear, fixture!("linear_price_limit"), "/data", "sellLmt");

    match parse(SystemStatus, fixture!("docs_system_status")) {
        ByBitWsMessage::SystemStatus(m) => assert_eq!(m.data[0].state, "completed"),
        m => panic!("{m:?}"),
    }
    assert_required(
        SystemStatus,
        fixture!("docs_system_status"),
        "/data/0",
        "env",
    );
}
