//! Hyperliquid `/info` over HTTP. Not on the latency-critical path.

use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, bail, Context};
use leaky_bucket::RateLimiter;
use crate::types::exchange::{cloid_hex, Cloid, L2Snapshot, Level};
use crate::types::{
    HistoricalFundingRate, HyperliquidCandleData, MetaAndAssetCtxs, SpotClearinghouseState,
    SpotMetaAndAssetCtxs, UserPerpetualsState,
};
use serde::Deserialize;
use serde_json::{json, Value};

pub const HL_INFO_URL: &str = "https://api.hyperliquid.xyz/info";
pub const HL_TESTNET_INFO_URL: &str = "https://api.hyperliquid-testnet.xyz/info";

#[derive(Clone, Debug, PartialEq)]
pub struct Fill {
    pub coin: String,
    pub px: f64,
    pub sz: f64,
    /// "B" / "A".
    pub side: String,
    /// e.g. "Open Long", "Close Short".
    pub dir: String,
    pub fee: f64,
    pub crossed: bool,
    pub time_ms: u64,
    pub closed_pnl: f64,
    pub cloid: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct OpenOrder {
    pub coin: String,
    pub oid: u64,
    pub cloid: Option<String>,
    pub is_buy: bool,
    pub px: f64,
    pub sz: f64,
}

/// Reusable HTTP client (keep-alive, 5 s timeouts by default). No request throttling unless
/// [`Info::with_rate_limit`] is set.
#[derive(Clone)]
pub struct Info {
    http: reqwest::Client,
    info_url: String,
    limiter: Option<Arc<RateLimiter>>,
}

fn http_client(timeout: Duration) -> anyhow::Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        // Pin rustls: a workspace build can enable reqwest's default-tls, which would otherwise win.
        .use_rustls_tls()
        .timeout(timeout)
        .connect_timeout(Duration::from_secs(5))
        .pool_idle_timeout(Duration::from_secs(90))
        .tcp_keepalive(Duration::from_secs(30))
        .tcp_nodelay(true)
        .build()?)
}

/// `fundingHistory` returns at most this many entries per request.
const FUNDING_HISTORY_PAGE: usize = 500;

fn num(s: &str) -> anyhow::Result<f64> {
    s.parse::<f64>()
        .with_context(|| format!("bad number {s:?}"))
}

#[derive(Deserialize)]
struct RawLevel {
    px: String,
    sz: String,
    n: u32,
}

#[derive(Deserialize)]
struct RawBook {
    time: u64,
    levels: (Vec<RawLevel>, Vec<RawLevel>),
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawFill {
    coin: String,
    px: String,
    sz: String,
    side: String,
    dir: String,
    fee: String,
    crossed: bool,
    time: u64,
    closed_pnl: String,
    #[serde(default)]
    cloid: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawOpenOrder {
    coin: String,
    oid: u64,
    #[serde(default)]
    cloid: Option<String>,
    side: String,
    limit_px: String,
    sz: String,
}

impl Info {
    pub fn new() -> anyhow::Result<Info> {
        Info::with_url(HL_INFO_URL)
    }

    pub fn testnet() -> anyhow::Result<Info> {
        Info::with_url(HL_TESTNET_INFO_URL)
    }

    /// For tests / alternative endpoints.
    pub fn with_url(info_url: &str) -> anyhow::Result<Info> {
        Ok(Info {
            http: http_client(Duration::from_secs(5))?,
            info_url: info_url.to_string(),
            limiter: None,
        })
    }

    /// Whole-request timeout (default 5 s).
    pub fn with_timeout(mut self, timeout: Duration) -> anyhow::Result<Info> {
        self.http = http_client(timeout)?;
        Ok(self)
    }

    /// Allow `refill` requests per `interval`, with no initial burst: every request waits for a token.
    /// Shared by clones of this `Info`.
    pub fn with_rate_limit(mut self, refill: usize, interval: Duration) -> Info {
        self.limiter = Some(Arc::new(
            RateLimiter::builder()
                .initial(0)
                .max(refill)
                .refill(refill)
                .interval(interval)
                .build(),
        ));
        self
    }

    async fn post(&self, body: Value) -> anyhow::Result<Value> {
        if let Some(l) = &self.limiter {
            l.acquire_one().await;
        }
        let resp = self.http.post(&self.info_url).json(&body).send().await?;
        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            bail!("info {}: HTTP {status}: {text}", body["type"]);
        }
        Ok(resp.json().await?)
    }

    async fn post_as<T: serde::de::DeserializeOwned>(&self, body: Value) -> anyhow::Result<T> {
        let v = self.post(body).await?;
        Ok(serde_json::from_value(v)?)
    }

    /// `metaAndAssetCtxs`: the perp universe and each asset's context, index-aligned.
    pub async fn meta_and_asset_ctxs(&self) -> anyhow::Result<MetaAndAssetCtxs> {
        self.post_as(json!({"type": "metaAndAssetCtxs"})).await
    }

    /// `spotMetaAndAssetCtxs`: spot tokens, the spot universe and each pair's context.
    pub async fn spot_meta_and_asset_ctxs(&self) -> anyhow::Result<SpotMetaAndAssetCtxs> {
        self.post_as(json!({"type": "spotMetaAndAssetCtxs"})).await
    }

    /// `candleSnapshot` in [start_ms, end_ms].
    pub async fn candle_snapshot(
        &self,
        coin: &str,
        interval: &str,
        start_ms: u64,
        end_ms: u64,
    ) -> anyhow::Result<Vec<HyperliquidCandleData>> {
        self.post_as(json!({"type": "candleSnapshot",
                            "req": {"coin": coin, "interval": interval, "startTime": start_ms, "endTime": end_ms}}))
            .await
    }

    /// `fundingHistory` for `coin` from `start_ms` (to `end_ms`, or now), following pages of
    /// [`FUNDING_HISTORY_PAGE`] entries until the range is exhausted.
    pub async fn funding_history(
        &self,
        coin: &str,
        start_ms: u64,
        end_ms: Option<u64>,
    ) -> anyhow::Result<Vec<HistoricalFundingRate>> {
        let mut all = Vec::new();
        let mut start = start_ms;
        loop {
            let mut req = json!({"type": "fundingHistory", "coin": coin, "startTime": start});
            if let Some(e) = end_ms {
                req["endTime"] = json!(e);
            }
            let page: Vec<HistoricalFundingRate> = self.post_as(req).await?;
            let n = page.len();
            let Some(last) = page.last().map(|r| r.time) else {
                break;
            };
            all.extend(page);
            if n < FUNDING_HISTORY_PAGE || end_ms.is_some_and(|e| last >= e) {
                break;
            }
            start = last + 1;
        }
        Ok(all)
    }

    /// `clearinghouseState`: perp margin summary and positions of `user`.
    pub async fn clearinghouse_state(&self, user: &str) -> anyhow::Result<UserPerpetualsState> {
        self.post_as(json!({"type": "clearinghouseState", "user": user}))
            .await
    }

    /// `spotClearinghouseState`: spot balances of `user`.
    pub async fn spot_clearinghouse_state(
        &self,
        user: &str,
    ) -> anyhow::Result<SpotClearinghouseState> {
        self.post_as(json!({"type": "spotClearinghouseState", "user": user}))
            .await
    }

    /// (asset index, szDecimals) of a perp coin.
    pub async fn asset_meta(&self, coin: &str) -> anyhow::Result<(u32, u32)> {
        let v = self.post(json!({"type": "meta"})).await?;
        let universe = v["universe"]
            .as_array()
            .ok_or_else(|| anyhow!("meta: no universe"))?;
        for (i, a) in universe.iter().enumerate() {
            if a["name"].as_str() == Some(coin) {
                let d = a["szDecimals"]
                    .as_u64()
                    .ok_or_else(|| anyhow!("meta: {coin} has no szDecimals"))?;
                return Ok((i as u32, d as u32));
            }
        }
        bail!("meta: unknown coin {coin}")
    }

    pub async fn l2_snapshot(&self, coin: &str) -> anyhow::Result<L2Snapshot> {
        let b: RawBook = self
            .post_as(json!({"type": "l2Book", "coin": coin}))
            .await?;
        let conv = |ls: Vec<RawLevel>| -> anyhow::Result<Vec<Level>> {
            ls.into_iter()
                .map(|l| {
                    Ok(Level {
                        px: num(&l.px)?,
                        sz: num(&l.sz)?,
                        n: l.n,
                    })
                })
                .collect()
        };
        Ok(L2Snapshot {
            time_ms: b.time,
            bids: conv(b.levels.0)?,
            asks: conv(b.levels.1)?,
        })
    }

    /// Candle closes in [start_ms, end_ms]: (open time ms, close). `interval` as in `candleSnapshot` ("1m", "1h", ...).
    pub async fn candle_closes(
        &self,
        coin: &str,
        interval: &str,
        start_ms: u64,
        end_ms: u64,
    ) -> anyhow::Result<Vec<(u64, f64)>> {
        let v = self
            .post(json!({"type": "candleSnapshot",
                         "req": {"coin": coin, "interval": interval, "startTime": start_ms, "endTime": end_ms}}))
            .await?;
        let arr = v
            .as_array()
            .ok_or_else(|| anyhow!("candleSnapshot: not an array"))?;
        arr.iter()
            .map(|c| {
                let t = c["t"].as_u64().ok_or_else(|| anyhow!("candle without t"))?;
                let close = num(c["c"].as_str().ok_or_else(|| anyhow!("candle without c"))?)?;
                Ok((t, close))
            })
            .collect()
    }

    /// Signed position size of `user` in `coin`; 0 if flat.
    pub async fn position_szi(&self, user: &str, coin: &str) -> anyhow::Result<f64> {
        let v = self
            .post(json!({"type": "clearinghouseState", "user": user}))
            .await?;
        let Some(ps) = v["assetPositions"].as_array() else {
            bail!("clearinghouseState: no assetPositions")
        };
        for p in ps {
            let pos = &p["position"];
            if pos["coin"].as_str() == Some(coin) {
                return num(pos["szi"]
                    .as_str()
                    .ok_or_else(|| anyhow!("position without szi"))?);
            }
        }
        Ok(0.0)
    }

    /// The master account of sub-account `user` (`userRole`), lower-cased.
    pub async fn master_of(&self, user: &str) -> anyhow::Result<String> {
        let v = self.post(json!({"type": "userRole", "user": user})).await?;
        if v["role"].as_str() != Some("subAccount") {
            bail!("userRole: not a sub-account (role {})", v["role"]);
        }
        Ok(v["data"]["master"]
            .as_str()
            .ok_or_else(|| anyhow!("userRole: no data.master"))?
            .to_lowercase())
    }

    /// Status string ("open", "filled", "canceled", ...) of an order by cloid; None if unknown.
    pub async fn order_status(&self, user: &str, cloid: &Cloid) -> anyhow::Result<Option<String>> {
        let v = self
            .post(json!({"type": "orderStatus", "user": user, "oid": cloid_hex(cloid)}))
            .await?;
        match v["status"].as_str() {
            Some("order") => Ok(Some(
                v["order"]["status"]
                    .as_str()
                    .ok_or_else(|| anyhow!("orderStatus: no order.status"))?
                    .to_string(),
            )),
            Some("unknownOid") => Ok(None),
            _ => bail!("orderStatus: unexpected response {v}"),
        }
    }

    pub async fn fills_since(&self, user: &str, start_ms: u64) -> anyhow::Result<Vec<Fill>> {
        let raw: Vec<RawFill> = self
            .post_as(json!({"type": "userFillsByTime", "user": user, "startTime": start_ms}))
            .await?;
        raw.into_iter()
            .map(|f| {
                Ok(Fill {
                    px: num(&f.px)?,
                    sz: num(&f.sz)?,
                    fee: num(&f.fee)?,
                    closed_pnl: num(&f.closed_pnl)?,
                    coin: f.coin,
                    side: f.side,
                    dir: f.dir,
                    crossed: f.crossed,
                    time_ms: f.time,
                    cloid: f.cloid,
                })
            })
            .collect()
    }

    /// `userRateLimit`: (nRequestsUsed, nRequestsCap, cumVlm) of the address-based action limit.
    pub async fn user_rate_limit(&self, user: &str) -> anyhow::Result<(u64, u64, f64)> {
        let v = self
            .post(json!({"type": "userRateLimit", "user": user}))
            .await?;
        let used = v["nRequestsUsed"]
            .as_u64()
            .ok_or_else(|| anyhow!("userRateLimit: no nRequestsUsed"))?;
        let cap = v["nRequestsCap"]
            .as_u64()
            .ok_or_else(|| anyhow!("userRateLimit: no nRequestsCap"))?;
        let vlm = v["cumVlm"]
            .as_str()
            .and_then(|s| s.parse().ok())
            .or_else(|| v["cumVlm"].as_f64())
            .unwrap_or(f64::NAN);
        Ok((used, cap, vlm))
    }

    pub async fn open_orders(&self, user: &str) -> anyhow::Result<Vec<OpenOrder>> {
        let raw: Vec<RawOpenOrder> = self
            .post_as(json!({"type": "frontendOpenOrders", "user": user}))
            .await?;
        raw.into_iter()
            .map(|o| {
                Ok(OpenOrder {
                    px: num(&o.limit_px)?,
                    sz: num(&o.sz)?,
                    is_buy: o.side == "B",
                    coin: o.coin,
                    oid: o.oid,
                    cloid: o.cloid,
                })
            })
            .collect()
    }
}
