//! Deribit historical trades fetcher job.
//!
//! This job fetches historical trades from Deribit's public API and stores them in ClickHouse.
//!
//! # Usage
//! ```
//! cargo run --bin roshar-jobs deribit-trades --currency BTC --start 20240101 --end 20240131
//! ```

use chrono::{Datelike, NaiveDate, Utc};
use clickhouse::{Client as ClickhouseClient, Row};
use log::{info, warn};
use anyhow::{anyhow, Result};
use crate::Config;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::{sleep, Instant};

// ============================================================================
// Date Parsing
// ============================================================================

/// Parse date string in YYYYMMDD or YYYY-MM-DD format to NaiveDate
fn parse_date(s: &str) -> Result<NaiveDate, String> {
    // Try YYYYMMDD format first
    if let Ok(date) = NaiveDate::parse_from_str(s, "%Y%m%d") {
        return Ok(date);
    }
    // Try YYYY-MM-DD format
    if let Ok(date) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return Ok(date);
    }
    Err(format!("Invalid date format: {}. Use YYYYMMDD or YYYY-MM-DD", s))
}

// ============================================================================
// Rate Limiter
// ============================================================================

/// Weight-based rate limiter similar to Binance's approach.
///
/// Tracks request weights and enforces limits. On 429 response, triggers
/// a global pause for all requests.
pub struct RateLimiter {
    /// Maximum weight allowed per interval
    max_weight: u32,
    /// Current accumulated weight
    current_weight: Arc<Mutex<u32>>,
    /// Timestamp when the current window started
    window_start: Arc<Mutex<Instant>>,
    /// Window duration in milliseconds
    window_duration_ms: u64,
    /// Global pause flag (set on 429)
    is_paused: Arc<AtomicBool>,
    /// Pause duration in milliseconds
    pause_duration_ms: u64,
}

impl RateLimiter {
    /// Create a new rate limiter.
    ///
    /// # Arguments
    /// * `max_weight` - Maximum weight allowed per window
    /// * `window_duration_ms` - Duration of the rate limit window in milliseconds
    /// * `pause_duration_ms` - How long to pause on 429 response
    pub fn new(max_weight: u32, window_duration_ms: u64, pause_duration_ms: u64) -> Self {
        Self {
            max_weight,
            current_weight: Arc::new(Mutex::new(0)),
            window_start: Arc::new(Mutex::new(Instant::now())),
            window_duration_ms,
            is_paused: Arc::new(AtomicBool::new(false)),
            pause_duration_ms,
        }
    }

    /// Acquire permission to make a request with the given weight.
    /// Blocks until the request can be made without exceeding rate limits.
    pub async fn acquire(&self, weight: u32) {
        loop {
            // Check if we're in a global pause
            if self.is_paused.load(Ordering::SeqCst) {
                info!("Rate limiter paused, waiting {}ms...", self.pause_duration_ms);
                sleep(Duration::from_millis(self.pause_duration_ms)).await;
                self.is_paused.store(false, Ordering::SeqCst);
            }

            let mut current = self.current_weight.lock().await;
            let mut window_start = self.window_start.lock().await;

            let elapsed = window_start.elapsed().as_millis() as u64;

            // Reset window if expired
            if elapsed >= self.window_duration_ms {
                *current = 0;
                *window_start = Instant::now();
            }

            // Check if we can accommodate this request
            if *current + weight <= self.max_weight {
                *current += weight;
                return;
            }

            // Need to wait for window to reset
            let wait_time = self.window_duration_ms - elapsed;
            drop(current);
            drop(window_start);

            info!("Rate limit reached, waiting {}ms for window reset...", wait_time);
            sleep(Duration::from_millis(wait_time + 100)).await;
        }
    }

    /// Trigger a global pause (call this on 429 response)
    pub fn trigger_pause(&self) {
        warn!("429 received, triggering global rate limit pause");
        self.is_paused.store(true, Ordering::SeqCst);
    }
}

// ============================================================================
// Deribit API Types
// ============================================================================

/// Trade data from Deribit API response
#[derive(Debug, Clone, Deserialize)]
pub struct DeribitTrade {
    /// Unique trade identifier
    pub trade_id: String,
    /// Trade timestamp in milliseconds
    pub timestamp: i64,
    /// Instrument name (e.g., "BTC-PERPETUAL")
    pub instrument_name: String,
    /// Trade price
    pub price: f64,
    /// Trade amount in base currency
    pub amount: f64,
    /// Trade direction: "buy" or "sell"
    pub direction: String,
    /// Mark price at the time of trade
    #[serde(default)]
    pub mark_price: Option<f64>,
    /// Index price at the time of trade
    #[serde(default)]
    pub index_price: Option<f64>,
    /// Number of contracts
    #[serde(default)]
    pub contracts: Option<f64>,
    /// Tick direction (0=plus, 1=zero_plus, 2=minus, 3=zero_minus)
    #[serde(default)]
    pub tick_direction: Option<u8>,
    /// Trade sequence number
    #[serde(default)]
    pub trade_seq: Option<i64>,
}

/// Response wrapper for Deribit API
#[derive(Debug, Deserialize)]
pub struct DeribitResponse<T> {
    pub result: T,
}

/// Result of get_last_trades_by_currency_and_time
#[derive(Debug, Deserialize)]
pub struct DeribitTradesResult {
    pub trades: Vec<DeribitTrade>,
    pub has_more: bool,
}

// ============================================================================
// ClickHouse Row Struct
// ============================================================================

/// ClickHouse row for storing Deribit historical trades
#[derive(Debug, Clone, Serialize, Row)]
pub struct DeribitHistoricalTrade {
    /// Unique trade identifier
    pub trade_id: String,
    /// Trade timestamp in milliseconds
    pub timestamp: i64,
    /// Instrument name (e.g., "BTC-PERPETUAL")
    pub instrument_name: String,
    /// Trade price
    pub price: f64,
    /// Trade amount in base currency
    pub amount: f64,
    /// Trade direction: "buy" or "sell"
    pub direction: String,
    /// Mark price at the time of trade
    pub mark_price: f64,
    /// Index price at the time of trade
    pub index_price: f64,
    /// Number of contracts
    pub contracts: f64,
    /// Tick direction (0=plus, 1=zero_plus, 2=minus, 3=zero_minus)
    pub tick_direction: u8,
    /// Trade sequence number
    pub trade_seq: i64,
    /// Currency (BTC or ETH)
    pub currency: String,
}

impl DeribitHistoricalTrade {
    /// Convert from API trade response to ClickHouse row
    pub fn from_api_trade(trade: DeribitTrade, currency: &str) -> Self {
        Self {
            trade_id: trade.trade_id,
            timestamp: trade.timestamp,
            instrument_name: trade.instrument_name,
            price: trade.price,
            amount: trade.amount,
            direction: trade.direction,
            mark_price: trade.mark_price.unwrap_or(0.0),
            index_price: trade.index_price.unwrap_or(0.0),
            contracts: trade.contracts.unwrap_or(0.0),
            tick_direction: trade.tick_direction.unwrap_or(0),
            trade_seq: trade.trade_seq.unwrap_or(0),
            currency: currency.to_string(),
        }
    }
}

// ============================================================================
// Deribit Client
// ============================================================================

const DERIBIT_HISTORY_API_URL: &str = "https://history.deribit.com/api/v2/public/get_last_trades_by_currency_and_time";

/// Deribit API client for fetching historical trades
pub struct DeribitClient {
    client: Client,
    rate_limiter: Arc<RateLimiter>,
}

impl DeribitClient {
    /// Create a new Deribit client
    pub fn new(rate_limiter: Arc<RateLimiter>) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            rate_limiter,
        }
    }

    /// Fetch trades by currency and time range
    ///
    /// # Arguments
    /// * `currency` - Currency to fetch (BTC or ETH)
    /// * `start_timestamp` - Start timestamp in milliseconds
    /// * `end_timestamp` - End timestamp in milliseconds
    /// * `count` - Maximum number of trades to return (max 1000)
    ///
    /// # Returns
    /// Tuple of (trades, has_more)
    pub async fn get_trades_by_currency_and_time(
        &self,
        currency: &str,
        start_timestamp: i64,
        end_timestamp: i64,
        count: u32,
    ) -> Result<(Vec<DeribitTrade>, bool), String> {
        // Acquire rate limit (weight of 1 per request)
        self.rate_limiter.acquire(1).await;

        let url = format!(
            "{}?currency={}&start_timestamp={}&end_timestamp={}&count={}",
            DERIBIT_HISTORY_API_URL, currency, start_timestamp, end_timestamp, count
        );

        let response = self.client.get(&url).send().await.map_err(|e| {
            format!("HTTP request failed: {}", e)
        })?;

        let status = response.status();

        // Handle rate limiting
        if status.as_u16() == 429 {
            self.rate_limiter.trigger_pause();
            return Err("Rate limited (429)".to_string());
        }

        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(format!("API error {}: {}", status.as_u16(), body));
        }

        let response_body: DeribitResponse<DeribitTradesResult> = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))?;

        Ok((response_body.result.trades, response_body.result.has_more))
    }

    /// Fetch all trades in a time range with automatic pagination
    pub async fn fetch_all_trades_in_range(
        &self,
        currency: &str,
        start_timestamp: i64,
        end_timestamp: i64,
    ) -> Result<Vec<DeribitTrade>, String> {
        let mut all_trades = Vec::new();
        let mut current_start = start_timestamp;
        let count = 1000;

        loop {
            match self
                .get_trades_by_currency_and_time(currency, current_start, end_timestamp, count)
                .await
            {
                Ok((trades, has_more)) => {
                    if trades.is_empty() {
                        break;
                    }

                    // Get the last trade's timestamp for pagination
                    let last_timestamp = trades.last().map(|t| t.timestamp).unwrap_or(end_timestamp);

                    all_trades.extend(trades);

                    if !has_more {
                        break;
                    }

                    // Use last trade's timestamp + 1 as next start
                    current_start = last_timestamp + 1;

                    // Safety check to avoid infinite loops
                    if current_start >= end_timestamp {
                        break;
                    }
                }
                Err(e) => {
                    if e.contains("429") {
                        // Wait and retry on rate limit
                        info!("Rate limited during pagination, waiting and retrying...");
                        sleep(Duration::from_secs(5)).await;
                        continue;
                    }
                    return Err(e);
                }
            }
        }

        Ok(all_trades)
    }
}

// ============================================================================
// ClickHouse Operations
// ============================================================================

const CLICKHOUSE_TABLE: &str = "crypto.deribit_historical_trades";
const BATCH_SIZE: usize = 1000;

/// Write trades to ClickHouse in batches
pub async fn trades_to_clickhouse(
    client: &ClickhouseClient,
    trades: Vec<DeribitHistoricalTrade>,
) -> Result<usize, String> {
    if trades.is_empty() {
        return Ok(0);
    }

    let total_trades = trades.len();
    let mut inserted = 0;

    for chunk in trades.chunks(BATCH_SIZE) {
        let mut insert = client
            .insert(CLICKHOUSE_TABLE)
            .map_err(|e| format!("Failed to create insert: {}", e))?;

        for trade in chunk {
            insert
                .write(trade)
                .await
                .map_err(|e| format!("Failed to write trade: {}", e))?;
        }

        insert
            .end()
            .await
            .map_err(|e| format!("Failed to finish insert: {}", e))?;

        inserted += chunk.len();
        info!("Inserted {}/{} trades to ClickHouse", inserted, total_trades);
    }

    Ok(inserted)
}

// ============================================================================
// Date Range Utilities
// ============================================================================

/// Generate month ranges between start and end dates
fn generate_month_ranges(start: NaiveDate, end: NaiveDate) -> Vec<(NaiveDate, NaiveDate)> {
    let mut ranges = Vec::new();
    let mut current = start;

    while current <= end {
        // Calculate end of current month
        let month_end = if current.month() == 12 {
            NaiveDate::from_ymd_opt(current.year() + 1, 1, 1)
                .unwrap()
                .pred_opt()
                .unwrap()
        } else {
            NaiveDate::from_ymd_opt(current.year(), current.month() + 1, 1)
                .unwrap()
                .pred_opt()
                .unwrap()
        };

        // Use the earlier of month_end or the overall end date
        let range_end = if month_end < end { month_end } else { end };

        ranges.push((current, range_end));

        // Move to next month
        if month_end >= end {
            break;
        }
        current = month_end.succ_opt().unwrap();
    }

    ranges
}

/// Convert NaiveDate to milliseconds timestamp (start of day UTC)
fn date_to_timestamp_ms(date: NaiveDate) -> i64 {
    date.and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc()
        .timestamp_millis()
}

/// Convert NaiveDate to milliseconds timestamp (end of day UTC)
fn date_to_end_timestamp_ms(date: NaiveDate) -> i64 {
    date.and_hms_opt(23, 59, 59)
        .unwrap()
        .and_utc()
        .timestamp_millis()
        + 999
}

// ============================================================================
// Main Run Function
// ============================================================================

/// Run the Deribit trades fetcher job
pub async fn run(
    config: Config,
    currency: String,
    start: String,
    end: Option<String>,
) -> Result<()> {
    // Validate currency
    let currency = currency.to_uppercase();
    if currency != "BTC" && currency != "ETH" {
        return Err(anyhow!("Invalid currency: {}. Must be BTC or ETH", currency));
    }

    // Use current date if end is not specified
    let end_date_str = end.unwrap_or_else(|| {
        Utc::now().format("%Y%m%d").to_string()
    });

    info!(
        "Starting Deribit trades fetch: currency={}, start={}, end={}",
        currency, start, end_date_str
    );

    // Parse dates
    let start_date = parse_date(&start).map_err(|e| anyhow!(e))?;
    let end_date = parse_date(&end_date_str).map_err(|e| anyhow!(e))?;

    if start_date > end_date {
        return Err(anyhow!("Start date must be before or equal to end date"));
    }

    // Create rate limiter (10 requests per second, 60s window, 30s pause on 429)
    let rate_limiter = Arc::new(RateLimiter::new(10, 1000, 30000));

    // Create Deribit client
    let deribit_client = DeribitClient::new(rate_limiter);

    // Create ClickHouse client
    let clickhouse_client = ClickhouseClient::default()
        .with_url(&config.clickhouse_url);

    // Generate month ranges
    let month_ranges = generate_month_ranges(start_date, end_date);
    info!("Processing {} month ranges", month_ranges.len());

    let mut total_trades = 0;
    let mut failed_ranges = Vec::new();

    // Process each month range
    for (range_start, range_end) in &month_ranges {
        let start_ts = date_to_timestamp_ms(*range_start);
        let end_ts = date_to_end_timestamp_ms(*range_end);

        info!(
            "Fetching trades for {}-{:02} to {}-{:02}...",
            range_start.year(),
            range_start.month(),
            range_end.year(),
            range_end.month()
        );

        match deribit_client
            .fetch_all_trades_in_range(&currency, start_ts, end_ts)
            .await
        {
            Ok(trades) => {
                if trades.is_empty() {
                    info!("No trades found for this period");
                    continue;
                }

                info!("Fetched {} trades, writing to ClickHouse...", trades.len());

                // Convert to ClickHouse rows
                let ch_trades: Vec<DeribitHistoricalTrade> = trades
                    .into_iter()
                    .map(|t| DeribitHistoricalTrade::from_api_trade(t, &currency))
                    .collect();

                match trades_to_clickhouse(&clickhouse_client, ch_trades).await {
                    Ok(count) => {
                        total_trades += count;
                        info!("Successfully inserted {} trades", count);
                    }
                    Err(e) => {
                        warn!(
                            "Failed to insert trades for {}-{:02}: {}",
                            range_start.year(),
                            range_start.month(),
                            e
                        );
                        failed_ranges.push((*range_start, *range_end, e));
                    }
                }
            }
            Err(e) => {
                warn!(
                    "Failed to fetch trades for {}-{:02}: {}",
                    range_start.year(),
                    range_start.month(),
                    e
                );
                failed_ranges.push((*range_start, *range_end, e));
            }
        }
    }

    // Report results
    info!("=== Job Complete ===");
    info!("Total trades inserted: {}", total_trades);

    if !failed_ranges.is_empty() {
        warn!("Failed ranges ({}):", failed_ranges.len());
        for (start, end, err) in &failed_ranges {
            warn!("  {}-{:02} to {}-{:02}: {}", start.year(), start.month(), end.year(), end.month(), err);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_date_yyyymmdd() {
        let date = parse_date("20240115").unwrap();
        assert_eq!(date.year(), 2024);
        assert_eq!(date.month(), 1);
        assert_eq!(date.day(), 15);
    }

    #[test]
    fn test_parse_date_yyyy_mm_dd() {
        let date = parse_date("2024-01-15").unwrap();
        assert_eq!(date.year(), 2024);
        assert_eq!(date.month(), 1);
        assert_eq!(date.day(), 15);
    }

    #[test]
    fn test_parse_date_invalid() {
        assert!(parse_date("invalid").is_err());
        assert!(parse_date("2024/01/15").is_err());
    }

    #[test]
    fn test_generate_month_ranges_single_month() {
        let start = NaiveDate::from_ymd_opt(2024, 1, 10).unwrap();
        let end = NaiveDate::from_ymd_opt(2024, 1, 20).unwrap();
        let ranges = generate_month_ranges(start, end);

        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0], (start, end));
    }

    #[test]
    fn test_generate_month_ranges_multiple_months() {
        let start = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();
        let end = NaiveDate::from_ymd_opt(2024, 3, 10).unwrap();
        let ranges = generate_month_ranges(start, end);

        assert_eq!(ranges.len(), 3);
        assert_eq!(ranges[0].0, NaiveDate::from_ymd_opt(2024, 1, 15).unwrap());
        assert_eq!(ranges[0].1, NaiveDate::from_ymd_opt(2024, 1, 31).unwrap());
        assert_eq!(ranges[1].0, NaiveDate::from_ymd_opt(2024, 2, 1).unwrap());
        assert_eq!(ranges[1].1, NaiveDate::from_ymd_opt(2024, 2, 29).unwrap()); // 2024 is leap year
        assert_eq!(ranges[2].0, NaiveDate::from_ymd_opt(2024, 3, 1).unwrap());
        assert_eq!(ranges[2].1, NaiveDate::from_ymd_opt(2024, 3, 10).unwrap());
    }

    #[test]
    fn test_date_to_timestamp_ms() {
        let date = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
        let ts = date_to_timestamp_ms(date);
        // 2024-01-01 00:00:00 UTC in milliseconds
        assert_eq!(ts, 1704067200000);
    }

    #[test]
    fn test_deribit_trade_deserialization() {
        let json = r#"{
            "trade_id": "123456789",
            "timestamp": 1704067200000,
            "instrument_name": "BTC-PERPETUAL",
            "price": 42000.5,
            "amount": 1.5,
            "direction": "buy",
            "mark_price": 42001.0,
            "index_price": 42000.0,
            "contracts": 1.5,
            "tick_direction": 0,
            "trade_seq": 987654321
        }"#;

        let trade: DeribitTrade = serde_json::from_str(json).unwrap();
        assert_eq!(trade.trade_id, "123456789");
        assert_eq!(trade.timestamp, 1704067200000);
        assert_eq!(trade.instrument_name, "BTC-PERPETUAL");
        assert_eq!(trade.price, 42000.5);
        assert_eq!(trade.amount, 1.5);
        assert_eq!(trade.direction, "buy");
        assert_eq!(trade.mark_price, Some(42001.0));
        assert_eq!(trade.index_price, Some(42000.0));
        assert_eq!(trade.contracts, Some(1.5));
        assert_eq!(trade.tick_direction, Some(0));
        assert_eq!(trade.trade_seq, Some(987654321));
    }

    #[test]
    fn test_deribit_trade_deserialization_minimal() {
        let json = r#"{
            "trade_id": "123",
            "timestamp": 1000,
            "instrument_name": "ETH-PERPETUAL",
            "price": 2000.0,
            "amount": 0.5,
            "direction": "sell"
        }"#;

        let trade: DeribitTrade = serde_json::from_str(json).unwrap();
        assert_eq!(trade.trade_id, "123");
        assert_eq!(trade.direction, "sell");
        assert_eq!(trade.mark_price, None);
        assert_eq!(trade.index_price, None);
        assert_eq!(trade.contracts, None);
        assert_eq!(trade.tick_direction, None);
        assert_eq!(trade.trade_seq, None);
    }

    #[test]
    fn test_historical_trade_from_api() {
        let api_trade = DeribitTrade {
            trade_id: "123".to_string(),
            timestamp: 1704067200000,
            instrument_name: "BTC-PERPETUAL".to_string(),
            price: 42000.0,
            amount: 1.0,
            direction: "buy".to_string(),
            mark_price: Some(42001.0),
            index_price: Some(42000.0),
            contracts: Some(1.0),
            tick_direction: Some(0),
            trade_seq: Some(100),
        };

        let ch_trade = DeribitHistoricalTrade::from_api_trade(api_trade, "BTC");

        assert_eq!(ch_trade.trade_id, "123");
        assert_eq!(ch_trade.currency, "BTC");
        assert_eq!(ch_trade.mark_price, 42001.0);
        assert_eq!(ch_trade.tick_direction, 0);
    }
}
