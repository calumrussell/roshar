//! ApeX Omni client: public market data (REST + websocket), API-key authenticated trading REST,
//! the private account websocket, and zkLink order signing.

/// Wire and message types (REST payloads, websocket messages).
pub mod types;

/// API-key HMAC request signing and the private websocket login message.
pub mod auth;
/// Order payloads (`Contract` message, zk signature, `limitFee`, form fields).
pub mod order;
/// zkLink musig signing (Alt-BabyJubjub + Rescue), ported from the SDK's native signer.
pub mod zk;

pub mod rest;

pub use rest::{MarketApi as ApexMarketApi, TradingApi as ApexTradingApi};

use crate::auth::ApexApiCredentials;
use crate::types::{
    ApexBookDepth, ApexDepth, ApexFundingHistory, ApexFundingHistoryQuery, ApexKline,
    ApexServerTime, ApexSymbols, ApexTicker, ApexTopic, ApexTrade, ApexWsFrequency, ApexWsRequest,
};
use anyhow::Result;
use async_trait::async_trait;
use roshar_http::RateLimitedClient;
use roshar_ws_mgr::{Config, Manager, Message};
use std::collections::HashMap;
use std::sync::Arc;

pub use rest::ApiResult;

pub const APEX_OMNI_REST_URL: &str = "https://omni.apex.exchange/api";
pub const APEX_OMNI_TESTNET_REST_URL: &str = "https://testnet.omni.apex.exchange/api";
pub const APEX_OMNI_PUBLIC_WSS_URL: &str = "wss://quote.omni.apex.exchange/realtime_public?v=2";
pub const APEX_OMNI_PRIVATE_WSS_URL: &str = "wss://quote.omni.apex.exchange/realtime_private?v=2";
pub const APEX_OMNI_TESTNET_PUBLIC_WSS_URL: &str =
    "wss://qa-quote.omni.apex.exchange/realtime_public?v=2";
pub const APEX_OMNI_TESTNET_PRIVATE_WSS_URL: &str =
    "wss://qa-quote.omni.apex.exchange/realtime_private?v=2";

pub struct ApexClient {
    http: RateLimitedClient,
    rest_url: &'static str,
    market_api: rest::MarketApi,
    ws_config: Config,
    ws_private_config: Config,
}

impl ApexClient {
    /// Documented IP limit: 600 requests / 60 s.
    const DEFAULT_REQUESTS_PER_SECOND: u32 = 10;
    const WS_CONN_NAME: &str = "apex";
    const WS_PRIVATE_CONN_NAME: &str = "apex-private";

    pub fn new() -> Self {
        Self::new_with_rate_limit(Self::DEFAULT_REQUESTS_PER_SECOND)
    }

    pub fn new_with_rate_limit(requests_per_second: u32) -> Self {
        Self::with_endpoints(
            requests_per_second,
            APEX_OMNI_REST_URL,
            APEX_OMNI_PUBLIC_WSS_URL,
            APEX_OMNI_PRIVATE_WSS_URL,
        )
    }

    pub fn testnet(requests_per_second: u32) -> Self {
        Self::with_endpoints(
            requests_per_second,
            APEX_OMNI_TESTNET_REST_URL,
            APEX_OMNI_TESTNET_PUBLIC_WSS_URL,
            APEX_OMNI_TESTNET_PRIVATE_WSS_URL,
        )
    }

    fn with_endpoints(
        requests_per_second: u32,
        rest_url: &'static str,
        public_wss: &str,
        private_wss: &str,
    ) -> Self {
        let http = RateLimitedClient::new(requests_per_second, 1);
        Self {
            market_api: rest::MarketApi::new(http.clone(), rest_url),
            http,
            rest_url,
            ws_config: Self::ws_config_for(Self::WS_CONN_NAME, public_wss),
            ws_private_config: Self::ws_config_for(Self::WS_PRIVATE_CONN_NAME, private_wss),
        }
    }

    fn ws_config_for(name: &str, url: &str) -> Config {
        Config {
            name: name.to_string(),
            url: url.to_string(),
            // The server answers protocol pings, so they detect a dead link. They do not keep the
            // session alive: the server sends `{"op":"ping",..}` every 15 s and closes the
            // connection about 150 s later unless each is answered with a pong
            // (`ApexWsRequest::pong_reply`, `ApexClient::send_pong`).
            ping_duration: 15,
            ping_message: String::new(),
            ping_timeout: 60,
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
        }
    }

    /// Public websocket config for roshar-ws-mgr.
    pub fn ws_config(&self) -> Config {
        self.ws_config.clone()
    }

    pub fn ws_config_mut(&mut self) -> &mut Config {
        &mut self.ws_config
    }

    /// Private websocket config for roshar-ws-mgr; send `login` after connecting.
    pub fn ws_private_config(&self) -> Config {
        self.ws_private_config.clone()
    }

    pub fn ws_private_config_mut(&mut self) -> &mut Config {
        &mut self.ws_private_config
    }

    /// Trading REST client sharing this client's rate limiter.
    pub fn trading_api(&self, credentials: ApexApiCredentials) -> rest::TradingApi {
        rest::TradingApi::new(self.http.clone(), self.rest_url, credentials)
    }

    fn send(&self, manager: &Arc<Manager>, config: &Config, text: String) -> Result<()> {
        manager.write(
            &config.name,
            Message::TextMessage(config.name.clone(), text),
        )?;
        Ok(())
    }

    fn send_public(&self, manager: &Arc<Manager>, msg: ApexWsRequest) -> Result<()> {
        self.send(manager, &self.ws_config, msg.to_json())
    }

    /// Answer a server ping on the public connection (`args` echoed from the ping).
    pub fn send_pong(&self, manager: &Arc<Manager>, args: Vec<String>) -> Result<()> {
        self.send_public(manager, ApexWsRequest::pong(args))
    }

    /// Answer a server ping on the private connection.
    pub fn send_private_pong(&self, manager: &Arc<Manager>, args: Vec<String>) -> Result<()> {
        self.send(
            manager,
            &self.ws_private_config,
            ApexWsRequest::pong(args).to_json(),
        )
    }

    /// Subscribe to arbitrary public topics (see `types::ApexTopic`).
    pub fn subscribe_topics(&self, manager: &Arc<Manager>, topics: &[String]) -> Result<()> {
        self.send_public(manager, ApexWsRequest::subscribe(topics))
    }

    pub fn unsubscribe_topics(&self, manager: &Arc<Manager>, topics: &[String]) -> Result<()> {
        self.send_public(manager, ApexWsRequest::unsubscribe(topics))
    }

    /// `orderBook{25|200}.{H|M}.{symbol}` for each `crossSymbolName`.
    pub fn subscribe_depth(
        &self,
        manager: &Arc<Manager>,
        symbols: &[&str],
        depth: ApexBookDepth,
        frequency: ApexWsFrequency,
    ) -> Result<()> {
        let topics: Vec<String> = symbols
            .iter()
            .map(|s| ApexTopic::order_book(s, depth, frequency))
            .collect();
        self.subscribe_topics(manager, &topics)
    }

    pub fn unsubscribe_depth(
        &self,
        manager: &Arc<Manager>,
        symbols: &[&str],
        depth: ApexBookDepth,
        frequency: ApexWsFrequency,
    ) -> Result<()> {
        let topics: Vec<String> = symbols
            .iter()
            .map(|s| ApexTopic::order_book(s, depth, frequency))
            .collect();
        self.unsubscribe_topics(manager, &topics)
    }

    /// `recentlyTrade.{H|M}.{symbol}`.
    pub fn subscribe_trades(
        &self,
        manager: &Arc<Manager>,
        symbols: &[&str],
        frequency: ApexWsFrequency,
    ) -> Result<()> {
        let topics: Vec<String> = symbols
            .iter()
            .map(|s| ApexTopic::trades(s, frequency))
            .collect();
        self.subscribe_topics(manager, &topics)
    }

    pub fn unsubscribe_trades(
        &self,
        manager: &Arc<Manager>,
        symbols: &[&str],
        frequency: ApexWsFrequency,
    ) -> Result<()> {
        let topics: Vec<String> = symbols
            .iter()
            .map(|s| ApexTopic::trades(s, frequency))
            .collect();
        self.unsubscribe_topics(manager, &topics)
    }

    /// `instrumentInfo.{H|M}.{symbol}`: ticker, mark/index price and funding rate.
    pub fn subscribe_ticker(
        &self,
        manager: &Arc<Manager>,
        symbols: &[&str],
        frequency: ApexWsFrequency,
    ) -> Result<()> {
        let topics: Vec<String> = symbols
            .iter()
            .map(|s| ApexTopic::ticker(s, frequency))
            .collect();
        self.subscribe_topics(manager, &topics)
    }

    pub fn unsubscribe_ticker(
        &self,
        manager: &Arc<Manager>,
        symbols: &[&str],
        frequency: ApexWsFrequency,
    ) -> Result<()> {
        let topics: Vec<String> = symbols
            .iter()
            .map(|s| ApexTopic::ticker(s, frequency))
            .collect();
        self.unsubscribe_topics(manager, &topics)
    }

    /// `instrumentInfo.all`.
    pub fn subscribe_all_tickers(&self, manager: &Arc<Manager>) -> Result<()> {
        self.subscribe_topics(manager, &[ApexTopic::all_tickers()])
    }

    pub fn unsubscribe_all_tickers(&self, manager: &Arc<Manager>) -> Result<()> {
        self.unsubscribe_topics(manager, &[ApexTopic::all_tickers()])
    }

    /// `candle.{interval}.{symbol}`.
    pub fn subscribe_candles(
        &self,
        manager: &Arc<Manager>,
        symbols: &[&str],
        interval: &str,
    ) -> Result<()> {
        let topics: Vec<String> = symbols
            .iter()
            .map(|s| ApexTopic::candle(s, interval))
            .collect();
        self.subscribe_topics(manager, &topics)
    }

    pub fn unsubscribe_candles(
        &self,
        manager: &Arc<Manager>,
        symbols: &[&str],
        interval: &str,
    ) -> Result<()> {
        let topics: Vec<String> = symbols
            .iter()
            .map(|s| ApexTopic::candle(s, interval))
            .collect();
        self.unsubscribe_topics(manager, &topics)
    }

    /// Log in on the private connection, subscribing `topics` (`types::APEX_TOPIC_ACCOUNT`,
    /// `types::APEX_TOPIC_NOTIFY`).
    pub fn login_private(
        &self,
        manager: &Arc<Manager>,
        credentials: &ApexApiCredentials,
        timestamp_ms: u64,
        topics: &[&str],
    ) -> Result<()> {
        self.send(
            manager,
            &self.ws_private_config,
            credentials.ws_login_message(timestamp_ms, topics),
        )
    }

    pub fn subscribe_private(&self, manager: &Arc<Manager>, topics: &[String]) -> Result<()> {
        self.send(
            manager,
            &self.ws_private_config,
            ApexWsRequest::subscribe(topics).to_json(),
        )
    }

    pub fn unsubscribe_private(&self, manager: &Arc<Manager>, topics: &[String]) -> Result<()> {
        self.send(
            manager,
            &self.ws_private_config,
            ApexWsRequest::unsubscribe(topics).to_json(),
        )
    }
}

impl Default for ApexClient {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
pub trait ApexApi {
    async fn get_time(&self) -> ApiResult<ApexServerTime>;

    async fn get_symbols(&self) -> ApiResult<ApexSymbols>;

    async fn get_depth(&self, symbol: &str, limit: Option<u32>) -> ApiResult<ApexDepth>;

    async fn get_trades(&self, symbol: &str, limit: Option<u32>) -> ApiResult<Vec<ApexTrade>>;

    async fn get_klines(
        &self,
        symbol: &str,
        interval: &str,
        start: Option<u64>,
        end: Option<u64>,
        limit: Option<u32>,
    ) -> ApiResult<HashMap<String, Vec<ApexKline>>>;

    async fn get_ticker(&self, symbol: &str) -> ApiResult<Vec<ApexTicker>>;

    async fn get_funding_history(
        &self,
        symbol: &str,
        query: &ApexFundingHistoryQuery,
    ) -> ApiResult<ApexFundingHistory>;
}

#[async_trait]
impl ApexApi for ApexClient {
    async fn get_time(&self) -> ApiResult<ApexServerTime> {
        self.market_api.get_time().await
    }

    async fn get_symbols(&self) -> ApiResult<ApexSymbols> {
        self.market_api.get_symbols().await
    }

    async fn get_depth(&self, symbol: &str, limit: Option<u32>) -> ApiResult<ApexDepth> {
        self.market_api.get_depth(symbol, limit).await
    }

    async fn get_trades(&self, symbol: &str, limit: Option<u32>) -> ApiResult<Vec<ApexTrade>> {
        self.market_api.get_trades(symbol, limit).await
    }

    async fn get_klines(
        &self,
        symbol: &str,
        interval: &str,
        start: Option<u64>,
        end: Option<u64>,
        limit: Option<u32>,
    ) -> ApiResult<HashMap<String, Vec<ApexKline>>> {
        self.market_api
            .get_klines(symbol, interval, start, end, limit)
            .await
    }

    async fn get_ticker(&self, symbol: &str) -> ApiResult<Vec<ApexTicker>> {
        self.market_api.get_ticker(symbol).await
    }

    async fn get_funding_history(
        &self,
        symbol: &str,
        query: &ApexFundingHistoryQuery,
    ) -> ApiResult<ApexFundingHistory> {
        self.market_api.get_funding_history(symbol, query).await
    }
}
