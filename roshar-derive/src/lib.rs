// Endpoints (v3): https://docs.derive.xyz/getting-started/introduction
pub const DERIVE_WSS_URL: &str = "wss://api.derive.xyz/v3/ws";

pub(crate) mod rest;

use rest::MarketApi;

pub(crate) mod ws;

pub use rest::{
    DeriveAllInstrumentsResponse, DeriveAssetEntry, DeriveAssetErc20, DeriveAssetUniverse,
    DeriveCurrency, DeriveCurrencyManagers, DeriveErc20Details, DeriveForwardFeedData,
    DeriveFundingFeedData, DeriveFundingRate, DeriveFundingRateHistoryResponse, DeriveIncident,
    DeriveIndexCandle, DeriveInstrument, DeriveInterestRateCandle, DeriveLendingDetails,
    DeriveLiquidationAuction, DeriveLiquidationBid, DeriveLiquidationHistoryResponse,
    DeriveLiveAuction, DeriveManagerCollateral, DeriveOhlc, DeriveOpenInterestStats,
    DeriveOptionDetails, DeriveOptionPricing, DeriveOptionSettlement, DeriveOracleSignatures,
    DerivePagination, DerivePerpDetails, DerivePerpFeedData, DerivePerpImpactTwap,
    DeriveRateFeedData, DeriveRiskUniverse, DeriveRiskUniverseManager, DeriveSecurityModule,
    DeriveSignedFeeds, DeriveSpotAsset, DeriveSpotFeedData, DeriveSpotUniverse,
    DeriveSrmPerpMarginRequirements, DeriveStats, DeriveSviParams, DeriveTicker,
    DeriveTickersResponse, DeriveTrade, DeriveTradeCandle, DeriveTradeHistoryParams,
    DeriveTradeHistoryResponse, DeriveVolFeedData,
};
pub use ws::{
    DeriveWsAuction, DeriveWsAuctionDetails, DeriveWsError, DeriveWsMargin, DeriveWsMessage,
    DeriveWsOrderbook, DeriveWsSpotFeed, DeriveWsSpotFeedEntry, DeriveWsSubscriptionResult,
    DeriveWsTickerSlim,
};

use anyhow::Result;
use async_trait::async_trait;
use roshar_ws_mgr::{Config, Manager, Message};
use std::collections::HashMap;
use std::sync::Arc;

pub struct DeriveClient {
    market_api: MarketApi,
    ws_config: Config,
}

impl Default for DeriveClient {
    fn default() -> Self {
        Self::new()
    }
}

impl DeriveClient {
    /// Public (unauthenticated) request budget per IP, mainnet reference:
    /// 5 TPS. https://docs.derive.xyz/rate-limits
    const DEFAULT_REQUESTS_PER_SECOND: u32 = 5;
    const WS_CONN_NAME: &str = "derive";

    pub fn new() -> Self {
        Self::new_with_rate_limit(Self::DEFAULT_REQUESTS_PER_SECOND)
    }

    pub fn new_with_rate_limit(requests_per_second: u32) -> Self {
        Self {
            market_api: MarketApi::new(requests_per_second),
            // The server closes connections idle for ~180s and answers
            // protocol-level Ping frames with Pong (https://docs.derive.xyz/connecting).
            // Frame pings let roshar-ws-mgr detect a dead socket from the Pong.
            ws_config: Config {
                name: Self::WS_CONN_NAME.to_string(),
                url: DERIVE_WSS_URL.to_string(),
                ping_duration: 30,
                ping_message: "ping".to_string(),
                ping_timeout: 60,
                reconnect_timeout: 5000,
                use_text_ping: None,
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

    fn make_subscribe_msg(channels: &[String]) -> String {
        serde_json::json!({
            "method": "subscribe",
            "params": { "channels": channels },
            "id": 1
        })
        .to_string()
    }

    fn make_unsubscribe_msg(channels: &[String]) -> String {
        serde_json::json!({
            "method": "unsubscribe",
            "params": { "channels": channels },
            "id": 1
        })
        .to_string()
    }

    pub fn subscribe_trades(&self, manager: &Arc<Manager>, instruments: &[&str]) -> Result<()> {
        let channels: Vec<String> = instruments.iter().map(|i| format!("trades.{i}")).collect();
        let msg = Self::make_subscribe_msg(&channels);
        manager.write(
            &self.ws_config.name,
            Message::TextMessage(self.ws_config.name.clone(), msg),
        )?;
        Ok(())
    }

    pub fn unsubscribe_trades(&self, manager: &Arc<Manager>, instruments: &[&str]) -> Result<()> {
        let channels: Vec<String> = instruments.iter().map(|i| format!("trades.{i}")).collect();
        let msg = Self::make_unsubscribe_msg(&channels);
        manager.write(
            &self.ws_config.name,
            Message::TextMessage(self.ws_config.name.clone(), msg),
        )?;
        Ok(())
    }

    /// Subscribe to `orderbook.{instrument}.{group}.{depth}` snapshots.
    ///
    /// * `group` - Price grouping: "1", "10", "100"
    /// * `depth` - Number of levels per side: "1", "10", "20", "100"
    pub fn subscribe_depth(
        &self,
        manager: &Arc<Manager>,
        instruments: &[&str],
        group: &str,
        depth: &str,
    ) -> Result<()> {
        let channels: Vec<String> = instruments
            .iter()
            .map(|i| format!("orderbook.{i}.{group}.{depth}"))
            .collect();
        let msg = Self::make_subscribe_msg(&channels);
        manager.write(
            &self.ws_config.name,
            Message::TextMessage(self.ws_config.name.clone(), msg),
        )?;
        Ok(())
    }

    pub fn unsubscribe_depth(
        &self,
        manager: &Arc<Manager>,
        instruments: &[&str],
        group: &str,
        depth: &str,
    ) -> Result<()> {
        let channels: Vec<String> = instruments
            .iter()
            .map(|i| format!("orderbook.{i}.{group}.{depth}"))
            .collect();
        let msg = Self::make_unsubscribe_msg(&channels);
        manager.write(
            &self.ws_config.name,
            Message::TextMessage(self.ws_config.name.clone(), msg),
        )?;
        Ok(())
    }

    /// Subscribe to `ticker_slim.{instrument}.{interval}` (the full `ticker`
    /// channel was removed in v3 and is rejected).
    ///
    /// * `interval` - Publish interval in ms: "100", "1000"
    pub fn subscribe_tickers(
        &self,
        manager: &Arc<Manager>,
        instruments: &[&str],
        interval: &str,
    ) -> Result<()> {
        let channels: Vec<String> = instruments
            .iter()
            .map(|i| format!("ticker_slim.{i}.{interval}"))
            .collect();
        let msg = Self::make_subscribe_msg(&channels);
        manager.write(
            &self.ws_config.name,
            Message::TextMessage(self.ws_config.name.clone(), msg),
        )?;
        Ok(())
    }

    pub fn unsubscribe_tickers(
        &self,
        manager: &Arc<Manager>,
        instruments: &[&str],
        interval: &str,
    ) -> Result<()> {
        let channels: Vec<String> = instruments
            .iter()
            .map(|i| format!("ticker_slim.{i}.{interval}"))
            .collect();
        let msg = Self::make_unsubscribe_msg(&channels);
        manager.write(
            &self.ws_config.name,
            Message::TextMessage(self.ws_config.name.clone(), msg),
        )?;
        Ok(())
    }

    fn send_channels(
        &self,
        manager: &Arc<Manager>,
        channels: &[String],
        subscribe: bool,
    ) -> Result<()> {
        let msg = if subscribe {
            Self::make_subscribe_msg(channels)
        } else {
            Self::make_unsubscribe_msg(channels)
        };
        manager.write(
            &self.ws_config.name,
            Message::TextMessage(self.ws_config.name.clone(), msg),
        )?;
        Ok(())
    }

    /// `trades.{instrument_type}.{currency}`.
    ///
    /// * `instrument_type` - "erc20", "option" or "perp"
    pub fn subscribe_trades_by_type(
        &self,
        manager: &Arc<Manager>,
        instrument_type: &str,
        currencies: &[&str],
    ) -> Result<()> {
        let channels: Vec<String> = currencies
            .iter()
            .map(|c| format!("trades.{instrument_type}.{c}"))
            .collect();
        self.send_channels(manager, &channels, true)
    }

    pub fn unsubscribe_trades_by_type(
        &self,
        manager: &Arc<Manager>,
        instrument_type: &str,
        currencies: &[&str],
    ) -> Result<()> {
        let channels: Vec<String> = currencies
            .iter()
            .map(|c| format!("trades.{instrument_type}.{c}"))
            .collect();
        self.send_channels(manager, &channels, false)
    }

    /// `spot_feed.{currency}` (the exchange also accepts `ALL`).
    pub fn subscribe_spot_feed(&self, manager: &Arc<Manager>, currencies: &[&str]) -> Result<()> {
        let channels: Vec<String> = currencies
            .iter()
            .map(|c| format!("spot_feed.{c}"))
            .collect();
        self.send_channels(manager, &channels, true)
    }

    pub fn unsubscribe_spot_feed(&self, manager: &Arc<Manager>, currencies: &[&str]) -> Result<()> {
        let channels: Vec<String> = currencies
            .iter()
            .map(|c| format!("spot_feed.{c}"))
            .collect();
        self.send_channels(manager, &channels, false)
    }

    /// `auctions.watch`: live liquidation auctions.
    pub fn subscribe_auctions_watch(&self, manager: &Arc<Manager>) -> Result<()> {
        self.send_channels(manager, &["auctions.watch".to_string()], true)
    }

    pub fn unsubscribe_auctions_watch(&self, manager: &Arc<Manager>) -> Result<()> {
        self.send_channels(manager, &["auctions.watch".to_string()], false)
    }

    /// `margin.watch`: margin state of all accounts.
    pub fn subscribe_margin_watch(&self, manager: &Arc<Manager>) -> Result<()> {
        self.send_channels(manager, &["margin.watch".to_string()], true)
    }

    pub fn unsubscribe_margin_watch(&self, manager: &Arc<Manager>) -> Result<()> {
        self.send_channels(manager, &["margin.watch".to_string()], false)
    }
}

#[async_trait]
pub trait DeriveApi {
    async fn get_all_instruments(
        &self,
        instrument_type: &str,
        currency: Option<&str>,
        expired: bool,
        risk_universe_id: Option<u32>,
        page: Option<u64>,
        page_size: Option<u64>,
    ) -> Result<DeriveAllInstrumentsResponse, Box<dyn std::error::Error + Send + Sync>>;

    async fn get_funding_rate_history(
        &self,
        instrument_name: &str,
        start_timestamp: Option<i64>,
        end_timestamp: Option<i64>,
        period: Option<i64>,
    ) -> Result<Vec<DeriveFundingRate>, Box<dyn std::error::Error + Send + Sync>>;

    async fn get_tickers(
        &self,
        instrument_type: &str,
        currency: Option<&str>,
        expiry_date: Option<i64>,
    ) -> Result<HashMap<String, DeriveTicker>, Box<dyn std::error::Error + Send + Sync>>;

    async fn get_instrument(
        &self,
        instrument_name: &str,
    ) -> Result<DeriveInstrument, Box<dyn std::error::Error + Send + Sync>>;

    async fn get_all_live_instruments(
        &self,
    ) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>>;

    async fn get_ticker(
        &self,
        instrument_name: &str,
    ) -> Result<DeriveTicker, Box<dyn std::error::Error + Send + Sync>>;

    async fn get_trade_history(
        &self,
        params: &DeriveTradeHistoryParams<'_>,
    ) -> Result<DeriveTradeHistoryResponse, Box<dyn std::error::Error + Send + Sync>>;

    async fn get_all_currencies(
        &self,
    ) -> Result<Vec<DeriveCurrency>, Box<dyn std::error::Error + Send + Sync>>;

    async fn get_currency(
        &self,
        currency: &str,
    ) -> Result<DeriveCurrency, Box<dyn std::error::Error + Send + Sync>>;

    async fn get_risk_universes(
        &self,
    ) -> Result<Vec<DeriveRiskUniverse>, Box<dyn std::error::Error + Send + Sync>>;

    async fn get_index_chart_data(
        &self,
        currency: &str,
        start_timestamp: i64,
        end_timestamp: i64,
        period: i64,
    ) -> Result<Vec<DeriveIndexCandle>, Box<dyn std::error::Error + Send + Sync>>;

    async fn get_tradingview_chart_data(
        &self,
        instrument_name: &str,
        start_timestamp: i64,
        end_timestamp: i64,
        period: i64,
    ) -> Result<Vec<DeriveTradeCandle>, Box<dyn std::error::Error + Send + Sync>>;

    async fn get_interest_rate_history(
        &self,
        currency: &str,
        start_timestamp: Option<i64>,
        end_timestamp: Option<i64>,
        period: Option<i64>,
        risk_universe_id: Option<u32>,
    ) -> Result<Vec<DeriveInterestRateCandle>, Box<dyn std::error::Error + Send + Sync>>;

    async fn get_option_settlement_prices(
        &self,
        currency: &str,
    ) -> Result<Vec<DeriveOptionSettlement>, Box<dyn std::error::Error + Send + Sync>>;

    async fn get_perp_impact_twap(
        &self,
        currency: &str,
        start_time: i64,
        end_time: i64,
    ) -> Result<DerivePerpImpactTwap, Box<dyn std::error::Error + Send + Sync>>;

    async fn get_time(&self) -> Result<i64, Box<dyn std::error::Error + Send + Sync>>;

    async fn get_liquidation_history(
        &self,
        start_timestamp: Option<i64>,
        end_timestamp: Option<i64>,
        subaccount_id: Option<u64>,
        page: Option<u64>,
        page_size: Option<u64>,
    ) -> Result<DeriveLiquidationHistoryResponse, Box<dyn std::error::Error + Send + Sync>>;

    async fn get_live_auctions(
        &self,
    ) -> Result<Vec<DeriveLiveAuction>, Box<dyn std::error::Error + Send + Sync>>;

    async fn get_latest_signed_feeds(
        &self,
        currency: Option<&str>,
        expiry: Option<i64>,
    ) -> Result<DeriveSignedFeeds, Box<dyn std::error::Error + Send + Sync>>;

    async fn get_live_incidents(
        &self,
    ) -> Result<Vec<DeriveIncident>, Box<dyn std::error::Error + Send + Sync>>;
}

#[async_trait]
impl DeriveApi for DeriveClient {
    async fn get_all_instruments(
        &self,
        instrument_type: &str,
        currency: Option<&str>,
        expired: bool,
        risk_universe_id: Option<u32>,
        page: Option<u64>,
        page_size: Option<u64>,
    ) -> Result<DeriveAllInstrumentsResponse, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api
            .get_all_instruments(
                instrument_type,
                currency,
                expired,
                risk_universe_id,
                page,
                page_size,
            )
            .await
    }

    async fn get_funding_rate_history(
        &self,
        instrument_name: &str,
        start_timestamp: Option<i64>,
        end_timestamp: Option<i64>,
        period: Option<i64>,
    ) -> Result<Vec<DeriveFundingRate>, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api
            .get_funding_rate_history(instrument_name, start_timestamp, end_timestamp, period)
            .await
    }

    async fn get_tickers(
        &self,
        instrument_type: &str,
        currency: Option<&str>,
        expiry_date: Option<i64>,
    ) -> Result<HashMap<String, DeriveTicker>, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api
            .get_tickers(instrument_type, currency, expiry_date)
            .await
    }

    async fn get_instrument(
        &self,
        instrument_name: &str,
    ) -> Result<DeriveInstrument, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api.get_instrument(instrument_name).await
    }

    async fn get_all_live_instruments(
        &self,
    ) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api.get_all_live_instruments().await
    }

    async fn get_ticker(
        &self,
        instrument_name: &str,
    ) -> Result<DeriveTicker, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api.get_ticker(instrument_name).await
    }

    async fn get_trade_history(
        &self,
        params: &DeriveTradeHistoryParams<'_>,
    ) -> Result<DeriveTradeHistoryResponse, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api.get_trade_history(params).await
    }

    async fn get_all_currencies(
        &self,
    ) -> Result<Vec<DeriveCurrency>, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api.get_all_currencies().await
    }

    async fn get_currency(
        &self,
        currency: &str,
    ) -> Result<DeriveCurrency, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api.get_currency(currency).await
    }

    async fn get_risk_universes(
        &self,
    ) -> Result<Vec<DeriveRiskUniverse>, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api.get_risk_universes().await
    }

    async fn get_index_chart_data(
        &self,
        currency: &str,
        start_timestamp: i64,
        end_timestamp: i64,
        period: i64,
    ) -> Result<Vec<DeriveIndexCandle>, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api
            .get_index_chart_data(currency, start_timestamp, end_timestamp, period)
            .await
    }

    async fn get_tradingview_chart_data(
        &self,
        instrument_name: &str,
        start_timestamp: i64,
        end_timestamp: i64,
        period: i64,
    ) -> Result<Vec<DeriveTradeCandle>, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api
            .get_tradingview_chart_data(instrument_name, start_timestamp, end_timestamp, period)
            .await
    }

    async fn get_interest_rate_history(
        &self,
        currency: &str,
        start_timestamp: Option<i64>,
        end_timestamp: Option<i64>,
        period: Option<i64>,
        risk_universe_id: Option<u32>,
    ) -> Result<Vec<DeriveInterestRateCandle>, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api
            .get_interest_rate_history(
                currency,
                start_timestamp,
                end_timestamp,
                period,
                risk_universe_id,
            )
            .await
    }

    async fn get_option_settlement_prices(
        &self,
        currency: &str,
    ) -> Result<Vec<DeriveOptionSettlement>, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api.get_option_settlement_prices(currency).await
    }

    async fn get_perp_impact_twap(
        &self,
        currency: &str,
        start_time: i64,
        end_time: i64,
    ) -> Result<DerivePerpImpactTwap, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api
            .get_perp_impact_twap(currency, start_time, end_time)
            .await
    }

    async fn get_time(&self) -> Result<i64, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api.get_time().await
    }

    async fn get_liquidation_history(
        &self,
        start_timestamp: Option<i64>,
        end_timestamp: Option<i64>,
        subaccount_id: Option<u64>,
        page: Option<u64>,
        page_size: Option<u64>,
    ) -> Result<DeriveLiquidationHistoryResponse, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api
            .get_liquidation_history(
                start_timestamp,
                end_timestamp,
                subaccount_id,
                page,
                page_size,
            )
            .await
    }

    async fn get_live_auctions(
        &self,
    ) -> Result<Vec<DeriveLiveAuction>, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api.get_live_auctions().await
    }

    async fn get_latest_signed_feeds(
        &self,
        currency: Option<&str>,
        expiry: Option<i64>,
    ) -> Result<DeriveSignedFeeds, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api
            .get_latest_signed_feeds(currency, expiry)
            .await
    }

    async fn get_live_incidents(
        &self,
    ) -> Result<Vec<DeriveIncident>, Box<dyn std::error::Error + Send + Sync>> {
        self.market_api.get_live_incidents().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_derive_client_instruments() {
        let client = DeriveClient::new();
        let result = client
            .get_all_instruments("perp", Some("ETH"), false, None, None, None)
            .await;

        assert!(
            result.is_ok(),
            "Failed to fetch Derive instruments: {:?}",
            result.err()
        );

        let response = result.unwrap();
        assert!(
            !response.instruments.is_empty(),
            "Expected some instruments, got none"
        );
    }
}
