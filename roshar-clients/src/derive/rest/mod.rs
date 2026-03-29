mod market;

pub use market::{
    DeriveInstrument, DeriveFundingRate, DeriveFundingRateHistoryResponse, DeriveInstrumentsResponse,
    DeriveOptionPricing, DerivePerpDetails, DeriveStats, DeriveTicker, DeriveTickersResponse,
    MarketApi,
};

pub(crate) const DERIVE_REST_URL: &str = "https://api.lyra.finance";
