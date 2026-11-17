//! Bullet (bullet.xyz) client: Binance-FAPI-shaped REST market data and account reads,
//! websocket topics, and ed25519-signed borsh transactions for trading.

/// Transaction encoding and ed25519 signing (`SigningPayload::V0`).
pub mod signer;
/// Wire and message types (REST payloads, websocket topics and messages).
pub mod types;
/// Websocket transaction requests (`order.place`, `order.cancel`, ...).
pub mod ws_tx;

pub(crate) mod rest;

pub use rest::{
    AccountApi as BulletAccountApi, BulletRestError, MarketApi as BulletMarketApi, RestResult,
};

use crate::signer::SignedTransaction;
use crate::types::ws::{self, BulletDepthLevels, BulletKlineInterval, BulletTopic};
use crate::types::{
    BulletAccount, BulletBalance, BulletBorrowLendPool, BulletCmcContract, BulletCoinGeckoContract,
    BulletCommissionRate, BulletDelegateOf, BulletExchangeInfo, BulletFundingInfo,
    BulletFundingPayment, BulletFundingRate, BulletHistoricalOrder, BulletHistoryQuery,
    BulletInsuranceBalance, BulletLeverageBracket, BulletListingOrderBook, BulletOpenInterest,
    BulletOpenOrder, BulletOraclePrice, BulletOrderBook, BulletOrderRef, BulletPage,
    BulletPosition, BulletPremiumIndex, BulletPriceTicker, BulletRiskEngineConfig,
    BulletServerTime, BulletSubmitTxResponse, BulletSymbolConfig, BulletTicker24hr, BulletTrade,
    BulletUserTrade,
};
use anyhow::Result;
use async_trait::async_trait;
use rest::{AccountApi, Http, MarketApi};
use roshar_http::RateLimitedClient;
use roshar_ws_mgr::{Config, Manager, Message};
use std::sync::Arc;

pub const BULLET_MAINNET_REST_URL: &str = "https://tradingapi.bullet.xyz";
pub const BULLET_TESTNET_REST_URL: &str = "https://tradingapi.testnet.bullet.xyz";
pub const BULLET_STAGING_REST_URL: &str = "https://tradingapi.staging.bullet.xyz";
pub const BULLET_MAINNET_WSS_URL: &str = "wss://tradingapi.bullet.xyz/ws";
pub const BULLET_TESTNET_WSS_URL: &str = "wss://tradingapi.testnet.bullet.xyz/ws";
pub const BULLET_STAGING_WSS_URL: &str = "wss://tradingapi.staging.bullet.xyz/ws";

/// Bullet environment. Each has its own chain hash (read it from `get_exchange_info`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BulletNetwork {
    Mainnet,
    Testnet,
    Staging,
}

impl BulletNetwork {
    pub fn rest_url(self) -> &'static str {
        match self {
            Self::Mainnet => BULLET_MAINNET_REST_URL,
            Self::Testnet => BULLET_TESTNET_REST_URL,
            Self::Staging => BULLET_STAGING_REST_URL,
        }
    }

    pub fn wss_url(self) -> &'static str {
        match self {
            Self::Mainnet => BULLET_MAINNET_WSS_URL,
            Self::Testnet => BULLET_TESTNET_WSS_URL,
            Self::Staging => BULLET_STAGING_WSS_URL,
        }
    }
}

pub struct BulletClient {
    market_api: MarketApi,
    account_api: AccountApi,
    ws_config: Config,
}

impl BulletClient {
    /// Documented HTTP limit: 50 requests/s sustained (burst 150) per client IP, counting REST
    /// and websocket upgrades (https://tradingapi.bullet.xyz/docs/connectivity.html#rate-limits).
    const DEFAULT_REQUESTS_PER_SECOND: u32 = 50;
    const WS_CONN_NAME: &str = "bullet";

    pub fn new(network: BulletNetwork) -> Self {
        Self::new_with_rate_limit(network, Self::DEFAULT_REQUESTS_PER_SECOND)
    }

    /// Market data, account reads and submission share one limiter of `requests_per_second`.
    pub fn new_with_rate_limit(network: BulletNetwork, requests_per_second: u32) -> Self {
        let http = Http::new(
            RateLimitedClient::new(requests_per_second, 1),
            network.rest_url(),
        );
        Self {
            market_api: MarketApi::new(http.clone()),
            account_api: AccountApi::new(http),
            ws_config: Config {
                name: Self::WS_CONN_NAME.to_string(),
                url: network.wss_url().to_string(),
                // The server pings every 30s and expects pongs (idle and pong timeouts are 60s).
                // A client protocol ping also counts as liveness, so send one every 30s.
                ping_duration: 30,
                ping_message: String::new(),
                ping_timeout: 60,
                reconnect_timeout: 5000,
                use_text_ping: Some(false),
                read_buffer_size: None,
                write_buffer_size: None,
                // Client messages are capped at 16,000 bytes; server messages are not.
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

    pub fn market(&self) -> &MarketApi {
        &self.market_api
    }

    pub fn account(&self) -> &AccountApi {
        &self.account_api
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

    /// Sends one `subscribe` request for `topics` (at most 1000).
    pub fn subscribe(
        &self,
        manager: &Arc<Manager>,
        id: Option<u64>,
        topics: &[BulletTopic<'_>],
    ) -> Result<()> {
        self.send(manager, ws::subscribe(id, topics))
    }

    /// Sends one `unsubscribe` request for `topics`.
    pub fn unsubscribe(
        &self,
        manager: &Arc<Manager>,
        id: Option<u64>,
        topics: &[BulletTopic<'_>],
    ) -> Result<()> {
        self.send(manager, ws::unsubscribe(id, topics))
    }

    fn per_symbol<'a>(
        symbols: &[&'a str],
        topic: impl Fn(&'a str) -> BulletTopic<'a>,
    ) -> Vec<BulletTopic<'a>> {
        symbols.iter().map(|s| topic(s)).collect()
    }

    /// `SYMBOL@depth[N]` for each symbol.
    pub fn subscribe_depth(
        &self,
        manager: &Arc<Manager>,
        id: Option<u64>,
        symbols: &[&str],
        levels: BulletDepthLevels,
    ) -> Result<()> {
        self.subscribe(
            manager,
            id,
            &Self::per_symbol(symbols, |s| BulletTopic::Depth(s, levels)),
        )
    }

    pub fn unsubscribe_depth(
        &self,
        manager: &Arc<Manager>,
        id: Option<u64>,
        symbols: &[&str],
        levels: BulletDepthLevels,
    ) -> Result<()> {
        self.unsubscribe(
            manager,
            id,
            &Self::per_symbol(symbols, |s| BulletTopic::Depth(s, levels)),
        )
    }

    /// `SYMBOL@aggTrade` for each symbol.
    pub fn subscribe_trades(
        &self,
        manager: &Arc<Manager>,
        id: Option<u64>,
        symbols: &[&str],
    ) -> Result<()> {
        self.subscribe(
            manager,
            id,
            &Self::per_symbol(symbols, BulletTopic::AggTrade),
        )
    }

    pub fn unsubscribe_trades(
        &self,
        manager: &Arc<Manager>,
        id: Option<u64>,
        symbols: &[&str],
    ) -> Result<()> {
        self.unsubscribe(
            manager,
            id,
            &Self::per_symbol(symbols, BulletTopic::AggTrade),
        )
    }

    /// `SYMBOL@bookTicker` for each symbol.
    pub fn subscribe_book_ticker(
        &self,
        manager: &Arc<Manager>,
        id: Option<u64>,
        symbols: &[&str],
    ) -> Result<()> {
        self.subscribe(
            manager,
            id,
            &Self::per_symbol(symbols, BulletTopic::BookTicker),
        )
    }

    pub fn unsubscribe_book_ticker(
        &self,
        manager: &Arc<Manager>,
        id: Option<u64>,
        symbols: &[&str],
    ) -> Result<()> {
        self.unsubscribe(
            manager,
            id,
            &Self::per_symbol(symbols, BulletTopic::BookTicker),
        )
    }

    /// `SYMBOL@markPrice` (mark, index, funding rate) for each symbol.
    pub fn subscribe_mark_price(
        &self,
        manager: &Arc<Manager>,
        id: Option<u64>,
        symbols: &[&str],
    ) -> Result<()> {
        self.subscribe(
            manager,
            id,
            &Self::per_symbol(symbols, BulletTopic::MarkPrice),
        )
    }

    pub fn unsubscribe_mark_price(
        &self,
        manager: &Arc<Manager>,
        id: Option<u64>,
        symbols: &[&str],
    ) -> Result<()> {
        self.unsubscribe(
            manager,
            id,
            &Self::per_symbol(symbols, BulletTopic::MarkPrice),
        )
    }

    /// `SYMBOL@ticker` (documented as coming soon: accepted, no messages yet).
    pub fn subscribe_ticker(
        &self,
        manager: &Arc<Manager>,
        id: Option<u64>,
        symbols: &[&str],
    ) -> Result<()> {
        self.subscribe(manager, id, &Self::per_symbol(symbols, BulletTopic::Ticker))
    }

    pub fn unsubscribe_ticker(
        &self,
        manager: &Arc<Manager>,
        id: Option<u64>,
        symbols: &[&str],
    ) -> Result<()> {
        self.unsubscribe(manager, id, &Self::per_symbol(symbols, BulletTopic::Ticker))
    }

    /// `SYMBOL@kline_INTERVAL` (documented as coming soon: accepted, no messages yet).
    pub fn subscribe_candles(
        &self,
        manager: &Arc<Manager>,
        id: Option<u64>,
        symbols: &[&str],
        interval: BulletKlineInterval,
    ) -> Result<()> {
        self.subscribe(
            manager,
            id,
            &Self::per_symbol(symbols, |s| BulletTopic::Kline(s, interval)),
        )
    }

    pub fn unsubscribe_candles(
        &self,
        manager: &Arc<Manager>,
        id: Option<u64>,
        symbols: &[&str],
        interval: BulletKlineInterval,
    ) -> Result<()> {
        self.unsubscribe(
            manager,
            id,
            &Self::per_symbol(symbols, |s| BulletTopic::Kline(s, interval)),
        )
    }

    /// `SYMBOL@liquidations` for each symbol.
    pub fn subscribe_liquidations(
        &self,
        manager: &Arc<Manager>,
        id: Option<u64>,
        symbols: &[&str],
    ) -> Result<()> {
        self.subscribe(
            manager,
            id,
            &Self::per_symbol(symbols, BulletTopic::Liquidations),
        )
    }

    pub fn unsubscribe_liquidations(
        &self,
        manager: &Arc<Manager>,
        id: Option<u64>,
        symbols: &[&str],
    ) -> Result<()> {
        self.unsubscribe(
            manager,
            id,
            &Self::per_symbol(symbols, BulletTopic::Liquidations),
        )
    }

    /// `ADDRESS@user.orders` (order and fill updates). Use the main account address.
    pub fn subscribe_user_orders(
        &self,
        manager: &Arc<Manager>,
        id: Option<u64>,
        address: &str,
    ) -> Result<()> {
        self.subscribe(manager, id, &[BulletTopic::UserOrders(address)])
    }

    pub fn unsubscribe_user_orders(
        &self,
        manager: &Arc<Manager>,
        id: Option<u64>,
        address: &str,
    ) -> Result<()> {
        self.unsubscribe(manager, id, &[BulletTopic::UserOrders(address)])
    }

    /// Sends a signed transaction over the websocket (`crate::ws_tx`).
    pub fn send_tx(
        &self,
        manager: &Arc<Manager>,
        id: Option<u64>,
        method: ws_tx::BulletTxMethod,
        tx: &SignedTransaction,
    ) -> Result<()> {
        self.send(manager, ws_tx::tx_request(id, method, tx))
    }
}

#[async_trait]
pub trait BulletApi {
    async fn get_server_time(&self) -> RestResult<BulletServerTime>;
    async fn get_exchange_info(&self) -> RestResult<BulletExchangeInfo>;
    async fn get_depth(&self, symbol: &str, limit: Option<u32>) -> RestResult<BulletOrderBook>;
    async fn get_trades(&self, symbol: &str, limit: Option<u32>) -> RestResult<Vec<BulletTrade>>;
    async fn get_ticker_24hr(&self) -> RestResult<Vec<BulletTicker24hr>>;
    async fn get_ticker_price(&self, symbol: Option<&str>) -> RestResult<Vec<BulletPriceTicker>>;
    async fn get_premium_index(&self, symbol: Option<&str>) -> RestResult<Vec<BulletPremiumIndex>>;
    async fn get_funding_rate_history(
        &self,
        symbol: Option<&str>,
        start_time: Option<i64>,
        end_time: Option<i64>,
        limit: Option<u32>,
    ) -> RestResult<Vec<BulletFundingRate>>;
    async fn get_funding_info(&self, symbol: Option<&str>) -> RestResult<Vec<BulletFundingInfo>>;
    async fn get_open_interest(&self) -> RestResult<Vec<BulletOpenInterest>>;
    async fn get_symbol_config(&self, symbol: Option<&str>) -> RestResult<Vec<BulletSymbolConfig>>;
    async fn get_leverage_brackets(
        &self,
        symbol: Option<&str>,
    ) -> RestResult<Vec<BulletLeverageBracket>>;
    async fn get_oracle_prices(&self, asset_id: Option<u16>) -> RestResult<Vec<BulletOraclePrice>>;
    async fn get_insurance_balance(&self) -> RestResult<BulletInsuranceBalance>;
    async fn get_borrow_lend_pools(
        &self,
        symbol: Option<&str>,
    ) -> RestResult<Vec<BulletBorrowLendPool>>;
    async fn get_risk_engine_config(&self) -> RestResult<BulletRiskEngineConfig>;
    async fn get_cmc_contracts(&self) -> RestResult<Vec<BulletCmcContract>>;
    async fn get_cmc_orderbook(
        &self,
        ticker_id: &str,
        depth: Option<u32>,
    ) -> RestResult<BulletListingOrderBook>;
    async fn get_coingecko_contracts(&self) -> RestResult<Vec<BulletCoinGeckoContract>>;
    async fn get_coingecko_orderbook(
        &self,
        ticker_id: &str,
        depth: Option<u32>,
    ) -> RestResult<BulletListingOrderBook>;

    async fn get_account(&self, address: &str) -> RestResult<BulletAccount>;
    async fn get_balance(&self, address: &str) -> RestResult<Vec<BulletBalance>>;
    async fn get_positions(
        &self,
        address: &str,
        symbol: Option<&str>,
    ) -> RestResult<Vec<BulletPosition>>;
    async fn get_open_orders(
        &self,
        address: &str,
        symbol: Option<&str>,
    ) -> RestResult<Vec<BulletOpenOrder>>;
    async fn get_open_order(
        &self,
        address: &str,
        symbol: &str,
        order: BulletOrderRef,
    ) -> RestResult<BulletOpenOrder>;
    async fn get_all_orders(
        &self,
        query: &BulletHistoryQuery<'_>,
        order_id: Option<u64>,
    ) -> RestResult<BulletPage<BulletHistoricalOrder>>;
    async fn get_user_trades(
        &self,
        query: &BulletHistoryQuery<'_>,
        is_degen: Option<bool>,
    ) -> RestResult<BulletPage<BulletUserTrade>>;
    async fn get_funding_payments(
        &self,
        query: &BulletHistoryQuery<'_>,
    ) -> RestResult<BulletPage<BulletFundingPayment>>;
    async fn get_commission_rate(
        &self,
        address: &str,
        symbol: &str,
    ) -> RestResult<BulletCommissionRate>;
    async fn get_delegate_of(&self, delegate_address: &str) -> RestResult<BulletDelegateOf>;
    async fn submit_tx(&self, tx: &SignedTransaction) -> RestResult<BulletSubmitTxResponse>;
}

#[async_trait]
impl BulletApi for BulletClient {
    async fn get_server_time(&self) -> RestResult<BulletServerTime> {
        self.market_api.get_server_time().await
    }

    async fn get_exchange_info(&self) -> RestResult<BulletExchangeInfo> {
        self.market_api.get_exchange_info().await
    }

    async fn get_depth(&self, symbol: &str, limit: Option<u32>) -> RestResult<BulletOrderBook> {
        self.market_api.get_depth(symbol, limit).await
    }

    async fn get_trades(&self, symbol: &str, limit: Option<u32>) -> RestResult<Vec<BulletTrade>> {
        self.market_api.get_trades(symbol, limit).await
    }

    async fn get_ticker_24hr(&self) -> RestResult<Vec<BulletTicker24hr>> {
        self.market_api.get_ticker_24hr().await
    }

    async fn get_ticker_price(&self, symbol: Option<&str>) -> RestResult<Vec<BulletPriceTicker>> {
        self.market_api.get_ticker_price(symbol).await
    }

    async fn get_premium_index(&self, symbol: Option<&str>) -> RestResult<Vec<BulletPremiumIndex>> {
        self.market_api.get_premium_index(symbol).await
    }

    async fn get_funding_rate_history(
        &self,
        symbol: Option<&str>,
        start_time: Option<i64>,
        end_time: Option<i64>,
        limit: Option<u32>,
    ) -> RestResult<Vec<BulletFundingRate>> {
        self.market_api
            .get_funding_rate_history(symbol, start_time, end_time, limit)
            .await
    }

    async fn get_funding_info(&self, symbol: Option<&str>) -> RestResult<Vec<BulletFundingInfo>> {
        self.market_api.get_funding_info(symbol).await
    }

    async fn get_open_interest(&self) -> RestResult<Vec<BulletOpenInterest>> {
        self.market_api.get_open_interest().await
    }

    async fn get_symbol_config(&self, symbol: Option<&str>) -> RestResult<Vec<BulletSymbolConfig>> {
        self.market_api.get_symbol_config(symbol).await
    }

    async fn get_leverage_brackets(
        &self,
        symbol: Option<&str>,
    ) -> RestResult<Vec<BulletLeverageBracket>> {
        self.market_api.get_leverage_brackets(symbol).await
    }

    async fn get_oracle_prices(&self, asset_id: Option<u16>) -> RestResult<Vec<BulletOraclePrice>> {
        self.market_api.get_oracle_prices(asset_id).await
    }

    async fn get_insurance_balance(&self) -> RestResult<BulletInsuranceBalance> {
        self.market_api.get_insurance_balance().await
    }

    async fn get_borrow_lend_pools(
        &self,
        symbol: Option<&str>,
    ) -> RestResult<Vec<BulletBorrowLendPool>> {
        self.market_api.get_borrow_lend_pools(symbol).await
    }

    async fn get_risk_engine_config(&self) -> RestResult<BulletRiskEngineConfig> {
        self.market_api.get_risk_engine_config().await
    }

    async fn get_cmc_contracts(&self) -> RestResult<Vec<BulletCmcContract>> {
        self.market_api.get_cmc_contracts().await
    }

    async fn get_cmc_orderbook(
        &self,
        ticker_id: &str,
        depth: Option<u32>,
    ) -> RestResult<BulletListingOrderBook> {
        self.market_api.get_cmc_orderbook(ticker_id, depth).await
    }

    async fn get_coingecko_contracts(&self) -> RestResult<Vec<BulletCoinGeckoContract>> {
        self.market_api.get_coingecko_contracts().await
    }

    async fn get_coingecko_orderbook(
        &self,
        ticker_id: &str,
        depth: Option<u32>,
    ) -> RestResult<BulletListingOrderBook> {
        self.market_api
            .get_coingecko_orderbook(ticker_id, depth)
            .await
    }

    async fn get_account(&self, address: &str) -> RestResult<BulletAccount> {
        self.account_api.get_account(address).await
    }

    async fn get_balance(&self, address: &str) -> RestResult<Vec<BulletBalance>> {
        self.account_api.get_balance(address).await
    }

    async fn get_positions(
        &self,
        address: &str,
        symbol: Option<&str>,
    ) -> RestResult<Vec<BulletPosition>> {
        self.account_api.get_positions(address, symbol).await
    }

    async fn get_open_orders(
        &self,
        address: &str,
        symbol: Option<&str>,
    ) -> RestResult<Vec<BulletOpenOrder>> {
        self.account_api.get_open_orders(address, symbol).await
    }

    async fn get_open_order(
        &self,
        address: &str,
        symbol: &str,
        order: BulletOrderRef,
    ) -> RestResult<BulletOpenOrder> {
        self.account_api
            .get_open_order(address, symbol, order)
            .await
    }

    async fn get_all_orders(
        &self,
        query: &BulletHistoryQuery<'_>,
        order_id: Option<u64>,
    ) -> RestResult<BulletPage<BulletHistoricalOrder>> {
        self.account_api.get_all_orders(query, order_id).await
    }

    async fn get_user_trades(
        &self,
        query: &BulletHistoryQuery<'_>,
        is_degen: Option<bool>,
    ) -> RestResult<BulletPage<BulletUserTrade>> {
        self.account_api.get_user_trades(query, is_degen).await
    }

    async fn get_funding_payments(
        &self,
        query: &BulletHistoryQuery<'_>,
    ) -> RestResult<BulletPage<BulletFundingPayment>> {
        self.account_api.get_funding_payments(query).await
    }

    async fn get_commission_rate(
        &self,
        address: &str,
        symbol: &str,
    ) -> RestResult<BulletCommissionRate> {
        self.account_api.get_commission_rate(address, symbol).await
    }

    async fn get_delegate_of(&self, delegate_address: &str) -> RestResult<BulletDelegateOf> {
        self.account_api.get_delegate_of(delegate_address).await
    }

    async fn submit_tx(&self, tx: &SignedTransaction) -> RestResult<BulletSubmitTxResponse> {
        self.account_api.submit_tx(tx).await
    }
}
