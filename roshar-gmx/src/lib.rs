//! GMX v2 client: GMX API and Oracle API reads, the GMX API stream, and express order
//! signing/submission through the GMX API order transaction routes.

/// Wire and message types (GMX API, Oracle API, order transactions, stream).
pub mod types;

pub mod exchange;
pub(crate) mod rest;
pub mod signer;

pub use exchange::OrderTxApi;
pub use rest::{MarketApi as GmxMarketApi, OracleApi as GmxOracleApi};

use crate::types::{
    GmxCandlePeriod, GmxDirection, GmxMarket, GmxMarketConfig, GmxMarketInfo, GmxMarketRates,
    GmxMarketTicker, GmxMarketValues, GmxOhlcvCandle, GmxOrder, GmxPair, GmxPeriod, GmxPosition,
    GmxRiskOracleMarket, GmxToken, GmxTokenInfo, GmxTokenPrices, GmxTradesList, GmxTradesQuery,
    GmxTradesSearchRequest, GmxTradingCapacity, GmxWalletBalance, GmxWsChannel, GmxWsRequest,
};
use anyhow::Result;
use async_trait::async_trait;
use rest::MarketApi;
use roshar_ws_mgr::{Config, Manager, Message};
use std::sync::Arc;

// GMX API deployments. Each chain has two independent peers (`.io`, `.ai`); neither is primary,
// and they do not share order-transaction records. Paths carry their `/v1` or `/v2` prefix.
pub const GMX_ARBITRUM_API_URL: &str = "https://arbitrum.gmxapi.io";
pub const GMX_ARBITRUM_API_URL_AI: &str = "https://arbitrum.gmxapi.ai";
pub const GMX_AVALANCHE_API_URL: &str = "https://avalanche.gmxapi.io";
pub const GMX_AVALANCHE_API_URL_AI: &str = "https://avalanche.gmxapi.ai";
pub const GMX_MEGAETH_API_URL: &str = "https://megaeth.gmxapi.io";
pub const GMX_MEGAETH_API_URL_AI: &str = "https://megaeth.gmxapi.ai";

/// GMX API stream (`@gmx-io/sdk` `toStreamUrl`). Only the Arbitrum `.io` host serves it; the
/// other hosts answer the upgrade with 404.
pub const GMX_ARBITRUM_WSS_URL: &str = "wss://arbitrum.gmxapi.io/v1/stream";

// Oracle API (Arbitrum and Avalanche only).
pub const GMX_ARBITRUM_ORACLE_URL: &str = "https://arbitrum-api.gmxinfra.io";
pub const GMX_AVALANCHE_ORACLE_URL: &str = "https://avalanche-api.gmxinfra.io";

// EIP-155 chain ids (`@gmx-io/sdk` `configs/chainIds`).
pub const ARBITRUM_CHAIN_ID: u64 = 42161;
pub const AVALANCHE_CHAIN_ID: u64 = 43114;
pub const MEGAETH_CHAIN_ID: u64 = 4326;
pub const ARBITRUM_SEPOLIA_CHAIN_ID: u64 = 421614;

type ApiResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// GMX API reads plus the stream config for roshar-ws-mgr.
pub struct GmxClient {
    market_api: MarketApi,
    ws_config: Config,
}

impl GmxClient {
    /// The GMX API publishes no request limit, only that callers may get HTTP 429 and should
    /// back off (docs.gmx.io/docs/api/integration-guide, "Retries, timeouts, and fallback
    /// URLs"). The SDK applies none either; 5 req/s is a conservative default.
    pub const DEFAULT_REQUESTS_PER_SECOND: u32 = 5;
    const WS_CONN_NAME: &str = "gmx";

    /// `api_url` is a deployment root (e.g. [`GMX_ARBITRUM_API_URL`]); `wss_url` the stream
    /// (e.g. [`GMX_ARBITRUM_WSS_URL`]).
    pub fn new(api_url: &str, wss_url: &str) -> Self {
        Self::new_with_rate_limit(api_url, wss_url, Self::DEFAULT_REQUESTS_PER_SECOND)
    }

    pub fn new_with_rate_limit(api_url: &str, wss_url: &str, requests_per_second: u32) -> Self {
        Self {
            market_api: MarketApi::new(api_url, requests_per_second),
            ws_config: Config {
                name: Self::WS_CONN_NAME.to_string(),
                url: wss_url.to_string(),
                // The server sends protocol pings (~every 20-30 s) and answers client pings with
                // pongs; it has no JSON ping. The SDK additionally probes with an empty
                // subscribe every 15 s, which the server acks.
                ping_duration: 20,
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
            },
        }
    }

    pub fn ws_config(&self) -> roshar_ws_mgr::Config {
        self.ws_config.clone()
    }

    pub fn ws_config_mut(&mut self) -> &mut roshar_ws_mgr::Config {
        &mut self.ws_config
    }

    fn send(&self, manager: &Arc<Manager>, msg: GmxWsRequest) -> Result<()> {
        manager.write(
            &self.ws_config.name,
            Message::TextMessage(self.ws_config.name.clone(), msg.to_json()),
        )?;
        Ok(())
    }

    /// `{"op":"subscribe","channels":[...]}`. The server rate-limits client messages (it closed
    /// a connection with 1008 "message rate limit exceeded" after ~25 messages in a second),
    /// so subscribe to many channels in one call.
    pub fn subscribe(&self, manager: &Arc<Manager>, channels: &[GmxWsChannel<'_>]) -> Result<()> {
        self.send(manager, GmxWsRequest::subscribe(channels))
    }

    pub fn unsubscribe(&self, manager: &Arc<Manager>, channels: &[GmxWsChannel<'_>]) -> Result<()> {
        self.send(manager, GmxWsRequest::unsubscribe(channels))
    }

    /// `prices`: oracle min/max prices of every token.
    pub fn subscribe_prices(&self, manager: &Arc<Manager>) -> Result<()> {
        self.subscribe(manager, &[GmxWsChannel::Prices])
    }

    pub fn unsubscribe_prices(&self, manager: &Arc<Manager>) -> Result<()> {
        self.unsubscribe(manager, &[GmxWsChannel::Prices])
    }

    /// `candles:{symbol}:{period}` for each token symbol.
    pub fn subscribe_candles(
        &self,
        manager: &Arc<Manager>,
        symbols: &[&str],
        period: GmxCandlePeriod,
    ) -> Result<()> {
        let channels: Vec<_> = symbols
            .iter()
            .map(|&symbol| GmxWsChannel::Candles { symbol, period })
            .collect();
        self.subscribe(manager, &channels)
    }

    pub fn unsubscribe_candles(
        &self,
        manager: &Arc<Manager>,
        symbols: &[&str],
        period: GmxCandlePeriod,
    ) -> Result<()> {
        let channels: Vec<_> = symbols
            .iter()
            .map(|&symbol| GmxWsChannel::Candles { symbol, period })
            .collect();
        self.unsubscribe(manager, &channels)
    }
}

/// GMX API reads. Market data and per-account state (positions, orders, trades, balances) are
/// all public GETs keyed by address.
#[async_trait]
pub trait GmxApi {
    async fn get_markets(&self) -> ApiResult<Vec<GmxMarket>>;
    async fn get_markets_info(&self) -> ApiResult<Vec<GmxMarketInfo>>;
    async fn get_markets_config(&self) -> ApiResult<Vec<GmxMarketConfig>>;
    async fn get_markets_values(&self) -> ApiResult<Vec<GmxMarketValues>>;
    async fn get_risk_oracle_markets(&self) -> ApiResult<Vec<GmxRiskOracleMarket>>;
    async fn get_markets_tickers(
        &self,
        addresses: &[&str],
        symbols: &[&str],
    ) -> ApiResult<Vec<GmxMarketTicker>>;
    async fn get_trading_capacity(
        &self,
        symbol: &str,
        direction: GmxDirection,
    ) -> ApiResult<GmxTradingCapacity>;
    async fn get_tokens(&self) -> ApiResult<Vec<GmxToken>>;
    async fn get_tokens_info(&self) -> ApiResult<Vec<GmxTokenInfo>>;
    async fn get_prices(&self) -> ApiResult<GmxTokenPrices>;
    async fn get_ohlcv(
        &self,
        symbol: &str,
        timeframe: GmxCandlePeriod,
        limit: Option<u32>,
        since: Option<u64>,
    ) -> ApiResult<Vec<GmxOhlcvCandle>>;
    async fn get_rates(
        &self,
        period: Option<GmxPeriod>,
        average_by: Option<GmxPeriod>,
        market_address: Option<&str>,
    ) -> ApiResult<Vec<GmxMarketRates>>;
    async fn get_pairs(&self) -> ApiResult<Vec<GmxPair>>;
    async fn get_solana_pairs(&self) -> ApiResult<Vec<GmxPair>>;
    async fn get_positions(
        &self,
        address: &str,
        include_related_orders: bool,
    ) -> ApiResult<Vec<GmxPosition>>;
    async fn get_position(&self, key: &str, include_related_orders: bool)
        -> ApiResult<GmxPosition>;
    async fn get_orders(&self, address: &str) -> ApiResult<Vec<GmxOrder>>;
    async fn get_order(&self, key: &str) -> ApiResult<GmxOrder>;
    async fn get_trades(&self, query: &GmxTradesQuery) -> ApiResult<GmxTradesList>;
    async fn search_trades(&self, request: &GmxTradesSearchRequest) -> ApiResult<GmxTradesList>;
    async fn get_wallet_balances(&self, address: &str) -> ApiResult<Vec<GmxWalletBalance>>;
}

#[async_trait]
impl GmxApi for GmxClient {
    async fn get_markets(&self) -> ApiResult<Vec<GmxMarket>> {
        self.market_api.get_markets().await
    }

    async fn get_markets_info(&self) -> ApiResult<Vec<GmxMarketInfo>> {
        self.market_api.get_markets_info().await
    }

    async fn get_markets_config(&self) -> ApiResult<Vec<GmxMarketConfig>> {
        self.market_api.get_markets_config().await
    }

    async fn get_markets_values(&self) -> ApiResult<Vec<GmxMarketValues>> {
        self.market_api.get_markets_values().await
    }

    async fn get_risk_oracle_markets(&self) -> ApiResult<Vec<GmxRiskOracleMarket>> {
        self.market_api.get_risk_oracle_markets().await
    }

    async fn get_markets_tickers(
        &self,
        addresses: &[&str],
        symbols: &[&str],
    ) -> ApiResult<Vec<GmxMarketTicker>> {
        self.market_api
            .get_markets_tickers(addresses, symbols)
            .await
    }

    async fn get_trading_capacity(
        &self,
        symbol: &str,
        direction: GmxDirection,
    ) -> ApiResult<GmxTradingCapacity> {
        self.market_api
            .get_trading_capacity(symbol, direction)
            .await
    }

    async fn get_tokens(&self) -> ApiResult<Vec<GmxToken>> {
        self.market_api.get_tokens().await
    }

    async fn get_tokens_info(&self) -> ApiResult<Vec<GmxTokenInfo>> {
        self.market_api.get_tokens_info().await
    }

    async fn get_prices(&self) -> ApiResult<GmxTokenPrices> {
        self.market_api.get_prices().await
    }

    async fn get_ohlcv(
        &self,
        symbol: &str,
        timeframe: GmxCandlePeriod,
        limit: Option<u32>,
        since: Option<u64>,
    ) -> ApiResult<Vec<GmxOhlcvCandle>> {
        self.market_api
            .get_ohlcv(symbol, timeframe, limit, since)
            .await
    }

    async fn get_rates(
        &self,
        period: Option<GmxPeriod>,
        average_by: Option<GmxPeriod>,
        market_address: Option<&str>,
    ) -> ApiResult<Vec<GmxMarketRates>> {
        self.market_api
            .get_rates(period, average_by, market_address)
            .await
    }

    async fn get_pairs(&self) -> ApiResult<Vec<GmxPair>> {
        self.market_api.get_pairs().await
    }

    async fn get_solana_pairs(&self) -> ApiResult<Vec<GmxPair>> {
        self.market_api.get_solana_pairs().await
    }

    async fn get_positions(
        &self,
        address: &str,
        include_related_orders: bool,
    ) -> ApiResult<Vec<GmxPosition>> {
        self.market_api
            .get_positions(address, include_related_orders)
            .await
    }

    async fn get_position(
        &self,
        key: &str,
        include_related_orders: bool,
    ) -> ApiResult<GmxPosition> {
        self.market_api
            .get_position(key, include_related_orders)
            .await
    }

    async fn get_orders(&self, address: &str) -> ApiResult<Vec<GmxOrder>> {
        self.market_api.get_orders(address).await
    }

    async fn get_order(&self, key: &str) -> ApiResult<GmxOrder> {
        self.market_api.get_order(key).await
    }

    async fn get_trades(&self, query: &GmxTradesQuery) -> ApiResult<GmxTradesList> {
        self.market_api.get_trades(query).await
    }

    async fn search_trades(&self, request: &GmxTradesSearchRequest) -> ApiResult<GmxTradesList> {
        self.market_api.search_trades(request).await
    }

    async fn get_wallet_balances(&self, address: &str) -> ApiResult<Vec<GmxWalletBalance>> {
        self.market_api.get_wallet_balances(address).await
    }
}
