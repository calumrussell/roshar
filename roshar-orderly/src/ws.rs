//! Websocket frames. Requests are `{"id": <client id>, "event": "subscribe"|"unsubscribe",
//! "topic": <topic>}`; the server echoes `id` in its reply. The caller picks ids.

use crate::wire::PyObject;

/// Mainnet public stream; append `/{account_id}`.
pub const ORDERLY_WSS_URL: &str = "wss://ws-evm.orderly.org/ws/stream";
/// Testnet public stream; append `/{account_id}`.
pub const ORDERLY_TESTNET_WSS_URL: &str = "wss://testnet-ws-evm.orderly.org/ws/stream";
/// Mainnet private stream; append `/{account_id}`, then send `OrderlyKey::ws_auth_message`.
pub const ORDERLY_PRIVATE_WSS_URL: &str = "wss://ws-private-evm.orderly.org/v2/ws/private/stream";
/// Testnet private stream; append `/{account_id}`.
pub const ORDERLY_TESTNET_PRIVATE_WSS_URL: &str =
    "wss://testnet-ws-private-evm.orderly.org/v2/ws/private/stream";

/// Client keepalive frame (the server answers `{"event":"pong","ts":..}`).
pub const PING: &str = r#"{"event":"ping"}"#;
/// Reply to the server's `{"event":"ping"}` (sent every 10 s; ten missed pongs disconnect).
pub const PONG: &str = r#"{"event":"pong"}"#;

/// `{base}/{account_id}`. The public stream also takes the account id in its path and closes
/// connections whose id it does not recognise.
pub fn stream_url(base: &str, account_id: &str) -> String {
    format!("{base}/{account_id}")
}

/// Public topics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderlyTopic<'a> {
    /// `{symbol}@orderbook`: 100 levels every 1 s.
    Orderbook(&'a str),
    /// `{symbol}@orderbookupdate`: incremental, every 200 ms.
    OrderbookUpdate(&'a str),
    /// `{symbol}@trade`
    Trade(&'a str),
    /// `{symbol}@ticker`: rolling 24 h.
    Ticker(&'a str),
    /// `tickers`: every symbol.
    Tickers,
    /// `{symbol}@bbo`: every 10 ms.
    Bbo(&'a str),
    /// `bbos`: every symbol, every 1 s.
    Bbos,
    /// `{symbol}@kline_{interval}`; interval `1m`, `5m`, `15m`, `30m`, `1h`, `4h`, `12h`, `1d`,
    /// `1w`, `1mon`.
    Kline { symbol: &'a str, interval: &'a str },
    /// `{symbol}@estfundingrate`: every 15 s.
    EstFundingRate(&'a str),
    /// `{symbol}@markprice`
    MarkPrice(&'a str),
    /// `markprices`
    MarkPrices,
    /// `{symbol}@indexprice`, e.g. `SPOT_ETH_USDC@indexprice`.
    IndexPrice(&'a str),
    /// `indexprices`
    IndexPrices,
    /// `{symbol}@openinterest`
    OpenInterest(&'a str),
    /// `{broker_id}${symbol}@ticker`: 24 h ticker of one builder's flow, every 1 s.
    BuilderTicker { broker_id: &'a str, symbol: &'a str },
    /// `{broker_id}$tickers`: every symbol, one builder's flow, every 1 s.
    BuilderTickers(&'a str),
    /// `price_changes`: price-change windows for every symbol, every 1 min.
    PriceChanges,
    /// `traders_open_interests`: long/short OI per symbol, every 1 min. (The docs' parameter
    /// table says `traders_open_interest`; the server rejects that as an invalid topic.)
    TradersOpenInterests,
    /// `history_charts_{interval}`: close-price series for every symbol, every 1 min. Live,
    /// `1h` is accepted and `1m` is rejected as an invalid topic.
    HistoryCharts(&'a str),
    /// `liquidation`: positions up for liquidation, pushed on change.
    Liquidation,
    /// `maintenance_status`: system status, pushed on change.
    MaintenanceStatus,
}

impl OrderlyTopic<'_> {
    pub fn name(&self) -> String {
        match self {
            Self::Orderbook(s) => format!("{s}@orderbook"),
            Self::OrderbookUpdate(s) => format!("{s}@orderbookupdate"),
            Self::Trade(s) => format!("{s}@trade"),
            Self::Ticker(s) => format!("{s}@ticker"),
            Self::Tickers => "tickers".to_string(),
            Self::Bbo(s) => format!("{s}@bbo"),
            Self::Bbos => "bbos".to_string(),
            Self::Kline { symbol, interval } => format!("{symbol}@kline_{interval}"),
            Self::EstFundingRate(s) => format!("{s}@estfundingrate"),
            Self::MarkPrice(s) => format!("{s}@markprice"),
            Self::MarkPrices => "markprices".to_string(),
            Self::IndexPrice(s) => format!("{s}@indexprice"),
            Self::IndexPrices => "indexprices".to_string(),
            Self::OpenInterest(s) => format!("{s}@openinterest"),
            Self::BuilderTicker { broker_id, symbol } => format!("{broker_id}${symbol}@ticker"),
            Self::BuilderTickers(b) => format!("{b}$tickers"),
            Self::PriceChanges => "price_changes".to_string(),
            Self::TradersOpenInterests => "traders_open_interests".to_string(),
            Self::HistoryCharts(i) => format!("history_charts_{i}"),
            Self::Liquidation => "liquidation".to_string(),
            Self::MaintenanceStatus => "maintenance_status".to_string(),
        }
    }
}

/// Private topics (after `auth`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderlyPrivateTopic<'a> {
    /// `executionreport`: order updates and fills. `Some("PERP_A_USDC,PERP_B_USDC")` limits it to
    /// those symbols; `None` is every symbol.
    ExecutionReport(Option<&'a str>),
    /// `position`
    Position,
    /// `balance`
    Balance,
    /// `account`
    Account,
}

impl OrderlyPrivateTopic<'_> {
    pub fn name(&self) -> &'static str {
        match self {
            Self::ExecutionReport(_) => "executionreport",
            Self::Position => "position",
            Self::Balance => "balance",
            Self::Account => "account",
        }
    }
}

fn topic_frame(id: &str, event: &str, topic: &str) -> String {
    let mut o = PyObject::new();
    o.str("id", id).str("event", event).str("topic", topic);
    o.finish()
}

fn private_frame(id: &str, event: &str, topic: &OrderlyPrivateTopic<'_>) -> String {
    let mut o = PyObject::new();
    o.str("id", id)
        .str("event", event)
        .str("topic", topic.name());
    if let OrderlyPrivateTopic::ExecutionReport(Some(symbols)) = topic {
        let mut params = PyObject::new();
        params.str("symbol", symbols);
        o.raw("params", &params.finish());
    }
    o.finish()
}

pub fn subscribe(id: &str, topic: &OrderlyTopic<'_>) -> String {
    topic_frame(id, "subscribe", &topic.name())
}

pub fn unsubscribe(id: &str, topic: &OrderlyTopic<'_>) -> String {
    topic_frame(id, "unsubscribe", &topic.name())
}

pub fn subscribe_private(id: &str, topic: &OrderlyPrivateTopic<'_>) -> String {
    private_frame(id, "subscribe", topic)
}

pub fn unsubscribe_private(id: &str, topic: &OrderlyPrivateTopic<'_>) -> String {
    private_frame(id, "unsubscribe", topic)
}

/// One-off full order book: `{"id", "event": "request", "params": {"type": "orderbook", "symbol"}}`.
pub fn request_orderbook(id: &str, symbol: &str) -> String {
    let mut params = PyObject::new();
    params.str("type", "orderbook").str("symbol", symbol);
    let mut o = PyObject::new();
    o.str("id", id)
        .str("event", "request")
        .raw("params", &params.finish());
    o.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames() {
        assert_eq!(
            subscribe(
                "c1",
                &OrderlyTopic::Kline {
                    symbol: "PERP_BTC_USDC",
                    interval: "1m"
                }
            ),
            r#"{"id": "c1", "event": "subscribe", "topic": "PERP_BTC_USDC@kline_1m"}"#
        );
        assert_eq!(
            unsubscribe("c2", &OrderlyTopic::Bbos),
            r#"{"id": "c2", "event": "unsubscribe", "topic": "bbos"}"#
        );
        for (topic, name) in [
            (
                OrderlyTopic::BuilderTicker {
                    broker_id: "woofi_pro",
                    symbol: "PERP_ETH_USDC",
                },
                "woofi_pro$PERP_ETH_USDC@ticker",
            ),
            (
                OrderlyTopic::BuilderTickers("woofi_pro"),
                "woofi_pro$tickers",
            ),
            (OrderlyTopic::PriceChanges, "price_changes"),
            (OrderlyTopic::TradersOpenInterests, "traders_open_interests"),
            (OrderlyTopic::HistoryCharts("1h"), "history_charts_1h"),
            (OrderlyTopic::Liquidation, "liquidation"),
            (OrderlyTopic::MaintenanceStatus, "maintenance_status"),
        ] {
            assert_eq!(topic.name(), name);
        }
        assert_eq!(
            subscribe_private(
                "p",
                &OrderlyPrivateTopic::ExecutionReport(Some("PERP_BTC_USDC"))
            ),
            r#"{"id": "p", "event": "subscribe", "topic": "executionreport", "params": {"symbol": "PERP_BTC_USDC"}}"#
        );
        assert_eq!(
            request_orderbook("r", "PERP_ETH_USDC"),
            r#"{"id": "r", "event": "request", "params": {"type": "orderbook", "symbol": "PERP_ETH_USDC"}}"#
        );
    }
}
