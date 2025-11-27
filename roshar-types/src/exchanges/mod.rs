pub mod binance;
pub mod bybit;
pub mod bybitspot;
pub mod hyperliquid;
pub mod kraken;
pub mod krakenspot;
pub mod mex;

use anyhow::{Result, anyhow};

pub use binance::*;
pub use bybit::*;
pub use bybitspot::*;
pub use hyperliquid::*;
pub use kraken::*;
pub use krakenspot::*;
pub use mex::*;

macro_rules! exchange_method_cant_fail {
    ($self:expr, $method:ident) => {
        match $self {
            WebsocketSupportedExchanges::Hyperliquid => Ok(Hyperliquid::$method()),
            WebsocketSupportedExchanges::Kraken => Ok(Kraken::$method()),
            WebsocketSupportedExchanges::KrakenSpot => Ok(KrakenSpot::$method()),
            WebsocketSupportedExchanges::ByBit => Ok(ByBit::$method()),
            WebsocketSupportedExchanges::ByBitSpot => Ok(ByBitSpot::$method()),
            WebsocketSupportedExchanges::Mexc => Ok(Mex::$method()),
            WebsocketSupportedExchanges::Binance => Ok(Binance::$method()),
        }
    };
    ($self:expr, $method:ident, $arg:expr) => {
        match $self {
            WebsocketSupportedExchanges::Hyperliquid => Ok(Hyperliquid::$method($arg)),
            WebsocketSupportedExchanges::Kraken => Ok(Kraken::$method($arg)),
            WebsocketSupportedExchanges::KrakenSpot => Ok(KrakenSpot::$method($arg)),
            WebsocketSupportedExchanges::ByBit => Ok(ByBit::$method($arg)),
            WebsocketSupportedExchanges::ByBitSpot => Ok(ByBitSpot::$method($arg)),
            WebsocketSupportedExchanges::Mexc => Ok(Mex::$method($arg)),
            WebsocketSupportedExchanges::Binance => Ok(Binance::$method($arg)),
        }
    };
}

pub enum WebsocketSupportedExchanges {
    Hyperliquid,
    ByBit,
    ByBitSpot,
    Kraken,
    KrakenSpot,
    Mexc,
    Binance,
}

impl WebsocketSupportedExchanges {
    pub fn ping(&self) -> Result<String> {
        exchange_method_cant_fail!(self, ping)
    }

    pub fn depth(&self, coin: &str) -> Result<String> {
        exchange_method_cant_fail!(self, depth, coin)
    }

    pub fn trades(&self, coin: &str) -> Result<String> {
        exchange_method_cant_fail!(self, trades, coin)
    }

    pub fn candle(&self, coin: &str) -> Result<String> {
        match self {
            WebsocketSupportedExchanges::Hyperliquid => Ok(Hyperliquid::candle(coin)),
            WebsocketSupportedExchanges::ByBit => Ok(ByBit::candle(coin)),
            _ => Err(anyhow!("Candle operation not supported for this exchange")),
        }
    }

    /// Get the websocket URL for this exchange
    pub fn websocket_url(&self) -> &'static str {
        match self {
            WebsocketSupportedExchanges::Hyperliquid => "wss://api.hyperliquid.xyz/ws",
            WebsocketSupportedExchanges::ByBit => "wss://stream.bybit.com/v5/public/linear",
            WebsocketSupportedExchanges::ByBitSpot => "wss://stream.bybit.com/v5/public/spot",
            WebsocketSupportedExchanges::Kraken => "wss://futures.kraken.com/ws/v1",
            WebsocketSupportedExchanges::KrakenSpot => "wss://ws.kraken.com/v2",
            WebsocketSupportedExchanges::Mexc => "wss://wbs.mexc.com/ws",
            WebsocketSupportedExchanges::Binance => "wss://fstream.binance.com/ws",
        }
    }

    /// Get the default ping interval in seconds for this exchange
    pub fn default_ping_interval(&self) -> u64 {
        match self {
            WebsocketSupportedExchanges::Hyperliquid => 20,
            WebsocketSupportedExchanges::ByBit => 20,
            WebsocketSupportedExchanges::ByBitSpot => 20,
            WebsocketSupportedExchanges::Kraken => 30,
            WebsocketSupportedExchanges::KrakenSpot => 30,
            WebsocketSupportedExchanges::Mexc => 30,
            WebsocketSupportedExchanges::Binance => 30,
        }
    }

    /// Get the default ping timeout in seconds for this exchange
    pub fn default_ping_timeout(&self) -> u64 {
        match self {
            WebsocketSupportedExchanges::Hyperliquid => 10,
            WebsocketSupportedExchanges::ByBit => 10,
            WebsocketSupportedExchanges::ByBitSpot => 10,
            WebsocketSupportedExchanges::Kraken => 15,
            WebsocketSupportedExchanges::KrakenSpot => 15,
            WebsocketSupportedExchanges::Mexc => 15,
            WebsocketSupportedExchanges::Binance => 15,
        }
    }

    /// Get the default reconnect timeout in milliseconds for this exchange
    pub fn default_reconnect_timeout(&self) -> u64 {
        match self {
            WebsocketSupportedExchanges::Hyperliquid => 5000,
            WebsocketSupportedExchanges::ByBit => 5000,
            WebsocketSupportedExchanges::ByBitSpot => 5000,
            WebsocketSupportedExchanges::Kraken => 10000,
            WebsocketSupportedExchanges::KrakenSpot => 10000,
            WebsocketSupportedExchanges::Mexc => 5000,
            WebsocketSupportedExchanges::Binance => 5000,
        }
    }
}
