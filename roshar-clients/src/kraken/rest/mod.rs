#![allow(clippy::too_many_arguments)]

mod auth;
mod charts;
mod market;
mod multi_collateral;
mod order_management;

pub(crate) use charts::ChartsApi;
pub(crate) use market::MarketApi;
pub use market::{KrakenRestCandleData, KrakenRestCandleResponse, KrakenTickerData};
pub use multi_collateral::{
    KrakenGetLeverageResponse, KrakenLeveragePreference, KrakenLeverageSettingResponse,
    MultiCollateralApi,
};
pub use order_management::{
    KrakenCancelResponse, KrakenDeadMansSwitchResponse, KrakenModifyResponse,
    KrakenOpenOrdersResponse, KrakenOrder, KrakenOrderResponse, KrakenOrderStatusResponse,
    OrderManagementApi,
};
