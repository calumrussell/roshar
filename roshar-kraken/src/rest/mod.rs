#![allow(clippy::too_many_arguments)]

mod auth;
mod charts;
mod history;
mod market;
mod multi_collateral;
mod order_management;

pub use auth::KrakenCredentials;
pub(crate) use charts::ChartsApi;
pub use charts::{
    KrakenAnalytics, KrakenAnalyticsBasis, KrakenAnalyticsCvd, KrakenAnalyticsData,
    KrakenAnalyticsError, KrakenAnalyticsFunding, KrakenAnalyticsLiquidityPool,
    KrakenAnalyticsLiquiditySide, KrakenAnalyticsLongShort, KrakenAnalyticsOrderbookSide,
    KrakenAnalyticsSides, KrakenAnalyticsSlippageSide, KrakenAnalyticsSpreadsSide,
    KrakenAnalyticsTopTraders, KrakenAnalyticsTopTradersLevel, KrakenAnalyticsType,
    KrakenResolution, KrakenRestCandleData, KrakenRestCandleResponse, KrakenTickType,
};
pub(crate) use history::MarketHistoryApi;
pub use history::{
    KrakenHistoryOrder, KrakenHistoryPage, KrakenHistoryQuery, KrakenMarkPriceChanged,
    KrakenMarkPriceElement, KrakenMarkPriceEvent, KrakenOrderCancelled, KrakenOrderPlaced,
    KrakenOrderUpdated, KrakenPublicExecution, KrakenPublicExecutionDetails,
    KrakenPublicExecutionElement, KrakenPublicExecutionEvent, KrakenPublicOrderElement,
    KrakenPublicOrderEvent, KrakenSortOrder,
};
pub(crate) use market::MarketApi;
pub use market::{
    KrakenContractType, KrakenHistoricalFundingRate, KrakenHistoryTrade, KrakenInstrument,
    KrakenInstrumentStatus, KrakenMarginLevel, KrakenMarginSchedule, KrakenOptionGreeks,
    KrakenOrderBook, KrakenTickerData,
};
pub use multi_collateral::{
    KrakenGetLeverageResponse, KrakenLeveragePreference, KrakenLeverageSettingResponse,
    MultiCollateralApi,
};
pub use order_management::{
    KrakenCancelResponse, KrakenCancelStatus, KrakenDeadMansSwitchResponse,
    KrakenDeadMansSwitchStatus, KrakenEditStatus, KrakenModifyResponse, KrakenOpenOrder,
    KrakenOpenOrdersResponse, KrakenOrder, KrakenOrderEvent, KrakenOrderResponse,
    KrakenOrderStatusItem, KrakenOrderStatusOrder, KrakenOrderStatusResponse, KrakenOrderTrigger,
    KrakenPriceTriggerOptions, KrakenResponse, KrakenSendStatus, KrakenTrailingStopOptions,
    OrderManagementApi,
};

/// Parses a `/derivatives/api/v3` reply. Kraken reports failures as `"result": "error"` with an
/// `error` code (often with HTTP 200); that becomes an error carrying the code.
pub(crate) fn parse_response<R: serde::de::DeserializeOwned>(
    text: &str,
) -> Result<R, Box<dyn std::error::Error>> {
    let value: serde_json::Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(e) => return Err(format!("Kraken response is not JSON ({e}): {text}").into()),
    };
    if value.get("result").and_then(|r| r.as_str()) == Some("error") {
        return Err(format!("Kraken error: {text}").into());
    }
    match serde_json::from_value(value) {
        Ok(r) => Ok(r),
        Err(e) => Err(format!("Failed to parse Kraken response ({e}): {text}").into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_result_is_an_error() {
        // Live reply of GET /derivatives/api/v3/openorders without credentials, 2026-11-15.
        let text = r#"{"result":"error","error":"authenticationError","serverTime":"2026-11-15T21:48:34.299Z"}"#;
        let err = parse_response::<KrakenOpenOrdersResponse>(text).unwrap_err();
        assert!(err.to_string().contains("authenticationError"));
    }
}
