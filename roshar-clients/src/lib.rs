pub mod aster;
pub mod binance;
pub mod bitget;
pub mod bybit;
pub mod coinbase;
pub mod constants;
pub mod deribit;
pub mod derive;
pub mod http;
pub mod hyperliquid;
pub mod kraken;
pub mod kucoin;
pub mod lighter;
pub mod okx;
pub mod pacifica;
pub mod paradex;
pub mod polymarket;
pub(crate) mod ws;

// Re-export URL constants
pub use constants::*;

// Re-export commonly used types
pub use hyperliquid::rest::{ExchangeDataStatus, ExchangeResponseStatus, HyperliquidOrderType};
pub use hyperliquid::validator::{OrderRequest, ValidatedOrder};
pub use hyperliquid::{HyperliquidApi, HyperliquidClient, HyperliquidConfig};

// Re-export Binance types
pub use binance::{BinanceApi, BinanceClient};
pub use roshar_types::BinancePremiumIndex;

// Re-export ByBit types
pub use bybit::{
    ByBitApi, ByBitClient, ByBitCreateOrderRequest, ByBitCreateOrderResponse,
    ByBitHistoricalFundingRate, ByBitMarketApi, ByBitTickerData, ByBitTickersResponse,
};

// Re-export Kraken types
pub use kraken::{
    KrakenApi, KrakenClient, KrakenGetLeverageResponse, KrakenLeveragePreference,
    KrakenLeverageSettingResponse, KrakenModifyResponse, KrakenOpenOrdersResponse, KrakenOrder,
    KrakenOrderResponse, KrakenOrderStatusResponse, KrakenRestCandleData, KrakenRestCandleResponse,
    KrakenTickerData, MultiCollateralApi as KrakenMultiCollateralApi,
    OrderManagementApi as KrakenOrderManagementApi,
};

// Re-export OKX types
pub use okx::{OkxApi, OkxClient, OkxMarketApi, OkxTickerData, OkxTickersResponse};

// Re-export Derive types
pub use derive::{
    DeriveApi, DeriveClient, DeriveFundingRate, DeriveInstrument, DeriveOptionPricing,
    DerivePerpDetails, DeriveStats, DeriveTicker,
};

// Re-export Polymarket types
pub use polymarket::GammaApi;

// Re-export KuCoin types
pub use kucoin::{KuCoinApi, KuCoinClient, KuCoinContract, KuCoinFundingRate, KuCoinMarketApi};

// Re-export Bitget types
pub use bitget::{
    BitgetApi, BitgetClient, BitgetContract, BitgetHistoricalFundingRate, BitgetMarketApi,
    BitgetTickerData,
};

// Re-export Coinbase types
pub use coinbase::{
    CoinbaseApi, CoinbaseClient, CoinbaseMarketApi, CoinbaseProduct, CoinbaseProductsResponse,
};

// Re-export Deribit types
pub use deribit::DeribitClient;

// Re-export Aster types
pub use aster::{AsterApi, AsterClient, AsterMarketApi};
pub use roshar_types::aster::{
    AsterFundingRate, AsterKline, AsterMarkPrice, AsterOrderBook, AsterTicker24hr,
};

// Re-export Pacifica types
pub use pacifica::{PacificaApi, PacificaClient, PacificaMarketApi};
pub use roshar_types::pacifica::{
    PacificaFundingRate, PacificaKline, PacificaMarket, PacificaOrderBook, PacificaTicker,
    PacificaTrade,
};

// Re-export Lighter types
pub use lighter::{LighterApi, LighterClient, LighterMarketApi};
pub use roshar_types::lighter::{
    LighterFundingRate, LighterMarket, LighterMarketDetail, LighterOrderBook, LighterTrade,
};

// Re-export Paradex types
pub use paradex::{ParadexApi, ParadexClient, ParadexMarketApi};
pub use roshar_types::paradex::{
    ParadexFundingEntry, ParadexKline, ParadexMarket, ParadexMarketSummary, ParadexOrderBook,
    ParadexTrade,
};
