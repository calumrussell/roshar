//! Hyperliquid websocket subscriptions for one user: `orderUpdates` and `userFills`.
//!
//! One task owns the connection: it subscribes to both channels, sends an application ping every 30 s, treats
//! 70 s without any frame as a dead socket, and reconnects with backoff (250 ms doubling to 5 s, reset after a
//! successful subscribe), resubscribing each time. Every reconnect is announced with `UserEvent::Reconnected`
//! (after the new subscriptions are written) so the consumer can reconcile what it may have missed; the first
//! `userFills` message of each connection is a snapshot (`snapshot: true`) that may repeat earlier fills, so
//! consumers should de-duplicate by `tid`. The user address is never logged.

use std::time::Duration;

use crate::poster::now_ns;
use anyhow::{anyhow, bail, Context};
use futures_util::{SinkExt, StreamExt};
use roshar_types::hyperliquid::exchange::Cloid;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::sync::mpsc::UnboundedSender;
use tokio::task::JoinHandle;
use tokio_tungstenite::tungstenite::Message;

use crate::poster::open;

const PING_EVERY: Duration = Duration::from_secs(30);
const IDLE_TIMEOUT: Duration = Duration::from_secs(70);
const BACKOFF_MIN: Duration = Duration::from_millis(250);
const BACKOFF_MAX: Duration = Duration::from_secs(5);

/// Order status from `orderUpdates`.
#[derive(Clone, Debug, PartialEq)]
pub enum OrderState {
    Open,
    Filled,
    /// "canceled" or any "...Canceled" / "scheduledCancel" status (text kept).
    Canceled(String),
    /// "rejected" or any "...Rejected" status, e.g. "badAloPxRejected".
    Rejected(String),
    /// "triggered" and anything unknown.
    Other(String),
}

impl OrderState {
    fn from_status(s: &str) -> OrderState {
        match s {
            "open" => OrderState::Open,
            "filled" => OrderState::Filled,
            "canceled" | "scheduledCancel" => OrderState::Canceled(s.to_string()),
            "rejected" => OrderState::Rejected(s.to_string()),
            _ if s.ends_with("Canceled") => OrderState::Canceled(s.to_string()),
            _ if s.ends_with("Rejected") => OrderState::Rejected(s.to_string()),
            _ => OrderState::Other(s.to_string()),
        }
    }

    /// The order no longer rests (filled, canceled or rejected).
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            OrderState::Filled | OrderState::Canceled(_) | OrderState::Rejected(_)
        )
    }
}

/// One `orderUpdates` entry.
#[derive(Clone, Debug, PartialEq)]
pub struct OrderUpdate {
    pub coin: String,
    pub oid: u64,
    pub cloid: Option<Cloid>,
    pub is_buy: bool,
    pub limit_px: f64,
    /// Remaining (unfilled) size at this status.
    pub sz: f64,
    pub orig_sz: f64,
    pub status: OrderState,
    pub status_ms: u64,
}

/// One `userFills` fill.
#[derive(Clone, Debug, PartialEq)]
pub struct UserFill {
    pub coin: String,
    pub px: f64,
    pub sz: f64,
    pub is_buy: bool,
    pub time_ms: u64,
    /// Signed position before this fill.
    pub start_position: Option<f64>,
    /// e.g. "Open Long", "Close Short".
    pub dir: String,
    pub closed_pnl: f64,
    pub oid: u64,
    pub tid: u64,
    /// true = we took liquidity.
    pub crossed: bool,
    /// Fee paid (negative = rebate), in `fee_token`.
    pub fee: f64,
    pub fee_token: Option<String>,
    pub cloid: Option<Cloid>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum UserEvent {
    /// First connection subscribed.
    Connected,
    /// A later connection subscribed: updates may have been missed while disconnected.
    Reconnected,
    Disconnected(String),
    Orders {
        updates: Vec<OrderUpdate>,
        recv_ns: u64,
    },
    /// `snapshot`: the per-connection initial batch (may repeat fills already delivered).
    Fills {
        snapshot: bool,
        fills: Vec<UserFill>,
        recv_ns: u64,
    },
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawOrderUpdate {
    order: RawBasicOrder,
    status: String,
    #[serde(default)]
    status_timestamp: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawBasicOrder {
    coin: String,
    side: String,
    limit_px: String,
    sz: String,
    oid: u64,
    orig_sz: String,
    #[serde(default)]
    cloid: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawUserFills {
    #[serde(default)]
    is_snapshot: bool,
    fills: Vec<RawFill>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawFill {
    coin: String,
    px: String,
    sz: String,
    side: String,
    time: u64,
    #[serde(default)]
    start_position: Option<String>,
    dir: String,
    closed_pnl: String,
    oid: u64,
    tid: u64,
    crossed: bool,
    fee: String,
    #[serde(default)]
    fee_token: Option<String>,
    #[serde(default)]
    cloid: Option<String>,
}

fn num(s: &str) -> anyhow::Result<f64> {
    s.parse::<f64>()
        .with_context(|| format!("bad number {s:?}"))
}

fn side_is_buy(s: &str) -> anyhow::Result<bool> {
    match s {
        "B" => Ok(true),
        "A" => Ok(false),
        _ => bail!("bad side {s:?}"),
    }
}

/// `0x` + 32 hex digits -> cloid bytes.
pub fn parse_cloid(s: &str) -> Option<Cloid> {
    let h = s.strip_prefix("0x").unwrap_or(s);
    let v = hex::decode(h).ok()?;
    v.try_into().ok()
}

/// Parse one websocket frame. Ok(None) for frames that carry no user event (pong, subscriptionResponse, other
/// channels); Err for unparseable frames and the server's `error` channel.
pub fn parse_user_frame(text: &str, recv_ns: u64) -> anyhow::Result<Option<UserEvent>> {
    let mut v: Value = serde_json::from_str(text)?;
    let channel = v
        .get("channel")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let data = v.get_mut("data").map(Value::take).unwrap_or(Value::Null);
    match channel.as_str() {
        "orderUpdates" => {
            let raw: Vec<RawOrderUpdate> = serde_json::from_value(data).context("orderUpdates")?;
            let updates = raw
                .into_iter()
                .map(|u| {
                    let o = u.order;
                    Ok(OrderUpdate {
                        is_buy: side_is_buy(&o.side)?,
                        limit_px: num(&o.limit_px)?,
                        sz: num(&o.sz)?,
                        orig_sz: num(&o.orig_sz)?,
                        cloid: o.cloid.as_deref().and_then(parse_cloid),
                        coin: o.coin,
                        oid: o.oid,
                        status: OrderState::from_status(&u.status),
                        status_ms: u.status_timestamp,
                    })
                })
                .collect::<anyhow::Result<Vec<_>>>()?;
            Ok(Some(UserEvent::Orders { updates, recv_ns }))
        }
        "userFills" => {
            let raw: RawUserFills = serde_json::from_value(data).context("userFills")?;
            let fills = raw
                .fills
                .into_iter()
                .map(|f| {
                    Ok(UserFill {
                        px: num(&f.px)?,
                        sz: num(&f.sz)?,
                        is_buy: side_is_buy(&f.side)?,
                        time_ms: f.time,
                        start_position: f.start_position.as_deref().map(num).transpose()?,
                        closed_pnl: num(&f.closed_pnl)?,
                        fee: num(&f.fee)?,
                        cloid: f.cloid.as_deref().and_then(parse_cloid),
                        coin: f.coin,
                        dir: f.dir,
                        oid: f.oid,
                        tid: f.tid,
                        crossed: f.crossed,
                        fee_token: f.fee_token,
                    })
                })
                .collect::<anyhow::Result<Vec<_>>>()?;
            Ok(Some(UserEvent::Fills {
                snapshot: raw.is_snapshot,
                fills,
                recv_ns,
            }))
        }
        "error" => bail!("server error: {data}"),
        _ => Ok(None),
    }
}

/// Handle to the subscription task; dropping it stops the task.
pub struct UserStream {
    task: JoinHandle<()>,
}

impl Drop for UserStream {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl UserStream {
    /// Connect to `url` (e.g. wss://api.hyperliquid.xyz/ws) and stream `user`'s order updates and fills into `tx`
    /// until the receiver or this handle is dropped. Returns immediately; the first connection is announced
    /// with `UserEvent::Connected`.
    pub fn spawn(url: &str, user: &str, tx: UnboundedSender<UserEvent>) -> UserStream {
        let (url, user) = (url.to_string(), user.to_lowercase());
        let task = tokio::spawn(async move {
            let mut backoff = BACKOFF_MIN;
            let mut subscribed_before = false;
            loop {
                let (subscribed, err) = match session(&url, &user, &tx, subscribed_before).await {
                    End::ReceiverGone => return,
                    End::Dropped { subscribed, err } => (subscribed, err),
                };
                if subscribed {
                    subscribed_before = true;
                    backoff = BACKOFF_MIN;
                }
                let err = redact(&format!("{err:#}"), &user);
                log::warn!("user stream: {err}; reconnecting in {backoff:?}");
                if tx.send(UserEvent::Disconnected(err)).is_err() {
                    return;
                }
                tokio::time::sleep(backoff).await;
                backoff = (backoff * 2).min(BACKOFF_MAX);
            }
        });
        UserStream { task }
    }
}

fn redact(s: &str, user: &str) -> String {
    if user.is_empty() {
        return s.to_string();
    }
    s.to_lowercase().replace(user, "<user>")
}

enum End {
    ReceiverGone,
    Dropped {
        subscribed: bool,
        err: anyhow::Error,
    },
}

async fn session(url: &str, user: &str, tx: &UnboundedSender<UserEvent>, reconnect: bool) -> End {
    let dropped = |subscribed, err| End::Dropped { subscribed, err };
    let ws = match open(url).await {
        Ok(ws) => ws,
        Err(e) => return dropped(false, e.context("connect")),
    };
    let (mut sink, mut stream) = ws.split();
    for kind in ["orderUpdates", "userFills"] {
        let sub = json!({"method": "subscribe", "subscription": {"type": kind, "user": user}})
            .to_string();
        if let Err(e) = sink.send(Message::Text(sub)).await {
            return dropped(false, anyhow!(e).context("subscribe"));
        }
    }
    log::info!(
        "user stream: subscribed to orderUpdates and userFills{}",
        if reconnect { " (reconnected)" } else { "" }
    );
    if tx
        .send(if reconnect {
            UserEvent::Reconnected
        } else {
            UserEvent::Connected
        })
        .is_err()
    {
        return End::ReceiverGone;
    }
    let mut ping = tokio::time::interval_at(tokio::time::Instant::now() + PING_EVERY, PING_EVERY);
    ping.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut last_frame = tokio::time::Instant::now();
    loop {
        let idle = tokio::time::sleep_until(last_frame + IDLE_TIMEOUT);
        tokio::select! {
            m = stream.next() => {
                last_frame = tokio::time::Instant::now();
                let msg = match m {
                    Some(Ok(m)) => m,
                    Some(Err(e)) => return dropped(true, anyhow!(e)),
                    None => return dropped(true, anyhow!("stream ended")),
                };
                let recv_ns = now_ns();
                match msg {
                    Message::Text(text) => match parse_user_frame(&text, recv_ns) {
                        Ok(Some(ev)) => {
                            if tx.send(ev).is_err() {
                                return End::ReceiverGone;
                            }
                        }
                        Ok(None) => {}
                        // frames can echo the subscription (and so the address): redact before logging
                        Err(e) => log::warn!("user stream: {}", redact(&format!("{e:#}"), user)),
                    },
                    Message::Ping(_) => {
                        if let Err(e) = sink.flush().await {
                            return dropped(true, anyhow!(e).context("pong"));
                        }
                    }
                    Message::Close(f) => return dropped(true, anyhow!("closed by server: {f:?}")),
                    _ => {}
                }
            }
            _ = ping.tick() => {
                if let Err(e) = sink.send(Message::Text(r#"{"method":"ping"}"#.into())).await {
                    return dropped(true, anyhow!(e).context("ping"));
                }
            }
            _ = idle => return dropped(true, anyhow!("no frame for {IDLE_TIMEOUT:?}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ORDER_UPDATES: &str = r#"{"channel":"orderUpdates","data":[
        {"order":{"coin":"PUMP","side":"B","limitPx":"0.004301","sz":"58140.0","oid":215876543210,"timestamp":1793400000000,
                  "origSz":"58140.0","cloid":"0x0123456789abcdef0123456789abcdef"},"status":"open","statusTimestamp":1793400000012},
        {"order":{"coin":"PUMP","side":"A","limitPx":"0.004305","sz":"0.0","oid":215876543211,"timestamp":1793400000000,
                  "origSz":"58072.0"},"status":"filled","statusTimestamp":1793400001234},
        {"order":{"coin":"PUMP","side":"A","limitPx":"0.004306","sz":"20000.0","oid":215876543212,"timestamp":1793400000000,
                  "origSz":"58072.0","cloid":"0x0123456789abcdef0123456789abcdee"},"status":"canceled","statusTimestamp":1793400001300},
        {"order":{"coin":"PUMP","side":"B","limitPx":"0.0043","sz":"58140.0","oid":215876543213,"timestamp":1793400000000,
                  "origSz":"58140.0"},"status":"badAloPxRejected","statusTimestamp":1793400001400},
        {"order":{"coin":"PUMP","side":"B","limitPx":"0.0043","sz":"58140.0","oid":215876543214,"timestamp":1793400000000,
                  "origSz":"58140.0"},"status":"reduceOnlyCanceled","statusTimestamp":1793400001500}]}"#;

    const USER_FILLS: &str = r#"{"channel":"userFills","data":{"isSnapshot":false,"user":"0x00000000000000000000000000000000000000aa",
        "fills":[{"coin":"PUMP","px":"0.004301","sz":"30000.0","side":"B","time":1793400000500,"startPosition":"0.0",
                  "dir":"Open Long","closedPnl":"0.0","hash":"0xa1b2c3","oid":215876543210,"crossed":false,"fee":"-0.00258",
                  "tid":987654321012345,"feeToken":"USDC","builderFee":"0.0","cloid":"0x0123456789abcdef0123456789abcdef"},
                 {"coin":"PUMP","px":"0.004298","sz":"30000.0","side":"A","time":1793400002000,"startPosition":"30000.0",
                  "dir":"Close Long","closedPnl":"-0.09","hash":"0xa1b2c4","oid":215876543299,"crossed":true,"fee":"0.058",
                  "tid":987654321012346,"feeToken":"USDC"}]}}"#;

    #[test]
    fn parses_order_updates() {
        let Some(UserEvent::Orders { updates, recv_ns }) =
            parse_user_frame(ORDER_UPDATES, 9).unwrap()
        else {
            panic!()
        };
        assert_eq!(recv_ns, 9);
        assert_eq!(updates.len(), 5);
        let u = &updates[0];
        assert_eq!(
            (
                u.coin.as_str(),
                u.oid,
                u.is_buy,
                u.limit_px,
                u.sz,
                u.orig_sz
            ),
            ("PUMP", 215876543210, true, 0.004301, 58140.0, 58140.0)
        );
        assert_eq!(u.cloid, parse_cloid("0x0123456789abcdef0123456789abcdef"));
        assert_eq!(u.cloid.unwrap()[0], 0x01);
        assert_eq!(
            (u.status.clone(), u.status_ms),
            (OrderState::Open, 1793400000012)
        );
        assert!(!u.status.is_terminal());
        let u = &updates[1];
        assert_eq!(
            (u.is_buy, u.sz, u.cloid, u.status.clone()),
            (false, 0.0, None, OrderState::Filled)
        );
        assert!(u.status.is_terminal());
        assert_eq!(updates[2].status, OrderState::Canceled("canceled".into()));
        assert_eq!(updates[2].sz, 20000.0, "remaining size at cancel");
        assert_eq!(
            updates[3].status,
            OrderState::Rejected("badAloPxRejected".into())
        );
        assert_eq!(
            updates[4].status,
            OrderState::Canceled("reduceOnlyCanceled".into())
        );
        assert!(updates.iter().skip(1).all(|u| u.status.is_terminal()));
    }

    #[test]
    fn parses_user_fills() {
        let Some(UserEvent::Fills {
            snapshot,
            fills,
            recv_ns,
        }) = parse_user_frame(USER_FILLS, 5).unwrap()
        else {
            panic!()
        };
        assert!(!snapshot);
        assert_eq!((fills.len(), recv_ns), (2, 5));
        let f = &fills[0];
        assert_eq!(
            (f.coin.as_str(), f.px, f.sz, f.is_buy, f.time_ms),
            ("PUMP", 0.004301, 30000.0, true, 1793400000500)
        );
        assert_eq!(
            (f.oid, f.tid, f.crossed, f.fee, f.start_position),
            (215876543210, 987654321012345, false, -0.00258, Some(0.0))
        );
        assert_eq!(
            (f.dir.as_str(), f.fee_token.as_deref()),
            ("Open Long", Some("USDC"))
        );
        assert!(f.cloid.is_some());
        let f = &fills[1];
        assert_eq!(
            (f.is_buy, f.crossed, f.closed_pnl, f.cloid),
            (false, true, -0.09, None)
        );
        let snap = USER_FILLS.replace(r#""isSnapshot":false"#, r#""isSnapshot":true"#);
        assert!(matches!(
            parse_user_frame(&snap, 0).unwrap(),
            Some(UserEvent::Fills { snapshot: true, .. })
        ));
    }

    #[test]
    fn ignores_control_frames_and_reports_errors() {
        assert_eq!(parse_user_frame(r#"{"channel":"pong"}"#, 0).unwrap(), None);
        let sub = r#"{"channel":"subscriptionResponse","data":{"method":"subscribe","subscription":{"type":"userFills","user":"0xab"}}}"#;
        assert_eq!(parse_user_frame(sub, 0).unwrap(), None);
        assert!(
            parse_user_frame(r#"{"channel":"error","data":"Invalid subscription"}"#, 0).is_err()
        );
        assert!(
            parse_user_frame(r#"{"channel":"orderUpdates","data":[{"order":{}}]}"#, 0).is_err()
        );
        assert!(parse_user_frame("not json", 0).is_err());
    }

    #[test]
    fn cloid_and_redaction() {
        assert_eq!(
            parse_cloid("0x000000000000000000000000000000ab").unwrap()[15],
            0xab
        );
        assert_eq!(parse_cloid("0x00ab"), None);
        assert_eq!(parse_cloid("zz"), None);
        assert_eq!(redact("bad sub for 0xAbC1", "0xabc1"), "bad sub for <user>");
    }
}
