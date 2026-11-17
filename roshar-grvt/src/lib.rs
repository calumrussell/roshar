/// EIP-712 order and wallet-login signing.
pub mod signer;
/// Wire types (`full` JSON encoding).
pub mod types;

pub(crate) mod rest;

pub use rest::{GrvtApiError, MarketApi as GrvtMarketApi, TradingApi as GrvtTradingApi};

use crate::types::{
    selector, subscribe_message, unsubscribe_message, GrvtAllInstrumentsRequest, GrvtBookRequest,
    GrvtCancelAllOrdersRequest, GrvtCancelOrderRequest, GrvtCandle, GrvtCandleInterval,
    GrvtCandleType, GrvtCollateralAssets, GrvtCreateOrderRequest, GrvtCurrency, GrvtEmptyRequest,
    GrvtFundingAggregation, GrvtFundingRate, GrvtFundingRequest, GrvtInstrument,
    GrvtInstrumentRequest, GrvtInstrumentsQuery, GrvtKind, GrvtKlineRequest, GrvtMarginRules,
    GrvtMiniTicker, GrvtOrderRequest, GrvtOrderbook, GrvtPage, GrvtPageQuery, GrvtRpcRequest,
    GrvtServerTime, GrvtSession, GrvtSupportedAssets, GrvtTicker, GrvtTrade,
    GrvtTradeHistoryRequest, GrvtTradeRequest, STREAM_BOOK_DELTA, STREAM_BOOK_SNAPSHOT,
    STREAM_CANCEL_STATUS, STREAM_CANDLE, STREAM_FILL, STREAM_MINI_TICKER_DELTA,
    STREAM_MINI_TICKER_SNAPSHOT, STREAM_ORDER, STREAM_ORDER_STATE, STREAM_POSITION,
    STREAM_TICKER_DELTA, STREAM_TICKER_SNAPSHOT, STREAM_TRADE,
};
use anyhow::Result;
use async_trait::async_trait;
use rest::MarketApi;
use roshar_ws_mgr::{Config, Manager, Message};
use serde::Serialize;
use std::sync::Arc;

// Endpoints (grvt-pysdk `grvt_raw_env.py`, api-docs.grvt.io).
pub const GRVT_MARKET_DATA_URL: &str = "https://market-data.grvt.io";
pub const GRVT_TRADES_URL: &str = "https://trades.grvt.io";
pub const GRVT_EDGE_URL: &str = "https://edge.grvt.io";
/// JSON-RPC websocket for public streams (`full` encoding).
pub const GRVT_MARKET_DATA_WSS_URL: &str = "wss://market-data.grvt.io/ws/full";
/// JSON-RPC websocket for private streams and order RPCs; needs the session headers.
pub const GRVT_TRADES_WSS_URL: &str = "wss://trades.grvt.io/ws/full";
pub const GRVT_TESTNET_MARKET_DATA_URL: &str = "https://market-data.testnet.grvt.io";
pub const GRVT_TESTNET_TRADES_URL: &str = "https://trades.testnet.grvt.io";
pub const GRVT_TESTNET_EDGE_URL: &str = "https://edge.testnet.grvt.io";
pub const GRVT_TESTNET_MARKET_DATA_WSS_URL: &str = "wss://market-data.testnet.grvt.io/ws/full";
pub const GRVT_TESTNET_TRADES_WSS_URL: &str = "wss://trades.testnet.grvt.io/ws/full";

type ApiResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// GRVT has no JSON-RPC ping (`"ping"` returns code 1200); the server answers protocol pings.
fn ws_config(name: &str, url: &str, headers: Vec<(String, String)>) -> Config {
    Config {
        name: name.to_string(),
        url: url.to_string(),
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
        headers,
    }
}

fn send(manager: &Arc<Manager>, config: &Config, text: String) -> Result<()> {
    manager.write(
        &config.name,
        Message::TextMessage(config.name.clone(), text),
    )?;
    Ok(())
}

/// Snapshot (`.s`, full payload every tick) or delta (`.d`, snapshot then changes) feed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrvtFeedMode {
    Snapshot,
    Delta,
}

/// Public market data: REST plus the market-data websocket.
pub struct GrvtClient {
    market_api: MarketApi,
    ws_config: Config,
}

impl Default for GrvtClient {
    fn default() -> Self {
        Self::new()
    }
}

impl GrvtClient {
    /// Market-data REST allows 500 requests/min per tier for most endpoints (1500/min for
    /// instrument, mini ticker and ticker): https://help.grvt.io/en/articles/9636566.
    /// 500/min rounds down to 8/s.
    const DEFAULT_REQUESTS_PER_SECOND: u32 = 8;
    const WS_CONN_NAME: &str = "grvt";

    /// Mainnet.
    pub fn new() -> Self {
        Self::new_with_rate_limit(Self::DEFAULT_REQUESTS_PER_SECOND)
    }

    pub fn new_with_rate_limit(requests_per_second: u32) -> Self {
        Self::with_urls(
            GRVT_MARKET_DATA_URL,
            GRVT_MARKET_DATA_WSS_URL,
            requests_per_second,
        )
    }

    /// Any environment, e.g. testnet: `with_urls(GRVT_TESTNET_MARKET_DATA_URL,
    /// GRVT_TESTNET_MARKET_DATA_WSS_URL, rps)`.
    pub fn with_urls(rest_url: &str, wss_url: &str, requests_per_second: u32) -> Self {
        Self {
            market_api: MarketApi::new(rest_url, requests_per_second),
            ws_config: ws_config(Self::WS_CONN_NAME, wss_url, vec![]),
        }
    }

    pub fn ws_config(&self) -> roshar_ws_mgr::Config {
        self.ws_config.clone()
    }

    pub fn ws_config_mut(&mut self) -> &mut roshar_ws_mgr::Config {
        &mut self.ws_config
    }

    fn sub(
        &self,
        manager: &Arc<Manager>,
        stream: &str,
        selectors: Vec<String>,
        id: Option<u64>,
    ) -> Result<()> {
        send(
            manager,
            &self.ws_config,
            subscribe_message(stream, &selectors, id),
        )
    }

    fn unsub(
        &self,
        manager: &Arc<Manager>,
        stream: &str,
        selectors: Vec<String>,
        id: Option<u64>,
    ) -> Result<()> {
        send(
            manager,
            &self.ws_config,
            unsubscribe_message(stream, &selectors, id),
        )
    }

    /// `v1.book.s` at `{instrument}@{rate_ms}-{depth}`. Rate 500 or 1000 ms; depth 10, 50, 100
    /// or 500.
    pub fn subscribe_book_snapshot(
        &self,
        manager: &Arc<Manager>,
        instruments: &[&str],
        rate_ms: u32,
        depth: u32,
        id: Option<u64>,
    ) -> Result<()> {
        let sels = instruments
            .iter()
            .map(|i| selector::book_snapshot(i, rate_ms, depth))
            .collect();
        self.sub(manager, STREAM_BOOK_SNAPSHOT, sels, id)
    }

    pub fn unsubscribe_book_snapshot(
        &self,
        manager: &Arc<Manager>,
        instruments: &[&str],
        rate_ms: u32,
        depth: u32,
        id: Option<u64>,
    ) -> Result<()> {
        let sels = instruments
            .iter()
            .map(|i| selector::book_snapshot(i, rate_ms, depth))
            .collect();
        self.unsub(manager, STREAM_BOOK_SNAPSHOT, sels, id)
    }

    /// `v1.book.d` at `{instrument}@{rate_ms}` (full depth snapshot, then changed levels;
    /// size `0` removes a level). Rate 50, 100, 500 or 1000 ms.
    pub fn subscribe_book_delta(
        &self,
        manager: &Arc<Manager>,
        instruments: &[&str],
        rate_ms: u32,
        id: Option<u64>,
    ) -> Result<()> {
        let sels = instruments
            .iter()
            .map(|i| selector::book_delta(i, rate_ms))
            .collect();
        self.sub(manager, STREAM_BOOK_DELTA, sels, id)
    }

    pub fn unsubscribe_book_delta(
        &self,
        manager: &Arc<Manager>,
        instruments: &[&str],
        rate_ms: u32,
        id: Option<u64>,
    ) -> Result<()> {
        let sels = instruments
            .iter()
            .map(|i| selector::book_delta(i, rate_ms))
            .collect();
        self.unsub(manager, STREAM_BOOK_DELTA, sels, id)
    }

    /// `v1.trade` at `{instrument}@{limit}`; `limit` (50, 200, 500, 1000) recent trades are sent
    /// first.
    pub fn subscribe_trades(
        &self,
        manager: &Arc<Manager>,
        instruments: &[&str],
        limit: u32,
        id: Option<u64>,
    ) -> Result<()> {
        let sels = instruments
            .iter()
            .map(|i| selector::trade(i, limit))
            .collect();
        self.sub(manager, STREAM_TRADE, sels, id)
    }

    pub fn unsubscribe_trades(
        &self,
        manager: &Arc<Manager>,
        instruments: &[&str],
        limit: u32,
        id: Option<u64>,
    ) -> Result<()> {
        let sels = instruments
            .iter()
            .map(|i| selector::trade(i, limit))
            .collect();
        self.unsub(manager, STREAM_TRADE, sels, id)
    }

    fn mini_stream(mode: GrvtFeedMode) -> &'static str {
        match mode {
            GrvtFeedMode::Snapshot => STREAM_MINI_TICKER_SNAPSHOT,
            GrvtFeedMode::Delta => STREAM_MINI_TICKER_DELTA,
        }
    }

    fn ticker_stream(mode: GrvtFeedMode) -> &'static str {
        match mode {
            GrvtFeedMode::Snapshot => STREAM_TICKER_SNAPSHOT,
            GrvtFeedMode::Delta => STREAM_TICKER_DELTA,
        }
    }

    /// `v1.mini.s` / `v1.mini.d` (best bid/ask, last, mark, index) at `{instrument}@{rate_ms}`.
    pub fn subscribe_mini_ticker(
        &self,
        manager: &Arc<Manager>,
        instruments: &[&str],
        mode: GrvtFeedMode,
        rate_ms: u32,
        id: Option<u64>,
    ) -> Result<()> {
        let sels = instruments
            .iter()
            .map(|i| selector::ticker(i, rate_ms))
            .collect();
        self.sub(manager, Self::mini_stream(mode), sels, id)
    }

    pub fn unsubscribe_mini_ticker(
        &self,
        manager: &Arc<Manager>,
        instruments: &[&str],
        mode: GrvtFeedMode,
        rate_ms: u32,
        id: Option<u64>,
    ) -> Result<()> {
        let sels = instruments
            .iter()
            .map(|i| selector::ticker(i, rate_ms))
            .collect();
        self.unsub(manager, Self::mini_stream(mode), sels, id)
    }

    /// `v1.ticker.s` / `v1.ticker.d` at `{instrument}@{rate_ms}`. Carries 24h stats, open
    /// interest and the funding rate / next funding time (GRVT has no separate funding stream).
    pub fn subscribe_ticker(
        &self,
        manager: &Arc<Manager>,
        instruments: &[&str],
        mode: GrvtFeedMode,
        rate_ms: u32,
        id: Option<u64>,
    ) -> Result<()> {
        let sels = instruments
            .iter()
            .map(|i| selector::ticker(i, rate_ms))
            .collect();
        self.sub(manager, Self::ticker_stream(mode), sels, id)
    }

    pub fn unsubscribe_ticker(
        &self,
        manager: &Arc<Manager>,
        instruments: &[&str],
        mode: GrvtFeedMode,
        rate_ms: u32,
        id: Option<u64>,
    ) -> Result<()> {
        let sels = instruments
            .iter()
            .map(|i| selector::ticker(i, rate_ms))
            .collect();
        self.unsub(manager, Self::ticker_stream(mode), sels, id)
    }

    /// `v1.candle` at `{instrument}@{interval}-{type}`; the 5 latest candles are sent first.
    pub fn subscribe_candles(
        &self,
        manager: &Arc<Manager>,
        instruments: &[&str],
        interval: GrvtCandleInterval,
        kind: GrvtCandleType,
        id: Option<u64>,
    ) -> Result<()> {
        let sels = instruments
            .iter()
            .map(|i| selector::candle(i, interval, kind))
            .collect();
        self.sub(manager, STREAM_CANDLE, sels, id)
    }

    pub fn unsubscribe_candles(
        &self,
        manager: &Arc<Manager>,
        instruments: &[&str],
        interval: GrvtCandleInterval,
        kind: GrvtCandleType,
        id: Option<u64>,
    ) -> Result<()> {
        let sels = instruments
            .iter()
            .map(|i| selector::candle(i, interval, kind))
            .collect();
        self.unsub(manager, STREAM_CANDLE, sels, id)
    }

    // JSON-RPC market-data queries on the public websocket. `params` are the REST bodies; each
    // reply is a `GrvtRpcResponse<R>` whose `R` is the matching REST reply type and whose
    // `method` echoes the query (e.g. `"v1/ticker"`).

    fn query<P: Serialize>(
        &self,
        manager: &Arc<Manager>,
        method: &str,
        params: P,
        id: Option<u64>,
    ) -> Result<()> {
        send(
            manager,
            &self.ws_config,
            GrvtRpcRequest::new(method, params, id).to_json(),
        )
    }

    /// `v1/instrument`; reply `GrvtRpcResponse<GrvtResult<GrvtInstrument>>`.
    pub fn query_instrument(
        &self,
        manager: &Arc<Manager>,
        instrument: &str,
        id: Option<u64>,
    ) -> Result<()> {
        self.query(
            manager,
            "v1/instrument",
            GrvtInstrumentRequest { instrument },
            id,
        )
    }

    /// `v1/all_instruments`; reply `GrvtRpcResponse<GrvtResult<Vec<GrvtInstrument>>>`.
    pub fn query_all_instruments(
        &self,
        manager: &Arc<Manager>,
        is_active: Option<bool>,
        kinds: Option<&[GrvtKind]>,
        id: Option<u64>,
    ) -> Result<()> {
        self.query(
            manager,
            "v1/all_instruments",
            GrvtAllInstrumentsRequest { is_active, kinds },
            id,
        )
    }

    /// `v1/instruments`; reply `GrvtRpcResponse<GrvtResult<Vec<GrvtInstrument>>>`.
    pub fn query_instruments(
        &self,
        manager: &Arc<Manager>,
        query: &GrvtInstrumentsQuery,
        id: Option<u64>,
    ) -> Result<()> {
        self.query(manager, "v1/instruments", query, id)
    }

    /// `v1/currency`; reply `GrvtRpcResponse<GrvtResult<Vec<GrvtCurrency>>>`.
    pub fn query_currencies(&self, manager: &Arc<Manager>, id: Option<u64>) -> Result<()> {
        self.query(manager, "v1/currency", GrvtEmptyRequest {}, id)
    }

    /// `v1/supported_assets`; reply `GrvtRpcResponse<GrvtSupportedAssets>`.
    pub fn query_supported_assets(&self, manager: &Arc<Manager>, id: Option<u64>) -> Result<()> {
        self.query(manager, "v1/supported_assets", GrvtEmptyRequest {}, id)
    }

    /// `v1/margin_rules`; reply `GrvtRpcResponse<GrvtMarginRules>`.
    pub fn query_margin_rules(
        &self,
        manager: &Arc<Manager>,
        instrument: &str,
        id: Option<u64>,
    ) -> Result<()> {
        self.query(
            manager,
            "v1/margin_rules",
            GrvtInstrumentRequest { instrument },
            id,
        )
    }

    /// `v1/get_all_collateral_asset_info`; reply `GrvtRpcResponse<GrvtCollateralAssets>`.
    pub fn query_all_collateral_asset_info(
        &self,
        manager: &Arc<Manager>,
        id: Option<u64>,
    ) -> Result<()> {
        self.query(
            manager,
            "v1/get_all_collateral_asset_info",
            GrvtEmptyRequest {},
            id,
        )
    }

    /// `v1/mini`; reply `GrvtRpcResponse<GrvtResult<GrvtMiniTicker>>`.
    pub fn query_mini_ticker(
        &self,
        manager: &Arc<Manager>,
        instrument: &str,
        id: Option<u64>,
    ) -> Result<()> {
        self.query(manager, "v1/mini", GrvtInstrumentRequest { instrument }, id)
    }

    /// `v1/ticker`; reply `GrvtRpcResponse<GrvtResult<GrvtTicker>>`.
    pub fn query_ticker(
        &self,
        manager: &Arc<Manager>,
        instrument: &str,
        id: Option<u64>,
    ) -> Result<()> {
        self.query(
            manager,
            "v1/ticker",
            GrvtInstrumentRequest { instrument },
            id,
        )
    }

    /// `v1/book`; reply `GrvtRpcResponse<GrvtResult<GrvtOrderbook>>`. Depth 10, 50, 100 or 500.
    pub fn query_orderbook(
        &self,
        manager: &Arc<Manager>,
        instrument: &str,
        depth: u32,
        id: Option<u64>,
    ) -> Result<()> {
        self.query(
            manager,
            "v1/book",
            GrvtBookRequest { instrument, depth },
            id,
        )
    }

    /// `v1/trade`; reply `GrvtRpcResponse<GrvtResult<Vec<GrvtTrade>>>`.
    pub fn query_trades(
        &self,
        manager: &Arc<Manager>,
        instrument: &str,
        limit: u32,
        id: Option<u64>,
    ) -> Result<()> {
        self.query(
            manager,
            "v1/trade",
            GrvtTradeRequest { instrument, limit },
            id,
        )
    }

    /// `v1/trade_history`; reply `GrvtRpcResponse<GrvtPage<GrvtTrade>>`.
    pub fn query_trade_history(
        &self,
        manager: &Arc<Manager>,
        instrument: &str,
        page: &GrvtPageQuery,
        id: Option<u64>,
    ) -> Result<()> {
        self.query(
            manager,
            "v1/trade_history",
            GrvtTradeHistoryRequest { instrument, page },
            id,
        )
    }

    /// `v1/kline`; reply `GrvtRpcResponse<GrvtPage<GrvtCandle>>`.
    pub fn query_candles(
        &self,
        manager: &Arc<Manager>,
        instrument: &str,
        interval: GrvtCandleInterval,
        kind: GrvtCandleType,
        page: &GrvtPageQuery,
        id: Option<u64>,
    ) -> Result<()> {
        self.query(
            manager,
            "v1/kline",
            GrvtKlineRequest {
                instrument,
                interval,
                kind,
                page,
            },
            id,
        )
    }

    /// `v1/funding`; reply `GrvtRpcResponse<GrvtPage<GrvtFundingRate>>`.
    pub fn query_funding_history(
        &self,
        manager: &Arc<Manager>,
        instrument: &str,
        agg_type: Option<GrvtFundingAggregation>,
        page: &GrvtPageQuery,
        id: Option<u64>,
    ) -> Result<()> {
        self.query(
            manager,
            "v1/funding",
            GrvtFundingRequest {
                instrument,
                page,
                agg_type,
            },
            id,
        )
    }
}

#[async_trait]
pub trait GrvtApi {
    async fn get_instrument(&self, instrument: &str) -> ApiResult<GrvtInstrument>;

    async fn get_all_instruments(
        &self,
        is_active: Option<bool>,
        kinds: Option<&[GrvtKind]>,
    ) -> ApiResult<Vec<GrvtInstrument>>;

    async fn get_instruments(&self, query: &GrvtInstrumentsQuery)
        -> ApiResult<Vec<GrvtInstrument>>;

    async fn get_currencies(&self) -> ApiResult<Vec<GrvtCurrency>>;

    async fn get_supported_assets(&self) -> ApiResult<GrvtSupportedAssets>;

    async fn get_all_collateral_asset_info(&self) -> ApiResult<GrvtCollateralAssets>;

    async fn get_margin_rules(&self, instrument: &str) -> ApiResult<GrvtMarginRules>;

    async fn get_mini_ticker(&self, instrument: &str) -> ApiResult<GrvtMiniTicker>;

    async fn get_ticker(&self, instrument: &str) -> ApiResult<GrvtTicker>;

    async fn get_orderbook(&self, instrument: &str, depth: u32) -> ApiResult<GrvtOrderbook>;

    async fn get_trades(&self, instrument: &str, limit: u32) -> ApiResult<Vec<GrvtTrade>>;

    async fn get_trade_history(
        &self,
        instrument: &str,
        page: &GrvtPageQuery,
    ) -> ApiResult<GrvtPage<GrvtTrade>>;

    async fn get_candles(
        &self,
        instrument: &str,
        interval: GrvtCandleInterval,
        kind: GrvtCandleType,
        page: &GrvtPageQuery,
    ) -> ApiResult<GrvtPage<GrvtCandle>>;

    async fn get_funding_history(
        &self,
        instrument: &str,
        agg_type: Option<GrvtFundingAggregation>,
        page: &GrvtPageQuery,
    ) -> ApiResult<GrvtPage<GrvtFundingRate>>;

    async fn get_server_time(&self) -> ApiResult<GrvtServerTime>;
}

#[async_trait]
impl GrvtApi for GrvtClient {
    async fn get_instrument(&self, instrument: &str) -> ApiResult<GrvtInstrument> {
        self.market_api.get_instrument(instrument).await
    }

    async fn get_all_instruments(
        &self,
        is_active: Option<bool>,
        kinds: Option<&[GrvtKind]>,
    ) -> ApiResult<Vec<GrvtInstrument>> {
        self.market_api.get_all_instruments(is_active, kinds).await
    }

    async fn get_instruments(
        &self,
        query: &GrvtInstrumentsQuery,
    ) -> ApiResult<Vec<GrvtInstrument>> {
        self.market_api.get_instruments(query).await
    }

    async fn get_currencies(&self) -> ApiResult<Vec<GrvtCurrency>> {
        self.market_api.get_currencies().await
    }

    async fn get_supported_assets(&self) -> ApiResult<GrvtSupportedAssets> {
        self.market_api.get_supported_assets().await
    }

    async fn get_all_collateral_asset_info(&self) -> ApiResult<GrvtCollateralAssets> {
        self.market_api.get_all_collateral_asset_info().await
    }

    async fn get_margin_rules(&self, instrument: &str) -> ApiResult<GrvtMarginRules> {
        self.market_api.get_margin_rules(instrument).await
    }

    async fn get_mini_ticker(&self, instrument: &str) -> ApiResult<GrvtMiniTicker> {
        self.market_api.get_mini_ticker(instrument).await
    }

    async fn get_ticker(&self, instrument: &str) -> ApiResult<GrvtTicker> {
        self.market_api.get_ticker(instrument).await
    }

    async fn get_orderbook(&self, instrument: &str, depth: u32) -> ApiResult<GrvtOrderbook> {
        self.market_api.get_orderbook(instrument, depth).await
    }

    async fn get_trades(&self, instrument: &str, limit: u32) -> ApiResult<Vec<GrvtTrade>> {
        self.market_api.get_trades(instrument, limit).await
    }

    async fn get_trade_history(
        &self,
        instrument: &str,
        page: &GrvtPageQuery,
    ) -> ApiResult<GrvtPage<GrvtTrade>> {
        self.market_api.get_trade_history(instrument, page).await
    }

    async fn get_candles(
        &self,
        instrument: &str,
        interval: GrvtCandleInterval,
        kind: GrvtCandleType,
        page: &GrvtPageQuery,
    ) -> ApiResult<GrvtPage<GrvtCandle>> {
        self.market_api
            .get_candles(instrument, interval, kind, page)
            .await
    }

    async fn get_funding_history(
        &self,
        instrument: &str,
        agg_type: Option<GrvtFundingAggregation>,
        page: &GrvtPageQuery,
    ) -> ApiResult<GrvtPage<GrvtFundingRate>> {
        self.market_api
            .get_funding_history(instrument, agg_type, page)
            .await
    }

    async fn get_server_time(&self) -> ApiResult<GrvtServerTime> {
        self.market_api.get_server_time().await
    }
}

/// Private trading websocket: order/state/fill/position/cancel streams and order RPCs.
/// The session cookie and account id go on the upgrade request; build a new config after
/// re-login.
pub struct GrvtTradingStream {
    ws_config: Config,
}

impl GrvtTradingStream {
    const WS_CONN_NAME: &str = "grvt-trades";

    /// `wss_url`: [`GRVT_TRADES_WSS_URL`] or [`GRVT_TESTNET_TRADES_WSS_URL`].
    pub fn new(wss_url: &str, session: &GrvtSession) -> Self {
        Self {
            ws_config: ws_config(Self::WS_CONN_NAME, wss_url, session.headers()),
        }
    }

    pub fn ws_config(&self) -> roshar_ws_mgr::Config {
        self.ws_config.clone()
    }

    pub fn ws_config_mut(&mut self) -> &mut roshar_ws_mgr::Config {
        &mut self.ws_config
    }

    fn account_sub(
        &self,
        manager: &Arc<Manager>,
        stream: &str,
        sub_account_id: u64,
        instrument: Option<&str>,
        id: Option<u64>,
        subscribe: bool,
    ) -> Result<()> {
        let sels = [selector::account(sub_account_id, instrument)];
        let msg = if subscribe {
            subscribe_message(stream, &sels, id)
        } else {
            unsubscribe_message(stream, &sels, id)
        };
        send(manager, &self.ws_config, msg)
    }

    /// `v1.order`: full order updates; `instrument: None` for all instruments.
    pub fn subscribe_orders(
        &self,
        manager: &Arc<Manager>,
        sub_account_id: u64,
        instrument: Option<&str>,
        id: Option<u64>,
    ) -> Result<()> {
        self.account_sub(manager, STREAM_ORDER, sub_account_id, instrument, id, true)
    }

    pub fn unsubscribe_orders(
        &self,
        manager: &Arc<Manager>,
        sub_account_id: u64,
        instrument: Option<&str>,
        id: Option<u64>,
    ) -> Result<()> {
        self.account_sub(manager, STREAM_ORDER, sub_account_id, instrument, id, false)
    }

    /// `v1.state`: order state changes only.
    pub fn subscribe_order_states(
        &self,
        manager: &Arc<Manager>,
        sub_account_id: u64,
        instrument: Option<&str>,
        id: Option<u64>,
    ) -> Result<()> {
        self.account_sub(
            manager,
            STREAM_ORDER_STATE,
            sub_account_id,
            instrument,
            id,
            true,
        )
    }

    pub fn unsubscribe_order_states(
        &self,
        manager: &Arc<Manager>,
        sub_account_id: u64,
        instrument: Option<&str>,
        id: Option<u64>,
    ) -> Result<()> {
        self.account_sub(
            manager,
            STREAM_ORDER_STATE,
            sub_account_id,
            instrument,
            id,
            false,
        )
    }

    /// `v1.fill`: private fills.
    pub fn subscribe_fills(
        &self,
        manager: &Arc<Manager>,
        sub_account_id: u64,
        instrument: Option<&str>,
        id: Option<u64>,
    ) -> Result<()> {
        self.account_sub(manager, STREAM_FILL, sub_account_id, instrument, id, true)
    }

    pub fn unsubscribe_fills(
        &self,
        manager: &Arc<Manager>,
        sub_account_id: u64,
        instrument: Option<&str>,
        id: Option<u64>,
    ) -> Result<()> {
        self.account_sub(manager, STREAM_FILL, sub_account_id, instrument, id, false)
    }

    /// `v1.position`.
    pub fn subscribe_positions(
        &self,
        manager: &Arc<Manager>,
        sub_account_id: u64,
        instrument: Option<&str>,
        id: Option<u64>,
    ) -> Result<()> {
        self.account_sub(
            manager,
            STREAM_POSITION,
            sub_account_id,
            instrument,
            id,
            true,
        )
    }

    pub fn unsubscribe_positions(
        &self,
        manager: &Arc<Manager>,
        sub_account_id: u64,
        instrument: Option<&str>,
        id: Option<u64>,
    ) -> Result<()> {
        self.account_sub(
            manager,
            STREAM_POSITION,
            sub_account_id,
            instrument,
            id,
            false,
        )
    }

    /// `v1.cancel`: TTL expiry of cancel requests (selector is the sub account only).
    pub fn subscribe_cancel_status(
        &self,
        manager: &Arc<Manager>,
        sub_account_id: u64,
        id: Option<u64>,
    ) -> Result<()> {
        self.account_sub(
            manager,
            STREAM_CANCEL_STATUS,
            sub_account_id,
            None,
            id,
            true,
        )
    }

    pub fn unsubscribe_cancel_status(
        &self,
        manager: &Arc<Manager>,
        sub_account_id: u64,
        id: Option<u64>,
    ) -> Result<()> {
        self.account_sub(
            manager,
            STREAM_CANCEL_STATUS,
            sub_account_id,
            None,
            id,
            false,
        )
    }

    /// JSON-RPC `v1/create_order`; the reply is a `GrvtRpcResponse<GrvtResult<GrvtOrder>>`.
    pub fn create_order(
        &self,
        manager: &Arc<Manager>,
        order: &GrvtOrderRequest,
        id: Option<u64>,
    ) -> Result<()> {
        let params = GrvtCreateOrderRequest {
            order: order.clone(),
        };
        let msg = GrvtRpcRequest::new("v1/create_order", params, id).to_json();
        send(manager, &self.ws_config, msg)
    }

    /// JSON-RPC `v1/cancel_order`; the reply is a `GrvtRpcResponse<GrvtResult<GrvtAck>>`.
    pub fn cancel_order(
        &self,
        manager: &Arc<Manager>,
        request: &GrvtCancelOrderRequest,
        id: Option<u64>,
    ) -> Result<()> {
        let msg = GrvtRpcRequest::new("v1/cancel_order", request, id).to_json();
        send(manager, &self.ws_config, msg)
    }

    /// JSON-RPC `v1/cancel_all_orders`; the reply is a `GrvtRpcResponse<GrvtResult<GrvtAck>>`.
    pub fn cancel_all_orders(
        &self,
        manager: &Arc<Manager>,
        request: &GrvtCancelAllOrdersRequest,
        id: Option<u64>,
    ) -> Result<()> {
        let msg = GrvtRpcRequest::new("v1/cancel_all_orders", request, id).to_json();
        send(manager, &self.ws_config, msg)
    }
}
