pub mod binance;
pub mod bybit;
pub mod constants;
pub mod http;
pub mod hyperliquid;
pub mod kraken;
mod state_handle;
mod state_manager;

// Re-export URL constants
pub use constants::*;

// Re-export commonly used types
pub use hyperliquid::rest::{
    ExchangeApi, ExchangeDataStatus, ExchangeResponseStatus, HyperliquidOrderType, InfoApi,
    ModifyOrderParams,
};
pub use hyperliquid::validator::{OrderRequest, ValidatedOrder};
pub use hyperliquid::{HyperliquidClient, HyperliquidConfig, MarketEvent};
pub use state_manager::{PendingOrderInfo, PositionState};

// Re-export Binance types
pub use binance::{BinanceClient, BinanceRestClient};
pub use binance::{MarketEvent as BinanceMarketEvent};

// Re-export ByBit types
pub use bybit::{
    ByBitClient, ByBitCreateOrderRequest, ByBitCreateOrderResponse, ByBitTickerData,
    ByBitTickersResponse, MarketApi as ByBitMarketApi, MarketEvent as ByBitMarketEvent,
    OrderManagementApi as ByBitOrderManagementApi,
};

// Re-export Kraken types
pub use kraken::{
    ChartsApi as KrakenChartsApi, KrakenClient, KrakenGetLeverageResponse, KrakenLeveragePreference,
    KrakenLeverageSettingResponse, KrakenModifyResponse, KrakenOpenOrdersResponse, KrakenOrder,
    KrakenOrderResponse, KrakenOrderStatusResponse, KrakenRestCandleData, KrakenRestCandleResponse,
    KrakenTickerData, MarketApi as KrakenMarketApi, MarketEvent as KrakenMarketEvent,
    MultiCollateralApi as KrakenMultiCollateralApi, OrderManagementApi as KrakenOrderManagementApi,
};
