use data::WssApi;
use market::MarketApi;
use order_management::OrderManagementApi;
use reqwest::Client;

mod auth;
mod data;
mod market;
mod order_management;

pub use data::{
    ByBitCandleMessage, ByBitDepthBookData, ByBitDepthMessage, ByBitMessage, ByBitTradesMessage,
    BybitOrderBook,
};
pub use market::{ByBitTickerData, ByBitTickersResponse};
pub use order_management::{ByBitCreateOrderRequest, ByBitCreateOrderResponse};

const BYBIT_REST_URL: &str = "https://api.bybit.com";

pub struct ByBit {
    http_client: Client,
}

impl Default for ByBit {
    fn default() -> Self {
        Self::new()
    }
}

impl ByBit {
    pub fn new() -> Self {
        Self {
            http_client: Client::new(),
        }
    }

    pub fn ping() -> String {
        WssApi::ping()
    }

    pub fn depth(coin: &str) -> String {
        WssApi::depth(coin)
    }

    pub fn depth_unsub(coin: &str) -> String {
        WssApi::depth_unsub(coin)
    }

    pub fn trades(coin: &str) -> String {
        WssApi::trades(coin)
    }

    pub fn trades_unsub(coin: &str) -> String {
        WssApi::trades_unsub(coin)
    }

    pub fn candle(coin: &str) -> String {
        WssApi::candle(coin)
    }

    pub async fn get_tickers(
        &self,
    ) -> Result<
        std::collections::HashMap<String, ByBitTickerData>,
        Box<dyn std::error::Error + Send + Sync>,
    > {
        MarketApi::get_tickers(&self.http_client).await
    }

    pub async fn create_order(
        &self,
        request: &ByBitCreateOrderRequest,
    ) -> Result<ByBitCreateOrderResponse, Box<dyn std::error::Error + Send + Sync>> {
        OrderManagementApi::create_order(&self.http_client, request).await
    }
}
