mod auth;
mod market;
mod metadata;
mod order_management;

pub use market::{
    ByBitInstrumentInfo, ByBitLeverageFilter, ByBitLotSizeFilter, ByBitPriceFilter,
    ByBitTickerData, ByBitTickersResponse, MarketApi,
};
pub use metadata::{ExchangeMetadataHandle, ExchangeMetadataManager};
pub use order_management::{ByBitCreateOrderRequest, ByBitCreateOrderResponse, ByBitOrderResult};

pub(crate) const BYBIT_REST_URL: &str = "https://api.bybit.com";
