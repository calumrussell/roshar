use chrono::{DateTime, Utc};
use log::debug;
use serde::{Deserialize, Deserializer, Serialize};

#[cfg(feature = "clickhouse")]
use clickhouse::Row;

pub mod exchanges;
pub mod hyperliquid;
pub mod orderbook;

pub use exchanges::WebsocketSupportedExchanges;
pub use exchanges::binance::*;
pub use exchanges::bybit::*;
pub use exchanges::bybitspot::*;
pub use exchanges::kraken::*;
pub use exchanges::krakenspot::*;
pub use exchanges::mex::*;
pub use hyperliquid::*;
pub use orderbook::*;

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


#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
#[repr(u8)]
pub enum Venue {
    ByBit = 0,
    Kraken = 1,
    Hyperliquid = 2,
    Binance = 4,
    ByBitSpot = 5,
    KrakenSpot = 6,
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
            5 => Ok(Venue::ByBitSpot),
            6 => Ok(Venue::KrakenSpot),
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
            "bybit-spot" => Self::ByBitSpot,
            "kraken-spot" => Self::KrakenSpot,
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
            "bybit-spot" => Self::ByBitSpot,
            "kraken-spot" => Self::KrakenSpot,
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
            Self::ByBitSpot => "bybit-spot",
            Self::KrakenSpot => "kraken-spot",
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
    KrakenSpotOhlcMessage(KrakenSpotOhlcMessage),
    ByBitDepthMessage(ByBitDepthMessage),
    ByBitTradesMessage(ByBitTradesMessage),
    ByBitCandleMessage(ByBitCandleMessage),
    ByBitMessage(ByBitMessage),
    ByBitSpotDepthMessage(ByBitSpotDepthMessage),
    ByBitSpotTradesMessage(ByBitSpotTradesMessage),
    KrakenSpotBookMessage(KrakenSpotBookMessage),
    KrakenSpotTradeMessage(KrakenSpotTradeMessage),
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

            Venue::ByBitSpot => serde_json::from_str::<ByBitSpotDepthMessage>(json)
                .map(SupportedMessages::ByBitSpotDepthMessage)
                .or_else(|_| {
                    serde_json::from_str::<ByBitTradesMessage>(json)
                        .map(SupportedMessages::ByBitTradesMessage)
                })
                .map_err(|err| {
                    // Log unrecognized message for debugging purposes
                    debug!("Unrecognized bybit-spot message format: {json} - Error: {err}");
                    err
                })
                .ok(),

            Venue::KrakenSpot => serde_json::from_str::<KrakenSpotBookMessage>(json)
                .map(SupportedMessages::KrakenSpotBookMessage)
                .or_else(|_| {
                    serde_json::from_str::<KrakenSpotTradeMessage>(json)
                        .map(SupportedMessages::KrakenSpotTradeMessage)
                })
                .map_err(|err| {
                    // Log unrecognized message for debugging purposes
                    debug!("Unrecognized kraken-spot message format: {json} - Error: {err}");
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

