//! Hyperliquid `/info` over HTTP. Not on the latency-critical path.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use crate::types::exchange::{cloid_hex, Cloid, L2Snapshot, Level};
use crate::types::{
    HistoricalFundingRate, HyperliquidBook, HyperliquidCandleData, HyperliquidTrade,
    MetaAndAssetCtxs, OutcomeMeta, OutcomeTemplate, PerpAnnotation, PerpConciseAnnotation, PerpDex,
    PerpDexLimits, PerpDexStatus, PredictedFundings, SettledOutcome, SpotClearinghouseState,
    SpotMeta, SpotMetaAndAssetCtxs, TokenDetails, Universe, UserPerpetualsState,
};
use anyhow::{anyhow, bail, Context};
use leaky_bucket::RateLimiter;
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

/// Perp dex of a coin name: `"xyz"` for the HIP-3 coin `"xyz:XYZ100"`, `""` (the first perp dex) otherwise.
pub fn perp_dex_of(coin: &str) -> &str {
    coin.split_once(':').map_or("", |(dex, _)| dex)
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

    /// Like [`Self::post_as`], but a `null` response (how `/info` answers an unknown coin/dex or
    /// invalid parameters) is an error naming the request.
    async fn post_some<T: serde::de::DeserializeOwned>(&self, body: Value) -> anyhow::Result<T> {
        let v = self.post(body.clone()).await?;
        if v.is_null() {
            bail!(
                "info {}: null response (unknown coin/dex or invalid parameters): {body}",
                body["type"]
            );
        }
        Ok(serde_json::from_value(v)?)
    }

    /// `allMids` of perp dex `dex` (`""` = the first perp dex, the only one that includes spot and
    /// outcome mids). Keyed by coin.
    pub async fn all_mids(&self, dex: &str) -> anyhow::Result<BTreeMap<String, String>> {
        self.post_as(json!({"type": "allMids", "dex": dex})).await
    }

    /// `l2Book` (at most 20 levels per side). `n_sig_figs` (2..=5) and `mantissa` (1, 2 or 5; only
    /// with `n_sig_figs = Some(5)`) aggregate levels; `None` omits them (full precision). An unknown
    /// coin or invalid combination is an error.
    pub async fn l2_book(
        &self,
        coin: &str,
        n_sig_figs: Option<u32>,
        mantissa: Option<u32>,
    ) -> anyhow::Result<HyperliquidBook> {
        let mut req = json!({"type": "l2Book", "coin": coin});
        if let Some(n) = n_sig_figs {
            req["nSigFigs"] = json!(n);
        }
        if let Some(m) = mantissa {
            req["mantissa"] = json!(m);
        }
        self.post_some(req).await
    }

    /// `recentTrades` of `coin`. Not in the info-endpoint docs; answered live (2026-11-16) with the
    /// websocket `WsTrade` shape.
    pub async fn recent_trades(&self, coin: &str) -> anyhow::Result<Vec<HyperliquidTrade>> {
        self.post_as(json!({"type": "recentTrades", "coin": coin}))
            .await
    }

    /// `meta` (universe and margin tables) of perp dex `dex` (`""` = the first perp dex).
    pub async fn meta(&self, dex: &str) -> anyhow::Result<Universe> {
        self.post_some(json!({"type": "meta", "dex": dex})).await
    }

    /// `allPerpMetas`: `meta` of every perp dex, in `perpDexs` order. (The docs show meta/ctx pairs;
    /// the live response is metas only.)
    pub async fn all_perp_metas(&self) -> anyhow::Result<Vec<Universe>> {
        self.post_as(json!({"type": "allPerpMetas"})).await
    }

    /// `perpDexs`. Element 0 is `None`: the first perp dex has no entry.
    pub async fn perp_dexs(&self) -> anyhow::Result<Vec<Option<PerpDex>>> {
        self.post_as(json!({"type": "perpDexs"})).await
    }

    /// `perpDexLimits` of a HIP-3 dex (`""` is not allowed).
    pub async fn perp_dex_limits(&self, dex: &str) -> anyhow::Result<PerpDexLimits> {
        self.post_some(json!({"type": "perpDexLimits", "dex": dex}))
            .await
    }

    /// `perpDexStatus` of perp dex `dex` (`""` = the first perp dex).
    pub async fn perp_dex_status(&self, dex: &str) -> anyhow::Result<PerpDexStatus> {
        self.post_some(json!({"type": "perpDexStatus", "dex": dex}))
            .await
    }

    /// `predictedFundings` (first perp dex only): per coin, each venue's next funding.
    pub async fn predicted_fundings(&self) -> anyhow::Result<PredictedFundings> {
        self.post_as(json!({"type": "predictedFundings"})).await
    }

    /// `perpsAtOpenInterestCap` of perp dex `dex` (`""` = the first perp dex).
    pub async fn perps_at_open_interest_cap(&self, dex: &str) -> anyhow::Result<Vec<String>> {
        self.post_as(json!({"type": "perpsAtOpenInterestCap", "dex": dex}))
            .await
    }

    /// `perpAnnotation` of `coin`; `None` when the coin has no annotation.
    pub async fn perp_annotation(&self, coin: &str) -> anyhow::Result<Option<PerpAnnotation>> {
        self.post_as(json!({"type": "perpAnnotation", "coin": coin}))
            .await
    }

    /// `perpCategories`: `[coin, category]` pairs.
    pub async fn perp_categories(&self) -> anyhow::Result<Vec<(String, String)>> {
        self.post_as(json!({"type": "perpCategories"})).await
    }

    /// `perpConciseAnnotations`: `[coin, annotation]` pairs.
    pub async fn perp_concise_annotations(
        &self,
    ) -> anyhow::Result<Vec<(String, PerpConciseAnnotation)>> {
        self.post_as(json!({"type": "perpConciseAnnotations"}))
            .await
    }

    /// `spotMeta`: spot tokens and pairs.
    pub async fn spot_meta(&self) -> anyhow::Result<SpotMeta> {
        self.post_as(json!({"type": "spotMeta"})).await
    }

    /// `tokenDetails` of `token_id` (the 34-character `tokenId` from `spotMeta`).
    pub async fn token_details(&self, token_id: &str) -> anyhow::Result<TokenDetails> {
        self.post_some(json!({"type": "tokenDetails", "tokenId": token_id}))
            .await
    }

    /// `outcomeMeta`: outcome markets, questions and deployers.
    pub async fn outcome_meta(&self) -> anyhow::Result<OutcomeMeta> {
        self.post_as(json!({"type": "outcomeMeta"})).await
    }

    /// `outcomeTemplates`.
    pub async fn outcome_templates(&self) -> anyhow::Result<Vec<OutcomeTemplate>> {
        self.post_as(json!({"type": "outcomeTemplates"})).await
    }

    /// `settledOutcome`; `None` for an unknown or unsettled outcome.
    pub async fn settled_outcome(&self, outcome: u64) -> anyhow::Result<Option<SettledOutcome>> {
        self.post_as(json!({"type": "settledOutcome", "outcome": outcome}))
            .await
    }

    /// `metaAndAssetCtxs` for perp dex `dex` (`""` = the first perp dex): the universe and each asset's
    /// context, index-aligned.
    pub async fn meta_and_asset_ctxs(&self, dex: &str) -> anyhow::Result<MetaAndAssetCtxs> {
        self.post_as(json!({"type": "metaAndAssetCtxs", "dex": dex}))
            .await
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

    /// `clearinghouseState`: perp margin summary and positions of `user` on perp dex `dex`
    /// (`""` = the first perp dex).
    pub async fn clearinghouse_state(
        &self,
        user: &str,
        dex: &str,
    ) -> anyhow::Result<UserPerpetualsState> {
        self.post_as(json!({"type": "clearinghouseState", "user": user, "dex": dex}))
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

    /// (asset id, szDecimals) of a perp coin. HIP-3 coins (`{dex}:{coin}`) are looked up in their dex's
    /// `meta` and get asset id `100000 + perp_dex_index * 10000 + index_in_meta`.
    pub async fn asset_meta(&self, coin: &str) -> anyhow::Result<(u32, u32)> {
        let dex = perp_dex_of(coin);
        let offset = if dex.is_empty() {
            0
        } else {
            let dexs = self.post(json!({"type": "perpDexs"})).await?;
            let i = dexs
                .as_array()
                .ok_or_else(|| anyhow!("perpDexs: not an array"))?
                .iter()
                .position(|d| d["name"].as_str() == Some(dex))
                .ok_or_else(|| anyhow!("perpDexs: unknown dex {dex}"))?;
            100_000 + 10_000 * i as u32
        };
        let v = self.post(json!({"type": "meta", "dex": dex})).await?;
        let universe = v["universe"]
            .as_array()
            .ok_or_else(|| anyhow!("meta {dex:?}: no universe"))?;
        for (i, a) in universe.iter().enumerate() {
            if a["name"].as_str() == Some(coin) {
                let d = a["szDecimals"]
                    .as_u64()
                    .ok_or_else(|| anyhow!("meta: {coin} has no szDecimals"))?;
                return Ok((offset + i as u32, d as u32));
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

    /// Signed position size of `user` in `coin`; 0 if flat. HIP-3 coins are looked up on their own dex.
    pub async fn position_szi(&self, user: &str, coin: &str) -> anyhow::Result<f64> {
        let v = self
            .post(json!({"type": "clearinghouseState", "user": user, "dex": perp_dex_of(coin)}))
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

    /// `userFillsByTime` from `start_ms` to now. The exchange returns at most 2000 fills per response and
    /// keeps only the 10000 most recent; this makes one request, so 2000 fills means the result may be cut.
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
        let vlm = num(v["cumVlm"]
            .as_str()
            .ok_or_else(|| anyhow!("userRateLimit: no cumVlm"))?)?;
        Ok((used, cap, vlm))
    }

    /// `frontendOpenOrders` of `user` on perp dex `dex` (`""` = the first perp dex, the only one whose
    /// response includes spot orders).
    pub async fn open_orders(&self, user: &str, dex: &str) -> anyhow::Result<Vec<OpenOrder>> {
        let raw: Vec<RawOpenOrder> = self
            .post_as(json!({"type": "frontendOpenOrders", "user": user, "dex": dex}))
            .await?;
        raw.into_iter()
            .map(|o| {
                Ok(OpenOrder {
                    px: num(&o.limit_px)?,
                    sz: num(&o.sz)?,
                    is_buy: match o.side.as_str() {
                        "B" => true,
                        "A" => false,
                        s => bail!("frontendOpenOrders: bad side {s:?}"),
                    },
                    coin: o.coin,
                    oid: o.oid,
                    cloid: o.cloid,
                })
            })
            .collect()
    }
}
