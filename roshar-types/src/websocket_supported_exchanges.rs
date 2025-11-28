pub enum WebsocketSupportedExchanges {
    Hyperliquid,
    ByBit,
    Kraken,
    Binance,
}

impl WebsocketSupportedExchanges {
    pub fn ping(&self) -> String {
        match self {
            WebsocketSupportedExchanges::Hyperliquid => crate::hyperliquid::HyperliquidWssMessage::ping().to_json(),
            WebsocketSupportedExchanges::Kraken => crate::kraken::KrakenWssMessage::ping().to_json(),
            WebsocketSupportedExchanges::ByBit => crate::bybit::ByBitWssMessage::ping().to_json(),
            WebsocketSupportedExchanges::Binance => crate::binance::BinanceWssMessage::ping().to_json(),
        }
    }

    pub fn depth(&self, coin: &str) -> String {
        match self {
            WebsocketSupportedExchanges::Hyperliquid => crate::hyperliquid::HyperliquidWssMessage::l2_book(coin).to_json(),
            WebsocketSupportedExchanges::Kraken => crate::kraken::KrakenWssMessage::depth(coin).to_json(),
            WebsocketSupportedExchanges::ByBit => crate::bybit::ByBitWssMessage::depth(coin).to_json(),
            WebsocketSupportedExchanges::Binance => crate::binance::BinanceWssMessage::depth(coin).to_json(),
        }
    }

    pub fn trades(&self, coin: &str) -> String {
        match self {
            WebsocketSupportedExchanges::Hyperliquid => crate::hyperliquid::HyperliquidWssMessage::trades(coin).to_json(),
            WebsocketSupportedExchanges::Kraken => crate::kraken::KrakenWssMessage::trades(coin).to_json(),
            WebsocketSupportedExchanges::ByBit => crate::bybit::ByBitWssMessage::trades(coin).to_json(),
            WebsocketSupportedExchanges::Binance => crate::binance::BinanceWssMessage::trades(coin).to_json(),
        }
    }

    pub fn candle(&self, coin: &str) -> Option<String> {
        match self {
            WebsocketSupportedExchanges::Hyperliquid => Some(crate::hyperliquid::HyperliquidWssMessage::candle(coin).to_json()),
            WebsocketSupportedExchanges::ByBit => Some(crate::bybit::ByBitWssMessage::candle(coin).to_json()),
            WebsocketSupportedExchanges::Binance => Some(crate::binance::BinanceWssMessage::candle(coin).to_json()),
            _ => None,
        }
    }

    /// Get the websocket URL for this exchange
    pub fn websocket_url(&self) -> &'static str {
        match self {
            WebsocketSupportedExchanges::Hyperliquid => "wss://api.hyperliquid.xyz/ws",
            WebsocketSupportedExchanges::ByBit => "wss://stream.bybit.com/v5/public/linear",
            WebsocketSupportedExchanges::Kraken => "wss://futures.kraken.com/ws/v1",
            WebsocketSupportedExchanges::Binance => "wss://fstream.binance.com/ws",
        }
    }

    /// Get the default ping interval in seconds for this exchange
    pub fn default_ping_interval(&self) -> u64 {
        match self {
            WebsocketSupportedExchanges::Hyperliquid => 20,
            WebsocketSupportedExchanges::ByBit => 20,
            WebsocketSupportedExchanges::Kraken => 30,
            WebsocketSupportedExchanges::Binance => 30,
        }
    }

    /// Get the default ping timeout in seconds for this exchange
    pub fn default_ping_timeout(&self) -> u64 {
        match self {
            WebsocketSupportedExchanges::Hyperliquid => 10,
            WebsocketSupportedExchanges::ByBit => 10,
            WebsocketSupportedExchanges::Kraken => 15,
            WebsocketSupportedExchanges::Binance => 15,
        }
    }

    /// Get the default reconnect timeout in milliseconds for this exchange
    pub fn default_reconnect_timeout(&self) -> u64 {
        match self {
            WebsocketSupportedExchanges::Hyperliquid => 5000,
            WebsocketSupportedExchanges::ByBit => 5000,
            WebsocketSupportedExchanges::Kraken => 10000,
            WebsocketSupportedExchanges::Binance => 5000,
        }
    }
}
