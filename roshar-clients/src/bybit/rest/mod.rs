mod auth;
mod market;
mod order_management;

pub use market::{ByBitTickerData, ByBitTickersResponse, MarketApi};
pub use order_management::{
    ByBitCreateOrderRequest, ByBitCreateOrderResponse, ByBitOrderResult, OrderManagementApi,
};

pub(crate) const BYBIT_REST_URL: &str = "https://api.bybit.com";
