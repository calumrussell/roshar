//! Polymarket Perps (`api.perpetuals.polymarket.com`): public market data, proxy-key signing,
//! signed trading routes, authenticated account reads and WebSocket message builders.

/// EIP-712 signing of trading ops, `createProxy` and `deleteProxy`.
pub mod signer;
/// Wire types (REST payloads, WebSocket frames and builders).
pub mod types;

pub mod rest;

pub use rest::market::MarketApi as PolymarketPerpsMarketApi;
pub use rest::trading::{ProxyCredentials, TradingApi as PolymarketPerpsTradingApi};
pub use rest::{ApiResult, PolymarketPerpsApiError};

use crate::rest::market::MarketApi;
use crate::rest::trading::TradingApi;
use crate::signer::SignedCommand;
use crate::types::{
    Bbo, Book, CollateralAsset, ExchangeInfo, ExchangeStatistics, FeesInfo, FundingHistory,
    IndexPrice, Instrument, InstrumentScope, KlineInterval, Klines, LimitTier, MarkHistory,
    PingResponse, Registration, ServerTime, Statistic, Ticker, Trades,
};
use anyhow::Result;
use async_trait::async_trait;
use roshar_http::RateLimitedClient;
use roshar_ws_mgr::{Config, Manager, Message};
use std::sync::Arc;

/// Production REST base URL.
pub const POLYMARKET_PERPS_API_URL: &str = "https://api.perpetuals.polymarket.com";
/// Production WebSocket URL (public and private channels, and WebSocket order entry).
pub const POLYMARKET_PERPS_WSS_URL: &str = "wss://ws.perpetuals.polymarket.com/v1/ws";

pub struct PolymarketPerpsClient {
    client: RateLimitedClient,
    market_api: MarketApi,
    ws_config: Config,
}

impl Default for PolymarketPerpsClient {
    fn default() -> Self {
        Self::new()
    }
}

impl PolymarketPerpsClient {
    /// Documented IP limit: 1,000 weighted tokens per minute; lightweight reads weigh 1 and
    /// broad or deep reads up to 20 (https://docs.polymarket.com/api-reference/perps/rate-limits).
    /// 16 requests/s = 960 weight-1 requests a minute.
    const DEFAULT_REQUESTS_PER_SECOND: u32 = 16;
    const WS_CONN_NAME: &'static str = "polymarket-perps";

    pub fn new() -> Self {
        Self::new_with_rate_limit(Self::DEFAULT_REQUESTS_PER_SECOND)
    }

    pub fn new_with_rate_limit(requests_per_second: u32) -> Self {
        let client = RateLimitedClient::new(requests_per_second, 1);
        Self {
            market_api: MarketApi::new(client.clone(), POLYMARKET_PERPS_API_URL),
            client,
            ws_config: Config {
                name: Self::WS_CONN_NAME.to_string(),
                url: POLYMARKET_PERPS_WSS_URL.to_string(),
                // The server closes connections after 60 s without traffic. The docs ask for an
                // application-level `ping` post about every 25 s; the reply is a JSON frame, not
                // a protocol pong, and the docs call a connection stale after ~65 s of silence.
                ping_duration: 25,
                ping_message: types::ping(None),
                ping_timeout: 65,
                reconnect_timeout: 5000,
                use_text_ping: Some(true),
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

    /// Signed trading routes and authenticated reads, sharing this client's IP rate limiter.
    pub fn trading_api(&self) -> TradingApi {
        TradingApi::new(self.client.clone(), POLYMARKET_PERPS_API_URL)
    }

    pub fn ws_config(&self) -> roshar_ws_mgr::Config {
        self.ws_config.clone()
    }

    pub fn ws_config_mut(&mut self) -> &mut roshar_ws_mgr::Config {
        &mut self.ws_config
    }

    fn send(&self, manager: &Arc<Manager>, text: String) -> Result<()> {
        manager.write(
            &self.ws_config.name,
            Message::TextMessage(self.ws_config.name.clone(), text),
        )?;
        Ok(())
    }

    /// Sends `{"id"?, "req": "sub", "chs": channels}` (see the `types::*_channel` builders).
    pub fn subscribe(
        &self,
        manager: &Arc<Manager>,
        id: Option<u64>,
        channels: &[String],
    ) -> Result<()> {
        self.send(manager, types::subscribe(id, channels))
    }

    /// Sends `{"id"?, "req": "unsub", "chs": channels}`.
    pub fn unsubscribe(
        &self,
        manager: &Arc<Manager>,
        id: Option<u64>,
        channels: &[String],
    ) -> Result<()> {
        self.send(manager, types::unsubscribe(id, channels))
    }

    /// `bbo::{iid}` for each instrument.
    pub fn subscribe_bbo(
        &self,
        manager: &Arc<Manager>,
        id: Option<u64>,
        instrument_ids: &[u32],
    ) -> Result<()> {
        self.subscribe(manager, id, &bbo_channels(instrument_ids))
    }

    pub fn unsubscribe_bbo(
        &self,
        manager: &Arc<Manager>,
        id: Option<u64>,
        instrument_ids: &[u32],
    ) -> Result<()> {
        self.unsubscribe(manager, id, &bbo_channels(instrument_ids))
    }

    /// `book::{iid}` (top 20) or `book::{iid}::{depth}` (the exchange accepts 20 and 50). Full
    /// snapshots every 100 ms.
    pub fn subscribe_book(
        &self,
        manager: &Arc<Manager>,
        id: Option<u64>,
        instrument_ids: &[u32],
        depth: Option<u32>,
    ) -> Result<()> {
        self.subscribe(manager, id, &book_channels(instrument_ids, depth))
    }

    /// `depth` must match the subscription.
    pub fn unsubscribe_book(
        &self,
        manager: &Arc<Manager>,
        id: Option<u64>,
        instrument_ids: &[u32],
        depth: Option<u32>,
    ) -> Result<()> {
        self.unsubscribe(manager, id, &book_channels(instrument_ids, depth))
    }

    /// `trades::{iid}` for each instrument.
    pub fn subscribe_trades(
        &self,
        manager: &Arc<Manager>,
        id: Option<u64>,
        instrument_ids: &[u32],
    ) -> Result<()> {
        self.subscribe(manager, id, &trades_channels(instrument_ids))
    }

    pub fn unsubscribe_trades(
        &self,
        manager: &Arc<Manager>,
        id: Option<u64>,
        instrument_ids: &[u32],
    ) -> Result<()> {
        self.unsubscribe(manager, id, &trades_channels(instrument_ids))
    }

    /// `tickers::{iid|all}`: prices, open interest, current funding rate and next funding time.
    /// This is also the public funding stream; there is no separate funding channel.
    pub fn subscribe_tickers(
        &self,
        manager: &Arc<Manager>,
        id: Option<u64>,
        scopes: &[InstrumentScope],
    ) -> Result<()> {
        self.subscribe(manager, id, &tickers_channels(scopes))
    }

    pub fn unsubscribe_tickers(
        &self,
        manager: &Arc<Manager>,
        id: Option<u64>,
        scopes: &[InstrumentScope],
    ) -> Result<()> {
        self.unsubscribe(manager, id, &tickers_channels(scopes))
    }

    /// `statistics::{iid|all}`: rolling 24 h statistics.
    pub fn subscribe_statistics(
        &self,
        manager: &Arc<Manager>,
        id: Option<u64>,
        scopes: &[InstrumentScope],
    ) -> Result<()> {
        self.subscribe(manager, id, &statistics_channels(scopes))
    }

    pub fn unsubscribe_statistics(
        &self,
        manager: &Arc<Manager>,
        id: Option<u64>,
        scopes: &[InstrumentScope],
    ) -> Result<()> {
        self.unsubscribe(manager, id, &statistics_channels(scopes))
    }

    /// `klines::{iid}::{interval}` for each instrument.
    pub fn subscribe_klines(
        &self,
        manager: &Arc<Manager>,
        id: Option<u64>,
        instrument_ids: &[u32],
        interval: KlineInterval,
    ) -> Result<()> {
        self.subscribe(manager, id, &klines_channels(instrument_ids, interval))
    }

    pub fn unsubscribe_klines(
        &self,
        manager: &Arc<Manager>,
        id: Option<u64>,
        instrument_ids: &[u32],
        interval: KlineInterval,
    ) -> Result<()> {
        self.unsubscribe(manager, id, &klines_channels(instrument_ids, interval))
    }

    /// Authenticates this connection for private channels and order entry. Check the response
    /// (`types::AuthResult`) before subscribing; resend after every reconnect.
    pub fn authenticate(
        &self,
        manager: &Arc<Manager>,
        id: Option<u64>,
        credentials: &ProxyCredentials,
    ) -> Result<()> {
        self.send(
            manager,
            types::auth(id, &credentials.proxy, &credentials.secret),
        )
    }

    /// Subscribes to private channels (`types::CHANNEL_ORDERS`, `CHANNEL_FILLS`,
    /// `CHANNEL_PORTFOLIO`, `CHANNEL_BALANCES`, `CHANNEL_FUNDING`) after `authenticate`.
    pub fn subscribe_private(
        &self,
        manager: &Arc<Manager>,
        id: Option<u64>,
        channels: &[&str],
    ) -> Result<()> {
        self.send(manager, types::subscribe(id, channels))
    }

    pub fn unsubscribe_private(
        &self,
        manager: &Arc<Manager>,
        id: Option<u64>,
        channels: &[&str],
    ) -> Result<()> {
        self.send(manager, types::unsubscribe(id, channels))
    }

    /// Sends a signed op as a WebSocket `post` frame; the response arrives with the same `id`.
    pub fn send_command(
        &self,
        manager: &Arc<Manager>,
        id: u64,
        command: &SignedCommand,
    ) -> Result<()> {
        self.send(manager, command.ws_frame(id)?)
    }
}

fn bbo_channels(instrument_ids: &[u32]) -> Vec<String> {
    instrument_ids
        .iter()
        .map(|&i| types::bbo_channel(i))
        .collect()
}

fn book_channels(instrument_ids: &[u32], depth: Option<u32>) -> Vec<String> {
    instrument_ids
        .iter()
        .map(|&i| types::book_channel(i, depth))
        .collect()
}

fn trades_channels(instrument_ids: &[u32]) -> Vec<String> {
    instrument_ids
        .iter()
        .map(|&i| types::trades_channel(i))
        .collect()
}

fn tickers_channels(scopes: &[InstrumentScope]) -> Vec<String> {
    scopes.iter().map(|&s| types::tickers_channel(s)).collect()
}

fn statistics_channels(scopes: &[InstrumentScope]) -> Vec<String> {
    scopes
        .iter()
        .map(|&s| types::statistics_channel(s))
        .collect()
}

fn klines_channels(instrument_ids: &[u32], interval: KlineInterval) -> Vec<String> {
    instrument_ids
        .iter()
        .map(|&i| types::klines_channel(i, interval))
        .collect()
}

/// Public REST market data.
#[async_trait]
pub trait PolymarketPerpsApi {
    async fn ping(&self) -> ApiResult<PingResponse>;
    async fn get_time(&self) -> ApiResult<ServerTime>;
    async fn get_exchange(&self) -> ApiResult<ExchangeInfo>;
    async fn get_assets(&self) -> ApiResult<Vec<CollateralAsset>>;
    async fn get_instruments(
        &self,
        instrument_id: Option<u32>,
        instrument_type: Option<&str>,
        category: Option<&str>,
    ) -> ApiResult<Vec<Instrument>>;
    async fn get_tickers(&self, instrument_id: Option<u32>) -> ApiResult<Vec<Ticker>>;
    async fn get_statistics(&self, instrument_id: Option<u32>) -> ApiResult<Vec<Statistic>>;
    async fn get_exchange_stats(
        &self,
        start_timestamp: i64,
        end_timestamp: i64,
    ) -> ApiResult<ExchangeStatistics>;
    async fn get_klines(
        &self,
        instrument_id: u32,
        interval: KlineInterval,
        start_timestamp: i64,
        end_timestamp: Option<i64>,
    ) -> ApiResult<Klines>;
    async fn get_mark_history(
        &self,
        instrument_id: u32,
        interval: KlineInterval,
        start_timestamp: i64,
        end_timestamp: Option<i64>,
    ) -> ApiResult<MarkHistory>;
    async fn get_bbo(&self, instrument_id: Option<u32>) -> ApiResult<Vec<Bbo>>;
    async fn get_book(&self, instrument_id: u32, depth: Option<u32>) -> ApiResult<Book>;
    async fn get_index(&self, asset: &str) -> ApiResult<IndexPrice>;
    async fn get_trades(
        &self,
        instrument_id: u32,
        start_timestamp: Option<i64>,
        end_timestamp: Option<i64>,
    ) -> ApiResult<Trades>;
    async fn get_funding_history(
        &self,
        instrument_id: u32,
        start_timestamp: Option<i64>,
        end_timestamp: Option<i64>,
    ) -> ApiResult<FundingHistory>;
    async fn get_fees(&self) -> ApiResult<FeesInfo>;
    async fn get_limit_tiers(&self) -> ApiResult<Vec<LimitTier>>;
    async fn get_registered(&self, address: &str) -> ApiResult<Registration>;
}

#[async_trait]
impl PolymarketPerpsApi for PolymarketPerpsClient {
    async fn ping(&self) -> ApiResult<PingResponse> {
        self.market_api.ping().await
    }

    async fn get_time(&self) -> ApiResult<ServerTime> {
        self.market_api.get_time().await
    }

    async fn get_exchange(&self) -> ApiResult<ExchangeInfo> {
        self.market_api.get_exchange().await
    }

    async fn get_assets(&self) -> ApiResult<Vec<CollateralAsset>> {
        self.market_api.get_assets().await
    }

    async fn get_instruments(
        &self,
        instrument_id: Option<u32>,
        instrument_type: Option<&str>,
        category: Option<&str>,
    ) -> ApiResult<Vec<Instrument>> {
        self.market_api
            .get_instruments(instrument_id, instrument_type, category)
            .await
    }

    async fn get_tickers(&self, instrument_id: Option<u32>) -> ApiResult<Vec<Ticker>> {
        self.market_api.get_tickers(instrument_id).await
    }

    async fn get_statistics(&self, instrument_id: Option<u32>) -> ApiResult<Vec<Statistic>> {
        self.market_api.get_statistics(instrument_id).await
    }

    async fn get_exchange_stats(
        &self,
        start_timestamp: i64,
        end_timestamp: i64,
    ) -> ApiResult<ExchangeStatistics> {
        self.market_api
            .get_exchange_stats(start_timestamp, end_timestamp)
            .await
    }

    async fn get_klines(
        &self,
        instrument_id: u32,
        interval: KlineInterval,
        start_timestamp: i64,
        end_timestamp: Option<i64>,
    ) -> ApiResult<Klines> {
        self.market_api
            .get_klines(instrument_id, interval, start_timestamp, end_timestamp)
            .await
    }

    async fn get_mark_history(
        &self,
        instrument_id: u32,
        interval: KlineInterval,
        start_timestamp: i64,
        end_timestamp: Option<i64>,
    ) -> ApiResult<MarkHistory> {
        self.market_api
            .get_mark_history(instrument_id, interval, start_timestamp, end_timestamp)
            .await
    }

    async fn get_bbo(&self, instrument_id: Option<u32>) -> ApiResult<Vec<Bbo>> {
        self.market_api.get_bbo(instrument_id).await
    }

    async fn get_book(&self, instrument_id: u32, depth: Option<u32>) -> ApiResult<Book> {
        self.market_api.get_book(instrument_id, depth).await
    }

    async fn get_index(&self, asset: &str) -> ApiResult<IndexPrice> {
        self.market_api.get_index(asset).await
    }

    async fn get_trades(
        &self,
        instrument_id: u32,
        start_timestamp: Option<i64>,
        end_timestamp: Option<i64>,
    ) -> ApiResult<Trades> {
        self.market_api
            .get_trades(instrument_id, start_timestamp, end_timestamp)
            .await
    }

    async fn get_funding_history(
        &self,
        instrument_id: u32,
        start_timestamp: Option<i64>,
        end_timestamp: Option<i64>,
    ) -> ApiResult<FundingHistory> {
        self.market_api
            .get_funding_history(instrument_id, start_timestamp, end_timestamp)
            .await
    }

    async fn get_fees(&self) -> ApiResult<FeesInfo> {
        self.market_api.get_fees().await
    }

    async fn get_limit_tiers(&self) -> ApiResult<Vec<LimitTier>> {
        self.market_api.get_limit_tiers().await
    }

    async fn get_registered(&self, address: &str) -> ApiResult<Registration> {
        self.market_api.get_registered(address).await
    }
}
