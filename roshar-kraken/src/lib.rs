/// Wire and message types for this exchange (websocket messages, REST payloads).
pub mod types;

/// Unified WebSocket, public endpoint. Market requests carry `asset_class: "derivatives"`.
pub const KRAKEN_WSS_URL: &str = "wss://uws.kraken.com/v2/public";

pub(crate) mod rest;

use rest::{ChartsApi, MarketApi, MarketHistoryApi};
pub use rest::{
    KrakenAnalytics, KrakenAnalyticsBasis, KrakenAnalyticsCvd, KrakenAnalyticsData,
    KrakenAnalyticsError, KrakenAnalyticsFunding, KrakenAnalyticsLiquidityPool,
    KrakenAnalyticsLiquiditySide, KrakenAnalyticsLongShort, KrakenAnalyticsOrderbookSide,
    KrakenAnalyticsSides, KrakenAnalyticsSlippageSide, KrakenAnalyticsSpreadsSide,
    KrakenAnalyticsTopTraders, KrakenAnalyticsTopTradersLevel, KrakenAnalyticsType,
    KrakenCancelResponse, KrakenCancelStatus, KrakenContractType, KrakenCredentials,
    KrakenDeadMansSwitchResponse, KrakenDeadMansSwitchStatus, KrakenEditStatus,
    KrakenGetLeverageResponse, KrakenHistoricalFundingRate, KrakenHistoryOrder, KrakenHistoryPage,
    KrakenHistoryQuery, KrakenHistoryTrade, KrakenInstrument, KrakenInstrumentStatus,
    KrakenLeveragePreference, KrakenLeverageSettingResponse, KrakenMarginLevel,
    KrakenMarginSchedule, KrakenMarkPriceChanged, KrakenMarkPriceElement, KrakenMarkPriceEvent,
    KrakenModifyResponse, KrakenOpenOrder, KrakenOpenOrdersResponse, KrakenOptionGreeks,
    KrakenOrder, KrakenOrderBook, KrakenOrderCancelled, KrakenOrderEvent, KrakenOrderPlaced,
    KrakenOrderResponse, KrakenOrderStatusItem, KrakenOrderStatusOrder, KrakenOrderStatusResponse,
    KrakenOrderTrigger, KrakenOrderUpdated, KrakenPriceTriggerOptions, KrakenPublicExecution,
    KrakenPublicExecutionDetails, KrakenPublicExecutionElement, KrakenPublicExecutionEvent,
    KrakenPublicOrderElement, KrakenPublicOrderEvent, KrakenResolution, KrakenResponse,
    KrakenRestCandleData, KrakenRestCandleResponse, KrakenSendStatus, KrakenSortOrder,
    KrakenTickType, KrakenTickerData, KrakenTrailingStopOptions, MultiCollateralApi,
    OrderManagementApi,
};
use roshar_http::RateLimitedClient;

use crate::types::{KrakenBookDepth, KrakenOhlcInterval, KrakenOhlcTickType, KrakenWssMessage};
use anyhow::Result;
use async_trait::async_trait;
use roshar_ws_mgr::{Config, Manager, Message};
use std::collections::HashMap;
use std::sync::Arc;

pub struct KrakenClient {
    charts_api: ChartsApi,
    market_api: MarketApi,
    history_api: MarketHistoryApi,
    ws_config: Config,
}

impl KrakenClient {
    const WS_CONN_NAME: &str = "kraken";

    pub fn new(requests_per_second: u32) -> Self {
        let http_client = Arc::new(RateLimitedClient::new(requests_per_second, 1));

        Self {
            charts_api: ChartsApi::new_with_client(http_client.clone()),
            market_api: MarketApi::new_with_client(http_client.clone()),
            history_api: MarketHistoryApi::new_with_client(http_client),
            ws_config: Config {
                name: Self::WS_CONN_NAME.to_string(),
                url: KRAKEN_WSS_URL.to_string(),
                // Protocol-level ping frames: the server answers each with a pong frame, which
                // roshar-ws-mgr uses for its timeout, and they keep an idle connection open.
                ping_duration: 20,
                ping_message: KrakenWssMessage::ping().to_json(),
                ping_timeout: 45,
                reconnect_timeout: 10000,
                use_text_ping: Some(false),
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

    fn send(&self, manager: &Arc<Manager>, msg: KrakenWssMessage) -> Result<()> {
        manager.write(
            &self.ws_config.name,
            Message::TextMessage(self.ws_config.name.clone(), msg.to_json()),
        )?;
        Ok(())
    }

    /// Subscribes to the `book` channel for `symbols` (e.g. `PF_XBTUSD`).
    pub fn subscribe_depth(
        &self,
        manager: &Arc<Manager>,
        symbols: &[&str],
        depth: KrakenBookDepth,
        snapshot: bool,
    ) -> Result<()> {
        self.send(manager, KrakenWssMessage::depth(symbols, depth, snapshot))
    }

    /// Subscribes to the `trade` channel for `symbols`.
    pub fn subscribe_trades(
        &self,
        manager: &Arc<Manager>,
        symbols: &[&str],
        snapshot: bool,
    ) -> Result<()> {
        self.send(manager, KrakenWssMessage::trades(symbols, snapshot))
    }

    /// `depth` must match the one subscribed with.
    pub fn unsubscribe_depth(
        &self,
        manager: &Arc<Manager>,
        symbols: &[&str],
        depth: KrakenBookDepth,
    ) -> Result<()> {
        self.send(manager, KrakenWssMessage::depth_unsub(symbols, depth))
    }

    pub fn unsubscribe_trades(&self, manager: &Arc<Manager>, symbols: &[&str]) -> Result<()> {
        self.send(manager, KrakenWssMessage::trades_unsub(symbols))
    }

    /// Subscribes to the `ticker` channel for `symbols`.
    pub fn subscribe_ticker(
        &self,
        manager: &Arc<Manager>,
        symbols: &[&str],
        snapshot: bool,
    ) -> Result<()> {
        self.send(manager, KrakenWssMessage::ticker(symbols, snapshot))
    }

    pub fn unsubscribe_ticker(&self, manager: &Arc<Manager>, symbols: &[&str]) -> Result<()> {
        self.send(manager, KrakenWssMessage::ticker_unsub(symbols))
    }

    /// Subscribes to the `ohlc` channel. `tick_type: None` leaves Kraken's default (`trade`).
    pub fn subscribe_ohlc(
        &self,
        manager: &Arc<Manager>,
        symbols: &[&str],
        interval: KrakenOhlcInterval,
        tick_type: Option<KrakenOhlcTickType>,
        snapshot: bool,
    ) -> Result<()> {
        self.send(
            manager,
            KrakenWssMessage::ohlc(symbols, interval, tick_type, snapshot),
        )
    }

    /// `interval` and `tick_type` must match the subscription.
    pub fn unsubscribe_ohlc(
        &self,
        manager: &Arc<Manager>,
        symbols: &[&str],
        interval: KrakenOhlcInterval,
        tick_type: Option<KrakenOhlcTickType>,
    ) -> Result<()> {
        self.send(
            manager,
            KrakenWssMessage::ohlc_unsub(symbols, interval, tick_type),
        )
    }

    /// Subscribes to the `instrument` channel (all derivatives markets and assets).
    pub fn subscribe_instruments(&self, manager: &Arc<Manager>, snapshot: bool) -> Result<()> {
        self.send(manager, KrakenWssMessage::instruments(snapshot))
    }

    pub fn unsubscribe_instruments(&self, manager: &Arc<Manager>) -> Result<()> {
        self.send(manager, KrakenWssMessage::instruments_unsub())
    }

    /// Subscribes to the `rfqs` channel (all open options RFQs).
    pub fn subscribe_rfqs(&self, manager: &Arc<Manager>, snapshot: bool) -> Result<()> {
        self.send(manager, KrakenWssMessage::rfqs(snapshot))
    }

    pub fn unsubscribe_rfqs(&self, manager: &Arc<Manager>) -> Result<()> {
        self.send(manager, KrakenWssMessage::rfqs_unsub())
    }
}

#[async_trait]
pub trait KrakenApi {
    /// Candles from the charts endpoint, as sent. `from`/`to` are in seconds.
    async fn get_candles(
        &self,
        tick_type: KrakenTickType,
        symbol: &str,
        resolution: KrakenResolution,
        from: Option<i64>,
        to: Option<i64>,
        count: Option<u32>,
    ) -> Result<KrakenRestCandleResponse, String>;
    /// Tick types of the charts endpoint.
    async fn get_chart_tick_types(&self) -> Result<Vec<String>, String>;
    /// Symbols with candles of `tick_type`.
    async fn get_chart_markets(&self, tick_type: KrakenTickType) -> Result<Vec<String>, String>;
    /// Candle resolutions for `tick_type` and `symbol`, as sent.
    async fn get_chart_resolutions(
        &self,
        tick_type: KrakenTickType,
        symbol: &str,
    ) -> Result<Vec<String>, String>;
    /// Market analytics bucketed by `interval`. `since`/`to` are in seconds.
    async fn get_analytics(
        &self,
        symbol: &str,
        analytics_type: KrakenAnalyticsType,
        since: i64,
        interval: KrakenResolution,
        to: Option<i64>,
    ) -> Result<KrakenAnalytics, String>;
    /// Liquidity pool USD value bucketed by `interval`. `since`/`to` are in seconds.
    async fn get_liquidity_pool(
        &self,
        since: i64,
        interval: KrakenResolution,
        to: Option<i64>,
    ) -> Result<KrakenAnalytics, String>;
    /// Tickers keyed by symbol. `None` filters are omitted (Kraken then returns futures only).
    async fn get_tickers(
        &self,
        contract_types: Option<&[KrakenContractType]>,
        symbols: Option<&[&str]>,
    ) -> Result<HashMap<String, KrakenTickerData>, String>;
    async fn get_ticker(&self, symbol: &str) -> Result<KrakenTickerData, String>;
    async fn get_instruments(
        &self,
        contract_types: Option<&[KrakenContractType]>,
        expired: Option<bool>,
    ) -> Result<Vec<KrakenInstrument>, String>;
    async fn get_instruments_status(
        &self,
        contract_types: Option<&[KrakenContractType]>,
    ) -> Result<Vec<KrakenInstrumentStatus>, String>;
    async fn get_instrument_status(&self, symbol: &str) -> Result<KrakenInstrumentStatus, String>;
    /// Full order book.
    async fn get_orderbook(&self, symbol: &str) -> Result<KrakenOrderBook, String>;
    /// Up to 100 trades before `last_time` (RFC 3339), newest first.
    async fn get_trade_history(
        &self,
        symbol: &str,
        last_time: Option<&str>,
        include_mtf_data: Option<bool>,
    ) -> Result<Vec<KrakenHistoryTrade>, String>;
    /// Funding rates of a perpetual, oldest first.
    async fn get_historical_funding_rates(
        &self,
        symbol: &str,
    ) -> Result<Vec<KrakenHistoricalFundingRate>, String>;
    /// Public executions of `tradeable` (`/api/history/v3/market/{tradeable}/executions`).
    async fn get_market_executions(
        &self,
        tradeable: &str,
        query: &KrakenHistoryQuery<'_>,
    ) -> Result<KrakenHistoryPage<KrakenPublicExecutionElement>, String>;
    /// Public order events of `tradeable`.
    async fn get_market_order_events(
        &self,
        tradeable: &str,
        query: &KrakenHistoryQuery<'_>,
    ) -> Result<KrakenHistoryPage<KrakenPublicOrderElement>, String>;
    /// Mark price changes of `tradeable`.
    async fn get_market_mark_price_events(
        &self,
        tradeable: &str,
        query: &KrakenHistoryQuery<'_>,
    ) -> Result<KrakenHistoryPage<KrakenMarkPriceElement>, String>;
}

#[async_trait]
impl KrakenApi for KrakenClient {
    async fn get_candles(
        &self,
        tick_type: KrakenTickType,
        symbol: &str,
        resolution: KrakenResolution,
        from: Option<i64>,
        to: Option<i64>,
        count: Option<u32>,
    ) -> Result<KrakenRestCandleResponse, String> {
        self.charts_api
            .get_candles(tick_type, symbol, resolution, from, to, count)
            .await
            .map_err(|e| format!("Failed to get candles: {}", e))
    }

    async fn get_chart_tick_types(&self) -> Result<Vec<String>, String> {
        self.charts_api
            .get_tick_types()
            .await
            .map_err(|e| format!("Failed to get chart tick types: {}", e))
    }

    async fn get_chart_markets(&self, tick_type: KrakenTickType) -> Result<Vec<String>, String> {
        self.charts_api
            .get_markets(tick_type)
            .await
            .map_err(|e| format!("Failed to get chart markets: {}", e))
    }

    async fn get_chart_resolutions(
        &self,
        tick_type: KrakenTickType,
        symbol: &str,
    ) -> Result<Vec<String>, String> {
        self.charts_api
            .get_resolutions(tick_type, symbol)
            .await
            .map_err(|e| format!("Failed to get chart resolutions: {}", e))
    }

    async fn get_analytics(
        &self,
        symbol: &str,
        analytics_type: KrakenAnalyticsType,
        since: i64,
        interval: KrakenResolution,
        to: Option<i64>,
    ) -> Result<KrakenAnalytics, String> {
        self.charts_api
            .get_analytics(symbol, analytics_type, since, interval, to)
            .await
            .map_err(|e| format!("Failed to get analytics: {}", e))
    }

    async fn get_liquidity_pool(
        &self,
        since: i64,
        interval: KrakenResolution,
        to: Option<i64>,
    ) -> Result<KrakenAnalytics, String> {
        self.charts_api
            .get_liquidity_pool(since, interval, to)
            .await
            .map_err(|e| format!("Failed to get liquidity pool: {}", e))
    }

    async fn get_tickers(
        &self,
        contract_types: Option<&[KrakenContractType]>,
        symbols: Option<&[&str]>,
    ) -> Result<HashMap<String, KrakenTickerData>, String> {
        self.market_api
            .get_tickers(contract_types, symbols)
            .await
            .map_err(|e| format!("Failed to get tickers: {}", e))
    }

    async fn get_ticker(&self, symbol: &str) -> Result<KrakenTickerData, String> {
        self.market_api
            .get_ticker(symbol)
            .await
            .map_err(|e| format!("Failed to get ticker: {}", e))
    }

    async fn get_instruments(
        &self,
        contract_types: Option<&[KrakenContractType]>,
        expired: Option<bool>,
    ) -> Result<Vec<KrakenInstrument>, String> {
        self.market_api
            .get_instruments(contract_types, expired)
            .await
            .map_err(|e| format!("Failed to get instruments: {}", e))
    }

    async fn get_instruments_status(
        &self,
        contract_types: Option<&[KrakenContractType]>,
    ) -> Result<Vec<KrakenInstrumentStatus>, String> {
        self.market_api
            .get_instruments_status(contract_types)
            .await
            .map_err(|e| format!("Failed to get instrument statuses: {}", e))
    }

    async fn get_instrument_status(&self, symbol: &str) -> Result<KrakenInstrumentStatus, String> {
        self.market_api
            .get_instrument_status(symbol)
            .await
            .map_err(|e| format!("Failed to get instrument status: {}", e))
    }

    async fn get_orderbook(&self, symbol: &str) -> Result<KrakenOrderBook, String> {
        self.market_api
            .get_orderbook(symbol)
            .await
            .map_err(|e| format!("Failed to get orderbook: {}", e))
    }

    async fn get_trade_history(
        &self,
        symbol: &str,
        last_time: Option<&str>,
        include_mtf_data: Option<bool>,
    ) -> Result<Vec<KrakenHistoryTrade>, String> {
        self.market_api
            .get_trade_history(symbol, last_time, include_mtf_data)
            .await
            .map_err(|e| format!("Failed to get trade history: {}", e))
    }

    async fn get_historical_funding_rates(
        &self,
        symbol: &str,
    ) -> Result<Vec<KrakenHistoricalFundingRate>, String> {
        self.market_api
            .get_historical_funding_rates(symbol)
            .await
            .map_err(|e| format!("Failed to get historical funding rates: {}", e))
    }

    async fn get_market_executions(
        &self,
        tradeable: &str,
        query: &KrakenHistoryQuery<'_>,
    ) -> Result<KrakenHistoryPage<KrakenPublicExecutionElement>, String> {
        self.history_api
            .get_executions(tradeable, query)
            .await
            .map_err(|e| format!("Failed to get market executions: {}", e))
    }

    async fn get_market_order_events(
        &self,
        tradeable: &str,
        query: &KrakenHistoryQuery<'_>,
    ) -> Result<KrakenHistoryPage<KrakenPublicOrderElement>, String> {
        self.history_api
            .get_order_events(tradeable, query)
            .await
            .map_err(|e| format!("Failed to get market order events: {}", e))
    }

    async fn get_market_mark_price_events(
        &self,
        tradeable: &str,
        query: &KrakenHistoryQuery<'_>,
    ) -> Result<KrakenHistoryPage<KrakenMarkPriceElement>, String> {
        self.history_api
            .get_mark_price_events(tradeable, query)
            .await
            .map_err(|e| format!("Failed to get market mark price events: {}", e))
    }
}
