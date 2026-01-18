use chrono::{DateTime, Utc};
use log::debug;
use serde::{Deserialize, Deserializer, Serialize};

#[cfg(feature = "clickhouse")]
use clickhouse::Row;

pub mod binance;
pub mod bybit;
pub mod hyperliquid;
pub mod kraken;
pub mod orderbook;
pub mod polymarket;

mod websocket_supported_exchanges;
pub use websocket_supported_exchanges::WebsocketSupportedExchanges;

pub use binance::*;
pub use bybit::*;
pub use hyperliquid::*;
pub use kraken::*;
pub use orderbook::*;
pub use polymarket::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "clickhouse", derive(Row))]
pub struct Candle {
    pub open: String,
    pub high: String,
    pub low: String,
    pub close: String,
    pub volume: String,
    pub exchange: String,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub time: DateTime<Utc>,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub close_time: DateTime<Utc>,
    pub coin: String,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "clickhouse", derive(Row))]
pub struct Trade {
    pub px: f64,
    pub sz: f64,
    pub time: i64,
    pub exchange: String,
    pub side: bool,
    pub coin: String,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "clickhouse", derive(Row))]
pub struct DepthUpdate {
    pub px: f64,
    pub sz: f64,
    pub time: i64,
    pub exchange: String,
    pub side: bool,
    pub coin: String,
}

// New exchange types for daily partitioned tables
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "clickhouse", derive(Row))]
pub struct TradeData {
    pub px: String,
    pub qty: String,
    pub time: u64,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub time_ts: DateTime<Utc>,
    pub ticker: String,
    pub meta: String,
    pub side: bool,
    pub venue: Venue,
}

impl TradeData {
    pub fn estimate_size(&self) -> usize {
        std::mem::size_of::<Self>()
            + self.px.capacity()
            + self.qty.capacity()
            + self.ticker.capacity()
            + self.meta.capacity()
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "clickhouse", derive(Row))]
pub struct DepthUpdateData {
    pub px: String,
    pub qty: String,
    pub time: u64,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub time_ts: DateTime<Utc>,
    pub ticker: String,
    pub meta: String,
    pub side: bool,
    pub venue: Venue,
}

impl DepthUpdateData {
    pub fn estimate_size(&self) -> usize {
        std::mem::size_of::<Self>()
            + self.px.capacity()
            + self.qty.capacity()
            + self.ticker.capacity()
            + self.meta.capacity()
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "clickhouse", derive(Row))]
pub struct DepthSnapshotData {
    pub bid_prices: Vec<String>,
    pub bid_sizes: Vec<String>,
    pub ask_prices: Vec<String>,
    pub ask_sizes: Vec<String>,
    pub time: u64,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub time_ts: DateTime<Utc>,
    pub ticker: String,
    pub venue: Venue,
}

impl DepthSnapshotData {
    pub fn estimate_size(&self) -> usize {
        std::mem::size_of::<Self>()
            + self.ticker.capacity()
            + self.bid_prices.iter().map(|s| s.capacity()).sum::<usize>()
            + self.bid_sizes.iter().map(|s| s.capacity()).sum::<usize>()
            + self.ask_prices.iter().map(|s| s.capacity()).sum::<usize>()
            + self.ask_sizes.iter().map(|s| s.capacity()).sum::<usize>()
    }
}

/// Represents the change in order book depth at a specific aggregate level.
///
/// Uses stacked format: one row per (depth_level, side) that changed.
/// Only rows with non-zero changes are emitted.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "clickhouse", derive(Row))]
pub struct BookDeltaData {
    /// Venue identifier
    pub venue: Venue,
    /// Trading pair ticker
    pub ticker: String,
    /// Timestamp of the current snapshot (when delta was computed)
    pub time: u64,
    /// Timestamp as DateTime for ClickHouse
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub time_ts: DateTime<Utc>,
    /// Depth level: "L5", "L10", etc.
    pub depth: String,
    /// Side: true = bid, false = ask
    pub side: bool,
    /// Absolute size flow (positive = depth added, negative = depth removed)
    pub size_flow: f64,
    /// Percentage change in total depth at this level
    pub size_change_pct: f64,
}

impl BookDeltaData {
    pub fn estimate_size(&self) -> usize {
        std::mem::size_of::<Self>() + self.ticker.capacity() + self.depth.capacity()
    }

    /// Parse size strings into f64 values
    fn parse_sizes(sizes: &[String]) -> Vec<f64> {
        sizes.iter().filter_map(|s| s.parse::<f64>().ok()).collect()
    }

    /// Compute deltas between two snapshots.
    /// Returns a Vec of deltas, one per (depth_level, side) that changed.
    /// The first element is the previous snapshot, the second is the current snapshot.
    pub fn from_snapshots(snapshots: [&DepthSnapshotData; 2]) -> Vec<Self> {
        const DEPTH_LEVELS: &[(usize, &str)] = &[(5, "L5"), (10, "L10")];

        let previous = snapshots[0];
        let current = snapshots[1];
        let venue = current.venue;

        let prev_bids = Self::parse_sizes(&previous.bid_sizes);
        let prev_asks = Self::parse_sizes(&previous.ask_sizes);
        let curr_bids = Self::parse_sizes(&current.bid_sizes);
        let curr_asks = Self::parse_sizes(&current.ask_sizes);

        let mut deltas = Vec::new();

        for &(n_levels, depth_name) in DEPTH_LEVELS {
            // Calculate bid delta for this depth level
            let prev_bid_depth: f64 = prev_bids.iter().take(n_levels).sum();
            let curr_bid_depth: f64 = curr_bids.iter().take(n_levels).sum();
            let bid_flow = curr_bid_depth - prev_bid_depth;

            if bid_flow.abs() > 1e-10 {
                let bid_change_pct = if prev_bid_depth > 1e-10 {
                    (bid_flow / prev_bid_depth) * 100.0
                } else if curr_bid_depth > 1e-10 {
                    100.0 // From zero to something
                } else {
                    0.0
                };

                deltas.push(BookDeltaData {
                    venue,
                    ticker: current.ticker.clone(),
                    time: current.time,
                    time_ts: current.time_ts,
                    depth: depth_name.to_string(),
                    side: true, // bid
                    size_flow: bid_flow,
                    size_change_pct: bid_change_pct,
                });
            }

            // Calculate ask delta for this depth level
            let prev_ask_depth: f64 = prev_asks.iter().take(n_levels).sum();
            let curr_ask_depth: f64 = curr_asks.iter().take(n_levels).sum();
            let ask_flow = curr_ask_depth - prev_ask_depth;

            if ask_flow.abs() > 1e-10 {
                let ask_change_pct = if prev_ask_depth > 1e-10 {
                    (ask_flow / prev_ask_depth) * 100.0
                } else if curr_ask_depth > 1e-10 {
                    100.0
                } else {
                    0.0
                };

                deltas.push(BookDeltaData {
                    venue,
                    ticker: current.ticker.clone(),
                    time: current.time,
                    time_ts: current.time_ts,
                    depth: depth_name.to_string(),
                    side: false, // ask
                    size_flow: ask_flow,
                    size_change_pct: ask_change_pct,
                });
            }
        }

        deltas
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
#[repr(u8)]
pub enum Venue {
    ByBit = 0,
    Kraken = 1,
    Hyperliquid = 2,
    Binance = 4,
}

impl serde::Serialize for Venue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_u8(*self as u8)
    }
}

impl<'de> serde::Deserialize<'de> for Venue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = u8::deserialize(deserializer)?;
        match value {
            0 => Ok(Venue::ByBit),
            1 => Ok(Venue::Kraken),
            2 => Ok(Venue::Hyperliquid),
            4 => Ok(Venue::Binance),
            _ => Err(serde::de::Error::custom(format!(
                "Invalid venue value: {}",
                value
            ))),
        }
    }
}

impl From<String> for Venue {
    fn from(value: String) -> Self {
        match value.as_str() {
            "bybit" => Self::ByBit,
            "kraken" => Self::Kraken,
            "hl" => Self::Hyperliquid,
            "hyperliquid" => Self::Hyperliquid,
            "binance" => Self::Binance,
            _ => panic!("Unknown exchange: {value:?}"),
        }
    }
}

impl From<&str> for Venue {
    fn from(value: &str) -> Self {
        match value {
            "bybit" => Self::ByBit,
            "kraken" => Self::Kraken,
            "hl" => Self::Hyperliquid,
            "hyperliquid" => Self::Hyperliquid,
            "binance" => Self::Binance,
            _ => panic!("Unknown exchange: {value:?}"),
        }
    }
}

impl Venue {
    /// Get a static string reference for this venue (avoids allocation)
    #[inline]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::ByBit => "bybit",
            Self::Kraken => "kraken",
            Self::Hyperliquid => "hyperliquid",
            Self::Binance => "binance",
        }
    }
}

impl std::fmt::Display for Venue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum SupportedMessages {
    HyperliquidBookMessage(HyperliquidBookMessage),
    HyperliquidTradesMessage(HyperliquidTradesMessage),
    HyperliquidUserFillsMessage(HyperliquidUserFillsMessage),
    HyperliquidOrderUpdatesMessage(HyperliquidOrderUpdatesMessage),
    HyperliquidBboMessage(HyperliquidBboMessage),
    ByBitDepthMessage(ByBitDepthMessage),
    ByBitTradesMessage(ByBitTradesMessage),
    ByBitCandleMessage(ByBitCandleMessage),
    ByBitMessage(ByBitMessage),
    KrakenBookSnapshotMessage(KrakenBookSnapshotMessage),
    KrakenBookDeltaMessage(KrakenBookDeltaMessage),
    KrakenTradeSnapshotMessage(KrakenTradeSnapshotMessage),
    KrakenTradeDeltaMessage(KrakenTradeDeltaMessage),
    KrakenSubscribeMessage(KrakenSubscribeMessage),
    BinanceDepthDiffMessage(BinanceDepthDiffMessage),
    BinanceTradeMessage(BinanceTradeMessage),
    BinanceOrderBookSnapshot(BinanceOrderBookSnapshot),

    // Internal normalized types
    DepthUpdateData(DepthUpdateData),
    TradeData(TradeData),
    DepthSnapshotData(DepthSnapshotData),
}

impl SupportedMessages {
    pub fn from_message(json: &str, exchange: Venue) -> Option<Self> {
        match exchange {
            Venue::Hyperliquid => serde_json::from_str::<HyperliquidBookMessage>(json)
                .map(SupportedMessages::HyperliquidBookMessage)
                .or_else(|_| {
                    serde_json::from_str::<HyperliquidTradesMessage>(json)
                        .map(SupportedMessages::HyperliquidTradesMessage)
                })
                .or_else(|_| {
                    serde_json::from_str::<HyperliquidUserFillsMessage>(json)
                        .map(SupportedMessages::HyperliquidUserFillsMessage)
                })
                .or_else(|_| {
                    serde_json::from_str::<HyperliquidOrderUpdatesMessage>(json)
                        .map(SupportedMessages::HyperliquidOrderUpdatesMessage)
                })
                .or_else(|_| {
                    serde_json::from_str::<HyperliquidBboMessage>(json)
                        .map(SupportedMessages::HyperliquidBboMessage)
                })
                .ok(),

            Venue::ByBit => serde_json::from_str::<ByBitDepthMessage>(json)
                .map(SupportedMessages::ByBitDepthMessage)
                .or_else(|_| {
                    serde_json::from_str::<ByBitTradesMessage>(json)
                        .map(SupportedMessages::ByBitTradesMessage)
                })
                .or_else(|_| {
                    serde_json::from_str::<ByBitCandleMessage>(json)
                        .map(SupportedMessages::ByBitCandleMessage)
                })
                .map_err(|err| {
                    // Log unrecognized message for debugging purposes
                    debug!("Unrecognized bybit message format: {json} - Error: {err}");
                    err
                })
                .ok(),

            Venue::Kraken => serde_json::from_str::<KrakenBookSnapshotMessage>(json)
                .map(SupportedMessages::KrakenBookSnapshotMessage)
                .or_else(|_| {
                    serde_json::from_str::<KrakenTradeSnapshotMessage>(json)
                        .map(SupportedMessages::KrakenTradeSnapshotMessage)
                })
                .or_else(|_| {
                    serde_json::from_str::<KrakenTradeDeltaMessage>(json)
                        .map(SupportedMessages::KrakenTradeDeltaMessage)
                })
                .or_else(|_| {
                    serde_json::from_str::<KrakenBookDeltaMessage>(json)
                        .map(SupportedMessages::KrakenBookDeltaMessage)
                })
                .or_else(|_| {
                    serde_json::from_str::<KrakenSubscribeMessage>(json)
                        .map(SupportedMessages::KrakenSubscribeMessage)
                })
                .map_err(|err| {
                    // Log unrecognized message for debugging purposes
                    debug!("Unrecognized kraken derivatives message format: {json} - Error: {err}");
                    err
                })
                .ok(),

            Venue::Binance => serde_json::from_str::<BinanceDepthDiffMessage>(json)
                .map(SupportedMessages::BinanceDepthDiffMessage)
                .or_else(|_| {
                    serde_json::from_str::<BinanceTradeMessage>(json)
                        .map(SupportedMessages::BinanceTradeMessage)
                })
                .map_err(|err| {
                    // Log unrecognized message for debugging purposes
                    debug!("Unrecognized binance message format: {json} - Error: {err}");
                    err
                })
                .ok(),
        }
    }

    pub fn from_message_with_date(msg: String, exchange: Venue) -> Option<Self> {
        let json = msg.split_once(' ')?.1;
        SupportedMessages::from_message(json, exchange)
    }

    // Legacy method for compatibility with string-based calls
    pub fn from_message_legacy(json: &str, exchange: String) -> Option<Self> {
        let venue = Venue::from(exchange);
        Self::from_message(json, venue)
    }

    pub fn from_message_with_date_legacy(msg: String, exchange: String) -> Option<Self> {
        let venue = Venue::from(exchange);
        Self::from_message_with_date(msg, venue)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn make_snapshot(
        ticker: &str,
        time: u64,
        bid_sizes: Vec<&str>,
        ask_sizes: Vec<&str>,
    ) -> DepthSnapshotData {
        // Prices don't matter for delta calculation, just need placeholders
        let bid_prices: Vec<String> = (0..bid_sizes.len())
            .map(|i| format!("{}", 100.0 - i as f64))
            .collect();
        let ask_prices: Vec<String> = (0..ask_sizes.len())
            .map(|i| format!("{}", 101.0 + i as f64))
            .collect();

        DepthSnapshotData {
            venue: Venue::Binance,
            ticker: ticker.to_string(),
            time,
            time_ts: Utc.timestamp_millis_opt(time as i64).unwrap(),
            bid_prices,
            bid_sizes: bid_sizes.into_iter().map(String::from).collect(),
            ask_prices,
            ask_sizes: ask_sizes.into_iter().map(String::from).collect(),
        }
    }

    #[test]
    fn test_no_change_returns_empty() {
        let snap1 = make_snapshot("BTCUSDT", 1000, vec!["10.0"; 5], vec!["10.0"; 5]);
        let snap2 = make_snapshot("BTCUSDT", 2000, vec!["10.0"; 5], vec!["10.0"; 5]);

        let deltas = BookDeltaData::from_snapshots([&snap1, &snap2]);

        assert!(deltas.is_empty());
    }

    #[test]
    fn test_bid_increase_emits_delta() {
        let snap1 = make_snapshot("BTCUSDT", 1000, vec!["10.0"; 5], vec!["10.0"; 5]);
        let snap2 = make_snapshot("BTCUSDT", 2000, vec!["15.0"; 5], vec!["10.0"; 5]);

        let deltas = BookDeltaData::from_snapshots([&snap1, &snap2]);

        // Should have L5 and L10 bid deltas (L10 sums same 5 levels, asks unchanged)
        // With only 5 levels, L5 and L10 compute the same sum
        assert_eq!(deltas.len(), 2);

        let l5_delta = deltas.iter().find(|d| d.depth == "L5" && d.side).unwrap();
        assert_eq!(l5_delta.size_flow, 25.0); // 5 * (15 - 10)
        assert_eq!(l5_delta.size_change_pct, 50.0); // 25 / 50 * 100
    }

    #[test]
    fn test_both_sides_change() {
        let snap1 = make_snapshot("BTCUSDT", 1000, vec!["10.0"; 5], vec!["10.0"; 5]);
        let snap2 = make_snapshot("BTCUSDT", 2000, vec!["15.0"; 5], vec!["5.0"; 5]);

        let deltas = BookDeltaData::from_snapshots([&snap1, &snap2]);

        // Should have L5 and L10 for both bid and ask (4 total)
        assert_eq!(deltas.len(), 4);

        let l5_bid = deltas.iter().find(|d| d.depth == "L5" && d.side).unwrap();
        assert_eq!(l5_bid.size_flow, 25.0);

        let l5_ask = deltas.iter().find(|d| d.depth == "L5" && !d.side).unwrap();
        assert_eq!(l5_ask.size_flow, -25.0);
    }

    #[test]
    fn test_l10_with_enough_levels() {
        let snap1 = make_snapshot("BTCUSDT", 1000, vec!["10.0"; 10], vec!["10.0"; 10]);
        let snap2 = make_snapshot("BTCUSDT", 2000, vec!["15.0"; 10], vec!["10.0"; 10]);

        let deltas = BookDeltaData::from_snapshots([&snap1, &snap2]);

        // Should have both L5 and L10 bid deltas
        assert_eq!(deltas.len(), 2);
        assert!(deltas.iter().any(|d| d.depth == "L5"));
        assert!(deltas.iter().any(|d| d.depth == "L10"));
    }
}
