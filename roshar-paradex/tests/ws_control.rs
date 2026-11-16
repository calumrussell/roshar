//! JSON control frames captured from prod
//! (`wss://ws.api.prod.paradex.trade/v1?sbeSchemaId=1&sbeSchemaVersion=3`).

use roshar_paradex::ws_control::{parse, ParadexWsControl, ParadexWsResult, ParadexWsTiming};
use serde_json::json;

fn fixture_lines() -> Vec<&'static str> {
    include_str!("fixtures/control.jsonl")
        .lines()
        .filter(|l| !l.trim().is_empty())
        .collect()
}

#[test]
fn captured_frames() {
    let lines = fixture_lines();
    assert_eq!(lines.len(), 8);
    let parsed: Vec<ParadexWsControl> = lines
        .iter()
        .map(|l| parse(l).unwrap_or_else(|e| panic!("{l}: {e:#}")))
        .collect();

    match &parsed[0] {
        ParadexWsControl::ChannelAck(a) => {
            assert_eq!(a.id, 1);
            assert_eq!(a.channel, "trades.BTC-USD-PERP");
            assert_eq!(
                a.timing,
                ParadexWsTiming {
                    us_in: 1794780123680554,
                    us_diff: 132,
                    us_out: Some(1794780123680686),
                }
            );
        }
        other => panic!("{other:?}"),
    }
    match &parsed[1] {
        ParadexWsControl::ChannelAck(a) => {
            assert_eq!(a.id, 6);
            assert_eq!(a.channel, "order_book.SOL-USD-PERP.snapshot@15@100ms@0_01");
        }
        other => panic!("{other:?}"),
    }
    // Unsubscribe ack: same shape as subscribe ack.
    match &parsed[2] {
        ParadexWsControl::ChannelAck(a) => {
            assert_eq!(a.id, 2);
            assert_eq!(a.channel, "trades.BTC-USD-16NOV26-54000-P");
        }
        other => panic!("{other:?}"),
    }
    match &parsed[3] {
        ParadexWsControl::Error(e) => {
            assert_eq!(e.id, 11);
            assert_eq!(e.code, -32600);
            assert_eq!(e.message, "invalid subscribe request");
            assert_eq!(e.data.as_deref(), Some("subscription failed"));
        }
        other => panic!("{other:?}"),
    }
    match &parsed[4] {
        ParadexWsControl::Error(e) => {
            assert_eq!((e.id, e.code), (98, -32600));
            assert_eq!(e.data.as_deref(), Some("invalid channel"));
        }
        other => panic!("{other:?}"),
    }
    match &parsed[5] {
        ParadexWsControl::Error(e) => {
            assert_eq!((e.id, e.code), (99, -32601));
            assert_eq!(e.message, "method does not exist");
            assert_eq!(e.data, None);
            assert_eq!(e.timing.us_out, Some(1794780123681296));
        }
        other => panic!("{other:?}"),
    }
    match &parsed[6] {
        ParadexWsControl::Error(e) => {
            assert_eq!((e.id, e.code), (0, -32700));
            assert_eq!(e.message, "unable to parse request");
            assert_eq!(e.timing.us_in, 0);
        }
        other => panic!("{other:?}"),
    }
    match &parsed[7] {
        ParadexWsControl::Notification(n) => {
            assert_eq!(n.channel, "system_status");
            assert_eq!(n.data, json!({"status": "ok"}));
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn docs_examples() {
    // https://docs.paradex.trade/ws/general-information/subscription-channels
    // (no usOut in the docs example).
    let ack = parse(
        r#"{"jsonrpc":"2.0","result":{"channel":"CHANNEL_NAME"},"usIn":1682556415569005368,"usDiff":1291796,"id":1}"#,
    )
    .unwrap();
    match ack {
        ParadexWsControl::ChannelAck(a) => {
            assert_eq!(a.channel, "CHANNEL_NAME");
            assert_eq!(a.timing.us_out, None);
        }
        other => panic!("{other:?}"),
    }
    // https://docs.paradex.trade/ws/general-information/error-handling
    let err = parse(
        r#"{"jsonrpc":"2.0","error":{"code":40111,"message":"invalid bearer token"},"usIn":1710522972729581,"usOut":1710522972729618,"usDiff":37,"id":0}"#,
    )
    .unwrap();
    match err {
        ParadexWsControl::Error(e) => {
            assert_eq!(e.code, 40111);
            assert_eq!(e.message, "invalid bearer token");
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn auth_result() {
    // Shape per https://docs.paradex.trade/ws/web-socket-channels/auth/auth
    // (result object; node_id documented, not required). Not captured live:
    // no authenticated requests are sent from tests.
    let with_node = parse(
        r#"{"jsonrpc":"2.0","id":7,"result":{"node_id":"abc"},"usIn":1,"usDiff":2,"usOut":3}"#,
    )
    .unwrap();
    let r: &ParadexWsResult = match &with_node {
        ParadexWsControl::Result(r) => r,
        other => panic!("{other:?}"),
    };
    assert_eq!(r.id, 7);
    assert_eq!(r.auth().unwrap().node_id.as_deref(), Some("abc"));

    let empty =
        parse(r#"{"jsonrpc":"2.0","id":8,"result":{},"usIn":1,"usDiff":2,"usOut":3}"#).unwrap();
    match &empty {
        ParadexWsControl::Result(r) => assert_eq!(r.auth().unwrap().node_id, None),
        other => panic!("{other:?}"),
    }
}

#[test]
fn strict_errors() {
    // Missing jsonrpc.
    assert!(parse(r#"{"id":1,"result":{"channel":"x"},"usIn":1,"usDiff":1}"#).is_err());
    // Wrong jsonrpc version.
    assert!(
        parse(r#"{"jsonrpc":"1.0","id":1,"result":{"channel":"x"},"usIn":1,"usDiff":1}"#).is_err()
    );
    // Response without id.
    assert!(parse(r#"{"jsonrpc":"2.0","result":{"channel":"x"},"usIn":1,"usDiff":1}"#).is_err());
    // Response without timing.
    assert!(parse(r#"{"jsonrpc":"2.0","id":1,"result":{"channel":"x"}}"#).is_err());
    // Neither result nor error.
    assert!(parse(r#"{"jsonrpc":"2.0","id":1,"usIn":1,"usDiff":1}"#).is_err());
    // Error missing message.
    assert!(
        parse(r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32600},"usIn":1,"usDiff":1}"#).is_err()
    );
    // Ack with non-string channel.
    assert!(
        parse(r#"{"jsonrpc":"2.0","id":1,"result":{"channel":5},"usIn":1,"usDiff":1}"#).is_err()
    );
    // Notification missing data / unknown method.
    assert!(
        parse(r#"{"jsonrpc":"2.0","method":"subscription","params":{"channel":"x"}}"#).is_err()
    );
    assert!(
        parse(r#"{"jsonrpc":"2.0","method":"other","params":{"channel":"x","data":{}}}"#).is_err()
    );
    // Not JSON.
    assert!(parse("not json").is_err());
}
