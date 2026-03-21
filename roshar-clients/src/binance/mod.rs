pub(crate) mod rest;

use rest::BinanceRestClient;

use anyhow::Result;
use roshar_types::{BinancePremiumIndex, BinanceWssMessage};
use roshar_ws_mgr::{Config, Manager, Message};
use std::collections::HashMap;
use std::sync::Arc;

use crate::BINANCE_WSS_URL;
use crate::ws::ws_config_methods;

pub struct BinanceClient {
    rest_client: BinanceRestClient,
    ws_config: Config,
}

impl BinanceClient {
    const WS_CONN_NAME: &str = "binance";

    pub fn new(requests_per_second: u32) -> Self {
        Self {
            rest_client: BinanceRestClient::new(requests_per_second),
            ws_config: Config {
                name: Self::WS_CONN_NAME.to_string(),
                url: BINANCE_WSS_URL.to_string(),
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
            },
        }
    }

    ws_config_methods!();

    pub fn subscribe_depth(&self, manager: &Arc<Manager>, symbols: &[&str]) -> Result<()> {
        let symbols_owned: Vec<String> = symbols.iter().map(|s| s.to_string()).collect();
        let msg = BinanceWssMessage::batch_depth(&symbols_owned).to_json();
        manager.write(
            &self.ws_config.name,
            Message::TextMessage(self.ws_config.name.clone(), msg),
        )?;
        Ok(())
    }

    pub fn subscribe_trades(&self, manager: &Arc<Manager>, symbols: &[&str]) -> Result<()> {
        let symbols_owned: Vec<String> = symbols.iter().map(|s| s.to_string()).collect();
        let msg = BinanceWssMessage::batch_trades(&symbols_owned).to_json();
        manager.write(
            &self.ws_config.name,
            Message::TextMessage(self.ws_config.name.clone(), msg),
        )?;
        Ok(())
    }

    pub fn subscribe_candles(&self, manager: &Arc<Manager>, symbols: &[&str]) -> Result<()> {
        let symbols_owned: Vec<String> = symbols.iter().map(|s| s.to_string()).collect();
        let msg = BinanceWssMessage::batch_candles(&symbols_owned).to_json();
        manager.write(
            &self.ws_config.name,
            Message::TextMessage(self.ws_config.name.clone(), msg),
        )?;
        Ok(())
    }

    pub async fn get_24hr_ticker(
        &self,
        symbol: Option<&str>,
    ) -> Result<Vec<roshar_types::TickerData>, String> {
        self.rest_client
            .get_24hr_ticker(symbol)
            .await
            .map_err(|e| format!("Failed to get 24hr ticker: {}", e))
    }

    pub async fn get_historical_funding_rates(
        &self,
        symbol: &str,
        start_time: i64,
        end_time: i64,
    ) -> Result<Vec<roshar_types::BinanceHistoricalFundingRate>, String> {
        self.rest_client
            .get_historical_funding_rates(symbol, start_time, end_time)
            .await
            .map_err(|e| format!("Failed to get historical funding rates: {}", e))
    }

    pub async fn get_exchange_info(&self) -> Result<roshar_types::ExchangeInfo, String> {
        self.rest_client
            .get_exchange_info()
            .await
            .map_err(|e| format!("Failed to get exchange info: {}", e))
    }

    pub async fn get_agg_trades(
        &self,
        symbol: &str,
        start_time: Option<i64>,
        end_time: Option<i64>,
        from_id: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<roshar_types::BinanceAggTrade>, String> {
        self.rest_client
            .get_agg_trades(symbol, start_time, end_time, from_id, limit)
            .await
            .map_err(|e| format!("Failed to get aggregate trades: {}", e))
    }

    pub async fn get_klines(
        &self,
        symbol: &str,
        interval: &str,
        start_time: Option<i64>,
        end_time: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<roshar_types::BinanceKline>, String> {
        self.rest_client
            .get_klines(symbol, interval, start_time, end_time, limit)
            .await
            .map_err(|e| format!("Failed to get klines: {}", e))
    }

    pub async fn get_realtime_funding_rates(
        &self,
    ) -> Result<HashMap<String, BinancePremiumIndex>, String> {
        let premium_index_list = self
            .rest_client
            .get_premium_index()
            .await
            .map_err(|e| format!("Failed to get premium index: {}", e))?;

        let mut funding_rates = HashMap::new();
        for premium_index in premium_index_list {
            funding_rates.insert(premium_index.symbol.clone(), premium_index);
        }

        Ok(funding_rates)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_realtime_funding_rates() {
        let client = BinanceClient::new(10);

        let result = client.get_realtime_funding_rates().await;

        assert!(
            result.is_ok(),
            "Failed to fetch realtime funding rates: {:?}",
            result.err()
        );

        let funding_rates = result.unwrap();

        assert!(
            !funding_rates.is_empty(),
            "Expected funding rates data, got empty HashMap"
        );

        assert!(
            funding_rates.contains_key("BTCUSDT"),
            "Expected BTCUSDT in funding rates"
        );

        if let Some(btc_data) = funding_rates.get("BTCUSDT") {
            assert_eq!(btc_data.symbol, "BTCUSDT");
            btc_data
                .mark_price
                .parse::<f64>()
                .expect("mark_price should be a valid number");
            btc_data
                .last_funding_rate
                .parse::<f64>()
                .expect("last_funding_rate should be a valid number");
            assert!(
                btc_data.next_funding_time > 0,
                "next_funding_time should be positive"
            );
        }

        println!(
            "Fetched {} funding rates (lookup working)",
            funding_rates.len()
        );
    }
}
