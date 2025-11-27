use data::WssApi;

mod data;
mod market_data;

pub use data::{
    BinanceCandleMessage, BinanceDepthDiffMessage, BinanceKlineData, BinanceOrderBook,
    BinanceOrderBookSnapshot, BinanceTradeMessage,
};
pub use market_data::{BinanceRestClient, ExchangeInfo, OpenInterestData, SymbolInfo, TickerData};

pub struct Binance;

impl Default for Binance {
    fn default() -> Self {
        Self::new()
    }
}

impl Binance {
    pub fn new() -> Self {
        Self
    }

    pub fn ping() -> String {
        WssApi::ping()
    }

    pub fn depth(symbol: &str) -> String {
        WssApi::depth(symbol)
    }

    pub fn depth_unsub(symbol: &str) -> String {
        WssApi::depth_unsub(symbol)
    }

    pub fn trades(symbol: &str) -> String {
        WssApi::trades(symbol)
    }

    pub fn trades_unsub(symbol: &str) -> String {
        WssApi::trades_unsub(symbol)
    }

    pub fn candle(symbol: &str) -> String {
        WssApi::candle(symbol)
    }

    pub fn candle_unsub(symbol: &str) -> String {
        WssApi::candle_unsub(symbol)
    }

    pub fn batch_depth(symbols: &[String]) -> String {
        WssApi::batch_depth(symbols)
    }

    pub fn batch_trades(symbols: &[String]) -> String {
        WssApi::batch_trades(symbols)
    }

    pub async fn get_exchange_info()
    -> Result<ExchangeInfo, Box<dyn std::error::Error + Send + Sync>> {
        let client = market_data::BinanceRestClient::new();
        client.get_exchange_info().await
    }

    pub async fn get_open_interest(
        symbol: &str,
    ) -> Result<OpenInterestData, Box<dyn std::error::Error + Send + Sync>> {
        let client = market_data::BinanceRestClient::new();
        client.get_open_interest(symbol).await
    }

    pub async fn get_24hr_ticker(
        symbol: Option<&str>,
    ) -> Result<Vec<TickerData>, Box<dyn std::error::Error + Send + Sync>> {
        let client = market_data::BinanceRestClient::new();
        client.get_24hr_ticker(symbol).await
    }
}
