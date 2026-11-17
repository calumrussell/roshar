//! Nado (Ink) client: public market data (gateway queries, archive, V2 REST, subscriptions
//! websocket), EIP-712 signed executes and the account reads needed to trade.

/// Wire and message types (REST replies, websocket frames).
pub mod types;

/// EIP-712 signing of executes and stream authentication.
pub mod signer;

pub mod rest;

pub use rest::trading::NadoMaxOrderSizeQuery;
pub use rest::{
    parse_archive_reply, parse_edge_control_reply, parse_execute_reply, parse_query_reply,
    ApiResult, BoxError, MarketApi as NadoMarketApi, NadoApiError, TradingApi as NadoTradingApi,
};

use crate::types::{
    NadoAllProducts, NadoCandlesticks, NadoCandlesticksQuery, NadoFundingRate,
    NadoFundingRateHistory, NadoFundingRateHistoryQuery, NadoFundingRates, NadoMarketLiquidity,
    NadoOrderbook, NadoProductType, NadoStream, NadoSymbols, NadoTickers, NadoTrade,
};
use anyhow::Result;
use async_trait::async_trait;
use rest::MarketApi;
use roshar_ws_mgr::{Config, Manager, Message};
use std::sync::Arc;

// Unified endpoints (https://docs.nado.xyz/developer-resources/api/endpoints).
pub const NADO_GATEWAY_URL: &str = "https://api.prod.nado.xyz/gateway/v1";
pub const NADO_GATEWAY_V2_URL: &str = "https://api.prod.nado.xyz/gateway/v2";
pub const NADO_ARCHIVE_URL: &str = "https://api.prod.nado.xyz/archive/v1";
pub const NADO_ARCHIVE_V2_URL: &str = "https://api.prod.nado.xyz/archive/v2";
/// Gateway websocket for executes and queries (same payloads as REST).
pub const NADO_WS_URL: &str = "wss://api.prod.nado.xyz/gateway/v1/ws";
/// Gateway websocket v2: concurrent dispatch, responses correlated by `id`.
pub const NADO_WS_V2_URL: &str = "wss://api.prod.nado.xyz/gateway/ws/v2";
/// Subscriptions. Rejects (HTTP 403) upgrades without `Sec-WebSocket-Extensions:
/// permessage-deflate`, which roshar-ws-mgr does not implement.
pub const NADO_SUBSCRIBE_URL: &str = "wss://api.prod.nado.xyz/gateway/v1/subscribe";
/// Subscriptions on the direct (Tokyo) host. Accepts uncompressed connections; the default for
/// `NadoClient::ws_config`.
pub const NADO_DIRECT_SUBSCRIBE_URL: &str =
    "wss://direct-gateway.prod.nado-backend.xyz/v1/subscribe";

pub const NADO_TESTNET_GATEWAY_URL: &str = "https://api.test.nado.xyz/gateway/v1";
pub const NADO_TESTNET_GATEWAY_V2_URL: &str = "https://api.test.nado.xyz/gateway/v2";
pub const NADO_TESTNET_ARCHIVE_URL: &str = "https://api.test.nado.xyz/archive/v1";
pub const NADO_TESTNET_ARCHIVE_V2_URL: &str = "https://api.test.nado.xyz/archive/v2";
pub const NADO_TESTNET_WS_URL: &str = "wss://api.test.nado.xyz/gateway/v1/ws";
pub const NADO_TESTNET_WS_V2_URL: &str = "wss://api.test.nado.xyz/gateway/ws/v2";
pub const NADO_TESTNET_SUBSCRIBE_URL: &str = "wss://api.test.nado.xyz/gateway/v1/subscribe";
pub const NADO_TESTNET_DIRECT_SUBSCRIBE_URL: &str =
    "wss://direct-gateway.test.nado-backend.xyz/v1/subscribe";

/// Mainnet (Ink) or testnet (Ink Sepolia).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NadoNetwork {
    Mainnet,
    Testnet,
}

impl NadoNetwork {
    pub fn gateway_url(self) -> &'static str {
        match self {
            Self::Mainnet => NADO_GATEWAY_URL,
            Self::Testnet => NADO_TESTNET_GATEWAY_URL,
        }
    }

    pub fn gateway_v2_url(self) -> &'static str {
        match self {
            Self::Mainnet => NADO_GATEWAY_V2_URL,
            Self::Testnet => NADO_TESTNET_GATEWAY_V2_URL,
        }
    }

    pub fn archive_url(self) -> &'static str {
        match self {
            Self::Mainnet => NADO_ARCHIVE_URL,
            Self::Testnet => NADO_TESTNET_ARCHIVE_URL,
        }
    }

    pub fn archive_v2_url(self) -> &'static str {
        match self {
            Self::Mainnet => NADO_ARCHIVE_V2_URL,
            Self::Testnet => NADO_TESTNET_ARCHIVE_V2_URL,
        }
    }

    pub fn ws_url(self) -> &'static str {
        match self {
            Self::Mainnet => NADO_WS_URL,
            Self::Testnet => NADO_TESTNET_WS_URL,
        }
    }

    pub fn direct_subscribe_url(self) -> &'static str {
        match self {
            Self::Mainnet => NADO_DIRECT_SUBSCRIBE_URL,
            Self::Testnet => NADO_TESTNET_DIRECT_SUBSCRIBE_URL,
        }
    }
}

/// Market-data REST plus the subscriptions websocket config for roshar-ws-mgr.
pub struct NadoClient {
    market_api: MarketApi,
    ws_config: Config,
}

impl NadoClient {
    /// Gateway and archive queries each allow 2400 weight / min (400 / 10 s) per IP; most
    /// market queries weigh 1-2, `all_products` 5. 20 requests / s keeps weight-2 queries within
    /// both windows.
    const DEFAULT_REQUESTS_PER_SECOND: u32 = 20;
    const WS_CONN_NAME: &str = "nado";

    pub fn new() -> Self {
        Self::new_with_rate_limit(NadoNetwork::Mainnet, Self::DEFAULT_REQUESTS_PER_SECOND)
    }

    pub fn new_with_rate_limit(network: NadoNetwork, requests_per_second: u32) -> Self {
        Self {
            market_api: MarketApi::new(network, requests_per_second),
            ws_config: Config {
                name: Self::WS_CONN_NAME.to_string(),
                url: network.direct_subscribe_url().to_string(),
                // The server requires a protocol ping frame at least every 30 s.
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

    pub fn market_api(&self) -> &MarketApi {
        &self.market_api
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

    /// Subscribe to any stream. Authenticated streams (`order_update`) need
    /// [`NadoClient::authenticate`] first.
    pub fn subscribe(
        &self,
        manager: &Arc<Manager>,
        stream: &NadoStream,
        id: Option<u64>,
    ) -> Result<()> {
        self.send(manager, stream.subscribe_message(id))
    }

    pub fn unsubscribe(
        &self,
        manager: &Arc<Manager>,
        stream: &NadoStream,
        id: Option<u64>,
    ) -> Result<()> {
        self.send(manager, stream.unsubscribe_message(id))
    }

    /// `subscribe_multi`: several streams in one frame. The server enables it per IP; others get
    /// an error reply.
    pub fn subscribe_multi(
        &self,
        manager: &Arc<Manager>,
        streams: &[NadoStream],
        id: Option<u64>,
    ) -> Result<()> {
        self.send(manager, types::subscribe_multi_message(streams, id))
    }

    /// `unsubscribe_multi`: see [`NadoClient::subscribe_multi`].
    pub fn unsubscribe_multi(
        &self,
        manager: &Arc<Manager>,
        streams: &[NadoStream],
        id: Option<u64>,
    ) -> Result<()> {
        self.send(manager, types::unsubscribe_multi_message(streams, id))
    }

    /// Send a signed `authenticate` frame from [`signer::Signer::stream_auth_message`].
    pub fn authenticate(&self, manager: &Arc<Manager>, auth_message: String) -> Result<()> {
        self.send(manager, auth_message)
    }

    fn for_products(
        &self,
        manager: &Arc<Manager>,
        product_ids: &[u32],
        id: Option<u64>,
        subscribe: bool,
        make: impl Fn(u32) -> NadoStream,
    ) -> Result<()> {
        for &product_id in product_ids {
            let stream = make(product_id);
            let text = if subscribe {
                stream.subscribe_message(id)
            } else {
                stream.unsubscribe_message(id)
            };
            self.send(manager, text)?;
        }
        Ok(())
    }

    /// `book_depth`: incremental depth diffs. Seed from `market_liquidity`.
    pub fn subscribe_book_depth(
        &self,
        manager: &Arc<Manager>,
        product_ids: &[u32],
        id: Option<u64>,
    ) -> Result<()> {
        self.for_products(manager, product_ids, id, true, |product_id| {
            NadoStream::BookDepth { product_id }
        })
    }

    pub fn unsubscribe_book_depth(
        &self,
        manager: &Arc<Manager>,
        product_ids: &[u32],
        id: Option<u64>,
    ) -> Result<()> {
        self.for_products(manager, product_ids, id, false, |product_id| {
            NadoStream::BookDepth { product_id }
        })
    }

    pub fn subscribe_trades(
        &self,
        manager: &Arc<Manager>,
        product_ids: &[u32],
        id: Option<u64>,
    ) -> Result<()> {
        self.for_products(manager, product_ids, id, true, |product_id| {
            NadoStream::Trade { product_id }
        })
    }

    pub fn unsubscribe_trades(
        &self,
        manager: &Arc<Manager>,
        product_ids: &[u32],
        id: Option<u64>,
    ) -> Result<()> {
        self.for_products(manager, product_ids, id, false, |product_id| {
            NadoStream::Trade { product_id }
        })
    }

    /// `best_bid_offer`: top of book with quantities, on change.
    pub fn subscribe_bbo(
        &self,
        manager: &Arc<Manager>,
        product_ids: &[u32],
        id: Option<u64>,
    ) -> Result<()> {
        self.for_products(manager, product_ids, id, true, |product_id| {
            NadoStream::BestBidOffer { product_id }
        })
    }

    pub fn unsubscribe_bbo(
        &self,
        manager: &Arc<Manager>,
        product_ids: &[u32],
        id: Option<u64>,
    ) -> Result<()> {
        self.for_products(manager, product_ids, id, false, |product_id| {
            NadoStream::BestBidOffer { product_id }
        })
    }

    /// `latest_candlestick` at `granularity` seconds.
    pub fn subscribe_candles(
        &self,
        manager: &Arc<Manager>,
        product_ids: &[u32],
        granularity: u32,
        id: Option<u64>,
    ) -> Result<()> {
        self.for_products(manager, product_ids, id, true, |product_id| {
            NadoStream::LatestCandlestick {
                product_id,
                granularity,
            }
        })
    }

    pub fn unsubscribe_candles(
        &self,
        manager: &Arc<Manager>,
        product_ids: &[u32],
        granularity: u32,
        id: Option<u64>,
    ) -> Result<()> {
        self.for_products(manager, product_ids, id, false, |product_id| {
            NadoStream::LatestCandlestick {
                product_id,
                granularity,
            }
        })
    }

    /// `funding_rate` (~20 s recalculations); `None` subscribes to every perp.
    pub fn subscribe_funding_rate(
        &self,
        manager: &Arc<Manager>,
        product_id: Option<u32>,
        id: Option<u64>,
    ) -> Result<()> {
        self.subscribe(manager, &NadoStream::FundingRate { product_id }, id)
    }

    pub fn unsubscribe_funding_rate(
        &self,
        manager: &Arc<Manager>,
        product_id: Option<u32>,
        id: Option<u64>,
    ) -> Result<()> {
        self.unsubscribe(manager, &NadoStream::FundingRate { product_id }, id)
    }

    /// `funding_payment` (hourly).
    pub fn subscribe_funding_payment(
        &self,
        manager: &Arc<Manager>,
        product_ids: &[u32],
        id: Option<u64>,
    ) -> Result<()> {
        self.for_products(manager, product_ids, id, true, |product_id| {
            NadoStream::FundingPayment { product_id }
        })
    }

    pub fn unsubscribe_funding_payment(
        &self,
        manager: &Arc<Manager>,
        product_ids: &[u32],
        id: Option<u64>,
    ) -> Result<()> {
        self.for_products(manager, product_ids, id, false, |product_id| {
            NadoStream::FundingPayment { product_id }
        })
    }

    /// `all_bbo`: top of book for every product, ~300 ms snapshots.
    pub fn subscribe_all_bbo(&self, manager: &Arc<Manager>, id: Option<u64>) -> Result<()> {
        self.subscribe(manager, &NadoStream::AllBbo, id)
    }

    pub fn unsubscribe_all_bbo(&self, manager: &Arc<Manager>, id: Option<u64>) -> Result<()> {
        self.unsubscribe(manager, &NadoStream::AllBbo, id)
    }
}

impl Default for NadoClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Core market-data REST calls. The full set is on [`NadoMarketApi`] via
/// [`NadoClient::market_api`].
#[async_trait]
pub trait NadoApi {
    async fn get_symbols(&self) -> ApiResult<NadoSymbols>;

    async fn get_all_products(&self) -> ApiResult<NadoAllProducts>;

    async fn get_tickers(&self, market: Option<NadoProductType>) -> ApiResult<NadoTickers>;

    async fn get_orderbook(&self, product_id: u32, depth: u32) -> ApiResult<NadoMarketLiquidity>;

    async fn get_v2_orderbook(&self, ticker_id: &str, depth: u32) -> ApiResult<NadoOrderbook>;

    async fn get_trades(
        &self,
        ticker_id: &str,
        limit: Option<u32>,
        max_trade_id: Option<u64>,
    ) -> ApiResult<Vec<NadoTrade>>;

    async fn get_candlesticks(&self, query: &NadoCandlesticksQuery) -> ApiResult<NadoCandlesticks>;

    async fn get_funding_rate(&self, product_id: u32) -> ApiResult<NadoFundingRate>;

    async fn get_funding_rates(&self, product_ids: &[u32]) -> ApiResult<NadoFundingRates>;

    async fn get_funding_rate_history(
        &self,
        query: &NadoFundingRateHistoryQuery,
    ) -> ApiResult<NadoFundingRateHistory>;
}

#[async_trait]
impl NadoApi for NadoClient {
    async fn get_symbols(&self) -> ApiResult<NadoSymbols> {
        self.market_api.symbols(None, None).await
    }

    async fn get_all_products(&self) -> ApiResult<NadoAllProducts> {
        self.market_api.all_products().await
    }

    async fn get_tickers(&self, market: Option<NadoProductType>) -> ApiResult<NadoTickers> {
        self.market_api.tickers(market, None).await
    }

    async fn get_orderbook(&self, product_id: u32, depth: u32) -> ApiResult<NadoMarketLiquidity> {
        self.market_api.market_liquidity(product_id, depth).await
    }

    async fn get_v2_orderbook(&self, ticker_id: &str, depth: u32) -> ApiResult<NadoOrderbook> {
        self.market_api.orderbook(ticker_id, depth).await
    }

    async fn get_trades(
        &self,
        ticker_id: &str,
        limit: Option<u32>,
        max_trade_id: Option<u64>,
    ) -> ApiResult<Vec<NadoTrade>> {
        self.market_api.trades(ticker_id, limit, max_trade_id).await
    }

    async fn get_candlesticks(&self, query: &NadoCandlesticksQuery) -> ApiResult<NadoCandlesticks> {
        self.market_api.candlesticks(query).await
    }

    async fn get_funding_rate(&self, product_id: u32) -> ApiResult<NadoFundingRate> {
        self.market_api.funding_rate(product_id).await
    }

    async fn get_funding_rates(&self, product_ids: &[u32]) -> ApiResult<NadoFundingRates> {
        self.market_api.funding_rates(product_ids).await
    }

    async fn get_funding_rate_history(
        &self,
        query: &NadoFundingRateHistoryQuery,
    ) -> ApiResult<NadoFundingRateHistory> {
        self.market_api.funding_rate_history(query).await
    }
}
