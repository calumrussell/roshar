use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::info::{MarketData, SpotMarketData};
use super::market::{OutcomeSpec, QuestionSpec};

// Hyperliquid Book Structures
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HyperliquidBookLevel {
    pub n: u32,
    pub px: String,
    pub sz: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HyperliquidBook {
    pub coin: String,
    pub levels: Vec<Vec<HyperliquidBookLevel>>,
    pub time: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HyperliquidBookMessage {
    pub channel: String,
    pub data: HyperliquidBook,
}

// Hyperliquid Trade Structures
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HyperliquidTrade {
    pub coin: String,
    pub hash: String,
    pub px: String,         // Price as a string to preserve precision
    pub side: String,       // "B" for buy, "A" for sell
    pub sz: String,         // Size as a string to preserve precision
    pub tid: u64,           // Trade ID
    pub time: u64,          // Trade timestamp in milliseconds
    pub users: Vec<String>, // List of users involved in the trade
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HyperliquidTradesMessage {
    pub channel: String,
    pub data: Vec<HyperliquidTrade>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HyperliquidCandleData {
    #[serde(rename = "T")]
    pub close_time: u64, // Close time (epoch in millis)
    pub c: String, // Close price
    pub h: String, // High price
    pub i: String, // Interval (e.g., "1m")
    pub l: String, // Low price
    pub n: u32,    // Number of trades
    pub o: String, // Open price
    pub s: String, // Symbol (e.g., "ETH")
    pub t: u64,    // Open time (epoch in millis)
    pub v: String, // Volume
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HyperliquidCandleMessage {
    pub channel: String,
    pub data: HyperliquidCandleData,
}

// Hyperliquid User Fills Structures
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct HyperliquidUserFill {
    pub coin: String,
    pub px: String,
    pub sz: String,
    pub side: String,
    pub time: u64,
    pub start_position: String,
    pub dir: String,
    pub closed_pnl: String,
    pub hash: String,
    pub oid: u64,
    pub crossed: bool,
    pub fee: String,
    pub tid: u64,
    pub fee_token: String,
    /// Absent unless the fill was a liquidation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub liquidation: Option<serde_json::Value>,
    /// Absent when zero.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub builder_fee: Option<String>,
    /// Absent when the order had no cloid.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cloid: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct HyperliquidUserFillsData {
    #[serde(default)]
    pub is_snapshot: Option<bool>,
    pub user: String,
    pub fills: Vec<HyperliquidUserFill>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HyperliquidUserFillsMessage {
    pub channel: String,
    pub data: HyperliquidUserFillsData,
}

// Hyperliquid Order Updates Structures
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct WsBasicOrder {
    pub coin: String,
    pub side: String,
    #[serde(rename = "limitPx")]
    pub limit_px: String,
    pub sz: String,
    pub oid: u64,
    pub timestamp: u64,
    #[serde(rename = "origSz")]
    pub orig_sz: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cloid: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WsOrder {
    pub order: WsBasicOrder,
    pub status: String,
    #[serde(rename = "statusTimestamp")]
    pub status_timestamp: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HyperliquidOrderUpdatesMessage {
    pub channel: String,
    pub data: Vec<WsOrder>,
}

// Hyperliquid BBO Structures
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WsLevel {
    pub px: String,
    pub sz: String,
    pub n: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HyperliquidBbo {
    pub coin: String,
    pub time: u64,
    pub bbo: [Option<WsLevel>; 2], // [bid, ask]
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HyperliquidBboMessage {
    pub channel: String,
    pub data: HyperliquidBbo,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct HyperliquidWssSubscription {
    #[serde(rename = "type")]
    pub typ: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interval: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coin: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    /// `l2Book` only: aggregate to this many significant figures (2..=5). The ack echoes `null`
    /// when unset.
    #[serde(rename = "nSigFigs", default, skip_serializing_if = "Option::is_none")]
    pub n_sig_figs: Option<u32>,
    /// `l2Book` only, with `nSigFigs: 5`: 1, 2 or 5. An invalid combination makes the server
    /// drop the connection without an error frame.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mantissa: Option<u32>,
    /// `l2Book` only: `fast: true` sends 5 levels per side on every block instead of 20 levels
    /// throttled (measured ~1.8 vs ~0.27 updates/s). The unsubscribe must carry the same value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fast: Option<bool>,
    /// Perp dex for dex-scoped feeds (`allMids`); absent = the first perp dex.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dex: Option<String>,
}

// Hyperliquid WebSocket Message Types
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct HyperliquidWssMessage {
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subscription: Option<HyperliquidWssSubscription>,
}

fn sub(method: &str, typ: &str) -> HyperliquidWssMessage {
    HyperliquidWssMessage {
        method: method.to_string(),
        subscription: Some(HyperliquidWssSubscription {
            typ: typ.to_string(),
            interval: None,
            coin: None,
            user: None,
            n_sig_figs: None,
            mantissa: None,
            fast: None,
            dex: None,
        }),
    }
}

fn coin_sub(method: &str, typ: &str, coin: &str) -> HyperliquidWssMessage {
    let mut m = sub(method, typ);
    if let Some(s) = m.subscription.as_mut() {
        s.coin = Some(coin.to_string());
    }
    m
}

fn user_sub(method: &str, typ: &str, user: &str) -> HyperliquidWssMessage {
    let mut m = sub(method, typ);
    if let Some(s) = m.subscription.as_mut() {
        s.user = Some(user.to_string());
    }
    m
}

impl HyperliquidWssMessage {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("failed to serialize HyperliquidWssMessage")
    }

    pub fn ping() -> Self {
        Self {
            method: "ping".to_string(),
            subscription: None,
        }
    }

    /// `allMids` of the first perp dex (includes spot mids).
    pub fn all_mids() -> Self {
        sub("subscribe", "allMids")
    }

    /// `allMids` of perp dex `dex` (HIP-3, e.g. `"xyz"`).
    pub fn all_mids_dex(dex: &str) -> Self {
        let mut m = sub("subscribe", "allMids");
        if let Some(s) = m.subscription.as_mut() {
            s.dex = Some(dex.to_string());
        }
        m
    }

    pub fn all_mids_unsub() -> Self {
        sub("unsubscribe", "allMids")
    }

    pub fn all_mids_dex_unsub(dex: &str) -> Self {
        let mut m = sub("unsubscribe", "allMids");
        if let Some(s) = m.subscription.as_mut() {
            s.dex = Some(dex.to_string());
        }
        m
    }

    /// `l2Book` with every optional parameter: `nSigFigs`, `mantissa` (only with `nSigFigs = 5`)
    /// and `fast`. `None` omits the field. Unsubscribe with [`Self::l2_book_with_unsub`] and the
    /// same arguments.
    pub fn l2_book_with(
        coin: &str,
        n_sig_figs: Option<u32>,
        mantissa: Option<u32>,
        fast: Option<bool>,
    ) -> Self {
        Self::l2_book_params("subscribe", coin, n_sig_figs, mantissa, fast)
    }

    pub fn l2_book_with_unsub(
        coin: &str,
        n_sig_figs: Option<u32>,
        mantissa: Option<u32>,
        fast: Option<bool>,
    ) -> Self {
        Self::l2_book_params("unsubscribe", coin, n_sig_figs, mantissa, fast)
    }

    fn l2_book_params(
        method: &str,
        coin: &str,
        n_sig_figs: Option<u32>,
        mantissa: Option<u32>,
        fast: Option<bool>,
    ) -> Self {
        let mut m = coin_sub(method, "l2Book", coin);
        if let Some(s) = m.subscription.as_mut() {
            s.n_sig_figs = n_sig_figs;
            s.mantissa = mantissa;
            s.fast = fast;
        }
        m
    }

    /// `activeAssetCtx` for a perp or spot coin. Perp coins stream on channel `activeAssetCtx`,
    /// spot coins (`@{index}`, `PURR/USDC`) on `activeSpotAssetCtx`.
    pub fn active_asset_ctx(coin: &str) -> Self {
        coin_sub("subscribe", "activeAssetCtx", coin)
    }

    pub fn active_asset_ctx_unsub(coin: &str) -> Self {
        coin_sub("unsubscribe", "activeAssetCtx", coin)
    }

    /// `allDexsAssetCtxs`: perp asset contexts of every perp dex.
    pub fn all_dexs_asset_ctxs() -> Self {
        sub("subscribe", "allDexsAssetCtxs")
    }

    pub fn all_dexs_asset_ctxs_unsub() -> Self {
        sub("unsubscribe", "allDexsAssetCtxs")
    }

    /// `outcomeMetaUpdates`: outcome/question creations and settlements.
    pub fn outcome_meta_updates() -> Self {
        sub("subscribe", "outcomeMetaUpdates")
    }

    pub fn outcome_meta_updates_unsub() -> Self {
        sub("unsubscribe", "outcomeMetaUpdates")
    }

    /// `fastAssetCtxs`: compressed mark/mid updates of every coin. Decode the payload with
    /// [`decode_fast_asset_ctxs`].
    pub fn fast_asset_ctxs() -> Self {
        sub("subscribe", "fastAssetCtxs")
    }

    pub fn fast_asset_ctxs_unsub() -> Self {
        sub("unsubscribe", "fastAssetCtxs")
    }

    pub fn l2_book(coin: &str) -> Self {
        coin_sub("subscribe", "l2Book", coin)
    }

    /// `l2Book` with `fast: true` (5 levels per side, every block). Unsubscribe with
    /// [`Self::l2_book_fast_unsub`].
    pub fn l2_book_fast(coin: &str) -> Self {
        let mut m = Self::l2_book(coin);
        if let Some(s) = m.subscription.as_mut() {
            s.fast = Some(true);
        }
        m
    }

    pub fn l2_book_fast_unsub(coin: &str) -> Self {
        let mut m = Self::l2_book_unsub(coin);
        if let Some(s) = m.subscription.as_mut() {
            s.fast = Some(true);
        }
        m
    }

    pub fn l2_book_unsub(coin: &str) -> Self {
        coin_sub("unsubscribe", "l2Book", coin)
    }

    /// `candle` for `coin` at `interval` ("1m", "3m", "5m", "15m", "30m", "1h", "2h", "4h", "8h", "12h",
    /// "1d", "3d", "1w", "1M").
    pub fn candle(coin: &str, interval: &str) -> Self {
        let mut m = coin_sub("subscribe", "candle", coin);
        if let Some(s) = m.subscription.as_mut() {
            s.interval = Some(interval.to_string());
        }
        m
    }

    pub fn candle_unsub(coin: &str, interval: &str) -> Self {
        let mut m = coin_sub("unsubscribe", "candle", coin);
        if let Some(s) = m.subscription.as_mut() {
            s.interval = Some(interval.to_string());
        }
        m
    }

    pub fn trades(coin: &str) -> Self {
        coin_sub("subscribe", "trades", coin)
    }

    pub fn trades_unsub(coin: &str) -> Self {
        coin_sub("unsubscribe", "trades", coin)
    }

    pub fn user_fills(user_address: &str) -> Self {
        user_sub("subscribe", "userFills", user_address)
    }

    pub fn user_fills_unsub(user_address: &str) -> Self {
        user_sub("unsubscribe", "userFills", user_address)
    }

    pub fn order_updates(user_address: &str) -> Self {
        user_sub("subscribe", "orderUpdates", user_address)
    }

    pub fn order_updates_unsub(user_address: &str) -> Self {
        user_sub("unsubscribe", "orderUpdates", user_address)
    }

    pub fn bbo(coin: &str) -> Self {
        coin_sub("subscribe", "bbo", coin)
    }

    pub fn bbo_unsub(coin: &str) -> Self {
        coin_sub("unsubscribe", "bbo", coin)
    }
}

/// `allMids` data. `dex` is present only on HIP-3 dex subscriptions.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WsAllMids {
    pub mids: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dex: Option<String>,
}

/// `activeAssetCtx` data (perp coin).
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WsActiveAssetCtx {
    pub coin: String,
    pub ctx: MarketData,
}

/// `activeSpotAssetCtx` data (spot coin subscribed through `activeAssetCtx`).
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WsActiveSpotAssetCtx {
    pub coin: String,
    pub ctx: SpotMarketData,
}

/// `allDexsAssetCtxs` data: `[dex name ("" = first perp dex), contexts index-aligned with that dex's meta]`.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WsAllDexsAssetCtxs {
    pub ctxs: Vec<(String, Vec<MarketData>)>,
}

/// One `outcomeMetaUpdates` element.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub enum WsOutcomeMetaUpdate {
    OutcomeCreated(OutcomeSpec),
    OutcomeSettled(u64),
    QuestionUpdated(QuestionSpec),
    QuestionSettled(u64),
}

/// One coin of a decoded `fastAssetCtxs` payload. The first message is a full snapshot; later ones
/// carry only changed coins and omit unchanged fields (`None`). `mid_px: Some(None)` is an explicit
/// `null` (empty book).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FastAssetCtx {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mark_px: Option<String>,
    #[serde(
        default,
        deserialize_with = "present_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub mid_px: Option<Option<String>>,
}

/// Distinguishes a present `null` (`Some(None)`) from an absent field (`None`, via `default`).
fn present_option<'de, D, T>(d: D) -> Result<Option<Option<T>>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(d).map(Some)
}

/// Decodes a `fastAssetCtxs` payload: base64, then raw DEFLATE (RFC 1951), then JSON keyed by coin.
pub fn decode_fast_asset_ctxs(payload: &str) -> anyhow::Result<BTreeMap<String, FastAssetCtx>> {
    use base64::Engine;
    use std::io::Read;
    let compressed = base64::engine::general_purpose::STANDARD.decode(payload)?;
    let mut json = Vec::new();
    flate2::read::DeflateDecoder::new(compressed.as_slice()).read_to_end(&mut json)?;
    Ok(serde_json::from_slice(&json)?)
}

/// A frame from the market-data websocket, tagged by `channel`. Unknown channels (e.g. `post`
/// responses, private user feeds other than `userFills`/`orderUpdates`) are a parse error.
#[derive(Deserialize, Debug, Clone)]
#[serde(tag = "channel", content = "data", rename_all = "camelCase")]
pub enum HyperliquidWsEvent {
    /// Ack of a subscribe/unsubscribe, echoing the subscription with server defaults filled in.
    SubscriptionResponse(HyperliquidWssMessage),
    /// e.g. `"Error parsing JSON into valid websocket request: ..."`.
    Error(String),
    Pong,
    AllMids(WsAllMids),
    L2Book(HyperliquidBook),
    Trades(Vec<HyperliquidTrade>),
    Candle(HyperliquidCandleData),
    Bbo(HyperliquidBbo),
    ActiveAssetCtx(WsActiveAssetCtx),
    ActiveSpotAssetCtx(WsActiveSpotAssetCtx),
    AllDexsAssetCtxs(WsAllDexsAssetCtxs),
    OutcomeMetaUpdates(Vec<WsOutcomeMetaUpdate>),
    /// Compressed payload; see [`decode_fast_asset_ctxs`].
    FastAssetCtxs(String),
    UserFills(HyperliquidUserFillsData),
    OrderUpdates(Vec<WsOrder>),
}

impl HyperliquidWsEvent {
    pub fn parse(text: &str) -> serde_json::Result<Self> {
        serde_json::from_str(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wss_message_ping() {
        let msg = HyperliquidWssMessage::ping();
        let json = msg.to_json();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["method"], "ping");
        assert!(parsed["subscription"].is_null());
    }

    #[test]
    fn test_wss_message_candle() {
        let coin = "ETH";
        let msg = HyperliquidWssMessage::candle(coin, "15m");
        let json = msg.to_json();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["method"], "subscribe");
        assert_eq!(parsed["subscription"]["type"], "candle");
        assert_eq!(parsed["subscription"]["coin"], coin);
        assert_eq!(parsed["subscription"]["interval"], "15m");
    }

    #[test]
    fn test_wss_message_all_mids_dex() {
        assert_eq!(
            HyperliquidWssMessage::all_mids_dex("xyz").to_json(),
            r#"{"method":"subscribe","subscription":{"type":"allMids","dex":"xyz"}}"#
        );
        assert_eq!(
            HyperliquidWssMessage::all_mids().to_json(),
            r#"{"method":"subscribe","subscription":{"type":"allMids"}}"#
        );
    }

    #[test]
    fn test_wss_message_l2_book() {
        let coin = "BTC";
        let msg = HyperliquidWssMessage::l2_book(coin);
        let json = msg.to_json();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["method"], "subscribe");
        assert_eq!(parsed["subscription"]["type"], "l2Book");
        assert_eq!(parsed["subscription"]["coin"], coin);
        assert!(parsed["subscription"]["interval"].is_null());
    }

    #[test]
    fn test_wss_message_trades() {
        let coin = "SOL";
        let msg = HyperliquidWssMessage::trades(coin);
        let json = msg.to_json();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["method"], "subscribe");
        assert_eq!(parsed["subscription"]["type"], "trades");
        assert_eq!(parsed["subscription"]["coin"], coin);
        assert!(parsed["subscription"]["interval"].is_null());
    }

    #[test]
    fn test_wss_message_all_mids() {
        let msg = HyperliquidWssMessage::all_mids();
        assert_eq!(msg.method, "subscribe");
        let sub = msg.subscription.unwrap();
        assert_eq!(sub.typ, "allMids");
        assert!(sub.coin.is_none());
        assert!(sub.interval.is_none());
    }
}

#[cfg(test)]
mod fast_l2_book_tests {
    use super::HyperliquidWssMessage;

    #[test]
    fn plain_l2_book_has_no_fast_field() {
        assert_eq!(
            HyperliquidWssMessage::l2_book("ETH").to_json(),
            r#"{"method":"subscribe","subscription":{"type":"l2Book","coin":"ETH"}}"#
        );
    }

    #[test]
    fn fast_l2_book_subscribe_and_unsubscribe_match() {
        assert_eq!(
            HyperliquidWssMessage::l2_book_fast("ETH").to_json(),
            r#"{"method":"subscribe","subscription":{"type":"l2Book","coin":"ETH","fast":true}}"#
        );
        assert_eq!(
            HyperliquidWssMessage::l2_book_fast_unsub("ETH").to_json(),
            r#"{"method":"unsubscribe","subscription":{"type":"l2Book","coin":"ETH","fast":true}}"#
        );
    }
}
