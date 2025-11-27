use std::collections::VecDeque;
use std::sync::{Arc, mpsc};
use std::time::Duration;

use anyhow::{self};
use chrono::{DateTime, Utc};
use log::{error, info, warn};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use tokio::sync::Semaphore;

use crate::{DepthUpdate, LocalOrderBook, LocalOrderBookError, Venue};

pub struct WssApi;

impl WssApi {
    pub fn ping() -> String {
        r#"{"id": 1, "method": "ping"}"#.to_string()
    }

    pub fn depth(symbol: &str) -> String {
        format!(
            r#"{{"id": 1, "method": "SUBSCRIBE", "params": ["{}@depth@100ms"]}}"#,
            symbol.to_lowercase()
        )
    }

    pub fn depth_unsub(symbol: &str) -> String {
        format!(
            r#"{{"id": 1, "method": "UNSUBSCRIBE", "params": ["{}@depth@100ms"]}}"#,
            symbol.to_lowercase()
        )
    }

    pub fn trades(symbol: &str) -> String {
        format!(
            r#"{{"id": 1, "method": "SUBSCRIBE", "params": ["{}@trade"]}}"#,
            symbol.to_lowercase()
        )
    }

    pub fn trades_unsub(symbol: &str) -> String {
        format!(
            r#"{{"id": 1, "method": "UNSUBSCRIBE", "params": ["{}@trade"]}}"#,
            symbol.to_lowercase()
        )
    }

    pub fn batch_depth(symbols: &[String]) -> String {
        let params: Vec<String> = symbols
            .iter()
            .map(|symbol| format!("{}@depth@100ms", symbol.to_lowercase()))
            .collect();
        format!(
            r#"{{"id": 1, "method": "SUBSCRIBE", "params": [{}]}}"#,
            params
                .iter()
                .map(|p| format!("\"{}\"", p))
                .collect::<Vec<_>>()
                .join(",")
        )
    }

    pub fn batch_trades(symbols: &[String]) -> String {
        let params: Vec<String> = symbols
            .iter()
            .map(|symbol| format!("{}@trade", symbol.to_lowercase()))
            .collect();
        format!(
            r#"{{"id": 1, "method": "SUBSCRIBE", "params": [{}]}}"#,
            params
                .iter()
                .map(|p| format!("\"{}\"", p))
                .collect::<Vec<_>>()
                .join(",")
        )
    }

    pub fn candle(symbol: &str) -> String {
        format!(
            r#"{{"id": 1, "method": "SUBSCRIBE", "params": ["{}@kline_1m"]}}"#,
            symbol.to_lowercase()
        )
    }

    pub fn candle_unsub(symbol: &str) -> String {
        format!(
            r#"{{"id": 1, "method": "UNSUBSCRIBE", "params": ["{}@kline_1m"]}}"#,
            symbol.to_lowercase()
        )
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct BinanceDepthDiffMessage {
    #[serde(rename = "e")]
    pub event_type: String, // "depthUpdate"
    #[serde(rename = "E")]
    pub event_time: u64,
    #[serde(rename = "s")]
    pub symbol: String,
    #[serde(rename = "U")]
    pub first_update_id: u64,
    #[serde(rename = "u")]
    pub final_update_id: u64,
    #[serde(rename = "b")]
    pub bids: Vec<[String; 2]>, // [price, quantity]
    #[serde(rename = "a")]
    pub asks: Vec<[String; 2]>, // [price, quantity]
    #[serde(rename = "pu")]
    pub previous_final_update_id: u64,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct BinanceOrderBookSnapshot {
    #[serde(rename = "lastUpdateId")]
    pub last_update_id: u64,
    pub bids: Vec<[String; 2]>,
    pub asks: Vec<[String; 2]>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct BinanceErrorResponse {
    pub code: i32,
    pub msg: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct BinanceKlineData {
    #[serde(rename = "t")]
    pub open_time: u64, // Kline start time
    #[serde(rename = "T")]
    pub close_time: u64, // Kline close time
    #[serde(rename = "s")]
    pub symbol: String, // Symbol
    #[serde(rename = "i")]
    pub interval: String, // Interval
    #[serde(rename = "f")]
    pub first_trade_id: Option<i64>, // First trade ID (can be -1 when no trades)
    #[serde(rename = "L")]
    pub last_trade_id: Option<i64>, // Last trade ID (can be -1 when no trades)
    #[serde(rename = "o")]
    pub open: String, // Open price
    #[serde(rename = "c")]
    pub close: String, // Close price
    #[serde(rename = "h")]
    pub high: String, // High price
    #[serde(rename = "l")]
    pub low: String, // Low price
    #[serde(rename = "v")]
    pub volume: String, // Base asset volume
    #[serde(rename = "n")]
    pub trades: u64, // Number of trades
    #[serde(rename = "x")]
    pub closed: bool, // Is this kline closed?
    #[serde(rename = "q")]
    pub quote_volume: String, // Quote asset volume
    #[serde(rename = "V")]
    pub taker_buy_volume: String, // Taker buy base asset volume
    #[serde(rename = "Q")]
    pub taker_buy_quote_volume: String, // Taker buy quote asset volume
    #[serde(rename = "B")]
    pub ignore: String, // Ignore
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct BinanceCandleMessage {
    #[serde(rename = "e")]
    pub event_type: String, // "kline"
    #[serde(rename = "E")]
    pub event_time: u64, // Event time
    #[serde(rename = "s")]
    pub symbol: String, // Symbol
    #[serde(rename = "k")]
    pub kline: BinanceKlineData, // Kline data
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct BinanceTradeMessage {
    #[serde(rename = "e")]
    pub event_type: String, // "trade"
    #[serde(rename = "E")]
    pub event_time: u64,
    #[serde(rename = "s")]
    pub symbol: String,
    #[serde(rename = "t")]
    pub trade_id: u64,
    #[serde(rename = "p")]
    pub price: String,
    #[serde(rename = "q")]
    pub qty: String,
    #[serde(rename = "T")]
    pub trade_time: u64,
    #[serde(rename = "X", skip_serializing_if = "Option::is_none")]
    pub order_type: Option<String>, // "MARKET", "LIMIT", etc. - optional field
    #[serde(rename = "m")]
    pub is_buyer_maker: bool,
}

pub struct BinanceOrderBook {
    pub symbol: String,
    pub book: Option<LocalOrderBook>,
    pub counter: u64,
    client: Client,
    snapshot: Option<BinanceOrderBookSnapshot>,
    snapshot_triggered: bool,
    event_buff: VecDeque<BinanceDepthDiffMessage>,
    snapshot_rx: Option<mpsc::Receiver<BinanceOrderBookSnapshot>>,
}

// Manual Clone implementation that excludes snapshot_rx
impl Clone for BinanceOrderBook {
    fn clone(&self) -> Self {
        Self {
            symbol: self.symbol.clone(),
            book: self.book.clone(),
            counter: self.counter,
            client: self.client.clone(),
            snapshot: self.snapshot.clone(),
            snapshot_triggered: self.snapshot_triggered,
            event_buff: self.event_buff.clone(),
            snapshot_rx: None, // Don't clone the receiver - it can't be cloned
        }
    }
}

impl BinanceOrderBook {
    pub fn new(symbol: String) -> Self {
        Self {
            symbol,
            book: None,
            counter: 0,
            client: Client::new(),
            snapshot: None,
            snapshot_triggered: false,
            event_buff: VecDeque::new(),
            snapshot_rx: None,
        }
    }

    /// Reset all order book state to handle reconnection
    /// Clears snapshot, book, counter, and event buffer to allow fresh snapshot fetch
    pub fn reset(&mut self) {
        info!("Binance: resetting order book state for {}", self.symbol);
        self.snapshot = None;
        self.snapshot_triggered = false;
        self.book = None;
        self.counter = 0;
        self.event_buff.clear();
        self.snapshot_rx = None;
    }

    /// Build the order book from snapshot and buffered events
    /// This method consumes snapshot and event buffer to construct the initial order book
    fn build_order_book_from_snapshot(
        &mut self,
        snapshot: &BinanceOrderBookSnapshot,
    ) -> Result<(), LocalOrderBookError> {
        info!(
            "Binance: building orderbook from snapshot for {} - snapshot.last_update_id: {}, event_buff.len: {}",
            self.symbol,
            snapshot.last_update_id,
            self.event_buff.len()
        );

        if !self.event_buff.is_empty() {
            if let Some(first_ev) = self.event_buff.front() {
                if let Some(last_ev) = self.event_buff.back() {
                    info!(
                        "Binance: event_buff for {} - first_update_id: {}, final_update_id: {}",
                        self.symbol, first_ev.first_update_id, last_ev.final_update_id
                    );
                }
            }
        }

        let mut updates = Vec::new();

        // Use static string for exchange, avoid allocation
        let exchange_str = Venue::Binance.as_str();
        let coin_str = self.symbol.as_str();
        let time = Utc::now().timestamp_millis();

        for bid in &snapshot.bids {
            if let (Ok(price), Ok(quantity)) = (bid[0].parse(), bid[1].parse()) {
                if quantity > 0.0 {
                    updates.push(DepthUpdate {
                        time,
                        exchange: exchange_str.to_string(),
                        side: false,
                        coin: coin_str.to_string(),
                        px: price,
                        sz: quantity,
                    });
                }
            } else {
                return Err(LocalOrderBookError::UnparseableInputs(
                    exchange_str.to_string(),
                    coin_str.to_string(),
                ));
            }
        }

        for ask in &snapshot.asks {
            if let (Ok(price), Ok(quantity)) = (ask[0].parse(), ask[1].parse()) {
                if quantity > 0.0 {
                    updates.push(DepthUpdate {
                        time,
                        exchange: exchange_str.to_string(),
                        side: true,
                        coin: coin_str.to_string(),
                        px: price,
                        sz: quantity,
                    });
                }
            } else {
                return Err(LocalOrderBookError::UnparseableInputs(
                    exchange_str.to_string(),
                    coin_str.to_string(),
                ));
            }
        }

        let mut local_book = LocalOrderBook::new(
            Utc::now().timestamp_millis(),
            "binance".to_string(),
            self.symbol.to_string(),
        );

        local_book.apply_updates(&updates, 50);

        // Start with snapshot's last_update_id, then update if we have buffered events
        let mut last_final_update_id: u64 = snapshot.last_update_id;
        while !self.event_buff.is_empty() {
            if let Some(event) = self.event_buff.pop_front() {
                last_final_update_id = event.final_update_id;
                if let Ok(updates) = event.try_into() {
                    local_book.apply_updates(&updates, 50);
                }
            }
        }

        self.counter = last_final_update_id;
        self.book = Some(local_book);
        self.event_buff.clear();

        // Log resulting orderbook state
        if let Some(ref book) = self.book {
            let (bid_str, ask_str) = book.get_bbo();
            info!(
                "Binance: orderbook built for {} - counter: {}, BBO: bid={} ask={}",
                self.symbol, self.counter, bid_str, ask_str
            );
        }

        Ok(())
    }

    pub fn new_update_diff(
        &mut self,
        diff: &BinanceDepthDiffMessage,
        snapshot_semaphore: &Arc<Semaphore>,
    ) -> Result<(), LocalOrderBookError> {
        let symbol = diff.symbol.clone();

        // Validate symbol
        if symbol != self.symbol {
            return Err(LocalOrderBookError::WrongSymbol(
                self.symbol.clone(),
                symbol,
            ));
        }

        // Check if there's a pending snapshot from the background thread (non-blocking)
        // try_recv() returns immediately whether data is ready or not
        if let Some(rx) = self.snapshot_rx.take() {
            match rx.try_recv() {
                Ok(snapshot) => {
                    info!(
                        "Binance: received snapshot from background thread for {}",
                        symbol
                    );
                    self.snapshot = Some(snapshot);
                    // Build the order book from the snapshot we just received
                    if let Some(ref snapshot) = self.snapshot {
                        // Clean old events from buffer before building orderbook
                        let mut cleaned_count = 0;
                        info!(
                            "Binance: before cleaning for {} - snapshot.last_update_id: {}, event_buff.len: {}",
                            symbol,
                            snapshot.last_update_id,
                            self.event_buff.len()
                        );
                        if let Some(first_ev) = self.event_buff.front() {
                            info!(
                                "Binance: first event before cleaning - first_update_id: {}, final_update_id: {}",
                                first_ev.first_update_id, first_ev.final_update_id
                            );
                        }
                        while let Some(ev) = self.event_buff.front() {
                            if ev.final_update_id < snapshot.last_update_id {
                                self.event_buff.pop_front();
                                cleaned_count += 1;
                            } else {
                                info!(
                                    "Binance: stopping cleaning for {} - event.final_update_id ({}) >= snapshot.last_update_id ({})",
                                    symbol, ev.final_update_id, snapshot.last_update_id
                                );
                                break;
                            }
                        }
                        if cleaned_count > 0 {
                            info!(
                                "Binance: cleaned {} old events from buffer for {} (older than snapshot.last_update_id: {})",
                                cleaned_count, symbol, snapshot.last_update_id
                            );
                        }

                        // Clone snapshot to avoid borrow checker issues
                        let snapshot_clone = snapshot.clone();
                        if let Err(e) = self.build_order_book_from_snapshot(&snapshot_clone) {
                            error!(
                                "Binance: failed to build order book from snapshot for {}: {}",
                                symbol, e
                            );
                            return Err(e);
                        }
                    }
                }
                Err(mpsc::TryRecvError::Empty) => {
                    // Not ready yet, put it back
                    self.snapshot_rx = Some(rx);
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    error!("Binance: snapshot channel closed for {}", symbol);
                    self.snapshot_triggered = false;
                }
            }
        }

        let no_snapshot = self.snapshot.is_none();
        let not_triggered = !self.snapshot_triggered;

        if no_snapshot && not_triggered {
            self.snapshot_triggered = true;
            let symbol_clone = symbol.clone();
            let semaphore_clone = snapshot_semaphore.clone();
            let client = self.client.clone();
            let (tx, rx) = mpsc::channel();
            self.snapshot_rx = Some(rx);

            std::thread::spawn(move || {
                // Add jitter delay to spread out snapshot fetches after mass reconnect
                use rand::Rng;
                let jitter_ms = rand::thread_rng().gen_range(0..2000);
                std::thread::sleep(Duration::from_millis(jitter_ms));

                // Acquire permit before retry loop to prevent thundering herd
                let _permit = loop {
                    match semaphore_clone.clone().try_acquire_owned() {
                        Ok(permit) => break permit,
                        Err(_) => std::thread::sleep(Duration::from_millis(100)),
                    }
                };

                const MAX_RETRIES: u32 = 5;
                let mut attempt = 0;

                loop {
                    attempt += 1;
                    match fetch_snapshot_impl_blocking(&client, &symbol_clone) {
                        Ok(snapshot) => {
                            info!(
                                "Binance: snapshot fetch completed successfully for {} after {} attempt(s)",
                                symbol_clone, attempt
                            );
                            // Send snapshot back to main instance
                            if tx.send(snapshot).is_err() {
                                error!(
                                    "Binance: failed to send snapshot back for {}",
                                    symbol_clone
                                );
                            }
                            break;
                        }
                        Err(e) => {
                            let error_msg = e.to_string();

                            // Check for rate limit with retry-after
                            if error_msg.starts_with("RETRY_AFTER:") {
                                if let Some(retry_secs_str) = error_msg.strip_prefix("RETRY_AFTER:")
                                {
                                    if let Ok(retry_secs) = retry_secs_str.parse::<u64>() {
                                        error!(
                                            "Binance: rate limited for {}. Waiting {} seconds before retry.",
                                            symbol_clone, retry_secs
                                        );
                                        std::thread::sleep(Duration::from_secs(retry_secs));
                                        continue;
                                    }
                                }
                            }

                            if attempt >= MAX_RETRIES {
                                error!(
                                    "Binance: snapshot fetch failed for {} after {} attempts: {}",
                                    symbol_clone, MAX_RETRIES, e
                                );
                                error!(
                                    "Binance: reset snapshot triggered to false for {}",
                                    symbol_clone
                                );
                                break;
                            } else {
                                let backoff_secs = 2u64.pow(attempt - 1);
                                error!(
                                    "Binance: snapshot fetch failed for {} (attempt {}/{}): {}. Retrying in {}s",
                                    symbol_clone, attempt, MAX_RETRIES, e, backoff_secs
                                );
                                std::thread::sleep(Duration::from_secs(backoff_secs));
                            }
                        }
                    }
                }
            });
        }

        if self.book.is_none() {
            self.event_buff.push_back(diff.clone());
        }

        if let Some(ref snapshot) = self.snapshot {
            if self.book.is_none() {
                // Clean old events from buffer
                let mut cleaned_count = 0;
                info!(
                    "Binance: before cleaning for {} - snapshot.last_update_id: {}, event_buff.len: {}",
                    symbol,
                    snapshot.last_update_id,
                    self.event_buff.len()
                );
                if let Some(first_ev) = self.event_buff.front() {
                    info!(
                        "Binance: first event before cleaning - first_update_id: {}, final_update_id: {}",
                        first_ev.first_update_id, first_ev.final_update_id
                    );
                }
                while let Some(ev) = self.event_buff.front() {
                    if ev.final_update_id < snapshot.last_update_id {
                        self.event_buff.pop_front();
                        cleaned_count += 1;
                    } else {
                        info!(
                            "Binance: stopping cleaning for {} - event.final_update_id ({}) >= snapshot.last_update_id ({})",
                            symbol, ev.final_update_id, snapshot.last_update_id
                        );
                        break;
                    }
                }
                if cleaned_count > 0 {
                    info!(
                        "Binance: cleaned {} old events from buffer for {} (older than snapshot.last_update_id: {})",
                        cleaned_count, symbol, snapshot.last_update_id
                    );
                }
                let ev_count = self.event_buff.len();

                let should_clear = if ev_count == 0 {
                    diff.final_update_id > snapshot.last_update_id
                } else {
                    let front_ev = self.event_buff.front().unwrap();
                    !(front_ev.first_update_id <= snapshot.last_update_id
                        && front_ev.final_update_id >= snapshot.last_update_id)
                };

                if should_clear {
                    error!(
                        "Binance: clearing snapshot for {} due to sequence validation failure (should_clear=true)",
                        symbol
                    );
                    self.snapshot = None;
                    self.snapshot_triggered = false;
                    self.book = None;
                    self.counter = 0;
                    self.event_buff.clear();
                    return Ok(());
                }

                if diff.final_update_id < snapshot.last_update_id {
                    //Waiting for more messages
                    return Ok(());
                }

                // Build order book from snapshot using extracted helper method
                // Clone snapshot to avoid borrow checker issues
                let snapshot_clone = snapshot.clone();
                self.build_order_book_from_snapshot(&snapshot_clone)?;
            } else {
                // Accept message if it chains correctly:
                // 1. Exact match: previous_final_update_id == counter (normal case)
                // 2. Counter within range: first_update_id <= counter <= final_update_id (after snapshot)
                let is_valid_sequence = diff.previous_final_update_id == self.counter
                    || (diff.first_update_id <= self.counter
                        && self.counter <= diff.final_update_id);

                if is_valid_sequence {
                    if let Some(ref mut book) = self.book
                        && let Ok(updates) = diff.clone().try_into()
                    {
                        book.apply_updates(&updates, 50);
                        self.counter = diff.final_update_id;
                    }
                } else {
                    warn!(
                        "Binance: sequence mismatch for {} - diff.first_update_id: {}, diff.previous_final_update_id: {}, self.counter: {}, diff.final_update_id: {}",
                        symbol,
                        diff.first_update_id,
                        diff.previous_final_update_id,
                        self.counter,
                        diff.final_update_id
                    );
                    error!(
                        "Binance: clearing data for {:?} previous: {:?} counter: {:?}",
                        symbol, diff.previous_final_update_id, self.counter
                    );
                    self.snapshot = None;
                    self.snapshot_triggered = false;
                    self.book = None;
                    self.counter = 0;
                    self.event_buff.clear();
                }
            }
        }

        Ok(())
    }
}

/// Blocking snapshot fetch for use with std::thread
fn fetch_snapshot_impl_blocking(
    client: &Client,
    symbol: &str,
) -> anyhow::Result<BinanceOrderBookSnapshot> {
    info!("Binance: fetching snapshot for {:?}", symbol);
    let url = format!(
        "https://fapi.binance.com/fapi/v1/depth?symbol={}&limit=1000",
        symbol.to_uppercase()
    );

    let response = client
        .get(&url)
        .timeout(Duration::from_secs(10))
        .send()
        .map_err(|e| anyhow::anyhow!("Failed to fetch snapshot for {}: {}", symbol, e))?;

    let status = response.status();
    let retry_after = response
        .headers()
        .get("Retry-After")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok());

    // Check for rate limit status codes
    if status.as_u16() == 429 || status.as_u16() == 418 {
        let retry_secs = retry_after.unwrap_or(120);
        return Err(anyhow::anyhow!("RETRY_AFTER:{}", retry_secs));
    }

    let response_text = response
        .text()
        .map_err(|e| anyhow::anyhow!("Failed to read response text for {}: {}", symbol, e))?;

    // Try to parse as error response first
    if let Ok(error_response) = serde_json::from_str::<BinanceErrorResponse>(&response_text) {
        if error_response.code == -1003 {
            return Err(anyhow::anyhow!("RATE_LIMIT: {}", error_response.msg));
        }
        return Err(anyhow::anyhow!(
            "Binance API error {}: {}",
            error_response.code,
            error_response.msg
        ));
    }

    let snapshot =
        serde_json::from_str::<BinanceOrderBookSnapshot>(&response_text).map_err(|e| {
            anyhow::anyhow!(
                "Failed to parse snapshot response for {}: {}. Response: {}",
                symbol,
                e,
                response_text
            )
        })?;

    info!("Binance: successfully fetched snapshot for {}", symbol);
    Ok(snapshot)
}

impl TryFrom<BinanceDepthDiffMessage> for Vec<DepthUpdate> {
    type Error = anyhow::Error;

    fn try_from(value: BinanceDepthDiffMessage) -> std::result::Result<Self, Self::Error> {
        let mut updates = Vec::new();
        let exchange_str = Venue::Binance.as_str();
        let coin_str = value.symbol.as_str();

        for bid in &value.bids {
            let price: f64 = bid[0]
                .parse()
                .map_err(|e| anyhow::Error::msg(format!("Invalid bid price: {e}")))?;
            let quantity: f64 = bid[1]
                .parse()
                .map_err(|e| anyhow::Error::msg(format!("Invalid bid quantity: {e}")))?;

            updates.push(DepthUpdate {
                time: value.event_time as i64,
                exchange: exchange_str.to_string(),
                side: false,
                coin: coin_str.to_string(),
                px: price,
                sz: quantity,
            });
        }

        for ask in &value.asks {
            let price: f64 = ask[0]
                .parse()
                .map_err(|e| anyhow::Error::msg(format!("Invalid ask price: {e}")))?;
            let quantity: f64 = ask[1]
                .parse()
                .map_err(|e| anyhow::Error::msg(format!("Invalid ask quantity: {e}")))?;

            updates.push(DepthUpdate {
                time: value.event_time as i64,
                exchange: exchange_str.to_string(),
                side: true,
                coin: coin_str.to_string(),
                px: price,
                sz: quantity,
            });
        }

        Ok(updates)
    }
}

impl TryFrom<BinanceOrderBookSnapshot> for Vec<DepthUpdate> {
    type Error = anyhow::Error;

    fn try_from(value: BinanceOrderBookSnapshot) -> std::result::Result<Self, Self::Error> {
        let mut updates = Vec::new();
        let timestamp = chrono::Utc::now().timestamp_millis();

        for bid in &value.bids {
            let price: f64 = bid[0]
                .parse()
                .map_err(|e| anyhow::Error::msg(format!("Invalid bid price: {e}")))?;
            let quantity: f64 = bid[1]
                .parse()
                .map_err(|e| anyhow::Error::msg(format!("Invalid bid quantity: {e}")))?;

            if quantity > 0.0 {
                updates.push(DepthUpdate {
                    time: timestamp,
                    exchange: Venue::Binance.to_string(),
                    side: false,
                    coin: "".to_string(),
                    px: price,
                    sz: quantity,
                });
            }
        }

        for ask in &value.asks {
            let price: f64 = ask[0]
                .parse()
                .map_err(|e| anyhow::Error::msg(format!("Invalid ask price: {e}")))?;
            let quantity: f64 = ask[1]
                .parse()
                .map_err(|e| anyhow::Error::msg(format!("Invalid ask quantity: {e}")))?;

            if quantity > 0.0 {
                updates.push(DepthUpdate {
                    time: timestamp,
                    exchange: Venue::Binance.to_string(),
                    side: true,
                    coin: "".to_string(),
                    px: price,
                    sz: quantity,
                });
            }
        }

        Ok(updates)
    }
}

// New From implementations for daily partitioned tables
impl From<BinanceTradeMessage> for Vec<crate::TradeData> {
    fn from(value: BinanceTradeMessage) -> Self {
        let time_ts = DateTime::from_timestamp_millis(value.trade_time as i64)
            .unwrap_or_else(|| {
                log::warn!(
                    "Invalid trade_time timestamp {} for Binance trade symbol {}, using default time",
                    value.trade_time,
                    value.symbol
                );
                DateTime::default()
            });

        vec![crate::TradeData {
            px: value.price,
            qty: value.qty,
            time: value.trade_time,
            time_ts,
            ticker: value.symbol,
            meta: format!(
                "{{\"event_time\": {}, \"is_buyer_maker\": {}}}",
                value.event_time, value.is_buyer_maker
            ),
            side: !value.is_buyer_maker, // side indicates taker direction: true = sell taker, false = buy taker
            venue: Venue::Binance,
        }]
    }
}

impl From<BinanceDepthDiffMessage> for Vec<crate::DepthUpdateData> {
    fn from(value: BinanceDepthDiffMessage) -> Self {
        let mut res = Vec::new();

        for bid in &value.bids {
            res.push(crate::DepthUpdateData {
                px: bid[0].clone(),
                qty: bid[1].clone(),
                time: value.event_time,
                time_ts: DateTime::from_timestamp_millis(value.event_time as i64)
                    .unwrap_or_default(),
                ticker: value.symbol.clone(),
                meta: format!(
                    "{{\"first_update_id\": {}, \"final_update_id\": {}}}",
                    value.first_update_id, value.final_update_id
                ),
                side: false, // bid side
                venue: Venue::Binance,
            });
        }

        for ask in &value.asks {
            res.push(crate::DepthUpdateData {
                px: ask[0].clone(),
                qty: ask[1].clone(),
                time: value.event_time,
                time_ts: DateTime::from_timestamp_millis(value.event_time as i64)
                    .unwrap_or_default(),
                ticker: value.symbol.clone(),
                meta: format!(
                    "{{\"first_update_id\": {}, \"final_update_id\": {}}}",
                    value.first_update_id, value.final_update_id
                ),
                side: true, // ask side
                venue: Venue::Binance,
            });
        }

        res
    }
}

impl From<BinanceOrderBookSnapshot> for crate::DepthSnapshotData {
    fn from(value: BinanceOrderBookSnapshot) -> Self {
        let bid_prices: Vec<String> = value.bids.iter().map(|b| b[0].clone()).collect();
        let bid_sizes: Vec<String> = value.bids.iter().map(|b| b[1].clone()).collect();
        let ask_prices: Vec<String> = value.asks.iter().map(|a| a[0].clone()).collect();
        let ask_sizes: Vec<String> = value.asks.iter().map(|a| a[1].clone()).collect();

        crate::DepthSnapshotData {
            bid_prices,
            bid_sizes,
            ask_prices,
            ask_sizes,
            time: 0, // Binance snapshots don't have a timestamp field, will use current time
            time_ts: chrono::Utc::now(),
            ticker: String::new(), // Symbol not included in snapshot, will be set from context
            venue: crate::Venue::Binance,
        }
    }
}

impl From<BinanceCandleMessage> for crate::Candle {
    fn from(value: BinanceCandleMessage) -> Self {
        use chrono::DateTime;

        let time = DateTime::from_timestamp_millis(value.kline.open_time as i64)
            .unwrap_or_else(|| {
                log::warn!(
                    "Invalid open_time timestamp {} for Binance candle symbol {}, using default time",
                    value.kline.open_time,
                    value.kline.symbol
                );
                DateTime::default()
            });

        let close_time = DateTime::from_timestamp_millis(value.kline.close_time as i64)
            .unwrap_or_else(|| {
                log::warn!(
                    "Invalid close_time timestamp {} for Binance candle symbol {}, using default time",
                    value.kline.close_time,
                    value.kline.symbol
                );
                DateTime::default()
            });

        Self {
            open: value.kline.open,
            high: value.kline.high,
            low: value.kline.low,
            close: value.kline.close,
            volume: value.kline.quote_volume,
            exchange: "binance".to_string(),
            time,
            close_time,
            coin: value.kline.symbol,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_snapshot() -> BinanceOrderBookSnapshot {
        BinanceOrderBookSnapshot {
            last_update_id: 1000,
            bids: vec![
                ["50000.0".to_string(), "1.0".to_string()],
                ["49999.0".to_string(), "2.0".to_string()],
            ],
            asks: vec![
                ["50001.0".to_string(), "1.5".to_string()],
                ["50002.0".to_string(), "2.5".to_string()],
            ],
        }
    }

    fn create_test_diff_message(
        symbol: &str,
        first_update_id: u64,
        final_update_id: u64,
        bids: Vec<[String; 2]>,
        asks: Vec<[String; 2]>,
        previous_final_update_id: u64,
    ) -> BinanceDepthDiffMessage {
        BinanceDepthDiffMessage {
            event_type: "depthUpdate".to_string(),
            event_time: 1234567890,
            symbol: symbol.to_string(),
            first_update_id,
            final_update_id,
            bids,
            asks,
            previous_final_update_id,
        }
    }

    #[test]
    fn test_binance_order_book_snapshot_initialization() {
        // Test that order book can be created and populated with test data
        let mut order_book = BinanceOrderBook::new("BTCUSDT".to_string());

        // Create test order book directly
        let mut local_book =
            LocalOrderBook::new(1000, "binance".to_string(), "BTCUSDT".to_string());

        // Add test data
        let updates = vec![
            DepthUpdate {
                time: 1000,
                exchange: "binance".to_string(),
                side: false,
                coin: "BTCUSDT".to_string(),
                px: 50000.0,
                sz: 1.0,
            },
            DepthUpdate {
                time: 1000,
                exchange: "binance".to_string(),
                side: false,
                coin: "BTCUSDT".to_string(),
                px: 49999.0,
                sz: 2.0,
            },
            DepthUpdate {
                time: 1000,
                exchange: "binance".to_string(),
                side: true,
                coin: "BTCUSDT".to_string(),
                px: 50001.0,
                sz: 1.5,
            },
            DepthUpdate {
                time: 1000,
                exchange: "binance".to_string(),
                side: true,
                coin: "BTCUSDT".to_string(),
                px: 50002.0,
                sz: 2.5,
            },
        ];

        local_book.apply_updates(&updates, 50);
        order_book.book = Some(local_book);
        order_book.counter = 1000;
        assert!(order_book.book.is_some());
        assert_eq!(order_book.counter, 1000);

        let book = order_book.book.as_ref().unwrap();
        assert_eq!(book.test_bid_prices().len(), 2);
        assert_eq!(book.test_ask_prices().len(), 2);
        assert_eq!(book.test_bid_prices()[0], "50000");
        assert_eq!(book.test_ask_prices()[0], "50001");
    }

    #[test]
    fn test_binance_order_book_wrong_symbol() {
        // Test that messages for wrong symbol return WrongSymbol error
        let mut order_book = BinanceOrderBook::new("BTCUSDT".to_string());
        order_book.book = Some(LocalOrderBook::new(
            1000,
            "binance".to_string(),
            "BTCUSDT".to_string(),
        ));
        order_book.counter = 1000;
        order_book.snapshot = Some(create_test_snapshot());

        let eth_msg = create_test_diff_message(
            "ETHUSDT",
            1001,
            1001,
            vec![["3000.0".to_string(), "10.0".to_string()]],
            vec![],
            1000,
        );

        let semaphore = Arc::new(Semaphore::new(2));
        let result = order_book.new_update_diff(&eth_msg, &semaphore);
        assert!(result.is_err());
        if let Err(LocalOrderBookError::WrongSymbol(expected, received)) = result {
            assert_eq!(expected, "BTCUSDT");
            assert_eq!(received, "ETHUSDT");
        } else {
            panic!("Expected WrongSymbol error");
        }
    }

    #[test]
    fn test_binance_snapshot_to_depth_updates() {
        // Test conversion of snapshot to DepthUpdate vector
        let snapshot = create_test_snapshot();
        let updates: Vec<DepthUpdate> = snapshot.try_into().unwrap();

        assert_eq!(updates.len(), 4);

        let bid_updates: Vec<&DepthUpdate> = updates.iter().filter(|u| !u.side).collect();
        let ask_updates: Vec<&DepthUpdate> = updates.iter().filter(|u| u.side).collect();

        assert_eq!(bid_updates.len(), 2);
        assert_eq!(ask_updates.len(), 2);

        assert_eq!(bid_updates[0].px, 50000.0);
        assert_eq!(bid_updates[0].sz, 1.0);
        assert_eq!(bid_updates[1].px, 49999.0);
        assert_eq!(bid_updates[1].sz, 2.0);

        assert_eq!(ask_updates[0].px, 50001.0);
        assert_eq!(ask_updates[0].sz, 1.5);
        assert_eq!(ask_updates[1].px, 50002.0);
        assert_eq!(ask_updates[1].sz, 2.5);
    }

    #[test]
    fn test_binance_candle_subscription() {
        let candle_msg = WssApi::candle("BTCUSDT");
        let expected = r#"{"id": 1, "method": "SUBSCRIBE", "params": ["btcusdt@kline_1m"]}"#;
        assert_eq!(candle_msg, expected);
    }

    #[test]
    fn test_binance_candle_message_parsing() {
        let json_str = r#"{
            "e": "kline",
            "E": 1640995200000,
            "s": "BTCUSDT",
            "k": {
                "t": 1640995200000,
                "T": 1640995260000,
                "s": "BTCUSDT",
                "i": "1m",
                "f": 100,
                "L": 200,
                "o": "50000.00",
                "c": "50100.00",
                "h": "50200.00",
                "l": "49900.00",
                "v": "1000",
                "n": 100,
                "x": true,
                "q": "50050000.00",
                "V": "500",
                "Q": "25025000.00",
                "B": "123456"
            }
        }"#;

        let candle_message: BinanceCandleMessage = serde_json::from_str(json_str).unwrap();
        assert_eq!(candle_message.event_type, "kline");
        assert_eq!(candle_message.symbol, "BTCUSDT");
        assert_eq!(candle_message.kline.open, "50000.00");
        assert_eq!(candle_message.kline.close, "50100.00");
        assert_eq!(candle_message.kline.high, "50200.00");
        assert_eq!(candle_message.kline.low, "49900.00");
        assert_eq!(candle_message.kline.interval, "1m");
        assert!(candle_message.kline.closed);
        assert_eq!(candle_message.kline.first_trade_id, Some(100));
        assert_eq!(candle_message.kline.last_trade_id, Some(200));
    }

    #[test]
    fn test_binance_candle_message_parsing_with_negative_trade_ids() {
        // Test parsing when first_trade_id and last_trade_id are -1 (no trades)
        let json_str = r#"{
            "e": "kline",
            "E": 1640995200000,
            "s": "BTCUSDT",
            "k": {
                "t": 1640995200000,
                "T": 1640995260000,
                "s": "BTCUSDT",
                "i": "1m",
                "f": -1,
                "L": -1,
                "o": "50000.00",
                "c": "50000.00",
                "h": "50000.00",
                "l": "50000.00",
                "v": "0",
                "n": 0,
                "x": true,
                "q": "0.00",
                "V": "0",
                "Q": "0.00",
                "B": "0"
            }
        }"#;

        let candle_message: BinanceCandleMessage = serde_json::from_str(json_str).unwrap();
        assert_eq!(candle_message.event_type, "kline");
        assert_eq!(candle_message.symbol, "BTCUSDT");
        assert_eq!(candle_message.kline.first_trade_id, Some(-1));
        assert_eq!(candle_message.kline.last_trade_id, Some(-1));
        assert_eq!(candle_message.kline.trades, 0);
        assert_eq!(candle_message.kline.volume, "0");

        // Ensure it can still be converted to a Candle
        let candle: crate::Candle = candle_message.into();
        assert_eq!(candle.exchange, "binance");
        assert_eq!(candle.coin, "BTCUSDT");
        assert_eq!(candle.open, "50000.00");
    }

    #[test]
    fn test_binance_candle_to_candle_conversion() {
        let binance_candle = BinanceCandleMessage {
            event_type: "kline".to_string(),
            event_time: 1640995200000,
            symbol: "BTCUSDT".to_string(),
            kline: BinanceKlineData {
                open_time: 1640995200000,
                close_time: 1640995260000,
                symbol: "BTCUSDT".to_string(),
                interval: "1m".to_string(),
                first_trade_id: Some(100),
                last_trade_id: Some(200),
                open: "50000.00".to_string(),
                close: "50100.00".to_string(),
                high: "50200.00".to_string(),
                low: "49900.00".to_string(),
                volume: "1000".to_string(),
                trades: 100,
                closed: true,
                quote_volume: "50050000.00".to_string(),
                taker_buy_volume: "500".to_string(),
                taker_buy_quote_volume: "25025000.00".to_string(),
                ignore: "123456".to_string(),
            },
        };

        let candle: crate::Candle = binance_candle.into();
        assert_eq!(candle.open, "50000.00");
        assert_eq!(candle.close, "50100.00");
        assert_eq!(candle.high, "50200.00");
        assert_eq!(candle.low, "49900.00");
        assert_eq!(candle.volume, "50050000.00");
        assert_eq!(candle.exchange, "binance");
        assert_eq!(candle.coin, "BTCUSDT");
    }

    #[test]
    fn test_binance_candle_invalid_timestamps() {
        // Test that candle conversion handles invalid timestamps gracefully with warning logs instead of panicking
        let invalid_candle = BinanceCandleMessage {
            event_type: "kline".to_string(),
            event_time: 1640995200000,
            symbol: "BTCUSDT".to_string(),
            kline: BinanceKlineData {
                open_time: u64::MAX,
                close_time: u64::MAX,
                symbol: "BTCUSDT".to_string(),
                interval: "1m".to_string(),
                first_trade_id: Some(100),
                last_trade_id: Some(200),
                open: "50000.00".to_string(),
                close: "50100.00".to_string(),
                high: "50200.00".to_string(),
                low: "49900.00".to_string(),
                volume: "1000".to_string(),
                trades: 100,
                closed: true,
                quote_volume: "50050000.00".to_string(),
                taker_buy_volume: "500".to_string(),
                taker_buy_quote_volume: "25025000.00".to_string(),
                ignore: "123456".to_string(),
            },
        };

        let candle: crate::Candle = invalid_candle.into();

        assert_eq!(candle.exchange, "binance");
        assert_eq!(candle.coin, "BTCUSDT");
        assert_eq!(candle.open, "50000.00");
        assert_eq!(candle.time.timestamp_millis(), -1);
        assert_eq!(candle.close_time.timestamp_millis(), -1);
    }

    #[test]
    fn test_binance_order_book_reset() {
        // Test that reset clears all state properly
        let mut order_book = BinanceOrderBook::new("BTCUSDT".to_string());

        // Set up some state
        order_book.snapshot = Some(create_test_snapshot());
        order_book.snapshot_triggered = true;
        order_book.counter = 1000;
        order_book.book = Some(LocalOrderBook::new(
            1000,
            "binance".to_string(),
            "BTCUSDT".to_string(),
        ));
        let test_msg = create_test_diff_message(
            "BTCUSDT",
            1001,
            1001,
            vec![["50000.0".to_string(), "1.0".to_string()]],
            vec![],
            1000,
        );
        order_book.event_buff.push_back(test_msg);

        // Verify state is set
        assert!(order_book.snapshot.is_some());
        assert!(order_book.snapshot_triggered);
        assert_eq!(order_book.counter, 1000);
        assert!(order_book.book.is_some());
        assert_eq!(order_book.event_buff.len(), 1);

        // Reset
        order_book.reset();

        // Verify all state is cleared
        assert!(order_book.snapshot.is_none());
        assert!(!order_book.snapshot_triggered);
        assert_eq!(order_book.counter, 0);
        assert!(order_book.book.is_none());
        assert_eq!(order_book.event_buff.len(), 0);
    }
}
