//! Websocket request frames: `{"id","method","params"}`. `id` is the caller's request id; the
//! server echoes it as `id` / `requestId`. Public channels and unsigned [`InfoPost`] queries go to
//! [`SYNTHETIX_INFO_WSS_URL`]; `subAccountUpdates`, `auth` and signed `post` actions to
//! [`SYNTHETIX_TRADE_WSS_URL`]. Signed frames come from
//! [`SignedRequest::ws_post`](crate::signer::SignedRequest::ws_post) and
//! [`WsAuth::ws_message`](crate::signer::WsAuth::ws_message).

use crate::rest::market::InfoParams;
use serde::Serialize;

pub const SYNTHETIX_INFO_WSS_URL: &str = "wss://papi.synthetix.io/v1/ws/info";
pub const SYNTHETIX_TRADE_WSS_URL: &str = "wss://papi.synthetix.io/v1/ws/trade";

/// Application-level keepalive. The docs ask for one every 30 s; the reply is
/// `{"id":"ping","result":{"message":"pong"},"status":200,…}`.
pub const SYNTHETIX_WS_PING: &str = r#"{"id":"ping","method":"ping","params":{}}"#;

#[derive(Serialize)]
struct Frame<'a, P: Serialize> {
    id: &'a str,
    method: &'a str,
    params: P,
}

fn frame<P: Serialize>(id: &str, method: &str, params: P) -> String {
    serde_json::to_string(&Frame { id, method, params }).expect("frame serialization cannot fail")
}

/// `orderbook` delivery format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum BookFormat {
    /// Initial snapshot, then changed levels (`quantity: "0"` removes a level).
    Diff,
    /// Full book on every update.
    Snapshot,
}

/// Optional `orderbook` subscription settings; unset fields use the exchange defaults
/// (diff, 50 levels, 250 ms).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BookOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<BookFormat>,
    /// 10, 50 or 100.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depth: Option<u32>,
    /// 50, 100, 250, 500 or 1000 (250/500/1000 at depth 100).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub update_frequency_ms: Option<u32>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BookParams<'a> {
    #[serde(rename = "type")]
    kind: &'a str,
    symbol: &'a str,
    #[serde(flatten)]
    options: BookOptions,
}

#[derive(Serialize)]
struct SymbolParams<'a> {
    #[serde(rename = "type")]
    kind: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    symbol: Option<&'a str>,
}

#[derive(Serialize)]
struct CandleParams<'a> {
    #[serde(rename = "type")]
    kind: &'a str,
    symbol: &'a str,
    timeframe: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SubAccountParams<'a> {
    #[serde(rename = "type")]
    kind: &'a str,
    sub_account_id: String,
}

/// `ping` with the caller's id.
pub fn ping(id: &str) -> String {
    frame(id, "ping", serde_json::Map::new())
}

/// `orderbook` (`orderbook_depth_update` pushes). One symbol per subscription; `ALL` is rejected.
pub fn subscribe_orderbook(id: &str, symbol: &str, options: BookOptions) -> String {
    frame(
        id,
        "subscribe",
        BookParams {
            kind: "orderbook",
            symbol,
            options,
        },
    )
}

pub fn unsubscribe_orderbook(id: &str, symbol: &str) -> String {
    frame(
        id,
        "unsubscribe",
        SymbolParams {
            kind: "orderbook",
            symbol: Some(symbol),
        },
    )
}

/// `trades` (`trade` pushes). `symbol: None` (or `"ALL"`) streams every market.
pub fn subscribe_trades(id: &str, symbol: Option<&str>) -> String {
    frame(
        id,
        "subscribe",
        SymbolParams {
            kind: "trades",
            symbol,
        },
    )
}

pub fn unsubscribe_trades(id: &str, symbol: Option<&str>) -> String {
    frame(
        id,
        "unsubscribe",
        SymbolParams {
            kind: "trades",
            symbol,
        },
    )
}

/// `marketPrices` (`market_price_update` pushes: one of mark/last/index/mid per message).
/// `symbol: None` (or `"ALL"`) streams every market.
pub fn subscribe_market_prices(id: &str, symbol: Option<&str>) -> String {
    frame(
        id,
        "subscribe",
        SymbolParams {
            kind: "marketPrices",
            symbol,
        },
    )
}

pub fn unsubscribe_market_prices(id: &str, symbol: Option<&str>) -> String {
    frame(
        id,
        "unsubscribe",
        SymbolParams {
            kind: "marketPrices",
            symbol,
        },
    )
}

/// `candles` (`candle_update` pushes). `timeframe`: 1m, 3m, 5m, 15m, 30m, 1h, 2h, 4h, 6h, 8h,
/// 12h, 1d, 3d, 1w, 1M, 3M.
pub fn subscribe_candles(id: &str, symbol: &str, timeframe: &str) -> String {
    frame(
        id,
        "subscribe",
        CandleParams {
            kind: "candles",
            symbol,
            timeframe,
        },
    )
}

pub fn unsubscribe_candles(id: &str, symbol: &str, timeframe: &str) -> String {
    frame(
        id,
        "unsubscribe",
        CandleParams {
            kind: "candles",
            symbol,
            timeframe,
        },
    )
}

/// `subAccountUpdates` on the authenticated trade socket: orders, fills, margin, liquidations,
/// funding, delegation events. Sent as `subAccountId` per the docs (the SDK sends
/// `sub_account_id`).
pub fn subscribe_sub_account_updates(id: &str, subaccount_id: u64) -> String {
    frame(
        id,
        "subscribe",
        SubAccountParams {
            kind: "subAccountUpdates",
            sub_account_id: subaccount_id.to_string(),
        },
    )
}

pub fn unsubscribe_sub_account_updates(id: &str, subaccount_id: u64) -> String {
    frame(
        id,
        "unsubscribe",
        SubAccountParams {
            kind: "subAccountUpdates",
            sub_account_id: subaccount_id.to_string(),
        },
    )
}

/// Public market-data action sent as `method: "post"` on the info socket. The reply is a
/// [`SynthetixWsReply`](crate::types::SynthetixWsReply); read the payload with
/// [`SynthetixWsReply::into_post_response`](crate::types::SynthetixWsReply::into_post_response)
/// using the type named on each variant (the same types as the REST actions). Parameters match
/// the REST actions in [`SynthetixMarketApi`](crate::SynthetixMarketApi). `getFundingRateHistory`
/// and `getTiers` are REST-only (the socket answers "Unsupported action").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InfoPost<'a> {
    /// `getMarkets` → `Vec<SynthetixMarket>`.
    GetMarkets { active_only: Option<bool> },
    /// `getMarketPrices` → `HashMap<String, SynthetixMarketPrice>`.
    GetMarketPrices,
    /// `getMids` → `HashMap<String, String>`.
    GetMids,
    /// `getOpenInterest` → `Vec<SynthetixOpenInterest>`.
    GetOpenInterest,
    /// `getOrderbook` → `SynthetixOrderBook`.
    GetOrderbook { symbol: &'a str, limit: Option<u32> },
    /// `getLastTrades` → `SynthetixLastTrades`.
    GetLastTrades { symbol: &'a str, limit: Option<u32> },
    /// `getCandles` → `SynthetixCandles`.
    GetCandles {
        symbol: &'a str,
        interval: &'a str,
        start_time: Option<u64>,
        end_time: Option<u64>,
        limit: Option<u32>,
    },
    /// `getFundingRate` → `SynthetixFundingRate`.
    GetFundingRate { symbol: &'a str },
    /// `getCollaterals` → `Vec<SynthetixCollateralConfig>`. Not listed in the websocket docs;
    /// answered live (2026-11-16).
    GetCollaterals,
    /// `getIsWhitelisted` → `bool`.
    GetIsWhitelisted { wallet_address: &'a str },
    /// `getSubAccountIds` → `Vec<String>`.
    GetSubAccountIds { wallet_address: &'a str },
    /// `getSubAccountIds` with `includeDelegations: true` →
    /// `SynthetixSubAccountIdsWithDelegations`.
    GetSubAccountIdsWithDelegations { wallet_address: &'a str },
}

impl InfoPost<'_> {
    fn params(&self) -> InfoParams {
        match *self {
            InfoPost::GetMarkets { active_only } => {
                InfoParams::new("getMarkets").opt("activeOnly", active_only)
            }
            InfoPost::GetMarketPrices => InfoParams::new("getMarketPrices"),
            InfoPost::GetMids => InfoParams::new("getMids"),
            InfoPost::GetOpenInterest => InfoParams::new("getOpenInterest"),
            InfoPost::GetOrderbook { symbol, limit } => InfoParams::new("getOrderbook")
                .param("symbol", symbol)
                .opt("limit", limit),
            InfoPost::GetLastTrades { symbol, limit } => InfoParams::new("getLastTrades")
                .param("symbol", symbol)
                .opt("limit", limit),
            InfoPost::GetCandles {
                symbol,
                interval,
                start_time,
                end_time,
                limit,
            } => InfoParams::new("getCandles")
                .param("symbol", symbol)
                .param("interval", interval)
                .opt("startTime", start_time)
                .opt("endTime", end_time)
                .opt("limit", limit),
            InfoPost::GetFundingRate { symbol } => {
                InfoParams::new("getFundingRate").param("symbol", symbol)
            }
            InfoPost::GetCollaterals => InfoParams::new("getCollaterals"),
            InfoPost::GetIsWhitelisted { wallet_address } => {
                InfoParams::new("getIsWhitelisted").param("walletAddress", wallet_address)
            }
            InfoPost::GetSubAccountIds { wallet_address } => {
                InfoParams::new("getSubAccountIds").param("walletAddress", wallet_address)
            }
            InfoPost::GetSubAccountIdsWithDelegations { wallet_address } => {
                InfoParams::new("getSubAccountIds")
                    .param("walletAddress", wallet_address)
                    .param("includeDelegations", true)
            }
        }
    }
}

/// Unsigned `post` on the info socket: `{"id","method":"post","params":{"action",…}}`.
pub fn post_info(id: &str, request: &InfoPost) -> String {
    frame(id, "post", request.params().into_map())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames() {
        assert_eq!(ping("ping"), SYNTHETIX_WS_PING);
        assert_eq!(
            subscribe_orderbook("sub-0", "BTC-USDT", BookOptions::default()),
            r#"{"id":"sub-0","method":"subscribe","params":{"type":"orderbook","symbol":"BTC-USDT"}}"#
        );
        assert_eq!(
            subscribe_orderbook(
                "sub-3",
                "ETH-USDT",
                BookOptions {
                    format: Some(BookFormat::Snapshot),
                    depth: Some(10),
                    update_frequency_ms: Some(1000),
                }
            ),
            r#"{"id":"sub-3","method":"subscribe","params":{"type":"orderbook","symbol":"ETH-USDT","format":"snapshot","depth":10,"updateFrequencyMs":1000}}"#
        );
        assert_eq!(
            unsubscribe_orderbook("unsub-0", "BTC-USDT"),
            r#"{"id":"unsub-0","method":"unsubscribe","params":{"type":"orderbook","symbol":"BTC-USDT"}}"#
        );
        assert_eq!(
            subscribe_trades("sub-0", None),
            r#"{"id":"sub-0","method":"subscribe","params":{"type":"trades"}}"#
        );
        assert_eq!(
            subscribe_market_prices("sub-2", Some("BTC-USDT")),
            r#"{"id":"sub-2","method":"subscribe","params":{"type":"marketPrices","symbol":"BTC-USDT"}}"#
        );
        assert_eq!(
            subscribe_candles("sub-1", "BTC-USDT", "1m"),
            r#"{"id":"sub-1","method":"subscribe","params":{"type":"candles","symbol":"BTC-USDT","timeframe":"1m"}}"#
        );
        assert_eq!(
            subscribe_sub_account_updates("s", 1867542890123456789),
            r#"{"id":"s","method":"subscribe","params":{"type":"subAccountUpdates","subAccountId":"1867542890123456789"}}"#
        );
    }

    fn json(s: &str) -> serde_json::Value {
        serde_json::from_str(s).unwrap()
    }

    #[test]
    fn info_post_frames() {
        assert_eq!(
            json(&post_info("p", &InfoPost::GetMids)),
            json(r#"{"id":"p","method":"post","params":{"action":"getMids"}}"#)
        );
        assert_eq!(
            json(&post_info("p", &InfoPost::GetMarkets { active_only: None })),
            json(r#"{"id":"p","method":"post","params":{"action":"getMarkets"}}"#)
        );
        assert_eq!(
            json(&post_info(
                "c",
                &InfoPost::GetCandles {
                    symbol: "BTC-USDT",
                    interval: "1h",
                    start_time: None,
                    end_time: Some(1794826900000),
                    limit: Some(3),
                }
            )),
            json(
                r#"{"id":"c","method":"post","params":{"action":"getCandles","symbol":"BTC-USDT","interval":"1h","endTime":1794826900000,"limit":3}}"#
            )
        );
        assert_eq!(
            json(&post_info(
                "o",
                &InfoPost::GetOrderbook {
                    symbol: "BTC-USDT",
                    limit: Some(5)
                }
            )),
            json(
                r#"{"id":"o","method":"post","params":{"action":"getOrderbook","symbol":"BTC-USDT","limit":5}}"#
            )
        );
        assert_eq!(
            json(&post_info(
                "s",
                &InfoPost::GetSubAccountIdsWithDelegations {
                    wallet_address: "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045"
                }
            )),
            json(
                r#"{"id":"s","method":"post","params":{"action":"getSubAccountIds","walletAddress":"0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045","includeDelegations":true}}"#
            )
        );
    }
}
