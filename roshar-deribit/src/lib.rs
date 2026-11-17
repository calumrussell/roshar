/// Wire and message types for this exchange (websocket messages, REST payloads).
pub mod types;

/// JSON-RPC method calls and channel names, shared by REST and the websocket.
pub mod request;
pub(crate) mod rest;

pub use request::{DeribitChannel, DeribitRequest, DeribitSettlementFilter, DeribitTradeFilter};
pub use rest::MarketApi as DeribitMarketApi;
pub use rest::{DERIBIT_REST_URL, DERIBIT_TESTNET_REST_URL};

// Endpoints
pub const DERIBIT_WSS_URL: &str = "wss://www.deribit.com/ws/api/v2";
pub const DERIBIT_TESTNET_WSS_URL: &str = "wss://test.deribit.com/ws/api/v2";

/// JSON-RPC id used by the keepalive `public/test` in the default `ws_config`.
/// Pick other ids for your own requests so replies can be told apart.
pub const DERIBIT_KEEPALIVE_ID: u64 = 1;

use crate::rest::MarketApi;
use crate::types::{
    DeribitAprHistory, DeribitBasketIndexComposition, DeribitBlockRfqTradesPage,
    DeribitBookSummary, DeribitCombo, DeribitContractSize, DeribitCurrency, DeribitDeliveryPrices,
    DeribitExpirations, DeribitFundingChartData, DeribitFundingRateHistory, DeribitIndexPrice,
    DeribitIndexPriceName, DeribitInstrument, DeribitOrderBookSnapshot, DeribitSettlementsPage,
    DeribitStatus, DeribitTickerData, DeribitTimeValue, DeribitTradeVolume, DeribitTradesPage,
    DeribitTradingViewChartData, DeribitVersion, DeribitVolatilityIndexData, DeribitWssMessage,
};
use anyhow::Result;
use async_trait::async_trait;
use roshar_ws_mgr::{Config, Manager, Message};
use std::sync::Arc;

type RestResult<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

pub struct DeribitClient {
    market_api: MarketApi,
    ws_config: Config,
}

impl DeribitClient {
    /// One shared REST bucket. `public/get_instruments` allows 1 request/s sustained (burst 50);
    /// every other public method 20/s (burst 100), per IP when unauthenticated.
    /// https://docs.deribit.com/articles/rate-limits
    const DEFAULT_RATE_LIMIT: (u32, u64) = (1, 1);
    const WS_CONN_NAME: &str = "deribit";

    pub fn new() -> Self {
        let (refill, interval_secs) = Self::DEFAULT_RATE_LIMIT;
        Self::new_with_rate_limit(refill, interval_secs)
    }

    /// `refill` REST requests every `interval_secs` seconds.
    pub fn new_with_rate_limit(refill: u32, interval_secs: u64) -> Self {
        Self {
            market_api: MarketApi::new(refill, interval_secs),
            ws_config: Config {
                name: Self::WS_CONN_NAME.to_string(),
                url: DERIBIT_WSS_URL.to_string(),
                ping_duration: 30,
                // A JSON text message: Deribit counts it as client activity for
                // `public/set_heartbeat`; websocket ping frames do not count.
                ping_message: DeribitWssMessage::new(
                    DeribitRequest::test(None),
                    DERIBIT_KEEPALIVE_ID,
                )
                .to_json(),
                ping_timeout: 10,
                reconnect_timeout: 90,
                use_text_ping: Some(true),
                read_buffer_size: Some(33554432),
                write_buffer_size: Some(2097152),
                max_message_size: Some(41943040),
                max_frame_size: Some(20971520),
                tcp_recv_buffer_size: Some(16777216),
                tcp_send_buffer_size: Some(4194304),
                tcp_nodelay: Some(true),
                broadcast_channel_size: Some(131072),
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

    /// Sends any JSON-RPC request over the websocket. The reply arrives as
    /// `DeribitWsIncoming::Response` with the same `id`; parse it with `result_as::<T>()`
    /// using the type the matching `DeribitMarketApi` method returns.
    pub fn send_request(
        &self,
        manager: &Arc<Manager>,
        request: DeribitRequest,
        id: u64,
    ) -> Result<()> {
        manager.write(
            &self.ws_config.name,
            Message::TextMessage(
                self.ws_config.name.clone(),
                DeribitWssMessage::new(request, id).to_json(),
            ),
        )?;
        Ok(())
    }

    fn channel_names(channels: &[DeribitChannel<'_>]) -> Vec<String> {
        channels.iter().map(DeribitChannel::name).collect()
    }

    /// `public/subscribe`. Costs 3,000 credits per call (about 3.3 calls/s sustained,
    /// burst 10), so put many channels in one call. The reply lists the subscribed channels.
    pub fn subscribe(
        &self,
        manager: &Arc<Manager>,
        channels: &[DeribitChannel<'_>],
        id: u64,
    ) -> Result<()> {
        let request = DeribitRequest::subscribe(Self::channel_names(channels));
        self.send_request(manager, request, id)
    }

    /// `public/unsubscribe`. The reply lists the channels that were removed.
    pub fn unsubscribe(
        &self,
        manager: &Arc<Manager>,
        channels: &[DeribitChannel<'_>],
        id: u64,
    ) -> Result<()> {
        let request = DeribitRequest::unsubscribe(Self::channel_names(channels));
        self.send_request(manager, request, id)
    }

    /// `public/unsubscribe_all` (reply `"ok"`).
    pub fn unsubscribe_all(&self, manager: &Arc<Manager>, id: u64) -> Result<()> {
        self.send_request(manager, DeribitRequest::unsubscribe_all(), id)
    }

    /// `public/set_heartbeat`. `interval` is 10..=300 seconds. The server then
    /// sends `heartbeat` notifications; answer each `test_request` with `send_test`, or the
    /// server closes the socket (code 4000) after `2 * interval + 0.5` s without a client
    /// message.
    pub fn set_heartbeat(&self, manager: &Arc<Manager>, interval: u32, id: u64) -> Result<()> {
        self.send_request(manager, DeribitRequest::set_heartbeat(interval), id)
    }

    /// `public/disable_heartbeat` (reply `"ok"`).
    pub fn disable_heartbeat(&self, manager: &Arc<Manager>, id: u64) -> Result<()> {
        self.send_request(manager, DeribitRequest::disable_heartbeat(), id)
    }

    /// `public/test`: the reply to a heartbeat `test_request`.
    pub fn send_test(&self, manager: &Arc<Manager>, id: u64) -> Result<()> {
        self.send_request(manager, DeribitRequest::test(None), id)
    }

    /// `public/hello` (websocket only); the reply is a `DeribitVersion`.
    pub fn hello(
        &self,
        manager: &Arc<Manager>,
        client_name: &str,
        client_version: &str,
        id: u64,
    ) -> Result<()> {
        self.send_request(
            manager,
            DeribitRequest::hello(client_name, client_version),
            id,
        )
    }
}

impl Default for DeribitClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Public REST market data. Parameters mirror the JSON-RPC methods of the same name
/// (see `DeribitRequest` for accepted values); `None` omits a parameter.
#[async_trait]
pub trait DeribitApi {
    async fn get_instruments(
        &self,
        currency: &str,
        kind: Option<&str>,
        expired: Option<bool>,
        extended: Option<bool>,
    ) -> RestResult<Vec<DeribitInstrument>>;

    async fn get_instrument(&self, instrument_name: &str) -> RestResult<DeribitInstrument>;

    async fn get_currencies(&self) -> RestResult<Vec<DeribitCurrency>>;

    async fn get_book_summary_by_currency(
        &self,
        currency: &str,
        kind: Option<&str>,
    ) -> RestResult<Vec<DeribitBookSummary>>;

    async fn get_book_summary_by_instrument(
        &self,
        instrument_name: &str,
    ) -> RestResult<Vec<DeribitBookSummary>>;

    async fn get_order_book(
        &self,
        instrument_name: &str,
        depth: Option<u32>,
    ) -> RestResult<DeribitOrderBookSnapshot>;

    async fn get_order_book_by_instrument_id(
        &self,
        instrument_id: u64,
        depth: Option<u32>,
    ) -> RestResult<DeribitOrderBookSnapshot>;

    async fn ticker(&self, instrument_name: &str) -> RestResult<DeribitTickerData>;

    async fn get_last_trades_by_instrument(
        &self,
        instrument_name: &str,
        start_seq: Option<u64>,
        end_seq: Option<u64>,
        filter: DeribitTradeFilter<'_>,
    ) -> RestResult<DeribitTradesPage>;

    async fn get_last_trades_by_instrument_and_time(
        &self,
        instrument_name: &str,
        filter: DeribitTradeFilter<'_>,
    ) -> RestResult<DeribitTradesPage>;

    async fn get_last_trades_by_currency(
        &self,
        currency: &str,
        kind: Option<&str>,
        start_id: Option<&str>,
        end_id: Option<&str>,
        filter: DeribitTradeFilter<'_>,
    ) -> RestResult<DeribitTradesPage>;

    async fn get_last_trades_by_currency_and_time(
        &self,
        currency: &str,
        kind: Option<&str>,
        filter: DeribitTradeFilter<'_>,
    ) -> RestResult<DeribitTradesPage>;

    async fn get_tradingview_chart_data(
        &self,
        instrument_name: &str,
        start_timestamp: u64,
        end_timestamp: u64,
        resolution: &str,
    ) -> RestResult<DeribitTradingViewChartData>;

    async fn get_funding_rate_history(
        &self,
        instrument_name: &str,
        start_timestamp: u64,
        end_timestamp: u64,
    ) -> RestResult<Vec<DeribitFundingRateHistory>>;

    async fn get_funding_rate_value(
        &self,
        instrument_name: &str,
        start_timestamp: u64,
        end_timestamp: u64,
    ) -> RestResult<f64>;

    async fn get_funding_chart_data(
        &self,
        instrument_name: &str,
        length: &str,
    ) -> RestResult<DeribitFundingChartData>;

    async fn get_historical_volatility(&self, currency: &str) -> RestResult<Vec<DeribitTimeValue>>;

    async fn get_volatility_index_data(
        &self,
        currency: &str,
        start_timestamp: u64,
        end_timestamp: u64,
        resolution: &str,
    ) -> RestResult<DeribitVolatilityIndexData>;

    async fn get_index_price(&self, index_name: &str) -> RestResult<DeribitIndexPrice>;

    async fn get_index_price_names(&self) -> RestResult<Vec<String>>;

    async fn get_index_price_names_extended(&self) -> RestResult<Vec<DeribitIndexPriceName>>;

    async fn get_index_chart_data(
        &self,
        index_name: &str,
        range: &str,
    ) -> RestResult<Vec<DeribitTimeValue>>;

    async fn get_basket_index_composition(
        &self,
        index_name: &str,
    ) -> RestResult<DeribitBasketIndexComposition>;

    async fn get_delivery_prices(
        &self,
        index_name: &str,
        offset: Option<u32>,
        count: Option<u32>,
        start_timestamp: Option<u64>,
        end_timestamp: Option<u64>,
    ) -> RestResult<DeribitDeliveryPrices>;

    async fn get_last_settlements_by_currency(
        &self,
        currency: &str,
        filter: DeribitSettlementFilter<'_>,
    ) -> RestResult<DeribitSettlementsPage>;

    async fn get_last_settlements_by_instrument(
        &self,
        instrument_name: &str,
        filter: DeribitSettlementFilter<'_>,
    ) -> RestResult<DeribitSettlementsPage>;

    async fn get_contract_size(&self, instrument_name: &str) -> RestResult<DeribitContractSize>;

    async fn get_mark_price_history(
        &self,
        instrument_name: &str,
        start_timestamp: u64,
        end_timestamp: u64,
    ) -> RestResult<Vec<DeribitTimeValue>>;

    async fn get_supported_index_names(&self, index_type: Option<&str>) -> RestResult<Vec<String>>;

    async fn get_expirations(
        &self,
        currency: Option<&str>,
        kind: Option<&str>,
        currency_pair: Option<&str>,
    ) -> RestResult<DeribitExpirations>;

    async fn get_apr_history(
        &self,
        currency: &str,
        limit: Option<u32>,
        before: Option<u64>,
    ) -> RestResult<DeribitAprHistory>;

    async fn get_trade_volumes(
        &self,
        extended: Option<bool>,
    ) -> RestResult<Vec<DeribitTradeVolume>>;

    async fn get_combos(&self, currency: &str) -> RestResult<Vec<DeribitCombo>>;

    async fn get_combo_ids(&self, currency: &str, state: Option<&str>) -> RestResult<Vec<String>>;

    async fn get_combo_details(&self, combo_id: &str) -> RestResult<DeribitCombo>;

    async fn get_block_rfq_trades(
        &self,
        currency: &str,
        continuation: Option<&str>,
        count: Option<u32>,
    ) -> RestResult<DeribitBlockRfqTradesPage>;

    async fn status(&self) -> RestResult<DeribitStatus>;

    /// Server time, ms since the UNIX epoch.
    async fn get_time(&self) -> RestResult<u64>;

    async fn test(&self, expected_result: Option<&str>) -> RestResult<DeribitVersion>;
}

#[async_trait]
impl DeribitApi for DeribitClient {
    async fn get_instruments(
        &self,
        currency: &str,
        kind: Option<&str>,
        expired: Option<bool>,
        extended: Option<bool>,
    ) -> RestResult<Vec<DeribitInstrument>> {
        self.market_api
            .get_instruments(currency, kind, expired, extended)
            .await
    }

    async fn get_instrument(&self, instrument_name: &str) -> RestResult<DeribitInstrument> {
        self.market_api.get_instrument(instrument_name).await
    }

    async fn get_currencies(&self) -> RestResult<Vec<DeribitCurrency>> {
        self.market_api.get_currencies().await
    }

    async fn get_book_summary_by_currency(
        &self,
        currency: &str,
        kind: Option<&str>,
    ) -> RestResult<Vec<DeribitBookSummary>> {
        self.market_api
            .get_book_summary_by_currency(currency, kind)
            .await
    }

    async fn get_book_summary_by_instrument(
        &self,
        instrument_name: &str,
    ) -> RestResult<Vec<DeribitBookSummary>> {
        self.market_api
            .get_book_summary_by_instrument(instrument_name)
            .await
    }

    async fn get_order_book(
        &self,
        instrument_name: &str,
        depth: Option<u32>,
    ) -> RestResult<DeribitOrderBookSnapshot> {
        self.market_api.get_order_book(instrument_name, depth).await
    }

    async fn get_order_book_by_instrument_id(
        &self,
        instrument_id: u64,
        depth: Option<u32>,
    ) -> RestResult<DeribitOrderBookSnapshot> {
        self.market_api
            .get_order_book_by_instrument_id(instrument_id, depth)
            .await
    }

    async fn ticker(&self, instrument_name: &str) -> RestResult<DeribitTickerData> {
        self.market_api.ticker(instrument_name).await
    }

    async fn get_last_trades_by_instrument(
        &self,
        instrument_name: &str,
        start_seq: Option<u64>,
        end_seq: Option<u64>,
        filter: DeribitTradeFilter<'_>,
    ) -> RestResult<DeribitTradesPage> {
        self.market_api
            .get_last_trades_by_instrument(instrument_name, start_seq, end_seq, filter)
            .await
    }

    async fn get_last_trades_by_instrument_and_time(
        &self,
        instrument_name: &str,
        filter: DeribitTradeFilter<'_>,
    ) -> RestResult<DeribitTradesPage> {
        self.market_api
            .get_last_trades_by_instrument_and_time(instrument_name, filter)
            .await
    }

    async fn get_last_trades_by_currency(
        &self,
        currency: &str,
        kind: Option<&str>,
        start_id: Option<&str>,
        end_id: Option<&str>,
        filter: DeribitTradeFilter<'_>,
    ) -> RestResult<DeribitTradesPage> {
        self.market_api
            .get_last_trades_by_currency(currency, kind, start_id, end_id, filter)
            .await
    }

    async fn get_last_trades_by_currency_and_time(
        &self,
        currency: &str,
        kind: Option<&str>,
        filter: DeribitTradeFilter<'_>,
    ) -> RestResult<DeribitTradesPage> {
        self.market_api
            .get_last_trades_by_currency_and_time(currency, kind, filter)
            .await
    }

    async fn get_tradingview_chart_data(
        &self,
        instrument_name: &str,
        start_timestamp: u64,
        end_timestamp: u64,
        resolution: &str,
    ) -> RestResult<DeribitTradingViewChartData> {
        self.market_api
            .get_tradingview_chart_data(instrument_name, start_timestamp, end_timestamp, resolution)
            .await
    }

    async fn get_funding_rate_history(
        &self,
        instrument_name: &str,
        start_timestamp: u64,
        end_timestamp: u64,
    ) -> RestResult<Vec<DeribitFundingRateHistory>> {
        self.market_api
            .get_funding_rate_history(instrument_name, start_timestamp, end_timestamp)
            .await
    }

    async fn get_funding_rate_value(
        &self,
        instrument_name: &str,
        start_timestamp: u64,
        end_timestamp: u64,
    ) -> RestResult<f64> {
        self.market_api
            .get_funding_rate_value(instrument_name, start_timestamp, end_timestamp)
            .await
    }

    async fn get_funding_chart_data(
        &self,
        instrument_name: &str,
        length: &str,
    ) -> RestResult<DeribitFundingChartData> {
        self.market_api
            .get_funding_chart_data(instrument_name, length)
            .await
    }

    async fn get_historical_volatility(&self, currency: &str) -> RestResult<Vec<DeribitTimeValue>> {
        self.market_api.get_historical_volatility(currency).await
    }

    async fn get_volatility_index_data(
        &self,
        currency: &str,
        start_timestamp: u64,
        end_timestamp: u64,
        resolution: &str,
    ) -> RestResult<DeribitVolatilityIndexData> {
        self.market_api
            .get_volatility_index_data(currency, start_timestamp, end_timestamp, resolution)
            .await
    }

    async fn get_index_price(&self, index_name: &str) -> RestResult<DeribitIndexPrice> {
        self.market_api.get_index_price(index_name).await
    }

    async fn get_index_price_names(&self) -> RestResult<Vec<String>> {
        self.market_api.get_index_price_names().await
    }

    async fn get_index_price_names_extended(&self) -> RestResult<Vec<DeribitIndexPriceName>> {
        self.market_api.get_index_price_names_extended().await
    }

    async fn get_index_chart_data(
        &self,
        index_name: &str,
        range: &str,
    ) -> RestResult<Vec<DeribitTimeValue>> {
        self.market_api
            .get_index_chart_data(index_name, range)
            .await
    }

    async fn get_basket_index_composition(
        &self,
        index_name: &str,
    ) -> RestResult<DeribitBasketIndexComposition> {
        self.market_api
            .get_basket_index_composition(index_name)
            .await
    }

    async fn get_delivery_prices(
        &self,
        index_name: &str,
        offset: Option<u32>,
        count: Option<u32>,
        start_timestamp: Option<u64>,
        end_timestamp: Option<u64>,
    ) -> RestResult<DeribitDeliveryPrices> {
        self.market_api
            .get_delivery_prices(index_name, offset, count, start_timestamp, end_timestamp)
            .await
    }

    async fn get_last_settlements_by_currency(
        &self,
        currency: &str,
        filter: DeribitSettlementFilter<'_>,
    ) -> RestResult<DeribitSettlementsPage> {
        self.market_api
            .get_last_settlements_by_currency(currency, filter)
            .await
    }

    async fn get_last_settlements_by_instrument(
        &self,
        instrument_name: &str,
        filter: DeribitSettlementFilter<'_>,
    ) -> RestResult<DeribitSettlementsPage> {
        self.market_api
            .get_last_settlements_by_instrument(instrument_name, filter)
            .await
    }

    async fn get_contract_size(&self, instrument_name: &str) -> RestResult<DeribitContractSize> {
        self.market_api.get_contract_size(instrument_name).await
    }

    async fn get_mark_price_history(
        &self,
        instrument_name: &str,
        start_timestamp: u64,
        end_timestamp: u64,
    ) -> RestResult<Vec<DeribitTimeValue>> {
        self.market_api
            .get_mark_price_history(instrument_name, start_timestamp, end_timestamp)
            .await
    }

    async fn get_supported_index_names(&self, index_type: Option<&str>) -> RestResult<Vec<String>> {
        self.market_api.get_supported_index_names(index_type).await
    }

    async fn get_expirations(
        &self,
        currency: Option<&str>,
        kind: Option<&str>,
        currency_pair: Option<&str>,
    ) -> RestResult<DeribitExpirations> {
        self.market_api
            .get_expirations(currency, kind, currency_pair)
            .await
    }

    async fn get_apr_history(
        &self,
        currency: &str,
        limit: Option<u32>,
        before: Option<u64>,
    ) -> RestResult<DeribitAprHistory> {
        self.market_api
            .get_apr_history(currency, limit, before)
            .await
    }

    async fn get_trade_volumes(
        &self,
        extended: Option<bool>,
    ) -> RestResult<Vec<DeribitTradeVolume>> {
        self.market_api.get_trade_volumes(extended).await
    }

    async fn get_combos(&self, currency: &str) -> RestResult<Vec<DeribitCombo>> {
        self.market_api.get_combos(currency).await
    }

    async fn get_combo_ids(&self, currency: &str, state: Option<&str>) -> RestResult<Vec<String>> {
        self.market_api.get_combo_ids(currency, state).await
    }

    async fn get_combo_details(&self, combo_id: &str) -> RestResult<DeribitCombo> {
        self.market_api.get_combo_details(combo_id).await
    }

    async fn get_block_rfq_trades(
        &self,
        currency: &str,
        continuation: Option<&str>,
        count: Option<u32>,
    ) -> RestResult<DeribitBlockRfqTradesPage> {
        self.market_api
            .get_block_rfq_trades(currency, continuation, count)
            .await
    }

    async fn status(&self) -> RestResult<DeribitStatus> {
        self.market_api.status().await
    }

    async fn get_time(&self) -> RestResult<u64> {
        self.market_api.get_time().await
    }

    async fn test(&self, expected_result: Option<&str>) -> RestResult<DeribitVersion> {
        self.market_api.test(expected_result).await
    }
}
