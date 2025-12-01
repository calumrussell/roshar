#![allow(clippy::too_many_arguments)]

mod auth;
mod charts;
mod market;
mod multi_collateral;
mod order_management;

pub use charts::ChartsApi;
pub use market::{KrakenRestCandleData, KrakenRestCandleResponse, KrakenTickerData, MarketApi};
pub use multi_collateral::{
    KrakenGetLeverageResponse, KrakenLeveragePreference, KrakenLeverageSettingResponse,
    MultiCollateralApi,
};
pub use order_management::{
    KrakenCancelResponse, KrakenDeadMansSwitchResponse, KrakenModifyResponse,
    KrakenOpenOrdersResponse, KrakenOrder, KrakenOrderResponse, KrakenOrderStatusResponse,
    OrderManagementApi,
};
