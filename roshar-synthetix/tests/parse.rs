//! Offline parse tests. `fixtures/*.json` without a `docs_` prefix are live captures from
//! papi.synthetix.io (2026-11-15, UK IP); `docs_*.json` are the response examples from
//! developers.synthetix.io (no funded account to capture private responses).

use roshar_synthetix::types::trading::{
    AccountTrades, CancelAllResult, DelegatedSigner, DelegatedSigners, FundingPayments,
    HistoricalOrder, ModifyOrderResult, OpenOrder, OrderStatus, OrderStatuses, Position,
    RateLimits, RemovedAllDelegatedSigners, RemovedDelegatedSigner, ScheduleCancelResult,
    SubAccount, SubAccountUpdate, UpdateLeverageResult,
};
use roshar_synthetix::types::*;
use serde::de::DeserializeOwned;
use std::collections::HashMap;

fn fixture(name: &str) -> String {
    let path = format!("{}/tests/fixtures/{name}.json", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn ok<T: DeserializeOwned>(name: &str) -> T {
    serde_json::from_str::<SynthetixEnvelope<T>>(&fixture(name))
        .unwrap_or_else(|e| panic!("{name}: {e}"))
        .into_result()
        .unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn err(name: &str) -> SynthetixApiError {
    serde_json::from_str::<SynthetixEnvelope<serde_json::Value>>(&fixture(name))
        .unwrap_or_else(|e| panic!("{name}: {e}"))
        .into_result()
        .expect_err(name)
}

fn plain<T: DeserializeOwned>(name: &str) -> T {
    serde_json::from_str(&fixture(name)).unwrap_or_else(|e| panic!("{name}: {e}"))
}

// ── Live: POST /v1/info ─────────────────────────────────────────────────────

#[test]
fn markets() {
    let m: Vec<SynthetixMarket> = ok("markets");
    assert!(m.len() >= 10);
    let btc = m.iter().find(|m| m.symbol == "BTC-USDT").unwrap();
    assert_eq!(btc.settle_asset, "USDT");
    assert!(!btc.maintenance_margin_tiers.is_empty());
    let last = m[0].maintenance_margin_tiers.last().unwrap();
    assert_eq!(last.max_position_size, "");
}

#[test]
fn market_prices() {
    let p: HashMap<String, SynthetixMarketPrice> = ok("market_prices");
    let btc = &p["BTC-USDT"];
    assert_eq!(btc.symbol, "BTC-USDT");
    assert_eq!(btc.timestamp, 1794779923149);
    assert_eq!(
        btc.period_stats.as_ref().unwrap()["24h"].reference_price,
        "64471"
    );
    assert_eq!(btc.on_hours, None);
}

#[test]
fn orderbook() {
    let b: SynthetixOrderBook = ok("orderbook");
    assert_eq!(b.bids.len(), 5);
    assert_eq!(b.bids[0], SynthetixLevel("65030".into(), "0.20423".into()));
    assert_eq!(b.asks[0].0, "65096");
}

#[test]
fn last_trades() {
    let t: SynthetixLastTrades = ok("last_trades");
    assert_eq!(t.trades.len(), 3);
    assert_eq!(t.trades[0].trade_id, "2122086887672799232");
    assert!(!t.trades[0].is_maker);
}

#[test]
fn funding() {
    let f: SynthetixFundingRate = ok("funding_rate");
    assert_eq!(f.funding_interval, 3_600_000);
    assert_eq!(f.next_funding_time, 1794780000000);

    let h: SynthetixFundingRateHistory = ok("funding_history");
    assert_eq!(h.symbol, "BTC-USDT");
    assert_eq!(h.funding_rates.len(), 3);
    assert_eq!(h.funding_rates[0].funding_rate, "0.00001236");
}

#[test]
fn candles() {
    let c: SynthetixCandles = ok("candles");
    assert_eq!(c.interval, "1h");
    assert_eq!(c.candles.len(), 3);
    assert_eq!(c.candles[0].trade_count, 169);
}

#[test]
fn open_interest_mids_collaterals_tiers() {
    let oi: Vec<SynthetixOpenInterest> = ok("open_interest");
    assert!(oi.iter().all(|o| o.error_message.is_none()));
    let mids: HashMap<String, String> = ok("mids");
    assert!(mids.contains_key("BTC-USDT"));
    let c: Vec<SynthetixCollateralConfig> = ok("collaterals");
    assert!(c.iter().any(|c| c.collateral == "USDT"));
    let t: Vec<SynthetixTier> = ok("tiers");
    assert_eq!(t[0].tier_id, "tier_0");
    assert_eq!(t[0].max_total_orders, 50);
}

#[test]
fn accounts() {
    assert!(ok::<bool>("whitelisted"));
    let ids: Vec<String> = ok("subaccount_ids");
    assert_eq!(ids, ["101", "102", "103"]);
    let d: SynthetixSubAccountIdsWithDelegations = ok("subaccount_ids_delegations");
    assert_eq!(d.sub_account_ids.len(), 3);
    assert!(d.delegated_sub_account_ids.is_empty());
}

#[test]
fn info_errors() {
    let e = err("bad_action");
    assert_eq!(e.code, "VALIDATION_ERROR");
    assert_eq!(e.category.as_deref(), Some("REQUEST"));
    assert_eq!(e.message, "Invalid request type");
    let e = err("bad_symbol");
    assert_eq!(e.message, "unknown market");
}

// ── Live: GET endpoints ─────────────────────────────────────────────────────

#[test]
fn get_endpoints() {
    let c: Vec<SynthetixContract> = plain("contracts");
    let btc = c.iter().find(|c| c.ticker_id == "BTC-USDT").unwrap();
    assert_eq!(btc.product_type, "Perpetual");
    assert_eq!(btc.next_funding_rate_timestamp, 1794780000);

    let s: Vec<SynthetixContractSpec> = plain("contract_specs");
    assert!(s.iter().all(|s| s.contract_type == "Vanilla"));

    let b: SynthetixAggregatorOrderBook = plain("agg_orderbook");
    assert_eq!(b.ticker_id, "BTC-USDT");
    assert_eq!(b.bids.len(), 5);

    let e: SynthetixPlainError = plain("agg_orderbook_error");
    assert_eq!(e.error, "unknown ticker_id: NOPE");

    let st: SynthetixStatus = plain("status");
    assert!(!st.maintenance);
    assert!(st.message.is_none());

    let x: SynthetixExchangeStatus = ok("exchange_status");
    assert_eq!(x.exchange_status, "RUNNING");
    let x: SynthetixExchangeStatus = ok("ws_exchange_status");
    assert_eq!(x.exchange_status, "RUNNING");
    assert_eq!(x.timestamp_ms, 1794826921676);
}

// ── Live: info websocket ────────────────────────────────────────────────────

#[test]
fn ws_orderbook() {
    let s: SynthetixWsOrderBook = plain("ws_orderbook_snapshot");
    assert_eq!(s.kind.as_deref(), Some("snapshot"));
    assert_eq!(s.prev_meseq, None);
    assert_eq!(s.data.bids.len(), 10);

    let d: SynthetixWsOrderBook = plain("ws_orderbook_diff");
    assert_eq!(d.kind.as_deref(), Some("diff"));
    assert_eq!(d.prev_meseq, Some(s.meseq));
    assert!(d.data.bids.iter().any(|l| l.quantity == "0"));

    let m: SynthetixWsOrderBook = plain("ws_orderbook_snapshot_mode");
    assert_eq!(m.kind, None);
    assert_eq!(m.data.symbol, "ETH-USDT");
}

#[test]
fn ws_pushes() {
    let t: SynthetixWsTrade = plain("ws_trade");
    assert_eq!(t.data.symbol, "XRP-USDT");
    assert_eq!(t.meseq, Some(496375));

    let p: SynthetixWsMarketPrice = plain("ws_market_price_update");
    assert_eq!(p.is_snapshot, Some(true));
    assert_eq!(p.data.update_type, "index");
    let p: SynthetixWsMarketPrice = plain("ws_market_price_update_all");
    assert_eq!(p.is_snapshot, None);
    assert_eq!(p.data.funding_rate, None);
    let p: SynthetixWsMarketPrice = plain("ws_market_price_update_mark");
    assert_eq!(p.data.update_type, "mark");
    assert_eq!(
        p.data.funding_rate.as_deref(),
        Some("0.0000123661257023817")
    );
    assert_eq!(p.data.next_funding_time, Some(1794830400000));

    let c: SynthetixWsCandle = plain("ws_candle_update");
    assert_eq!(c.data.timeframe, "1m");
    assert_eq!(c.data.trade_count, 3);
}

#[test]
fn ws_replies() {
    let r: SynthetixWsReply = plain("ws_subscribed_orderbook");
    assert_eq!(r.status, 200);
    assert_eq!(r.result.as_ref().unwrap()["status"], "subscribed");
    let r: SynthetixWsReply = plain("ws_subscribed_candles");
    assert_eq!(r.result.as_ref().unwrap()["timeframe"], "1m");
    let r: SynthetixWsReply = plain("ws_unsubscribed_orderbook");
    assert_eq!(r.result.as_ref().unwrap()["status"], "unsubscribed");

    let r: SynthetixWsReply = plain("ws_subscribe_error");
    let e = r.error.unwrap();
    assert_eq!((e.code, e.error_code.as_str()), (400, "VALIDATION_ERROR"));

    let r: SynthetixWsReply = plain("ws_auth_error");
    assert_eq!(r.error.unwrap().error_code, "UNAUTHORIZED");

    let r: SynthetixWsReply = plain("ws_post_get_mids");
    let post: SynthetixWsPostResult<HashMap<String, String>> =
        serde_json::from_value(r.result.unwrap()).unwrap();
    assert_eq!(post.status, "success");
    assert!(post.response.contains_key("ETH-USDT"));

    let r: SynthetixWsReply = plain("ws_pong");
    assert_eq!(r.result.unwrap()["message"], "pong");
    assert_eq!(r.ip_rate_limit, None);
}

fn post<T: DeserializeOwned>(name: &str) -> T {
    let r: SynthetixWsReply = plain(name);
    assert_eq!(r.ip_rate_limit.map(|l| l.requests_cap), Some(10000));
    r.into_post_response()
        .unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn post_err(name: &str) -> SynthetixWsError {
    let r: SynthetixWsReply = plain(name);
    *r.into_post_response::<serde_json::Value>()
        .unwrap_err()
        .downcast::<SynthetixWsError>()
        .unwrap_or_else(|e| panic!("{name}: {e}"))
}

#[test]
fn ws_post_replies() {
    let m: Vec<SynthetixMarket> = post("ws_post_get_markets");
    assert_eq!(m.len(), 2);
    assert!(m.iter().any(|m| m.symbol == "BTC-USDT"));

    let p: HashMap<String, SynthetixMarketPrice> = post("ws_post_get_market_prices");
    assert_eq!(p["BTC-USDT"].mark_price, "64588");

    let mids: HashMap<String, String> = post("ws_post_get_mids");
    assert_eq!(mids.len(), 11);

    let oi: Vec<SynthetixOpenInterest> = post("ws_post_get_open_interest");
    assert_eq!(oi[1].symbol, "BTC-USDT");

    let b: SynthetixOrderBook = post("ws_post_get_orderbook");
    assert_eq!(b.bids[0], SynthetixLevel("64585".into(), "0.1952".into()));
    assert_eq!(b.asks.len(), 5);

    let t: SynthetixLastTrades = post("ws_post_get_last_trades");
    assert_eq!(t.trades.len(), 3);
    assert_eq!(t.trades[0].trade_id, "2122283907607977985");

    let c: SynthetixCandles = post("ws_post_get_candles");
    assert_eq!(c.interval, "1h");
    assert_eq!(c.candles.len(), 3);

    let f: SynthetixFundingRate = post("ws_post_get_funding_rate");
    assert_eq!(f.funding_interval, 3_600_000);

    let col: Vec<SynthetixCollateralConfig> = post("ws_post_get_collaterals");
    assert_eq!(col.len(), 2);

    assert!(post::<bool>("ws_post_get_is_whitelisted"));
    assert!(post::<Vec<String>>("ws_post_get_sub_account_ids").is_empty());
    let d: SynthetixSubAccountIdsWithDelegations = post("ws_post_get_sub_account_ids_delegations");
    assert!(d.sub_account_ids.is_empty());

    let e = post_err("ws_post_unsupported_action");
    assert_eq!(e.error_code, "VALIDATION_ERROR");
    assert_eq!(e.message, "Unsupported action: getFundingRateHistory");
    let e = post_err("ws_post_unknown_market");
    assert_eq!(e.message, "unknown market");
    assert_eq!(e.details.unwrap()["symbol"], "NOPE-USDT");
}

#[test]
fn ws_post_replies_are_strict() {
    let mut v: serde_json::Value = serde_json::from_str(&fixture("ws_post_get_candles")).unwrap();
    v["result"]["response"]["candles"][0]
        .as_object_mut()
        .unwrap()
        .remove("closePrice");
    let r: SynthetixWsReply = serde_json::from_value(v).unwrap();
    assert!(r.into_post_response::<SynthetixCandles>().is_err());

    let mut v: serde_json::Value =
        serde_json::from_str(&fixture("ws_post_get_funding_rate")).unwrap();
    v["result"].as_object_mut().unwrap().remove("status");
    let r: SynthetixWsReply = serde_json::from_value(v).unwrap();
    assert!(r.into_post_response::<SynthetixFundingRate>().is_err());

    let mut v: serde_json::Value = serde_json::from_str(&fixture("ws_post_get_orderbook")).unwrap();
    v.as_object_mut().unwrap().remove("result");
    let r: SynthetixWsReply = serde_json::from_value(v).unwrap();
    assert!(r.into_post_response::<SynthetixOrderBook>().is_err());
}

/// Removing a required field from a live push makes it fail to parse.
fn without<T: DeserializeOwned>(name: &str, pointer: &str, key: &str) -> bool {
    let mut v: serde_json::Value = serde_json::from_str(&fixture(name)).unwrap();
    v.pointer_mut(pointer)
        .unwrap()
        .as_object_mut()
        .unwrap()
        .remove(key)
        .unwrap();
    serde_json::from_value::<T>(v).is_err()
}

#[test]
fn ws_pushes_are_strict() {
    assert!(without::<SynthetixWsOrderBook>(
        "ws_orderbook_diff",
        "",
        "meseq"
    ));
    assert!(without::<SynthetixWsOrderBook>(
        "ws_orderbook_diff",
        "/data/bids/0",
        "quantity"
    ));
    assert!(without::<SynthetixWsTrade>("ws_trade", "/data", "price"));
    assert!(without::<SynthetixWsMarketPrice>(
        "ws_market_price_update_mark",
        "/data",
        "updateType"
    ));
    assert!(without::<SynthetixWsCandle>(
        "ws_candle_update",
        "/data",
        "close_price"
    ));
    assert!(without::<SynthetixWsReply>("ws_pong", "", "requestId"));
}

// ── Live: /v1/trade error envelopes ─────────────────────────────────────────

#[test]
fn trade_errors() {
    let e = err("trade_bad_signature");
    assert_eq!(e.code, "UNAUTHORIZED");
    assert_eq!(e.category.as_deref(), Some("AUTH"));
    assert_eq!(e.retryable, Some(false));
    let e = err("trade_not_authorized");
    assert!(e
        .message
        .contains("not authorized to act on this subaccount"));
}

// ── Docs examples: /v1/trade responses ──────────────────────────────────────

#[test]
fn docs_order_statuses() {
    let s: OrderStatuses = ok("docs_place_orders_mixed");
    assert!(
        matches!(&s.statuses[0], OrderStatus::Resting { resting } if resting.order.venue_id == "1948058938469519360")
    );
    assert!(
        matches!(&s.statuses[1], OrderStatus::Filled { filled } if filled.avg_price == "2999.50")
    );
    match &s.statuses[2] {
        OrderStatus::Error {
            error_code, order, ..
        } => {
            assert_eq!(error_code, "INSUFFICIENT_MARGIN");
            assert_eq!(order.as_ref().unwrap().venue_id, None);
        }
        other => panic!("{other:?}"),
    }

    let s: OrderStatuses = ok("docs_place_orders_filled");
    assert!(matches!(&s.statuses[0], OrderStatus::Filled { .. }));

    let s: OrderStatuses = ok("docs_cancel_orders_partial");
    assert!(matches!(&s.statuses[0], OrderStatus::Canceled { .. }));
    assert!(matches!(
        &s.statuses[1],
        OrderStatus::Error { order: None, .. }
    ));

    let e = err("docs_cancel_orders_error");
    assert_eq!(e.code, "ORDER_NOT_FOUND");
}

#[test]
fn docs_order_management() {
    let c: Vec<CancelAllResult> = ok("docs_cancel_all_orders");
    assert_eq!(c.len(), 2);
    assert_eq!(c[1].symbol, "ETH-USDT");

    let m: ModifyOrderResult = ok("docs_modify_order");
    assert_eq!(m.status, "modified");
    assert_eq!(m.price.as_deref(), Some("45000.50"));
    let m: ModifyOrderResult = ok("docs_modify_order_rejected");
    assert_eq!(m.error_code.as_deref(), Some("ORDER_NOT_FOUND"));

    let s: ScheduleCancelResult = ok("docs_schedule_cancel");
    assert_eq!(s.trigger_time, Some(1704067500000));
    let s: ScheduleCancelResult = ok("docs_schedule_cancel_disarmed");
    assert!(!s.is_active);
    assert_eq!(s.trigger_time, None);

    let l: UpdateLeverageResult = ok("docs_update_leverage");
    assert_eq!(l.new_leverage, "20");
}

#[test]
fn docs_orders() {
    let o: Vec<OpenOrder> = ok("docs_open_orders");
    assert_eq!(o.len(), 3);
    assert!(o[0].take_profit_order.is_some());
    assert_eq!(o[2].expires_at, Some(1755850000000));

    let twap: OpenOrder = plain("docs_open_order_twap");
    assert_eq!(twap.twap_details.unwrap().total_trades, 60);

    let h: Vec<HistoricalOrder> = ok("docs_order_history");
    assert_eq!(h[0].status, "FILLED");
    assert_eq!(h[0].update_time, 1755846290000);
}

#[test]
fn docs_account() {
    let p: Vec<Position> = ok("docs_positions");
    assert_eq!(p.len(), 2);
    assert_eq!(p[1].stop_loss_orders[0].venue_id, "2026771048053084162");

    let s: SubAccount = ok("docs_sub_account");
    assert_eq!(s.collaterals.len(), 2);
    assert_eq!(s.market_preferences.leverages["BTC-USDT"], 20);
    assert_eq!(s.account_limits.max_total_orders, 1000);

    let t: AccountTrades = ok("docs_trades");
    assert_eq!(t.total, 3);
    assert!(t.trades[2].triggered_by_liquidation);

    let f: FundingPayments = ok("docs_funding_payments");
    assert_eq!(f.summary.net_funding, "36.50000000");
    assert_eq!(f.funding_history[0].funding_time, 1735689600000);

    let r: RateLimits = ok("docs_rate_limits");
    assert_eq!(r.requests_cap, 1200);
}

#[test]
fn docs_delegation() {
    let d: DelegatedSigners = ok("docs_delegated_signers");
    assert_eq!(d.delegated_signers[0].expires_at, None);
    assert_eq!(d.delegated_signers[1].expires_at, Some(1767225600000));

    let a: DelegatedSigner = ok("docs_add_delegated_signer");
    assert_eq!(a.permissions, ["session"]);

    let r: RemovedDelegatedSigner = ok("docs_remove_delegated_signer");
    assert_eq!(r.cascade_removed_signers.unwrap().len(), 2);

    let r: RemovedAllDelegatedSigners = ok("docs_remove_all_delegated_signers");
    assert_eq!(r.removed_signers.len(), 3);
}

#[test]
fn docs_sub_account_update() {
    let u: SubAccountUpdate = plain("docs_sub_account_update_order_placed");
    assert_eq!(u.channel, "subAccountUpdate");
    assert_eq!(u.event_type(), Some("orderPlaced"));
}
