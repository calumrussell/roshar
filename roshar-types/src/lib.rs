use log::debug;

pub mod common;
pub use common::*;

pub mod http;

pub mod exchanges;
pub use exchanges::WebsocketSupportedExchanges;
pub use exchanges::binance::*;
pub use exchanges::bybit::*;
pub use exchanges::bybitspot::*;
pub use exchanges::hyperliquid::*;
pub use exchanges::kraken::*;
pub use exchanges::krakenspot::*;
pub use exchanges::mex::*;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum SupportedMessages {
    HyperliquidBookMessage(HyperliquidBookMessage),
    HyperliquidTradesMessage(HyperliquidTradesMessage),
    HyperliquidUserFillsMessage(HyperliquidUserFillsMessage),
    HyperliquidOrderUpdatesMessage(HyperliquidOrderUpdatesMessage),
    HyperliquidBboMessage(HyperliquidBboMessage),
    KrakenSpotOhlcMessage(KrakenSpotOhlcMessage),
    MexDepthMessage(MexDepthMessage),
    MexDealsMessage(MexDealsMessage),
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

            Venue::Mex => serde_json::from_str::<MexDepthMessage>(json)
                .map(SupportedMessages::MexDepthMessage)
                .or_else(|_| {
                    serde_json::from_str::<MexDealsMessage>(json)
                        .map(SupportedMessages::MexDealsMessage)
                })
                .map_err(|err| {
                    // Log unrecognized message for debugging purposes
                    debug!("Unrecognized mex message format: {json} - Error: {err}");
                    err
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

// Legacy websocket URL constants
pub const HL_WSS_URL: &str = "wss://api.hyperliquid.xyz/ws";
pub const HL_TESTNET_WSS_URL: &str = "wss://api.hyperliquid-testnet.xyz/ws";
pub const MEX_WSS_URL: &str = "wss://wbs.mexc.com/ws";
pub const HL_CANDLE_WSS_URL: &str = "wss://api.hyperliquid.xyz/ws";
pub const BYBIT_WSS_URL: &str = "wss://stream.bybit.com/v5/public/linear";
pub const BYBIT_SPOT_WSS_URL: &str = "wss://stream.bybit.com/v5/public/spot";
pub const KRAKEN_SPOT_WSS_URL: &str = "wss://ws.kraken.com/v2";
pub const KRAKEN_WSS_URL: &str = "wss://futures.kraken.com/ws/v1";
pub const BINANCE_WSS_URL: &str = "wss://fstream.binance.com/ws";
