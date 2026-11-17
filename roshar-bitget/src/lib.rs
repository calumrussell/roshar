/// Wire and message types for this exchange (websocket messages, REST payloads).
pub mod types;

// Endpoints
pub const BITGET_WSS_URL: &str = "wss://ws.bitget.com/v2/ws/public";
pub const BITGET_REST_URL: &str = "https://api.bitget.com";

pub(crate) mod rest;

pub use rest::MarketApi as BitgetMarketApi;
pub use rest::SpotMarketApi as BitgetSpotMarketApi;
pub use types::{
    BitgetBookChannel, BitgetCandle, BitgetCandleParams, BitgetContract, BitgetCurrentFundingRate,
    BitgetFillsHistoryParams, BitgetHistoricalFundingRate, BitgetKLineType, BitgetMarketTrade,
    BitgetMergeDepth, BitgetNextFundingTime, BitgetOiLimit, BitgetOpenInterest,
    BitgetOpenInterestEntry, BitgetProductType, BitgetServerTime, BitgetSpotAuction,
    BitgetSpotCandle, BitgetSpotOrderBook, BitgetSpotSymbol, BitgetSpotTicker, BitgetSymbolPrice,
    BitgetTickerData, BitgetWsIncoming, BitgetWsInstType,
};

use crate::types::BitgetWssMessage;
use anyhow::Result;
use async_trait::async_trait;
use roshar_http::RateLimitedClient;
use roshar_ws_mgr::{Config, Manager, Message};
use std::collections::HashMap;
use std::sync::Arc;

type BoxError = Box<dyn std::error::Error + Send + Sync>;

pub struct BitgetClient {
    market_api: rest::MarketApi,
    spot_api: rest::SpotMarketApi,
    ws_config: Config,
}

impl BitgetClient {
    /// Most public market endpoints are limited to 20 requests per second per IP (a few to 10,
    /// see each method). Futures and spot share one limiter.
    const DEFAULT_REQUESTS_PER_SECOND: u32 = 20;
    const WS_CONN_NAME: &str = "bitget";

    pub fn new() -> Self {
        Self::new_with_rate_limit(Self::DEFAULT_REQUESTS_PER_SECOND)
    }

    pub fn new_with_rate_limit(requests_per_second: u32) -> Self {
        let http = Arc::new(RateLimitedClient::new(requests_per_second, 1));
        Self {
            market_api: rest::MarketApi::new_with_client(http.clone()),
            spot_api: rest::SpotMarketApi::new_with_client(http),
            ws_config: Config {
                name: Self::WS_CONN_NAME.to_string(),
                url: BITGET_WSS_URL.to_string(),
                // Bitget: send text "ping" every 30s, expect "pong"; idle connections are
                // closed after 2 minutes without a ping.
                ping_duration: 30,
                ping_message: BitgetWssMessage::ping(),
                ping_timeout: 15,
                reconnect_timeout: 5000,
                use_text_ping: Some(true),
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

    fn send_all(
        &self,
        manager: &Arc<Manager>,
        symbols: &[&str],
        build: impl Fn(&str) -> BitgetWssMessage,
    ) -> Result<()> {
        for symbol in symbols {
            let msg = build(symbol).to_json();
            manager.write(
                &self.ws_config.name,
                Message::TextMessage(self.ws_config.name.clone(), msg),
            )?;
        }
        Ok(())
    }

    /// `books` (full depth: snapshot, then updates).
    pub fn subscribe_depth(
        &self,
        manager: &Arc<Manager>,
        inst_type: impl Into<BitgetWsInstType> + Copy,
        symbols: &[&str],
    ) -> Result<()> {
        self.send_all(manager, symbols, |s| BitgetWssMessage::depth(inst_type, s))
    }

    pub fn unsubscribe_depth(
        &self,
        manager: &Arc<Manager>,
        inst_type: impl Into<BitgetWsInstType> + Copy,
        symbols: &[&str],
    ) -> Result<()> {
        self.send_all(manager, symbols, |s| {
            BitgetWssMessage::depth_unsub(inst_type, s)
        })
    }

    /// Any of `books`, `books1`, `books5`, `books15`.
    pub fn subscribe_books(
        &self,
        manager: &Arc<Manager>,
        inst_type: impl Into<BitgetWsInstType> + Copy,
        channel: BitgetBookChannel,
        symbols: &[&str],
    ) -> Result<()> {
        self.send_all(manager, symbols, |s| {
            BitgetWssMessage::books(inst_type, channel, s)
        })
    }

    pub fn unsubscribe_books(
        &self,
        manager: &Arc<Manager>,
        inst_type: impl Into<BitgetWsInstType> + Copy,
        channel: BitgetBookChannel,
        symbols: &[&str],
    ) -> Result<()> {
        self.send_all(manager, symbols, |s| {
            BitgetWssMessage::books_unsub(inst_type, channel, s)
        })
    }

    pub fn subscribe_trades(
        &self,
        manager: &Arc<Manager>,
        inst_type: impl Into<BitgetWsInstType> + Copy,
        symbols: &[&str],
    ) -> Result<()> {
        self.send_all(manager, symbols, |s| BitgetWssMessage::trades(inst_type, s))
    }

    pub fn unsubscribe_trades(
        &self,
        manager: &Arc<Manager>,
        inst_type: impl Into<BitgetWsInstType> + Copy,
        symbols: &[&str],
    ) -> Result<()> {
        self.send_all(manager, symbols, |s| {
            BitgetWssMessage::trades_unsub(inst_type, s)
        })
    }

    pub fn subscribe_tickers(
        &self,
        manager: &Arc<Manager>,
        inst_type: impl Into<BitgetWsInstType> + Copy,
        symbols: &[&str],
    ) -> Result<()> {
        self.send_all(manager, symbols, |s| {
            BitgetWssMessage::tickers(inst_type, s)
        })
    }

    pub fn unsubscribe_tickers(
        &self,
        manager: &Arc<Manager>,
        inst_type: impl Into<BitgetWsInstType> + Copy,
        symbols: &[&str],
    ) -> Result<()> {
        self.send_all(manager, symbols, |s| {
            BitgetWssMessage::tickers_unsub(inst_type, s)
        })
    }

    /// `candle<interval>`, e.g. `interval = "1m"`.
    pub fn subscribe_candles(
        &self,
        manager: &Arc<Manager>,
        inst_type: impl Into<BitgetWsInstType> + Copy,
        interval: &str,
        symbols: &[&str],
    ) -> Result<()> {
        self.send_all(manager, symbols, |s| {
            BitgetWssMessage::candles(inst_type, interval, s)
        })
    }

    pub fn unsubscribe_candles(
        &self,
        manager: &Arc<Manager>,
        inst_type: impl Into<BitgetWsInstType> + Copy,
        interval: &str,
        symbols: &[&str],
    ) -> Result<()> {
        self.send_all(manager, symbols, |s| {
            BitgetWssMessage::candles_unsub(inst_type, interval, s)
        })
    }

    /// Spot call auction.
    pub fn subscribe_auction(&self, manager: &Arc<Manager>, symbols: &[&str]) -> Result<()> {
        self.send_all(manager, symbols, BitgetWssMessage::auction)
    }

    pub fn unsubscribe_auction(&self, manager: &Arc<Manager>, symbols: &[&str]) -> Result<()> {
        self.send_all(manager, symbols, BitgetWssMessage::auction_unsub)
    }
}

impl Default for BitgetClient {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
pub trait BitgetApi {
    async fn get_server_time(&self) -> Result<BitgetServerTime, BoxError>;

    // Futures (`/api/v2/mix/market/*`)

    async fn get_contracts(
        &self,
        product_type: BitgetProductType,
        symbol: Option<&str>,
    ) -> Result<HashMap<String, BitgetContract>, BoxError>;

    async fn get_tickers(
        &self,
        product_type: BitgetProductType,
    ) -> Result<HashMap<String, BitgetTickerData>, BoxError>;

    async fn get_ticker(
        &self,
        product_type: BitgetProductType,
        symbol: &str,
    ) -> Result<Vec<BitgetTickerData>, BoxError>;

    async fn get_merge_depth(
        &self,
        product_type: BitgetProductType,
        symbol: &str,
        precision: Option<&str>,
        limit: Option<&str>,
    ) -> Result<BitgetMergeDepth, BoxError>;

    async fn get_recent_trades(
        &self,
        product_type: BitgetProductType,
        symbol: &str,
        limit: Option<u32>,
    ) -> Result<Vec<BitgetMarketTrade>, BoxError>;

    async fn get_trades_history(
        &self,
        product_type: BitgetProductType,
        symbol: &str,
        params: &BitgetFillsHistoryParams,
    ) -> Result<Vec<BitgetMarketTrade>, BoxError>;

    async fn get_candles(
        &self,
        product_type: BitgetProductType,
        symbol: &str,
        granularity: &str,
        kline_type: Option<BitgetKLineType>,
        params: BitgetCandleParams,
    ) -> Result<Vec<BitgetCandle>, BoxError>;

    async fn get_history_candles(
        &self,
        product_type: BitgetProductType,
        symbol: &str,
        granularity: &str,
        params: BitgetCandleParams,
    ) -> Result<Vec<BitgetCandle>, BoxError>;

    async fn get_history_index_candles(
        &self,
        product_type: BitgetProductType,
        symbol: &str,
        granularity: &str,
        params: BitgetCandleParams,
    ) -> Result<Vec<BitgetCandle>, BoxError>;

    async fn get_history_mark_candles(
        &self,
        product_type: BitgetProductType,
        symbol: &str,
        granularity: &str,
        params: BitgetCandleParams,
    ) -> Result<Vec<BitgetCandle>, BoxError>;

    async fn get_open_interest(
        &self,
        product_type: BitgetProductType,
        symbol: &str,
    ) -> Result<BitgetOpenInterest, BoxError>;

    async fn get_current_funding_rates(
        &self,
        product_type: BitgetProductType,
        symbol: Option<&str>,
    ) -> Result<Vec<BitgetCurrentFundingRate>, BoxError>;

    async fn get_historical_funding_rates(
        &self,
        product_type: BitgetProductType,
        symbol: &str,
        page_size: u32,
        page_no: u32,
    ) -> Result<Vec<BitgetHistoricalFundingRate>, BoxError>;

    async fn get_next_funding_time(
        &self,
        product_type: BitgetProductType,
        symbol: &str,
    ) -> Result<Vec<BitgetNextFundingTime>, BoxError>;

    async fn get_symbol_price(
        &self,
        product_type: BitgetProductType,
        symbol: &str,
    ) -> Result<Vec<BitgetSymbolPrice>, BoxError>;

    async fn get_oi_limits(
        &self,
        product_type: BitgetProductType,
        symbol: Option<&str>,
    ) -> Result<Vec<BitgetOiLimit>, BoxError>;

    // Spot (`/api/v2/spot/*`)

    async fn get_spot_symbols(
        &self,
        symbol: Option<&str>,
    ) -> Result<Vec<BitgetSpotSymbol>, BoxError>;

    async fn get_spot_tickers(
        &self,
        symbol: Option<&str>,
    ) -> Result<Vec<BitgetSpotTicker>, BoxError>;

    async fn get_spot_orderbook(
        &self,
        symbol: &str,
        step: Option<&str>,
        limit: Option<u32>,
    ) -> Result<BitgetSpotOrderBook, BoxError>;

    async fn get_spot_merge_depth(
        &self,
        symbol: &str,
        precision: Option<&str>,
        limit: Option<&str>,
    ) -> Result<BitgetMergeDepth, BoxError>;

    async fn get_spot_recent_trades(
        &self,
        symbol: &str,
        limit: Option<u32>,
    ) -> Result<Vec<BitgetMarketTrade>, BoxError>;

    async fn get_spot_trades_history(
        &self,
        symbol: &str,
        params: &BitgetFillsHistoryParams,
    ) -> Result<Vec<BitgetMarketTrade>, BoxError>;

    async fn get_spot_candles(
        &self,
        symbol: &str,
        granularity: &str,
        start_time: Option<i64>,
        end_time: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<BitgetSpotCandle>, BoxError>;

    async fn get_spot_history_candles(
        &self,
        symbol: &str,
        granularity: &str,
        end_time: i64,
        limit: Option<u32>,
    ) -> Result<Vec<BitgetSpotCandle>, BoxError>;

    async fn get_spot_auction(&self, symbol: &str) -> Result<BitgetSpotAuction, BoxError>;
}

#[async_trait]
impl BitgetApi for BitgetClient {
    async fn get_server_time(&self) -> Result<BitgetServerTime, BoxError> {
        self.market_api.get_server_time().await
    }

    async fn get_contracts(
        &self,
        product_type: BitgetProductType,
        symbol: Option<&str>,
    ) -> Result<HashMap<String, BitgetContract>, BoxError> {
        self.market_api.get_contracts(product_type, symbol).await
    }

    async fn get_tickers(
        &self,
        product_type: BitgetProductType,
    ) -> Result<HashMap<String, BitgetTickerData>, BoxError> {
        self.market_api.get_tickers(product_type).await
    }

    async fn get_ticker(
        &self,
        product_type: BitgetProductType,
        symbol: &str,
    ) -> Result<Vec<BitgetTickerData>, BoxError> {
        self.market_api.get_ticker(product_type, symbol).await
    }

    async fn get_merge_depth(
        &self,
        product_type: BitgetProductType,
        symbol: &str,
        precision: Option<&str>,
        limit: Option<&str>,
    ) -> Result<BitgetMergeDepth, BoxError> {
        self.market_api
            .get_merge_depth(product_type, symbol, precision, limit)
            .await
    }

    async fn get_recent_trades(
        &self,
        product_type: BitgetProductType,
        symbol: &str,
        limit: Option<u32>,
    ) -> Result<Vec<BitgetMarketTrade>, BoxError> {
        self.market_api
            .get_recent_trades(product_type, symbol, limit)
            .await
    }

    async fn get_trades_history(
        &self,
        product_type: BitgetProductType,
        symbol: &str,
        params: &BitgetFillsHistoryParams,
    ) -> Result<Vec<BitgetMarketTrade>, BoxError> {
        self.market_api
            .get_trades_history(product_type, symbol, params)
            .await
    }

    async fn get_candles(
        &self,
        product_type: BitgetProductType,
        symbol: &str,
        granularity: &str,
        kline_type: Option<BitgetKLineType>,
        params: BitgetCandleParams,
    ) -> Result<Vec<BitgetCandle>, BoxError> {
        self.market_api
            .get_candles(product_type, symbol, granularity, kline_type, params)
            .await
    }

    async fn get_history_candles(
        &self,
        product_type: BitgetProductType,
        symbol: &str,
        granularity: &str,
        params: BitgetCandleParams,
    ) -> Result<Vec<BitgetCandle>, BoxError> {
        self.market_api
            .get_history_candles(product_type, symbol, granularity, params)
            .await
    }

    async fn get_history_index_candles(
        &self,
        product_type: BitgetProductType,
        symbol: &str,
        granularity: &str,
        params: BitgetCandleParams,
    ) -> Result<Vec<BitgetCandle>, BoxError> {
        self.market_api
            .get_history_index_candles(product_type, symbol, granularity, params)
            .await
    }

    async fn get_history_mark_candles(
        &self,
        product_type: BitgetProductType,
        symbol: &str,
        granularity: &str,
        params: BitgetCandleParams,
    ) -> Result<Vec<BitgetCandle>, BoxError> {
        self.market_api
            .get_history_mark_candles(product_type, symbol, granularity, params)
            .await
    }

    async fn get_open_interest(
        &self,
        product_type: BitgetProductType,
        symbol: &str,
    ) -> Result<BitgetOpenInterest, BoxError> {
        self.market_api
            .get_open_interest(product_type, symbol)
            .await
    }

    async fn get_current_funding_rates(
        &self,
        product_type: BitgetProductType,
        symbol: Option<&str>,
    ) -> Result<Vec<BitgetCurrentFundingRate>, BoxError> {
        self.market_api
            .get_current_funding_rates(product_type, symbol)
            .await
    }

    async fn get_historical_funding_rates(
        &self,
        product_type: BitgetProductType,
        symbol: &str,
        page_size: u32,
        page_no: u32,
    ) -> Result<Vec<BitgetHistoricalFundingRate>, BoxError> {
        self.market_api
            .get_historical_funding_rates(product_type, symbol, page_size, page_no)
            .await
    }

    async fn get_next_funding_time(
        &self,
        product_type: BitgetProductType,
        symbol: &str,
    ) -> Result<Vec<BitgetNextFundingTime>, BoxError> {
        self.market_api
            .get_next_funding_time(product_type, symbol)
            .await
    }

    async fn get_symbol_price(
        &self,
        product_type: BitgetProductType,
        symbol: &str,
    ) -> Result<Vec<BitgetSymbolPrice>, BoxError> {
        self.market_api.get_symbol_price(product_type, symbol).await
    }

    async fn get_oi_limits(
        &self,
        product_type: BitgetProductType,
        symbol: Option<&str>,
    ) -> Result<Vec<BitgetOiLimit>, BoxError> {
        self.market_api.get_oi_limits(product_type, symbol).await
    }

    async fn get_spot_symbols(
        &self,
        symbol: Option<&str>,
    ) -> Result<Vec<BitgetSpotSymbol>, BoxError> {
        self.spot_api.get_symbols(symbol).await
    }

    async fn get_spot_tickers(
        &self,
        symbol: Option<&str>,
    ) -> Result<Vec<BitgetSpotTicker>, BoxError> {
        self.spot_api.get_tickers(symbol).await
    }

    async fn get_spot_orderbook(
        &self,
        symbol: &str,
        step: Option<&str>,
        limit: Option<u32>,
    ) -> Result<BitgetSpotOrderBook, BoxError> {
        self.spot_api.get_orderbook(symbol, step, limit).await
    }

    async fn get_spot_merge_depth(
        &self,
        symbol: &str,
        precision: Option<&str>,
        limit: Option<&str>,
    ) -> Result<BitgetMergeDepth, BoxError> {
        self.spot_api
            .get_merge_depth(symbol, precision, limit)
            .await
    }

    async fn get_spot_recent_trades(
        &self,
        symbol: &str,
        limit: Option<u32>,
    ) -> Result<Vec<BitgetMarketTrade>, BoxError> {
        self.spot_api.get_recent_trades(symbol, limit).await
    }

    async fn get_spot_trades_history(
        &self,
        symbol: &str,
        params: &BitgetFillsHistoryParams,
    ) -> Result<Vec<BitgetMarketTrade>, BoxError> {
        self.spot_api.get_trades_history(symbol, params).await
    }

    async fn get_spot_candles(
        &self,
        symbol: &str,
        granularity: &str,
        start_time: Option<i64>,
        end_time: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<BitgetSpotCandle>, BoxError> {
        self.spot_api
            .get_candles(symbol, granularity, start_time, end_time, limit)
            .await
    }

    async fn get_spot_history_candles(
        &self,
        symbol: &str,
        granularity: &str,
        end_time: i64,
        limit: Option<u32>,
    ) -> Result<Vec<BitgetSpotCandle>, BoxError> {
        self.spot_api
            .get_history_candles(symbol, granularity, end_time, limit)
            .await
    }

    async fn get_spot_auction(&self, symbol: &str) -> Result<BitgetSpotAuction, BoxError> {
        self.spot_api.get_auction(symbol).await
    }
}
