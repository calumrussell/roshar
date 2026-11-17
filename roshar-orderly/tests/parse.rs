//! Offline parse tests against live captures in `tests/fixtures` (2026-11-15, mainnet).

use roshar_orderly::types::*;
use serde::de::DeserializeOwned;
use serde::Deserialize;

#[derive(Deserialize)]
struct Envelope<T> {
    success: bool,
    data: T,
}

#[derive(Deserialize)]
struct RowsOf<T> {
    rows: Vec<T>,
}

fn data<T: DeserializeOwned>(json: &str) -> T {
    let env: Envelope<T> = serde_json::from_str(json).expect("parse");
    assert!(env.success);
    env.data
}

fn rows<T: DeserializeOwned>(json: &str) -> Vec<T> {
    let rows: RowsOf<T> = data(json);
    assert!(!rows.rows.is_empty());
    rows.rows
}

fn push<T: DeserializeOwned>(json: &str) -> OrderlyWsPush<T> {
    serde_json::from_str(json).expect("parse push")
}

#[test]
fn symbols() {
    let all: Vec<OrderlySymbol> = rows(include_str!("fixtures/rest_public_info.json"));
    assert!(all.iter().any(|s| s.symbol == "PERP_BTC_USDC"));
    assert!(all.iter().any(|s| s.broker_id.is_some()));
    assert!(all.iter().any(|s| s.base_imr_on_market.is_some()));
    let btc: OrderlySymbol = data(include_str!("fixtures/rest_public_info_symbol.json"));
    assert_eq!(btc.symbol, "PERP_BTC_USDC");
    assert!(btc.quote_tick > 0.0 && btc.base_tick > 0.0);
}

#[test]
fn futures() {
    let all: Vec<OrderlyFuture> = rows(include_str!("fixtures/rest_public_futures.json"));
    assert!(all.len() > 50);
    let btc: OrderlyFuture = data(include_str!("fixtures/rest_public_futures_symbol.json"));
    assert_eq!(btc.symbol, "PERP_BTC_USDC");
    assert!(btc.mark_price > 0.0 && btc.volume_24h >= 0.0);
}

#[test]
fn market_trades() {
    let trades: Vec<OrderlyMarketTrade> = rows(include_str!("fixtures/rest_market_trades.json"));
    assert_eq!(trades.len(), 5);
    assert!(trades.iter().all(|t| t.side == "BUY" || t.side == "SELL"));
}

#[test]
fn funding() {
    let all: Vec<OrderlyFundingRate> = rows(include_str!("fixtures/rest_funding_rates.json"));
    assert!(all.len() > 50);
    let one: OrderlyFundingRate = data(include_str!("fixtures/rest_funding_rate_symbol.json"));
    assert_eq!(one.symbol, "PERP_BTC_USDC");
    let hist: OrderlyPage<OrderlyFundingRateHistory> =
        data(include_str!("fixtures/rest_funding_rate_history.json"));
    assert_eq!(hist.rows.len(), 3);
    assert_eq!(hist.meta.records_per_page, 3);
    assert_eq!(hist.meta.current_page, 1);
    let summary: Vec<OrderlyFundingSummary> = rows(include_str!(
        "fixtures/rest_market_info_funding_history.json"
    ));
    assert!(summary.iter().any(|s| s.funding.d180.is_none()));
    assert!(summary.iter().any(|s| s.funding.d180.is_some()));
}

#[test]
fn market_info() {
    let changes: Vec<OrderlyPriceChanges> = rows(include_str!("fixtures/rest_price_changes.json"));
    assert!(changes.iter().any(|c| c.d30.is_none()));
    let oi: Vec<OrderlyTradersOpenInterest> =
        rows(include_str!("fixtures/rest_traders_open_interests.json"));
    assert!(oi.iter().all(|r| r.short_oi <= 0.0));
    let sys: OrderlySystemInfo = data(include_str!("fixtures/rest_system_info.json"));
    assert_eq!(sys.status, 0);
}

#[test]
fn tv_bars() {
    for json in [
        include_str!("fixtures/rest_tv_kline_history.json"),
        include_str!("fixtures/rest_tv_history.json"),
    ] {
        let bars: OrderlyTvBars = serde_json::from_str(json).unwrap();
        assert_eq!(bars.s, "ok");
        let n = bars.t.len();
        assert!(n > 0);
        for col in [&bars.o, &bars.h, &bars.l, &bars.c, &bars.v, &bars.a] {
            assert_eq!(col.len(), n);
        }
    }
    let empty: OrderlyTvBars =
        serde_json::from_str(include_str!("fixtures/rest_tv_history_no_data.json")).unwrap();
    assert_eq!(empty.s, "no_data");
    assert!(empty.t.is_empty());
}

#[test]
fn ws_control_frames() {
    let ack: OrderlyWsResponse =
        serde_json::from_str(include_str!("fixtures/ws_subscribe_ack.json")).unwrap();
    assert!(ack.success);
    assert_eq!(ack.event.as_deref(), Some("subscribe"));
    let err: OrderlyWsResponse =
        serde_json::from_str(include_str!("fixtures/ws_subscribe_error.json")).unwrap();
    assert!(!err.success);
    assert!(err.event.is_none());
    assert_eq!(
        err.error_msg.as_deref(),
        Some("invalid symbol PERP_NOPE_USDC")
    );
    for (json, event) in [
        (include_str!("fixtures/ws_ping.json"), "ping"),
        (include_str!("fixtures/ws_pong.json"), "pong"),
    ] {
        let hb: OrderlyWsHeartbeat = serde_json::from_str(json).unwrap();
        assert_eq!(hb.event, event);
    }
    let book: OrderlyWsBookRequestReply =
        serde_json::from_str(include_str!("fixtures/ws_request_orderbook.json")).unwrap();
    assert!(book.success);
    assert!(!book.data.asks.is_empty());
}

#[test]
fn ws_book() {
    let snap: OrderlyWsPush<OrderlyWsBook> = push(include_str!("fixtures/ws_orderbook.json"));
    assert_eq!(snap.topic, "PERP_BTC_USDC@orderbook");
    assert!(!snap.data.asks.is_empty() && !snap.data.bids.is_empty());
    assert!(snap.data.asks[0][0] > snap.data.bids[0][0]);
    let upd: OrderlyWsPush<OrderlyWsBookUpdate> =
        push(include_str!("fixtures/ws_orderbookupdate.json"));
    assert!(upd.data.prev_ts < upd.ts);
}

#[test]
fn ws_market() {
    let trade: OrderlyWsPush<OrderlyWsTrade> = push(include_str!("fixtures/ws_trade.json"));
    assert_eq!(trade.topic, "PERP_ETH_USDC@trade");
    let ticker: OrderlyWsPush<OrderlyWsTicker> = push(include_str!("fixtures/ws_ticker.json"));
    assert_eq!(ticker.data.symbol, "PERP_ETH_USDC");
    let tickers: OrderlyWsPush<Vec<OrderlyWsTicker>> =
        push(include_str!("fixtures/ws_tickers.json"));
    assert!(tickers.data.len() > 50);
    let bbo: OrderlyWsPush<OrderlyWsBbo> = push(include_str!("fixtures/ws_bbo.json"));
    assert!(bbo.data.ask >= bbo.data.bid);
    let bbos: OrderlyWsPush<Vec<OrderlyWsBbo>> = push(include_str!("fixtures/ws_bbos.json"));
    assert!(bbos.data.iter().any(|b| b.down_bid.is_some()));
    let kline: OrderlyWsPush<OrderlyWsKline> = push(include_str!("fixtures/ws_kline.json"));
    assert_eq!(kline.data.interval, "1m");
    assert_eq!(kline.data.end_time - kline.data.start_time, 60_000);
    let mark: OrderlyWsPush<OrderlyWsPrice> = push(include_str!("fixtures/ws_markprice.json"));
    assert!(mark.data.price > 0.0);
    let marks: OrderlyWsPush<Vec<OrderlyWsPrice>> =
        push(include_str!("fixtures/ws_markprices.json"));
    assert!(marks.data.len() > 50);
    let index: OrderlyWsPush<OrderlyWsPrice> = push(include_str!("fixtures/ws_indexprice.json"));
    assert_eq!(index.data.symbol, "SPOT_BTC_USDC");
    let oi: OrderlyWsPush<OrderlyWsOpenInterest> =
        push(include_str!("fixtures/ws_openinterest.json"));
    assert!(oi.data.open_interest > 0.0);
    let fr: OrderlyWsPush<OrderlyWsEstFundingRate> =
        push(include_str!("fixtures/ws_estfundingrate.json"));
    assert!(fr.data.funding_ts > fr.ts);
}

#[test]
fn strict_parsing_rejects_missing_fields() {
    let r = serde_json::from_str::<OrderlyWsTrade>(
        r#"{"symbol":"PERP_BTC_USDC","price":1.0,"side":"BUY"}"#,
    );
    assert!(r.is_err());
}

#[test]
fn private_ws_doc_examples() {
    use roshar_orderly::types::trading::{
        OrderlyWsBalances, OrderlyWsExecutionReport, OrderlyWsPositions,
    };
    let er: OrderlyWsPush<OrderlyWsExecutionReport> =
        push(include_str!("fixtures/docs_ws_executionreport.json"));
    assert_eq!(er.topic, "executionreport");
    assert_eq!(er.data.status, "NEW");
    let pos: OrderlyWsPush<OrderlyWsPositions> =
        push(include_str!("fixtures/docs_ws_position.json"));
    assert_eq!(pos.data.positions[0].symbol, "PERP_ETH_USDC");
    let bal: OrderlyWsPush<OrderlyWsBalances> = push(include_str!("fixtures/docs_ws_balance.json"));
    assert!(bal.data.balances.contains_key("USDC"));
}

#[derive(Deserialize)]
struct QueryEnvelope<T> {
    success: bool,
    data: T,
    #[allow(dead_code)]
    ts: u64,
}

fn query<T: DeserializeOwned>(json: &str) -> T {
    let env: QueryEnvelope<T> = serde_json::from_str(json).expect("parse query");
    assert!(env.success);
    env.data
}

#[test]
fn more_market_data() {
    let charts: Vec<OrderlyHistoryChart> = rows(include_str!("fixtures/rest_history_charts.json"));
    assert!(charts.iter().any(|c| c.symbol == "PERP_BTC_USDC"));
    assert!(charts[0].price_list[0].ts > charts[0].price_list[1].ts);
    let index: Vec<OrderlyIndexPricePoint> = rows(include_str!("fixtures/rest_index_price.json"));
    assert!(index.windows(2).all(|w| w[0].timestamp < w[1].timestamp));
    let sources: Vec<OrderlyIndexPriceSource> =
        rows(include_str!("fixtures/rest_index_price_source.json"));
    assert!(sources.iter().all(|s| !s.sources.is_empty()));
    let fund: OrderlyInsuranceFund = data(include_str!("fixtures/rest_insurance_fund.json"));
    assert!(fund.balance > 0.0 && !fund.rows.is_empty());
    let cfg: OrderlyLeverageConfig = data(include_str!("fixtures/rest_config.json"));
    assert!(cfg.available_futures_leverage.contains(','));
    let max: OrderlyMaxLeverage = data(include_str!("fixtures/rest_leverage.json"));
    assert_eq!(max.max_futures_leverage, "100");
    let tokens: Vec<OrderlyToken> = rows(include_str!("fixtures/rest_token.json"));
    assert!(tokens.iter().any(|t| t.discount_factor.is_none()));
    assert!(!tokens[0].chain_details.is_empty());
    let chains: Vec<OrderlyChainInfo> = rows(include_str!("fixtures/rest_chain_info.json"));
    assert_eq!(chains[0].chain_id, "42161");
}

#[test]
fn liquidations() {
    let done: OrderlyPage<OrderlyLiquidatedPosition> =
        data(include_str!("fixtures/rest_liquidated_positions.json"));
    assert!(done.rows.iter().any(|r| r.positions_by_perp.len() > 1));
    // `transfer_price` is absent (not null) on some live positions.
    let perps = done.rows.iter().flat_map(|r| &r.positions_by_perp);
    assert!(perps.clone().any(|p| p.transfer_price.is_none()));
    assert!(perps.clone().any(|p| p.transfer_price.is_some()));
    let empty: OrderlyPage<OrderlyLiquidation> =
        data(include_str!("fixtures/rest_liquidation_empty.json"));
    assert!(empty.rows.is_empty());
    let docs: OrderlyPage<OrderlyLiquidation> =
        data(include_str!("fixtures/docs_rest_liquidation.json"));
    assert_eq!(docs.rows[0].positions_by_perp[0].symbol, "PERP_BTC_USDC");
}

#[test]
fn predict_rounds() {
    let rounds: Vec<OrderlyBinaryRound> = rows(include_str!("fixtures/rest_binary_info.json"));
    assert!(rounds.iter().any(|r| r.outcome.is_none() && r.s0.is_none()));
    assert!(rounds.iter().any(|r| r.outcome.as_deref() == Some("DOWN")));
    let one: OrderlyBinaryRoundDetail = data(include_str!("fixtures/rest_binary_info_symbol.json"));
    assert_eq!(one.round.product_type, "BINARY");
    assert!(one.price_evidence.is_none());
    // Nullable fields are always sent: null parses, a missing key does not.
    let mut v: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/rest_binary_info_symbol.json")).unwrap();
    let d = v["data"].as_object_mut().unwrap();
    assert!(serde_json::from_value::<OrderlyBinaryRoundDetail>(d.clone().into()).is_ok());
    d.remove("s0");
    assert!(serde_json::from_value::<OrderlyBinaryRoundDetail>(d.clone().into()).is_err());
}

#[test]
fn tradingview_meta() {
    let info: OrderlyTvSymbolInfo =
        serde_json::from_str(include_str!("fixtures/rest_tv_symbol_info.json")).unwrap();
    assert_eq!(info.s, "ok");
    let n = info.symbol.len();
    assert!(n > 0);
    assert_eq!(info.pricescale.len(), n);
    assert_eq!(info.session_regular.len(), n);
    let cfg: OrderlyTvConfig =
        serde_json::from_str(include_str!("fixtures/rest_tv_config.json")).unwrap();
    assert!(cfg.supported_resolutions.iter().any(|r| r == "1D"));
}

#[test]
fn public_info_api() {
    let summary: OrderlyQueryMarketSummary =
        query(include_str!("fixtures/query_market_summary.json"));
    assert!(summary.total_open_interest.is_some());
    assert!(summary.markets.iter().any(|m| m.bid_price.is_none()));
    assert!(summary.markets.iter().any(|m| m.symbol == "PERP_BTC_USDC"));
    let book: OrderlyQueryOrderbook = query(include_str!("fixtures/query_orderbook.json"));
    assert_eq!(book.asks.len(), 3);
    assert!(book.mid_price.is_some());
    let candles: OrderlyQueryPage<OrderlyQueryCandle> =
        query(include_str!("fixtures/query_candles.json"));
    assert!(candles.next_cursor.is_some());
    let trades: OrderlyQueryPage<OrderlyQueryTrade> =
        query(include_str!("fixtures/query_market_trades.json"));
    assert!(trades.next_cursor.is_none());
    let liqs: OrderlyQueryPage<OrderlyQueryLiquidation> =
        query(include_str!("fixtures/query_liquidations.json"));
    assert!(liqs
        .rows
        .iter()
        .all(|r| r.side == "LONG" || r.side == "SHORT"));
    let frh: OrderlyQueryPage<OrderlyQueryFundingRate> =
        query(include_str!("fixtures/query_funding_rate_history.json"));
    assert_eq!(frh.rows.len(), 3);
    #[derive(Deserialize)]
    struct Rows2<T> {
        rows: Vec<T>,
    }
    let fc: Rows2<OrderlyQueryFundingComparison> =
        query(include_str!("fixtures/query_funding_comparison.json"));
    assert!(fc.rows[0].exchanges.iter().any(|e| e.name == "binance"));
    let detail: OrderlyQueryMarketDetail = query(include_str!("fixtures/query_market_detail.json"));
    assert!(detail.market_info.is_some() && detail.candles.is_some());
    let partial: OrderlyQueryMarketDetail =
        serde_json::from_str(r#"{"symbol":"PERP_BTC_USDC"}"#).unwrap();
    assert!(partial.orderbook.is_none());
}

#[test]
fn new_types_reject_missing_fields() {
    // REST (enveloped rows)
    assert!(serde_json::from_str::<OrderlyIndexPricePoint>(r#"{"timestamp":1}"#).is_err());
    assert!(serde_json::from_str::<OrderlyLiquidatedPerp>(
        r#"{"symbol":"PERP_LIT_USDC","position_qty":1,"liquidator_fee":0.006}"#
    )
    .is_err());
    assert!(serde_json::from_str::<OrderlyInsuranceFund>(r#"{"rows":[]}"#).is_err());
    assert!(serde_json::from_str::<OrderlyToken>(r#"{"token":"USDC"}"#).is_err());
    assert!(serde_json::from_str::<OrderlyChainInfo>(r#"{"name":"Arbitrum"}"#).is_err());
    assert!(serde_json::from_str::<OrderlyBinaryRound>(r#"{"symbol":"x"}"#).is_err());
    assert!(serde_json::from_str::<OrderlyTvConfig>(r#"{"supported_resolutions":[]}"#).is_err());
    // Public Info API
    assert!(serde_json::from_str::<OrderlyQueryCandle>(
        r#"{"open":"1","high":"1","low":"1","close":"1","timestamp":1}"#
    )
    .is_err());
    assert!(serde_json::from_str::<OrderlyQueryMarketSummary>(
        r#"{"total_24h_volume":null,"total_open_interest":null,"markets":[]}"#
    )
    .is_ok());
    assert!(serde_json::from_str::<OrderlyQueryMarketSummary>(r#"{"markets":[]}"#).is_err());
    assert!(
        serde_json::from_str::<OrderlyQueryPage<OrderlyQueryCandle>>(r#"{"rows":[]}"#).is_err()
    );
    assert!(serde_json::from_str::<OrderlyToken>(
        r#"{"token":"USDC","token_hash":"0x","decimals":6,"minimum_withdraw_amount":1,"base_weight":1,"haircut":0,"user_max_qty":-1,"is_collateral":true,"on_chain_swap":false,"chain_details":[]}"#
    )
    .is_err());
    assert!(serde_json::from_str::<OrderlyQueryMarket>(r#"{"symbol":"PERP_BTC_USDC"}"#).is_err());
    // Websocket
    assert!(serde_json::from_str::<OrderlyWsLiquidation>(
        r#"{"liquidationId":1,"timestamp":1,"type":"liquidated"}"#
    )
    .is_err());
    assert!(OrderlyWsMessage::parse(
        r#"{"topic":"traders_open_interests","ts":1,"data":[{"symbol":"PERP_BTC_USDC","long_oi":1}]}"#
    )
    .is_err());
}

#[test]
fn ws_message_enum() {
    let parse = |s: &str| OrderlyWsMessage::parse(s).unwrap();
    assert!(matches!(
        parse(include_str!("fixtures/ws_subscribe_ack.json")),
        OrderlyWsMessage::Response(r) if r.success
    ));
    assert!(matches!(
        parse(include_str!("fixtures/ws_subscribe_invalid_topic.json")),
        OrderlyWsMessage::Response(r) if r.error_msg.as_deref() == Some("invalid topic")
    ));
    assert!(matches!(
        parse(include_str!("fixtures/ws_ping.json")),
        OrderlyWsMessage::Heartbeat(_)
    ));
    assert!(matches!(
        parse(include_str!("fixtures/ws_pong.json")),
        OrderlyWsMessage::Heartbeat(_)
    ));
    assert!(matches!(
        parse(include_str!("fixtures/ws_request_orderbook.json")),
        OrderlyWsMessage::BookRequest(_)
    ));
    macro_rules! is {
        ($file:literal, $variant:ident) => {
            assert!(
                matches!(
                    parse(include_str!(concat!("fixtures/", $file))),
                    OrderlyWsMessage::$variant(_)
                ),
                $file
            );
        };
    }
    is!("ws_orderbook.json", Orderbook);
    is!("ws_orderbookupdate.json", OrderbookUpdate);
    is!("ws_trade.json", Trade);
    is!("ws_ticker.json", Ticker);
    is!("ws_tickers.json", Tickers);
    is!("ws_bbo.json", Bbo);
    is!("ws_bbos.json", Bbos);
    is!("ws_kline.json", Kline);
    is!("ws_estfundingrate.json", EstFundingRate);
    is!("ws_markprice.json", MarkPrice);
    is!("ws_markprices.json", MarkPrices);
    is!("ws_indexprice.json", IndexPrice);
    is!("ws_openinterest.json", OpenInterest);
    match parse(include_str!("fixtures/ws_builder_ticker.json")) {
        OrderlyWsMessage::Ticker(t) => {
            assert_eq!(t.topic, "woofi_pro$PERP_ETH_USDC@ticker");
            assert_eq!(t.data.symbol, "PERP_ETH_USDC");
        }
        other => panic!("{other:?}"),
    }
    match parse(include_str!("fixtures/ws_builder_tickers.json")) {
        OrderlyWsMessage::Tickers(t) => assert_eq!(t.topic, "woofi_pro$tickers"),
        other => panic!("{other:?}"),
    }
    match parse(include_str!("fixtures/ws_price_changes.json")) {
        OrderlyWsMessage::PriceChanges(p) => {
            assert!(p.data[0].h24.is_some());
            // Live pushes omit windows older than the symbol instead of sending null.
            assert!(p.data.iter().any(|r| r.d30.is_none() && r.m5.is_some()));
            assert!(p.data.iter().any(|r| r.m5.is_none()));
        }
        other => panic!("{other:?}"),
    }
    match parse(include_str!("fixtures/ws_traders_open_interests.json")) {
        OrderlyWsMessage::TradersOpenInterests(p) => {
            assert!(p.data.iter().all(|r| r.short_oi <= 0.0))
        }
        other => panic!("{other:?}"),
    }
    match parse(include_str!("fixtures/ws_history_charts.json")) {
        OrderlyWsMessage::HistoryCharts(p) => {
            assert_eq!(p.topic, "history_charts_1h");
            assert!(!p.data[0].price_list.is_empty());
        }
        other => panic!("{other:?}"),
    }
    match parse(include_str!("fixtures/docs_ws_liquidation.json")) {
        OrderlyWsMessage::Liquidation(p) => {
            assert_eq!(p.data[0].positions_by_perp[0].position_qty, 12.6)
        }
        other => panic!("{other:?}"),
    }
    match parse(include_str!("fixtures/docs_ws_maintenance_status.json")) {
        OrderlyWsMessage::MaintenanceStatus(p) => {
            assert_eq!(p.data[0].status, 2);
            assert!(p.data[0].scheduled_maintenance.is_some());
        }
        other => panic!("{other:?}"),
    }
    assert!(OrderlyWsMessage::parse(r#"{"topic":"announcement","ts":1,"data":[]}"#).is_err());
}
