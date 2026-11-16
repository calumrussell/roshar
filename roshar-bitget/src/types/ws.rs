use super::{BitgetProductType, BitgetSpotAuction};
use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Deserializer, Serialize};

/// `instType` of a public websocket subscription.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BitgetWsInstType {
    UsdtFutures,
    CoinFutures,
    UsdcFutures,
    Spot,
}

impl BitgetWsInstType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::UsdtFutures => "USDT-FUTURES",
            Self::CoinFutures => "COIN-FUTURES",
            Self::UsdcFutures => "USDC-FUTURES",
            Self::Spot => "SPOT",
        }
    }
}

impl From<BitgetProductType> for BitgetWsInstType {
    fn from(p: BitgetProductType) -> Self {
        match p {
            BitgetProductType::UsdtFutures => Self::UsdtFutures,
            BitgetProductType::CoinFutures => Self::CoinFutures,
            BitgetProductType::UsdcFutures => Self::UsdcFutures,
        }
    }
}

/// Order book channels. `books` sends a snapshot then incremental updates; `books1`, `books5`
/// and `books15` send a full snapshot of that many levels on every push.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BitgetBookChannel {
    Books,
    Books1,
    Books5,
    Books15,
}

impl BitgetBookChannel {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Books => "books",
            Self::Books1 => "books1",
            Self::Books5 => "books5",
            Self::Books15 => "books15",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BitgetWssArg {
    #[serde(rename = "instType")]
    pub inst_type: String,
    pub channel: String,
    #[serde(rename = "instId")]
    pub inst_id: String,
}

/// Outgoing subscribe/unsubscribe request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BitgetWssMessage {
    pub op: String,
    pub args: Vec<BitgetWssArg>,
}

impl BitgetWssMessage {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("failed to serialize BitgetWssMessage")
    }

    /// Bitget uses a plain text "ping" (server responds with "pong")
    pub fn ping() -> String {
        "ping".to_string()
    }

    fn single(op: &str, inst_type: BitgetWsInstType, channel: &str, symbol: &str) -> Self {
        Self {
            op: op.to_string(),
            args: vec![BitgetWssArg {
                inst_type: inst_type.as_str().to_string(),
                channel: channel.to_string(),
                inst_id: symbol.to_string(),
            }],
        }
    }

    /// Subscribe to any public channel by name.
    pub fn subscribe(inst_type: impl Into<BitgetWsInstType>, channel: &str, symbol: &str) -> Self {
        Self::single("subscribe", inst_type.into(), channel, symbol)
    }

    /// Unsubscribe from any public channel by name.
    pub fn unsubscribe(
        inst_type: impl Into<BitgetWsInstType>,
        channel: &str,
        symbol: &str,
    ) -> Self {
        Self::single("unsubscribe", inst_type.into(), channel, symbol)
    }

    /// Full orderbook (`books` channel: a snapshot, then incremental updates)
    pub fn depth(inst_type: impl Into<BitgetWsInstType>, symbol: &str) -> Self {
        Self::books(inst_type, BitgetBookChannel::Books, symbol)
    }

    pub fn depth_unsub(inst_type: impl Into<BitgetWsInstType>, symbol: &str) -> Self {
        Self::books_unsub(inst_type, BitgetBookChannel::Books, symbol)
    }

    pub fn books(
        inst_type: impl Into<BitgetWsInstType>,
        channel: BitgetBookChannel,
        symbol: &str,
    ) -> Self {
        Self::subscribe(inst_type, channel.as_str(), symbol)
    }

    pub fn books_unsub(
        inst_type: impl Into<BitgetWsInstType>,
        channel: BitgetBookChannel,
        symbol: &str,
    ) -> Self {
        Self::unsubscribe(inst_type, channel.as_str(), symbol)
    }

    pub fn trades(inst_type: impl Into<BitgetWsInstType>, symbol: &str) -> Self {
        Self::subscribe(inst_type, "trade", symbol)
    }

    pub fn trades_unsub(inst_type: impl Into<BitgetWsInstType>, symbol: &str) -> Self {
        Self::unsubscribe(inst_type, "trade", symbol)
    }

    pub fn tickers(inst_type: impl Into<BitgetWsInstType>, symbol: &str) -> Self {
        Self::subscribe(inst_type, "ticker", symbol)
    }

    pub fn tickers_unsub(inst_type: impl Into<BitgetWsInstType>, symbol: &str) -> Self {
        Self::unsubscribe(inst_type, "ticker", symbol)
    }

    /// `candle<interval>`, e.g. interval `"1m"` -> channel `candle1m`. Valid intervals are listed
    /// on Bitget's candlestick channel pages (futures and spot lists differ slightly).
    pub fn candles(inst_type: impl Into<BitgetWsInstType>, interval: &str, symbol: &str) -> Self {
        Self::subscribe(inst_type, &format!("candle{interval}"), symbol)
    }

    pub fn candles_unsub(
        inst_type: impl Into<BitgetWsInstType>,
        interval: &str,
        symbol: &str,
    ) -> Self {
        Self::unsubscribe(inst_type, &format!("candle{interval}"), symbol)
    }

    /// Spot call auction (`auction` channel).
    pub fn auction(symbol: &str) -> Self {
        Self::subscribe(BitgetWsInstType::Spot, "auction", symbol)
    }

    pub fn auction_unsub(symbol: &str) -> Self {
        Self::unsubscribe(BitgetWsInstType::Spot, "auction", symbol)
    }
}

/// `action` of a data push.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BitgetWsAction {
    Snapshot,
    Update,
}

/// A data push: `{"action", "arg", "data": [...], "ts"}`.
#[derive(Debug, Clone, Deserialize)]
pub struct BitgetWsPush<T> {
    pub action: BitgetWsAction,
    pub arg: BitgetWssArg,
    pub data: Vec<T>,
    /// Push time, Unix ms
    pub ts: i64,
}

/// `books`, `books1`, `books5`, `books15` data (futures and spot).
#[derive(Debug, Clone, Deserialize)]
pub struct BitgetWsBook {
    /// `[price, size]`; size `"0"` on a `books` update removes the level.
    pub asks: Vec<[String; 2]>,
    pub bids: Vec<[String; 2]>,
    /// Matching engine time, Unix ms
    pub ts: String,
    pub seq: u64,
    /// `seq` of the previous `books` update; 0 on snapshots and on books1/5/15.
    pub pseq: u64,
}

/// `trade` data (futures and spot).
#[derive(Debug, Clone, Deserialize)]
pub struct BitgetWsTrade {
    /// Unix ms
    pub ts: String,
    pub price: String,
    pub size: String,
    /// "buy" / "sell"
    pub side: String,
    #[serde(rename = "tradeId")]
    pub trade_id: String,
}

/// Futures `ticker` data.
#[derive(Debug, Clone, Deserialize)]
pub struct BitgetWsFuturesTicker {
    #[serde(rename = "instId")]
    pub inst_id: String,
    pub symbol: String,
    #[serde(rename = "lastPr")]
    pub last_pr: String,
    #[serde(rename = "bidPr")]
    pub bid_pr: String,
    #[serde(rename = "askPr")]
    pub ask_pr: String,
    #[serde(rename = "bidSz")]
    pub bid_sz: String,
    #[serde(rename = "askSz")]
    pub ask_sz: String,
    #[serde(rename = "open24h")]
    pub open_24h: String,
    #[serde(rename = "high24h")]
    pub high_24h: String,
    #[serde(rename = "low24h")]
    pub low_24h: String,
    #[serde(rename = "change24h")]
    pub change_24h: String,
    #[serde(rename = "fundingRate")]
    pub funding_rate: String,
    /// Unix ms
    #[serde(rename = "nextFundingTime")]
    pub next_funding_time: String,
    #[serde(rename = "markPrice")]
    pub mark_price: String,
    #[serde(rename = "indexPrice")]
    pub index_price: String,
    /// Open interest
    #[serde(rename = "holdingAmount")]
    pub holding_amount: String,
    #[serde(rename = "baseVolume")]
    pub base_volume: String,
    #[serde(rename = "quoteVolume")]
    pub quote_volume: String,
    #[serde(rename = "openUtc")]
    pub open_utc: String,
    /// "1" perpetual, "2" delivery
    #[serde(rename = "symbolType")]
    pub symbol_type: String,
    /// Delivery price of delivery futures. Documented, but absent from live perpetual pushes.
    #[serde(rename = "deliveryPrice", default)]
    pub delivery_price: Option<String>,
    /// Unix ms
    pub ts: String,
}

/// Spot `ticker` data.
#[derive(Debug, Clone, Deserialize)]
pub struct BitgetWsSpotTicker {
    #[serde(rename = "instId")]
    pub inst_id: String,
    #[serde(rename = "lastPr")]
    pub last_pr: String,
    #[serde(rename = "open24h")]
    pub open_24h: String,
    #[serde(rename = "high24h")]
    pub high_24h: String,
    #[serde(rename = "low24h")]
    pub low_24h: String,
    #[serde(rename = "change24h")]
    pub change_24h: String,
    /// Absent (not `null`) when the bid side is empty (seen live).
    #[serde(rename = "bidPr", default)]
    pub bid_pr: Option<String>,
    /// Absent (not `null`) when the ask side is empty (seen live).
    #[serde(rename = "askPr", default)]
    pub ask_pr: Option<String>,
    /// Absent with `bidPr`.
    #[serde(rename = "bidSz", default)]
    pub bid_sz: Option<String>,
    /// Absent with `askPr`.
    #[serde(rename = "askSz", default)]
    pub ask_sz: Option<String>,
    #[serde(rename = "baseVolume")]
    pub base_volume: String,
    #[serde(rename = "quoteVolume")]
    pub quote_volume: String,
    #[serde(rename = "openUtc")]
    pub open_utc: String,
    #[serde(rename = "changeUtc24h")]
    pub change_utc_24h: String,
    /// Unix ms
    pub ts: String,
}

/// `candle*` data (futures and spot):
/// `[ts, open, high, low, close, baseVolume, quoteVolume, usdtVolume]`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(from = "[String; 8]")]
pub struct BitgetWsCandle {
    /// Start time, Unix ms
    pub ts: String,
    pub open: String,
    pub high: String,
    pub low: String,
    pub close: String,
    pub base_volume: String,
    pub quote_volume: String,
    pub usdt_volume: String,
}

impl From<[String; 8]> for BitgetWsCandle {
    fn from(a: [String; 8]) -> Self {
        let [ts, open, high, low, close, base_volume, quote_volume, usdt_volume] = a;
        Self {
            ts,
            open,
            high,
            low,
            close,
            base_volume,
            quote_volume,
            usdt_volume,
        }
    }
}

/// `{"event": "error", "code", "msg", "arg"?, "op"?}`. `code` arrives as a JSON number live
/// (documented as a string); both are accepted.
#[derive(Debug, Clone, Deserialize)]
pub struct BitgetWsError {
    #[serde(deserialize_with = "code_as_string")]
    pub code: String,
    pub msg: String,
    pub arg: Option<BitgetWssArg>,
    pub op: Option<String>,
}

fn code_as_string<'de, D: Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Code {
        Str(String),
        Num(i64),
    }
    Ok(match Code::deserialize(d)? {
        Code::Str(s) => s,
        Code::Num(n) => n.to_string(),
    })
}

#[derive(Deserialize)]
struct AckFrame {
    arg: BitgetWssArg,
}

#[derive(Deserialize)]
struct FrameHead<'a> {
    #[serde(borrow, default)]
    event: Option<&'a str>,
    #[serde(borrow, default)]
    arg: Option<HeadArg<'a>>,
}

#[derive(Deserialize)]
struct HeadArg<'a> {
    #[serde(rename = "instType", borrow)]
    inst_type: &'a str,
    #[serde(borrow)]
    channel: &'a str,
}

/// A frame received on the public websocket.
#[derive(Debug, Clone)]
pub enum BitgetWsIncoming {
    /// Text `pong` reply to the text `ping`.
    Pong,
    Subscribed(BitgetWssArg),
    Unsubscribed(BitgetWssArg),
    Error(BitgetWsError),
    /// `books`, `books1`, `books5`, `books15`
    Books(BitgetWsPush<BitgetWsBook>),
    Trades(BitgetWsPush<BitgetWsTrade>),
    FuturesTicker(BitgetWsPush<BitgetWsFuturesTicker>),
    SpotTicker(BitgetWsPush<BitgetWsSpotTicker>),
    /// `candle*`
    Candles(BitgetWsPush<BitgetWsCandle>),
    /// Spot `auction`
    Auction(BitgetWsPush<BitgetSpotAuction>),
}

impl BitgetWsIncoming {
    /// Parse one text frame. Unknown events and channels are errors.
    pub fn parse(text: &str) -> Result<Self> {
        if text == "pong" {
            return Ok(Self::Pong);
        }
        let head: FrameHead = serde_json::from_str(text)
            .map_err(|e| anyhow!("invalid Bitget websocket frame: {e}: {text}"))?;
        let parsed = match (head.event, head.arg) {
            (Some("subscribe"), _) => {
                Self::Subscribed(serde_json::from_str::<AckFrame>(text).map(|a| a.arg)?)
            }
            (Some("unsubscribe"), _) => {
                Self::Unsubscribed(serde_json::from_str::<AckFrame>(text).map(|a| a.arg)?)
            }
            (Some("error"), _) => Self::Error(serde_json::from_str(text)?),
            (Some(other), _) => bail!("unknown Bitget websocket event {other:?}: {text}"),
            (None, Some(arg)) => match arg.channel {
                "books" | "books1" | "books5" | "books15" => {
                    Self::Books(serde_json::from_str(text)?)
                }
                "trade" => Self::Trades(serde_json::from_str(text)?),
                "ticker" if arg.inst_type == "SPOT" => {
                    Self::SpotTicker(serde_json::from_str(text)?)
                }
                "ticker" => Self::FuturesTicker(serde_json::from_str(text)?),
                "auction" => Self::Auction(serde_json::from_str(text)?),
                c if c.starts_with("candle") => Self::Candles(serde_json::from_str(text)?),
                other => bail!("unknown Bitget websocket channel {other:?}: {text}"),
            },
            (None, None) => bail!("Bitget websocket frame without event or arg: {text}"),
        };
        Ok(parsed)
    }
}
