//! Public market-data websocket requests and their JSON replies, captured from prod
//! (`wss://ws.api.prod.paradex.trade/v1?sbeSchemaId=1&sbeSchemaVersion=3`) on 2026-11-16.

use roshar_paradex::types::{
    ParadexBookFeed, ParadexBookRefresh, ParadexFundingComparisonRefresh, ParadexSystemStatus,
    ParadexWssMessage,
};
use roshar_paradex::ws_control::{parse, ParadexWsControl, ParadexWsNotification};
use serde_json::{json, Value};

fn request(msg: &ParadexWssMessage) -> Value {
    serde_json::from_str(&msg.to_json()).unwrap()
}

fn req(method: &str, params: Value) -> Value {
    json!({"id": 1, "jsonrpc": "2.0", "method": method, "params": params})
}

#[test]
fn request_builders() {
    let cases = [
        (
            ParadexWssMessage::bbo_unsub("BTC-USD-PERP"),
            req("unsubscribe", json!({"channel": "bbo.BTC-USD-PERP"})),
        ),
        (
            ParadexWssMessage::funding_unsub("BTC-USD-PERP"),
            req(
                "unsubscribe",
                json!({"channel": "funding_data.BTC-USD-PERP"}),
            ),
        ),
        (
            ParadexWssMessage::markets_summary(None),
            req("subscribe", json!({"channel": "markets_summary"})),
        ),
        (
            ParadexWssMessage::markets_summary(Some("BTC-USD-PERP")),
            req(
                "subscribe",
                json!({"channel": "markets_summary.BTC-USD-PERP"}),
            ),
        ),
        (
            ParadexWssMessage::markets_summary_unsub(None),
            req("unsubscribe", json!({"channel": "markets_summary"})),
        ),
        (
            ParadexWssMessage::markets_summary_unsub(Some("BTC-USD-PERP")),
            req(
                "unsubscribe",
                json!({"channel": "markets_summary.BTC-USD-PERP"}),
            ),
        ),
        (
            ParadexWssMessage::markets_summary_scoped(&["ETH-USD-PERP", "SOL-USD-PERP"]),
            req(
                "subscribe",
                json!({"channel": "markets_summary", "markets": ["ETH-USD-PERP", "SOL-USD-PERP"]}),
            ),
        ),
        (
            ParadexWssMessage::modify_subscription(
                "markets_summary",
                Some(&["BTC-USD-PERP"][..]),
                Some(&["SOL-USD-PERP"][..]),
            ),
            req(
                "modify_subscription",
                json!({"channel": "markets_summary", "add": ["BTC-USD-PERP"], "remove": ["SOL-USD-PERP"]}),
            ),
        ),
        (
            ParadexWssMessage::modify_subscription(
                "markets_summary",
                Some(&["SOL-USD-PERP"][..]),
                None,
            ),
            req(
                "modify_subscription",
                json!({"channel": "markets_summary", "add": ["SOL-USD-PERP"]}),
            ),
        ),
        (
            ParadexWssMessage::markets(None),
            req("subscribe", json!({"channel": "markets"})),
        ),
        (
            ParadexWssMessage::markets(Some("BTC-USD-PERP")),
            req("subscribe", json!({"channel": "markets.BTC-USD-PERP"})),
        ),
        (
            ParadexWssMessage::markets_unsub(None),
            req("unsubscribe", json!({"channel": "markets"})),
        ),
        (
            ParadexWssMessage::system_status(),
            req("subscribe", json!({"channel": "system_status"})),
        ),
        (
            ParadexWssMessage::system_status_unsub(),
            req("unsubscribe", json!({"channel": "system_status"})),
        ),
        (
            ParadexWssMessage::funding_rate_comparison(ParadexFundingComparisonRefresh::Ms500),
            req(
                "subscribe",
                json!({"channel": "funding_rate_comparison.ALL@500ms"}),
            ),
        ),
        (
            ParadexWssMessage::funding_rate_comparison_unsub(
                ParadexFundingComparisonRefresh::Ms1000,
            ),
            req(
                "unsubscribe",
                json!({"channel": "funding_rate_comparison.ALL@1000ms"}),
            ),
        ),
    ];
    for (msg, want) in cases {
        assert_eq!(request(&msg), want);
    }

    let feed = ParadexBookFeed::Interactive {
        refresh: ParadexBookRefresh::Ms200,
        price_tick: Some("10"),
    };
    assert_eq!(
        feed.channel("BTC-USD-PERP"),
        "order_book.BTC-USD-PERP.interactive@15@200ms@10"
    );
    let feed = ParadexBookFeed::Interactive {
        refresh: ParadexBookRefresh::Ms400,
        price_tick: None,
    };
    assert_eq!(
        feed.channel("SOL-USD-PERP"),
        "order_book.SOL-USD-PERP.interactive@15@400ms"
    );
}

#[test]
fn captured_replies() {
    let parsed: Vec<ParadexWsControl> = include_str!("fixtures/control_market.jsonl")
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| parse(l).unwrap_or_else(|e| panic!("{l}: {e:#}")))
        .collect();
    assert_eq!(parsed.len(), 11);

    let ack = |i: usize| match &parsed[i] {
        ParadexWsControl::ChannelAck(a) => a.clone(),
        other => panic!("{other:?}"),
    };
    let err = |i: usize| match &parsed[i] {
        ParadexWsControl::Error(e) => e.clone(),
        other => panic!("{other:?}"),
    };

    // Scoped markets_summary subscribe.
    let a = ack(0);
    assert_eq!((a.id, a.channel.as_str()), (2, "markets_summary"));
    assert_eq!(
        a.markets,
        Some(vec!["ETH-USD-PERP".to_string(), "SOL-USD-PERP".to_string()])
    );
    assert_eq!((a.add.clone(), a.remove.clone()), (None, None));

    let a = ack(1);
    assert_eq!(a.channel, "order_book.ETH-USD-PERP.interactive@15@200ms");
    assert_eq!(a.markets, None);

    // modify_subscription with add and remove.
    let a = ack(2);
    assert_eq!(a.id, 11);
    assert_eq!(a.add, Some(vec!["BTC-USD-PERP".to_string()]));
    assert_eq!(a.remove, Some(vec!["SOL-USD-PERP".to_string()]));

    // Unsubscribe acks: bbo, funding_data, scoped markets_summary.
    assert_eq!(ack(3).channel, "bbo.BTC-USD-PERP");
    assert_eq!(ack(4).channel, "funding_data.BTC-USD-PERP");
    assert_eq!(
        (ack(5).id, ack(5).channel),
        (15, "markets_summary".to_string())
    );

    // modify_subscription without a scoped subscription.
    let e = err(6);
    assert_eq!((e.id, e.code), (16, -32600));
    assert_eq!(e.message, "invalid modify_subscription request");
    assert_eq!(e.data.as_deref(), Some("modify failed"));

    // Refresh rates are validated per feed.
    assert_eq!(
        err(7).data.as_deref(),
        Some("invalid frequency '200ms'. Expecting one of: [50ms 100ms]")
    );
    assert_eq!(
        err(8).data.as_deref(),
        Some("invalid frequency '50ms'. Expecting one of: [100ms 200ms 400ms]")
    );

    // One-sided modify acks omit the other list.
    let a = ack(9);
    assert_eq!(
        (a.add, a.remove),
        (Some(vec!["SOL-USD-PERP".to_string()]), None)
    );
    let a = ack(10);
    assert_eq!(
        (a.add, a.remove),
        (None, Some(vec!["ETH-USD-PERP".to_string()]))
    );

    // Non-array filter lists are rejected.
    assert!(parse(
        r#"{"jsonrpc":"2.0","id":1,"result":{"channel":"markets_summary","add":"x"},"usIn":1,"usDiff":1}"#
    )
    .is_err());
}

fn notification(text: &str) -> ParadexWsNotification {
    match parse(text).unwrap() {
        ParadexWsControl::Notification(n) => n,
        other => panic!("{other:?}"),
    }
}

#[test]
fn system_status_notification() {
    // Captured live (also in control.jsonl).
    let n = notification(
        r#"{"jsonrpc":"2.0","method":"subscription","params":{"channel":"system_status","data":{"status":"ok"}}}"#,
    );
    assert_eq!(n.system_status().unwrap().status, ParadexSystemStatus::Ok);
    assert!(n.market_event().is_err());

    let bad = notification(
        r#"{"jsonrpc":"2.0","method":"subscription","params":{"channel":"system_status","data":{}}}"#,
    );
    assert!(bad.system_status().is_err());
}

#[test]
fn markets_notification() {
    // No `markets` event was published during a 58-minute live capture (the REST history had
    // none in that window either). The docs give the payload the same schema as
    // `GET /v1/markets/history` results, so a captured REST event is wrapped here.
    let rest: Value =
        serde_json::from_str(include_str!("fixtures/rest_markets_history.json")).unwrap();
    let frame = |channel: &str, data: &Value| {
        json!({"jsonrpc": "2.0", "method": "subscription", "params": {"channel": channel, "data": data}})
            .to_string()
    };
    let settled = &rest["results"][1];
    let n = notification(&frame("markets.SNDK-USD-PERP", settled));
    let e = n.market_event().unwrap();
    assert_eq!(
        (e.symbol.as_str(), e.event_type.as_str()),
        ("SNDK-USD-PERP", "SETTLED")
    );
    assert!(n.system_status().is_err());

    let created = notification(&frame("markets", &rest["results"][0]));
    assert_eq!(
        created
            .market_event()
            .unwrap()
            .changed_fields
            .option_type
            .as_deref(),
        Some("PUT")
    );

    let mut missing = settled.clone();
    missing.as_object_mut().unwrap().remove("updated_at");
    assert!(notification(&frame("markets", &missing))
        .market_event()
        .is_err());
}
