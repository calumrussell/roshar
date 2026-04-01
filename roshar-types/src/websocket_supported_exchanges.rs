#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebsocketSupportedExchanges {
    Hyperliquid,
    ByBit,
    Kraken,
    Binance,
    Okx,
    Aster,
    Pacifica,
    Lighter,
    Paradex,
}

impl WebsocketSupportedExchanges {
    pub fn ping(&self) -> String {
        match self {
            WebsocketSupportedExchanges::Hyperliquid => {
                crate::hyperliquid::HyperliquidWssMessage::ping().to_json()
            }
            WebsocketSupportedExchanges::Kraken => {
                crate::kraken::KrakenWssMessage::ping().to_json()
            }
            WebsocketSupportedExchanges::ByBit => crate::bybit::ByBitWssMessage::ping().to_json(),
            WebsocketSupportedExchanges::Binance => {
                crate::binance::BinanceWssMessage::ping().to_json()
            }
            WebsocketSupportedExchanges::Okx => crate::okx::OkxWssMessage::ping(),
            WebsocketSupportedExchanges::Aster => {
                crate::aster::AsterWssMessage::ping().to_json()
            }
            WebsocketSupportedExchanges::Pacifica => {
                crate::pacifica::PacificaWssMessage::ping().to_json()
            }
            WebsocketSupportedExchanges::Lighter => {
                crate::lighter::LighterWssMessage::ping().to_json()
            }
            WebsocketSupportedExchanges::Paradex => {
                crate::paradex::ParadexWssMessage::ping().to_json()
            }
        }
    }

    pub fn depth(&self, coin: &str) -> String {
        match self {
            WebsocketSupportedExchanges::Hyperliquid => {
                crate::hyperliquid::HyperliquidWssMessage::l2_book(coin).to_json()
            }
            WebsocketSupportedExchanges::Kraken => {
                crate::kraken::KrakenWssMessage::depth(coin).to_json()
            }
            WebsocketSupportedExchanges::ByBit => {
                crate::bybit::ByBitWssMessage::depth(coin).to_json()
            }
            WebsocketSupportedExchanges::Binance => {
                crate::binance::BinanceWssMessage::batch_depth(&[coin.to_string()]).to_json()
            }
            WebsocketSupportedExchanges::Okx => {
                crate::okx::OkxWssMessage::depth(coin).to_json()
            }
            WebsocketSupportedExchanges::Aster => {
                crate::aster::AsterWssMessage::depth(coin).to_json()
            }
            WebsocketSupportedExchanges::Pacifica => {
                crate::pacifica::PacificaWssMessage::depth(coin).to_json()
            }
            // Lighter uses integer market indices rather than symbol strings;
            // callers should use LighterClient::subscribe_depth directly.
            WebsocketSupportedExchanges::Lighter => {
                crate::lighter::LighterWssMessage::depth(0).to_json()
            }
            WebsocketSupportedExchanges::Paradex => {
                crate::paradex::ParadexWssMessage::depth(coin).to_json()
            }
        }
    }

    pub fn trades(&self, coin: &str) -> String {
        match self {
            WebsocketSupportedExchanges::Hyperliquid => {
                crate::hyperliquid::HyperliquidWssMessage::trades(coin).to_json()
            }
            WebsocketSupportedExchanges::Kraken => {
                crate::kraken::KrakenWssMessage::trades(coin).to_json()
            }
            WebsocketSupportedExchanges::ByBit => {
                crate::bybit::ByBitWssMessage::trades(coin).to_json()
            }
            WebsocketSupportedExchanges::Binance => {
                crate::binance::BinanceWssMessage::batch_trades(&[coin.to_string()]).to_json()
            }
            WebsocketSupportedExchanges::Okx => {
                crate::okx::OkxWssMessage::trades(coin).to_json()
            }
            WebsocketSupportedExchanges::Aster => {
                crate::aster::AsterWssMessage::trades(coin).to_json()
            }
            WebsocketSupportedExchanges::Pacifica => {
                crate::pacifica::PacificaWssMessage::trades(coin).to_json()
            }
            WebsocketSupportedExchanges::Lighter => {
                crate::lighter::LighterWssMessage::trades(0).to_json()
            }
            WebsocketSupportedExchanges::Paradex => {
                crate::paradex::ParadexWssMessage::trades(coin).to_json()
            }
        }
    }

    pub fn candle(&self, coin: &str) -> Option<String> {
        match self {
            WebsocketSupportedExchanges::Hyperliquid => {
                Some(crate::hyperliquid::HyperliquidWssMessage::candle(coin).to_json())
            }
            WebsocketSupportedExchanges::ByBit => {
                Some(crate::bybit::ByBitWssMessage::candle(coin).to_json())
            }
            WebsocketSupportedExchanges::Binance => Some(
                crate::binance::BinanceWssMessage::batch_candles(&[coin.to_string()]).to_json(),
            ),
            WebsocketSupportedExchanges::Aster => {
                Some(crate::aster::AsterWssMessage::klines(coin).to_json())
            }
            WebsocketSupportedExchanges::Pacifica => {
                Some(crate::pacifica::PacificaWssMessage::candles(coin).to_json())
            }
            _ => None,
        }
    }

    /// Subscribe to depth for multiple symbols in a single message (Binance only)
    /// Other exchanges don't support batching and will return None
    pub fn batch_depth(&self, coins: &[String]) -> Option<String> {
        match self {
            WebsocketSupportedExchanges::Binance => {
                Some(crate::binance::BinanceWssMessage::batch_depth(coins).to_json())
            }
            _ => None,
        }
    }

    /// Subscribe to trades for multiple symbols in a single message (Binance only)
    /// Other exchanges don't support batching and will return None
    pub fn batch_trades(&self, coins: &[String]) -> Option<String> {
        match self {
            WebsocketSupportedExchanges::Binance => {
                Some(crate::binance::BinanceWssMessage::batch_trades(coins).to_json())
            }
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
            WebsocketSupportedExchanges::Okx => "wss://ws.okx.com:8443/ws/v5/public",
            WebsocketSupportedExchanges::Aster => "wss://fstream.asterdex.com",
            WebsocketSupportedExchanges::Pacifica => "wss://ws.pacifica.fi/ws",
            WebsocketSupportedExchanges::Lighter => "wss://mainnet.zklighter.elliot.ai/stream",
            WebsocketSupportedExchanges::Paradex => "wss://ws.api.prod.paradex.trade/v1",
        }
    }

    /// Get the default ping interval in seconds for this exchange
    pub fn default_ping_interval(&self) -> u64 {
        match self {
            WebsocketSupportedExchanges::Hyperliquid => 20,
            WebsocketSupportedExchanges::ByBit => 20,
            WebsocketSupportedExchanges::Kraken => 30,
            WebsocketSupportedExchanges::Binance => 30,
            WebsocketSupportedExchanges::Okx => 20,
            WebsocketSupportedExchanges::Aster => 10,
            WebsocketSupportedExchanges::Pacifica => 30,
            WebsocketSupportedExchanges::Lighter => 90,
            WebsocketSupportedExchanges::Paradex => 30,
        }
    }

    /// Get the default ping timeout in seconds for this exchange
    pub fn default_ping_timeout(&self) -> u64 {
        match self {
            WebsocketSupportedExchanges::Hyperliquid => 10,
            WebsocketSupportedExchanges::ByBit => 10,
            WebsocketSupportedExchanges::Kraken => 15,
            WebsocketSupportedExchanges::Binance => 15,
            WebsocketSupportedExchanges::Okx => 10,
            WebsocketSupportedExchanges::Aster => 10,
            WebsocketSupportedExchanges::Pacifica => 10,
            WebsocketSupportedExchanges::Lighter => 10,
            WebsocketSupportedExchanges::Paradex => 10,
        }
    }

    /// Get the default reconnect timeout in milliseconds for this exchange
    pub fn default_reconnect_timeout(&self) -> u64 {
        match self {
            WebsocketSupportedExchanges::Hyperliquid => 5000,
            WebsocketSupportedExchanges::ByBit => 5000,
            WebsocketSupportedExchanges::Kraken => 10000,
            WebsocketSupportedExchanges::Binance => 5000,
            WebsocketSupportedExchanges::Okx => 5000,
            WebsocketSupportedExchanges::Aster => 5000,
            WebsocketSupportedExchanges::Pacifica => 5000,
            WebsocketSupportedExchanges::Lighter => 5000,
            WebsocketSupportedExchanges::Paradex => 5000,
        }
    }
}
