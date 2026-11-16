mod market;

pub use market::{
    DeriveAllInstrumentsResponse, DeriveAssetEntry, DeriveAssetErc20, DeriveAssetUniverse,
    DeriveCurrency, DeriveCurrencyManagers, DeriveErc20Details, DeriveForwardFeedData,
    DeriveFundingFeedData, DeriveFundingRate, DeriveFundingRateHistoryResponse, DeriveIncident,
    DeriveIndexCandle, DeriveInstrument, DeriveInterestRateCandle, DeriveLendingDetails,
    DeriveLiquidationAuction, DeriveLiquidationBid, DeriveLiquidationHistoryResponse,
    DeriveLiveAuction, DeriveManagerCollateral, DeriveOhlc, DeriveOpenInterestStats,
    DeriveOptionDetails, DeriveOptionPricing, DeriveOptionSettlement, DeriveOracleSignatures,
    DerivePagination, DerivePerpDetails, DerivePerpFeedData, DerivePerpImpactTwap,
    DeriveRateFeedData, DeriveRiskUniverse, DeriveRiskUniverseManager, DeriveSecurityModule,
    DeriveSignedFeeds, DeriveSpotAsset, DeriveSpotFeedData, DeriveSpotUniverse,
    DeriveSrmPerpMarginRequirements, DeriveStats, DeriveSviParams, DeriveTicker,
    DeriveTickersResponse, DeriveTrade, DeriveTradeCandle, DeriveTradeHistoryParams,
    DeriveTradeHistoryResponse, DeriveVolFeedData, MarketApi,
};

pub(crate) const DERIVE_REST_URL: &str = "https://api.derive.xyz/v3";
