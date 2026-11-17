/// Wire and message types for this exchange (websocket messages, REST payloads).
pub mod types;

// USDⓈ-M futures market streams are routed by data type; the unrouted
// `wss://fstream.binance.com/ws` was decommissioned on 2026-04-23.
/// USDⓈ-M futures `/public` route: depth, book tickers, trades.
pub const BINANCE_WSS_PUBLIC_URL: &str = "wss://fstream.binance.com/public/ws";
/// USDⓈ-M futures `/market` route: all other market streams.
pub const BINANCE_WSS_MARKET_URL: &str = "wss://fstream.binance.com/market/ws";
pub const BINANCE_SPOT_WSS_URL: &str = "wss://stream.binance.com:9443/ws";

pub(crate) mod rest;

pub use crate::types::{
    BinanceContractType, BinanceDepthLevels, BinanceDepthSpeed, BinanceKlineInterval,
    BinanceMarkPriceSpeed, BinanceStatsPeriod, BinanceStream, BinanceWsEvent, BinanceWsRoute,
};

use rest::BinanceRestClient;

use crate::types::{
    AdlRisk, AssetIndex, Basis, BinanceAggTrade, BinanceHistoricalFundingRate, BinanceKline,
    BinanceOrderBookSnapshot, BinancePremiumIndex, BinanceTrade, BinanceWssMessage, BookTicker,
    CompositeIndexInfo, DeliveryPrice, ExchangeInfo, FundingInfo, IndexConstituents,
    InsuranceBalance, LongShortRatio, OpenInterestData, OpenInterestHist, PriceTicker, ServerTime,
    TakerBuySellVolume, TickerData, TradingSchedule,
};
use anyhow::Result;
use async_trait::async_trait;
use roshar_ws_mgr::{Config, Manager, Message};
use std::sync::Arc;

pub struct BinanceClient {
    rest_client: BinanceRestClient,
    ws_public_config: Config,
    ws_market_config: Config,
}

fn ws_config(name: &str, url: &str) -> Config {
    Config {
        name: name.to_string(),
        url: url.to_string(),
        ping_duration: 10,
        ping_message: BinanceWssMessage::ping().to_json(),
        ping_timeout: 10,
        reconnect_timeout: 90,
        use_text_ping: Some(false),
        read_buffer_size: Some(33554432),
        write_buffer_size: Some(2097152),
        max_message_size: Some(41943040),
        max_frame_size: Some(20971520),
        tcp_recv_buffer_size: Some(16777216),
        tcp_send_buffer_size: Some(4194304),
        tcp_nodelay: Some(true),
        broadcast_channel_size: Some(131072),
        headers: vec![],
    }
}

impl BinanceClient {
    const WS_PUBLIC_CONN_NAME: &str = "binance-public";
    const WS_MARKET_CONN_NAME: &str = "binance-market";

    pub fn new(requests_per_second: u32) -> Self {
        Self {
            rest_client: BinanceRestClient::new(requests_per_second),
            ws_public_config: ws_config(Self::WS_PUBLIC_CONN_NAME, BINANCE_WSS_PUBLIC_URL),
            ws_market_config: ws_config(Self::WS_MARKET_CONN_NAME, BINANCE_WSS_MARKET_URL),
        }
    }

    /// Connection for `/public` streams: depth, book tickers, trades.
    pub fn ws_public_config(&self) -> roshar_ws_mgr::Config {
        self.ws_public_config.clone()
    }

    pub fn ws_public_config_mut(&mut self) -> &mut roshar_ws_mgr::Config {
        &mut self.ws_public_config
    }

    /// Connection for `/market` streams: everything else.
    pub fn ws_market_config(&self) -> roshar_ws_mgr::Config {
        self.ws_market_config.clone()
    }

    pub fn ws_market_config_mut(&mut self) -> &mut roshar_ws_mgr::Config {
        &mut self.ws_market_config
    }

    fn config(&self, route: BinanceWsRoute) -> &Config {
        match route {
            BinanceWsRoute::Public => &self.ws_public_config,
            BinanceWsRoute::Market => &self.ws_market_config,
        }
    }

    fn send(
        &self,
        manager: &Arc<Manager>,
        route: BinanceWsRoute,
        msg: BinanceWssMessage,
    ) -> Result<()> {
        let config = self.config(route);
        manager.write(
            &config.name,
            Message::TextMessage(config.name.clone(), msg.to_json()),
        )?;
        Ok(())
    }

    /// Sends one batched message per route that `streams` touch.
    fn send_routed(
        &self,
        manager: &Arc<Manager>,
        streams: &[BinanceStream],
        build: fn(&[BinanceStream]) -> BinanceWssMessage,
    ) -> Result<()> {
        for route in [BinanceWsRoute::Public, BinanceWsRoute::Market] {
            let routed: Vec<BinanceStream> = streams
                .iter()
                .filter(|s| s.route() == route)
                .cloned()
                .collect();
            if !routed.is_empty() {
                self.send(manager, route, build(&routed))?;
            }
        }
        Ok(())
    }

    /// Subscribes each stream on the connection of its route.
    pub fn subscribe(&self, manager: &Arc<Manager>, streams: &[BinanceStream]) -> Result<()> {
        self.send_routed(manager, streams, BinanceWssMessage::subscribe)
    }

    pub fn unsubscribe(&self, manager: &Arc<Manager>, streams: &[BinanceStream]) -> Result<()> {
        self.send_routed(manager, streams, BinanceWssMessage::unsubscribe)
    }

    /// Asks one connection for its subscriptions; the reply is a `BinanceWsEvent::Response`.
    pub fn list_subscriptions(&self, manager: &Arc<Manager>, route: BinanceWsRoute) -> Result<()> {
        self.send(manager, route, BinanceWssMessage::list_subscriptions())
    }
}

/// USDⓈ-M futures REST market data. Parameters mirror the endpoint's query; `None` omits them.
/// Limits and weights are documented on `rest::BinanceRestClient`.
#[async_trait]
pub trait BinanceApi {
    async fn ping(&self) -> Result<(), String>;
    async fn get_server_time(&self) -> Result<ServerTime, String>;
    async fn get_exchange_info(&self) -> Result<ExchangeInfo, String>;
    async fn get_depth_snapshot(
        &self,
        symbol: &str,
        limit: Option<u32>,
    ) -> Result<BinanceOrderBookSnapshot, String>;
    async fn get_rpi_depth_snapshot(
        &self,
        symbol: &str,
        limit: Option<u32>,
    ) -> Result<BinanceOrderBookSnapshot, String>;
    async fn get_recent_trades(
        &self,
        symbol: &str,
        limit: Option<u32>,
    ) -> Result<Vec<BinanceTrade>, String>;
    async fn get_historical_trades(
        &self,
        api_key: &str,
        symbol: &str,
        limit: Option<u32>,
        from_id: Option<i64>,
    ) -> Result<Vec<BinanceTrade>, String>;
    async fn get_agg_trades(
        &self,
        symbol: &str,
        start_time: Option<i64>,
        end_time: Option<i64>,
        from_id: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<BinanceAggTrade>, String>;
    async fn get_klines(
        &self,
        symbol: &str,
        interval: BinanceKlineInterval,
        start_time: Option<i64>,
        end_time: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<BinanceKline>, String>;
    async fn get_continuous_klines(
        &self,
        pair: &str,
        contract_type: BinanceContractType,
        interval: BinanceKlineInterval,
        start_time: Option<i64>,
        end_time: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<BinanceKline>, String>;
    async fn get_index_price_klines(
        &self,
        pair: &str,
        interval: BinanceKlineInterval,
        start_time: Option<i64>,
        end_time: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<BinanceKline>, String>;
    async fn get_mark_price_klines(
        &self,
        symbol: &str,
        interval: BinanceKlineInterval,
        start_time: Option<i64>,
        end_time: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<BinanceKline>, String>;
    async fn get_premium_index_klines(
        &self,
        symbol: &str,
        interval: BinanceKlineInterval,
        start_time: Option<i64>,
        end_time: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<BinanceKline>, String>;
    async fn get_premium_index(
        &self,
        symbol: Option<&str>,
    ) -> Result<Vec<BinancePremiumIndex>, String>;
    async fn get_funding_rate_history(
        &self,
        symbol: Option<&str>,
        start_time: Option<i64>,
        end_time: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<BinanceHistoricalFundingRate>, String>;
    async fn get_funding_info(&self) -> Result<Vec<FundingInfo>, String>;
    async fn get_24hr_ticker(&self, symbol: Option<&str>) -> Result<Vec<TickerData>, String>;
    async fn get_price_ticker(&self, symbol: Option<&str>) -> Result<Vec<PriceTicker>, String>;
    async fn get_price_ticker_v2(&self, symbol: Option<&str>) -> Result<Vec<PriceTicker>, String>;
    async fn get_book_ticker(&self, symbol: Option<&str>) -> Result<Vec<BookTicker>, String>;
    async fn get_open_interest(&self, symbol: &str) -> Result<OpenInterestData, String>;
    async fn get_delivery_price(&self, pair: &str) -> Result<Vec<DeliveryPrice>, String>;
    async fn get_open_interest_hist(
        &self,
        symbol: &str,
        period: BinanceStatsPeriod,
        limit: Option<u32>,
        start_time: Option<i64>,
        end_time: Option<i64>,
    ) -> Result<Vec<OpenInterestHist>, String>;
    async fn get_top_long_short_account_ratio(
        &self,
        symbol: &str,
        period: BinanceStatsPeriod,
        limit: Option<u32>,
        start_time: Option<i64>,
        end_time: Option<i64>,
    ) -> Result<Vec<LongShortRatio>, String>;
    async fn get_top_long_short_position_ratio(
        &self,
        symbol: &str,
        period: BinanceStatsPeriod,
        limit: Option<u32>,
        start_time: Option<i64>,
        end_time: Option<i64>,
    ) -> Result<Vec<LongShortRatio>, String>;
    async fn get_global_long_short_account_ratio(
        &self,
        symbol: &str,
        period: BinanceStatsPeriod,
        limit: Option<u32>,
        start_time: Option<i64>,
        end_time: Option<i64>,
    ) -> Result<Vec<LongShortRatio>, String>;
    async fn get_taker_buy_sell_volume(
        &self,
        symbol: &str,
        period: BinanceStatsPeriod,
        limit: Option<u32>,
        start_time: Option<i64>,
        end_time: Option<i64>,
    ) -> Result<Vec<TakerBuySellVolume>, String>;
    async fn get_basis(
        &self,
        pair: &str,
        contract_type: BinanceContractType,
        period: BinanceStatsPeriod,
        limit: Option<u32>,
        start_time: Option<i64>,
        end_time: Option<i64>,
    ) -> Result<Vec<Basis>, String>;
    async fn get_composite_index_info(
        &self,
        symbol: Option<&str>,
    ) -> Result<Vec<CompositeIndexInfo>, String>;
    async fn get_asset_index(&self, symbol: Option<&str>) -> Result<Vec<AssetIndex>, String>;
    async fn get_index_constituents(&self, symbol: &str) -> Result<IndexConstituents, String>;
    async fn get_insurance_balance(
        &self,
        symbol: Option<&str>,
    ) -> Result<Vec<InsuranceBalance>, String>;
    async fn get_adl_risk(&self, symbol: Option<&str>) -> Result<Vec<AdlRisk>, String>;
    async fn get_trading_schedule(&self) -> Result<TradingSchedule, String>;
}

#[async_trait]
impl BinanceApi for BinanceClient {
    async fn ping(&self) -> Result<(), String> {
        self.rest_client
            .ping()
            .await
            .map_err(|e| format!("Failed to get ping: {e}"))
    }

    async fn get_server_time(&self) -> Result<ServerTime, String> {
        self.rest_client
            .get_server_time()
            .await
            .map_err(|e| format!("Failed to get server time: {e}"))
    }

    async fn get_exchange_info(&self) -> Result<ExchangeInfo, String> {
        self.rest_client
            .get_exchange_info()
            .await
            .map_err(|e| format!("Failed to get exchange info: {e}"))
    }

    async fn get_depth_snapshot(
        &self,
        symbol: &str,
        limit: Option<u32>,
    ) -> Result<BinanceOrderBookSnapshot, String> {
        self.rest_client
            .get_depth_snapshot(symbol, limit)
            .await
            .map_err(|e| format!("Failed to get depth snapshot: {e}"))
    }

    async fn get_rpi_depth_snapshot(
        &self,
        symbol: &str,
        limit: Option<u32>,
    ) -> Result<BinanceOrderBookSnapshot, String> {
        self.rest_client
            .get_rpi_depth_snapshot(symbol, limit)
            .await
            .map_err(|e| format!("Failed to get RPI depth snapshot: {e}"))
    }

    async fn get_recent_trades(
        &self,
        symbol: &str,
        limit: Option<u32>,
    ) -> Result<Vec<BinanceTrade>, String> {
        self.rest_client
            .get_recent_trades(symbol, limit)
            .await
            .map_err(|e| format!("Failed to get recent trades: {e}"))
    }

    async fn get_historical_trades(
        &self,
        api_key: &str,
        symbol: &str,
        limit: Option<u32>,
        from_id: Option<i64>,
    ) -> Result<Vec<BinanceTrade>, String> {
        self.rest_client
            .get_historical_trades(api_key, symbol, limit, from_id)
            .await
            .map_err(|e| format!("Failed to get historical trades: {e}"))
    }

    async fn get_agg_trades(
        &self,
        symbol: &str,
        start_time: Option<i64>,
        end_time: Option<i64>,
        from_id: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<BinanceAggTrade>, String> {
        self.rest_client
            .get_agg_trades(symbol, start_time, end_time, from_id, limit)
            .await
            .map_err(|e| format!("Failed to get aggregate trades: {e}"))
    }

    async fn get_klines(
        &self,
        symbol: &str,
        interval: BinanceKlineInterval,
        start_time: Option<i64>,
        end_time: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<BinanceKline>, String> {
        self.rest_client
            .get_klines(symbol, interval, start_time, end_time, limit)
            .await
            .map_err(|e| format!("Failed to get klines: {e}"))
    }

    async fn get_continuous_klines(
        &self,
        pair: &str,
        contract_type: BinanceContractType,
        interval: BinanceKlineInterval,
        start_time: Option<i64>,
        end_time: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<BinanceKline>, String> {
        self.rest_client
            .get_continuous_klines(pair, contract_type, interval, start_time, end_time, limit)
            .await
            .map_err(|e| format!("Failed to get continuous klines: {e}"))
    }

    async fn get_index_price_klines(
        &self,
        pair: &str,
        interval: BinanceKlineInterval,
        start_time: Option<i64>,
        end_time: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<BinanceKline>, String> {
        self.rest_client
            .get_index_price_klines(pair, interval, start_time, end_time, limit)
            .await
            .map_err(|e| format!("Failed to get index price klines: {e}"))
    }

    async fn get_mark_price_klines(
        &self,
        symbol: &str,
        interval: BinanceKlineInterval,
        start_time: Option<i64>,
        end_time: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<BinanceKline>, String> {
        self.rest_client
            .get_mark_price_klines(symbol, interval, start_time, end_time, limit)
            .await
            .map_err(|e| format!("Failed to get mark price klines: {e}"))
    }

    async fn get_premium_index_klines(
        &self,
        symbol: &str,
        interval: BinanceKlineInterval,
        start_time: Option<i64>,
        end_time: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<BinanceKline>, String> {
        self.rest_client
            .get_premium_index_klines(symbol, interval, start_time, end_time, limit)
            .await
            .map_err(|e| format!("Failed to get premium index klines: {e}"))
    }

    async fn get_premium_index(
        &self,
        symbol: Option<&str>,
    ) -> Result<Vec<BinancePremiumIndex>, String> {
        self.rest_client
            .get_premium_index(symbol)
            .await
            .map_err(|e| format!("Failed to get premium index: {e}"))
    }

    async fn get_funding_rate_history(
        &self,
        symbol: Option<&str>,
        start_time: Option<i64>,
        end_time: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<BinanceHistoricalFundingRate>, String> {
        self.rest_client
            .get_funding_rate_history(symbol, start_time, end_time, limit)
            .await
            .map_err(|e| format!("Failed to get funding rate history: {e}"))
    }

    async fn get_funding_info(&self) -> Result<Vec<FundingInfo>, String> {
        self.rest_client
            .get_funding_info()
            .await
            .map_err(|e| format!("Failed to get funding info: {e}"))
    }

    async fn get_24hr_ticker(&self, symbol: Option<&str>) -> Result<Vec<TickerData>, String> {
        self.rest_client
            .get_24hr_ticker(symbol)
            .await
            .map_err(|e| format!("Failed to get 24hr ticker: {e}"))
    }

    async fn get_price_ticker(&self, symbol: Option<&str>) -> Result<Vec<PriceTicker>, String> {
        self.rest_client
            .get_price_ticker(symbol)
            .await
            .map_err(|e| format!("Failed to get price ticker: {e}"))
    }

    async fn get_price_ticker_v2(&self, symbol: Option<&str>) -> Result<Vec<PriceTicker>, String> {
        self.rest_client
            .get_price_ticker_v2(symbol)
            .await
            .map_err(|e| format!("Failed to get price ticker v2: {e}"))
    }

    async fn get_book_ticker(&self, symbol: Option<&str>) -> Result<Vec<BookTicker>, String> {
        self.rest_client
            .get_book_ticker(symbol)
            .await
            .map_err(|e| format!("Failed to get book ticker: {e}"))
    }

    async fn get_open_interest(&self, symbol: &str) -> Result<OpenInterestData, String> {
        self.rest_client
            .get_open_interest(symbol)
            .await
            .map_err(|e| format!("Failed to get open interest: {e}"))
    }

    async fn get_delivery_price(&self, pair: &str) -> Result<Vec<DeliveryPrice>, String> {
        self.rest_client
            .get_delivery_price(pair)
            .await
            .map_err(|e| format!("Failed to get delivery price: {e}"))
    }

    async fn get_open_interest_hist(
        &self,
        symbol: &str,
        period: BinanceStatsPeriod,
        limit: Option<u32>,
        start_time: Option<i64>,
        end_time: Option<i64>,
    ) -> Result<Vec<OpenInterestHist>, String> {
        self.rest_client
            .get_open_interest_hist(symbol, period, limit, start_time, end_time)
            .await
            .map_err(|e| format!("Failed to get open interest history: {e}"))
    }

    async fn get_top_long_short_account_ratio(
        &self,
        symbol: &str,
        period: BinanceStatsPeriod,
        limit: Option<u32>,
        start_time: Option<i64>,
        end_time: Option<i64>,
    ) -> Result<Vec<LongShortRatio>, String> {
        self.rest_client
            .get_top_long_short_account_ratio(symbol, period, limit, start_time, end_time)
            .await
            .map_err(|e| format!("Failed to get top trader account ratio: {e}"))
    }

    async fn get_top_long_short_position_ratio(
        &self,
        symbol: &str,
        period: BinanceStatsPeriod,
        limit: Option<u32>,
        start_time: Option<i64>,
        end_time: Option<i64>,
    ) -> Result<Vec<LongShortRatio>, String> {
        self.rest_client
            .get_top_long_short_position_ratio(symbol, period, limit, start_time, end_time)
            .await
            .map_err(|e| format!("Failed to get top trader position ratio: {e}"))
    }

    async fn get_global_long_short_account_ratio(
        &self,
        symbol: &str,
        period: BinanceStatsPeriod,
        limit: Option<u32>,
        start_time: Option<i64>,
        end_time: Option<i64>,
    ) -> Result<Vec<LongShortRatio>, String> {
        self.rest_client
            .get_global_long_short_account_ratio(symbol, period, limit, start_time, end_time)
            .await
            .map_err(|e| format!("Failed to get global long/short ratio: {e}"))
    }

    async fn get_taker_buy_sell_volume(
        &self,
        symbol: &str,
        period: BinanceStatsPeriod,
        limit: Option<u32>,
        start_time: Option<i64>,
        end_time: Option<i64>,
    ) -> Result<Vec<TakerBuySellVolume>, String> {
        self.rest_client
            .get_taker_buy_sell_volume(symbol, period, limit, start_time, end_time)
            .await
            .map_err(|e| format!("Failed to get taker buy/sell volume: {e}"))
    }

    async fn get_basis(
        &self,
        pair: &str,
        contract_type: BinanceContractType,
        period: BinanceStatsPeriod,
        limit: Option<u32>,
        start_time: Option<i64>,
        end_time: Option<i64>,
    ) -> Result<Vec<Basis>, String> {
        self.rest_client
            .get_basis(pair, contract_type, period, limit, start_time, end_time)
            .await
            .map_err(|e| format!("Failed to get basis: {e}"))
    }

    async fn get_composite_index_info(
        &self,
        symbol: Option<&str>,
    ) -> Result<Vec<CompositeIndexInfo>, String> {
        self.rest_client
            .get_composite_index_info(symbol)
            .await
            .map_err(|e| format!("Failed to get composite index info: {e}"))
    }

    async fn get_asset_index(&self, symbol: Option<&str>) -> Result<Vec<AssetIndex>, String> {
        self.rest_client
            .get_asset_index(symbol)
            .await
            .map_err(|e| format!("Failed to get asset index: {e}"))
    }

    async fn get_index_constituents(&self, symbol: &str) -> Result<IndexConstituents, String> {
        self.rest_client
            .get_index_constituents(symbol)
            .await
            .map_err(|e| format!("Failed to get index constituents: {e}"))
    }

    async fn get_insurance_balance(
        &self,
        symbol: Option<&str>,
    ) -> Result<Vec<InsuranceBalance>, String> {
        self.rest_client
            .get_insurance_balance(symbol)
            .await
            .map_err(|e| format!("Failed to get insurance balance: {e}"))
    }

    async fn get_adl_risk(&self, symbol: Option<&str>) -> Result<Vec<AdlRisk>, String> {
        self.rest_client
            .get_adl_risk(symbol)
            .await
            .map_err(|e| format!("Failed to get ADL risk: {e}"))
    }

    async fn get_trading_schedule(&self) -> Result<TradingSchedule, String> {
        self.rest_client
            .get_trading_schedule()
            .await
            .map_err(|e| format!("Failed to get trading schedule: {e}"))
    }
}
