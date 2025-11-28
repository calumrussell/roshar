pub mod binance;
pub mod bybit;
pub mod bybitspot;
pub mod kraken;
pub mod krakenspot;

pub use binance::*;
pub use bybit::*;
pub use bybitspot::*;
pub use kraken::*;
pub use krakenspot::*;

pub enum WebsocketSupportedExchanges {
    Hyperliquid,
    ByBit,
    ByBitSpot,
    Kraken,
    KrakenSpot,
    Binance,
}

impl WebsocketSupportedExchanges {
    pub fn ping(&self) -> String {
        match self {
            WebsocketSupportedExchanges::Hyperliquid => crate::hyperliquid::HyperliquidWssMessage::ping().to_json(),
            WebsocketSupportedExchanges::Kraken => kraken::KrakenWssMessage::ping().to_json(),
            WebsocketSupportedExchanges::KrakenSpot => krakenspot::WssApi::ping(),
            WebsocketSupportedExchanges::ByBit => bybit::ByBitWssMessage::ping().to_json(),
            WebsocketSupportedExchanges::ByBitSpot => bybitspot::WssApi::ping(),
            WebsocketSupportedExchanges::Binance => binance::BinanceWssMessage::ping().to_json(),
        }
    }

    pub fn depth(&self, coin: &str) -> String {
        match self {
            WebsocketSupportedExchanges::Hyperliquid => crate::hyperliquid::HyperliquidWssMessage::l2_book(coin).to_json(),
            WebsocketSupportedExchanges::Kraken => kraken::KrakenWssMessage::depth(coin).to_json(),
            WebsocketSupportedExchanges::KrakenSpot => krakenspot::WssApi::depth(coin),
            WebsocketSupportedExchanges::ByBit => bybit::ByBitWssMessage::depth(coin).to_json(),
            WebsocketSupportedExchanges::ByBitSpot => bybitspot::WssApi::depth(coin),
            WebsocketSupportedExchanges::Binance => binance::BinanceWssMessage::depth(coin).to_json(),
        }
    }

    pub fn trades(&self, coin: &str) -> String {
        match self {
            WebsocketSupportedExchanges::Hyperliquid => crate::hyperliquid::HyperliquidWssMessage::trades(coin).to_json(),
            WebsocketSupportedExchanges::Kraken => kraken::KrakenWssMessage::trades(coin).to_json(),
            WebsocketSupportedExchanges::KrakenSpot => krakenspot::WssApi::trades(coin),
            WebsocketSupportedExchanges::ByBit => bybit::ByBitWssMessage::trades(coin).to_json(),
            WebsocketSupportedExchanges::ByBitSpot => bybitspot::WssApi::trades(coin),
            WebsocketSupportedExchanges::Binance => binance::BinanceWssMessage::trades(coin).to_json(),
        }
    }

    pub fn candle(&self, coin: &str) -> Option<String> {
        match self {
            WebsocketSupportedExchanges::Hyperliquid => Some(crate::hyperliquid::HyperliquidWssMessage::candle(coin).to_json()),
            WebsocketSupportedExchanges::ByBit => Some(bybit::ByBitWssMessage::candle(coin).to_json()),
            WebsocketSupportedExchanges::Binance => Some(binance::BinanceWssMessage::candle(coin).to_json()),
            _ => None,
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
            WebsocketSupportedExchanges::Binance => 5000,
        }
    }
}
