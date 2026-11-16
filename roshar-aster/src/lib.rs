/// Wire and message types for this exchange (websocket messages, REST payloads).
pub mod types;

/// Spot (`sapi.asterdex.com` / `sstream.asterdex.com`) public market data.
pub mod spot;

// Endpoints
/// Raw-stream market WebSocket endpoint. The bare host rejects connections
/// (close code 3001 "illegal request"); streams are added with `SUBSCRIBE`.
pub const ASTER_WSS_URL: &str = "wss://fstream.asterdex.com/ws";
/// Combined-stream endpoint. Payloads are wrapped as `{"stream":..,"data":..}`. Streams can
/// be given in the URL (see [`types::combined_stream_url`]) or added with `SUBSCRIBE`.
pub const ASTER_COMBINED_WSS_URL: &str = "wss://fstream.asterdex.com/stream";

pub(crate) mod rest;

use rest::MarketApi;

pub use rest::MarketApi as AsterMarketApi;

use crate::types::{
    get_combined_property_request, list_subscriptions_request, set_combined_property_request,
    AsterAggTrade, AsterBookTicker, AsterExchangeInfo, AsterFundingInfo, AsterFundingRate,
    AsterIndexReferences, AsterKline, AsterMarkPrice, AsterOpenInterest, AsterOrderBook,
    AsterPriceKline, AsterPriceTicker, AsterRemainingOpenableNotional, AsterServerTime,
    AsterTicker24hr, AsterTrade, AsterWssMessage,
};
pub use crate::types::{
    AsterDepthLevels, AsterDepthSpeed, AsterKlineInterval, AsterMarkPriceSpeed, AsterWsMessage,
};
use anyhow::Result;
use async_trait::async_trait;
use roshar_ws_mgr::{Config, Manager, Message};
use std::collections::HashMap;
use std::sync::Arc;

type BoxError = Box<dyn std::error::Error + Send + Sync>;

pub struct AsterClient {
    market_api: MarketApi,
    ws_config: Config,
}

impl Default for AsterClient {
    fn default() -> Self {
        Self::new()
    }
}

impl AsterClient {
    const DEFAULT_REQUESTS_PER_SECOND: u32 = 10;
    const WS_CONN_NAME: &str = "aster";

    pub fn new() -> Self {
        Self::new_with_rate_limit(Self::DEFAULT_REQUESTS_PER_SECOND)
    }

    pub fn new_with_rate_limit(requests_per_second: u32) -> Self {
        Self {
            market_api: MarketApi::new(requests_per_second),
            ws_config: Config {
                name: Self::WS_CONN_NAME.to_string(),
                url: ASTER_WSS_URL.to_string(),
                ping_duration: 10,
                // Sent as a WebSocket ping frame payload (`use_text_ping: false`);
                // the server answers with a pong frame and has no JSON ping method.
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

    /// Switches the WebSocket config to the combined `/stream` endpoint, so every payload
    /// is wrapped as [`AsterWsMessage::Combined`]. The `subscribe_*` helpers work unchanged.
    pub fn with_combined_stream(mut self) -> Self {
        self.ws_config.url = ASTER_COMBINED_WSS_URL.to_string();
        self
    }

    pub fn ws_config(&self) -> roshar_ws_mgr::Config {
        self.ws_config.clone()
    }

    pub fn ws_config_mut(&mut self) -> &mut roshar_ws_mgr::Config {
        &mut self.ws_config
    }

    fn send_text(&self, manager: &Arc<Manager>, text: String) -> Result<()> {
        manager.write(
            &self.ws_config.name,
            Message::TextMessage(self.ws_config.name.clone(), text),
        )
    }

    /// Sends one request for all `symbols`: the server disconnects connections
    /// that send more than 10 messages per second.
    fn send(&self, manager: &Arc<Manager>, msg: AsterWssMessage) -> Result<()> {
        self.send_text(manager, msg.to_json())
    }

    pub fn subscribe_depth(
        &self,
        manager: &Arc<Manager>,
        symbols: &[&str],
        speed: AsterDepthSpeed,
    ) -> Result<()> {
        self.send(manager, AsterWssMessage::depth(symbols, speed))
    }

    pub fn unsubscribe_depth(
        &self,
        manager: &Arc<Manager>,
        symbols: &[&str],
        speed: AsterDepthSpeed,
    ) -> Result<()> {
        self.send(manager, AsterWssMessage::depth_unsub(symbols, speed))
    }

    /// `<symbol>@depth<levels>[@500ms|@100ms]`: top `levels` of each side.
    pub fn subscribe_partial_depth(
        &self,
        manager: &Arc<Manager>,
        symbols: &[&str],
        levels: AsterDepthLevels,
        speed: AsterDepthSpeed,
    ) -> Result<()> {
        self.send(
            manager,
            AsterWssMessage::partial_depth(symbols, levels, speed),
        )
    }

    pub fn unsubscribe_partial_depth(
        &self,
        manager: &Arc<Manager>,
        symbols: &[&str],
        levels: AsterDepthLevels,
        speed: AsterDepthSpeed,
    ) -> Result<()> {
        self.send(
            manager,
            AsterWssMessage::partial_depth_unsub(symbols, levels, speed),
        )
    }

    pub fn subscribe_trades(&self, manager: &Arc<Manager>, symbols: &[&str]) -> Result<()> {
        self.send(manager, AsterWssMessage::trades(symbols))
    }

    pub fn unsubscribe_trades(&self, manager: &Arc<Manager>, symbols: &[&str]) -> Result<()> {
        self.send(manager, AsterWssMessage::trades_unsub(symbols))
    }

    pub fn subscribe_mark_price(
        &self,
        manager: &Arc<Manager>,
        symbols: &[&str],
        speed: AsterMarkPriceSpeed,
    ) -> Result<()> {
        self.send(manager, AsterWssMessage::mark_price(symbols, speed))
    }

    pub fn unsubscribe_mark_price(
        &self,
        manager: &Arc<Manager>,
        symbols: &[&str],
        speed: AsterMarkPriceSpeed,
    ) -> Result<()> {
        self.send(manager, AsterWssMessage::mark_price_unsub(symbols, speed))
    }

    /// `!markPrice@arr[@1s]`.
    pub fn subscribe_all_mark_prices(
        &self,
        manager: &Arc<Manager>,
        speed: AsterMarkPriceSpeed,
    ) -> Result<()> {
        self.send(manager, AsterWssMessage::all_mark_prices(speed))
    }

    pub fn unsubscribe_all_mark_prices(
        &self,
        manager: &Arc<Manager>,
        speed: AsterMarkPriceSpeed,
    ) -> Result<()> {
        self.send(manager, AsterWssMessage::all_mark_prices_unsub(speed))
    }

    pub fn subscribe_klines(
        &self,
        manager: &Arc<Manager>,
        symbols: &[&str],
        interval: AsterKlineInterval,
    ) -> Result<()> {
        self.send(manager, AsterWssMessage::klines(symbols, interval))
    }

    pub fn unsubscribe_klines(
        &self,
        manager: &Arc<Manager>,
        symbols: &[&str],
        interval: AsterKlineInterval,
    ) -> Result<()> {
        self.send(manager, AsterWssMessage::klines_unsub(symbols, interval))
    }

    pub fn subscribe_mini_tickers(&self, manager: &Arc<Manager>, symbols: &[&str]) -> Result<()> {
        self.send(manager, AsterWssMessage::mini_tickers(symbols))
    }

    pub fn unsubscribe_mini_tickers(&self, manager: &Arc<Manager>, symbols: &[&str]) -> Result<()> {
        self.send(manager, AsterWssMessage::mini_tickers_unsub(symbols))
    }

    /// `!miniTicker@arr`: only symbols that changed are in each array.
    pub fn subscribe_all_mini_tickers(&self, manager: &Arc<Manager>) -> Result<()> {
        self.send(manager, AsterWssMessage::all_mini_tickers())
    }

    pub fn unsubscribe_all_mini_tickers(&self, manager: &Arc<Manager>) -> Result<()> {
        self.send(manager, AsterWssMessage::all_mini_tickers_unsub())
    }

    pub fn subscribe_tickers(&self, manager: &Arc<Manager>, symbols: &[&str]) -> Result<()> {
        self.send(manager, AsterWssMessage::tickers(symbols))
    }

    pub fn unsubscribe_tickers(&self, manager: &Arc<Manager>, symbols: &[&str]) -> Result<()> {
        self.send(manager, AsterWssMessage::tickers_unsub(symbols))
    }

    /// `!ticker@arr`: only symbols that changed are in each array.
    pub fn subscribe_all_tickers(&self, manager: &Arc<Manager>) -> Result<()> {
        self.send(manager, AsterWssMessage::all_tickers())
    }

    pub fn unsubscribe_all_tickers(&self, manager: &Arc<Manager>) -> Result<()> {
        self.send(manager, AsterWssMessage::all_tickers_unsub())
    }

    pub fn subscribe_book_tickers(&self, manager: &Arc<Manager>, symbols: &[&str]) -> Result<()> {
        self.send(manager, AsterWssMessage::book_tickers(symbols))
    }

    pub fn unsubscribe_book_tickers(&self, manager: &Arc<Manager>, symbols: &[&str]) -> Result<()> {
        self.send(manager, AsterWssMessage::book_tickers_unsub(symbols))
    }

    /// `!bookTicker`: one event per update for every symbol (thousands per second live).
    pub fn subscribe_all_book_tickers(&self, manager: &Arc<Manager>) -> Result<()> {
        self.send(manager, AsterWssMessage::all_book_tickers())
    }

    pub fn unsubscribe_all_book_tickers(&self, manager: &Arc<Manager>) -> Result<()> {
        self.send(manager, AsterWssMessage::all_book_tickers_unsub())
    }

    /// `<symbol>@forceOrder`: at most one liquidation snapshot per symbol per second.
    pub fn subscribe_liquidations(&self, manager: &Arc<Manager>, symbols: &[&str]) -> Result<()> {
        self.send(manager, AsterWssMessage::liquidations(symbols))
    }

    pub fn unsubscribe_liquidations(&self, manager: &Arc<Manager>, symbols: &[&str]) -> Result<()> {
        self.send(manager, AsterWssMessage::liquidations_unsub(symbols))
    }

    /// `!forceOrder@arr`.
    pub fn subscribe_all_liquidations(&self, manager: &Arc<Manager>) -> Result<()> {
        self.send(manager, AsterWssMessage::all_liquidations())
    }

    pub fn unsubscribe_all_liquidations(&self, manager: &Arc<Manager>) -> Result<()> {
        self.send(manager, AsterWssMessage::all_liquidations_unsub())
    }

    /// `LIST_SUBSCRIPTIONS`; the reply carries `id`.
    pub fn list_subscriptions(&self, manager: &Arc<Manager>, id: u32) -> Result<()> {
        self.send_text(manager, list_subscriptions_request(id))
    }

    /// `SET_PROPERTY combined`; the reply carries `id`.
    pub fn set_combined(&self, manager: &Arc<Manager>, combined: bool, id: u32) -> Result<()> {
        self.send_text(manager, set_combined_property_request(combined, id))
    }

    /// `GET_PROPERTY combined`; the reply carries `id`.
    pub fn get_combined(&self, manager: &Arc<Manager>, id: u32) -> Result<()> {
        self.send_text(manager, get_combined_property_request(id))
    }
}

#[async_trait]
pub trait AsterApi {
    /// `GET /fapi/v3/ping`.
    async fn ping(&self) -> Result<(), BoxError>;

    /// `GET /fapi/v3/time`.
    async fn get_server_time(&self) -> Result<AsterServerTime, BoxError>;

    /// `GET /fapi/v3/exchangeInfo`.
    async fn get_exchange_info(&self) -> Result<AsterExchangeInfo, BoxError>;

    /// Fetch order book snapshot. `None` omits `limit` (exchange default 500); the WS
    /// diff stream needs an initial snapshot from this endpoint to initialise local book state.
    async fn get_orderbook(
        &self,
        symbol: &str,
        limit: Option<u32>,
    ) -> Result<AsterOrderBook, BoxError>;

    /// `GET /fapi/v3/trades`.
    async fn get_recent_trades(
        &self,
        symbol: &str,
        limit: Option<u32>,
    ) -> Result<Vec<AsterTrade>, BoxError>;

    /// `GET /fapi/v3/historicalTrades`.
    async fn get_historical_trades(
        &self,
        symbol: &str,
        limit: Option<u32>,
        from_id: Option<u64>,
    ) -> Result<Vec<AsterTrade>, BoxError>;

    /// `GET /fapi/v3/aggTrades`.
    async fn get_agg_trades(
        &self,
        symbol: &str,
        from_id: Option<u64>,
        start_time: Option<i64>,
        end_time: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<AsterAggTrade>, BoxError>;

    async fn get_klines(
        &self,
        symbol: &str,
        interval: AsterKlineInterval,
        start_time: Option<i64>,
        end_time: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<AsterKline>, BoxError>;

    /// `GET /fapi/v3/indexPriceKlines` (keyed by `pair`).
    async fn get_index_price_klines(
        &self,
        pair: &str,
        interval: AsterKlineInterval,
        start_time: Option<i64>,
        end_time: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<AsterPriceKline>, BoxError>;

    /// `GET /fapi/v3/markPriceKlines`.
    async fn get_mark_price_klines(
        &self,
        symbol: &str,
        interval: AsterKlineInterval,
        start_time: Option<i64>,
        end_time: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<AsterPriceKline>, BoxError>;

    /// `GET /fapi/v3/premiumIndexKlines` (live, not in the V3 docs).
    async fn get_premium_index_klines(
        &self,
        symbol: &str,
        interval: AsterKlineInterval,
        start_time: Option<i64>,
        end_time: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<AsterPriceKline>, BoxError>;

    async fn get_mark_prices(&self) -> Result<Vec<AsterMarkPrice>, BoxError>;

    /// `GET /fapi/v3/premiumIndex?symbol=...`.
    async fn get_mark_price(&self, symbol: &str) -> Result<AsterMarkPrice, BoxError>;

    /// `GET /fapi/v3/fundingInfo`; `None` returns every symbol.
    async fn get_funding_info(
        &self,
        symbol: Option<&str>,
    ) -> Result<Vec<AsterFundingInfo>, BoxError>;

    async fn get_tickers(&self) -> Result<HashMap<String, AsterTicker24hr>, BoxError>;

    /// `GET /fapi/v3/ticker/24hr?symbol=...`.
    async fn get_ticker(&self, symbol: &str) -> Result<AsterTicker24hr, BoxError>;

    /// `GET /fapi/v3/ticker/price` (all symbols).
    async fn get_price_tickers(&self) -> Result<Vec<AsterPriceTicker>, BoxError>;

    /// `GET /fapi/v3/ticker/price?symbol=...`.
    async fn get_price_ticker(&self, symbol: &str) -> Result<AsterPriceTicker, BoxError>;

    /// `GET /fapi/v3/ticker/bookTicker` (all symbols).
    async fn get_book_tickers(&self) -> Result<Vec<AsterBookTicker>, BoxError>;

    /// `GET /fapi/v3/ticker/bookTicker?symbol=...`.
    async fn get_book_ticker(&self, symbol: &str) -> Result<AsterBookTicker, BoxError>;

    /// `GET /fapi/v3/indexreferences`.
    async fn get_index_references(&self, symbol: &str) -> Result<AsterIndexReferences, BoxError>;

    /// `GET /fapi/v3/remainingOpenableNotionalValue`.
    async fn get_remaining_openable_notional(
        &self,
        symbol: &str,
        leverage: u32,
    ) -> Result<AsterRemainingOpenableNotional, BoxError>;

    /// `GET /fapi/v3/openInterest` (live, not in the V3 docs).
    async fn get_open_interest(&self, symbol: &str) -> Result<AsterOpenInterest, BoxError>;

    async fn get_historical_funding_rates(
        &self,
        symbol: &str,
        start_time: i64,
        end_time: i64,
    ) -> Result<Vec<AsterFundingRate>, BoxError>;
}

#[async_trait]
impl AsterApi for AsterClient {
    async fn ping(&self) -> Result<(), BoxError> {
        self.market_api.ping().await
    }

    async fn get_server_time(&self) -> Result<AsterServerTime, BoxError> {
        self.market_api.get_server_time().await
    }

    async fn get_exchange_info(&self) -> Result<AsterExchangeInfo, BoxError> {
        self.market_api.get_exchange_info().await
    }

    async fn get_orderbook(
        &self,
        symbol: &str,
        limit: Option<u32>,
    ) -> Result<AsterOrderBook, BoxError> {
        self.market_api.get_orderbook(symbol, limit).await
    }

    async fn get_recent_trades(
        &self,
        symbol: &str,
        limit: Option<u32>,
    ) -> Result<Vec<AsterTrade>, BoxError> {
        self.market_api.get_recent_trades(symbol, limit).await
    }

    async fn get_historical_trades(
        &self,
        symbol: &str,
        limit: Option<u32>,
        from_id: Option<u64>,
    ) -> Result<Vec<AsterTrade>, BoxError> {
        self.market_api
            .get_historical_trades(symbol, limit, from_id)
            .await
    }

    async fn get_agg_trades(
        &self,
        symbol: &str,
        from_id: Option<u64>,
        start_time: Option<i64>,
        end_time: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<AsterAggTrade>, BoxError> {
        self.market_api
            .get_agg_trades(symbol, from_id, start_time, end_time, limit)
            .await
    }

    async fn get_klines(
        &self,
        symbol: &str,
        interval: AsterKlineInterval,
        start_time: Option<i64>,
        end_time: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<AsterKline>, BoxError> {
        self.market_api
            .get_klines(symbol, interval, start_time, end_time, limit)
            .await
    }

    async fn get_index_price_klines(
        &self,
        pair: &str,
        interval: AsterKlineInterval,
        start_time: Option<i64>,
        end_time: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<AsterPriceKline>, BoxError> {
        self.market_api
            .get_index_price_klines(pair, interval, start_time, end_time, limit)
            .await
    }

    async fn get_mark_price_klines(
        &self,
        symbol: &str,
        interval: AsterKlineInterval,
        start_time: Option<i64>,
        end_time: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<AsterPriceKline>, BoxError> {
        self.market_api
            .get_mark_price_klines(symbol, interval, start_time, end_time, limit)
            .await
    }

    async fn get_premium_index_klines(
        &self,
        symbol: &str,
        interval: AsterKlineInterval,
        start_time: Option<i64>,
        end_time: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<AsterPriceKline>, BoxError> {
        self.market_api
            .get_premium_index_klines(symbol, interval, start_time, end_time, limit)
            .await
    }

    async fn get_mark_prices(&self) -> Result<Vec<AsterMarkPrice>, BoxError> {
        self.market_api.get_mark_prices().await
    }

    async fn get_mark_price(&self, symbol: &str) -> Result<AsterMarkPrice, BoxError> {
        self.market_api.get_mark_price(symbol).await
    }

    async fn get_funding_info(
        &self,
        symbol: Option<&str>,
    ) -> Result<Vec<AsterFundingInfo>, BoxError> {
        self.market_api.get_funding_info(symbol).await
    }

    async fn get_tickers(&self) -> Result<HashMap<String, AsterTicker24hr>, BoxError> {
        self.market_api.get_tickers().await
    }

    async fn get_ticker(&self, symbol: &str) -> Result<AsterTicker24hr, BoxError> {
        self.market_api.get_ticker(symbol).await
    }

    async fn get_price_tickers(&self) -> Result<Vec<AsterPriceTicker>, BoxError> {
        self.market_api.get_price_tickers().await
    }

    async fn get_price_ticker(&self, symbol: &str) -> Result<AsterPriceTicker, BoxError> {
        self.market_api.get_price_ticker(symbol).await
    }

    async fn get_book_tickers(&self) -> Result<Vec<AsterBookTicker>, BoxError> {
        self.market_api.get_book_tickers().await
    }

    async fn get_book_ticker(&self, symbol: &str) -> Result<AsterBookTicker, BoxError> {
        self.market_api.get_book_ticker(symbol).await
    }

    async fn get_index_references(&self, symbol: &str) -> Result<AsterIndexReferences, BoxError> {
        self.market_api.get_index_references(symbol).await
    }

    async fn get_remaining_openable_notional(
        &self,
        symbol: &str,
        leverage: u32,
    ) -> Result<AsterRemainingOpenableNotional, BoxError> {
        self.market_api
            .get_remaining_openable_notional(symbol, leverage)
            .await
    }

    async fn get_open_interest(&self, symbol: &str) -> Result<AsterOpenInterest, BoxError> {
        self.market_api.get_open_interest(symbol).await
    }

    async fn get_historical_funding_rates(
        &self,
        symbol: &str,
        start_time: i64,
        end_time: i64,
    ) -> Result<Vec<AsterFundingRate>, BoxError> {
        self.market_api
            .get_historical_funding_rates(symbol, start_time, end_time)
            .await
    }
}
