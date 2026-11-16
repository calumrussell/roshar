/// Wire and message types for this exchange (websocket messages, REST payloads).
pub mod types;

// Endpoints
pub const PHOENIX_WSS_URL: &str = "wss://perp-api.phoenix.trade/v1/ws";
pub const PHOENIX_REST_URL: &str = "https://perp-api.phoenix.trade";

pub(crate) mod rest;

use rest::MarketApi;

pub use rest::MarketApi as PhoenixMarketApi;

use crate::types::{
    PhoenixCandle, PhoenixCollateralEvent, PhoenixCommodityCalendar, PhoenixExchange,
    PhoenixExchangeKeys, PhoenixExchangeSnapshot, PhoenixExchangeStatus, PhoenixFill,
    PhoenixFundingHistoryResponse, PhoenixFundingOverview, PhoenixFundingRateHistory,
    PhoenixMarkPrice, PhoenixMarketCalendarRecord, PhoenixMarketCalendarResponse,
    PhoenixMarketConfig, PhoenixMarketStatsHistory, PhoenixNextCommodityMarketTransition,
    PhoenixNextMarketCalendarTransition, PhoenixOrderBook, PhoenixOrderHistory,
    PhoenixPaginatedResponse, PhoenixPnlPoint, PhoenixTradeHistory, PhoenixTraderState,
};
use anyhow::Result;
use async_trait::async_trait;
use roshar_ws_mgr::{Config, Manager, Message};
use std::sync::Arc;

pub struct PhoenixClient {
    market_api: MarketApi,
    ws_config: Config,
}

impl Default for PhoenixClient {
    fn default() -> Self {
        Self::new()
    }
}

impl PhoenixClient {
    /// Phoenix publishes no numeric limit. Measured from an unauthenticated client on
    /// 2026-11-15: 2 req/s sustained is accepted; 3 req/s and above draws 429s
    /// (`retry-after: 2`). Authenticated sessions get higher limits.
    const DEFAULT_REQUESTS_PER_SECOND: u32 = 2;
    const WS_CONN_NAME: &str = "phoenix";

    pub fn new() -> Self {
        Self::new_with_rate_limit(Self::DEFAULT_REQUESTS_PER_SECOND)
    }

    pub fn new_with_rate_limit(requests_per_second: u32) -> Self {
        Self {
            market_api: MarketApi::new(requests_per_second),
            ws_config: Config {
                name: Self::WS_CONN_NAME.to_string(),
                url: PHOENIX_WSS_URL.to_string(),
                // Phoenix has no JSON ping (a `{"type":"ping"}` frame is rejected with a
                // 400 error); keepalive is WebSocket ping/pong frames.
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

    /// Subscribe to mid prices for all listed markets.
    pub fn subscribe_all_mids(&self, manager: &Arc<Manager>) -> Result<()> {
        let msg = crate::types::PhoenixWssMessage::all_mids().to_json();
        manager.write(
            &self.ws_config.name,
            Message::TextMessage(self.ws_config.name.clone(), msg),
        )?;
        Ok(())
    }

    pub fn unsubscribe_all_mids(&self, manager: &Arc<Manager>) -> Result<()> {
        let msg = crate::types::PhoenixWssMessage::all_mids_unsub().to_json();
        manager.write(
            &self.ws_config.name,
            Message::TextMessage(self.ws_config.name.clone(), msg),
        )?;
        Ok(())
    }

    /// Subscribe to exchange/market configuration (snapshot, then deltas).
    /// `encoding`: `Some("json")` for a JSON snapshot; `None` for the default
    /// `base64+zstd` encoded snapshot. The server rejects re-requesting a snapshot
    /// within ~15 s on the same connection.
    pub fn subscribe_exchange(&self, manager: &Arc<Manager>, encoding: Option<&str>) -> Result<()> {
        let msg = crate::types::PhoenixWssMessage::exchange(encoding).to_json();
        manager.write(
            &self.ws_config.name,
            Message::TextMessage(self.ws_config.name.clone(), msg),
        )?;
        Ok(())
    }

    pub fn unsubscribe_exchange(
        &self,
        manager: &Arc<Manager>,
        encoding: Option<&str>,
    ) -> Result<()> {
        let msg = crate::types::PhoenixWssMessage::exchange_unsub(encoding).to_json();
        manager.write(
            &self.ws_config.name,
            Message::TextMessage(self.ws_config.name.clone(), msg),
        )?;
        Ok(())
    }

    /// Subscribe to L2 orderbook updates for the given market symbols.
    /// `bypass_execution_band: Some(true)` returns the full book during commodities
    /// after-hours.
    pub fn subscribe_orderbook(
        &self,
        manager: &Arc<Manager>,
        symbols: &[&str],
        bypass_execution_band: Option<bool>,
    ) -> Result<()> {
        for symbol in symbols {
            let msg =
                crate::types::PhoenixWssMessage::orderbook(symbol, bypass_execution_band).to_json();
            manager.write(
                &self.ws_config.name,
                Message::TextMessage(self.ws_config.name.clone(), msg),
            )?;
        }
        Ok(())
    }

    pub fn unsubscribe_orderbook(
        &self,
        manager: &Arc<Manager>,
        symbols: &[&str],
        bypass_execution_band: Option<bool>,
    ) -> Result<()> {
        for symbol in symbols {
            let msg =
                crate::types::PhoenixWssMessage::orderbook_unsub(symbol, bypass_execution_band)
                    .to_json();
            manager.write(
                &self.ws_config.name,
                Message::TextMessage(self.ws_config.name.clone(), msg),
            )?;
        }
        Ok(())
    }

    /// Subscribe to trade execution stream for the given market symbols.
    pub fn subscribe_trades(&self, manager: &Arc<Manager>, symbols: &[&str]) -> Result<()> {
        for symbol in symbols {
            let msg = crate::types::PhoenixWssMessage::trades(symbol).to_json();
            manager.write(
                &self.ws_config.name,
                Message::TextMessage(self.ws_config.name.clone(), msg),
            )?;
        }
        Ok(())
    }

    pub fn unsubscribe_trades(&self, manager: &Arc<Manager>, symbols: &[&str]) -> Result<()> {
        for symbol in symbols {
            let msg = crate::types::PhoenixWssMessage::trades_unsub(symbol).to_json();
            manager.write(
                &self.ws_config.name,
                Message::TextMessage(self.ws_config.name.clone(), msg),
            )?;
        }
        Ok(())
    }

    /// Subscribe to market stats (mark price, OI, funding, volume) for the given symbols.
    pub fn subscribe_market(&self, manager: &Arc<Manager>, symbols: &[&str]) -> Result<()> {
        for symbol in symbols {
            let msg = crate::types::PhoenixWssMessage::market(symbol).to_json();
            manager.write(
                &self.ws_config.name,
                Message::TextMessage(self.ws_config.name.clone(), msg),
            )?;
        }
        Ok(())
    }

    pub fn unsubscribe_market(&self, manager: &Arc<Manager>, symbols: &[&str]) -> Result<()> {
        for symbol in symbols {
            let msg = crate::types::PhoenixWssMessage::market_unsub(symbol).to_json();
            manager.write(
                &self.ws_config.name,
                Message::TextMessage(self.ws_config.name.clone(), msg),
            )?;
        }
        Ok(())
    }

    /// Subscribe to OHLCV candle stream for the given symbols.
    /// Valid timeframes: "1s","5s","1m","5m","15m","30m","1h","4h","1d".
    pub fn subscribe_candles(
        &self,
        manager: &Arc<Manager>,
        symbols: &[&str],
        timeframe: &str,
    ) -> Result<()> {
        for symbol in symbols {
            let msg = crate::types::PhoenixWssMessage::candles(symbol, timeframe).to_json();
            manager.write(
                &self.ws_config.name,
                Message::TextMessage(self.ws_config.name.clone(), msg),
            )?;
        }
        Ok(())
    }

    pub fn unsubscribe_candles(
        &self,
        manager: &Arc<Manager>,
        symbols: &[&str],
        timeframe: &str,
    ) -> Result<()> {
        for symbol in symbols {
            let msg = crate::types::PhoenixWssMessage::candles_unsub(symbol, timeframe).to_json();
            manager.write(
                &self.ws_config.name,
                Message::TextMessage(self.ws_config.name.clone(), msg),
            )?;
        }
        Ok(())
    }

    /// Subscribe to per-interval funding rate updates for the given symbols.
    pub fn subscribe_funding_rate(&self, manager: &Arc<Manager>, symbols: &[&str]) -> Result<()> {
        for symbol in symbols {
            let msg = crate::types::PhoenixWssMessage::funding_rate(symbol).to_json();
            manager.write(
                &self.ws_config.name,
                Message::TextMessage(self.ws_config.name.clone(), msg),
            )?;
        }
        Ok(())
    }

    pub fn unsubscribe_funding_rate(&self, manager: &Arc<Manager>, symbols: &[&str]) -> Result<()> {
        for symbol in symbols {
            let msg = crate::types::PhoenixWssMessage::funding_rate_unsub(symbol).to_json();
            manager.write(
                &self.ws_config.name,
                Message::TextMessage(self.ws_config.name.clone(), msg),
            )?;
        }
        Ok(())
    }

    /// Subscribe to trader state updates (positions, orders, collateral) for an authority.
    pub fn subscribe_trader_state(
        &self,
        manager: &Arc<Manager>,
        authority: &str,
        trader_pda_index: u8,
    ) -> Result<()> {
        let msg =
            crate::types::PhoenixWssMessage::trader_state(authority, trader_pda_index).to_json();
        manager.write(
            &self.ws_config.name,
            Message::TextMessage(self.ws_config.name.clone(), msg),
        )?;
        Ok(())
    }

    pub fn unsubscribe_trader_state(
        &self,
        manager: &Arc<Manager>,
        authority: &str,
        trader_pda_index: u8,
    ) -> Result<()> {
        let msg = crate::types::PhoenixWssMessage::trader_state_unsub(authority, trader_pda_index)
            .to_json();
        manager.write(
            &self.ws_config.name,
            Message::TextMessage(self.ws_config.name.clone(), msg),
        )?;
        Ok(())
    }
}

type BoxError = Box<dyn std::error::Error + Send + Sync>;

#[async_trait]
pub trait PhoenixApi {
    async fn get_exchange(&self) -> Result<PhoenixExchange, BoxError>;

    async fn get_markets(&self) -> Result<Vec<PhoenixMarketConfig>, BoxError>;

    async fn get_market(&self, symbol: &str) -> Result<PhoenixMarketConfig, BoxError>;

    async fn get_exchange_keys(&self) -> Result<PhoenixExchangeKeys, BoxError>;

    async fn get_exchange_status(&self) -> Result<PhoenixExchangeStatus, BoxError>;

    async fn get_exchange_snapshot(&self) -> Result<PhoenixExchangeSnapshot, BoxError>;

    async fn get_orderbook(
        &self,
        symbol: &str,
        include_splines: Option<bool>,
        bypass_execution_band: Option<bool>,
    ) -> Result<PhoenixOrderBook, BoxError>;

    async fn get_mark_price(&self, symbol: &str) -> Result<PhoenixMarkPrice, BoxError>;

    async fn get_market_stats(
        &self,
        symbol: &str,
        start_time: Option<&str>,
        end_time: Option<&str>,
        limit: Option<u32>,
        timeframe: Option<&str>,
    ) -> Result<PhoenixMarketStatsHistory, BoxError>;

    async fn get_market_calendar(
        &self,
        symbol: &str,
    ) -> Result<PhoenixMarketCalendarResponse, BoxError>;

    async fn get_market_calendar_by_id(
        &self,
        market_calendar_id: &str,
    ) -> Result<PhoenixMarketCalendarRecord, BoxError>;

    async fn get_commodity_calendar(&self) -> Result<PhoenixCommodityCalendar, BoxError>;

    async fn get_next_market_calendar_transition(
        &self,
        symbol: &str,
    ) -> Result<PhoenixNextMarketCalendarTransition, BoxError>;

    async fn get_next_commodity_market_transition(
        &self,
    ) -> Result<PhoenixNextCommodityMarketTransition, BoxError>;

    async fn get_funding_rates(
        &self,
        symbol: &str,
        start_time: Option<i64>,
        end_time: Option<i64>,
        limit: Option<u32>,
    ) -> Result<PhoenixFundingRateHistory, BoxError>;

    async fn get_funding_overview(
        &self,
        start_time: Option<i64>,
        end_time: Option<i64>,
        per_market_limit: Option<u32>,
    ) -> Result<PhoenixFundingOverview, BoxError>;

    async fn get_market_fills(
        &self,
        symbol: &str,
        limit: Option<u32>,
        cursor: Option<&str>,
        start_time: Option<i64>,
        end_time: Option<i64>,
    ) -> Result<PhoenixPaginatedResponse<PhoenixFill>, BoxError>;

    async fn get_candles(
        &self,
        symbol: &str,
        timeframe: &str,
        start_time: Option<i64>,
        end_time: Option<i64>,
        limit: Option<u32>,
        enable_external_source: Option<bool>,
    ) -> Result<Vec<PhoenixCandle>, BoxError>;

    async fn get_trader_state(
        &self,
        authority: &str,
        trader_pda_index: Option<u8>,
    ) -> Result<PhoenixTraderState, BoxError>;

    async fn get_trader_pnl(
        &self,
        authority: &str,
        resolution: &str,
        start_time: Option<i64>,
        end_time: Option<i64>,
        limit: Option<i64>,
    ) -> Result<Vec<PhoenixPnlPoint>, BoxError>;

    async fn get_order_history(
        &self,
        authority: &str,
        trader_pda_index: Option<u8>,
        market_symbol: Option<&str>,
        limit: i64,
        cursor: Option<&str>,
    ) -> Result<PhoenixPaginatedResponse<PhoenixOrderHistory>, BoxError>;

    async fn get_trades_history(
        &self,
        authority: &str,
        pda_index: u8,
        market_symbol: Option<&str>,
        limit: Option<i64>,
        cursor: Option<&str>,
    ) -> Result<PhoenixPaginatedResponse<PhoenixTradeHistory>, BoxError>;

    #[allow(clippy::too_many_arguments)]
    async fn get_funding_history(
        &self,
        authority: &str,
        pda_index: u8,
        symbol: Option<&str>,
        start_time: Option<i64>,
        end_time: Option<i64>,
        limit: Option<i64>,
        cursor: Option<&str>,
    ) -> Result<PhoenixFundingHistoryResponse, BoxError>;

    async fn get_collateral_history(
        &self,
        authority: &str,
        pda_index: u8,
        limit: i64,
        next_cursor: Option<&str>,
    ) -> Result<PhoenixPaginatedResponse<PhoenixCollateralEvent>, BoxError>;
}

#[async_trait]
impl PhoenixApi for PhoenixClient {
    async fn get_exchange(&self) -> Result<PhoenixExchange, BoxError> {
        self.market_api.get_exchange().await
    }

    async fn get_markets(&self) -> Result<Vec<PhoenixMarketConfig>, BoxError> {
        self.market_api.get_markets().await
    }

    async fn get_market(&self, symbol: &str) -> Result<PhoenixMarketConfig, BoxError> {
        self.market_api.get_market(symbol).await
    }

    async fn get_exchange_keys(&self) -> Result<PhoenixExchangeKeys, BoxError> {
        self.market_api.get_exchange_keys().await
    }

    async fn get_exchange_status(&self) -> Result<PhoenixExchangeStatus, BoxError> {
        self.market_api.get_exchange_status().await
    }

    async fn get_exchange_snapshot(&self) -> Result<PhoenixExchangeSnapshot, BoxError> {
        self.market_api.get_exchange_snapshot().await
    }

    async fn get_orderbook(
        &self,
        symbol: &str,
        include_splines: Option<bool>,
        bypass_execution_band: Option<bool>,
    ) -> Result<PhoenixOrderBook, BoxError> {
        self.market_api
            .get_orderbook(symbol, include_splines, bypass_execution_band)
            .await
    }

    async fn get_mark_price(&self, symbol: &str) -> Result<PhoenixMarkPrice, BoxError> {
        self.market_api.get_mark_price(symbol).await
    }

    async fn get_market_stats(
        &self,
        symbol: &str,
        start_time: Option<&str>,
        end_time: Option<&str>,
        limit: Option<u32>,
        timeframe: Option<&str>,
    ) -> Result<PhoenixMarketStatsHistory, BoxError> {
        self.market_api
            .get_market_stats(symbol, start_time, end_time, limit, timeframe)
            .await
    }

    async fn get_market_calendar(
        &self,
        symbol: &str,
    ) -> Result<PhoenixMarketCalendarResponse, BoxError> {
        self.market_api.get_market_calendar(symbol).await
    }

    async fn get_market_calendar_by_id(
        &self,
        market_calendar_id: &str,
    ) -> Result<PhoenixMarketCalendarRecord, BoxError> {
        self.market_api
            .get_market_calendar_by_id(market_calendar_id)
            .await
    }

    async fn get_commodity_calendar(&self) -> Result<PhoenixCommodityCalendar, BoxError> {
        self.market_api.get_commodity_calendar().await
    }

    async fn get_next_market_calendar_transition(
        &self,
        symbol: &str,
    ) -> Result<PhoenixNextMarketCalendarTransition, BoxError> {
        self.market_api
            .get_next_market_calendar_transition(symbol)
            .await
    }

    async fn get_next_commodity_market_transition(
        &self,
    ) -> Result<PhoenixNextCommodityMarketTransition, BoxError> {
        self.market_api.get_next_commodity_market_transition().await
    }

    async fn get_funding_rates(
        &self,
        symbol: &str,
        start_time: Option<i64>,
        end_time: Option<i64>,
        limit: Option<u32>,
    ) -> Result<PhoenixFundingRateHistory, BoxError> {
        self.market_api
            .get_funding_rates(symbol, start_time, end_time, limit)
            .await
    }

    async fn get_funding_overview(
        &self,
        start_time: Option<i64>,
        end_time: Option<i64>,
        per_market_limit: Option<u32>,
    ) -> Result<PhoenixFundingOverview, BoxError> {
        self.market_api
            .get_funding_overview(start_time, end_time, per_market_limit)
            .await
    }

    async fn get_market_fills(
        &self,
        symbol: &str,
        limit: Option<u32>,
        cursor: Option<&str>,
        start_time: Option<i64>,
        end_time: Option<i64>,
    ) -> Result<PhoenixPaginatedResponse<PhoenixFill>, BoxError> {
        self.market_api
            .get_market_fills(symbol, limit, cursor, start_time, end_time)
            .await
    }

    async fn get_candles(
        &self,
        symbol: &str,
        timeframe: &str,
        start_time: Option<i64>,
        end_time: Option<i64>,
        limit: Option<u32>,
        enable_external_source: Option<bool>,
    ) -> Result<Vec<PhoenixCandle>, BoxError> {
        self.market_api
            .get_candles(
                symbol,
                timeframe,
                start_time,
                end_time,
                limit,
                enable_external_source,
            )
            .await
    }

    async fn get_trader_state(
        &self,
        authority: &str,
        trader_pda_index: Option<u8>,
    ) -> Result<PhoenixTraderState, BoxError> {
        self.market_api
            .get_trader_state(authority, trader_pda_index)
            .await
    }

    async fn get_trader_pnl(
        &self,
        authority: &str,
        resolution: &str,
        start_time: Option<i64>,
        end_time: Option<i64>,
        limit: Option<i64>,
    ) -> Result<Vec<PhoenixPnlPoint>, BoxError> {
        self.market_api
            .get_trader_pnl(authority, resolution, start_time, end_time, limit)
            .await
    }

    async fn get_order_history(
        &self,
        authority: &str,
        trader_pda_index: Option<u8>,
        market_symbol: Option<&str>,
        limit: i64,
        cursor: Option<&str>,
    ) -> Result<PhoenixPaginatedResponse<PhoenixOrderHistory>, BoxError> {
        self.market_api
            .get_order_history(authority, trader_pda_index, market_symbol, limit, cursor)
            .await
    }

    async fn get_trades_history(
        &self,
        authority: &str,
        pda_index: u8,
        market_symbol: Option<&str>,
        limit: Option<i64>,
        cursor: Option<&str>,
    ) -> Result<PhoenixPaginatedResponse<PhoenixTradeHistory>, BoxError> {
        self.market_api
            .get_trades_history(authority, pda_index, market_symbol, limit, cursor)
            .await
    }

    async fn get_funding_history(
        &self,
        authority: &str,
        pda_index: u8,
        symbol: Option<&str>,
        start_time: Option<i64>,
        end_time: Option<i64>,
        limit: Option<i64>,
        cursor: Option<&str>,
    ) -> Result<PhoenixFundingHistoryResponse, BoxError> {
        self.market_api
            .get_funding_history(
                authority, pda_index, symbol, start_time, end_time, limit, cursor,
            )
            .await
    }

    async fn get_collateral_history(
        &self,
        authority: &str,
        pda_index: u8,
        limit: i64,
        next_cursor: Option<&str>,
    ) -> Result<PhoenixPaginatedResponse<PhoenixCollateralEvent>, BoxError> {
        self.market_api
            .get_collateral_history(authority, pda_index, limit, next_cursor)
            .await
    }
}
