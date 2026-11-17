mod market;

pub use market::{
    BoxError, MarketApi, OkxInstrumentsResponse, OkxResponse, OkxTickerData, OkxTickersResponse,
};

/// OKX Global REST domain (recommended since 2026-05-20; `www.okx.com` still works).
pub(crate) const OKX_REST_URL: &str = "https://openapi.okx.com";
