/// Authenticated REST: JWT auth, orders, positions, balances, fills.
pub mod exchange;
/// SBE binary decoding of websocket channel payloads.
pub mod sbe;
/// Stark-curve typed-data hashing and signing (auth, orders).
pub mod signer;
/// Wire and message types for this exchange (websocket messages, REST payloads).
pub mod types;
/// JSON-RPC control frames on the websocket (subscribe acks, errors, auth results).
pub mod ws_control;
/// Authenticated websocket: auth request and private channel subscriptions and payloads.
pub mod ws_private;

// Endpoints. Public channel payloads are SBE binary frames; the exchange rejects (HTTP 400)
// connections that do not negotiate SBE schema 1. Subscribe/unsubscribe requests, their
// acknowledgements and errors stay JSON text frames. Version 3 matches the pinned schema in
// `schema/` and what paradex-py negotiates and the server asks for (the docs page still says 1).
pub const PARADEX_WSS_URL: &str =
    "wss://ws.api.prod.paradex.trade/v1?sbeSchemaId=1&sbeSchemaVersion=3";
pub const PARADEX_TESTNET_WSS_URL: &str =
    "wss://ws.api.testnet.paradex.trade/v1?sbeSchemaId=1&sbeSchemaVersion=3";

pub(crate) mod rest;

use rest::MarketApi;

pub use rest::MarketApi as ParadexMarketApi;

use crate::types::{
    ParadexBbo, ParadexBookFeed, ParadexExchangeStats, ParadexFundingComparisonRefresh,
    ParadexFundingEntry, ParadexImpactPrice, ParadexInsuranceFund, ParadexInteractiveOrderBook,
    ParadexKline, ParadexListResponse, ParadexMarginConfig, ParadexMarket, ParadexMarketEvent,
    ParadexMarketSummary, ParadexOrderBook, ParadexPageQuery, ParadexSettlementPrice,
    ParadexSystemConfig, ParadexSystemState, ParadexSystemTime, ParadexTrade, ParadexWssMessage,
};
use anyhow::Result;
use async_trait::async_trait;
use roshar_ws_mgr::{Config, Manager, Message};
use std::collections::HashMap;
use std::sync::Arc;

type ApiResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

pub struct ParadexClient {
    market_api: MarketApi,
    ws_config: Config,
}

impl Default for ParadexClient {
    fn default() -> Self {
        Self::new()
    }
}

impl ParadexClient {
    /// Documented public REST limit: 1500 req/min per IP.
    const DEFAULT_REQUESTS_PER_SECOND: u32 = 25;
    const WS_CONN_NAME: &str = "paradex";

    pub fn new() -> Self {
        Self::new_with_rate_limit(Self::DEFAULT_REQUESTS_PER_SECOND)
    }

    pub fn new_with_rate_limit(requests_per_second: u32) -> Self {
        Self {
            market_api: MarketApi::new(requests_per_second),
            ws_config: Config {
                name: Self::WS_CONN_NAME.to_string(),
                url: PARADEX_WSS_URL.to_string(),
                // Paradex has no JSON-RPC ping method. The server sends protocol pings every 55s
                // and drops the connection without a pong within 5s; the client sends protocol
                // pings and the server answers with pongs.
                ping_duration: 30,
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

    fn send(&self, manager: &Arc<Manager>, msg: ParadexWssMessage) -> Result<()> {
        manager.write(
            &self.ws_config.name,
            Message::TextMessage(self.ws_config.name.clone(), msg.to_json()),
        )?;
        Ok(())
    }

    /// Subscribe to `order_book.{market}.{feed}` for each market.
    pub fn subscribe_depth(
        &self,
        manager: &Arc<Manager>,
        markets: &[&str],
        feed: ParadexBookFeed<'_>,
    ) -> Result<()> {
        for market in markets {
            self.send(manager, ParadexWssMessage::depth(market, feed))?;
        }
        Ok(())
    }

    /// Unsubscribe from `order_book.{market}.{feed}`; `feed` must match the subscription.
    pub fn unsubscribe_depth(
        &self,
        manager: &Arc<Manager>,
        markets: &[&str],
        feed: ParadexBookFeed<'_>,
    ) -> Result<()> {
        for market in markets {
            self.send(manager, ParadexWssMessage::depth_unsub(market, feed))?;
        }
        Ok(())
    }

    pub fn subscribe_trades(&self, manager: &Arc<Manager>, markets: &[&str]) -> Result<()> {
        for market in markets {
            self.send(manager, ParadexWssMessage::trades(market))?;
        }
        Ok(())
    }

    pub fn unsubscribe_trades(&self, manager: &Arc<Manager>, markets: &[&str]) -> Result<()> {
        for market in markets {
            self.send(manager, ParadexWssMessage::trades_unsub(market))?;
        }
        Ok(())
    }

    pub fn subscribe_bbo(&self, manager: &Arc<Manager>, markets: &[&str]) -> Result<()> {
        for market in markets {
            self.send(manager, ParadexWssMessage::bbo(market))?;
        }
        Ok(())
    }

    pub fn unsubscribe_bbo(&self, manager: &Arc<Manager>, markets: &[&str]) -> Result<()> {
        for market in markets {
            self.send(manager, ParadexWssMessage::bbo_unsub(market))?;
        }
        Ok(())
    }

    pub fn subscribe_funding(&self, manager: &Arc<Manager>, markets: &[&str]) -> Result<()> {
        for market in markets {
            self.send(manager, ParadexWssMessage::funding(market))?;
        }
        Ok(())
    }

    pub fn unsubscribe_funding(&self, manager: &Arc<Manager>, markets: &[&str]) -> Result<()> {
        for market in markets {
            self.send(manager, ParadexWssMessage::funding_unsub(market))?;
        }
        Ok(())
    }

    /// Subscribe to `markets_summary` (`market: None`, all markets) or
    /// `markets_summary.{market}`.
    pub fn subscribe_markets_summary(
        &self,
        manager: &Arc<Manager>,
        market: Option<&str>,
    ) -> Result<()> {
        self.send(manager, ParadexWssMessage::markets_summary(market))
    }

    /// Unsubscribe `markets_summary[.{market}]`; `None` also closes a scoped subscription.
    pub fn unsubscribe_markets_summary(
        &self,
        manager: &Arc<Manager>,
        market: Option<&str>,
    ) -> Result<()> {
        self.send(manager, ParadexWssMessage::markets_summary_unsub(market))
    }

    /// Scoped `markets_summary` subscription filtered to `markets`.
    pub fn subscribe_markets_summary_scoped(
        &self,
        manager: &Arc<Manager>,
        markets: &[&str],
    ) -> Result<()> {
        self.send(manager, ParadexWssMessage::markets_summary_scoped(markets))
    }

    /// `modify_subscription` on a live scoped subscription (e.g. `markets_summary`).
    pub fn modify_subscription(
        &self,
        manager: &Arc<Manager>,
        channel: &str,
        add: Option<&[&str]>,
        remove: Option<&[&str]>,
    ) -> Result<()> {
        self.send(
            manager,
            ParadexWssMessage::modify_subscription(channel, add, remove),
        )
    }

    /// Subscribe to `markets` (`None`) or `markets.{market}` spec change events.
    pub fn subscribe_markets(&self, manager: &Arc<Manager>, market: Option<&str>) -> Result<()> {
        self.send(manager, ParadexWssMessage::markets(market))
    }

    pub fn unsubscribe_markets(&self, manager: &Arc<Manager>, market: Option<&str>) -> Result<()> {
        self.send(manager, ParadexWssMessage::markets_unsub(market))
    }

    pub fn subscribe_system_status(&self, manager: &Arc<Manager>) -> Result<()> {
        self.send(manager, ParadexWssMessage::system_status())
    }

    pub fn unsubscribe_system_status(&self, manager: &Arc<Manager>) -> Result<()> {
        self.send(manager, ParadexWssMessage::system_status_unsub())
    }

    pub fn subscribe_funding_rate_comparison(
        &self,
        manager: &Arc<Manager>,
        refresh: ParadexFundingComparisonRefresh,
    ) -> Result<()> {
        self.send(manager, ParadexWssMessage::funding_rate_comparison(refresh))
    }

    pub fn unsubscribe_funding_rate_comparison(
        &self,
        manager: &Arc<Manager>,
        refresh: ParadexFundingComparisonRefresh,
    ) -> Result<()> {
        self.send(
            manager,
            ParadexWssMessage::funding_rate_comparison_unsub(refresh),
        )
    }
}

#[async_trait]
pub trait ParadexApi {
    async fn get_markets(
        &self,
        market: Option<&str>,
        asset_kind: Option<&str>,
        base_asset: Option<&str>,
    ) -> ApiResult<Vec<ParadexMarket>>;

    async fn get_markets_summary(
        &self,
        market: Option<&str>,
        asset_kind: Option<&str>,
        base_asset: Option<&str>,
    ) -> ApiResult<HashMap<String, ParadexMarketSummary>>;

    async fn get_orderbook(
        &self,
        market: &str,
        depth: Option<u32>,
        price_tick: Option<&str>,
    ) -> ApiResult<ParadexOrderBook>;

    async fn get_trades(
        &self,
        market: &str,
        page: &ParadexPageQuery,
    ) -> ApiResult<ParadexListResponse<ParadexTrade>>;

    async fn get_klines(
        &self,
        market: &str,
        resolution: u32,
        start_at: i64,
        end_at: i64,
        price_kind: Option<&str>,
    ) -> ApiResult<Vec<ParadexKline>>;

    async fn get_funding_history(
        &self,
        market: &str,
        page: &ParadexPageQuery,
    ) -> ApiResult<ParadexListResponse<ParadexFundingEntry>>;

    async fn get_bbo(&self, market: &str) -> ApiResult<ParadexBbo>;

    async fn get_bbo_interactive(&self, market: &str) -> ApiResult<ParadexBbo>;

    async fn get_orderbook_interactive(
        &self,
        market: &str,
        depth: Option<u32>,
        price_tick: Option<&str>,
    ) -> ApiResult<ParadexInteractiveOrderBook>;

    async fn get_impact_price(
        &self,
        market: &str,
        side: &str,
        size: &str,
    ) -> ApiResult<ParadexImpactPrice>;

    async fn get_markets_history(&self, market: Option<&str>)
        -> ApiResult<Vec<ParadexMarketEvent>>;

    async fn get_settlement_price(&self, market: &str) -> ApiResult<ParadexSettlementPrice>;

    async fn get_insurance_fund(&self) -> ApiResult<ParadexInsuranceFund>;

    async fn get_system_config(&self) -> ApiResult<ParadexSystemConfig>;

    async fn get_system_state(&self) -> ApiResult<ParadexSystemState>;

    async fn get_system_time(&self) -> ApiResult<ParadexSystemTime>;

    async fn get_exchange_stats(&self) -> ApiResult<ParadexExchangeStats>;

    async fn get_margin_config(
        &self,
        base_asset: Option<&str>,
    ) -> ApiResult<Vec<ParadexMarginConfig>>;
}

#[async_trait]
impl ParadexApi for ParadexClient {
    async fn get_markets(
        &self,
        market: Option<&str>,
        asset_kind: Option<&str>,
        base_asset: Option<&str>,
    ) -> ApiResult<Vec<ParadexMarket>> {
        self.market_api
            .get_markets(market, asset_kind, base_asset)
            .await
    }

    async fn get_markets_summary(
        &self,
        market: Option<&str>,
        asset_kind: Option<&str>,
        base_asset: Option<&str>,
    ) -> ApiResult<HashMap<String, ParadexMarketSummary>> {
        self.market_api
            .get_markets_summary(market, asset_kind, base_asset)
            .await
    }

    async fn get_orderbook(
        &self,
        market: &str,
        depth: Option<u32>,
        price_tick: Option<&str>,
    ) -> ApiResult<ParadexOrderBook> {
        self.market_api
            .get_orderbook(market, depth, price_tick)
            .await
    }

    async fn get_trades(
        &self,
        market: &str,
        page: &ParadexPageQuery,
    ) -> ApiResult<ParadexListResponse<ParadexTrade>> {
        self.market_api.get_trades(market, page).await
    }

    async fn get_klines(
        &self,
        market: &str,
        resolution: u32,
        start_at: i64,
        end_at: i64,
        price_kind: Option<&str>,
    ) -> ApiResult<Vec<ParadexKline>> {
        self.market_api
            .get_klines(market, resolution, start_at, end_at, price_kind)
            .await
    }

    async fn get_funding_history(
        &self,
        market: &str,
        page: &ParadexPageQuery,
    ) -> ApiResult<ParadexListResponse<ParadexFundingEntry>> {
        self.market_api.get_funding_history(market, page).await
    }

    async fn get_bbo(&self, market: &str) -> ApiResult<ParadexBbo> {
        self.market_api.get_bbo(market).await
    }

    async fn get_bbo_interactive(&self, market: &str) -> ApiResult<ParadexBbo> {
        self.market_api.get_bbo_interactive(market).await
    }

    async fn get_orderbook_interactive(
        &self,
        market: &str,
        depth: Option<u32>,
        price_tick: Option<&str>,
    ) -> ApiResult<ParadexInteractiveOrderBook> {
        self.market_api
            .get_orderbook_interactive(market, depth, price_tick)
            .await
    }

    async fn get_impact_price(
        &self,
        market: &str,
        side: &str,
        size: &str,
    ) -> ApiResult<ParadexImpactPrice> {
        self.market_api.get_impact_price(market, side, size).await
    }

    async fn get_markets_history(
        &self,
        market: Option<&str>,
    ) -> ApiResult<Vec<ParadexMarketEvent>> {
        self.market_api.get_markets_history(market).await
    }

    async fn get_settlement_price(&self, market: &str) -> ApiResult<ParadexSettlementPrice> {
        self.market_api.get_settlement_price(market).await
    }

    async fn get_insurance_fund(&self) -> ApiResult<ParadexInsuranceFund> {
        self.market_api.get_insurance_fund().await
    }

    async fn get_system_config(&self) -> ApiResult<ParadexSystemConfig> {
        self.market_api.get_system_config().await
    }

    async fn get_system_state(&self) -> ApiResult<ParadexSystemState> {
        self.market_api.get_system_state().await
    }

    async fn get_system_time(&self) -> ApiResult<ParadexSystemTime> {
        self.market_api.get_system_time().await
    }

    async fn get_exchange_stats(&self) -> ApiResult<ParadexExchangeStats> {
        self.market_api.get_exchange_stats().await
    }

    async fn get_margin_config(
        &self,
        base_asset: Option<&str>,
    ) -> ApiResult<Vec<ParadexMarginConfig>> {
        self.market_api.get_margin_config(base_asset).await
    }
}
