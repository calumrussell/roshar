//! Offline parse tests over live captures in `tests/fixtures` (2026-11-15, UK IP).

use roshar_grvt::types::*;
use serde::de::DeserializeOwned;
use serde_json::{json, Value};

fn fixture<T: DeserializeOwned>(name: &str) -> T {
    let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&path).unwrap();
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{name}: {e}"))
}

#[test]
fn instruments() {
    let one: GrvtResult<GrvtInstrument> = fixture("instrument.json");
    assert_eq!(one.result.instrument, "BTC_USDT_Perp");
    assert_eq!(one.result.instrument_hash, "0x030501");
    assert_eq!(one.result.kind, GrvtKind::Perpetual);
    assert_eq!(one.result.base_decimals, 9);
    assert_eq!(
        one.result.settlement_period,
        Some(GrvtSettlementPeriod::Perpetual)
    );

    let all: GrvtResult<Vec<GrvtInstrument>> = fixture("instruments_all.json");
    assert!(all.result.len() > 100);
    let swap = all
        .result
        .iter()
        .find(|i| i.kind == GrvtKind::SpotSwap)
        .unwrap();
    assert!(swap.settlement_period.is_none());
    assert!(swap.max_position_size.is_none());
    assert_eq!(swap.venues, vec![GrvtVenue::Orderbook]);
    let sfp = all
        .result
        .iter()
        .find(|i| i.kind == GrvtKind::StablePerp)
        .unwrap();
    assert!(sfp.triple_swap_day.is_some());
    assert!(all.result.iter().any(|i| i.status == "DELISTED"));
}

#[test]
fn currency_and_margin_rules() {
    let c: GrvtResult<Vec<GrvtCurrency>> = fixture("currency.json");
    assert!(c.result.iter().any(|c| c.symbol == "USDT"));

    let m: GrvtMarginRules = fixture("margin_rules.json");
    assert_eq!(m.instrument, "BTC_USDT_Perp");
    assert!(m.risk_brackets[0].notional_cap.is_some());
    assert!(m.risk_brackets.last().unwrap().notional_cap.is_none());
}

#[test]
fn tickers() {
    let mini: GrvtResult<GrvtMiniTicker> = fixture("mini.json");
    assert_eq!(mini.result.instrument.as_deref(), Some("BTC_USDT_Perp"));
    assert!(mini.result.best_bid_price.is_some());

    let t: GrvtResult<GrvtTicker> = fixture("ticker.json");
    assert!(t.result.funding_rate.is_some());
    assert!(t.result.next_funding_time.is_some());
    assert!(t.result.open_interest.is_some());
}

#[test]
fn book_trades_candles_funding() {
    let b: GrvtResult<GrvtOrderbook> = fixture("book.json");
    assert_eq!(b.result.bids.len(), 10);
    assert!(b.result.asks[0].num_orders > 0);

    let t: GrvtResult<Vec<GrvtTrade>> = fixture("trade.json");
    assert_eq!(t.result[0].venue, GrvtVenue::Orderbook);

    let h: GrvtPage<GrvtTrade> = fixture("trade_history.json");
    assert!(!h.result.is_empty());

    let k: GrvtPage<GrvtCandle> = fixture("kline.json");
    assert_eq!(k.result.len(), 3);
    assert!(k.next.is_some());

    let f: GrvtPage<GrvtFundingRate> = fixture("funding.json");
    assert_eq!(f.result[0].funding_interval_hours, 8);

    let st: GrvtServerTime = fixture("server_time.json");
    assert!(st.server_time > 1_700_000_000_000);
}

#[test]
fn error_envelopes() {
    let e: GrvtError = fixture("err_depth.json");
    assert_eq!((e.code, e.status), (3031, 400));
    assert_eq!(e.message, "Depth is invalid");
    let e: GrvtError = fixture("err_not_found.json");
    assert_eq!(e.code, 1003);
    let e: GrvtError = fixture("err_unauth.json");
    assert_eq!((e.code, e.status), (1000, 401));
    let l: GrvtLoginFailure = fixture("login_fail.json");
    assert_eq!(l.status, "failure");
    assert_eq!(l.error, "Invalid Credentials");
}

#[test]
fn ws_feeds() {
    let s: GrvtWsFeed<GrvtOrderbook> = fixture("ws_book_snapshot.json");
    assert_eq!(s.stream, STREAM_BOOK_SNAPSHOT);
    assert_eq!(s.sequence_number, "0");
    let d: GrvtWsFeed<GrvtOrderbook> = fixture("ws_book_delta.json");
    assert_eq!(d.stream, STREAM_BOOK_DELTA);
    assert_ne!(d.sequence_number, "0");
    let t: GrvtWsFeed<GrvtTrade> = fixture("ws_trade.json");
    assert_eq!(t.stream, STREAM_TRADE);
    let m: GrvtWsFeed<GrvtMiniTicker> = fixture("ws_mini.json");
    assert_eq!(m.stream, STREAM_MINI_TICKER_SNAPSHOT);
    let k: GrvtWsFeed<GrvtTicker> = fixture("ws_ticker.json");
    assert_eq!(k.feed.funding_interval_hours, Some(8));
    let c: GrvtWsFeed<GrvtCandle> = fixture("ws_candle.json");
    assert_eq!(c.selector, "BTC_USDT_Perp@CI_1_M-TRADE");
}

#[test]
fn ws_rpc_replies() {
    let ack: GrvtRpcResponse<GrvtSubscribeResult> = fixture("ws_subscribe_ack.json");
    assert_eq!(ack.id, Some(1));
    let r = ack.into_result().unwrap();
    assert_eq!(r.subs, vec!["BTC_USDT_Perp@500-10".to_string()]);
    assert_eq!(r.num_snapshots, vec![1]);

    let un: GrvtRpcResponse<GrvtUnsubscribeResult> = fixture("ws_unsubscribe_ack.json");
    assert_eq!(un.into_result().unwrap().unsubs.len(), 1);

    let err: GrvtRpcResponse<GrvtSubscribeResult> = fixture("ws_subscribe_error.json");
    let e = err.into_result().unwrap_err();
    assert_eq!((e.code, e.message.as_str()), (3030, "Feed rate is invalid"));

    let nf: GrvtRpcResponse<Value> = fixture("ws_method_not_found.json");
    assert_eq!(nf.into_result().unwrap_err().code, 1200);
}

#[test]
fn ws_messages() {
    // The exact frames sent live on 2026-11-15.
    let sels = vec![selector::book_snapshot("BTC_USDT_Perp", 500, 10)];
    let msg: Value =
        serde_json::from_str(&subscribe_message(STREAM_BOOK_SNAPSHOT, &sels, Some(1))).unwrap();
    assert_eq!(
        msg,
        json!({"jsonrpc":"2.0","method":"subscribe","params":{"stream":"v1.book.s","selectors":["BTC_USDT_Perp@500-10"]},"id":1})
    );
    let sels = vec![selector::trade("BTC_USDT_Perp", 50)];
    let msg: Value = serde_json::from_str(&unsubscribe_message(STREAM_TRADE, &sels, None)).unwrap();
    assert_eq!(
        msg,
        json!({"jsonrpc":"2.0","method":"unsubscribe","params":{"stream":"v1.trade","selectors":["BTC_USDT_Perp@50"]}})
    );
    assert_eq!(
        selector::candle(
            "BTC_USDT_Perp",
            GrvtCandleInterval::OneMinute,
            GrvtCandleType::Trade
        ),
        "BTC_USDT_Perp@CI_1_M-TRADE"
    );
    assert_eq!(
        selector::account(42, Some("ETH_USDT_Perp")),
        "42-ETH_USDT_Perp"
    );
    assert_eq!(selector::account(42, None), "42");
}

#[test]
fn request_bodies() {
    let req = GrvtHistoryRequest {
        sub_account_id: "7".to_string(),
        filter: GrvtAssetFilter {
            kind: Some(vec![GrvtKind::Perpetual]),
            base: None,
            quote: None,
        },
        page: GrvtPageQuery {
            start_time: Some(1),
            end_time: None,
            limit: Some(500),
            cursor: None,
        },
    };
    assert_eq!(
        serde_json::to_value(&req).unwrap(),
        json!({"sub_account_id":"7","kind":["PERPETUAL"],"start_time":"1","limit":500})
    );
    let cancel = GrvtCancelOrderRequest {
        sub_account_id: "7".to_string(),
        order_id: None,
        client_order_id: Some("9300000000000000000".to_string()),
        time_to_live_ms: Some(500),
    };
    assert_eq!(
        serde_json::to_value(&cancel).unwrap(),
        json!({"sub_account_id":"7","client_order_id":"9300000000000000000","time_to_live_ms":"500"})
    );
}

fn fixture_value(name: &str) -> Value {
    fixture(name)
}

#[test]
fn supported_assets_and_collateral() {
    let a: GrvtSupportedAssets = fixture("supported_assets.json");
    assert!(a.funding.iter().any(|x| x.asset_code == "USDT"));
    assert!(!a.spot.is_empty());
    assert!(a
        .futures
        .iter()
        .any(|f| f.mode == GrvtSubAccountMode::SingleAssetMode));
    assert!(a
        .futures
        .iter()
        .any(|f| f.mode == GrvtSubAccountMode::MultiAssetMode));
    assert!(a
        .futures
        .iter()
        .flat_map(|f| &f.supported_assets)
        .any(|x| x.transfer_disabled));

    let c: GrvtCollateralAssets = fixture("collateral_asset_info.json");
    let usdt = c.assets.iter().find(|x| x.currency == "USDT").unwrap();
    assert_eq!(usdt.cdc_status, GrvtCdcStatus::Normal);
    assert_eq!(usdt.collateral_value_ratio, "99.8");
}

#[test]
fn supported_assets_and_collateral_strict() {
    let mut v = fixture_value("supported_assets.json");
    v["futures"][0]["supported_assets"][0]
        .as_object_mut()
        .unwrap()
        .remove("balance_decimals");
    assert!(serde_json::from_value::<GrvtSupportedAssets>(v).is_err());
    let mut v = fixture_value("supported_assets.json");
    v["futures"][0]["mode"] = json!("SOMETHING_ELSE");
    assert!(serde_json::from_value::<GrvtSupportedAssets>(v).is_err());

    let mut v = fixture_value("collateral_asset_info.json");
    v["assets"][0].as_object_mut().unwrap().remove("cdc_status");
    assert!(serde_json::from_value::<GrvtCollateralAssets>(v).is_err());
}

#[test]
fn ws_market_queries() {
    let t: GrvtRpcResponse<GrvtResult<GrvtTicker>> = fixture("ws_rpc_ticker.json");
    assert_eq!(t.method.as_deref(), Some("v1/ticker"));
    assert_eq!(t.id, Some(108));
    let t = t.into_result().unwrap().result;
    assert_eq!(t.instrument.as_deref(), Some("BTC_USDT_Perp"));

    let f: GrvtRpcResponse<GrvtPage<GrvtFundingRate>> = fixture("ws_rpc_funding.json");
    let f = f.into_result().unwrap();
    assert_eq!(f.result.len(), 2);
    assert!(f.next.is_some());

    let m: GrvtRpcResponse<GrvtMarginRules> = fixture("ws_rpc_margin_rules.json");
    assert_eq!(m.into_result().unwrap().instrument, "BTC_USDT_Perp");

    let s: GrvtRpcResponse<GrvtSupportedAssets> = fixture("ws_rpc_supported_assets.json");
    assert!(!s.into_result().unwrap().funding.is_empty());

    let e: GrvtRpcResponse<GrvtResult<GrvtOrderbook>> = fixture("ws_rpc_error_depth.json");
    assert_eq!(e.method.as_deref(), Some("v1/book"));
    let e = e.into_result().unwrap_err();
    assert_eq!((e.code, e.message.as_str()), (3031, "Depth is invalid"));

    // Sent without an id; answered with id 0.
    let n: GrvtRpcResponse<GrvtResult<GrvtTicker>> = fixture("ws_rpc_no_id.json");
    assert_eq!(n.id, Some(0));

    // Request params are the REST bodies.
    let page = GrvtPageQuery {
        limit: Some(2),
        ..Default::default()
    };
    let msg: Value = serde_json::from_str(
        &GrvtRpcRequest::new(
            "v1/kline",
            GrvtKlineRequest {
                instrument: "BTC_USDT_Perp",
                interval: GrvtCandleInterval::OneMinute,
                kind: GrvtCandleType::Trade,
                page: &page,
            },
            Some(112),
        )
        .to_json(),
    )
    .unwrap();
    assert_eq!(
        msg,
        json!({"jsonrpc":"2.0","method":"v1/kline","params":{"instrument":"BTC_USDT_Perp","interval":"CI_1_M","type":"TRADE","limit":2},"id":112})
    );
    let msg: Value = serde_json::from_str(
        &GrvtRpcRequest::new("v1/currency", GrvtEmptyRequest {}, None).to_json(),
    )
    .unwrap();
    assert_eq!(
        msg,
        json!({"jsonrpc":"2.0","method":"v1/currency","params":{}})
    );
}
