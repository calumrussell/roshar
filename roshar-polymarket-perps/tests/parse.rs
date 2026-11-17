//! Offline parse tests. `tests/fixtures/*.json` are live captures (2026-11-15/16, from the UK);
//! `tests/fixtures/docs/*.json` are the documented examples of private responses, which need a
//! funded account to capture.

use roshar_polymarket_perps::types::*;
use serde::de::DeserializeOwned;

fn parse<T: DeserializeOwned>(json: &str) -> T {
    serde_json::from_str(json).unwrap_or_else(|e| panic!("{e}: {json}"))
}

macro_rules! fixture {
    ($name:literal) => {
        include_str!(concat!("fixtures/", $name, ".json"))
    };
}

#[test]
fn info_endpoints() {
    assert_eq!(parse::<PingResponse>(fixture!("ping")).status, "ok");
    assert_eq!(parse::<ServerTime>(fixture!("time")).time, 1794780054688);

    let ex: ExchangeInfo = parse(fixture!("exchange"));
    assert_eq!(
        (ex.name.as_str(), ex.version.as_str(), ex.chain_id),
        ("Polymarket", "1", 137)
    );
    assert!(!ex.cancel_only);

    let assets: Vec<CollateralAsset> = parse(fixture!("assets"));
    assert_eq!(assets[0].asset, "pUSD");
    assert_eq!(assets[0].decimals, 6);

    let fees: FeesInfo = parse(fixture!("fees"));
    assert!(!fees.fee_schedule[0].tiers.is_empty());

    let tiers: Vec<LimitTier> = parse(fixture!("limit_tiers"));
    assert_eq!(tiers[0].open_orders_limit, 1000);

    assert!(!parse::<Registration>(fixture!("registered")).registered);

    let stats: ExchangeStatistics = parse(fixture!("exchange_stats"));
    assert!(stats.open_interest.is_some());
}

#[test]
fn instruments() {
    let ins: Vec<Instrument> = parse(fixture!("instruments"));
    assert_eq!(ins.len(), 3);
    assert_eq!(ins[0].instrument_id, 1);
    assert_eq!(ins[0].display_symbol.as_deref(), Some("USA500-USD"));
    assert!(ins[0].risk_tiers.len() > 1);
    assert!(ins[0].logo.is_some());
    assert!(ins[0].settlement.is_none());
    assert!(ins[1].display_symbol.is_none());
    let promo = ins[2].fee_override.as_ref().expect("fee override");
    assert!(promo.end_time.is_none());
    assert!(!promo.tiers.is_empty());
}

#[test]
fn tickers_statistics_bbo() {
    let t: Vec<Ticker> = parse(fixture!("tickers"));
    assert_eq!(t[0].funding_rate, "0.00000625");
    assert!(t[0].next_funding > t[0].timestamp);

    let s: Vec<Statistic> = parse(fixture!("statistics"));
    assert_eq!(s[0].klines.len(), 3);
    assert_eq!(s[0].klines[0].trades, 18);

    let b: Vec<Bbo> = parse(fixture!("bbo"));
    assert_eq!(b[0].bid_price, "7536.1");
}

#[test]
fn book_trades_klines_funding() {
    let book: Book = parse(fixture!("book"));
    assert_eq!(book.bids.len(), 10);
    assert_eq!(book.bids[0].price, "7536.1");
    assert!(book.asks[0].price > book.bids[0].price);

    let empty: Book = parse(fixture!("book_empty"));
    assert!(empty.bids.is_empty() && empty.asks.is_empty());

    let trades: Trades = parse(fixture!("trades"));
    assert!(trades.more);
    assert_eq!(trades.data[0].side, Side::Long);
    assert_eq!(trades.data[0].hash, "0x");

    let k: Klines = parse(fixture!("klines"));
    assert!(!k.more);
    assert_eq!(k.data[0].open_time, 1794758400000);

    let m: MarkHistory = parse(fixture!("mark_history"));
    assert_eq!(m.data[0].mark_price, "7515.4");

    let f: FundingHistory = parse(fixture!("funding"));
    assert_eq!(f.data[0].funding_rate, "0.00000625");

    let idx: IndexPrice = parse(fixture!("index"));
    assert_eq!(idx.asset, "BTC");
}

#[test]
fn fills() {
    // Public fills share the authenticated fills shape (no builder identity).
    let f: AccountFills = parse(fixture!("fills_public"));
    assert_eq!(f.data.len(), 3);
    assert!(f.data[0].taker);
    assert!(f.data[0].builder.is_none());
}

#[test]
fn private_documented_examples() {
    let p: Portfolio = parse(fixture!("docs/portfolio"));
    assert_eq!(p.positions[0].adl_index, 0);
    assert_eq!(p.margin.available_order_margin, "870");

    let b: Vec<Balance> = parse(fixture!("docs/balances"));
    assert_eq!(b[0].asset, "pUSD");

    let open: Vec<Order> = parse(fixture!("docs/open_orders"));
    assert_eq!(open[0].tif, TimeInForce::Gtc);
    assert!(open[0].client_order_id.is_none());

    let orders: Vec<Order> = parse(fixture!("docs/orders"));
    assert_eq!(orders[0].status, "filled");
    assert_eq!(
        orders[0].client_order_id.as_deref(),
        Some("7f9e4a2b6c8d0e1f1234567890abcdef")
    );

    let funding: AccountFundingHistory = parse(fixture!("docs/account_funding"));
    assert_eq!(funding.data[0].funding, "0.05");

    let ac: AutoCancelStatus = parse(fixture!("docs/auto_cancel_status"));
    assert_eq!(ac.daily_limit, 1000);
}

#[test]
fn trading_responses() {
    let ok: Vec<OrderAck> = parse(fixture!("docs/create_orders_ok"));
    assert!(matches!(
        &ok[0],
        OrderAck::Ok {
            oid: 1234567890,
            coid: Some(_),
            ..
        }
    ));

    let err: Vec<OrderAck> = parse(fixture!("docs/create_orders_err"));
    assert!(
        matches!(&err[0], OrderAck::Err { error, oid: None, .. } if error == "insufficient_margin")
    );

    let cancels: Vec<CancelResult> = parse(fixture!("docs/cancel_orders"));
    assert!(matches!(
        cancels[0],
        CancelResult::Ok {
            oid: 1234567890,
            ..
        }
    ));
    assert!(
        matches!(&cancels[1], CancelResult::Err { error, oid: Some(1234567891), .. } if error == "order_not_in_orderbook")
    );

    let modify: Vec<ModifyResult> = parse(fixture!("docs/modify_orders"));
    match &modify[0] {
        ModifyResult::Ok { order } => {
            assert_eq!((order.rest.as_str(), order.fill.as_str()), ("3", "4"))
        }
        other => panic!("{other:?}"),
    }
    assert!(matches!(&modify[1], ModifyResult::Err { error, .. } if error == "modify_would_cross"));

    assert_eq!(parse::<Ack>(r#"{"status":"ok"}"#), Ack::Ok);
    assert_eq!(
        parse::<AutoCancelResult>(fixture!("docs/auto_cancel")),
        AutoCancelResult::Ok {
            deadline: 1767000130000
        }
    );
}

#[test]
fn ws_public_updates() {
    let bbo: WsUpdate<WsBbo> = parse(fixture!("ws_bbo"));
    assert_eq!((bbo.ch.as_str(), bbo.data.iid), ("bbo::6", 6));

    let book: WsUpdate<WsBook> = parse(fixture!("ws_book"));
    assert_eq!(book.ch, "book::6::50");
    assert_eq!(book.data.a.len(), 5);

    let trades: WsTradesUpdate = parse(fixture!("ws_trades"));
    assert_eq!(trades.data[0].side, Side::Short);

    let ticker: WsUpdate<WsTicker> = parse(fixture!("ws_tickers"));
    assert_eq!(ticker.data.fr, "0.00000625");

    let klines: WsKlinesUpdate = parse(fixture!("ws_klines"));
    assert_eq!(klines.ch, "klines::6::1m");
    assert_eq!(klines.data[0].trades, 1);

    assert!(matches!(
        parse::<WsFrame>(fixture!("ws_bbo")),
        WsFrame::Update(_)
    ));
    assert!(matches!(
        parse::<WsFrame>(fixture!("ws_sub_ack")),
        WsFrame::Response(_)
    ));
}

#[test]
fn ws_statistics() {
    let s: WsStatisticsUpdate = parse(fixture!("ws_statistics"));
    assert_eq!((s.ch.as_str(), s.data.iid), ("statistics::6", 6));
    assert_eq!(s.data.open, "65533");
    assert_eq!(s.data.klines.len(), 3);
    assert_eq!(s.data.klines[0].trades, 370);

    // Strict: a required field removed is an error.
    let mut v: serde_json::Value = parse(fixture!("ws_statistics"));
    v["data"].as_object_mut().unwrap().remove("vol");
    assert!(serde_json::from_value::<WsStatisticsUpdate>(v).is_err());
}

#[test]
fn ws_responses() {
    let ack: WsResponse<Vec<Ack>> = parse(fixture!("ws_sub_ack"));
    assert_eq!(ack.id, Some(1));
    assert!(ack.data.iter().all(|a| *a == Ack::Ok));

    let mixed: WsResponse<Vec<Ack>> = parse(fixture!("ws_sub_ack_mixed"));
    assert_eq!(
        mixed.data[1],
        Ack::Err {
            error: "invalid channel".into()
        }
    );

    let pong: WsResponse<Pong> = parse(fixture!("ws_pong"));
    assert_eq!((pong.id, pong.data.status.as_str()), (Some(0), "ok"));

    let auth_err: WsResponse<AuthResult> = parse(fixture!("ws_auth_err"));
    assert_eq!(
        auth_err.data,
        AuthResult::Err {
            error: "unauthorized".into()
        }
    );
    let auth_ok: WsResponse<AuthResult> = parse(fixture!("docs/ws_auth_ok"));
    assert!(matches!(auth_ok.data, AuthResult::Ok { pod: Some(_) }));

    let order: WsOrdersUpdate = parse(fixture!("docs/ws_order"));
    assert_eq!(order.data.status, "open");
}

#[test]
fn ws_request_builders() {
    assert_eq!(
        subscribe(
            Some(1),
            &[
                bbo_channel(1),
                trades_channel(1),
                klines_channel(1, KlineInterval::M1)
            ]
        ),
        r#"{"id":1,"req":"sub","chs":["bbo::1","trades::1","klines::1::1m"]}"#
    );
    assert_eq!(
        unsubscribe(
            None,
            &[
                book_channel(6, Some(50)),
                tickers_channel(InstrumentScope::All)
            ]
        ),
        r#"{"req":"unsub","chs":["book::6::50","tickers::all"]}"#
    );
    assert_eq!(book_channel(6, None), "book::6");
    assert_eq!(statistics_channel(InstrumentScope::One(3)), "statistics::3");
    assert_eq!(
        ping(Some(0)),
        r#"{"id":0,"req":"post","op":{"type":"ping"}}"#
    );
    assert_eq!(
        auth(Some(1), "0xabc", "s3cret"),
        r#"{"id":1,"req":"post","op":{"type":"auth","args":{"proxy":"0xabc","secret":"s3cret"}}}"#
    );
    assert_eq!(
        subscribe(Some(2), &[CHANNEL_ORDERS, CHANNEL_FILLS]),
        r#"{"id":2,"req":"sub","chs":["orders","fills"]}"#
    );
}
