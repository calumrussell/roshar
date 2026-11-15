#![allow(clippy::too_many_arguments)]

mod auth;
mod charts;
mod market;
mod multi_collateral;
mod order_management;

pub use auth::KrakenCredentials;
pub(crate) use charts::ChartsApi;
pub use charts::{
    KrakenResolution, KrakenRestCandleData, KrakenRestCandleResponse, KrakenTickType,
};
pub use market::KrakenTickerData;
pub(crate) use market::MarketApi;
pub use multi_collateral::{
    KrakenGetLeverageResponse, KrakenLeveragePreference, KrakenLeverageSettingResponse,
    MultiCollateralApi,
};
pub use order_management::{
    KrakenModifyResponse, KrakenOpenOrdersResponse, KrakenOrder, KrakenOrderResponse,
    KrakenOrderStatusResponse, OrderManagementApi,
};
