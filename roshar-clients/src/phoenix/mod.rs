pub(crate) mod rest;

use rest::MarketApi;

pub use rest::MarketApi as PhoenixMarketApi;

use anyhow::Result;
use async_trait::async_trait;
use roshar_types::phoenix::{
    PhoenixCandle, PhoenixCollateralEvent, PhoenixExchange, PhoenixFundingHistory,
    PhoenixMarketConfig, PhoenixOrderBook, PhoenixOrderHistory, PhoenixPaginatedResponse,
    PhoenixPnlPoint, PhoenixTradeHistory, PhoenixTraderState,
};
use roshar_ws_mgr::{Config, Manager, Message};
use std::sync::Arc;

use crate::ws::ws_config_methods;
use crate::PHOENIX_WSS_URL;

pub struct PhoenixClient {
    market_api: MarketApi,
    ws_config: Config,
}

impl PhoenixClient {
    const DEFAULT_REQUESTS_PER_SECOND: u32 = 10;
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
                ping_duration: 30,
                ping_message: roshar_types::phoenix::PhoenixWssMessage::ping().to_json(),
                ping_timeout: 10,
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
            },
        }
    }

    ws_config_methods!();

    /// Subscribe to mid prices for all listed markets.
    pub fn subscribe_all_mids(&self, manager: &Arc<Manager>) -> Result<()> {
        let msg = roshar_types::phoenix::PhoenixWssMessage::all_mids().to_json();
        manager.write(
            &self.ws_config.name,
            Message::TextMessage(self.ws_config.name.clone(), msg),
        )?;
        Ok(())
    }

    pub fn unsubscribe_all_mids(&self, manager: &Arc<Manager>) -> Result<()> {
        let msg = roshar_types::phoenix::PhoenixWssMessage::all_mids_unsub().to_json();
        manager.write(
            &self.ws_config.name,
            Message::TextMessage(self.ws_config.name.clone(), msg),
        )?;
        Ok(())
    }

    /// Subscribe to L2 orderbook updates for the given market symbols.
    pub fn subscribe_orderbook(&self, manager: &Arc<Manager>, symbols: &[&str]) -> Result<()> {
        for symbol in symbols {
            let msg = roshar_types::phoenix::PhoenixWssMessage::orderbook(symbol).to_json();
            manager.write(
                &self.ws_config.name,
                Message::TextMessage(self.ws_config.name.clone(), msg),
            )?;
        }
        Ok(())
    }

    pub fn unsubscribe_orderbook(&self, manager: &Arc<Manager>, symbols: &[&str]) -> Result<()> {
        for symbol in symbols {
            let msg = roshar_types::phoenix::PhoenixWssMessage::orderbook_unsub(symbol).to_json();
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
            let msg = roshar_types::phoenix::PhoenixWssMessage::trades(symbol).to_json();
            manager.write(
                &self.ws_config.name,
                Message::TextMessage(self.ws_config.name.clone(), msg),
            )?;
        }
        Ok(())
    }

    pub fn unsubscribe_trades(&self, manager: &Arc<Manager>, symbols: &[&str]) -> Result<()> {
        for symbol in symbols {
            let msg = roshar_types::phoenix::PhoenixWssMessage::trades_unsub(symbol).to_json();
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
            let msg = roshar_types::phoenix::PhoenixWssMessage::market(symbol).to_json();
            manager.write(
                &self.ws_config.name,
                Message::TextMessage(self.ws_config.name.clone(), msg),
            )?;
        }
        Ok(())
    }

    pub fn unsubscribe_market(&self, manager: &Arc<Manager>, symbols: &[&str]) -> Result<()> {
        for symbol in symbols {
            let msg = roshar_types::phoenix::PhoenixWssMessage::market_unsub(symbol).to_json();
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
            let msg =
                roshar_types::phoenix::PhoenixWssMessage::candles(symbol, timeframe).to_json();
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
            let msg =
                roshar_types::phoenix::PhoenixWssMessage::candles_unsub(symbol, timeframe)
                    .to_json();
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
            let msg =
                roshar_types::phoenix::PhoenixWssMessage::funding_rate(symbol).to_json();
            manager.write(
                &self.ws_config.name,
                Message::TextMessage(self.ws_config.name.clone(), msg),
            )?;
        }
        Ok(())
    }

    pub fn unsubscribe_funding_rate(
        &self,
        manager: &Arc<Manager>,
        symbols: &[&str],
    ) -> Result<()> {
        for symbol in symbols {
            let msg =
                roshar_types::phoenix::PhoenixWssMessage::funding_rate_unsub(symbol).to_json();
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
            roshar_types::phoenix::PhoenixWssMessage::trader_state(authority, trader_pda_index)
                .to_json();
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
        let msg = roshar_types::phoenix::PhoenixWssMessage::trader_state_unsub(
            authority,
            trader_pda_index,
        )
        .to_json();
        manager.write(
            &self.ws_config.name,
            Message::TextMessage(self.ws_config.name.clone(), msg),
        )?;
        Ok(())
    }
}

#[async_trait]
pub trait PhoenixApi {
    async fn get_exchange(
        &self,
    ) -> Result<PhoenixExchange, Box<dyn std::error::Error + Send + Sync>>;

    async fn get_markets(
        &self,
    ) -> Result<Vec<PhoenixMarketConfig>, Box<dyn std::error::Error + Send + Sync>>;

    async fn get_market(
        &self,
        symbol: &str,
    ) -> Result<PhoenixMarketConfig, Box<dyn std::error::Error + Send + Sync>>;

    async fn get_orderbook(
        &self,
        symbol: &str,
    ) -> Result<PhoenixOrderBook, Box<dyn std::error::Error + Send + Sync>>;

    async fn get_candles(
        &self,
        symbol: &str,
        timeframe: &str,
        start_time: Option<i64>,
        end_time: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<PhoenixCandle>, Box<dyn std::error::Error + Send + Sync>>;

    async fn get_trader_state(
        &self,
        authority: &str,
        pda_index: Option<u8>,
    ) -> Result<Vec<PhoenixTraderState>, Box<dyn std::error::Error + Send + Sync>>;

    async fn get_trader_pnl(
        &self,
        authority: &str,
        resolution: &str,
        start_time: Option<i64>,
        end_time: Option<i64>,
        limit: Option<i64>,
    ) -> Result<Vec<PhoenixPnlPoint>, Box<dyn std::error::Error + Send + Sync>>;

    async fn get_order_history(
        &self,
        authority: &str,
        market_symbol: Option<&str>,
        limit: Option<i64>,
        cursor: Option<&str>,
    ) -> Result<
        PhoenixPaginatedResponse<PhoenixOrderHistory>,
        Box<dyn std::error::Error + Send + Sync>,
    >;

    async fn get_trades_history(
        &self,
        authority: &str,
        pda_index: u8,
        market_symbol: Option<&str>,
        limit: Option<i64>,
        cursor: Option<&str>,
    ) -> Result<
        PhoenixPaginatedResponse<PhoenixTradeHistory>,
        Box<dyn std::error::Error + Send + Sync>,
    >;

    async fn get_funding_history(
        &self,
        authority: &str,
        pda_index: u8,
        symbol: Option<&str>,
        start_time: Option<i64>,
        end_time: Option<i64>,
        limit: Option<i64>,
    ) -> Result<Vec<PhoenixFundingHistory>, Box<dyn std::error::Error + Send + Sync>>;

    async fn get_collateral_history(
        &self,
        authority: &str,
        pda_index: u8,
        limit: Option<i64>,
        cursor: Option<&str>,
    ) -> Result<Vec<PhoenixCollateralEvent>, Box<dyn std::error::Error + Send + Sync>>;
}

#[async_trait]
impl PhoenixApi for PhoenixClient {
    async fn get_exchange(
        &self,
    ) -> Result<PhoenixExchange, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api.get_exchange().await
    }

    async fn get_markets(
        &self,
    ) -> Result<Vec<PhoenixMarketConfig>, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api.get_markets().await
    }

    async fn get_market(
        &self,
        symbol: &str,
    ) -> Result<PhoenixMarketConfig, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api.get_market(symbol).await
    }

    async fn get_orderbook(
        &self,
        symbol: &str,
    ) -> Result<PhoenixOrderBook, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api.get_orderbook(symbol).await
    }

    async fn get_candles(
        &self,
        symbol: &str,
        timeframe: &str,
        start_time: Option<i64>,
        end_time: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<PhoenixCandle>, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api
            .get_candles(symbol, timeframe, start_time, end_time, limit)
            .await
    }

    async fn get_trader_state(
        &self,
        authority: &str,
        pda_index: Option<u8>,
    ) -> Result<Vec<PhoenixTraderState>, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api.get_trader_state(authority, pda_index).await
    }

    async fn get_trader_pnl(
        &self,
        authority: &str,
        resolution: &str,
        start_time: Option<i64>,
        end_time: Option<i64>,
        limit: Option<i64>,
    ) -> Result<Vec<PhoenixPnlPoint>, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api
            .get_trader_pnl(authority, resolution, start_time, end_time, limit)
            .await
    }

    async fn get_order_history(
        &self,
        authority: &str,
        market_symbol: Option<&str>,
        limit: Option<i64>,
        cursor: Option<&str>,
    ) -> Result<
        PhoenixPaginatedResponse<PhoenixOrderHistory>,
        Box<dyn std::error::Error + Send + Sync>,
    > {
        self.market_api
            .get_order_history(authority, market_symbol, limit, cursor)
            .await
    }

    async fn get_trades_history(
        &self,
        authority: &str,
        pda_index: u8,
        market_symbol: Option<&str>,
        limit: Option<i64>,
        cursor: Option<&str>,
    ) -> Result<
        PhoenixPaginatedResponse<PhoenixTradeHistory>,
        Box<dyn std::error::Error + Send + Sync>,
    > {
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
    ) -> Result<Vec<PhoenixFundingHistory>, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api
            .get_funding_history(authority, pda_index, symbol, start_time, end_time, limit)
            .await
    }

    async fn get_collateral_history(
        &self,
        authority: &str,
        pda_index: u8,
        limit: Option<i64>,
        cursor: Option<&str>,
    ) -> Result<Vec<PhoenixCollateralEvent>, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api
            .get_collateral_history(authority, pda_index, limit, cursor)
            .await
    }
}
