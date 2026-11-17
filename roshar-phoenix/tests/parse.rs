//! Offline parse tests against live payloads captured on 2026-11-15 from
//! `https://perp-api.phoenix.trade` and `wss://perp-api.phoenix.trade/v1/ws`.
//! Large arrays in the fixtures were truncated; no field values were edited.

use roshar_phoenix::types::*;
use serde::de::DeserializeOwned;
use serde_json::Value;

fn fixture(name: &str) -> Value {
    let path = format!("{}/tests/fixtures/{name}.json", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn parse<T: DeserializeOwned>(v: Value) -> Result<T, serde_json::Error> {
    serde_json::from_value(v)
}

fn parse_fixture<T: DeserializeOwned>(name: &str) -> T {
    parse(fixture(name)).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn ws_fixture(name: &str) -> PhoenixWsServerMessage {
    parse_fixture(name)
}

/// Removes `key` from the object at JSON pointer `at` and returns the modified value.
fn without(mut v: Value, at: &str, key: &str) -> Value {
    v.pointer_mut(at)
        .and_then(Value::as_object_mut)
        .unwrap_or_else(|| panic!("no object at {at}"))
        .remove(key)
        .unwrap_or_else(|| panic!("no key {key} at {at}"));
    v
}

fn with(mut v: Value, at: &str, value: Value) -> Value {
    *v.pointer_mut(at)
        .unwrap_or_else(|| panic!("no value at {at}")) = value;
    v
}

// ── Live payloads parse ───────────────────────────────────────────────────────

#[test]
fn rest_exchange() {
    let ex: PhoenixExchange = parse_fixture("exchange");
    assert!(!ex.keys.global_config.is_empty());
    assert!(!ex.markets.is_empty());
}

#[test]
fn rest_markets() {
    let markets: Vec<PhoenixMarketConfig> = parse_fixture("markets");
    let sol = markets.iter().find(|m| m.symbol == "SOL").unwrap();
    assert_eq!(sol.market_status, PhoenixMarketStatus::Active);
    assert!(sol.open_interest_cap_base_lots > 0);
    assert!(sol.commodity_metadata.is_none());
    let commodity = markets
        .iter()
        .find(|m| m.commodity_metadata.is_some())
        .expect("fixture has a commodity market");
    assert!(commodity.commodity_metadata.as_ref().unwrap().is_commodity);
    assert!(markets.iter().any(|m| m.symbol == "kSHIB"));
}

#[test]
fn rest_market() {
    let m: PhoenixMarketConfig = parse_fixture("market");
    assert_eq!(m.symbol, "kSHIB");
}

#[test]
fn rest_orderbook() {
    let book: PhoenixOrderBook = parse_fixture("orderbook");
    assert_eq!(book.symbol, "SOL");
    assert!(!book.bids.is_empty() && !book.asks.is_empty());
    assert!(book.bids[0][0] < book.asks[0][0]);
}

#[test]
fn rest_candles() {
    let candles: Vec<PhoenixCandle> = parse_fixture("candles");
    assert!(!candles.is_empty());
    // REST candle times are unix milliseconds.
    assert!(candles[0].time > 1_000_000_000_000);
}

#[test]
fn rest_trader_state() {
    let s: PhoenixTraderState = parse_fixture("trader_state");
    let subs = &s.snapshot.subaccounts;
    assert!(subs.iter().any(|a| !a.positions.is_empty()));
    assert!(subs.iter().any(|a| a.positions.is_empty()));
    let with_pos = subs.iter().find(|a| !a.positions.is_empty()).unwrap();
    let margin = with_pos.margin.as_ref().expect("margin present");
    assert!(margin.positions.contains_key(&with_pos.positions[0].symbol));
}

#[test]
fn rest_trader_state_uninitialized_pda() {
    let s: PhoenixTraderState = parse_fixture("trader_state_empty");
    assert_eq!(s.trader_pda_index, 1);
    assert_eq!(
        s.snapshot.capabilities.state,
        PhoenixTraderActivityState::Uninitialized
    );
}

#[test]
fn rest_pnl() {
    let pnl: Vec<PhoenixPnlPoint> = parse_fixture("pnl");
    assert!(!pnl.is_empty());
    assert!(pnl[0].end_time > pnl[0].start_time);
}

#[test]
fn rest_order_history() {
    let r: PhoenixPaginatedResponse<PhoenixOrderHistory> = parse_fixture("orders");
    assert!(!r.data.is_empty());
    assert!(r.next_cursor.is_some());
}

#[test]
fn rest_trades_history() {
    let r: PhoenixPaginatedResponse<PhoenixTradeHistory> = parse_fixture("trades");
    assert!(!r.data.is_empty());
}

#[test]
fn rest_funding_history() {
    let r: PhoenixFundingHistoryResponse = parse_fixture("funding");
    assert!(!r.events.is_empty());
}

#[test]
fn rest_collateral_history() {
    let r: PhoenixPaginatedResponse<PhoenixCollateralEvent> = parse_fixture("collateral");
    assert!(!r.data.is_empty());
}

#[test]
fn ws_all_mids() {
    let m: PhoenixAllMids = parse_fixture("ws_allMids");
    assert!(m.mids.contains_key("SOL"));
}

#[test]
fn ws_funding_rate() {
    let f: PhoenixFundingRate = parse_fixture("ws_fundingRate");
    assert_eq!(f.symbol, "SOL");
}

#[test]
fn ws_market_stats() {
    let m: PhoenixMarketStats = parse_fixture("ws_market");
    assert!(m.mark_price > 0.0);
    assert!(m.mid_price.is_some());
}

#[test]
fn ws_orderbook() {
    let b: PhoenixL2BookUpdate = parse_fixture("ws_orderbook");
    assert_eq!(b.symbol, "SOL");
    // The server omits bypassExecutionBand on default subscriptions.
    assert!(!b.bypass_execution_band);
}

#[test]
fn ws_trades() {
    let t: PhoenixTradesMessage = parse_fixture("ws_trades");
    assert!(!t.trades.is_empty());
    assert!(t.trades[0].slot > 0);
}

#[test]
fn ws_candle() {
    let c: PhoenixCandleUpdate = parse_fixture("ws_candle");
    // WebSocket candle times are unix seconds.
    assert!(c.candle.time < 10_000_000_000);
}

// ── Strictness: documented-required fields must be present ─────────────────────

#[test]
fn market_config_requires_tick_size() {
    let v = without(fixture("market"), "", "tickSize");
    assert!(parse::<PhoenixMarketConfig>(v).is_err());
}

#[test]
fn market_config_rejects_unknown_status() {
    let v = with(fixture("market"), "/marketStatus", "bogus".into());
    assert!(parse::<PhoenixMarketConfig>(v).is_err());
}

#[test]
fn market_config_optional_metadata_may_be_absent() {
    let v = without(fixture("market"), "", "metadata");
    assert!(parse::<PhoenixMarketConfig>(v).is_ok());
}

#[test]
fn js_safe_ints_accept_string_or_number_only() {
    let v = with(fixture("market"), "/openInterestCapBaseLots", 42.into());
    assert_eq!(
        parse::<PhoenixMarketConfig>(v)
            .unwrap()
            .open_interest_cap_base_lots,
        42
    );
    let v = with(fixture("market"), "/openInterestCapBaseLots", "abc".into());
    assert!(parse::<PhoenixMarketConfig>(v).is_err());
}

#[test]
fn exchange_keys_require_global_vault() {
    let v = without(fixture("exchange"), "/keys", "globalVault");
    assert!(parse::<PhoenixExchange>(v).is_err());
}

#[test]
fn orderbook_requires_slot() {
    let v = without(fixture("orderbook"), "", "slot");
    assert!(parse::<PhoenixOrderBook>(v).is_err());
}

#[test]
fn candle_requires_close_but_not_volume() {
    let v = without(fixture("candles"), "/0", "close");
    assert!(parse::<Vec<PhoenixCandle>>(v).is_err());
    let v = without(fixture("candles"), "/0", "volume");
    assert!(parse::<Vec<PhoenixCandle>>(v).is_ok());
}

#[test]
fn trader_state_subaccount_requires_collateral() {
    let v = without(
        fixture("trader_state"),
        "/snapshot/subaccounts/0",
        "collateral",
    );
    assert!(parse::<PhoenixTraderState>(v).is_err());
}

#[test]
fn trader_state_margin_requires_risk_state() {
    let v = without(
        fixture("trader_state"),
        "/snapshot/subaccounts/0/margin",
        "riskState",
    );
    assert!(parse::<PhoenixTraderState>(v).is_err());
}

#[test]
fn position_requires_base_position_lots() {
    let v = without(
        fixture("trader_state"),
        "/snapshot/subaccounts/0/positions/0",
        "basePositionLots",
    );
    assert!(parse::<PhoenixTraderState>(v).is_err());
}

#[test]
fn pnl_requires_cumulative_pnl() {
    let v = without(fixture("pnl"), "/0", "cumulativePnl");
    assert!(parse::<Vec<PhoenixPnlPoint>>(v).is_err());
}

#[test]
fn paginated_requires_has_more_not_cursor() {
    let v = without(fixture("orders"), "", "hasMore");
    assert!(parse::<PhoenixPaginatedResponse<PhoenixOrderHistory>>(v).is_err());
    let v = without(fixture("orders"), "", "nextCursor");
    assert!(parse::<PhoenixPaginatedResponse<PhoenixOrderHistory>>(v).is_ok());
}

#[test]
fn order_history_rejects_unknown_side() {
    let v = with(fixture("orders"), "/data/0/side", "buy".into());
    assert!(parse::<PhoenixPaginatedResponse<PhoenixOrderHistory>>(v).is_err());
}

#[test]
fn trade_history_requires_trade_type() {
    let v = without(fixture("trades"), "/data/0", "tradeType");
    assert!(parse::<PhoenixPaginatedResponse<PhoenixTradeHistory>>(v).is_err());
}

#[test]
fn funding_history_requires_funding_payment() {
    let v = without(fixture("funding"), "/events/0", "fundingPayment");
    assert!(parse::<PhoenixFundingHistoryResponse>(v).is_err());
}

#[test]
fn collateral_event_requires_amount() {
    let v = without(fixture("collateral"), "/data/0", "amount");
    assert!(parse::<PhoenixPaginatedResponse<PhoenixCollateralEvent>>(v).is_err());
}

#[test]
fn all_mids_requires_slot() {
    let v = without(fixture("ws_allMids"), "", "slot");
    assert!(parse::<PhoenixAllMids>(v).is_err());
}

#[test]
fn market_stats_requires_mark_px_but_not_mid_px() {
    let v = without(fixture("ws_market"), "", "markPx");
    assert!(parse::<PhoenixMarketStats>(v).is_err());
    let v = without(fixture("ws_market"), "", "midPx");
    assert!(parse::<PhoenixMarketStats>(v).unwrap().mid_price.is_none());
}

#[test]
fn trade_event_requires_taker() {
    let v = without(fixture("ws_trades"), "/trades/0", "taker");
    assert!(parse::<PhoenixTradesMessage>(v).is_err());
}

// ── Client messages ───────────────────────────────────────────────────────────

#[test]
fn subscription_messages_match_docs() {
    assert_eq!(
        PhoenixWssMessage::orderbook("SOL", None).to_json(),
        r#"{"type":"subscribe","subscription":{"channel":"orderbook","symbol":"SOL"}}"#
    );
    assert_eq!(
        PhoenixWssMessage::orderbook_unsub("SOL", Some(true)).to_json(),
        r#"{"type":"unsubscribe","subscription":{"channel":"orderbook","symbol":"SOL","bypassExecutionBand":true}}"#
    );
    assert_eq!(
        PhoenixWssMessage::exchange(Some("json")).to_json(),
        r#"{"type":"subscribe","subscription":{"channel":"exchange","encoding":"json"}}"#
    );
    assert_eq!(
        PhoenixWssMessage::exchange(None).to_json(),
        r#"{"type":"subscribe","subscription":{"channel":"exchange"}}"#
    );
    assert_eq!(
        PhoenixWssMessage::candles_unsub("SOL", "1m").to_json(),
        r#"{"type":"unsubscribe","subscription":{"channel":"candles","symbol":"SOL","timeframe":"1m"}}"#
    );
    assert_eq!(
        PhoenixWssMessage::trader_state("wallet", 0).to_json(),
        r#"{"type":"subscribe","subscription":{"channel":"traderState","authority":"wallet","traderPdaIndex":0}}"#
    );
    assert_eq!(
        PhoenixWssMessage::all_mids().to_json(),
        r#"{"type":"subscribe","subscription":{"channel":"allMids"}}"#
    );
}

// ── Exchange status / keys / snapshot (captured 2026-11-16) ─────────────────────

#[test]
fn rest_exchange_keys() {
    let k: PhoenixExchangeKeys = parse_fixture("exchange_keys");
    assert!(!k.global_vault.is_empty());
    let v = without(fixture("exchange_keys"), "", "withdrawQueue");
    assert!(parse::<PhoenixExchangeKeys>(v).is_err());
}

#[test]
fn rest_exchange_status() {
    let s: PhoenixExchangeStatus = parse_fixture("exchange_status");
    assert_eq!(s.running_state, Some(PhoenixExchangeRunningState::Active));
    let v = without(fixture("exchange_status"), "", "gated");
    assert!(parse::<PhoenixExchangeStatus>(v).is_err());
    let v = without(fixture("exchange_status"), "", "withdrawalsAvailable");
    assert!(parse::<PhoenixExchangeStatus>(v).is_ok());
}

#[test]
fn rest_exchange_snapshot() {
    let s: PhoenixExchangeSnapshot = parse_fixture("exchange_snapshot");
    assert!(s.sequence_number.is_none());
    assert_eq!(
        s.exchange.running_state,
        PhoenixExchangeRunningState::Active
    );
    let sol = s.markets.iter().find(|m| m.symbol == "SOL").unwrap();
    assert!(sol.commodity_metadata.is_none());
    assert_eq!(sol.funding_config.funding_interval_seconds, 3600);
    let banded = s
        .markets
        .iter()
        .filter_map(|m| m.commodity_metadata.as_ref())
        .find(|c| c.mark_price_band.is_some())
        .expect("fixture has an after-hours commodity market");
    assert!(banded.last_known_index_price.is_some());
    let unbanded = s
        .markets
        .iter()
        .filter_map(|m| m.commodity_metadata.as_ref())
        .find(|c| c.mark_price_band.is_none())
        .expect("fixture has an in-session commodity market");
    assert!(unbanded.last_index_expiry_timestamp.is_none());
    // A market whose metadata has null fields.
    assert!(s
        .markets
        .iter()
        .any(|m| m.metadata.as_ref().is_some_and(|md| md.name.is_none())));
}

#[test]
fn exchange_snapshot_strictness() {
    let v = without(
        fixture("exchange_snapshot"),
        "/markets/0",
        "markPriceParameters",
    );
    assert!(parse::<PhoenixExchangeSnapshot>(v).is_err());
    let v = without(fixture("exchange_snapshot"), "/exchange", "usdcMint");
    assert!(parse::<PhoenixExchangeSnapshot>(v).is_err());
    let v = without(
        fixture("exchange_snapshot"),
        "/markets/0/fundingConfig",
        "fundingPeriodSeconds",
    );
    assert!(parse::<PhoenixExchangeSnapshot>(v).is_err());
    let v = with(
        fixture("exchange_snapshot"),
        "/markets/0/markPriceParameters/riskActionPriceValidityRules/0/0/0",
        "bogus".into(),
    );
    assert!(parse::<PhoenixExchangeSnapshot>(v).is_err());
}

// ── Orderbook splines ──────────────────────────────────────────────────────────

#[test]
fn rest_orderbook_with_splines() {
    let b: PhoenixOrderBook = parse_fixture("orderbook_splines");
    let splines = b.splines.expect("requested with includeSplines=true");
    assert!(!splines.is_empty());
    // The plain orderbook fixture has no splines key.
    let plain: PhoenixOrderBook = parse_fixture("orderbook");
    assert!(plain.splines.is_none());
    let v = without(fixture("orderbook_splines"), "/splines/0", "traderKey");
    assert!(parse::<PhoenixOrderBook>(v).is_err());
}

// ── Mark price, stats, funding, fills ─────────────────────────────────────────

#[test]
fn rest_mark_price() {
    let m: PhoenixMarkPrice = parse_fixture("mark_price");
    assert!(m.mark_price.unwrap().price > 0.0);
    let v = with(fixture("mark_price"), "/markPrice", Value::Null);
    assert!(parse::<PhoenixMarkPrice>(v).unwrap().mark_price.is_none());
    let v = without(fixture("mark_price"), "", "markPrice");
    assert!(parse::<PhoenixMarkPrice>(v).is_err());
}

#[test]
fn rest_market_stats() {
    let raw: PhoenixMarketStatsHistory = parse_fixture("market_stats");
    assert!(raw.timeframe.is_none());
    assert!(!raw.stats.is_empty());
    let agg: PhoenixMarketStatsHistory = parse_fixture("market_stats_1h");
    assert_eq!(agg.timeframe.as_deref(), Some("1h"));
    let v = without(fixture("market_stats"), "", "timeframe");
    assert!(parse::<PhoenixMarketStatsHistory>(v).is_err());
    let v = without(fixture("market_stats"), "/stats/0", "mark_price");
    assert!(parse::<PhoenixMarketStatsHistory>(v).is_err());
}

#[test]
fn rest_funding_rates() {
    let r: PhoenixFundingRateHistory = parse_fixture("funding_rates");
    // Unix seconds, not the documented ISO string.
    assert!(r.rates[0].timestamp < 10_000_000_000);
    let v = without(
        fixture("funding_rates"),
        "/rates/0",
        "fundingRatePercentage",
    );
    assert!(parse::<PhoenixFundingRateHistory>(v).is_err());
}

#[test]
fn rest_funding_overview() {
    let o: PhoenixFundingOverview = parse_fixture("funding_overview");
    assert!(!o.series.is_empty() && !o.series[0].points.is_empty());
    let v = without(
        fixture("funding_overview"),
        "/series/0/points/0",
        "markPrice",
    );
    assert!(parse::<PhoenixFundingOverview>(v).is_err());
}

#[test]
fn rest_market_fills() {
    let f: PhoenixPaginatedResponse<PhoenixFill> = parse_fixture("fills");
    assert!(f.has_more && f.next_cursor.is_some() && f.prev_cursor.is_none());
    assert_eq!(f.data[0].market_symbol, "SOL");
    let v = without(fixture("fills"), "/data/0", "transactionSignature");
    assert!(parse::<PhoenixPaginatedResponse<PhoenixFill>>(v).is_err());
}

// ── Calendars ─────────────────────────────────────────────────────────────────

#[test]
fn rest_market_calendar() {
    let c: PhoenixMarketCalendarResponse = parse_fixture("market_calendar");
    assert!(c.calendar.unwrap().weekly_schedule.contains_key("Mon"));
    let v = without(fixture("market_calendar"), "", "calendar");
    assert!(parse::<PhoenixMarketCalendarResponse>(v).is_err());
    let v = without(
        fixture("market_calendar"),
        "/calendar/weeklySchedule/Mon/sessions/0",
        "mode",
    );
    assert!(parse::<PhoenixMarketCalendarResponse>(v).is_err());
    // Crypto markets have no calendar: 404 body is not a calendar.
    assert!(
        parse::<PhoenixMarketCalendarResponse>(fixture("market_calendar_not_configured")).is_err()
    );
}

#[test]
fn rest_market_calendar_record() {
    let c: PhoenixMarketCalendarRecord = parse_fixture("market_calendar_record");
    assert_eq!(c.market_calendar_id, "cme_commodities");
    let v = without(fixture("market_calendar_record"), "", "s3Path");
    assert!(parse::<PhoenixMarketCalendarRecord>(v).is_err());
}

#[test]
fn rest_commodity_calendar() {
    let c: PhoenixCommodityCalendar = parse_fixture("commodity_calendar");
    assert!(!c.calendar.date_overrides.is_empty());
    let v = without(fixture("commodity_calendar"), "/calendar", "dateOverrides");
    assert!(parse::<PhoenixCommodityCalendar>(v).is_err());
}

#[test]
fn rest_calendar_transitions() {
    let t: PhoenixNextMarketCalendarTransition = parse_fixture("next_market_calendar_transition");
    assert_eq!(t.current_state, PhoenixCalendarState::Open);
    assert_eq!(t.next_market_state, Some(PhoenixCalendarState::AfterHours));
    let t: PhoenixNextCommodityMarketTransition = parse_fixture("next_commodity_market_transition");
    assert!(t.utc_next_transition.is_some());
    let v = with(
        fixture("next_commodity_market_transition"),
        "/currentState",
        "closed".into(),
    );
    assert!(parse::<PhoenixNextCommodityMarketTransition>(v).is_err());
    let v = without(
        fixture("next_market_calendar_transition"),
        "",
        "utcNextTransition",
    );
    assert!(parse::<PhoenixNextMarketCalendarTransition>(v).is_err());
}

// ── WebSocket server-message enum (captured 2026-11-16) ─────────────────────────

#[test]
fn ws_enum_routes_every_channel() {
    for name in [
        "ws_allMids",
        "ws_fundingRate",
        "ws_orderbook",
        "ws_market",
        "ws_trades",
        "ws_candle",
    ] {
        parse::<PhoenixWsServerMessage>(fixture(name)).unwrap_or_else(|e| panic!("{name}: {e}"));
    }
    assert!(matches!(
        parse_fixture::<PhoenixWsServerMessage>("ws_candle"),
        PhoenixWsServerMessage::Candle(_)
    ));
    let v = with(fixture("ws_market"), "/channel", "bogus".into());
    assert!(parse::<PhoenixWsServerMessage>(v).is_err());
}

#[test]
fn ws_orderbook_bypass() {
    let PhoenixWsServerMessage::Orderbook(b) = ws_fixture("ws_orderbook_bypass") else {
        panic!("not an orderbook frame");
    };
    assert!(b.bypass_execution_band);
}

#[test]
fn ws_subscription_status() {
    let PhoenixWsServerMessage::SubscriptionStatus(s) = ws_fixture("ws_subscription_status") else {
        panic!("not a subscriptionStatus frame");
    };
    assert_eq!(s.status, PhoenixSubscriptionState::Subscribed);
    assert_eq!(s.subscription.channel, "exchange");
    assert_eq!(s.subscription.encoding.as_deref(), Some("json"));
    let v = without(fixture("ws_subscription_status"), "", "clientId");
    assert!(parse::<PhoenixWsServerMessage>(v).is_err());
    let v = with(
        fixture("ws_subscription_status"),
        "/status",
        "pending".into(),
    );
    assert!(parse::<PhoenixWsServerMessage>(v).is_err());
}

#[test]
fn ws_errors() {
    let PhoenixWsServerMessage::Error(e) = ws_fixture("ws_error_subscription") else {
        panic!("not an error frame");
    };
    assert_eq!(e.code, 400);
    assert_eq!(e.subscription.unwrap().channel, "exchange");
    let PhoenixWsServerMessage::Error(e) = ws_fixture("ws_error_plain") else {
        panic!("not an error frame");
    };
    assert!(e.subscription.is_none());
    let v = without(fixture("ws_error_plain"), "", "error");
    assert!(parse::<PhoenixWsServerMessage>(v).is_err());
}

#[test]
fn ws_exchange_snapshot() {
    let PhoenixWsServerMessage::Exchange(PhoenixExchangeMessage::Snapshot(s)) =
        ws_fixture("ws_exchange_snapshot")
    else {
        panic!("not an exchange snapshot");
    };
    assert!(s.sequence_number > 0);
    assert_eq!(s.markets.len(), 4);
    let v = without(fixture("ws_exchange_snapshot"), "", "sequenceNumber");
    assert!(parse::<PhoenixWsServerMessage>(v).is_err());
    let v = without(fixture("ws_exchange_snapshot"), "/markets/0", "tickSize");
    assert!(parse::<PhoenixWsServerMessage>(v).is_err());
}

#[test]
fn ws_exchange_encoded_snapshot() {
    let PhoenixWsServerMessage::Exchange(PhoenixExchangeMessage::EncodedSnapshot(s)) =
        ws_fixture("ws_exchange_encoded_snapshot")
    else {
        panic!("not an encoded exchange snapshot");
    };
    assert_eq!(s.encoding, "base64+zstd");
    let v = without(fixture("ws_exchange_encoded_snapshot"), "", "payload");
    assert!(parse::<PhoenixWsServerMessage>(v).is_err());
}

/// No delta was observed live during capture; this is the documented example
/// (https://docs.phoenix.trade/api/websocket), minus its abbreviated `marketAdded` op.
#[test]
fn ws_exchange_delta_docs_example() {
    let doc = serde_json::json!({
        "channel": "exchange",
        "messageType": "delta",
        "version": 1,
        "sequenceNumber": "124",
        "slot": 123456790,
        "slotIndex": 0,
        "ops": [{
            "kind": "marketClosed",
            "symbol": "SOL",
            "previous_market_status": "active",
            "finalized_mark_price": "150000000"
        }]
    });
    let PhoenixWsServerMessage::Exchange(PhoenixExchangeMessage::Delta(d)) =
        parse::<PhoenixWsServerMessage>(doc.clone()).unwrap()
    else {
        panic!("not an exchange delta");
    };
    assert_eq!(d.sequence_number, 124);
    assert!(matches!(
        &d.ops[0],
        PhoenixExchangeDeltaOp::MarketClosed {
            finalized_mark_price: 150000000,
            ..
        }
    ));
    let v = without(doc.clone(), "/ops/0", "finalized_mark_price");
    assert!(parse::<PhoenixWsServerMessage>(v).is_err());
    let v = with(doc, "/ops/0/kind", "bogus".into());
    assert!(parse::<PhoenixWsServerMessage>(v).is_err());
}
