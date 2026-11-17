//! Orderly Network client: public market data (REST + websocket), Orderly-key (ed25519) request
//! signing, wallet (EIP-712) signing for adding an Orderly key, and the signed trading API.

/// Wire and message types (REST payloads, websocket pushes).
pub mod types;

pub mod request;
pub mod signer;
pub mod ws;

pub(crate) mod rest;
mod wire;

pub use request::{
    Method, OrderlyEditOrderRequest, OrderlyFundingFeeQuery, OrderlyMarginMode,
    OrderlyOrderRequest, OrderlyOrderType, OrderlyOrdersQuery, OrderlySide, OrderlyTradesQuery,
    PreparedRequest,
};
pub use rest::{
    AccountApi as OrderlyAccountApi, MarketApi as OrderlyMarketApi, OrderlyError,
    OrderlyMarketDetailQuery, TradingApi as OrderlyTradingApi, ORDERLY_API_URL,
    ORDERLY_TESTNET_API_URL,
};
pub use signer::{
    AddOrderlyKey, OrderlyAuthHeaders, OrderlyKey, SignedAddOrderlyKey, WalletSigner,
};
pub use ws::{
    OrderlyPrivateTopic, OrderlyTopic, ORDERLY_PRIVATE_WSS_URL, ORDERLY_TESTNET_PRIVATE_WSS_URL,
    ORDERLY_TESTNET_WSS_URL, ORDERLY_WSS_URL,
};

use crate::types::{
    OrderlyBinaryRound, OrderlyBinaryRoundDetail, OrderlyChainInfo, OrderlyFundingRate,
    OrderlyFundingRateHistory, OrderlyFundingSummary, OrderlyFuture, OrderlyHistoryChart,
    OrderlyIndexPricePoint, OrderlyIndexPriceSource, OrderlyInsuranceFund, OrderlyLeverageConfig,
    OrderlyLiquidatedPosition, OrderlyLiquidation, OrderlyMarketTrade, OrderlyMaxLeverage,
    OrderlyPage, OrderlyPriceChanges, OrderlyQueryCandle, OrderlyQueryFundingComparison,
    OrderlyQueryFundingRate, OrderlyQueryLiquidation, OrderlyQueryMarketDetail,
    OrderlyQueryMarketSummary, OrderlyQueryOrderbook, OrderlyQueryPage, OrderlyQueryTrade,
    OrderlySymbol, OrderlySystemInfo, OrderlyToken, OrderlyTradersOpenInterest, OrderlyTvBars,
    OrderlyTvConfig, OrderlyTvSymbolInfo,
};
use anyhow::Result;
use async_trait::async_trait;
use roshar_http::RateLimitedClient;
use roshar_ws_mgr::{Config, Manager, Message};
use std::sync::Arc;

type ApiResult<T> = rest::ApiResult<T>;

/// Documented public REST limit: 10 requests per second per IP on the market endpoints
/// (https://orderly.network/docs/build-on-omnichain/restful-api/public/get-available-symbols).
/// Private endpoints are mostly 10 per second per Orderly key.
pub const DEFAULT_REQUESTS_PER_SECOND: u32 = 10;

fn ws_config(name: &str, url: String) -> Config {
    Config {
        name: name.to_string(),
        url,
        // The server sends `{"event":"ping"}` every 10 s and drops the connection after ten
        // unanswered pings; a client `{"event":"ping"}` every 10 s also keeps it alive. Text
        // pongs are not protocol pongs, so ws-mgr's pong timeout never fires for this feed.
        ping_duration: 10,
        ping_message: ws::PING.to_string(),
        ping_timeout: 30,
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
    }
}

fn send(manager: &Arc<Manager>, config: &Config, frame: String) -> Result<()> {
    manager.write(
        &config.name,
        Message::TextMessage(config.name.clone(), frame),
    )?;
    Ok(())
}

/// Public market data: REST plus the public websocket connection.
pub struct OrderlyClient {
    market_api: rest::MarketApi,
    ws_config: Config,
}

impl OrderlyClient {
    const WS_CONN_NAME: &str = "orderly";

    /// Mainnet. `ws_account_id` goes in the public stream path
    /// (`wss://ws-evm.orderly.org/ws/stream/{account_id}`); the server closes connections whose
    /// id it does not recognise.
    pub fn new(ws_account_id: &str) -> Self {
        Self::new_with_rate_limit(ws_account_id, DEFAULT_REQUESTS_PER_SECOND)
    }

    pub fn new_with_rate_limit(ws_account_id: &str, requests_per_second: u32) -> Self {
        Self::with_endpoints(
            RateLimitedClient::new(requests_per_second, 1),
            ORDERLY_API_URL,
            &ws::stream_url(ORDERLY_WSS_URL, ws_account_id),
        )
    }

    /// Custom endpoints, e.g. testnet: `ORDERLY_TESTNET_API_URL` and
    /// `ws::stream_url(ORDERLY_TESTNET_WSS_URL, id)`.
    pub fn with_endpoints(client: RateLimitedClient, api_url: &str, ws_url: &str) -> Self {
        Self {
            market_api: rest::MarketApi::new(client, api_url),
            ws_config: ws_config(Self::WS_CONN_NAME, ws_url.to_string()),
        }
    }

    pub fn market_api(&self) -> &rest::MarketApi {
        &self.market_api
    }

    pub fn ws_config(&self) -> roshar_ws_mgr::Config {
        self.ws_config.clone()
    }

    pub fn ws_config_mut(&mut self) -> &mut roshar_ws_mgr::Config {
        &mut self.ws_config
    }

    /// Sends `{"id", "event": "subscribe", "topic"}`; the reply echoes `id`.
    pub fn subscribe(
        &self,
        manager: &Arc<Manager>,
        id: &str,
        topic: &OrderlyTopic<'_>,
    ) -> Result<()> {
        send(manager, &self.ws_config, ws::subscribe(id, topic))
    }

    pub fn unsubscribe(
        &self,
        manager: &Arc<Manager>,
        id: &str,
        topic: &OrderlyTopic<'_>,
    ) -> Result<()> {
        send(manager, &self.ws_config, ws::unsubscribe(id, topic))
    }

    /// One-off full order book over the websocket.
    pub fn request_orderbook(&self, manager: &Arc<Manager>, id: &str, symbol: &str) -> Result<()> {
        send(manager, &self.ws_config, ws::request_orderbook(id, symbol))
    }

    /// Answers a server `{"event":"ping"}`.
    pub fn pong(&self, manager: &Arc<Manager>) -> Result<()> {
        send(manager, &self.ws_config, ws::PONG.to_string())
    }
}

/// The private websocket connection: authenticate, then subscribe to account topics.
pub struct OrderlyPrivateWs {
    ws_config: Config,
}

impl OrderlyPrivateWs {
    const WS_CONN_NAME: &str = "orderly-private";

    /// Mainnet private stream for `account_id`.
    pub fn new(account_id: &str) -> Self {
        Self::with_url(&ws::stream_url(ORDERLY_PRIVATE_WSS_URL, account_id))
    }

    pub fn with_url(ws_url: &str) -> Self {
        Self {
            ws_config: ws_config(Self::WS_CONN_NAME, ws_url.to_string()),
        }
    }

    pub fn ws_config(&self) -> roshar_ws_mgr::Config {
        self.ws_config.clone()
    }

    pub fn ws_config_mut(&mut self) -> &mut roshar_ws_mgr::Config {
        &mut self.ws_config
    }

    /// Sends the `auth` frame. Must be sent (again) after every (re)connect, before subscribing;
    /// the server disconnects on failed auth.
    pub fn auth(
        &self,
        manager: &Arc<Manager>,
        key: &OrderlyKey,
        id: &str,
        timestamp_ms: u64,
    ) -> Result<()> {
        send(
            manager,
            &self.ws_config,
            key.ws_auth_message(id, timestamp_ms),
        )
    }

    pub fn subscribe(
        &self,
        manager: &Arc<Manager>,
        id: &str,
        topic: &OrderlyPrivateTopic<'_>,
    ) -> Result<()> {
        send(manager, &self.ws_config, ws::subscribe_private(id, topic))
    }

    pub fn unsubscribe(
        &self,
        manager: &Arc<Manager>,
        id: &str,
        topic: &OrderlyPrivateTopic<'_>,
    ) -> Result<()> {
        send(manager, &self.ws_config, ws::unsubscribe_private(id, topic))
    }

    /// Answers a server `{"event":"ping"}`.
    pub fn pong(&self, manager: &Arc<Manager>) -> Result<()> {
        send(manager, &self.ws_config, ws::PONG.to_string())
    }
}

/// Public REST market data.
#[async_trait]
pub trait OrderlyApi {
    async fn get_symbols(&self) -> ApiResult<Vec<OrderlySymbol>>;
    async fn get_symbol(&self, symbol: &str) -> ApiResult<OrderlySymbol>;
    async fn get_futures(&self) -> ApiResult<Vec<OrderlyFuture>>;
    async fn get_future(&self, symbol: &str) -> ApiResult<OrderlyFuture>;
    async fn get_market_trades(
        &self,
        symbol: &str,
        limit: Option<u32>,
    ) -> ApiResult<Vec<OrderlyMarketTrade>>;
    async fn get_funding_rates(&self) -> ApiResult<Vec<OrderlyFundingRate>>;
    async fn get_funding_rate(&self, symbol: &str) -> ApiResult<OrderlyFundingRate>;
    async fn get_funding_rate_history(
        &self,
        symbol: &str,
        start_t: Option<u64>,
        end_t: Option<u64>,
        page: Option<u32>,
        size: Option<u32>,
    ) -> ApiResult<OrderlyPage<OrderlyFundingRateHistory>>;
    async fn get_funding_summary(&self) -> ApiResult<Vec<OrderlyFundingSummary>>;
    async fn get_price_changes(&self) -> ApiResult<Vec<OrderlyPriceChanges>>;
    async fn get_traders_open_interests(&self) -> ApiResult<Vec<OrderlyTradersOpenInterest>>;
    async fn get_kline_history(
        &self,
        symbol: &str,
        resolution: &str,
        from: i64,
        to: i64,
        limit: Option<u32>,
    ) -> ApiResult<OrderlyTvBars>;
    async fn get_tv_history(
        &self,
        symbol: &str,
        resolution: &str,
        from: i64,
        to: i64,
    ) -> ApiResult<OrderlyTvBars>;
    async fn get_system_info(&self) -> ApiResult<OrderlySystemInfo>;
    async fn get_history_charts(&self, time_interval: &str) -> ApiResult<Vec<OrderlyHistoryChart>>;
    async fn get_index_price_history(
        &self,
        symbol: &str,
        start_t: u64,
        end_t: u64,
        limit: Option<u32>,
    ) -> ApiResult<Vec<OrderlyIndexPricePoint>>;
    async fn get_index_price_sources(&self) -> ApiResult<Vec<OrderlyIndexPriceSource>>;
    async fn get_insurance_fund(&self) -> ApiResult<OrderlyInsuranceFund>;
    async fn get_leverage_config(&self) -> ApiResult<OrderlyLeverageConfig>;
    async fn get_max_leverage(&self) -> ApiResult<OrderlyMaxLeverage>;
    async fn get_liquidated_positions(
        &self,
        symbol: Option<&str>,
        start_t: Option<u64>,
        end_t: Option<u64>,
        page: Option<u32>,
        size: Option<u32>,
    ) -> ApiResult<OrderlyPage<OrderlyLiquidatedPosition>>;
    async fn get_liquidations(
        &self,
        start_t: Option<u64>,
        end_t: Option<u64>,
        page: Option<u32>,
        size: Option<u32>,
    ) -> ApiResult<OrderlyPage<OrderlyLiquidation>>;
    async fn get_tokens(&self, chain_id: Option<&str>) -> ApiResult<Vec<OrderlyToken>>;
    async fn get_chain_info(&self, broker_id: Option<&str>) -> ApiResult<Vec<OrderlyChainInfo>>;
    async fn get_binary_rounds(&self) -> ApiResult<Vec<OrderlyBinaryRound>>;
    async fn get_binary_round(&self, symbol: &str) -> ApiResult<OrderlyBinaryRoundDetail>;
    async fn get_tv_symbol_info(&self, group: &str) -> ApiResult<OrderlyTvSymbolInfo>;
    async fn get_tv_config(&self, locale: &str) -> ApiResult<OrderlyTvConfig>;
    async fn query_market_summary(
        &self,
        symbols: Option<&[&str]>,
    ) -> ApiResult<OrderlyQueryMarketSummary>;
    async fn query_orderbook(
        &self,
        symbol: &str,
        max_level: Option<u32>,
    ) -> ApiResult<OrderlyQueryOrderbook>;
    async fn query_candles(
        &self,
        symbol: &str,
        interval: &str,
        start_time: Option<u64>,
        end_time: Option<u64>,
        limit: Option<u32>,
        cursor: Option<&str>,
    ) -> ApiResult<OrderlyQueryPage<OrderlyQueryCandle>>;
    async fn query_market_trades(
        &self,
        symbol: &str,
        limit: Option<u32>,
    ) -> ApiResult<Vec<OrderlyQueryTrade>>;
    async fn query_liquidations(
        &self,
        symbol: Option<&str>,
        limit: Option<u32>,
        cursor: Option<&str>,
    ) -> ApiResult<OrderlyQueryPage<OrderlyQueryLiquidation>>;
    async fn query_funding_rate_history(
        &self,
        symbol: &str,
        start_time: Option<u64>,
        end_time: Option<u64>,
        limit: Option<u32>,
        cursor: Option<&str>,
    ) -> ApiResult<OrderlyQueryPage<OrderlyQueryFundingRate>>;
    async fn query_funding_comparison(
        &self,
        symbol: Option<&str>,
    ) -> ApiResult<Vec<OrderlyQueryFundingComparison>>;
    async fn query_market_detail(
        &self,
        q: &OrderlyMarketDetailQuery<'_>,
    ) -> ApiResult<OrderlyQueryMarketDetail>;
}

#[async_trait]
impl OrderlyApi for OrderlyClient {
    async fn get_symbols(&self) -> ApiResult<Vec<OrderlySymbol>> {
        self.market_api.get_symbols().await
    }

    async fn get_symbol(&self, symbol: &str) -> ApiResult<OrderlySymbol> {
        self.market_api.get_symbol(symbol).await
    }

    async fn get_futures(&self) -> ApiResult<Vec<OrderlyFuture>> {
        self.market_api.get_futures().await
    }

    async fn get_future(&self, symbol: &str) -> ApiResult<OrderlyFuture> {
        self.market_api.get_future(symbol).await
    }

    async fn get_market_trades(
        &self,
        symbol: &str,
        limit: Option<u32>,
    ) -> ApiResult<Vec<OrderlyMarketTrade>> {
        self.market_api.get_market_trades(symbol, limit).await
    }

    async fn get_funding_rates(&self) -> ApiResult<Vec<OrderlyFundingRate>> {
        self.market_api.get_funding_rates().await
    }

    async fn get_funding_rate(&self, symbol: &str) -> ApiResult<OrderlyFundingRate> {
        self.market_api.get_funding_rate(symbol).await
    }

    async fn get_funding_rate_history(
        &self,
        symbol: &str,
        start_t: Option<u64>,
        end_t: Option<u64>,
        page: Option<u32>,
        size: Option<u32>,
    ) -> ApiResult<OrderlyPage<OrderlyFundingRateHistory>> {
        self.market_api
            .get_funding_rate_history(symbol, start_t, end_t, page, size)
            .await
    }

    async fn get_funding_summary(&self) -> ApiResult<Vec<OrderlyFundingSummary>> {
        self.market_api.get_funding_summary().await
    }

    async fn get_price_changes(&self) -> ApiResult<Vec<OrderlyPriceChanges>> {
        self.market_api.get_price_changes().await
    }

    async fn get_traders_open_interests(&self) -> ApiResult<Vec<OrderlyTradersOpenInterest>> {
        self.market_api.get_traders_open_interests().await
    }

    async fn get_kline_history(
        &self,
        symbol: &str,
        resolution: &str,
        from: i64,
        to: i64,
        limit: Option<u32>,
    ) -> ApiResult<OrderlyTvBars> {
        self.market_api
            .get_kline_history(symbol, resolution, from, to, limit)
            .await
    }

    async fn get_tv_history(
        &self,
        symbol: &str,
        resolution: &str,
        from: i64,
        to: i64,
    ) -> ApiResult<OrderlyTvBars> {
        self.market_api
            .get_tv_history(symbol, resolution, from, to)
            .await
    }

    async fn get_system_info(&self) -> ApiResult<OrderlySystemInfo> {
        self.market_api.get_system_info().await
    }

    async fn get_history_charts(&self, time_interval: &str) -> ApiResult<Vec<OrderlyHistoryChart>> {
        self.market_api.get_history_charts(time_interval).await
    }

    async fn get_index_price_history(
        &self,
        symbol: &str,
        start_t: u64,
        end_t: u64,
        limit: Option<u32>,
    ) -> ApiResult<Vec<OrderlyIndexPricePoint>> {
        self.market_api
            .get_index_price_history(symbol, start_t, end_t, limit)
            .await
    }

    async fn get_index_price_sources(&self) -> ApiResult<Vec<OrderlyIndexPriceSource>> {
        self.market_api.get_index_price_sources().await
    }

    async fn get_insurance_fund(&self) -> ApiResult<OrderlyInsuranceFund> {
        self.market_api.get_insurance_fund().await
    }

    async fn get_leverage_config(&self) -> ApiResult<OrderlyLeverageConfig> {
        self.market_api.get_leverage_config().await
    }

    async fn get_max_leverage(&self) -> ApiResult<OrderlyMaxLeverage> {
        self.market_api.get_max_leverage().await
    }

    async fn get_liquidated_positions(
        &self,
        symbol: Option<&str>,
        start_t: Option<u64>,
        end_t: Option<u64>,
        page: Option<u32>,
        size: Option<u32>,
    ) -> ApiResult<OrderlyPage<OrderlyLiquidatedPosition>> {
        self.market_api
            .get_liquidated_positions(symbol, start_t, end_t, page, size)
            .await
    }

    async fn get_liquidations(
        &self,
        start_t: Option<u64>,
        end_t: Option<u64>,
        page: Option<u32>,
        size: Option<u32>,
    ) -> ApiResult<OrderlyPage<OrderlyLiquidation>> {
        self.market_api
            .get_liquidations(start_t, end_t, page, size)
            .await
    }

    async fn get_tokens(&self, chain_id: Option<&str>) -> ApiResult<Vec<OrderlyToken>> {
        self.market_api.get_tokens(chain_id).await
    }

    async fn get_chain_info(&self, broker_id: Option<&str>) -> ApiResult<Vec<OrderlyChainInfo>> {
        self.market_api.get_chain_info(broker_id).await
    }

    async fn get_binary_rounds(&self) -> ApiResult<Vec<OrderlyBinaryRound>> {
        self.market_api.get_binary_rounds().await
    }

    async fn get_binary_round(&self, symbol: &str) -> ApiResult<OrderlyBinaryRoundDetail> {
        self.market_api.get_binary_round(symbol).await
    }

    async fn get_tv_symbol_info(&self, group: &str) -> ApiResult<OrderlyTvSymbolInfo> {
        self.market_api.get_tv_symbol_info(group).await
    }

    async fn get_tv_config(&self, locale: &str) -> ApiResult<OrderlyTvConfig> {
        self.market_api.get_tv_config(locale).await
    }

    async fn query_market_summary(
        &self,
        symbols: Option<&[&str]>,
    ) -> ApiResult<OrderlyQueryMarketSummary> {
        self.market_api.query_market_summary(symbols).await
    }

    async fn query_orderbook(
        &self,
        symbol: &str,
        max_level: Option<u32>,
    ) -> ApiResult<OrderlyQueryOrderbook> {
        self.market_api.query_orderbook(symbol, max_level).await
    }

    async fn query_candles(
        &self,
        symbol: &str,
        interval: &str,
        start_time: Option<u64>,
        end_time: Option<u64>,
        limit: Option<u32>,
        cursor: Option<&str>,
    ) -> ApiResult<OrderlyQueryPage<OrderlyQueryCandle>> {
        self.market_api
            .query_candles(symbol, interval, start_time, end_time, limit, cursor)
            .await
    }

    async fn query_market_trades(
        &self,
        symbol: &str,
        limit: Option<u32>,
    ) -> ApiResult<Vec<OrderlyQueryTrade>> {
        self.market_api.query_market_trades(symbol, limit).await
    }

    async fn query_liquidations(
        &self,
        symbol: Option<&str>,
        limit: Option<u32>,
        cursor: Option<&str>,
    ) -> ApiResult<OrderlyQueryPage<OrderlyQueryLiquidation>> {
        self.market_api
            .query_liquidations(symbol, limit, cursor)
            .await
    }

    async fn query_funding_rate_history(
        &self,
        symbol: &str,
        start_time: Option<u64>,
        end_time: Option<u64>,
        limit: Option<u32>,
        cursor: Option<&str>,
    ) -> ApiResult<OrderlyQueryPage<OrderlyQueryFundingRate>> {
        self.market_api
            .query_funding_rate_history(symbol, start_time, end_time, limit, cursor)
            .await
    }

    async fn query_funding_comparison(
        &self,
        symbol: Option<&str>,
    ) -> ApiResult<Vec<OrderlyQueryFundingComparison>> {
        self.market_api.query_funding_comparison(symbol).await
    }

    async fn query_market_detail(
        &self,
        q: &OrderlyMarketDetailQuery<'_>,
    ) -> ApiResult<OrderlyQueryMarketDetail> {
        self.market_api.query_market_detail(q).await
    }
}
