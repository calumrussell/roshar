use anyhow::{Context, Result};
use chrono::{DateTime, NaiveDate, Utc};
use clickhouse::Client;
use log::{error, info};
use serde::{Deserialize, Serialize};

/// Deribit API response for volatility index data
#[derive(Debug, Deserialize)]
struct DeribitResponse {
    result: DeribitResult,
}

#[derive(Debug, Deserialize)]
struct DeribitResult {
    /// Array of [timestamp, open, high, low, close] data points
    data: Vec<Vec<f64>>,
    /// Continuation token for pagination (unused but part of API response)
    #[allow(dead_code)]
    continuation: Option<i64>,
}

/// Row structure for ClickHouse insertion
#[derive(Debug, Serialize, clickhouse::Row)]
pub struct DvolRow {
    pub timestamp: i64,
    pub currency: String,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
}

/// Currency options for DVOL
#[derive(Debug, Clone, Copy)]
pub enum Currency {
    BTC,
    ETH,
}

impl Currency {
    pub fn as_str(&self) -> &'static str {
        match self {
            Currency::BTC => "BTC",
            Currency::ETH => "ETH",
        }
    }
}

impl std::str::FromStr for Currency {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_uppercase().as_str() {
            "BTC" => Ok(Currency::BTC),
            "ETH" => Ok(Currency::ETH),
            _ => anyhow::bail!("Invalid currency: {}. Must be BTC or ETH", s),
        }
    }
}

/// Resolution for DVOL data (in seconds)
pub const RESOLUTION_HOURLY: i64 = 3600;

/// Maximum data points per request (Deribit limit)
const MAX_POINTS_PER_REQUEST: i64 = 10000;

/// Chunk size in milliseconds (approximately 10000 hours)
const CHUNK_SIZE_MS: i64 = MAX_POINTS_PER_REQUEST * RESOLUTION_HOURLY * 1000;

/// Fetch DVOL data from Deribit API for a given time range
async fn fetch_dvol_chunk(
    client: &reqwest::Client,
    currency: Currency,
    start_ms: i64,
    end_ms: i64,
) -> Result<Vec<DvolRow>> {
    let url = format!(
        "https://www.deribit.com/api/v2/public/get_volatility_index_data?currency={}&resolution={}&start_timestamp={}&end_timestamp={}",
        currency.as_str(),
        RESOLUTION_HOURLY,
        start_ms,
        end_ms
    );

    info!(
        "Fetching DVOL data for {} from {} to {}",
        currency.as_str(),
        DateTime::<Utc>::from_timestamp_millis(start_ms)
            .map(|dt| dt.to_string())
            .unwrap_or_else(|| start_ms.to_string()),
        DateTime::<Utc>::from_timestamp_millis(end_ms)
            .map(|dt| dt.to_string())
            .unwrap_or_else(|| end_ms.to_string())
    );

    let response = client
        .get(&url)
        .send()
        .await
        .context("Failed to send request to Deribit API")?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("Deribit API error: {} - {}", status, body);
    }

    let data: DeribitResponse = response
        .json()
        .await
        .context("Failed to parse Deribit API response")?;

    let rows: Vec<DvolRow> = data
        .result
        .data
        .into_iter()
        .filter_map(|point| {
            if point.len() >= 5 {
                Some(DvolRow {
                    timestamp: point[0] as i64,
                    currency: currency.as_str().to_string(),
                    open: point[1],
                    high: point[2],
                    low: point[3],
                    close: point[4],
                })
            } else {
                error!("Invalid data point with {} elements", point.len());
                None
            }
        })
        .collect();

    info!("Fetched {} DVOL data points", rows.len());
    Ok(rows)
}

/// Write DVOL rows to ClickHouse
async fn write_to_clickhouse(ch_client: &Client, rows: &[DvolRow]) -> Result<()> {
    if rows.is_empty() {
        return Ok(());
    }

    let mut insert = ch_client.insert("crypto.deribit_dvol")?;

    for row in rows {
        insert.write(row).await?;
    }

    insert.end().await?;
    info!("Wrote {} rows to ClickHouse", rows.len());
    Ok(())
}

/// Run the Deribit DVOL backfill job
pub async fn run(
    currency: Currency,
    start_date: NaiveDate,
    end_date: NaiveDate,
    clickhouse_url: &str,
) -> Result<()> {
    info!(
        "Starting Deribit DVOL backfill for {} from {} to {}",
        currency.as_str(),
        start_date,
        end_date
    );

    // Convert dates to millisecond timestamps
    let start_ms = start_date
        .and_hms_opt(0, 0, 0)
        .context("Invalid start date")?
        .and_utc()
        .timestamp_millis();

    let end_ms = end_date
        .and_hms_opt(23, 59, 59)
        .context("Invalid end date")?
        .and_utc()
        .timestamp_millis();

    // Create HTTP client
    let http_client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .context("Failed to create HTTP client")?;

    // Create ClickHouse client
    let ch_client = Client::default().with_url(clickhouse_url);

    // Ensure the table exists
    ch_client
        .query(
            r#"
            CREATE TABLE IF NOT EXISTS crypto.deribit_dvol (
                timestamp Int64,
                currency String,
                open Float64,
                high Float64,
                low Float64,
                close Float64
            ) ENGINE = ReplacingMergeTree()
            ORDER BY (currency, timestamp)
            "#,
        )
        .execute()
        .await
        .context("Failed to create ClickHouse table")?;

    // Fetch and insert data in chunks
    let mut current_start = start_ms;
    let mut total_rows = 0;

    while current_start < end_ms {
        let chunk_end = std::cmp::min(current_start + CHUNK_SIZE_MS, end_ms);

        match fetch_dvol_chunk(&http_client, currency, current_start, chunk_end).await {
            Ok(rows) => {
                total_rows += rows.len();
                write_to_clickhouse(&ch_client, &rows).await?;
            }
            Err(e) => {
                error!(
                    "Failed to fetch chunk from {} to {}: {}",
                    current_start, chunk_end, e
                );
                return Err(e);
            }
        }

        current_start = chunk_end;

        // Rate limiting: wait between requests
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }

    info!(
        "Deribit DVOL backfill complete. Total rows inserted: {}",
        total_rows
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_currency_from_str() {
        assert!(matches!("BTC".parse::<Currency>().unwrap(), Currency::BTC));
        assert!(matches!("btc".parse::<Currency>().unwrap(), Currency::BTC));
        assert!(matches!("ETH".parse::<Currency>().unwrap(), Currency::ETH));
        assert!(matches!("eth".parse::<Currency>().unwrap(), Currency::ETH));
        assert!("XRP".parse::<Currency>().is_err());
    }
}
