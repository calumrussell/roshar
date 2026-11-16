//! Fixtures are responses captured from https://www.deribit.com/api/v2/public/* on 2026-11-16
//! (lists trimmed, values unedited).

use crate::types::*;
use serde::de::DeserializeOwned;
use serde_json::Value;

macro_rules! fixture {
    ($name:literal) => {
        include_str!(concat!("../../tests/fixtures/", $name, ".json"))
    };
}

fn ok<T: DeserializeOwned>(json: &str) -> T {
    serde_json::from_str::<DeribitRpcResponse<T>>(json)
        .unwrap()
        .into_result()
        .unwrap()
}

/// Removes `key` from `result` (or its first element, or `result[field][0]`) and re-parses.
fn without<T: DeserializeOwned>(json: &str, field: Option<&str>, key: &str) -> bool {
    let mut v: Value = serde_json::from_str(json).unwrap();
    let mut target = &mut v["result"];
    if let Some(f) = field {
        target = &mut target[f];
    }
    if target.is_array() {
        target = &mut target[0];
    }
    assert!(
        target.as_object_mut().unwrap().remove(key).is_some(),
        "fixture lacks {key}"
    );
    serde_json::from_value::<DeribitRpcResponse<T>>(v)
        .map(|r| r.into_result().is_ok())
        .unwrap_or(false)
}

#[test]
fn test_instruments() {
    let futures: Vec<DeribitInstrument> = ok(fixture!("rest_instruments_future"));
    assert_eq!(futures[0].settlement_period.as_deref(), Some("perpetual"));
    assert!(futures.iter().all(|i| i.future_type.is_some()));

    let options: Vec<DeribitInstrument> = ok(fixture!("rest_instruments_option"));
    assert!(options
        .iter()
        .all(|i| i.strike.is_some() && i.option_type.is_some()));
    assert!(!options[0].tick_size_steps.is_empty());

    let spot: Vec<DeribitInstrument> = ok(fixture!("rest_instruments_spot"));
    assert!(spot.iter().all(|i| i.settlement_period.is_none()));
    assert_eq!(spot[0].is_cbe_routed, Some(true));
    assert!(spot[1].is_cbe_routed.is_none());

    let combos: Vec<DeribitInstrument> = ok(fixture!("rest_instruments_future_combo"));
    assert_eq!(combos[0].kind, "future_combo");

    let one: DeribitInstrument = ok(fixture!("rest_instrument"));
    assert_eq!(one.instrument_name, "BTC-PERPETUAL");

    assert!(!without::<Vec<DeribitInstrument>>(
        fixture!("rest_instruments_future"),
        None,
        "contract_size"
    ));
}

#[test]
fn test_instrument_creation_doc_example() {
    // Documented `instrument.creation.*` example: instrument fields plus `timestamp`.
    let json = r#"{"tick_size":0.0005,"tick_size_steps":[{"above_price":120,"tick_size":0.001}],
        "taker_commission":0.0003,"strike":16000,"settlement_period":"week",
        "settlement_currency":"BTC","quote_currency":"BTC","price_index":"btc_usd",
        "index_id":1000033,"product_group":"BTC","option_type":"put","min_trade_amount":0.1,
        "maker_commission":0.0003,"kind":"option","is_active":true,
        "instrument_name":"BTC-13JAN23-16000-P","instrument_id":144613,
        "expiration_timestamp":1673596800000,"creation_timestamp":1671696002000,
        "timestamp":1671696002000,"counter_currency":"USD","contract_size":1,"lot_size":1,
        "base_currency":"BTC","base_currency_uuid":"5b71fc48-3dd3-540c-809b-f8c94d0e68b5",
        "quote_currency_uuid":"5b71fc48-3dd3-540c-809b-f8c94d0e68b5",
        "instrument_type":"reversed","underlying_type":"crypto","state":"open"}"#;
    let msg = format!(
        r#"{{"jsonrpc":"2.0","method":"subscription","params":{{"channel":"instrument.creation.any.any","data":{json}}}}}"#
    );
    let parsed: DeribitSubscriptionMessage = serde_json::from_str(&msg).unwrap();
    let DeribitData::InstrumentCreation(i) = parsed.params.data else {
        panic!("expected instrument creation");
    };
    assert_eq!(i.timestamp, Some(1671696002000));
}

#[test]
fn test_currencies() {
    let c: Vec<DeribitCurrency> = ok(fixture!("rest_currencies"));
    assert_eq!(c[0].currency, "BTC");
    assert!(c.iter().any(|c| c.coinbase_networks.is_some()));
    assert!(!without::<Vec<DeribitCurrency>>(
        fixture!("rest_currencies"),
        None,
        "currency_long"
    ));
}

#[test]
fn test_book_summaries() {
    let f: Vec<DeribitBookSummary> = ok(fixture!("rest_book_summary_future"));
    assert!(f[0].funding_8h.is_some() && f[1].funding_8h.is_none());
    assert!(f.iter().all(|s| s.open_interest.is_some()));

    let o: Vec<DeribitBookSummary> = ok(fixture!("rest_book_summary_option"));
    assert!(o[0].bid_price.is_none());
    assert!(o.iter().all(|s| s.mark_iv.is_some()));

    let s: Vec<DeribitBookSummary> = ok(fixture!("rest_book_summary_spot"));
    assert!(s.iter().all(|s| s.open_interest.is_none()));
    assert!(s[0].volume_usd.is_some() && s[1].volume_usd.is_none());

    // Nullable but always sent.
    assert!(!without::<Vec<DeribitBookSummary>>(
        fixture!("rest_book_summary_option"),
        None,
        "bid_price"
    ));
}

#[test]
fn test_order_book_and_ticker() {
    let b: DeribitOrderBookSnapshot = ok(fixture!("rest_order_book_perpetual"));
    assert_eq!(b.ticker.instrument_name, "BTC-PERPETUAL");
    // `depth=3` was requested; the server returned 5 levels per side.
    assert!(b.bids.len() > 1 && b.asks.len() > 1);
    assert!(b.bids[0].0 > b.bids[1].0 && b.asks[0].0 < b.asks[1].0);
    assert!(b.ticker.funding_8h.is_some());

    let b: DeribitOrderBookSnapshot = ok(fixture!("rest_order_book_option"));
    assert!(b.ticker.greeks.is_some() && b.ticker.last_price.is_none());

    let b: DeribitOrderBookSnapshot = ok(fixture!("rest_order_book_by_instrument_id"));
    assert_eq!(b.ticker.instrument_name, "BTC-PERPETUAL");

    let t: DeribitTickerData = ok(fixture!("rest_ticker_perpetual"));
    assert!(t.current_funding.is_some());
    let t: DeribitTickerData = ok(fixture!("rest_ticker_option"));
    assert!(t.underlying_index.is_some());

    assert!(!without::<DeribitOrderBookSnapshot>(
        fixture!("rest_order_book_perpetual"),
        None,
        "change_id"
    ));
    assert!(!without::<DeribitOrderBookSnapshot>(
        fixture!("rest_order_book_perpetual"),
        None,
        "mark_price"
    ));
}

#[test]
fn test_trades_pages() {
    let p: DeribitTradesPage = ok(fixture!("rest_last_trades_by_instrument"));
    assert_eq!(p.trades.len(), 2);
    assert!(p.has_more);

    let p: DeribitTradesPage = ok(fixture!("rest_last_trades_by_currency_and_time"));
    assert!(p.trades.iter().all(|t| t.iv.is_some()));

    assert!(!without::<DeribitTradesPage>(
        fixture!("rest_last_trades_by_instrument"),
        None,
        "has_more"
    ));
    assert!(!without::<DeribitTradesPage>(
        fixture!("rest_last_trades_by_instrument"),
        Some("trades"),
        "trade_seq"
    ));
}

#[test]
fn test_charts_and_volatility() {
    let c: DeribitTradingViewChartData = ok(fixture!("rest_tradingview_chart_data"));
    assert_eq!(c.status, "ok");
    assert_eq!(c.ticks.len(), c.close.len());

    let c: DeribitTradingViewChartData = ok(fixture!("rest_tradingview_chart_data_no_data"));
    assert_eq!(c.status, "no_data");
    assert!(c.ticks.is_empty());

    assert!(!without::<DeribitTradingViewChartData>(
        fixture!("rest_tradingview_chart_data"),
        None,
        "cost"
    ));

    let h: Vec<DeribitTimeValue> = ok(fixture!("rest_historical_volatility"));
    assert_eq!(h.len(), 3);

    let v: DeribitVolatilityIndexData = ok(fixture!("rest_volatility_index_data"));
    assert_eq!(v.data.len(), 3);
    assert!(v.continuation.is_none());
    assert!(!without::<DeribitVolatilityIndexData>(
        fixture!("rest_volatility_index_data"),
        None,
        "continuation"
    ));

    let i: Vec<DeribitTimeValue> = ok(fixture!("rest_index_chart_data"));
    assert_eq!(i.len(), 3);

    let m: Vec<DeribitTimeValue> = ok(fixture!("rest_mark_price_history_empty"));
    assert!(m.is_empty());
    // Documented example shape.
    let m: Vec<DeribitTimeValue> =
        serde_json::from_str("[[1608142381229,0.5165791606037885]]").unwrap();
    assert_eq!(m[0].0, 1608142381229);
}

#[test]
fn test_funding() {
    let h: Vec<DeribitFundingRateHistory> = ok(fixture!("rest_funding_rate_history"));
    assert!(!h.is_empty());
    assert!(!without::<Vec<DeribitFundingRateHistory>>(
        fixture!("rest_funding_rate_history"),
        None,
        "interest_8h"
    ));

    let v: f64 = ok(fixture!("rest_funding_rate_value"));
    assert!(v != 0.0);

    let c: DeribitFundingChartData = ok(fixture!("rest_funding_chart_data"));
    assert_eq!(c.data.len(), 3);
    assert!(!without::<DeribitFundingChartData>(
        fixture!("rest_funding_chart_data"),
        None,
        "current_interest"
    ));
}

#[test]
fn test_index_endpoints() {
    let p: DeribitIndexPrice = ok(fixture!("rest_index_price"));
    assert!(p.index_price > 0.0);
    assert!(!without::<DeribitIndexPrice>(
        fixture!("rest_index_price"),
        None,
        "estimated_delivery_price"
    ));

    let n: Vec<String> = ok(fixture!("rest_index_price_names"));
    assert_eq!(n.len(), 3);
    let n: Vec<DeribitIndexPriceName> = ok(fixture!("rest_index_price_names_extended"));
    assert!(n[0].future_combo_creation_enabled.is_some());
    assert!(n[1].future_combo_creation_enabled.is_none());

    let s: Vec<String> = ok(fixture!("rest_supported_index_names"));
    assert_eq!(s.len(), 3);

    let b: DeribitBasketIndexComposition = ok(fixture!("rest_basket_index_composition"));
    assert_eq!(b.product_id, "COIN50");
    assert_eq!(b.constituents.len(), 2);
    assert!(!without::<DeribitBasketIndexComposition>(
        fixture!("rest_basket_index_composition"),
        Some("constituents"),
        "weight"
    ));

    let d: DeribitDeliveryPrices = ok(fixture!("rest_delivery_prices"));
    assert_eq!(d.data.len(), 2);
    assert!(!without::<DeribitDeliveryPrices>(
        fixture!("rest_delivery_prices"),
        None,
        "records_total"
    ));
}

#[test]
fn test_settlements() {
    let s: DeribitSettlementsPage = ok(fixture!("rest_last_settlements_by_currency"));
    assert_eq!(s.settlements.len(), 2);
    assert!(s
        .settlements
        .iter()
        .any(|s| s.settlement_type == "exercise"));
    assert!(!without::<DeribitSettlementsPage>(
        fixture!("rest_last_settlements_by_currency"),
        Some("settlements"),
        "session_profit_loss"
    ));
}

#[test]
fn test_misc_market_endpoints() {
    let c: DeribitContractSize = ok(fixture!("rest_contract_size"));
    assert_eq!(c.contract_size, 10.0);

    let e: DeribitExpirations = ok(fixture!("rest_expirations_by_currency"));
    let DeribitExpirations::ByCurrency(map) = &e else {
        panic!("expected keyed expirations, got {e:?}");
    };
    assert!(map["btc"]
        .future
        .as_ref()
        .unwrap()
        .contains(&"PERPETUAL".to_string()));
    let e: DeribitExpirations =
        serde_json::from_str(r#"{"future":["17NOV26","PERPETUAL"]}"#).unwrap();
    assert!(matches!(e, DeribitExpirations::Kinds(_)));

    let a: DeribitAprHistory = ok(fixture!("rest_apr_history"));
    assert_eq!(a.data.len(), 2);
    assert!(a.continuation.is_some());
    let a: DeribitAprHistory = ok(fixture!("rest_apr_history_end"));
    assert!(a.data.is_empty() && a.continuation.is_none());
    assert!(!without::<DeribitAprHistory>(
        fixture!("rest_apr_history"),
        Some("data"),
        "apr"
    ));

    let v: Vec<DeribitTradeVolume> = ok(fixture!("rest_trade_volumes"));
    assert!(v[0].spot_volume.is_some() && v[1].spot_volume.is_none());
    let v: Vec<DeribitTradeVolume> = ok(fixture!("rest_trade_volumes_extended"));
    assert!(v.iter().all(|v| v.calls_volume_30d.is_some()));
    assert!(!without::<Vec<DeribitTradeVolume>>(
        fixture!("rest_trade_volumes"),
        None,
        "futures_volume"
    ));
}

#[test]
fn test_combos_and_block_rfq() {
    let c: Vec<DeribitCombo> = ok(fixture!("rest_combos"));
    assert_eq!(c.len(), 2);
    assert!(c[0].legs.iter().any(|l| l.amount < 0));
    let ids: Vec<String> = ok(fixture!("rest_combo_ids"));
    assert_eq!(ids.len(), 3);
    let d: DeribitCombo = ok(fixture!("rest_combo_details"));
    assert_eq!(d.legs.len(), 2);
    assert!(!without::<Vec<DeribitCombo>>(
        fixture!("rest_combos"),
        None,
        "legs"
    ));

    let b: DeribitBlockRfqTradesPage = ok(fixture!("rest_block_rfq_trades"));
    assert!(b.continuation.is_some());
    assert!(b.block_rfqs.iter().any(|r| r.hedge.is_some()));
    assert!(b.block_rfqs.iter().any(|r| r.combo_id.is_none()));
    assert!(b.block_rfqs.iter().any(|r| r.combo_id.is_some()));
    assert!(!without::<DeribitBlockRfqTradesPage>(
        fixture!("rest_block_rfq_trades"),
        Some("block_rfqs"),
        "combo_id"
    ));
}

#[test]
fn test_supporting() {
    let s: DeribitStatus = ok(fixture!("rest_status"));
    assert_eq!(s.locked, "partial");
    assert!(s.locked_indices.is_some());
    let t: u64 = ok(fixture!("rest_get_time"));
    assert!(t > 1_700_000_000_000);
    let v: DeribitVersion = ok(fixture!("rest_test"));
    assert_eq!(v.version, "1.2.26");
}

#[test]
fn test_errors_surface_code_and_message() {
    let err = serde_json::from_str::<DeribitRpcResponse<DeribitVersion>>(fixture!(
        "rest_error_hello_websocket_only"
    ))
    .unwrap()
    .into_result()
    .unwrap_err();
    assert_eq!(err.code, 10030);
    assert_eq!(err.message, "must_be_websocket_request");

    let err = serde_json::from_str::<DeribitRpcResponse<DeribitCombo>>(fixture!(
        "rest_error_invalid_params"
    ))
    .unwrap()
    .into_result()
    .unwrap_err();
    assert_eq!(err.code, -32602);
    assert_eq!(err.data.unwrap()["param"], "combo_id");
}

#[test]
fn test_full_dataset_edge_rows() {
    // From `currency=any`: an index future without `base_currency_uuid`, and an option combo.
    let i: Vec<DeribitInstrument> = ok(fixture!("rest_instruments_any_dvol_and_option_combo"));
    assert!(i[0].base_currency_uuid.is_none());
    assert_eq!(i[1].kind, "option_combo");

    // Options expiring today have a `null` estimated delivery price (still sent).
    let s: Vec<DeribitBookSummary> = ok(fixture!("rest_book_summary_btc_expiring"));
    assert!(s[0].estimated_delivery_price.is_none());
    assert!(s[1].estimated_delivery_price.is_some());
    assert!(!without::<Vec<DeribitBookSummary>>(
        fixture!("rest_book_summary_btc_expiring"),
        None,
        "estimated_delivery_price"
    ));

    // Bankruptcy records carry no instrument, index price or position.
    let b: DeribitSettlementsPage = ok(fixture!("rest_last_settlements_bankruptcy"));
    assert!(b.settlements.iter().all(|s| s.instrument_name.is_none()));
    assert!(b.settlements.iter().all(|s| s.session_bankruptcy.is_some()));

    // Combo trades nest their per-leg trades.
    let t: DeribitTradesPage = ok(fixture!("rest_last_trades_future_combo"));
    let legs = t.trades[0].legs.as_ref().unwrap();
    assert!(legs.len() >= 2);
    assert!(legs
        .iter()
        .all(|l| l.combo_id.is_some() && l.legs.is_none()));
}
