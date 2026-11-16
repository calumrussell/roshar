//! Aster spot public market data: REST `https://sapi.asterdex.com/api/v3/*` and
//! market streams on `wss://sstream.asterdex.com`.

pub mod types;

pub(crate) mod rest;

pub use types::*;

use crate::types::AsterKlineInterval;
use anyhow::Result;
use rest::MarketApi;
use roshar_ws_mgr::{Config, Manager, Message};
use std::sync::Arc;

/// Raw-stream spot market WebSocket endpoint; streams are added with `SUBSCRIBE`.
pub const ASTER_SPOT_WSS_URL: &str = "wss://sstream.asterdex.com/ws";

/// Combined-stream URL `wss://sstream.asterdex.com/stream?streams=a/b`. Frames
/// on it are `{"stream":..,"data":..}`; parse with [`AsterSpotWsCombinedMessage::parse`].
pub fn combined_stream_url(streams: &[&str]) -> String {
    format!(
        "wss://sstream.asterdex.com/stream?streams={}",
        streams.join("/")
    )
}

type BoxError = Box<dyn std::error::Error + Send + Sync>;

pub struct AsterSpotClient {
    market_api: MarketApi,
    ws_config: Config,
}

impl Default for AsterSpotClient {
    fn default() -> Self {
        Self::new()
    }
}

impl AsterSpotClient {
    const DEFAULT_REQUESTS_PER_SECOND: u32 = 10;
    const WS_CONN_NAME: &str = "aster-spot";

    pub fn new() -> Self {
        Self::new_with_rate_limit(Self::DEFAULT_REQUESTS_PER_SECOND)
    }

    pub fn new_with_rate_limit(requests_per_second: u32) -> Self {
        Self {
            market_api: MarketApi::new(requests_per_second),
            ws_config: Config {
                name: Self::WS_CONN_NAME.to_string(),
                url: ASTER_SPOT_WSS_URL.to_string(),
                ping_duration: 10,
                // Sent as a WebSocket ping frame payload (`use_text_ping: false`).
                // The docs also accept an application-level `{"method":"PONG"}`.
                ping_message: "ping".to_string(),
                ping_timeout: 10,
                reconnect_timeout: 5000,
                use_text_ping: Some(false),
                read_buffer_size: None,
                write_buffer_size: None,
                max_message_size: None,
                max_frame_size: None,
                tcp_recv_buffer_size: None,
                tcp_send_buffer_size: None,
                tcp_nodelay: Some(true),
                broadcast_channel_size: None,
                headers: vec![],
            },
        }
    }

    pub fn ws_config(&self) -> roshar_ws_mgr::Config {
        self.ws_config.clone()
    }

    pub fn ws_config_mut(&mut self) -> &mut roshar_ws_mgr::Config {
        &mut self.ws_config
    }

    // ── REST ──

    pub async fn ping(&self) -> Result<AsterSpotPing, BoxError> {
        self.market_api.ping().await
    }

    pub async fn get_server_time(&self) -> Result<AsterSpotServerTime, BoxError> {
        self.market_api.get_server_time().await
    }

    pub async fn get_exchange_info(&self) -> Result<AsterSpotExchangeInfo, BoxError> {
        self.market_api.get_exchange_info().await
    }

    pub async fn get_orderbook(
        &self,
        symbol: &str,
        limit: Option<u32>,
    ) -> Result<AsterSpotOrderBook, BoxError> {
        self.market_api.get_orderbook(symbol, limit).await
    }

    pub async fn get_trades(
        &self,
        symbol: &str,
        limit: Option<u32>,
    ) -> Result<Vec<AsterSpotTrade>, BoxError> {
        self.market_api.get_trades(symbol, limit).await
    }

    pub async fn get_historical_trades(
        &self,
        symbol: &str,
        limit: Option<u32>,
        from_id: Option<u64>,
    ) -> Result<Vec<AsterSpotTrade>, BoxError> {
        self.market_api
            .get_historical_trades(symbol, limit, from_id)
            .await
    }

    pub async fn get_agg_trades(
        &self,
        symbol: &str,
        from_id: Option<u64>,
        start_time: Option<i64>,
        end_time: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<AsterSpotAggTrade>, BoxError> {
        self.market_api
            .get_agg_trades(symbol, from_id, start_time, end_time, limit)
            .await
    }

    pub async fn get_klines(
        &self,
        symbol: &str,
        interval: AsterKlineInterval,
        start_time: Option<i64>,
        end_time: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<AsterSpotKline>, BoxError> {
        self.market_api
            .get_klines(symbol, interval, start_time, end_time, limit)
            .await
    }

    pub async fn get_ticker_24hr(&self, symbol: &str) -> Result<AsterSpotTicker24hr, BoxError> {
        self.market_api.get_ticker_24hr(symbol).await
    }

    pub async fn get_tickers_24hr(&self) -> Result<Vec<AsterSpotTicker24hr>, BoxError> {
        self.market_api.get_tickers_24hr().await
    }

    pub async fn get_price_ticker(&self, symbol: &str) -> Result<AsterSpotPriceTicker, BoxError> {
        self.market_api.get_price_ticker(symbol).await
    }

    pub async fn get_price_tickers(&self) -> Result<Vec<AsterSpotPriceTicker>, BoxError> {
        self.market_api.get_price_tickers().await
    }

    pub async fn get_book_ticker(&self, symbol: &str) -> Result<AsterSpotBookTicker, BoxError> {
        self.market_api.get_book_ticker(symbol).await
    }

    pub async fn get_book_tickers(&self) -> Result<Vec<AsterSpotBookTicker>, BoxError> {
        self.market_api.get_book_tickers().await
    }

    // ── WebSocket ──

    /// Sends one request carrying every stream.
    fn send(&self, manager: &Arc<Manager>, msg: AsterSpotWsRequest) -> Result<()> {
        self.send_text(manager, msg.to_json())
    }

    fn sub(&self, manager: &Arc<Manager>, streams: Vec<String>) -> Result<()> {
        self.send(manager, AsterSpotWsRequest::subscribe(streams))
    }

    fn unsub(&self, manager: &Arc<Manager>, streams: Vec<String>) -> Result<()> {
        self.send(manager, AsterSpotWsRequest::unsubscribe(streams))
    }

    fn send_text(&self, manager: &Arc<Manager>, text: String) -> Result<()> {
        manager.write(
            &self.ws_config.name,
            Message::TextMessage(self.ws_config.name.clone(), text),
        )
    }

    /// `LIST_SUBSCRIPTIONS`; the reply carries `id`.
    pub fn list_subscriptions(&self, manager: &Arc<Manager>, id: u32) -> Result<()> {
        self.send_text(manager, crate::types::list_subscriptions_request(id))
    }

    /// `SET_PROPERTY combined`; the reply carries `id`.
    pub fn set_combined(&self, manager: &Arc<Manager>, combined: bool, id: u32) -> Result<()> {
        self.send_text(
            manager,
            crate::types::set_combined_property_request(combined, id),
        )
    }

    /// `GET_PROPERTY combined`; the reply carries `id`.
    pub fn get_combined(&self, manager: &Arc<Manager>, id: u32) -> Result<()> {
        self.send_text(manager, crate::types::get_combined_property_request(id))
    }

    fn each(symbols: &[&str], f: impl Fn(&str) -> String) -> Vec<String> {
        symbols.iter().map(|s| f(s)).collect()
    }

    pub fn subscribe_agg_trades(&self, manager: &Arc<Manager>, symbols: &[&str]) -> Result<()> {
        self.sub(manager, Self::each(symbols, streams::agg_trade))
    }

    pub fn unsubscribe_agg_trades(&self, manager: &Arc<Manager>, symbols: &[&str]) -> Result<()> {
        self.unsub(manager, Self::each(symbols, streams::agg_trade))
    }

    pub fn subscribe_trades(&self, manager: &Arc<Manager>, symbols: &[&str]) -> Result<()> {
        self.sub(manager, Self::each(symbols, streams::trade))
    }

    pub fn unsubscribe_trades(&self, manager: &Arc<Manager>, symbols: &[&str]) -> Result<()> {
        self.unsub(manager, Self::each(symbols, streams::trade))
    }

    pub fn subscribe_klines(
        &self,
        manager: &Arc<Manager>,
        symbols: &[&str],
        interval: AsterKlineInterval,
    ) -> Result<()> {
        self.sub(
            manager,
            Self::each(symbols, |s| streams::kline(s, interval)),
        )
    }

    pub fn unsubscribe_klines(
        &self,
        manager: &Arc<Manager>,
        symbols: &[&str],
        interval: AsterKlineInterval,
    ) -> Result<()> {
        self.unsub(
            manager,
            Self::each(symbols, |s| streams::kline(s, interval)),
        )
    }

    pub fn subscribe_mini_tickers(&self, manager: &Arc<Manager>, symbols: &[&str]) -> Result<()> {
        self.sub(manager, Self::each(symbols, streams::mini_ticker))
    }

    pub fn unsubscribe_mini_tickers(&self, manager: &Arc<Manager>, symbols: &[&str]) -> Result<()> {
        self.unsub(manager, Self::each(symbols, streams::mini_ticker))
    }

    /// `!miniTicker@arr`
    pub fn subscribe_all_mini_tickers(&self, manager: &Arc<Manager>) -> Result<()> {
        self.sub(manager, vec![streams::ALL_MINI_TICKERS.to_string()])
    }

    pub fn unsubscribe_all_mini_tickers(&self, manager: &Arc<Manager>) -> Result<()> {
        self.unsub(manager, vec![streams::ALL_MINI_TICKERS.to_string()])
    }

    pub fn subscribe_tickers(&self, manager: &Arc<Manager>, symbols: &[&str]) -> Result<()> {
        self.sub(manager, Self::each(symbols, streams::ticker))
    }

    pub fn unsubscribe_tickers(&self, manager: &Arc<Manager>, symbols: &[&str]) -> Result<()> {
        self.unsub(manager, Self::each(symbols, streams::ticker))
    }

    /// `!ticker@arr`
    pub fn subscribe_all_tickers(&self, manager: &Arc<Manager>) -> Result<()> {
        self.sub(manager, vec![streams::ALL_TICKERS.to_string()])
    }

    pub fn unsubscribe_all_tickers(&self, manager: &Arc<Manager>) -> Result<()> {
        self.unsub(manager, vec![streams::ALL_TICKERS.to_string()])
    }

    pub fn subscribe_book_tickers(&self, manager: &Arc<Manager>, symbols: &[&str]) -> Result<()> {
        self.sub(manager, Self::each(symbols, streams::book_ticker))
    }

    pub fn unsubscribe_book_tickers(&self, manager: &Arc<Manager>, symbols: &[&str]) -> Result<()> {
        self.unsub(manager, Self::each(symbols, streams::book_ticker))
    }

    /// `!bookTicker`
    pub fn subscribe_all_book_tickers(&self, manager: &Arc<Manager>) -> Result<()> {
        self.sub(manager, vec![streams::ALL_BOOK_TICKERS.to_string()])
    }

    pub fn unsubscribe_all_book_tickers(&self, manager: &Arc<Manager>) -> Result<()> {
        self.unsub(manager, vec![streams::ALL_BOOK_TICKERS.to_string()])
    }

    /// `<symbol>@depth<levels>[@100ms]`
    pub fn subscribe_partial_depth(
        &self,
        manager: &Arc<Manager>,
        symbols: &[&str],
        levels: AsterSpotDepthLevels,
        speed: AsterSpotDepthSpeed,
    ) -> Result<()> {
        self.sub(
            manager,
            Self::each(symbols, |s| streams::partial_depth(s, levels, speed)),
        )
    }

    pub fn unsubscribe_partial_depth(
        &self,
        manager: &Arc<Manager>,
        symbols: &[&str],
        levels: AsterSpotDepthLevels,
        speed: AsterSpotDepthSpeed,
    ) -> Result<()> {
        self.unsub(
            manager,
            Self::each(symbols, |s| streams::partial_depth(s, levels, speed)),
        )
    }

    /// `<symbol>@depth[@100ms]` diff stream; seed local books from [`Self::get_orderbook`].
    pub fn subscribe_depth(
        &self,
        manager: &Arc<Manager>,
        symbols: &[&str],
        speed: AsterSpotDepthSpeed,
    ) -> Result<()> {
        self.sub(
            manager,
            Self::each(symbols, |s| streams::diff_depth(s, speed)),
        )
    }

    pub fn unsubscribe_depth(
        &self,
        manager: &Arc<Manager>,
        symbols: &[&str],
        speed: AsterSpotDepthSpeed,
    ) -> Result<()> {
        self.unsub(
            manager,
            Self::each(symbols, |s| streams::diff_depth(s, speed)),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combined_url() {
        assert_eq!(
            combined_stream_url(&["btcusdt@aggTrade", "!miniTicker@arr"]),
            "wss://sstream.asterdex.com/stream?streams=btcusdt@aggTrade/!miniTicker@arr"
        );
    }

    #[test]
    fn ws_config_has_spot_url() {
        let c = AsterSpotClient::new();
        assert_eq!(c.ws_config().url, ASTER_SPOT_WSS_URL);
        assert!(c.ws_config().headers.is_empty());
    }
}
