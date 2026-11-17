//! Synthetix perps (offchain orderbook, Ethereum mainnet settlement): public market data over
//! `POST /v1/info` and the aggregator `GET` endpoints, websocket frames, EIP-712 request signing
//! and `POST /v1/trade` submission.
//!
//! Every REST request and websocket upgrade must carry a descriptive `User-Agent` (HTTP 403
//! otherwise), so the clients take one.

/// EIP-712 signing of trade actions and websocket auth.
pub mod signer;
/// Wire and message types (REST payloads, websocket pushes).
pub mod types;
/// Websocket request frames and URLs.
pub mod ws;

pub(crate) mod rest;

pub use rest::{
    MarketApi as SynthetixMarketApi, TradeApi, SYNTHETIX_API_URL, SYNTHETIX_INFO_URL,
    SYNTHETIX_TRADE_URL,
};
pub use ws::{SYNTHETIX_INFO_WSS_URL, SYNTHETIX_TRADE_WSS_URL, SYNTHETIX_WS_PING};

use crate::signer::{SignedRequest, WsAuth};
use crate::types::{
    SynthetixAggregatorOrderBook, SynthetixCandles, SynthetixCollateralConfig, SynthetixContract,
    SynthetixContractSpec, SynthetixExchangeStatus, SynthetixFundingRate,
    SynthetixFundingRateHistory, SynthetixLastTrades, SynthetixMarket, SynthetixMarketPrice,
    SynthetixOpenInterest, SynthetixOrderBook, SynthetixStatus,
    SynthetixSubAccountIdsWithDelegations, SynthetixTier,
};
use anyhow::Result;
use async_trait::async_trait;
use rest::MarketApi;
use roshar_ws_mgr::{Config, Manager, Message};
use std::collections::HashMap;
use std::sync::Arc;

type ApiResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

pub struct SynthetixClient {
    market_api: MarketApi,
    ws_config: Config,
    trade_ws_config: Config,
}

impl SynthetixClient {
    /// Per-IP budget: 10,000 tokens per 10 s across REST and websocket. Info actions cost 50-250
    /// tokens (`getFundingRateHistory` 1,000); 4 req/s at the common 250-token cost stays inside it.
    const DEFAULT_REQUESTS_PER_SECOND: u32 = 4;
    const WS_CONN_NAME: &str = "synthetix";
    const TRADE_WS_CONN_NAME: &str = "synthetix-trade";

    /// `user_agent` identifies the caller's application (e.g. `my-bot/1.0`); the exchange
    /// requires one on every request.
    pub fn new(user_agent: &str) -> Self {
        Self::new_with_rate_limit(user_agent, Self::DEFAULT_REQUESTS_PER_SECOND)
    }

    pub fn new_with_rate_limit(user_agent: &str, requests_per_second: u32) -> Self {
        Self {
            market_api: MarketApi::new(user_agent, requests_per_second),
            ws_config: Self::config(Self::WS_CONN_NAME, SYNTHETIX_INFO_WSS_URL, user_agent),
            trade_ws_config: Self::config(
                Self::TRADE_WS_CONN_NAME,
                SYNTHETIX_TRADE_WSS_URL,
                user_agent,
            ),
        }
    }

    fn config(name: &str, url: &str, user_agent: &str) -> Config {
        Config {
            name: name.to_string(),
            url: url.to_string(),
            // Docs: send an application-level `ping` every 30 s; connections are closed after
            // 24 h. Pongs are JSON text replies, so pong tracking is not used.
            ping_duration: 30,
            ping_message: SYNTHETIX_WS_PING.to_string(),
            ping_timeout: 60,
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
            headers: vec![("User-Agent".to_string(), user_agent.to_string())],
        }
    }

    /// Public market-data socket (`/v1/ws/info`).
    pub fn ws_config(&self) -> roshar_ws_mgr::Config {
        self.ws_config.clone()
    }

    pub fn ws_config_mut(&mut self) -> &mut roshar_ws_mgr::Config {
        &mut self.ws_config
    }

    /// Authenticated socket (`/v1/ws/trade`): send [`Self::authenticate`] within 30 s of
    /// connecting, then signed actions and `subAccountUpdates`.
    pub fn trade_ws_config(&self) -> roshar_ws_mgr::Config {
        self.trade_ws_config.clone()
    }

    pub fn trade_ws_config_mut(&mut self) -> &mut roshar_ws_mgr::Config {
        &mut self.trade_ws_config
    }

    fn send(&self, manager: &Arc<Manager>, config: &Config, text: String) -> Result<()> {
        manager.write(
            &config.name,
            Message::TextMessage(config.name.clone(), text),
        )?;
        Ok(())
    }

    fn send_info(&self, manager: &Arc<Manager>, text: String) -> Result<()> {
        self.send(manager, &self.ws_config, text)
    }

    fn send_trade(&self, manager: &Arc<Manager>, text: String) -> Result<()> {
        self.send(manager, &self.trade_ws_config, text)
    }

    /// `orderbook` for one symbol; `id` is echoed in the reply.
    pub fn subscribe_depth(
        &self,
        manager: &Arc<Manager>,
        id: &str,
        symbol: &str,
        options: ws::BookOptions,
    ) -> Result<()> {
        self.send_info(manager, ws::subscribe_orderbook(id, symbol, options))
    }

    pub fn unsubscribe_depth(&self, manager: &Arc<Manager>, id: &str, symbol: &str) -> Result<()> {
        self.send_info(manager, ws::unsubscribe_orderbook(id, symbol))
    }

    /// `trades`; `symbol: None` streams all markets.
    pub fn subscribe_trades(
        &self,
        manager: &Arc<Manager>,
        id: &str,
        symbol: Option<&str>,
    ) -> Result<()> {
        self.send_info(manager, ws::subscribe_trades(id, symbol))
    }

    pub fn unsubscribe_trades(
        &self,
        manager: &Arc<Manager>,
        id: &str,
        symbol: Option<&str>,
    ) -> Result<()> {
        self.send_info(manager, ws::unsubscribe_trades(id, symbol))
    }

    /// `marketPrices` (mark/last/index/mid ticks); `symbol: None` streams all markets.
    pub fn subscribe_market_prices(
        &self,
        manager: &Arc<Manager>,
        id: &str,
        symbol: Option<&str>,
    ) -> Result<()> {
        self.send_info(manager, ws::subscribe_market_prices(id, symbol))
    }

    pub fn unsubscribe_market_prices(
        &self,
        manager: &Arc<Manager>,
        id: &str,
        symbol: Option<&str>,
    ) -> Result<()> {
        self.send_info(manager, ws::unsubscribe_market_prices(id, symbol))
    }

    /// `candles` for one symbol and timeframe.
    pub fn subscribe_candles(
        &self,
        manager: &Arc<Manager>,
        id: &str,
        symbol: &str,
        timeframe: &str,
    ) -> Result<()> {
        self.send_info(manager, ws::subscribe_candles(id, symbol, timeframe))
    }

    pub fn unsubscribe_candles(
        &self,
        manager: &Arc<Manager>,
        id: &str,
        symbol: &str,
        timeframe: &str,
    ) -> Result<()> {
        self.send_info(manager, ws::unsubscribe_candles(id, symbol, timeframe))
    }

    /// Info socket: an unsigned market-data query as `method: "post"`; `id` is echoed in the
    /// reply. Parse it with [`types::SynthetixWsReply::into_post_response`].
    pub fn post_info(
        &self,
        manager: &Arc<Manager>,
        id: &str,
        request: &ws::InfoPost,
    ) -> Result<()> {
        self.send_info(manager, ws::post_info(id, request))
    }

    /// Trade socket: `auth` with a payload from
    /// [`Signer::sign_ws_auth`](crate::signer::Signer::sign_ws_auth).
    pub fn authenticate(&self, manager: &Arc<Manager>, id: &str, auth: &WsAuth) -> Result<()> {
        self.send_trade(manager, auth.ws_message(id))
    }

    /// Trade socket: a signed action as `method: "post"`.
    pub fn post(&self, manager: &Arc<Manager>, id: &str, req: &SignedRequest) -> Result<()> {
        self.send_trade(manager, req.ws_post(id))
    }

    /// Trade socket: `subAccountUpdates` (after `auth`).
    pub fn subscribe_sub_account_updates(
        &self,
        manager: &Arc<Manager>,
        id: &str,
        subaccount_id: u64,
    ) -> Result<()> {
        self.send_trade(
            manager,
            ws::subscribe_sub_account_updates(id, subaccount_id),
        )
    }

    pub fn unsubscribe_sub_account_updates(
        &self,
        manager: &Arc<Manager>,
        id: &str,
        subaccount_id: u64,
    ) -> Result<()> {
        self.send_trade(
            manager,
            ws::unsubscribe_sub_account_updates(id, subaccount_id),
        )
    }
}

/// Public market-data REST endpoints. See [`SynthetixMarketApi`] for parameter rules.
#[async_trait]
pub trait SynthetixApi {
    async fn get_markets(&self, active_only: Option<bool>) -> ApiResult<Vec<SynthetixMarket>>;

    async fn get_market_prices(&self) -> ApiResult<HashMap<String, SynthetixMarketPrice>>;

    async fn get_orderbook(
        &self,
        symbol: &str,
        limit: Option<u32>,
    ) -> ApiResult<SynthetixOrderBook>;

    async fn get_last_trades(
        &self,
        symbol: &str,
        limit: Option<u32>,
    ) -> ApiResult<SynthetixLastTrades>;

    async fn get_candles(
        &self,
        symbol: &str,
        interval: &str,
        start_time: Option<u64>,
        end_time: Option<u64>,
        limit: Option<u32>,
    ) -> ApiResult<SynthetixCandles>;

    async fn get_funding_rate(&self, symbol: &str) -> ApiResult<SynthetixFundingRate>;

    async fn get_funding_rate_history(
        &self,
        symbol: &str,
        start_time: u64,
        end_time: u64,
        limit: Option<u32>,
    ) -> ApiResult<SynthetixFundingRateHistory>;

    async fn get_open_interest(&self) -> ApiResult<Vec<SynthetixOpenInterest>>;

    async fn get_mids(&self) -> ApiResult<HashMap<String, String>>;

    async fn get_collaterals(&self) -> ApiResult<Vec<SynthetixCollateralConfig>>;

    async fn get_tiers(&self) -> ApiResult<Vec<SynthetixTier>>;

    async fn get_is_whitelisted(&self, wallet_address: &str) -> ApiResult<bool>;

    async fn get_sub_account_ids(&self, wallet_address: &str) -> ApiResult<Vec<String>>;

    async fn get_sub_account_ids_with_delegations(
        &self,
        wallet_address: &str,
    ) -> ApiResult<SynthetixSubAccountIdsWithDelegations>;

    async fn get_contracts(&self) -> ApiResult<Vec<SynthetixContract>>;

    async fn get_contract_specs(&self) -> ApiResult<Vec<SynthetixContractSpec>>;

    async fn get_aggregator_orderbook(
        &self,
        ticker_id: &str,
        depth: Option<u32>,
    ) -> ApiResult<SynthetixAggregatorOrderBook>;

    async fn get_status(&self) -> ApiResult<SynthetixStatus>;

    async fn get_exchange_status(&self) -> ApiResult<SynthetixExchangeStatus>;

    async fn get_ws_exchange_status(&self) -> ApiResult<SynthetixExchangeStatus>;
}

#[async_trait]
impl SynthetixApi for SynthetixClient {
    async fn get_markets(&self, active_only: Option<bool>) -> ApiResult<Vec<SynthetixMarket>> {
        self.market_api.get_markets(active_only).await
    }

    async fn get_market_prices(&self) -> ApiResult<HashMap<String, SynthetixMarketPrice>> {
        self.market_api.get_market_prices().await
    }

    async fn get_orderbook(
        &self,
        symbol: &str,
        limit: Option<u32>,
    ) -> ApiResult<SynthetixOrderBook> {
        self.market_api.get_orderbook(symbol, limit).await
    }

    async fn get_last_trades(
        &self,
        symbol: &str,
        limit: Option<u32>,
    ) -> ApiResult<SynthetixLastTrades> {
        self.market_api.get_last_trades(symbol, limit).await
    }

    async fn get_candles(
        &self,
        symbol: &str,
        interval: &str,
        start_time: Option<u64>,
        end_time: Option<u64>,
        limit: Option<u32>,
    ) -> ApiResult<SynthetixCandles> {
        self.market_api
            .get_candles(symbol, interval, start_time, end_time, limit)
            .await
    }

    async fn get_funding_rate(&self, symbol: &str) -> ApiResult<SynthetixFundingRate> {
        self.market_api.get_funding_rate(symbol).await
    }

    async fn get_funding_rate_history(
        &self,
        symbol: &str,
        start_time: u64,
        end_time: u64,
        limit: Option<u32>,
    ) -> ApiResult<SynthetixFundingRateHistory> {
        self.market_api
            .get_funding_rate_history(symbol, start_time, end_time, limit)
            .await
    }

    async fn get_open_interest(&self) -> ApiResult<Vec<SynthetixOpenInterest>> {
        self.market_api.get_open_interest().await
    }

    async fn get_mids(&self) -> ApiResult<HashMap<String, String>> {
        self.market_api.get_mids().await
    }

    async fn get_collaterals(&self) -> ApiResult<Vec<SynthetixCollateralConfig>> {
        self.market_api.get_collaterals().await
    }

    async fn get_tiers(&self) -> ApiResult<Vec<SynthetixTier>> {
        self.market_api.get_tiers().await
    }

    async fn get_is_whitelisted(&self, wallet_address: &str) -> ApiResult<bool> {
        self.market_api.get_is_whitelisted(wallet_address).await
    }

    async fn get_sub_account_ids(&self, wallet_address: &str) -> ApiResult<Vec<String>> {
        self.market_api.get_sub_account_ids(wallet_address).await
    }

    async fn get_sub_account_ids_with_delegations(
        &self,
        wallet_address: &str,
    ) -> ApiResult<SynthetixSubAccountIdsWithDelegations> {
        self.market_api
            .get_sub_account_ids_with_delegations(wallet_address)
            .await
    }

    async fn get_contracts(&self) -> ApiResult<Vec<SynthetixContract>> {
        self.market_api.get_contracts().await
    }

    async fn get_contract_specs(&self) -> ApiResult<Vec<SynthetixContractSpec>> {
        self.market_api.get_contract_specs().await
    }

    async fn get_aggregator_orderbook(
        &self,
        ticker_id: &str,
        depth: Option<u32>,
    ) -> ApiResult<SynthetixAggregatorOrderBook> {
        self.market_api
            .get_aggregator_orderbook(ticker_id, depth)
            .await
    }

    async fn get_status(&self) -> ApiResult<SynthetixStatus> {
        self.market_api.get_status().await
    }

    async fn get_exchange_status(&self) -> ApiResult<SynthetixExchangeStatus> {
        self.market_api.get_exchange_status().await
    }

    async fn get_ws_exchange_status(&self) -> ApiResult<SynthetixExchangeStatus> {
        self.market_api.get_ws_exchange_status().await
    }
}
