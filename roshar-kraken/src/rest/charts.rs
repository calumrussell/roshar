use roshar_http::RateLimitedClient;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

const CHARTS_URL: &str = "https://futures.kraken.com/api/charts/v1";

/// Price series of the charts endpoint (`GET /api/charts/v1/` lists them).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KrakenTickType {
    Trade,
    Mark,
    Spot,
}

impl KrakenTickType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Trade => "trade",
            Self::Mark => "mark",
            Self::Spot => "spot",
        }
    }
}

/// Candle resolutions of the charts endpoint (`GET /api/charts/v1/{tick_type}/{symbol}` lists
/// them).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KrakenResolution {
    M1,
    M5,
    M15,
    M30,
    H1,
    H4,
    H12,
    D1,
    W1,
}

impl KrakenResolution {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::M1 => "1m",
            Self::M5 => "5m",
            Self::M15 => "15m",
            Self::M30 => "30m",
            Self::H1 => "1h",
            Self::H4 => "4h",
            Self::H12 => "12h",
            Self::D1 => "1d",
            Self::W1 => "1w",
        }
    }
}

/// One candle as sent by the charts endpoint. `time` is the open time in milliseconds.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct KrakenRestCandleData {
    pub time: i64,
    pub open: String,
    pub high: String,
    pub low: String,
    pub close: String,
    pub volume: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct KrakenRestCandleResponse {
    pub candles: Vec<KrakenRestCandleData>,
    /// True when the range held more candles than one response returns.
    pub more_candles: bool,
}

pub struct ChartsApi {
    client: Arc<RateLimitedClient>,
}

impl ChartsApi {
    pub fn new_with_client(client: Arc<RateLimitedClient>) -> Self {
        Self { client }
    }

    /// `GET /api/charts/v1/{tick_type}/{symbol}/{resolution}?from=&to=`.
    ///
    /// `from` and `to` are in seconds; `None` leaves the range to the exchange. The candles are
    /// returned as sent, including the one still forming.
    pub async fn get_candles(
        &self,
        tick_type: KrakenTickType,
        symbol: &str,
        resolution: KrakenResolution,
        from: Option<i64>,
        to: Option<i64>,
    ) -> Result<KrakenRestCandleResponse, Box<dyn std::error::Error + Send + Sync>> {
        let mut url = format!(
            "{CHARTS_URL}/{}/{symbol}/{}",
            tick_type.as_str(),
            resolution.as_str()
        );
        let mut sep = '?';
        if let Some(from) = from {
            url.push_str(&format!("{sep}from={from}"));
            sep = '&';
        }
        if let Some(to) = to {
            url.push_str(&format!("{sep}to={to}"));
        }

        let response = self.client.get(&url).await?;
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("Kraken charts request failed ({status}): {body}").into());
        }

        let text = response.text().await?;
        let candles: KrakenRestCandleResponse = serde_json::from_str(&text)
            .map_err(|e| format!("Failed to parse Kraken charts response: {e}"))?;
        Ok(candles)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_wire_response() {
        let text = r#"{"candles":[{"time":1794747060000,"open":"65385","high":"65410","low":"65380","close":"65403","volume":"0.10440000"}],"more_candles":false}"#;
        let r: KrakenRestCandleResponse = serde_json::from_str(text).unwrap();
        assert_eq!(r.candles.len(), 1);
        assert_eq!(r.candles[0].time, 1794747060000);
        assert_eq!(r.candles[0].volume, "0.10440000");
        assert!(!r.more_candles);
    }

    #[tokio::test]
    async fn test_get_candles() {
        let api = ChartsApi::new_with_client(Arc::new(RateLimitedClient::new(10, 1)));
        let to = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let r = api
            .get_candles(
                KrakenTickType::Trade,
                "PF_XBTUSD",
                KrakenResolution::H1,
                Some(to - 6 * 3600),
                Some(to),
            )
            .await
            .expect("candles");
        assert!(!r.candles.is_empty());
    }
}
