/// Wire and message types for this exchange (websocket messages, REST payloads).
pub mod types;

// Endpoints
/// Classic Futures WebSocket host. Connecting needs `?token=` from bullet-public; use
/// `KuCoinClient::get_ws_url`, which returns the server-assigned endpoint with a token.
pub const KUCOIN_FUTURES_WSS_URL: &str = "wss://ws-api-futures.kucoin.com/";
pub const KUCOIN_FUTURES_REST_URL: &str = "https://api-futures.kucoin.com";
/// UTA REST host; used only for open interest, which Classic Futures does not serve.
pub const KUCOIN_UTA_REST_URL: &str = "https://api.kucoin.com";

pub(crate) mod rest;

pub use rest::MarketApi as KuCoinMarketApi;
pub use types::{
    KuCoinContract, KuCoinCurrentFundingRate, KuCoinFundingRate, KuCoinIndexPage, KuCoinIndexQuery,
    KuCoinIndexValue, KuCoinKline, KuCoinMarkPrice, KuCoinOpenInterest, KuCoinOpenInterestPoint,
    KuCoinOrderBook, KuCoinServiceStatus, KuCoinSpotIndex, KuCoinTicker, KuCoinTrade,
    KuCoinWsTokenData,
};

use crate::types::{
    kucoin_announcement_topic, kucoin_candle_topic, KuCoinChannel, KuCoinWsInstanceServer,
    KuCoinWssMessage,
};
use anyhow::Result;
use async_trait::async_trait;
use roshar_ws_mgr::{Config, Manager, Message};
use std::collections::HashMap;
use std::sync::Arc;

pub struct KuCoinClient {
    market_api: rest::MarketApi,
    ws_config: Config,
}

impl KuCoinClient {
    /// The Public rate-limit pool is 2000 weight per 30 s per IP (~66 weight/s). The
    /// heaviest endpoints (bullet-public, open interest) weigh 10, so 6 req/s stays inside
    /// the pool whatever mix of endpoints is called.
    const DEFAULT_REQUESTS_PER_SECOND: u32 = 6;
    const WS_CONN_NAME: &str = "kucoin";

    pub fn new() -> Self {
        Self::new_with_rate_limit(Self::DEFAULT_REQUESTS_PER_SECOND)
    }

    pub fn new_with_rate_limit(requests_per_second: u32) -> Self {
        Self {
            market_api: rest::MarketApi::new(requests_per_second),
            ws_config: Config {
                name: Self::WS_CONN_NAME.to_string(),
                // Untokenised host; set `ws_config_mut().url = get_ws_url().await?`
                // before connecting.
                url: KUCOIN_FUTURES_WSS_URL.to_string(),
                // bullet-public returns pingInterval 18000 ms and pingTimeout 10000 ms.
                ping_duration: 18,
                ping_message: KuCoinWssMessage::ping().to_json(),
                ping_timeout: 10,
                reconnect_timeout: 5000,
                use_text_ping: Some(true),
                read_buffer_size: None,
                write_buffer_size: None,
                max_message_size: None,
                max_frame_size: None,
                tcp_recv_buffer_size: None,
                tcp_send_buffer_size: None,
                tcp_nodelay: None,
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

    /// Obtain a fresh public WebSocket token and return the full WSS URL.
    ///
    /// Calls POST /api/v1/bullet-public and returns `<endpoint>?token=<token>`. Set it
    /// with `ws_config_mut().url = ...` before `Manager::new_conn`. The token is valid for
    /// 24 hours and the server drops the connection after that, so fetch a new URL and
    /// recreate the connection rather than reconnecting with the old one.
    pub async fn get_ws_url(&self) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api.get_ws_url().await
    }

    /// Fetch a public token: the token plus every instance server with its
    /// `pingInterval`/`pingTimeout`. Pair with `apply_ws_server` to use the server values.
    pub async fn get_ws_token(
        &self,
    ) -> Result<KuCoinWsTokenData, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api.get_ws_token().await
    }

    /// Set `ws_config` from a bullet-public instance server: `url` becomes
    /// `<endpoint>?token=<token>`, `ping_duration`/`ping_timeout` become the server's
    /// `pingInterval`/`pingTimeout` (milliseconds, rounded down to whole seconds, min 1).
    /// Nothing else changes.
    pub fn apply_ws_server(&mut self, token: &str, server: &KuCoinWsInstanceServer) {
        self.ws_config.url = format!("{}?token={}", server.endpoint, token);
        self.ws_config.ping_duration = (server.ping_interval / 1000).max(1);
        self.ws_config.ping_timeout = (server.ping_timeout / 1000).max(1);
    }

    fn send(&self, manager: &Arc<Manager>, msg: KuCoinWssMessage) -> Result<()> {
        manager.write(
            &self.ws_config.name,
            Message::TextMessage(self.ws_config.name.clone(), msg.to_json()),
        )
    }

    /// Subscribe `channel` for `symbols` in one request (comma-separated topic; the
    /// exchange accepts up to 100 symbols per request).
    pub fn subscribe(
        &self,
        manager: &Arc<Manager>,
        channel: KuCoinChannel,
        symbols: &[&str],
    ) -> Result<()> {
        self.send(manager, KuCoinWssMessage::subscribe(channel.topic(symbols)))
    }

    pub fn unsubscribe(
        &self,
        manager: &Arc<Manager>,
        channel: KuCoinChannel,
        symbols: &[&str],
    ) -> Result<()> {
        self.send(
            manager,
            KuCoinWssMessage::unsubscribe(channel.topic(symbols)),
        )
    }

    /// `/contractMarket/level2:{symbols}` (order-book increments).
    pub fn subscribe_depth(&self, manager: &Arc<Manager>, symbols: &[&str]) -> Result<()> {
        self.subscribe(manager, KuCoinChannel::Level2, symbols)
    }

    pub fn unsubscribe_depth(&self, manager: &Arc<Manager>, symbols: &[&str]) -> Result<()> {
        self.unsubscribe(manager, KuCoinChannel::Level2, symbols)
    }

    /// `/contractMarket/execution:{symbols}`.
    pub fn subscribe_trades(&self, manager: &Arc<Manager>, symbols: &[&str]) -> Result<()> {
        self.subscribe(manager, KuCoinChannel::Execution, symbols)
    }

    pub fn unsubscribe_trades(&self, manager: &Arc<Manager>, symbols: &[&str]) -> Result<()> {
        self.unsubscribe(manager, KuCoinChannel::Execution, symbols)
    }

    /// `/contractMarket/limitCandle:{symbol}_{type},...` for `(symbol, kline type)` pairs.
    pub fn subscribe_candles(&self, manager: &Arc<Manager>, pairs: &[(&str, &str)]) -> Result<()> {
        self.send(
            manager,
            KuCoinWssMessage::subscribe(kucoin_candle_topic(pairs)),
        )
    }

    pub fn unsubscribe_candles(
        &self,
        manager: &Arc<Manager>,
        pairs: &[(&str, &str)],
    ) -> Result<()> {
        self.send(
            manager,
            KuCoinWssMessage::unsubscribe(kucoin_candle_topic(pairs)),
        )
    }

    /// `/contract/announcement[:{symbols}]` (funding settlement begin/end).
    pub fn subscribe_funding_settlement(
        &self,
        manager: &Arc<Manager>,
        symbols: Option<&[&str]>,
    ) -> Result<()> {
        self.send(
            manager,
            KuCoinWssMessage::subscribe(kucoin_announcement_topic(symbols)),
        )
    }

    pub fn unsubscribe_funding_settlement(
        &self,
        manager: &Arc<Manager>,
        symbols: Option<&[&str]>,
    ) -> Result<()> {
        self.send(
            manager,
            KuCoinWssMessage::unsubscribe(kucoin_announcement_topic(symbols)),
        )
    }
}

impl Default for KuCoinClient {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
pub trait KuCoinApi {
    async fn get_contracts(
        &self,
    ) -> Result<HashMap<String, KuCoinContract>, Box<dyn std::error::Error + Send + Sync>>;

    async fn get_historical_funding_rates(
        &self,
        symbol: &str,
        start_time: i64,
        end_time: i64,
    ) -> Result<Vec<KuCoinFundingRate>, Box<dyn std::error::Error + Send + Sync>>;

    async fn get_contract(
        &self,
        symbol: &str,
    ) -> Result<KuCoinContract, Box<dyn std::error::Error + Send + Sync>>;

    async fn get_ticker(
        &self,
        symbol: &str,
    ) -> Result<KuCoinTicker, Box<dyn std::error::Error + Send + Sync>>;

    async fn get_all_tickers(
        &self,
    ) -> Result<Vec<KuCoinTicker>, Box<dyn std::error::Error + Send + Sync>>;

    async fn get_full_order_book(
        &self,
        symbol: &str,
    ) -> Result<KuCoinOrderBook, Box<dyn std::error::Error + Send + Sync>>;

    async fn get_part_order_book(
        &self,
        symbol: &str,
        size: u32,
    ) -> Result<KuCoinOrderBook, Box<dyn std::error::Error + Send + Sync>>;

    async fn get_trade_history(
        &self,
        symbol: &str,
    ) -> Result<Vec<KuCoinTrade>, Box<dyn std::error::Error + Send + Sync>>;

    async fn get_klines(
        &self,
        symbol: &str,
        granularity: u32,
        from: Option<i64>,
        to: Option<i64>,
    ) -> Result<Vec<KuCoinKline>, Box<dyn std::error::Error + Send + Sync>>;

    async fn get_mark_price(
        &self,
        symbol: &str,
    ) -> Result<KuCoinMarkPrice, Box<dyn std::error::Error + Send + Sync>>;

    async fn get_spot_index_price(
        &self,
        symbol: &str,
        query: &KuCoinIndexQuery,
    ) -> Result<KuCoinIndexPage<KuCoinSpotIndex>, Box<dyn std::error::Error + Send + Sync>>;

    async fn get_interest_rate_index(
        &self,
        symbol: &str,
        query: &KuCoinIndexQuery,
    ) -> Result<KuCoinIndexPage<KuCoinIndexValue>, Box<dyn std::error::Error + Send + Sync>>;

    async fn get_premium_index(
        &self,
        symbol: &str,
        query: &KuCoinIndexQuery,
    ) -> Result<KuCoinIndexPage<KuCoinIndexValue>, Box<dyn std::error::Error + Send + Sync>>;

    async fn get_current_funding_rate(
        &self,
        symbol: &str,
    ) -> Result<KuCoinCurrentFundingRate, Box<dyn std::error::Error + Send + Sync>>;

    async fn get_server_time(&self) -> Result<i64, Box<dyn std::error::Error + Send + Sync>>;

    async fn get_service_status(
        &self,
    ) -> Result<KuCoinServiceStatus, Box<dyn std::error::Error + Send + Sync>>;

    async fn get_open_interest(
        &self,
        symbols: Option<&[&str]>,
    ) -> Result<Vec<KuCoinOpenInterest>, Box<dyn std::error::Error + Send + Sync>>;

    async fn get_open_interest_history(
        &self,
        symbol: &str,
        interval: &str,
        start_at: Option<i64>,
        end_at: Option<i64>,
        page_size: Option<u32>,
    ) -> Result<Vec<KuCoinOpenInterestPoint>, Box<dyn std::error::Error + Send + Sync>>;
}

#[async_trait]
impl KuCoinApi for KuCoinClient {
    async fn get_contracts(
        &self,
    ) -> Result<HashMap<String, KuCoinContract>, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api.get_contracts().await
    }

    async fn get_historical_funding_rates(
        &self,
        symbol: &str,
        start_time: i64,
        end_time: i64,
    ) -> Result<Vec<KuCoinFundingRate>, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api
            .get_historical_funding_rates(symbol, start_time, end_time)
            .await
    }

    async fn get_contract(
        &self,
        symbol: &str,
    ) -> Result<KuCoinContract, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api.get_contract(symbol).await
    }

    async fn get_ticker(
        &self,
        symbol: &str,
    ) -> Result<KuCoinTicker, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api.get_ticker(symbol).await
    }

    async fn get_all_tickers(
        &self,
    ) -> Result<Vec<KuCoinTicker>, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api.get_all_tickers().await
    }

    async fn get_full_order_book(
        &self,
        symbol: &str,
    ) -> Result<KuCoinOrderBook, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api.get_full_order_book(symbol).await
    }

    async fn get_part_order_book(
        &self,
        symbol: &str,
        size: u32,
    ) -> Result<KuCoinOrderBook, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api.get_part_order_book(symbol, size).await
    }

    async fn get_trade_history(
        &self,
        symbol: &str,
    ) -> Result<Vec<KuCoinTrade>, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api.get_trade_history(symbol).await
    }

    async fn get_klines(
        &self,
        symbol: &str,
        granularity: u32,
        from: Option<i64>,
        to: Option<i64>,
    ) -> Result<Vec<KuCoinKline>, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api
            .get_klines(symbol, granularity, from, to)
            .await
    }

    async fn get_mark_price(
        &self,
        symbol: &str,
    ) -> Result<KuCoinMarkPrice, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api.get_mark_price(symbol).await
    }

    async fn get_spot_index_price(
        &self,
        symbol: &str,
        query: &KuCoinIndexQuery,
    ) -> Result<KuCoinIndexPage<KuCoinSpotIndex>, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api.get_spot_index_price(symbol, query).await
    }

    async fn get_interest_rate_index(
        &self,
        symbol: &str,
        query: &KuCoinIndexQuery,
    ) -> Result<KuCoinIndexPage<KuCoinIndexValue>, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api.get_interest_rate_index(symbol, query).await
    }

    async fn get_premium_index(
        &self,
        symbol: &str,
        query: &KuCoinIndexQuery,
    ) -> Result<KuCoinIndexPage<KuCoinIndexValue>, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api.get_premium_index(symbol, query).await
    }

    async fn get_current_funding_rate(
        &self,
        symbol: &str,
    ) -> Result<KuCoinCurrentFundingRate, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api.get_current_funding_rate(symbol).await
    }

    async fn get_server_time(&self) -> Result<i64, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api.get_server_time().await
    }

    async fn get_service_status(
        &self,
    ) -> Result<KuCoinServiceStatus, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api.get_service_status().await
    }

    async fn get_open_interest(
        &self,
        symbols: Option<&[&str]>,
    ) -> Result<Vec<KuCoinOpenInterest>, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api.get_open_interest(symbols).await
    }

    async fn get_open_interest_history(
        &self,
        symbol: &str,
        interval: &str,
        start_at: Option<i64>,
        end_at: Option<i64>,
        page_size: Option<u32>,
    ) -> Result<Vec<KuCoinOpenInterestPoint>, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api
            .get_open_interest_history(symbol, interval, start_at, end_at, page_size)
            .await
    }
}
