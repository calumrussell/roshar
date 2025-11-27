#![allow(clippy::too_many_arguments)]

use charts::ChartsApi;
use data::WssApi;
use market::MarketApi;
use multi_collateral::MultiCollateralApi;
use order_management::OrderManagementApi;
use reqwest::Client;

mod auth;
mod charts;
mod data;
mod market;
mod multi_collateral;
mod order_management;

pub use data::{
    KrakenBookDeltaMessage, KrakenBookLevel, KrakenBookSnapshotMessage, KrakenOrderBook,
    KrakenSubscribeMessage, KrakenTradeDeltaMessage, KrakenTradeSnapshotMessage,
};
pub use market::{
    KrakenRestCandleData, KrakenRestCandleResponse, KrakenTickerData, KrakenTickerResponse,
};
pub use multi_collateral::{KrakenGetLeverageResponse, KrakenLeverageSettingResponse};
pub use order_management::{KrakenModifyResponse, KrakenOrderResponse};

use crate::exchanges::kraken::order_management::{
    KrakenCancelResponse, KrakenDeadMansSwitchResponse, KrakenOpenOrdersResponse,
    KrakenOrderStatusResponse,
};

const TEST_REST_URL: &str = "https://demo-futures.kraken.com";
const PROD_REST_URL: &str = "https://futures.kraken.com";

pub struct Kraken {
    base_url: String,
    http_client: Client,
}

impl Kraken {
    pub fn new(is_prod: bool) -> Self {
        let base_url = if is_prod {
            PROD_REST_URL
        } else {
            TEST_REST_URL
        };

        let http_client = Client::new();

        Self {
            base_url: base_url.to_string(),
            http_client,
        }
    }

    pub async fn get_all_funding_rates_with_size()
    -> Result<Vec<(String, f64, f64, f64)>, Box<dyn std::error::Error>> {
        MarketApi::get_all_funding_rates_with_size().await
    }

    pub async fn get_tickers()
    -> Result<std::collections::HashMap<String, KrakenTickerData>, Box<dyn std::error::Error>> {
        MarketApi::get_tickers().await
    }

    pub async fn fetch_candle(
        symbol: &str,
    ) -> Result<Vec<crate::common::Candle>, Box<dyn std::error::Error + Send + Sync>> {
        ChartsApi::fetch_candle(symbol).await
    }

    pub fn ping() -> String {
        WssApi::ping()
    }

    pub fn depth(coin: &str) -> String {
        WssApi::depth(coin)
    }

    pub fn trades(coin: &str) -> String {
        WssApi::trades(coin)
    }

    pub fn depth_unsub(coin: &str) -> String {
        WssApi::depth_unsub(coin)
    }

    pub fn trades_unsub(coin: &str) -> String {
        WssApi::trades_unsub(coin)
    }

    pub async fn create_order(
        &self,
        symbol: &str,
        side: &str,
        order_type: &str,
        size: &str,
        limit_price: &str,
        cli_ord_id: Option<&str>,
        stop_price: Option<String>,
        reduce_only: Option<bool>,
        time_in_force: Option<&str>,
        post_only: Option<bool>,
    ) -> Result<KrakenOrderResponse, Box<dyn std::error::Error>> {
        OrderManagementApi::create_order(
            &self.http_client,
            &self.base_url,
            symbol,
            side,
            order_type,
            size,
            limit_price,
            cli_ord_id,
            stop_price,
            reduce_only,
            time_in_force,
            post_only,
        )
        .await
    }

    pub async fn cancel_order(
        &self,
        order_id: Option<&str>,
        cli_ord_id: Option<&str>,
    ) -> Result<KrakenCancelResponse, Box<dyn std::error::Error>> {
        OrderManagementApi::cancel_order(&self.http_client, &self.base_url, order_id, cli_ord_id)
            .await
    }

    pub async fn modify_order(
        &self,
        order_id: &str,
        size: Option<f64>,
        limit_price: Option<f64>,
        stop_price: Option<f64>,
    ) -> Result<KrakenModifyResponse, Box<dyn std::error::Error>> {
        OrderManagementApi::modify_order(
            &self.http_client,
            &self.base_url,
            order_id,
            size,
            limit_price,
            stop_price,
        )
        .await
    }

    pub async fn get_open_orders(
        &self,
    ) -> Result<KrakenOpenOrdersResponse, Box<dyn std::error::Error>> {
        OrderManagementApi::get_open_orders(&self.http_client, &self.base_url).await
    }

    pub async fn get_order_status(
        &self,
        order_ids: Option<Vec<String>>,
        cli_ord_ids: Option<Vec<String>>,
    ) -> Result<KrakenOrderStatusResponse, Box<dyn std::error::Error>> {
        OrderManagementApi::get_order_status(
            &self.http_client,
            &self.base_url,
            order_ids,
            cli_ord_ids,
        )
        .await
    }

    pub async fn dead_mans_switch(
        &self,
        timeout: u32,
    ) -> Result<KrakenDeadMansSwitchResponse, Box<dyn std::error::Error>> {
        OrderManagementApi::dead_mans_switch(&self.http_client, &self.base_url, timeout).await
    }

    pub async fn get_leverage(
        &self,
    ) -> Result<KrakenGetLeverageResponse, Box<dyn std::error::Error>> {
        MultiCollateralApi::get_leverage(&self.http_client, &self.base_url).await
    }

    pub async fn set_leverage(
        &self,
        symbol: &str,
        margin_type: &str,
    ) -> Result<KrakenLeverageSettingResponse, Box<dyn std::error::Error>> {
        MultiCollateralApi::set_leverage(&self.http_client, &self.base_url, symbol, margin_type)
            .await
    }
}
