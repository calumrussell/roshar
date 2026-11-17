/// Wire and message types for this exchange (websocket messages, REST payloads).
pub mod types;

// Endpoints. OKX retired WebSocket port 8443 on 2026-10-31 (public internet); port 443 is the
// default for wss://. Market-data channels live on /ws/v5/public, except candlestick channels
// and `trades-all`, which live on /ws/v5/business.
pub const OKX_WSS_URL: &str = "wss://ws.okx.com/ws/v5/public";
pub const OKX_BUSINESS_WSS_URL: &str = "wss://ws.okx.com/ws/v5/business";
pub const OKX_PRIVATE_WSS_URL: &str = "wss://ws.okx.com/ws/v5/private";

pub(crate) mod rest;

use rest::MarketApi;

pub use rest::MarketApi as OkxMarketApi;
pub use rest::{BoxError, OkxInstrumentsResponse, OkxResponse, OkxTickerData, OkxTickersResponse};

use crate::types::*;
use anyhow::Result;
use async_trait::async_trait;
use roshar_ws_mgr::{Config, Manager, Message};
use std::collections::HashMap;
use std::sync::Arc;

pub struct OkxClient {
    market_api: MarketApi,
    ws_config: Config,
    business_ws_config: Config,
}

impl Default for OkxClient {
    fn default() -> Self {
        Self::new()
    }
}

fn ws_config(name: &str, url: &str) -> Config {
    Config {
        name: name.to_string(),
        url: url.to_string(),
        ping_duration: 20,
        ping_message: OkxWssMessage::ping(),
        ping_timeout: 10,
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
    }
}

impl OkxClient {
    /// OKX limits each endpoint per IP: mostly 20 requests / 2 s, some 10 / 2 s (funding-rate,
    /// mark-price, time, insurance-fund, position-tiers, books-full, ...), instrument-tick-bands
    /// 5 / 2 s and platform-24-volume 2 / 2 s. One shared 10/s bucket fits the 20 / 2 s
    /// endpoints; use `new_with_rate_limit` for a lower rate when bursting the slower ones.
    const DEFAULT_REQUESTS_PER_SECOND: u32 = 10;
    const WS_CONN_NAME: &str = "okx";
    const BUSINESS_WS_CONN_NAME: &str = "okx-business";

    pub fn new() -> Self {
        Self::new_with_rate_limit(Self::DEFAULT_REQUESTS_PER_SECOND)
    }

    pub fn new_with_rate_limit(requests_per_second: u32) -> Self {
        Self {
            market_api: MarketApi::new(requests_per_second),
            ws_config: ws_config(Self::WS_CONN_NAME, OKX_WSS_URL),
            business_ws_config: ws_config(Self::BUSINESS_WS_CONN_NAME, OKX_BUSINESS_WSS_URL),
        }
    }

    /// Connection config for `/ws/v5/public`.
    pub fn ws_config(&self) -> roshar_ws_mgr::Config {
        self.ws_config.clone()
    }

    pub fn ws_config_mut(&mut self) -> &mut roshar_ws_mgr::Config {
        &mut self.ws_config
    }

    /// Connection config for `/ws/v5/business` (candles, mark/index candles, `trades-all`).
    pub fn business_ws_config(&self) -> roshar_ws_mgr::Config {
        self.business_ws_config.clone()
    }

    pub fn business_ws_config_mut(&mut self) -> &mut roshar_ws_mgr::Config {
        &mut self.business_ws_config
    }

    /// Subscribe to channels built with [`OkxWssArg`]'s constructors. Each arg is sent to the
    /// public or business connection per [`OkxWssArg::is_business`]; the matching connection
    /// must already be added to the manager.
    pub fn subscribe(&self, manager: &Arc<Manager>, args: &[OkxWssArg]) -> Result<()> {
        self.send("subscribe", manager, args)
    }

    pub fn unsubscribe(&self, manager: &Arc<Manager>, args: &[OkxWssArg]) -> Result<()> {
        self.send("unsubscribe", manager, args)
    }

    fn send(&self, op: &str, manager: &Arc<Manager>, args: &[OkxWssArg]) -> Result<()> {
        let (business, public): (Vec<OkxWssArg>, Vec<OkxWssArg>) =
            args.iter().cloned().partition(OkxWssArg::is_business);
        for (config, args) in [
            (&self.ws_config, public),
            (&self.business_ws_config, business),
        ] {
            if args.is_empty() {
                continue;
            }
            let msg = OkxWssMessage {
                op: op.to_string(),
                args,
            }
            .to_json();
            manager.write(&config.name, Message::TextMessage(config.name.clone(), msg))?;
        }
        Ok(())
    }
}

/// Public market-data REST endpoints. Optional parameters are omitted from the request when
/// `None`; see each method for OKX's defaults and limits.
#[async_trait]
pub trait OkxApi {
    /// `GET /api/v5/public/instruments`. Map of instId -> instrument (all states; caller filters).
    /// `inst_type`: SPOT, MARGIN, SWAP, FUTURES, OPTION or EVENTS. `inst_family` is required by OKX for OPTION; `series_id` is required for EVENTS.
    async fn get_instruments_info(
        &self,
        inst_type: &str,
        inst_family: Option<&str>,
        inst_id: Option<&str>,
        series_id: Option<&str>,
    ) -> Result<HashMap<String, OkxInstrumentInfo>, BoxError>;

    /// `GET /api/v5/market/tickers`. Map of instId -> ticker.
    /// `inst_type`: SPOT, SWAP, FUTURES, OPTION or EVENTS; `inst_family` applies to FUTURES/SWAP/OPTION.
    async fn get_tickers(
        &self,
        inst_type: &str,
        inst_family: Option<&str>,
    ) -> Result<HashMap<String, OkxTickerData>, BoxError>;

    /// `GET /api/v5/market/ticker`: one instrument.
    async fn get_ticker(&self, inst_id: &str) -> Result<OkxTickerData, BoxError>;

    /// `GET /api/v5/market/books`. `sz`: depth per side (OKX max 400, default 1).
    async fn get_order_book(
        &self,
        inst_id: &str,
        sz: Option<u32>,
    ) -> Result<OkxOrderBook, BoxError>;

    /// `GET /api/v5/market/books-full`. `sz`: depth per side (OKX max 5000, default 1).
    async fn get_order_book_full(
        &self,
        inst_id: &str,
        sz: Option<u32>,
    ) -> Result<OkxOrderBookFull, BoxError>;

    /// `GET /api/v5/market/books-rpi`: book including Retail Price Improvement orders; level index 2 is the size excluding RPI. `sz`: depth per side (OKX max 400, default 1).
    async fn get_order_book_rpi(
        &self,
        inst_id: &str,
        sz: Option<u32>,
    ) -> Result<OkxOrderBook, BoxError>;

    /// `GET /api/v5/market/trades`: most recent trades, newest first. `limit`: OKX max 500, default 100.
    async fn get_trades(
        &self,
        inst_id: &str,
        limit: Option<u32>,
    ) -> Result<Vec<OkxTrade>, BoxError>;

    /// `GET /api/v5/market/history-trades` (last 3 months). `pagination_type`: 1 = by tradeId (OKX default), 2 = by timestamp. `after`/`before` are tradeIds or (after only) ms timestamps per `pagination_type`. `limit`: OKX max/default 100.
    async fn get_history_trades(
        &self,
        inst_id: &str,
        pagination_type: Option<u32>,
        after: Option<&str>,
        before: Option<&str>,
        limit: Option<u32>,
    ) -> Result<Vec<OkxTrade>, BoxError>;

    /// `GET /api/v5/market/candles`: roughly the latest 1,440 bars, newest first. `bar` defaults to 1m at OKX; `after`/`before` are ms timestamps; `limit` max 300 (default 100); `adjust` = "forward" for equity perpetuals.
    async fn get_candles(
        &self,
        inst_id: &str,
        bar: Option<&str>,
        after: Option<i64>,
        before: Option<i64>,
        limit: Option<u32>,
        adjust: Option<&str>,
    ) -> Result<Vec<OkxCandle>, BoxError>;

    /// `GET /api/v5/market/history-candles`: older bars than `/market/candles`. Same parameters.
    async fn get_history_candles(
        &self,
        inst_id: &str,
        bar: Option<&str>,
        after: Option<i64>,
        before: Option<i64>,
        limit: Option<u32>,
        adjust: Option<&str>,
    ) -> Result<Vec<OkxCandle>, BoxError>;

    /// `GET /api/v5/market/mark-price-candles` (recent mark-price bars). `bar` defaults to 1m at OKX; `after`/`before` are ms timestamps; `limit` max/default 100.
    async fn get_mark_price_candles(
        &self,
        inst_id: &str,
        bar: Option<&str>,
        after: Option<i64>,
        before: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<OkxPriceCandle>, BoxError>;

    /// `GET /api/v5/market/history-mark-price-candles`. `bar` defaults to 1m at OKX; `after`/`before` are ms timestamps; `limit` max/default 100.
    async fn get_history_mark_price_candles(
        &self,
        inst_id: &str,
        bar: Option<&str>,
        after: Option<i64>,
        before: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<OkxPriceCandle>, BoxError>;

    /// `GET /api/v5/market/index-candles` (`inst_id` is an index, e.g. "BTC-USD"). `bar` defaults to 1m at OKX; `after`/`before` are ms timestamps; `limit` max/default 100.
    async fn get_index_candles(
        &self,
        inst_id: &str,
        bar: Option<&str>,
        after: Option<i64>,
        before: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<OkxPriceCandle>, BoxError>;

    /// `GET /api/v5/market/history-index-candles` (`inst_id` is an index). `bar` defaults to 1m at OKX; `after`/`before` are ms timestamps; `limit` max/default 100.
    async fn get_history_index_candles(
        &self,
        inst_id: &str,
        bar: Option<&str>,
        after: Option<i64>,
        before: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<OkxPriceCandle>, BoxError>;

    /// `GET /api/v5/market/index-tickers`. OKX requires one of `quote_ccy` or `inst_id`.
    async fn get_index_tickers(
        &self,
        quote_ccy: Option<&str>,
        inst_id: Option<&str>,
    ) -> Result<Vec<OkxIndexTicker>, BoxError>;

    /// `GET /api/v5/market/index-components` (e.g. index "BTC-USDT").
    async fn get_index_components(&self, index: &str) -> Result<OkxIndexComponents, BoxError>;

    /// `GET /api/v5/market/exchange-rate` (USD/CNY, averaged over two weeks).
    async fn get_exchange_rate(&self) -> Result<OkxExchangeRate, BoxError>;

    /// `GET /api/v5/market/platform-24-volume`.
    async fn get_platform_24_volume(&self) -> Result<OkxPlatformVolume, BoxError>;

    /// `GET /api/v5/market/call-auction-details`.
    async fn get_call_auction_details(
        &self,
        inst_id: &str,
    ) -> Result<OkxCallAuctionDetails, BoxError>;

    /// `GET /api/v5/market/option/instrument-family-trades`: latest option trades grouped by call/put.
    async fn get_option_family_trades(
        &self,
        inst_family: &str,
    ) -> Result<Vec<OkxOptionFamilyTrades>, BoxError>;

    /// `GET /api/v5/public/funding-rate`. `inst_id` may be "ANY" for every perpetual.
    async fn get_funding_rate(&self, inst_id: &str) -> Result<Vec<OkxFundingRate>, BoxError>;

    /// `GET /api/v5/public/funding-rate-history`. `before`/`after` are fundingTime ms; `limit` max/default 400.
    async fn get_funding_rate_history(
        &self,
        inst_id: &str,
        before: Option<i64>,
        after: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<OkxFundingRateHistory>, BoxError>;

    /// `GET /api/v5/public/open-interest`. `inst_type`: SWAP, FUTURES, OPTION or EVENTS; `inst_family` required for OPTION.
    async fn get_open_interest(
        &self,
        inst_type: &str,
        inst_family: Option<&str>,
        inst_id: Option<&str>,
    ) -> Result<Vec<OkxOpenInterest>, BoxError>;

    /// `GET /api/v5/public/price-limit`.
    async fn get_price_limit(&self, inst_id: &str) -> Result<OkxPriceLimit, BoxError>;

    /// `GET /api/v5/public/mark-price`. `inst_type`: MARGIN, SWAP, FUTURES, OPTION or EVENTS.
    async fn get_mark_price(
        &self,
        inst_type: &str,
        inst_family: Option<&str>,
        inst_id: Option<&str>,
    ) -> Result<Vec<OkxMarkPrice>, BoxError>;

    /// `GET /api/v5/public/estimated-price` (FUTURES/OPTION/EVENTS; published one hour before delivery/exercise).
    async fn get_estimated_price(&self, inst_id: &str) -> Result<OkxEstimatedPrice, BoxError>;

    /// `GET /api/v5/public/estimated-settlement-info` (FUTURES; data only in the 30 minutes before settlement, OKX error 51001 otherwise).
    async fn get_estimated_settlement_info(
        &self,
        inst_id: &str,
    ) -> Result<OkxEstimatedSettlement, BoxError>;

    /// `GET /api/v5/public/delivery-exercise-history`. `inst_type`: FUTURES or OPTION; `after`/`before` ms; `limit` max/default 100.
    async fn get_delivery_exercise_history(
        &self,
        inst_type: &str,
        inst_family: &str,
        after: Option<i64>,
        before: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<OkxDeliveryExercise>, BoxError>;

    /// `GET /api/v5/public/settlement-history` (futures settlements, last 3 months).
    async fn get_settlement_history(
        &self,
        inst_family: &str,
        after: Option<i64>,
        before: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<OkxSettlement>, BoxError>;

    /// `GET /api/v5/public/insurance-fund`. `kind` is OKX `type`; `inst_family` required for FUTURES/SWAP/OPTION, `ccy` for MARGIN.
    async fn get_insurance_fund(
        &self,
        inst_type: &str,
        kind: Option<&str>,
        inst_family: Option<&str>,
        ccy: Option<&str>,
        before: Option<i64>,
        after: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<OkxInsuranceFund>, BoxError>;

    /// `GET /api/v5/public/premium-history` (SWAP; last 6 months).
    async fn get_premium_history(
        &self,
        inst_id: &str,
        after: Option<i64>,
        before: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<OkxPremium>, BoxError>;

    /// `GET /api/v5/public/opt-summary`. `exp_time` is "YYMMDD".
    async fn get_opt_summary(
        &self,
        inst_family: &str,
        exp_time: Option<&str>,
    ) -> Result<Vec<OkxOptionSummary>, BoxError>;

    /// `GET /api/v5/public/option-trades`. OKX requires `inst_id` or `inst_family`; `opt_type` is "C" or "P".
    async fn get_option_trades(
        &self,
        inst_id: Option<&str>,
        inst_family: Option<&str>,
        opt_type: Option<&str>,
    ) -> Result<Vec<OkxOptionTrade>, BoxError>;

    /// `GET /api/v5/public/underlying`. `inst_type`: SWAP, FUTURES or OPTION.
    async fn get_underlying(&self, inst_type: &str) -> Result<Vec<String>, BoxError>;

    /// `GET /api/v5/public/position-tiers`. `td_mode`: cross or isolated; see OKX docs for which of `inst_family`/`inst_id`/`ccy` each `inst_type` needs.
    async fn get_position_tiers(
        &self,
        inst_type: &str,
        td_mode: &str,
        inst_family: Option<&str>,
        inst_id: Option<&str>,
        ccy: Option<&str>,
        tier: Option<&str>,
    ) -> Result<Vec<OkxPositionTier>, BoxError>;

    /// `GET /api/v5/public/instrument-tick-bands`. `inst_type`: OPTION or EVENTS.
    async fn get_instrument_tick_bands(
        &self,
        inst_type: &str,
        inst_family: Option<&str>,
    ) -> Result<Vec<OkxInstrumentTickBands>, BoxError>;

    /// `GET /api/v5/public/event-contract/series` (all series when `series_id` is None).
    async fn get_event_series(
        &self,
        series_id: Option<&str>,
    ) -> Result<Vec<OkxEventSeries>, BoxError>;

    /// `GET /api/v5/public/event-contract/events`. `state`: preopen, live, settling or expired; `before`/`after` are expTime ms; `limit` max/default 100.
    async fn get_events(
        &self,
        series_id: &str,
        event_id: Option<&str>,
        state: Option<&str>,
        limit: Option<u32>,
        before: Option<i64>,
        after: Option<i64>,
    ) -> Result<Vec<OkxEvent>, BoxError>;

    /// `GET /api/v5/public/event-contract/markets`. Same filters as `get_events` plus `inst_id`.
    async fn get_event_markets(
        &self,
        series_id: &str,
        event_id: Option<&str>,
        inst_id: Option<&str>,
        state: Option<&str>,
        limit: Option<u32>,
        before: Option<i64>,
        after: Option<i64>,
    ) -> Result<Vec<OkxEventMarket>, BoxError>;

    /// `GET /api/v5/public/time`.
    async fn get_system_time(&self) -> Result<OkxSystemTime, BoxError>;

    /// `GET /api/v5/system/status`. `state`: scheduled, ongoing, pre_open, completed or canceled (OKX default: scheduled + ongoing + pre_open).
    async fn get_system_status(
        &self,
        state: Option<&str>,
    ) -> Result<Vec<OkxSystemStatus>, BoxError>;
}

#[async_trait]
impl OkxApi for OkxClient {
    async fn get_instruments_info(
        &self,
        inst_type: &str,
        inst_family: Option<&str>,
        inst_id: Option<&str>,
        series_id: Option<&str>,
    ) -> Result<HashMap<String, OkxInstrumentInfo>, BoxError> {
        self.market_api
            .get_instruments_info(inst_type, inst_family, inst_id, series_id)
            .await
    }

    async fn get_tickers(
        &self,
        inst_type: &str,
        inst_family: Option<&str>,
    ) -> Result<HashMap<String, OkxTickerData>, BoxError> {
        self.market_api.get_tickers(inst_type, inst_family).await
    }

    async fn get_ticker(&self, inst_id: &str) -> Result<OkxTickerData, BoxError> {
        self.market_api.get_ticker(inst_id).await
    }

    async fn get_order_book(
        &self,
        inst_id: &str,
        sz: Option<u32>,
    ) -> Result<OkxOrderBook, BoxError> {
        self.market_api.get_order_book(inst_id, sz).await
    }

    async fn get_order_book_full(
        &self,
        inst_id: &str,
        sz: Option<u32>,
    ) -> Result<OkxOrderBookFull, BoxError> {
        self.market_api.get_order_book_full(inst_id, sz).await
    }

    async fn get_order_book_rpi(
        &self,
        inst_id: &str,
        sz: Option<u32>,
    ) -> Result<OkxOrderBook, BoxError> {
        self.market_api.get_order_book_rpi(inst_id, sz).await
    }

    async fn get_trades(
        &self,
        inst_id: &str,
        limit: Option<u32>,
    ) -> Result<Vec<OkxTrade>, BoxError> {
        self.market_api.get_trades(inst_id, limit).await
    }

    async fn get_history_trades(
        &self,
        inst_id: &str,
        pagination_type: Option<u32>,
        after: Option<&str>,
        before: Option<&str>,
        limit: Option<u32>,
    ) -> Result<Vec<OkxTrade>, BoxError> {
        self.market_api
            .get_history_trades(inst_id, pagination_type, after, before, limit)
            .await
    }

    async fn get_candles(
        &self,
        inst_id: &str,
        bar: Option<&str>,
        after: Option<i64>,
        before: Option<i64>,
        limit: Option<u32>,
        adjust: Option<&str>,
    ) -> Result<Vec<OkxCandle>, BoxError> {
        self.market_api
            .get_candles(inst_id, bar, after, before, limit, adjust)
            .await
    }

    async fn get_history_candles(
        &self,
        inst_id: &str,
        bar: Option<&str>,
        after: Option<i64>,
        before: Option<i64>,
        limit: Option<u32>,
        adjust: Option<&str>,
    ) -> Result<Vec<OkxCandle>, BoxError> {
        self.market_api
            .get_history_candles(inst_id, bar, after, before, limit, adjust)
            .await
    }

    async fn get_mark_price_candles(
        &self,
        inst_id: &str,
        bar: Option<&str>,
        after: Option<i64>,
        before: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<OkxPriceCandle>, BoxError> {
        self.market_api
            .get_mark_price_candles(inst_id, bar, after, before, limit)
            .await
    }

    async fn get_history_mark_price_candles(
        &self,
        inst_id: &str,
        bar: Option<&str>,
        after: Option<i64>,
        before: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<OkxPriceCandle>, BoxError> {
        self.market_api
            .get_history_mark_price_candles(inst_id, bar, after, before, limit)
            .await
    }

    async fn get_index_candles(
        &self,
        inst_id: &str,
        bar: Option<&str>,
        after: Option<i64>,
        before: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<OkxPriceCandle>, BoxError> {
        self.market_api
            .get_index_candles(inst_id, bar, after, before, limit)
            .await
    }

    async fn get_history_index_candles(
        &self,
        inst_id: &str,
        bar: Option<&str>,
        after: Option<i64>,
        before: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<OkxPriceCandle>, BoxError> {
        self.market_api
            .get_history_index_candles(inst_id, bar, after, before, limit)
            .await
    }

    async fn get_index_tickers(
        &self,
        quote_ccy: Option<&str>,
        inst_id: Option<&str>,
    ) -> Result<Vec<OkxIndexTicker>, BoxError> {
        self.market_api.get_index_tickers(quote_ccy, inst_id).await
    }

    async fn get_index_components(&self, index: &str) -> Result<OkxIndexComponents, BoxError> {
        self.market_api.get_index_components(index).await
    }

    async fn get_exchange_rate(&self) -> Result<OkxExchangeRate, BoxError> {
        self.market_api.get_exchange_rate().await
    }

    async fn get_platform_24_volume(&self) -> Result<OkxPlatformVolume, BoxError> {
        self.market_api.get_platform_24_volume().await
    }

    async fn get_call_auction_details(
        &self,
        inst_id: &str,
    ) -> Result<OkxCallAuctionDetails, BoxError> {
        self.market_api.get_call_auction_details(inst_id).await
    }

    async fn get_option_family_trades(
        &self,
        inst_family: &str,
    ) -> Result<Vec<OkxOptionFamilyTrades>, BoxError> {
        self.market_api.get_option_family_trades(inst_family).await
    }

    async fn get_funding_rate(&self, inst_id: &str) -> Result<Vec<OkxFundingRate>, BoxError> {
        self.market_api.get_funding_rate(inst_id).await
    }

    async fn get_funding_rate_history(
        &self,
        inst_id: &str,
        before: Option<i64>,
        after: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<OkxFundingRateHistory>, BoxError> {
        self.market_api
            .get_funding_rate_history(inst_id, before, after, limit)
            .await
    }

    async fn get_open_interest(
        &self,
        inst_type: &str,
        inst_family: Option<&str>,
        inst_id: Option<&str>,
    ) -> Result<Vec<OkxOpenInterest>, BoxError> {
        self.market_api
            .get_open_interest(inst_type, inst_family, inst_id)
            .await
    }

    async fn get_price_limit(&self, inst_id: &str) -> Result<OkxPriceLimit, BoxError> {
        self.market_api.get_price_limit(inst_id).await
    }

    async fn get_mark_price(
        &self,
        inst_type: &str,
        inst_family: Option<&str>,
        inst_id: Option<&str>,
    ) -> Result<Vec<OkxMarkPrice>, BoxError> {
        self.market_api
            .get_mark_price(inst_type, inst_family, inst_id)
            .await
    }

    async fn get_estimated_price(&self, inst_id: &str) -> Result<OkxEstimatedPrice, BoxError> {
        self.market_api.get_estimated_price(inst_id).await
    }

    async fn get_estimated_settlement_info(
        &self,
        inst_id: &str,
    ) -> Result<OkxEstimatedSettlement, BoxError> {
        self.market_api.get_estimated_settlement_info(inst_id).await
    }

    async fn get_delivery_exercise_history(
        &self,
        inst_type: &str,
        inst_family: &str,
        after: Option<i64>,
        before: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<OkxDeliveryExercise>, BoxError> {
        self.market_api
            .get_delivery_exercise_history(inst_type, inst_family, after, before, limit)
            .await
    }

    async fn get_settlement_history(
        &self,
        inst_family: &str,
        after: Option<i64>,
        before: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<OkxSettlement>, BoxError> {
        self.market_api
            .get_settlement_history(inst_family, after, before, limit)
            .await
    }

    async fn get_insurance_fund(
        &self,
        inst_type: &str,
        kind: Option<&str>,
        inst_family: Option<&str>,
        ccy: Option<&str>,
        before: Option<i64>,
        after: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<OkxInsuranceFund>, BoxError> {
        self.market_api
            .get_insurance_fund(inst_type, kind, inst_family, ccy, before, after, limit)
            .await
    }

    async fn get_premium_history(
        &self,
        inst_id: &str,
        after: Option<i64>,
        before: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<OkxPremium>, BoxError> {
        self.market_api
            .get_premium_history(inst_id, after, before, limit)
            .await
    }

    async fn get_opt_summary(
        &self,
        inst_family: &str,
        exp_time: Option<&str>,
    ) -> Result<Vec<OkxOptionSummary>, BoxError> {
        self.market_api.get_opt_summary(inst_family, exp_time).await
    }

    async fn get_option_trades(
        &self,
        inst_id: Option<&str>,
        inst_family: Option<&str>,
        opt_type: Option<&str>,
    ) -> Result<Vec<OkxOptionTrade>, BoxError> {
        self.market_api
            .get_option_trades(inst_id, inst_family, opt_type)
            .await
    }

    async fn get_underlying(&self, inst_type: &str) -> Result<Vec<String>, BoxError> {
        self.market_api.get_underlying(inst_type).await
    }

    async fn get_position_tiers(
        &self,
        inst_type: &str,
        td_mode: &str,
        inst_family: Option<&str>,
        inst_id: Option<&str>,
        ccy: Option<&str>,
        tier: Option<&str>,
    ) -> Result<Vec<OkxPositionTier>, BoxError> {
        self.market_api
            .get_position_tiers(inst_type, td_mode, inst_family, inst_id, ccy, tier)
            .await
    }

    async fn get_instrument_tick_bands(
        &self,
        inst_type: &str,
        inst_family: Option<&str>,
    ) -> Result<Vec<OkxInstrumentTickBands>, BoxError> {
        self.market_api
            .get_instrument_tick_bands(inst_type, inst_family)
            .await
    }

    async fn get_event_series(
        &self,
        series_id: Option<&str>,
    ) -> Result<Vec<OkxEventSeries>, BoxError> {
        self.market_api.get_event_series(series_id).await
    }

    async fn get_events(
        &self,
        series_id: &str,
        event_id: Option<&str>,
        state: Option<&str>,
        limit: Option<u32>,
        before: Option<i64>,
        after: Option<i64>,
    ) -> Result<Vec<OkxEvent>, BoxError> {
        self.market_api
            .get_events(series_id, event_id, state, limit, before, after)
            .await
    }

    async fn get_event_markets(
        &self,
        series_id: &str,
        event_id: Option<&str>,
        inst_id: Option<&str>,
        state: Option<&str>,
        limit: Option<u32>,
        before: Option<i64>,
        after: Option<i64>,
    ) -> Result<Vec<OkxEventMarket>, BoxError> {
        self.market_api
            .get_event_markets(series_id, event_id, inst_id, state, limit, before, after)
            .await
    }

    async fn get_system_time(&self) -> Result<OkxSystemTime, BoxError> {
        self.market_api.get_system_time().await
    }

    async fn get_system_status(
        &self,
        state: Option<&str>,
    ) -> Result<Vec<OkxSystemStatus>, BoxError> {
        self.market_api.get_system_status(state).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_okx_client_instruments() {
        let client = OkxClient::new();
        let instruments = client
            .get_instruments_info("SWAP", None, None, None)
            .await
            .expect("instruments");
        assert!(
            !instruments.is_empty(),
            "Expected some instruments, got none"
        );
    }

    #[test]
    fn business_and_public_configs() {
        let client = OkxClient::new();
        assert_eq!(client.ws_config().url, OKX_WSS_URL);
        assert_eq!(client.business_ws_config().url, OKX_BUSINESS_WSS_URL);
        assert_ne!(client.ws_config().name, client.business_ws_config().name);
    }
}
