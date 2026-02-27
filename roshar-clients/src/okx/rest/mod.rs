mod market;

pub use market::{MarketApi, OkxInstrumentsResponse, OkxTickerData, OkxTickersResponse};

pub(crate) const OKX_REST_URL: &str = "https://www.okx.com";
