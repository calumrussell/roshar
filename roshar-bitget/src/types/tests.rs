//! Offline parse tests. REST fixtures were captured from https://api.bitget.com and websocket
//! fixtures from wss://ws.bitget.com/v2/ws/public on 2026-11-16 (lists trimmed, values unedited).

use super::*;
use crate::rest::parse_data;

macro_rules! fixture {
    ($name:literal) => {
        include_str!(concat!("../../tests/fixtures/", $name))
    };
}

fn ok<T: serde::de::DeserializeOwned>(text: &str) -> T {
    parse_data(text, 200, "test").unwrap()
}

/// Remove the first occurrence of `"key":value,` (or `,"key":value`) from a JSON fixture.
fn drop_field(text: &str, key: &str) -> String {
    let mut v: serde_json::Value = serde_json::from_str(text).unwrap();
    fn walk(v: &mut serde_json::Value, key: &str) -> bool {
        match v {
            serde_json::Value::Object(m) => {
                if m.remove(key).is_some() {
                    return true;
                }
                m.values_mut().any(|c| walk(c, key))
            }
            serde_json::Value::Array(a) => a.iter_mut().any(|c| walk(c, key)),
            _ => false,
        }
    }
    assert!(walk(&mut v, key), "fixture has no {key}");
    v.to_string()
}

// ---- REST: envelope and errors ----

#[test]
fn test_parse_history_fund_rate() {
    // Documented example reply of GET /api/v2/mix/market/history-fund-rate.
    let text = r#"{"code":"00000","msg":"success","requestTime":1695796546319,"data":[{"symbol":"BTCUSDT","fundingRate":"0.0005","fundingTime":"1695776400000"}]}"#;
    let parsed: Vec<BitgetHistoricalFundingRate> = ok(text);
    assert_eq!(parsed[0].funding_time, "1695776400000");
}

#[test]
fn test_error_replies_surface_code_and_msg() {
    for text in [
        fixture!("error_bad_symbol.json"),
        fixture!("error_bad_granularity.json"),
    ] {
        let head: BitgetStatus = serde_json::from_str(text).unwrap();
        let err = parse_data::<Vec<BitgetCandle>>(text, 400, "test")
            .unwrap_err()
            .to_string();
        assert!(err.contains(&head.code) && err.contains(&head.msg), "{err}");
    }
}

#[test]
fn test_server_time() {
    let t: BitgetServerTime = ok(fixture!("mix_time.json"));
    assert_eq!(t.server_time, "1794827102794");
}

// ---- REST: futures ----

#[test]
fn test_contracts() {
    let c: Vec<BitgetContract> = ok(fixture!("mix_contracts.json"));
    assert_eq!(c[0].symbol, "BTCUSDT");
    assert_eq!(c[0].support_margin_coins, vec!["USDT".to_string()]);
    let empty: Vec<BitgetContract> = ok(fixture!("mix_contracts_coin_empty.json"));
    assert!(empty.is_empty());
    let text = drop_field(fixture!("mix_contracts.json"), "maxOrderQty");
    assert!(parse_data::<Vec<BitgetContract>>(&text, 200, "t").is_err());
}

#[test]
fn test_tickers() {
    let t: Vec<BitgetTickerData> = ok(fixture!("mix_ticker.json"));
    assert_eq!(t[0].symbol, "BTCUSDT");
    assert_eq!(t[0].delivery_time, None);
    let text = drop_field(fixture!("mix_ticker.json"), "deliveryTime");
    assert!(parse_data::<Vec<BitgetTickerData>>(&text, 200, "t").is_err());
    let usdc: Vec<BitgetTickerData> = ok(fixture!("mix_tickers_usdc.json"));
    assert_eq!(usdc[0].symbol, "BTCPERP");
    let text = drop_field(fixture!("mix_ticker.json"), "markPrice");
    assert!(parse_data::<Vec<BitgetTickerData>>(&text, 200, "t").is_err());
}

#[test]
fn test_merge_depth() {
    let d: BitgetMergeDepth = ok(fixture!("mix_merge_depth.json"));
    assert_eq!(d.asks[0], [64542.0, 1.41]);
    assert_eq!(d.precision, "scale1");
    let s: BitgetMergeDepth = ok(fixture!("spot_merge_depth.json"));
    assert_eq!(s.bids.len(), 5);
    let text = drop_field(fixture!("mix_merge_depth.json"), "isMaxPrecision");
    assert!(parse_data::<BitgetMergeDepth>(&text, 200, "t").is_err());
}

#[test]
fn test_market_trades() {
    for text in [
        fixture!("mix_fills.json"),
        fixture!("mix_fills_history.json"),
        fixture!("spot_fills.json"),
        fixture!("spot_fills_history.json"),
    ] {
        let t: Vec<BitgetMarketTrade> = ok(text);
        assert!(!t.is_empty());
        let text = drop_field(text, "tradeId");
        assert!(parse_data::<Vec<BitgetMarketTrade>>(&text, 200, "t").is_err());
    }
}

#[test]
fn test_futures_candles() {
    for text in [
        fixture!("mix_candles.json"),
        fixture!("mix_candles_mark.json"),
        fixture!("mix_history_candles.json"),
        fixture!("mix_history_index_candles.json"),
        fixture!("mix_history_mark_candles.json"),
    ] {
        let c: Vec<BitgetCandle> = ok(text);
        assert!(!c.is_empty());
    }
    let c: Vec<BitgetCandle> = ok(fixture!("mix_candles.json"));
    assert_eq!(c[0].ts, "1794826980000");
    assert_eq!(c[0].quote_volume, "970496.72858");
    // Wrong arity is an error.
    let short = r#"{"code":"00000","msg":"success","data":[["1","2","3","4","5","6"]]}"#;
    assert!(parse_data::<Vec<BitgetCandle>>(short, 200, "t").is_err());
    // A spot (8 column) candle is not a futures candle.
    assert!(parse_data::<Vec<BitgetCandle>>(fixture!("spot_candles.json"), 200, "t").is_err());
}

#[test]
fn test_open_interest() {
    let oi: BitgetOpenInterest = ok(fixture!("mix_open_interest.json"));
    assert_eq!(oi.open_interest_list[0].symbol, "BTCUSDT");
    let text = drop_field(fixture!("mix_open_interest.json"), "size");
    assert!(parse_data::<BitgetOpenInterest>(&text, 200, "t").is_err());
}

#[test]
fn test_current_funding_rates() {
    let one: Vec<BitgetCurrentFundingRate> = ok(fixture!("mix_current_fund_rate.json"));
    assert_eq!(one[0].funding_rate_interval, "8");
    let all: Vec<BitgetCurrentFundingRate> = ok(fixture!("mix_current_fund_rates.json"));
    let text = drop_field(fixture!("mix_current_fund_rates.json"), "minFundingRate");
    assert!(parse_data::<Vec<BitgetCurrentFundingRate>>(&text, 200, "t").is_err());
    assert_eq!(all[2].min_funding_rate, None);
    let text = drop_field(fixture!("mix_current_fund_rate.json"), "nextUpdate");
    assert!(parse_data::<Vec<BitgetCurrentFundingRate>>(&text, 200, "t").is_err());
}

#[test]
fn test_funding_time() {
    let f: Vec<BitgetNextFundingTime> = ok(fixture!("mix_funding_time.json"));
    assert_eq!(f[0].rate_period, "8");
    let text = drop_field(fixture!("mix_funding_time.json"), "ratePeriod");
    assert!(parse_data::<Vec<BitgetNextFundingTime>>(&text, 200, "t").is_err());
}

#[test]
fn test_symbol_price() {
    let p: Vec<BitgetSymbolPrice> = ok(fixture!("mix_symbol_price.json"));
    assert_eq!(p[0].mark_price, "64550");
    let text = drop_field(fixture!("mix_symbol_price.json"), "indexPrice");
    assert!(parse_data::<Vec<BitgetSymbolPrice>>(&text, 200, "t").is_err());
}

#[test]
fn test_oi_limit() {
    let l: Vec<BitgetOiLimit> = ok(fixture!("mix_oi_limit.json"));
    assert_eq!(l[0].single_position_ratio, None);
    let text = drop_field(fixture!("mix_oi_limit.json"), "singlePositionRatio");
    assert!(parse_data::<Vec<BitgetOiLimit>>(&text, 200, "t").is_err());
    let text = drop_field(fixture!("mix_oi_limit.json"), "notionalValue");
    assert!(parse_data::<Vec<BitgetOiLimit>>(&text, 200, "t").is_err());
}

// ---- REST: spot ----

#[test]
fn test_spot_symbols() {
    let s: Vec<BitgetSpotSymbol> = ok(fixture!("spot_symbols.json"));
    assert_eq!(s[0].symbol, "BTCUSDT");
    let text = drop_field(fixture!("spot_symbols.json"), "quantityPrecision");
    assert!(parse_data::<Vec<BitgetSpotSymbol>>(&text, 200, "t").is_err());
}

#[test]
fn test_spot_tickers() {
    let t: Vec<BitgetSpotTicker> = ok(fixture!("spot_tickers.json"));
    assert!(t[0].bid_sz.is_some());
    assert_eq!(t[2].bid_sz, None);
    let text = drop_field(fixture!("spot_tickers.json"), "bidSz");
    assert!(parse_data::<Vec<BitgetSpotTicker>>(&text, 200, "t").is_err());
    let text = drop_field(fixture!("spot_tickers.json"), "usdtVolume");
    assert!(parse_data::<Vec<BitgetSpotTicker>>(&text, 200, "t").is_err());
}

#[test]
fn test_spot_orderbook() {
    let b: BitgetSpotOrderBook = ok(fixture!("spot_orderbook.json"));
    assert_eq!(b.asks[0][0], "64568.56");
    let text = drop_field(fixture!("spot_orderbook.json"), "bids");
    let step: BitgetSpotOrderBook = ok(fixture!("spot_orderbook_step2.json"));
    assert_eq!(
        step.asks[0],
        ["64652.0".to_string(), "0.439369".to_string()]
    );
    assert!(parse_data::<BitgetSpotOrderBook>(&text, 200, "t").is_err());
}

#[test]
fn test_spot_candles() {
    for text in [
        fixture!("spot_candles.json"),
        fixture!("spot_history_candles.json"),
    ] {
        let c: Vec<BitgetSpotCandle> = ok(text);
        assert!(!c.is_empty());
    }
    assert!(parse_data::<Vec<BitgetSpotCandle>>(fixture!("mix_candles.json"), 200, "t").is_err());
}

#[test]
fn test_spot_auction() {
    let a: BitgetSpotAuction = ok(fixture!("spot_auction.json"));
    assert_eq!(a.stage, None);
    // Always sent, even when null: a missing key is an error.
    let text = drop_field(fixture!("spot_auction.json"), "stage");
    assert!(parse_data::<BitgetSpotAuction>(&text, 200, "t").is_err());
}

// ---- Websocket ----

#[test]
fn test_ping_is_text() {
    assert_eq!(BitgetWssMessage::ping(), "ping");
    assert!(matches!(
        BitgetWsIncoming::parse("pong").unwrap(),
        BitgetWsIncoming::Pong
    ));
}

#[test]
fn test_subscribe_builders() {
    let msg = BitgetWssMessage::depth(BitgetProductType::UsdtFutures, "BTCUSDT");
    let parsed: serde_json::Value = serde_json::from_str(&msg.to_json()).unwrap();
    assert_eq!(parsed["op"], "subscribe");
    assert_eq!(parsed["args"][0]["channel"], "books");
    assert_eq!(parsed["args"][0]["instId"], "BTCUSDT");
    assert_eq!(parsed["args"][0]["instType"], "USDT-FUTURES");

    let msg = BitgetWssMessage::trades(BitgetProductType::UsdcFutures, "BTCPERP");
    assert_eq!(msg.args[0].channel, "trade");
    assert_eq!(msg.args[0].inst_type, "USDC-FUTURES");

    let msg = BitgetWssMessage::candles(BitgetWsInstType::Spot, "1m", "BTCUSDT");
    assert_eq!(msg.args[0].channel, "candle1m");
    assert_eq!(msg.args[0].inst_type, "SPOT");

    let msg = BitgetWssMessage::books_unsub(
        BitgetProductType::CoinFutures,
        BitgetBookChannel::Books15,
        "BTCUSD",
    );
    assert_eq!(msg.op, "unsubscribe");
    assert_eq!(msg.args[0].channel, "books15");

    assert_eq!(
        BitgetWssMessage::auction("BTCUSDT").args[0].channel,
        "auction"
    );
}

#[test]
fn test_ws_acks_and_errors() {
    match BitgetWsIncoming::parse(fixture!("ws_mix_books_subscribe.json")).unwrap() {
        BitgetWsIncoming::Subscribed(a) => assert_eq!(a.channel, "books"),
        other => panic!("{other:?}"),
    }
    match BitgetWsIncoming::parse(fixture!("ws_spot_auction_subscribe.json")).unwrap() {
        BitgetWsIncoming::Subscribed(a) => assert_eq!(a.inst_type, "SPOT"),
        other => panic!("{other:?}"),
    }
    match BitgetWsIncoming::parse(fixture!("ws_mix_books_unsubscribe.json")).unwrap() {
        BitgetWsIncoming::Unsubscribed(a) => assert_eq!(a.inst_id, "BTCUSDT"),
        other => panic!("{other:?}"),
    }
    match BitgetWsIncoming::parse(fixture!("ws_mix_nope_error.json")).unwrap() {
        BitgetWsIncoming::Error(e) => {
            assert_eq!(e.code, "30016");
            assert_eq!(e.msg, "Param error");
            assert_eq!(e.arg.unwrap().channel, "nope");
        }
        other => panic!("{other:?}"),
    }
    // Documented string code form.
    match BitgetWsIncoming::parse(r#"{"event":"error","code":"30005","msg":"error"}"#).unwrap() {
        BitgetWsIncoming::Error(e) => assert_eq!(e.code, "30005"),
        other => panic!("{other:?}"),
    }
    assert!(BitgetWsIncoming::parse(r#"{"event":"what"}"#).is_err());
    assert!(BitgetWsIncoming::parse("not json").is_err());
}

#[test]
fn test_ws_books() {
    for (text, action) in [
        (
            fixture!("ws_mix_books_snapshot.json"),
            BitgetWsAction::Snapshot,
        ),
        (fixture!("ws_mix_books_update.json"), BitgetWsAction::Update),
        (
            fixture!("ws_mix_books1_snapshot.json"),
            BitgetWsAction::Snapshot,
        ),
        (
            fixture!("ws_mix_books5_snapshot.json"),
            BitgetWsAction::Snapshot,
        ),
        (
            fixture!("ws_mix_books15_snapshot.json"),
            BitgetWsAction::Snapshot,
        ),
        (
            fixture!("ws_spot_books_snapshot.json"),
            BitgetWsAction::Snapshot,
        ),
        (
            fixture!("ws_spot_books_update.json"),
            BitgetWsAction::Update,
        ),
        (
            fixture!("ws_spot_books1_snapshot.json"),
            BitgetWsAction::Snapshot,
        ),
        (
            fixture!("ws_spot_books5_snapshot.json"),
            BitgetWsAction::Snapshot,
        ),
        (
            fixture!("ws_spot_books15_snapshot.json"),
            BitgetWsAction::Snapshot,
        ),
    ] {
        match BitgetWsIncoming::parse(text).unwrap() {
            BitgetWsIncoming::Books(p) => {
                assert_eq!(p.action, action);
                assert!(!p.data[0].asks.is_empty());
            }
            other => panic!("{other:?}"),
        }
    }
    match BitgetWsIncoming::parse(fixture!("ws_mix_books_update.json")).unwrap() {
        BitgetWsIncoming::Books(p) => assert_eq!(p.data[0].pseq, 1248417496092),
        other => panic!("{other:?}"),
    }
    let text = drop_field(fixture!("ws_mix_books_update.json"), "seq");
    assert!(BitgetWsIncoming::parse(&text).is_err());
}

#[test]
fn test_ws_trades() {
    for text in [
        fixture!("ws_mix_trade_snapshot.json"),
        fixture!("ws_mix_trade_update.json"),
        fixture!("ws_spot_trade_snapshot.json"),
        fixture!("ws_spot_trade_update.json"),
    ] {
        match BitgetWsIncoming::parse(text).unwrap() {
            BitgetWsIncoming::Trades(p) => assert!(!p.data[0].trade_id.is_empty()),
            other => panic!("{other:?}"),
        }
    }
    let text = drop_field(fixture!("ws_mix_trade_update.json"), "side");
    assert!(BitgetWsIncoming::parse(&text).is_err());
}

#[test]
fn test_ws_tickers() {
    match BitgetWsIncoming::parse(fixture!("ws_mix_ticker_snapshot.json")).unwrap() {
        BitgetWsIncoming::FuturesTicker(p) => {
            assert_eq!(p.data[0].next_funding_time, "1794844800000");
            assert_eq!(p.data[0].delivery_price, None);
        }
        other => panic!("{other:?}"),
    }
    match BitgetWsIncoming::parse(fixture!("ws_spot_ticker_snapshot.json")).unwrap() {
        BitgetWsIncoming::SpotTicker(p) => assert_eq!(p.data[0].inst_id, "BTCUSDT"),
        other => panic!("{other:?}"),
    }
    match BitgetWsIncoming::parse(fixture!("ws_spot_ticker_empty_book.json")).unwrap() {
        BitgetWsIncoming::SpotTicker(p) => {
            assert_eq!(p.data[0].bid_pr, None);
            assert_eq!(p.data[0].ask_sz, None);
        }
        other => panic!("{other:?}"),
    }
    let text = drop_field(fixture!("ws_mix_ticker_snapshot.json"), "holdingAmount");
    assert!(BitgetWsIncoming::parse(&text).is_err());
    let text = drop_field(fixture!("ws_spot_ticker_snapshot.json"), "changeUtc24h");
    assert!(BitgetWsIncoming::parse(&text).is_err());
}

#[test]
fn test_ws_candles() {
    for text in [
        fixture!("ws_mix_candle1m_snapshot.json"),
        fixture!("ws_mix_candle1m_update.json"),
        fixture!("ws_spot_candle1m_snapshot.json"),
        fixture!("ws_spot_candle1m_update.json"),
    ] {
        match BitgetWsIncoming::parse(text).unwrap() {
            BitgetWsIncoming::Candles(p) => assert_eq!(p.data[0].ts.len(), 13),
            other => panic!("{other:?}"),
        }
    }
    let short = r#"{"action":"update","arg":{"instType":"SPOT","channel":"candle1m","instId":"BTCUSDT"},"data":[["1","2","3","4","5","6","7"]],"ts":1}"#;
    assert!(BitgetWsIncoming::parse(short).is_err());
}

#[test]
fn test_ws_auction_and_unknown_channel() {
    // No live auction push was received (BTCUSDT has no auction); this is the documented example.
    let doc = r#"{"action":"snapshot","arg":{"instType":"SPOT","channel":"auction","instId":"BTCUSDT"},"data":[{"stage":"stage_1","stageEndTime":"1728876900000","estOpeningPrice":null,"matchedVolume":null,"auctionEndTime":"1728877500000"}],"ts":1695711090682}"#;
    match BitgetWsIncoming::parse(doc).unwrap() {
        BitgetWsIncoming::Auction(p) => assert_eq!(p.data[0].stage.as_deref(), Some("stage_1")),
        other => panic!("{other:?}"),
    }
    let unknown = r#"{"action":"snapshot","arg":{"instType":"SPOT","channel":"nope","instId":"X"},"data":[],"ts":1}"#;
    assert!(BitgetWsIncoming::parse(unknown).is_err());
}
