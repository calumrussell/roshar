use chrono::DateTime;
use roshar_types::Candle;

use super::market::KrakenRestCandleResponse;

pub struct ChartsApi;

impl ChartsApi {
    pub async fn fetch_candle(
        symbol: &str,
    ) -> Result<Vec<Candle>, Box<dyn std::error::Error + Send + Sync>> {
        let client = crate::http::get_http_client();

        let url = format!("https://futures.kraken.com/api/charts/v1/trade/{symbol}/1m");

        let response = client.get(&url).send().await?;
        let candle_response: KrakenRestCandleResponse = response.json().await?;
        Ok(Self::process_candle_response(candle_response, symbol))
    }

    fn process_candle_response(
        candle_response: KrakenRestCandleResponse,
        symbol: &str,
    ) -> Vec<Candle> {
        if candle_response.candles.is_empty() {
            return Vec::new();
        }

        let now = chrono::Utc::now().timestamp_millis();
        let current_minute_start = (now / 60_000) * 60_000;

        let most_recent_completed = candle_response
            .candles
            .iter()
            .filter(|kraken_candle| kraken_candle.time < current_minute_start)
            .max_by_key(|kraken_candle| kraken_candle.time);

        if let Some(kraken_candle) = most_recent_completed {
            let candle = Candle {
                open: kraken_candle.open.clone(),
                high: kraken_candle.high.clone(),
                low: kraken_candle.low.clone(),
                close: kraken_candle.close.clone(),
                volume: kraken_candle.volume.clone(),
                exchange: "kraken".to_string(),
                time: DateTime::from_timestamp_millis(kraken_candle.time).unwrap_or_default(),
                close_time: DateTime::from_timestamp_millis(kraken_candle.time + 60_000)
                    .unwrap_or_default(),
                coin: symbol.to_string(),
            };
            vec![candle]
        } else {
            Vec::new()
        }
    }
}
