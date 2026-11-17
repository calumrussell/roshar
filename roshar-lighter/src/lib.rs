/// Wire and message types for this exchange (websocket messages, REST payloads).
pub mod types;

// Endpoints (https://docs.lighter.xyz/api/websocket)
pub const LIGHTER_WSS_URL: &str = "wss://mainnet.zklighter.elliot.ai/stream";
/// Market-data-only stream for clients in restricted jurisdictions (the plain URL returns HTTP 400,
/// code 20558, there).
pub const LIGHTER_READONLY_WSS_URL: &str = "wss://mainnet.zklighter.elliot.ai/stream?readonly=true";
pub const LIGHTER_TESTNET_WSS_URL: &str = "wss://testnet.zklighter.elliot.ai/stream";

pub mod exchange;
pub(crate) mod rest;
pub mod signer;
pub mod ws_tx;

use rest::MarketApi;

pub use rest::market::{LighterCandleRange, LighterEventsQuery, LighterTradesQuery};
pub use rest::MarketApi as LighterMarketApi;

use crate::types::{
    LighterAsset, LighterCandles, LighterEvents, LighterExchangeStats, LighterFeeTiers,
    LighterFundingRate, LighterFundings, LighterMarkPriceCandles, LighterMarket,
    LighterMarketPriceCharts, LighterOrderBook, LighterOrderBookDetails, LighterPriceOracleInfo,
    LighterStatus, LighterTrade, LighterTradesResponse, LighterUpDownCandles, LighterWssMessage,
};
use anyhow::Result;
use async_trait::async_trait;
use roshar_ws_mgr::{Config, Manager, Message};
use std::sync::Arc;

pub struct LighterClient {
    market_api: MarketApi,
    ws_config: Config,
}

impl Default for LighterClient {
    fn default() -> Self {
        Self::new()
    }
}

impl LighterClient {
    /// Standard (unauthenticated) accounts get 60 requests per rolling minute, and `recentTrades`
    /// (weight 600 against the 24,000 weighted budget) caps at 40 per minute. 2 requests every 3 s
    /// keeps every endpoint here within the limit.
    /// https://docs.lighter.xyz/developers/reference/rate-limits
    const DEFAULT_RATE_LIMIT: (u32, u64) = (2, 3);
    const WS_CONN_NAME: &str = "lighter";

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
                url: LIGHTER_WSS_URL.to_string(),
                // Lighter disconnects if no frame is received within 2 minutes.
                ping_duration: 90,
                ping_message: crate::types::LIGHTER_WS_PING.to_string(),
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

    /// Subscribe to order book updates for a market by its integer index.
    pub fn subscribe_depth(&self, manager: &Arc<Manager>, market_indices: &[u32]) -> Result<()> {
        for &idx in market_indices {
            let msg = crate::types::LighterWssMessage::depth(idx).to_json();
            manager.write(
                &self.ws_config.name,
                Message::TextMessage(self.ws_config.name.clone(), msg),
            )?;
        }
        Ok(())
    }

    pub fn unsubscribe_depth(&self, manager: &Arc<Manager>, market_indices: &[u32]) -> Result<()> {
        for &idx in market_indices {
            let msg = crate::types::LighterWssMessage::depth_unsub(idx).to_json();
            manager.write(
                &self.ws_config.name,
                Message::TextMessage(self.ws_config.name.clone(), msg),
            )?;
        }
        Ok(())
    }

    /// Subscribe to trade execution stream for a market.
    pub fn subscribe_trades(&self, manager: &Arc<Manager>, market_indices: &[u32]) -> Result<()> {
        for &idx in market_indices {
            let msg = crate::types::LighterWssMessage::trades(idx).to_json();
            manager.write(
                &self.ws_config.name,
                Message::TextMessage(self.ws_config.name.clone(), msg),
            )?;
        }
        Ok(())
    }

    pub fn unsubscribe_trades(&self, manager: &Arc<Manager>, market_indices: &[u32]) -> Result<()> {
        for &idx in market_indices {
            let msg = crate::types::LighterWssMessage::trades_unsub(idx).to_json();
            manager.write(
                &self.ws_config.name,
                Message::TextMessage(self.ws_config.name.clone(), msg),
            )?;
        }
        Ok(())
    }

    /// Subscribe to market stats (mark price, funding rate, 24h volume) for a market.
    pub fn subscribe_market_stats(
        &self,
        manager: &Arc<Manager>,
        market_indices: &[u32],
    ) -> Result<()> {
        for &idx in market_indices {
            let msg = crate::types::LighterWssMessage::market_stats(idx).to_json();
            manager.write(
                &self.ws_config.name,
                Message::TextMessage(self.ws_config.name.clone(), msg),
            )?;
        }
        Ok(())
    }

    /// Subscribe to market stats for all perp markets in a single message (`partial` sent only
    /// when `Some`; parse partial updates with `LighterWsMessage::parse_partial`).
    pub fn subscribe_all_market_stats(
        &self,
        manager: &Arc<Manager>,
        partial: Option<bool>,
    ) -> Result<()> {
        self.send_ws_message(manager, &LighterWssMessage::all_market_stats(partial))
    }

    /// Subscribe to best-bid/offer ticker for a market.
    pub fn subscribe_ticker(&self, manager: &Arc<Manager>, market_indices: &[u32]) -> Result<()> {
        for &idx in market_indices {
            self.send_ws_message(manager, &LighterWssMessage::ticker(idx))?;
        }
        Ok(())
    }

    /// Subscribe to `spot_market_stats/{m}` for each spot market.
    pub fn subscribe_spot_market_stats(
        &self,
        manager: &Arc<Manager>,
        market_indices: &[u32],
    ) -> Result<()> {
        for &idx in market_indices {
            self.send_ws_message(manager, &LighterWssMessage::spot_market_stats(idx))?;
        }
        Ok(())
    }

    /// Subscribe to `bo_market_stats/{m}` for each binary-options market.
    pub fn subscribe_bo_market_stats(
        &self,
        manager: &Arc<Manager>,
        market_indices: &[u32],
    ) -> Result<()> {
        for &idx in market_indices {
            self.send_ws_message(manager, &LighterWssMessage::bo_market_stats(idx))?;
        }
        Ok(())
    }

    /// Subscribe to `candle/{m}/{resolution}` for each market.
    pub fn subscribe_candles(
        &self,
        manager: &Arc<Manager>,
        market_indices: &[u32],
        resolution: &str,
    ) -> Result<()> {
        for &idx in market_indices {
            self.send_ws_message(manager, &LighterWssMessage::candle(idx, resolution))?;
        }
        Ok(())
    }

    /// Subscribe to `mark_price_candle/{m}/{resolution}` for each market.
    pub fn subscribe_mark_price_candles(
        &self,
        manager: &Arc<Manager>,
        market_indices: &[u32],
        resolution: &str,
    ) -> Result<()> {
        for &idx in market_indices {
            self.send_ws_message(
                manager,
                &LighterWssMessage::mark_price_candle(idx, resolution),
            )?;
        }
        Ok(())
    }

    /// Sends any subscribe/unsubscribe message (list and partial forms, `up_down_candle`,
    /// `height`, unsubscribes built with `into_unsubscribe`).
    pub fn send_ws_message(&self, manager: &Arc<Manager>, msg: &LighterWssMessage) -> Result<()> {
        manager.write(
            &self.ws_config.name,
            Message::TextMessage(self.ws_config.name.clone(), msg.to_json()),
        )?;
        Ok(())
    }
}

type BoxError = Box<dyn std::error::Error + Send + Sync>;

#[async_trait]
pub trait LighterApi {
    async fn get_status(&self) -> Result<LighterStatus, BoxError>;

    async fn get_markets(&self) -> Result<Vec<LighterMarket>, BoxError>;

    /// `filter`: `all`, `spot` or `perp`.
    async fn get_market_details(
        &self,
        market_id: Option<u32>,
        filter: Option<&str>,
    ) -> Result<LighterOrderBookDetails, BoxError>;

    /// `limit`: orders per side, 1..=250.
    async fn get_orderbook(&self, market_id: u32, limit: u32)
        -> Result<LighterOrderBook, BoxError>;

    /// `limit`: 1..=100.
    async fn get_recent_trades(
        &self,
        market_id: u32,
        limit: u32,
    ) -> Result<Vec<LighterTrade>, BoxError>;

    async fn get_trades(
        &self,
        query: &LighterTradesQuery<'_>,
    ) -> Result<LighterTradesResponse, BoxError>;

    async fn get_candles(
        &self,
        range: &LighterCandleRange<'_>,
        set_timestamp_to_end: Option<bool>,
        merge_mark_price: Option<bool>,
    ) -> Result<LighterCandles, BoxError>;

    async fn get_mark_price_candles(
        &self,
        range: &LighterCandleRange<'_>,
        connect_opens: Option<bool>,
    ) -> Result<LighterMarkPriceCandles, BoxError>;

    async fn get_market_price_charts(
        &self,
        market_ids: Option<&[u32]>,
    ) -> Result<LighterMarketPriceCharts, BoxError>;

    /// `resolution`: `1h` or `1d`. Timestamps in Unix ms.
    async fn get_fundings(
        &self,
        market_id: u32,
        resolution: &str,
        start_timestamp: i64,
        end_timestamp: i64,
        count_back: i64,
    ) -> Result<LighterFundings, BoxError>;

    async fn get_funding_rates(&self) -> Result<Vec<LighterFundingRate>, BoxError>;

    async fn get_exchange_stats(&self) -> Result<LighterExchangeStats, BoxError>;

    async fn get_asset_details(&self, asset_id: Option<u32>)
        -> Result<Vec<LighterAsset>, BoxError>;

    async fn get_price_oracle_info(
        &self,
        symbol: Option<&str>,
    ) -> Result<LighterPriceOracleInfo, BoxError>;

    async fn get_fee_tiers(&self, filter: Option<&str>) -> Result<LighterFeeTiers, BoxError>;

    async fn get_events(&self, query: &LighterEventsQuery<'_>) -> Result<LighterEvents, BoxError>;

    /// Timestamps in Unix seconds.
    async fn get_up_down_candles(
        &self,
        symbol: &str,
        duration: &str,
        start_timestamp: i64,
        end_timestamp: i64,
        resolution: Option<&str>,
        limit: Option<u32>,
    ) -> Result<LighterUpDownCandles, BoxError>;
}

#[async_trait]
impl LighterApi for LighterClient {
    async fn get_status(&self) -> Result<LighterStatus, BoxError> {
        self.market_api.get_status().await
    }

    async fn get_markets(&self) -> Result<Vec<LighterMarket>, BoxError> {
        self.market_api.get_markets().await
    }

    async fn get_market_details(
        &self,
        market_id: Option<u32>,
        filter: Option<&str>,
    ) -> Result<LighterOrderBookDetails, BoxError> {
        self.market_api.get_market_details(market_id, filter).await
    }

    async fn get_orderbook(
        &self,
        market_id: u32,
        limit: u32,
    ) -> Result<LighterOrderBook, BoxError> {
        self.market_api.get_orderbook(market_id, limit).await
    }

    async fn get_recent_trades(
        &self,
        market_id: u32,
        limit: u32,
    ) -> Result<Vec<LighterTrade>, BoxError> {
        self.market_api.get_recent_trades(market_id, limit).await
    }

    async fn get_trades(
        &self,
        query: &LighterTradesQuery<'_>,
    ) -> Result<LighterTradesResponse, BoxError> {
        self.market_api.get_trades(query).await
    }

    async fn get_candles(
        &self,
        range: &LighterCandleRange<'_>,
        set_timestamp_to_end: Option<bool>,
        merge_mark_price: Option<bool>,
    ) -> Result<LighterCandles, BoxError> {
        self.market_api
            .get_candles(range, set_timestamp_to_end, merge_mark_price)
            .await
    }

    async fn get_mark_price_candles(
        &self,
        range: &LighterCandleRange<'_>,
        connect_opens: Option<bool>,
    ) -> Result<LighterMarkPriceCandles, BoxError> {
        self.market_api
            .get_mark_price_candles(range, connect_opens)
            .await
    }

    async fn get_market_price_charts(
        &self,
        market_ids: Option<&[u32]>,
    ) -> Result<LighterMarketPriceCharts, BoxError> {
        self.market_api.get_market_price_charts(market_ids).await
    }

    async fn get_fundings(
        &self,
        market_id: u32,
        resolution: &str,
        start_timestamp: i64,
        end_timestamp: i64,
        count_back: i64,
    ) -> Result<LighterFundings, BoxError> {
        self.market_api
            .get_fundings(
                market_id,
                resolution,
                start_timestamp,
                end_timestamp,
                count_back,
            )
            .await
    }

    async fn get_funding_rates(&self) -> Result<Vec<LighterFundingRate>, BoxError> {
        self.market_api.get_funding_rates().await
    }

    async fn get_exchange_stats(&self) -> Result<LighterExchangeStats, BoxError> {
        self.market_api.get_exchange_stats().await
    }

    async fn get_asset_details(
        &self,
        asset_id: Option<u32>,
    ) -> Result<Vec<LighterAsset>, BoxError> {
        self.market_api.get_asset_details(asset_id).await
    }

    async fn get_price_oracle_info(
        &self,
        symbol: Option<&str>,
    ) -> Result<LighterPriceOracleInfo, BoxError> {
        self.market_api.get_price_oracle_info(symbol).await
    }

    async fn get_fee_tiers(&self, filter: Option<&str>) -> Result<LighterFeeTiers, BoxError> {
        self.market_api.get_fee_tiers(filter).await
    }

    async fn get_events(&self, query: &LighterEventsQuery<'_>) -> Result<LighterEvents, BoxError> {
        self.market_api.get_events(query).await
    }

    async fn get_up_down_candles(
        &self,
        symbol: &str,
        duration: &str,
        start_timestamp: i64,
        end_timestamp: i64,
        resolution: Option<&str>,
        limit: Option<u32>,
    ) -> Result<LighterUpDownCandles, BoxError> {
        self.market_api
            .get_up_down_candles(
                symbol,
                duration,
                start_timestamp,
                end_timestamp,
                resolution,
                limit,
            )
            .await
    }
}
