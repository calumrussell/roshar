//! WebSocket `post` client for the Hyperliquid exchange endpoint.
//!
//! `post` writes the frame from the caller's task (one async mutex, held only for the write) and returns
//! immediately with a oneshot for the response. A single reader task matches responses by id; a
//! housekeeping task sends pings and expires requests. On disconnect every pending request fails with
//! "disconnected" and the connection is re-established with backoff; posts made meanwhile fail fast.

use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Weak};
use std::time::Duration;

use crate::types::exchange::OrderStatus;
use anyhow::{anyhow, bail};
use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::net::TcpStream;
use tokio::sync::{oneshot, Notify};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

pub(crate) type Ws = WebSocketStream<MaybeTlsStream<TcpStream>>;
type Sink = SplitSink<Ws, Message>;
type Stream = SplitStream<Ws>;

const REQUEST_TIMEOUT_NS: u64 = 10_000_000_000;
const PING_EVERY: Duration = Duration::from_secs(20);
const SWEEP_EVERY: Duration = Duration::from_millis(250);
/// Upper bound on waiting for the write lock plus the write itself.
const WRITE_TIMEOUT: Duration = Duration::from_secs(2);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const BACKOFF_MIN: Duration = Duration::from_millis(250);
const BACKOFF_MAX: Duration = Duration::from_secs(5);

/// Timestamps: `*_ns` are wall-clock nanoseconds since the Unix epoch (`now_ns()`), so they compare
/// directly with exchange millisecond times after `/ 1_000_000`.
pub fn now_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

/// Outcome of one posted action.
#[derive(Clone, Debug)]
pub struct PostResult {
    pub id: u64,
    /// Just before the frame was written to the socket.
    pub sent_ns: u64,
    /// When the matching response frame arrived.
    pub recv_ns: u64,
    /// Err = transport problem (disconnect, timeout, unparseable response).
    pub status: Result<OrderStatus, String>,
}

/// A posted request whose response has not necessarily arrived yet.
#[derive(Debug)]
pub struct PendingPost {
    pub id: u64,
    /// Taken immediately before the frame was written.
    pub sent_ns: u64,
    /// Always resolves: response, "timeout" (10 s) or "disconnected".
    pub rx: oneshot::Receiver<PostResult>,
}

struct Waiter {
    sent_ns: u64,
    tx: oneshot::Sender<PostResult>,
}

struct Conn {
    sink: Sink,
    /// Wakes this connection's reader to tear the connection down (write failure / shutdown).
    kill: Arc<Notify>,
}

struct Inner {
    url: String,
    next_id: AtomicU64,
    connected: AtomicBool,
    conn: tokio::sync::Mutex<Option<Conn>>,
    pending: Mutex<HashMap<u64, Waiter>>,
    /// Remote address of the current connection.
    peer: Mutex<Option<std::net::SocketAddr>>,
}

impl Drop for Inner {
    fn drop(&mut self) {
        if let Some(c) = self.conn.get_mut() {
            c.kill.notify_one();
        }
    }
}

impl Inner {
    fn fail_all(&self, why: &str) {
        let drained: Vec<(u64, Waiter)> = self.pending.lock().drain().collect();
        let now = now_ns();
        for (id, w) in drained {
            let _ = w.tx.send(PostResult {
                id,
                sent_ns: w.sent_ns,
                recv_ns: now,
                status: Err(why.to_string()),
            });
        }
    }

    fn expire(&self, now: u64) {
        let expired: Vec<(u64, Waiter)> = {
            let mut p = self.pending.lock();
            let ids: Vec<u64> = p
                .iter()
                .filter(|(_, w)| now.saturating_sub(w.sent_ns) > REQUEST_TIMEOUT_NS)
                .map(|(id, _)| *id)
                .collect();
            ids.into_iter()
                .filter_map(|id| p.remove(&id).map(|w| (id, w)))
                .collect()
        };
        for (id, w) in expired {
            let _ = w.tx.send(PostResult {
                id,
                sent_ns: w.sent_ns,
                recv_ns: now,
                status: Err("timeout".into()),
            });
        }
    }

    fn on_text(&self, text: &str, recv_ns: u64) {
        let v: Value = match serde_json::from_str(text) {
            Ok(v) => v,
            Err(e) => {
                log::warn!("ws post: unparseable frame: {e}");
                return;
            }
        };
        if v.get("channel").and_then(Value::as_str) != Some("post") {
            return; // pong, subscriptionResponse, ...
        }
        let data = &v["data"];
        let Some(id) = data.get("id").and_then(Value::as_u64) else {
            log::warn!("ws post: response without id");
            return;
        };
        let Some(w) = self.pending.lock().remove(&id) else {
            return; // already timed out
        };
        let resp = &data["response"];
        let payload = &resp["payload"];
        let status = match resp.get("type").and_then(Value::as_str) {
            Some("action") => decode_status(payload),
            Some("info") => Ok(OrderStatus::Success),
            Some("error") => Err(match payload.as_str() {
                Some(s) => s.to_string(),
                None => payload.to_string(),
            }),
            _ => Err(resp.to_string()),
        };
        let _ = w.tx.send(PostResult {
            id,
            sent_ns: w.sent_ns,
            recv_ns,
            status,
        });
    }
}

/// First status of an exchange action response payload.
pub fn decode_status(payload: &Value) -> Result<OrderStatus, String> {
    let raw = || Err(payload.to_string());
    match payload.get("status").and_then(Value::as_str) {
        Some("ok") => {}
        Some("err") => {
            return match payload.get("response") {
                Some(Value::String(s)) => Ok(OrderStatus::Rejected(s.clone())),
                _ => raw(),
            }
        }
        _ => return raw(),
    }
    let resp = &payload["response"];
    if resp.get("type").and_then(Value::as_str) == Some("default") {
        return Ok(OrderStatus::Success);
    }
    let Some(s0) = resp
        .get("data")
        .and_then(|d| d.get("statuses"))
        .and_then(|s| s.get(0))
    else {
        return raw();
    };
    if s0.as_str() == Some("success") {
        return Ok(OrderStatus::Success);
    }
    if let Some(r) = s0.get("resting") {
        return match r.get("oid").and_then(Value::as_u64) {
            Some(oid) => Ok(OrderStatus::Resting { oid }),
            None => raw(),
        };
    }
    if let Some(f) = s0.get("filled") {
        let num = |k: &str| {
            f.get(k)
                .and_then(Value::as_str)
                .and_then(|s| s.parse::<f64>().ok())
        };
        return match (
            num("totalSz"),
            num("avgPx"),
            f.get("oid").and_then(Value::as_u64),
        ) {
            (Some(total_sz), Some(avg_px), Some(oid)) => Ok(OrderStatus::Filled {
                total_sz,
                avg_px,
                oid,
            }),
            _ => raw(),
        };
    }
    if let Some(Value::String(e)) = s0.get("error") {
        return Ok(OrderStatus::Rejected(e.clone()));
    }
    raw()
}

/// Remote address of a websocket's TCP connection.
fn peer_of(ws: &Ws) -> Option<std::net::SocketAddr> {
    match ws.get_ref() {
        MaybeTlsStream::Plain(s) => s.peer_addr().ok(),
        MaybeTlsStream::Rustls(s) => s.get_ref().0.peer_addr().ok(),
        _ => None,
    }
}

/// The `post` frame for request `id` with an already serialized payload, embedded byte for byte.
pub fn post_frame(id: u64, kind: &str, payload_json: &str) -> String {
    format!(
        r#"{{"method":"post","id":{id},"request":{{"type":"{kind}","payload":{payload_json}}}}}"#
    )
}

pub(crate) async fn open(url: &str) -> anyhow::Result<Ws> {
    let fut = tokio_tungstenite::connect_async_tls_with_config(url, None, true, None);
    let (ws, _) = tokio::time::timeout(CONNECT_TIMEOUT, fut)
        .await
        .map_err(|_| anyhow!("connect timeout"))??;
    Ok(ws)
}

#[derive(Clone)]
pub struct WsPoster {
    inner: Arc<Inner>,
}

impl WsPoster {
    /// Connects (failing if the first connection fails) and starts the reader/housekeeping tasks.
    pub async fn connect(url: &str) -> anyhow::Result<WsPoster> {
        let ws = open(url).await?;
        let peer = peer_of(&ws);
        let (sink, stream) = ws.split();
        let kill = Arc::new(Notify::new());
        let inner = Arc::new(Inner {
            url: url.to_string(),
            next_id: AtomicU64::new(1),
            connected: AtomicBool::new(true),
            conn: tokio::sync::Mutex::new(Some(Conn {
                sink,
                kill: kill.clone(),
            })),
            pending: Mutex::new(HashMap::new()),
            peer: Mutex::new(peer),
        });
        tokio::spawn(reader(Arc::downgrade(&inner), stream, kill));
        tokio::spawn(housekeeping(Arc::downgrade(&inner)));
        Ok(WsPoster { inner })
    }

    pub fn is_connected(&self) -> bool {
        self.inner.connected.load(Ordering::Acquire)
    }

    /// Remote address (ip:port) of the current connection.
    pub fn peer_addr(&self) -> Option<std::net::SocketAddr> {
        *self.inner.peer.lock()
    }

    /// Sends an already serialized signed payload (`Signer::sign` output as JSON text), embedded verbatim, so the
    /// same bytes can be written on several connections. Does not wait for the response.
    pub async fn post_raw(&self, payload_json: &str) -> anyhow::Result<PendingPost> {
        if !self.inner.connected.load(Ordering::Acquire) {
            bail!("not connected");
        }
        let id = self.inner.next_id.fetch_add(1, Ordering::Relaxed);
        self.write_text(id, post_frame(id, "action", payload_json))
            .await
    }

    /// Sends a signed exchange payload (`Signer::sign` output). Does not wait for the response.
    pub async fn post(&self, payload: Value) -> anyhow::Result<PendingPost> {
        self.send("action", payload).await
    }

    /// Sends an info request over the post channel; resolves to Ok(Success) on any non-error response.
    pub async fn post_info(&self, req: Value) -> anyhow::Result<PendingPost> {
        self.send("info", req).await
    }

    async fn send(&self, kind: &'static str, payload: Value) -> anyhow::Result<PendingPost> {
        let inner = &self.inner;
        if !inner.connected.load(Ordering::Acquire) {
            bail!("not connected");
        }
        let id = inner.next_id.fetch_add(1, Ordering::Relaxed);
        let text =
            json!({"method": "post", "id": id, "request": {"type": kind, "payload": payload}})
                .to_string();
        self.write_text(id, text).await
    }

    async fn write_text(&self, id: u64, text: String) -> anyhow::Result<PendingPost> {
        let inner = &self.inner;
        let (tx, rx) = oneshot::channel();
        let write = async {
            let mut guard = inner.conn.lock().await;
            let Some(conn) = guard.as_mut() else {
                bail!("not connected")
            };
            let sent_ns = now_ns();
            inner.pending.lock().insert(id, Waiter { sent_ns, tx });
            if let Err(e) = conn.sink.send(Message::Text(text)).await {
                conn.kill.notify_one();
                bail!("write failed: {e}");
            }
            Ok(sent_ns)
        };
        match tokio::time::timeout(WRITE_TIMEOUT, write).await {
            Ok(Ok(sent_ns)) => Ok(PendingPost { id, sent_ns, rx }),
            Ok(Err(e)) => {
                inner.pending.lock().remove(&id);
                Err(e)
            }
            Err(_) => {
                inner.pending.lock().remove(&id);
                // A write may have been cut mid-frame: tear the connection down.
                if let Ok(g) = inner.conn.try_lock() {
                    if let Some(c) = g.as_ref() {
                        c.kill.notify_one();
                    }
                }
                bail!("write timeout")
            }
        }
    }
}

/// One fan-out job: (signed payload JSON text, connection index, caller tag) writes, performed back-to-back in order.
pub type FanJob<T> = Vec<(Arc<str>, usize, T)>;

/// Writer for a pool of connections: each job's writes go out back-to-back in order (the payload text is embedded
/// verbatim, so a payload written on several connections is the same bytes on each). Every write's result
/// (response, "timeout", "disconnected" or a write error) is reported through `on_result(tag, connection, result)`.
pub fn spawn_fan_writer<T, F>(
    posters: Vec<WsPoster>,
    on_result: F,
) -> tokio::sync::mpsc::UnboundedSender<FanJob<T>>
where
    T: Send + 'static,
    F: Fn(T, usize, PostResult) + Send + Sync + 'static,
{
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<FanJob<T>>();
    let on_result = Arc::new(on_result);
    tokio::spawn(async move {
        while let Some(job) = rx.recv().await {
            for (raw, conn, tag) in job {
                let Some(poster) = posters.get(conn) else {
                    let t = now_ns();
                    on_result(
                        tag,
                        conn,
                        PostResult {
                            id: 0,
                            sent_ns: t,
                            recv_ns: t,
                            status: Err(format!("no connection {conn}")),
                        },
                    );
                    continue;
                };
                match poster.post_raw(&raw).await {
                    Ok(p) => {
                        let on_result = on_result.clone();
                        tokio::spawn(async move {
                            let (id, sent_ns) = (p.id, p.sent_ns);
                            let res = p.rx.await.unwrap_or(PostResult {
                                id,
                                sent_ns,
                                recv_ns: now_ns(),
                                status: Err("response channel dropped".into()),
                            });
                            on_result(tag, conn, res);
                        });
                    }
                    Err(e) => {
                        let t = now_ns();
                        on_result(
                            tag,
                            conn,
                            PostResult {
                                id: 0,
                                sent_ns: t,
                                recv_ns: t,
                                status: Err(format!("{e:#}")),
                            },
                        );
                    }
                }
            }
        }
    });
    tx
}

/// A fanned-out copy's status text says the exchange already processed this nonce (another copy won), e.g.
/// "Invalid nonce: duplicate nonce ...".
pub fn is_duplicate_rejection(status: &str) -> bool {
    let s = status.to_ascii_lowercase();
    s.contains("nonce") || s.contains("duplicate")
}

async fn reader(weak: Weak<Inner>, mut stream: Stream, mut kill: Arc<Notify>) {
    let mut backoff = BACKOFF_MIN;
    loop {
        let why = loop {
            tokio::select! {
                m = stream.next() => match m {
                    Some(Ok(Message::Text(t))) => {
                        let recv_ns = now_ns();
                        let Some(inner) = weak.upgrade() else { return };
                        inner.on_text(&t, recv_ns);
                    }
                    Some(Ok(Message::Close(f))) => break format!("closed by server: {f:?}"),
                    Some(Ok(_)) => {}
                    Some(Err(e)) => break format!("read error: {e}"),
                    None => break "stream ended".to_string(),
                },
                _ = kill.notified() => break "torn down".to_string(),
            }
        };
        let Some(inner) = weak.upgrade() else { return };
        log::warn!("ws post: {why}; reconnecting");
        inner.connected.store(false, Ordering::Release);
        if let Some(mut c) = inner.conn.lock().await.take() {
            let _ = tokio::time::timeout(Duration::from_millis(200), c.sink.close()).await;
        }
        inner.fail_all("disconnected");
        let url = inner.url.clone();
        drop(inner);

        loop {
            tokio::time::sleep(backoff).await;
            backoff = (backoff * 2).min(BACKOFF_MAX);
            if weak.strong_count() == 0 {
                return;
            }
            match open(&url).await {
                Ok(ws) => {
                    let Some(inner) = weak.upgrade() else { return };
                    *inner.peer.lock() = peer_of(&ws);
                    let (sink, s) = ws.split();
                    stream = s;
                    kill = Arc::new(Notify::new());
                    *inner.conn.lock().await = Some(Conn {
                        sink,
                        kill: kill.clone(),
                    });
                    inner.connected.store(true, Ordering::Release);
                    backoff = BACKOFF_MIN;
                    log::info!("ws post: reconnected");
                    break;
                }
                Err(e) => log::warn!("ws post: reconnect failed: {e}"),
            }
        }
    }
}

async fn housekeeping(weak: Weak<Inner>) {
    let mut sweep = tokio::time::interval(SWEEP_EVERY);
    let mut ping = tokio::time::interval_at(tokio::time::Instant::now() + PING_EVERY, PING_EVERY);
    sweep.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    ping.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            _ = sweep.tick() => {
                let Some(inner) = weak.upgrade() else { return };
                inner.expire(now_ns());
            }
            _ = ping.tick() => {
                let Some(inner) = weak.upgrade() else { return };
                let res = tokio::time::timeout(WRITE_TIMEOUT, async {
                    let mut g = inner.conn.lock().await;
                    if let Some(c) = g.as_mut() {
                        if let Err(e) = c.sink.send(Message::Text(r#"{"method":"ping"}"#.into())).await {
                            log::warn!("ws post: ping failed: {e}");
                            c.kill.notify_one();
                        }
                    }
                })
                .await;
                if res.is_err() {
                    log::warn!("ws post: ping write timed out");
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn post_frame_embeds_the_payload_verbatim() {
        let raw = r#"{"action":{"type":"cancel","cancels":[{"a":200,"o":7}]},"nonce":5,"signature":{"r":"0x1","s":"0x2","v":27},"vaultAddress":null}"#;
        let f = post_frame(42, "action", raw);
        assert!(f.contains(raw));
        let v: Value = serde_json::from_str(&f).unwrap();
        assert_eq!(
            (
                v["method"].as_str(),
                v["id"].as_u64(),
                v["request"]["type"].as_str()
            ),
            (Some("post"), Some(42), Some("action"))
        );
        assert_eq!(
            v["request"]["payload"],
            serde_json::from_str::<Value>(raw).unwrap()
        );
    }

    #[test]
    fn decode_resting() {
        let p = json!({"status":"ok","response":{"type":"order","data":{"statuses":[{"resting":{"oid":77738308}}]}}});
        assert_eq!(
            decode_status(&p),
            Ok(OrderStatus::Resting { oid: 77738308 })
        );
    }

    #[test]
    fn decode_filled() {
        let p = json!({"status":"ok","response":{"type":"order","data":{"statuses":[
            {"filled":{"totalSz":"3500.0","avgPx":"0.004201","oid":12}}]}}});
        assert_eq!(
            decode_status(&p),
            Ok(OrderStatus::Filled {
                total_sz: 3500.0,
                avg_px: 0.004201,
                oid: 12
            })
        );
    }

    #[test]
    fn decode_error_status() {
        let txt = "Order could not immediately match against any resting orders.";
        let p =
            json!({"status":"ok","response":{"type":"order","data":{"statuses":[{"error": txt}]}}});
        assert_eq!(decode_status(&p), Ok(OrderStatus::Rejected(txt.into())));
    }

    #[test]
    fn decode_success_variants() {
        let p = json!({"status":"ok","response":{"type":"cancel","data":{"statuses":["success"]}}});
        assert_eq!(decode_status(&p), Ok(OrderStatus::Success));
        let p = json!({"status":"ok","response":{"type":"batchModify","data":{"statuses":["success"]}}});
        assert_eq!(decode_status(&p), Ok(OrderStatus::Success));
        let p = json!({"status":"ok","response":{"type":"default"}});
        assert_eq!(decode_status(&p), Ok(OrderStatus::Success));
    }

    #[test]
    fn decode_err() {
        let p = json!({"status":"err","response":"User or API Wallet 0xabc does not exist."});
        assert_eq!(
            decode_status(&p),
            Ok(OrderStatus::Rejected(
                "User or API Wallet 0xabc does not exist.".into()
            ))
        );
    }

    #[test]
    fn decode_garbage() {
        assert!(decode_status(&json!({"foo": 1})).is_err());
        assert!(decode_status(
            &json!({"status":"ok","response":{"type":"order","data":{"statuses":[]}}})
        )
        .is_err());
        assert!(decode_status(
            &json!({"status":"ok","response":{"type":"order","data":{"statuses":[{"x":1}]}}})
        )
        .is_err());
        assert!(decode_status(&json!("text")).is_err());
    }

    #[test]
    fn on_text_matches_by_id() {
        let inner = Inner {
            url: String::new(),
            next_id: AtomicU64::new(1),
            connected: AtomicBool::new(false),
            conn: tokio::sync::Mutex::new(None),
            pending: Mutex::new(HashMap::new()),
            peer: Mutex::new(None),
        };
        let (tx, mut rx) = oneshot::channel();
        inner.pending.lock().insert(5, Waiter { sent_ns: 1, tx });
        let (tx2, mut rx2) = oneshot::channel();
        inner.pending.lock().insert(
            6,
            Waiter {
                sent_ns: 1,
                tx: tx2,
            },
        );
        inner.on_text(r#"{"channel":"pong"}"#, 2);
        inner.on_text(
            r#"{"channel":"post","data":{"id":5,"response":{"type":"action","payload":{"status":"ok","response":{"type":"default"}}}}}"#,
            3,
        );
        let r = rx.try_recv().unwrap();
        assert_eq!((r.id, r.sent_ns, r.recv_ns), (5, 1, 3));
        assert_eq!(r.status, Ok(OrderStatus::Success));
        assert!(rx2.try_recv().is_err());
        inner.on_text(
            r#"{"channel":"post","data":{"id":6,"response":{"type":"error","payload":"bad"}}}"#,
            4,
        );
        assert_eq!(rx2.try_recv().unwrap().status, Err("bad".into()));
    }

    #[test]
    fn expire_and_fail_all() {
        let inner = Inner {
            url: String::new(),
            next_id: AtomicU64::new(1),
            connected: AtomicBool::new(false),
            conn: tokio::sync::Mutex::new(None),
            pending: Mutex::new(HashMap::new()),
            peer: Mutex::new(None),
        };
        let (tx, mut rx) = oneshot::channel();
        inner.pending.lock().insert(1, Waiter { sent_ns: 0, tx });
        let (tx2, mut rx2) = oneshot::channel();
        inner.pending.lock().insert(
            2,
            Waiter {
                sent_ns: REQUEST_TIMEOUT_NS,
                tx: tx2,
            },
        );
        inner.expire(REQUEST_TIMEOUT_NS + 1);
        assert_eq!(rx.try_recv().unwrap().status, Err("timeout".into()));
        assert!(rx2.try_recv().is_err());
        inner.fail_all("disconnected");
        assert_eq!(rx2.try_recv().unwrap().status, Err("disconnected".into()));
    }
}
