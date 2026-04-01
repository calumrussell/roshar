pub mod binance;
pub mod bitget;
pub mod bybit;
pub mod coinbase;
pub mod constants;
pub mod http;
pub mod hyperliquid;
pub mod kraken;
pub mod kucoin;
pub mod okx;
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
