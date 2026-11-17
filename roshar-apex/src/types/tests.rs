//! Offline parse tests. REST fixtures were captured from https://omni.apex.exchange/api and websocket
//! fixtures from wss://quote.omni.apex.exchange/realtime_public?v=2 on 2026-11-15 (`symbols` is
//! trimmed to a few entries per list).

use super::*;
use std::collections::HashMap;

const TIME: &str = include_str!("../../tests/fixtures/time.json");
const SYMBOLS: &str = include_str!("../../tests/fixtures/symbols_trimmed.json");
const DEPTH: &str = include_str!("../../tests/fixtures/depth.json");
const DEPTH_UNKNOWN: &str = include_str!("../../tests/fixtures/depth_unknown_symbol.json");
const TRADES: &str = include_str!("../../tests/fixtures/trades.json");
const KLINES: &str = include_str!("../../tests/fixtures/klines.json");
const TICKER: &str = include_str!("../../tests/fixtures/ticker.json");
const FUNDING: &str = include_str!("../../tests/fixtures/history_funding.json");
const ERROR: &str = include_str!("../../tests/fixtures/error_invalid_symbol.json");

const WS_BOOK_SNAPSHOT: &str = include_str!("../../tests/fixtures/ws_orderbook_snapshot.json");
const WS_BOOK_DELTA: &str = include_str!("../../tests/fixtures/ws_orderbook_delta.json");
const WS_TRADES_SNAPSHOT: &str = include_str!("../../tests/fixtures/ws_trades_snapshot.json");
const WS_TRADES_DELTA: &str = include_str!("../../tests/fixtures/ws_trades_delta.json");
const WS_TICKER_SNAPSHOT: &str = include_str!("../../tests/fixtures/ws_ticker_snapshot.json");
const WS_TICKER_DELTA: &str = include_str!("../../tests/fixtures/ws_ticker_delta.json");
const WS_CANDLE: &str = include_str!("../../tests/fixtures/ws_candle_snapshot.json");
const WS_ALL_TICKERS: &str = include_str!("../../tests/fixtures/ws_all_tickers.json");
const WS_SUBSCRIBE: &str = include_str!("../../tests/fixtures/ws_subscribe_response.json");
const WS_UNSUBSCRIBE: &str = include_str!("../../tests/fixtures/ws_unsubscribe_response.json");
const WS_PONG: &str = include_str!("../../tests/fixtures/ws_pong_response.json");
const WS_BOOK200: &str = include_str!("../../tests/fixtures/ws_orderbook200_snapshot_trimmed.json");
const WS_CANDLE_CONFIRM: &str = include_str!("../../tests/fixtures/ws_candle_confirm.json");
const WS_SUBSCRIBE_ERROR: &str = include_str!("../../tests/fixtures/ws_subscribe_error.json");
const WS_DECODE_ERROR: &str = include_str!("../../tests/fixtures/ws_decode_error.json");
const WS_SERVER_PING: &str = include_str!("../../tests/fixtures/ws_server_ping.json");

fn data<T: serde::de::DeserializeOwned>(json: &str) -> T {
    serde_json::from_str::<ApexEnvelope<T>>(json)
        .expect("envelope parses")
        .into_result()
        .expect("success envelope")
}

#[test]
fn parses_time() {
    let time: ApexServerTime = data(TIME);
    assert_eq!(time.time, 1794780189932);
}

#[test]
fn parses_symbols() {
    let symbols: ApexSymbols = data(SYMBOLS);
    let cc = &symbols.contract_config;
    let btc = &cc.perpetual_contract[0];
    assert_eq!(btc.symbol, "BTC-USDT");
    assert_eq!(btc.cross_symbol_name, "BTCUSDT");
    assert_eq!(btc.l2_pair_id_u16().unwrap(), 50001);
    assert_eq!(btc.tick_size, "0.1");
    assert_eq!(btc.settle_asset_id, "USDT");
    assert!(btc.delivery_time.is_none());
    assert_eq!(btc.risk_limit_config.imr_steps.len(), 3);

    // A contract scheduled for removal, and one without `category`.
    assert!(cc
        .perpetual_contract
        .iter()
        .any(|c| c.pull_off_time.is_some() && c.delivery_time.is_some()));
    assert!(cc.perpetual_contract.iter().any(|c| c.category.is_none()));
    assert_eq!(
        cc.prediction_contract[0].contract_type,
        "PREDICTION_CONTRACT"
    );
    assert_eq!(cc.stock_contract[0].contract_type, "STOCK_CONTRACT");

    let usdt = cc.assets.iter().find(|a| a.token == "USDT").unwrap();
    assert_eq!(usdt.decimals, 18);
    assert!(usdt.cross_collateral_discount_rate.is_none());
    assert!(cc.assets.iter().any(|a| a
        .cross_collateral_discount_rate
        .as_ref()
        .is_some_and(|d| !d.is_empty())));
    assert!(!symbols.spot_config.assets.is_empty());
}

#[test]
fn parses_depth() {
    let depth: ApexDepth = data(DEPTH);
    assert_eq!(depth.s, "BTCUSDT");
    assert_eq!(
        depth.a.as_ref().unwrap()[0],
        ["65010.4".to_string(), "0.010".to_string()]
    );
    assert_eq!(depth.b.as_ref().unwrap().len(), 5);
    assert_eq!(depth.u, 13394263);

    let unknown: ApexDepth = data(DEPTH_UNKNOWN);
    assert!(unknown.a.is_none() && unknown.b.is_none());
    assert_eq!(unknown.s, "");
}

#[test]
fn parses_trades() {
    let trades: Vec<ApexTrade> = data(TRADES);
    assert_eq!(trades.len(), 3);
    assert_eq!(trades[0].side, "Sell");
    assert_eq!(trades[0].p, "65021.1");
    assert_eq!(trades[0].time, 1794780176010);
}

#[test]
fn parses_klines() {
    let klines: HashMap<String, Vec<ApexKline>> = data(KLINES);
    let btc = &klines["BTCUSDT"];
    assert_eq!(btc.len(), 3);
    assert_eq!(btc[0].i, "1");
    assert_eq!(btc[0].t, 1794780060000);
    assert_eq!(btc[0].tr, "126320.3605");
}

#[test]
fn parses_ticker() {
    let ticker: Vec<ApexTicker> = data(TICKER);
    assert_eq!(ticker[0].symbol, "BTCUSDT");
    assert_eq!(ticker[0].funding_rate, "0.0000125");
    assert_eq!(ticker[0].next_funding_time, "2026-11-15T23:00:00Z");
    assert_eq!(ticker[0].oracle_price, "");
}

#[test]
fn parses_funding_history() {
    let funding: ApexFundingHistory = data(FUNDING);
    assert_eq!(funding.history_funds.len(), 3);
    assert_eq!(funding.history_funds[0].symbol, "BTC-USDT");
    assert_eq!(funding.history_funds[0].funding_time, 1794780000000);
    assert_eq!(funding.total_size, 4);
}

#[test]
fn surfaces_error_envelope() {
    let err = serde_json::from_str::<ApexEnvelope<ApexFundingHistory>>(ERROR)
        .unwrap()
        .into_result()
        .unwrap_err();
    assert_eq!(err.code, 3);
    assert_eq!(err.msg, "invalid symbol: BTCUSDT");
}

#[test]
fn rejects_missing_fields() {
    let r = serde_json::from_str::<ApexEnvelope<ApexTrade>>(r#"{"data":{"p":"1"},"timeCost":1}"#);
    assert!(r.is_err());
}

#[test]
fn parses_ws_book() {
    let snap: ApexWsPush<ApexWsBook> = serde_json::from_str(WS_BOOK_SNAPSHOT).unwrap();
    assert_eq!(snap.topic, "orderBook25.H.BTCUSDT");
    assert_eq!(snap.kind, "snapshot");
    assert!(!snap.data.b.is_empty() && !snap.data.a.is_empty());
    assert!(snap.cs.is_some());

    let delta: ApexWsPush<ApexWsBook> = serde_json::from_str(WS_BOOK_DELTA).unwrap();
    assert_eq!(delta.kind, "delta");
    assert_eq!(delta.data.b[0], ["65000.5".to_string(), "0".to_string()]);
    assert!(delta.data.a.is_empty());
}

#[test]
fn parses_ws_trades() {
    let snap: ApexWsPush<Vec<ApexWsTrade>> = serde_json::from_str(WS_TRADES_SNAPSHOT).unwrap();
    assert_eq!(snap.data.len(), 50);
    let delta: ApexWsPush<Vec<ApexWsTrade>> = serde_json::from_str(WS_TRADES_DELTA).unwrap();
    assert_eq!(delta.data[0].tick_direction, "MinusTick");
    assert_eq!(delta.data[0].side, "Sell");
}

#[test]
fn parses_ws_ticker() {
    let snap: ApexWsPush<ApexWsTicker> = serde_json::from_str(WS_TICKER_SNAPSHOT).unwrap();
    assert_eq!(snap.data.funding_rate.as_deref(), Some("0.0000125"));
    assert_eq!(snap.data.mark_price.as_deref(), Some("65052.48"));
    let delta: ApexWsPush<ApexWsTicker> = serde_json::from_str(WS_TICKER_DELTA).unwrap();
    assert_eq!(delta.kind, "delta");
    assert!(delta.data.last_price.is_none());
    assert_eq!(delta.data.index_price.as_deref(), Some("65069.99"));
}

#[test]
fn parses_ws_candle() {
    let candle: ApexWsPush<Vec<ApexWsCandle>> = serde_json::from_str(WS_CANDLE).unwrap();
    assert!(candle.cs.is_none());
    assert_eq!(candle.data[0].interval, "1");
    assert!(!candle.data[0].confirm);
}

#[test]
fn parses_ws_all_tickers() {
    let all: ApexWsPush<Vec<ApexWsCompactTicker>> = serde_json::from_str(WS_ALL_TICKERS).unwrap();
    assert_eq!(all.topic, "instrumentInfo.all");
    assert_eq!(all.data.len(), 4);
    assert!(all.data.iter().all(|t| t.fr.is_some() && t.mp.is_some()));
}

#[test]
fn parses_ws_responses() {
    let sub: ApexWsResponse = serde_json::from_str(WS_SUBSCRIBE).unwrap();
    assert!(sub.success);
    assert_eq!(sub.request.op, "subscribe");
    assert_eq!(sub.request.args.as_ref().unwrap().len(), 6);
    let unsub: ApexWsResponse = serde_json::from_str(WS_UNSUBSCRIBE).unwrap();
    assert_eq!(
        unsub.request.args,
        Some(vec!["candle.1.BTCUSDT".to_string()])
    );
    let pong: ApexWsResponse = serde_json::from_str(WS_PONG).unwrap();
    assert_eq!(pong.ret_msg, "pong");
}

#[test]
fn ws_requests_match_captured_formats() {
    let topics = vec![
        ApexTopic::order_book("BTCUSDT", ApexBookDepth::D25, ApexWsFrequency::High),
        ApexTopic::trades("BTCUSDT", ApexWsFrequency::High),
        ApexTopic::ticker("BTCUSDT", ApexWsFrequency::High),
        ApexTopic::candle("BTCUSDT", "1"),
        ApexTopic::all_tickers(),
        ApexTopic::order_book("BTCUSDT", ApexBookDepth::D200, ApexWsFrequency::High),
    ];
    let sub: ApexWsResponse = serde_json::from_str(WS_SUBSCRIBE).unwrap();
    assert_eq!(Some(topics.clone()), sub.request.args);
    assert_eq!(
        ApexWsRequest::subscribe(&topics[..1]).to_json(),
        r#"{"op":"subscribe","args":["orderBook25.H.BTCUSDT"]}"#
    );
    assert_eq!(
        ApexWsRequest::pong_reply(r#"{"op":"ping","args":["1794780503937"]}"#).as_deref(),
        Some(r#"{"op":"pong","args":["1794780503937"]}"#)
    );
    assert!(ApexWsRequest::pong_reply(WS_PONG).is_none());
}

#[test]
fn parses_documented_private_push() {
    // Example from the "Successful Order Execution Push" section of the API docs.
    let json = r#"{"type":"delta","timestamp":1647502440973,"topic":"ws_zk_accounts_v3","contents":{
        "fills":[{"symbol":"ETH-USDT","side":"BUY","orderId":"2048046080","fee":"0.625000","liquidity":"TAKER",
          "accountId":"1024000","createdAt":1652185521361,"isOpen":true,"size":"0.500","price":"2500.0",
          "quoteAmount":"1250.0000","id":"2048000182272","updatedAt":1652185678345}],
        "positions":[{"symbol":"ETH-USDT","exitPrice":"0","side":"LONG","maxSize":"2820.000","sumOpen":"1.820",
          "sumClose":"0.000","netFunding":"0.000000","entryPrice":"2500.000000000000000000","accountId":"1024000",
          "createdAt":1652179377769,"size":"1.820","realizedPnl":"0","closedAt":1652185521392,"updatedAt":1652185521392}],
        "orders":[{"symbol":"ETH-USDT","cumSuccessFillFee":"0.625000","trailingPercent":"0","type":"LIMIT",
          "unfillableAt":1654779600000,"isDeleverage":false,"createdAt":1652185521339,"price":"2500.0",
          "cumSuccessFillValue":"1250.0000","id":"2048046080","cancelReason":"","timeInForce":1,
          "updatedAt":1652185521392,"limitFee":"0.625000","side":"BUY","clientOrderId":"522843990",
          "triggerPrice":"","expiresAt":1654779600000,"cumSuccessFillSize":"0.500","accountId":"1024000",
          "size":"0.500","reduceOnly":false,"isLiquidate":false,"remainingSize":"0.000","status":"FILLED"}],
        "accounts":[],"transfers":[]}}"#;
    let push: ApexWsPrivatePush<ApexWsAccountContents> = serde_json::from_str(json).unwrap();
    assert_eq!(push.topic, APEX_TOPIC_ACCOUNT);
    let contents = push.contents;
    assert_eq!(contents.fills.unwrap()[0].liquidity, "TAKER");
    assert_eq!(contents.orders.unwrap()[0].status, "FILLED");
    assert_eq!(contents.positions.unwrap()[0].side, "LONG");
    assert!(contents.contract_wallets.is_none());
}

#[test]
fn dispatches_public_ws_messages() {
    use ApexWsPublicMessage as M;
    assert!(matches!(M::parse(WS_BOOK_SNAPSHOT).unwrap(), M::Book(_)));
    match M::parse(WS_BOOK200).unwrap() {
        M::Book(b) => {
            assert_eq!(b.topic, "orderBook200.H.BTCUSDT");
            assert_eq!(b.kind, "snapshot");
            assert_eq!(b.data.b.len(), 5);
        }
        other => panic!("{other:?}"),
    }
    assert!(matches!(M::parse(WS_TRADES_DELTA).unwrap(), M::Trades(_)));
    assert!(matches!(M::parse(WS_TICKER_DELTA).unwrap(), M::Ticker(_)));
    assert!(matches!(
        M::parse(WS_ALL_TICKERS).unwrap(),
        M::AllTickers(_)
    ));
    match M::parse(WS_CANDLE_CONFIRM).unwrap() {
        M::Candles(c) => {
            assert_eq!(c.data.len(), 2);
            assert!(c.data[0].confirm && !c.data[1].confirm);
        }
        other => panic!("{other:?}"),
    }
    match M::parse(WS_SERVER_PING).unwrap() {
        M::Ping(p) => assert_eq!(p.args, vec!["1794826811517".to_string()]),
        other => panic!("{other:?}"),
    }
    assert!(matches!(M::parse(WS_PONG).unwrap(), M::Response(r) if r.success));
    match M::parse(WS_SUBSCRIBE_ERROR).unwrap() {
        M::Response(r) => {
            assert!(!r.success);
            assert_eq!(r.ret_msg, "error:handler not found");
            assert_eq!(
                r.request.args,
                Some(vec!["orderBook25.M.ETHUSDT".to_string()])
            );
        }
        other => panic!("{other:?}"),
    }
    match M::parse(WS_DECODE_ERROR).unwrap() {
        M::Response(r) => {
            assert!(!r.success && r.request.args.is_none() && r.request.op.is_empty());
        }
        other => panic!("{other:?}"),
    }
    // `args` may be null but must be present.
    let no_args = WS_DECODE_ERROR.replacen(r#""args""#, r#""zz""#, 1);
    assert!(ApexWsPublicMessage::parse(&no_args).is_err());
    assert!(M::parse(r#"{"topic":"mystery.X","type":"snapshot","data":{},"ts":1}"#).is_err());
}

#[test]
fn ws_pushes_reject_missing_fields() {
    let cases = [
        (WS_BOOK_DELTA, r#""u""#),
        (WS_TRADES_DELTA, r#""L""#),
        (WS_TICKER_DELTA, r#""symbol""#),
        (WS_ALL_TICKERS, r#""s""#),
        (WS_CANDLE_CONFIRM, r#""confirm""#),
        (WS_SUBSCRIBE_ERROR, r#""success""#),
    ];
    for (json, key) in cases {
        assert!(json.contains(key), "{key}");
        let broken = json.replacen(key, r#""zz""#, 1);
        assert!(ApexWsPublicMessage::parse(&broken).is_err(), "{key}");
    }
}
