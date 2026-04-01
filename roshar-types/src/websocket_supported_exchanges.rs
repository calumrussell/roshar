#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebsocketSupportedExchanges {
    Hyperliquid,
    ByBit,
    Kraken,
    Binance,
    Okx,
    KuCoin,
    Bitget,
    Coinbase,
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
            WebsocketSupportedExchanges::KuCoin => {
                crate::kucoin::KuCoinWssMessage::ping().to_json()
            }
            WebsocketSupportedExchanges::Bitget => crate::bitget::BitgetWssMessage::ping(),
            WebsocketSupportedExchanges::Coinbase => String::new(),
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
            WebsocketSupportedExchanges::KuCoin => {
                crate::kucoin::KuCoinWssMessage::depth(coin).to_json()
            }
            WebsocketSupportedExchanges::Bitget => {
                crate::bitget::BitgetWssMessage::depth(coin).to_json()
            }
            WebsocketSupportedExchanges::Coinbase => {
                crate::coinbase::CoinbaseWssMessage::depth(coin).to_json()
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
            WebsocketSupportedExchanges::KuCoin => {
                crate::kucoin::KuCoinWssMessage::trades(coin).to_json()
            }
            WebsocketSupportedExchanges::Bitget => {
                crate::bitget::BitgetWssMessage::trades(coin).to_json()
            }
            WebsocketSupportedExchanges::Coinbase => {
                crate::coinbase::CoinbaseWssMessage::trades(coin).to_json()
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
            WebsocketSupportedExchanges::Kraken
            | WebsocketSupportedExchanges::Okx
            | WebsocketSupportedExchanges::KuCoin
            | WebsocketSupportedExchanges::Bitget
            | WebsocketSupportedExchanges::Coinbase => None,
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

    /// Get the websocket URL for this exchange.
    ///
    /// Note: KuCoin requires a short-lived token appended as a query parameter.
    /// Use `KuCoinClient::get_ws_url()` to obtain the full authenticated URL.
    /// The value returned here is the base server endpoint without a token.
    pub fn websocket_url(&self) -> &'static str {
        match self {
            WebsocketSupportedExchanges::Hyperliquid => "wss://api.hyperliquid.xyz/ws",
            WebsocketSupportedExchanges::ByBit => "wss://stream.bybit.com/v5/public/linear",
            WebsocketSupportedExchanges::Kraken => "wss://futures.kraken.com/ws/v1",
            WebsocketSupportedExchanges::Binance => "wss://fstream.binance.com/ws",
            WebsocketSupportedExchanges::Okx => "wss://ws.okx.com:8443/ws/v5/public",
            WebsocketSupportedExchanges::KuCoin => "wss://ws-api-futures.kucoin.com/endpoint",
            WebsocketSupportedExchanges::Bitget => "wss://ws.bitget.com/v2/ws/public",
            WebsocketSupportedExchanges::Coinbase => "wss://advanced-trade-ws.coinbase.com",
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
            // KuCoin recommends sending a ping every 18 seconds (pingInterval: 18000ms)
            WebsocketSupportedExchanges::KuCoin => 18,
            WebsocketSupportedExchanges::Bitget => 30,
            WebsocketSupportedExchanges::Coinbase => 30,
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
            WebsocketSupportedExchanges::KuCoin => 10,
            WebsocketSupportedExchanges::Bitget => 15,
            WebsocketSupportedExchanges::Coinbase => 15,
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
            WebsocketSupportedExchanges::KuCoin => 5000,
            WebsocketSupportedExchanges::Bitget => 5000,
            WebsocketSupportedExchanges::Coinbase => 5000,
        }
    }
}
