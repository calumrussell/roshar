use crate::request::{DeribitRequest, DeribitSettlementFilter, DeribitTradeFilter};
use crate::types::{
    DeribitAprHistory, DeribitBasketIndexComposition, DeribitBlockRfqTradesPage,
    DeribitBookSummary, DeribitCombo, DeribitContractSize, DeribitCurrency, DeribitDeliveryPrices,
    DeribitExpirations, DeribitFundingChartData, DeribitFundingRateHistory, DeribitIndexPrice,
    DeribitIndexPriceName, DeribitInstrument, DeribitOrderBookSnapshot, DeribitRpcResponse,
    DeribitSettlementsPage, DeribitStatus, DeribitTickerData, DeribitTimeValue, DeribitTradeVolume,
    DeribitTradesPage, DeribitTradingViewChartData, DeribitVersion, DeribitVolatilityIndexData,
};
use roshar_http::RateLimitedClient;
use serde::de::DeserializeOwned;

pub const DERIBIT_REST_URL: &str = "https://www.deribit.com/api/v2";
pub const DERIBIT_TESTNET_REST_URL: &str = "https://test.deribit.com/api/v2";

type Res<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// Public market data over `GET {base}/public/{method}?{params}`. Each method maps 1:1 to the
/// JSON-RPC method of the same name; exchange errors come back as `DeribitRpcError`.
pub struct MarketApi {
    client: RateLimitedClient,
    base_url: String,
}

impl MarketApi {
    /// `refill` requests every `interval_secs` seconds (see `roshar_http::RateLimitedClient`),
    /// against `DERIBIT_REST_URL`.
    pub fn new(refill: u32, interval_secs: u64) -> Self {
        Self::with_base_url(refill, interval_secs, DERIBIT_REST_URL)
    }

    /// As `new`, against another base URL (e.g. `DERIBIT_TESTNET_REST_URL`).
    pub fn with_base_url(refill: u32, interval_secs: u64, base_url: &str) -> Self {
        Self {
            client: RateLimitedClient::new(refill, interval_secs),
            base_url: base_url.trim_end_matches('/').to_string(),
        }
    }

    /// Sends any request and parses its `result` as `T`. The typed methods below use this.
    pub async fn call<T: DeserializeOwned>(&self, request: &DeribitRequest) -> Res<T> {
        let query = request.query_string();
        let url = if query.is_empty() {
            format!("{}/{}", self.base_url, request.method)
        } else {
            format!("{}/{}?{}", self.base_url, request.method, query)
        };
        let response = self.client.get(&url).await?;
        let status = response.status();
        let text = response.text().await?;
        // Errors arrive as HTTP 400 with the same JSON-RPC envelope.
        let envelope: DeribitRpcResponse<T> = serde_json::from_str(&text).map_err(|e| {
            format!(
                "Failed to parse Deribit {} ({status}): {e}: {text}",
                request.method
            )
        })?;
        Ok(envelope.into_result()?)
    }

    pub async fn get_instruments(
        &self,
        currency: &str,
        kind: Option<&str>,
        expired: Option<bool>,
        extended: Option<bool>,
    ) -> Res<Vec<DeribitInstrument>> {
        self.call(&DeribitRequest::get_instruments(
            currency, kind, expired, extended,
        ))
        .await
    }

    pub async fn get_instrument(&self, instrument_name: &str) -> Res<DeribitInstrument> {
        self.call(&DeribitRequest::get_instrument(instrument_name))
            .await
    }

    pub async fn get_currencies(&self) -> Res<Vec<DeribitCurrency>> {
        self.call(&DeribitRequest::get_currencies()).await
    }

    pub async fn get_book_summary_by_currency(
        &self,
        currency: &str,
        kind: Option<&str>,
    ) -> Res<Vec<DeribitBookSummary>> {
        self.call(&DeribitRequest::get_book_summary_by_currency(
            currency, kind,
        ))
        .await
    }

    pub async fn get_book_summary_by_instrument(
        &self,
        instrument_name: &str,
    ) -> Res<Vec<DeribitBookSummary>> {
        self.call(&DeribitRequest::get_book_summary_by_instrument(
            instrument_name,
        ))
        .await
    }

    pub async fn get_order_book(
        &self,
        instrument_name: &str,
        depth: Option<u32>,
    ) -> Res<DeribitOrderBookSnapshot> {
        self.call(&DeribitRequest::get_order_book(instrument_name, depth))
            .await
    }

    pub async fn get_order_book_by_instrument_id(
        &self,
        instrument_id: u64,
        depth: Option<u32>,
    ) -> Res<DeribitOrderBookSnapshot> {
        self.call(&DeribitRequest::get_order_book_by_instrument_id(
            instrument_id,
            depth,
        ))
        .await
    }

    pub async fn ticker(&self, instrument_name: &str) -> Res<DeribitTickerData> {
        self.call(&DeribitRequest::ticker(instrument_name)).await
    }

    pub async fn get_last_trades_by_instrument(
        &self,
        instrument_name: &str,
        start_seq: Option<u64>,
        end_seq: Option<u64>,
        filter: DeribitTradeFilter<'_>,
    ) -> Res<DeribitTradesPage> {
        self.call(&DeribitRequest::get_last_trades_by_instrument(
            instrument_name,
            start_seq,
            end_seq,
            filter,
        ))
        .await
    }

    pub async fn get_last_trades_by_instrument_and_time(
        &self,
        instrument_name: &str,
        filter: DeribitTradeFilter<'_>,
    ) -> Res<DeribitTradesPage> {
        self.call(&DeribitRequest::get_last_trades_by_instrument_and_time(
            instrument_name,
            filter,
        ))
        .await
    }

    pub async fn get_last_trades_by_currency(
        &self,
        currency: &str,
        kind: Option<&str>,
        start_id: Option<&str>,
        end_id: Option<&str>,
        filter: DeribitTradeFilter<'_>,
    ) -> Res<DeribitTradesPage> {
        self.call(&DeribitRequest::get_last_trades_by_currency(
            currency, kind, start_id, end_id, filter,
        ))
        .await
    }

    pub async fn get_last_trades_by_currency_and_time(
        &self,
        currency: &str,
        kind: Option<&str>,
        filter: DeribitTradeFilter<'_>,
    ) -> Res<DeribitTradesPage> {
        self.call(&DeribitRequest::get_last_trades_by_currency_and_time(
            currency, kind, filter,
        ))
        .await
    }

    pub async fn get_tradingview_chart_data(
        &self,
        instrument_name: &str,
        start_timestamp: u64,
        end_timestamp: u64,
        resolution: &str,
    ) -> Res<DeribitTradingViewChartData> {
        self.call(&DeribitRequest::get_tradingview_chart_data(
            instrument_name,
            start_timestamp,
            end_timestamp,
            resolution,
        ))
        .await
    }

    pub async fn get_funding_rate_history(
        &self,
        instrument_name: &str,
        start_timestamp: u64,
        end_timestamp: u64,
    ) -> Res<Vec<DeribitFundingRateHistory>> {
        self.call(&DeribitRequest::get_funding_rate_history(
            instrument_name,
            start_timestamp,
            end_timestamp,
        ))
        .await
    }

    pub async fn get_funding_rate_value(
        &self,
        instrument_name: &str,
        start_timestamp: u64,
        end_timestamp: u64,
    ) -> Res<f64> {
        self.call(&DeribitRequest::get_funding_rate_value(
            instrument_name,
            start_timestamp,
            end_timestamp,
        ))
        .await
    }

    pub async fn get_funding_chart_data(
        &self,
        instrument_name: &str,
        length: &str,
    ) -> Res<DeribitFundingChartData> {
        self.call(&DeribitRequest::get_funding_chart_data(
            instrument_name,
            length,
        ))
        .await
    }

    pub async fn get_historical_volatility(&self, currency: &str) -> Res<Vec<DeribitTimeValue>> {
        self.call(&DeribitRequest::get_historical_volatility(currency))
            .await
    }

    pub async fn get_volatility_index_data(
        &self,
        currency: &str,
        start_timestamp: u64,
        end_timestamp: u64,
        resolution: &str,
    ) -> Res<DeribitVolatilityIndexData> {
        self.call(&DeribitRequest::get_volatility_index_data(
            currency,
            start_timestamp,
            end_timestamp,
            resolution,
        ))
        .await
    }

    pub async fn get_index_price(&self, index_name: &str) -> Res<DeribitIndexPrice> {
        self.call(&DeribitRequest::get_index_price(index_name))
            .await
    }

    pub async fn get_index_price_names(&self) -> Res<Vec<String>> {
        self.call(&DeribitRequest::get_index_price_names(None))
            .await
    }

    /// `get_index_price_names` with `extended=true`.
    pub async fn get_index_price_names_extended(&self) -> Res<Vec<DeribitIndexPriceName>> {
        self.call(&DeribitRequest::get_index_price_names(Some(true)))
            .await
    }

    pub async fn get_index_chart_data(
        &self,
        index_name: &str,
        range: &str,
    ) -> Res<Vec<DeribitTimeValue>> {
        self.call(&DeribitRequest::get_index_chart_data(index_name, range))
            .await
    }

    pub async fn get_basket_index_composition(
        &self,
        index_name: &str,
    ) -> Res<DeribitBasketIndexComposition> {
        self.call(&DeribitRequest::get_basket_index_composition(index_name))
            .await
    }

    pub async fn get_delivery_prices(
        &self,
        index_name: &str,
        offset: Option<u32>,
        count: Option<u32>,
        start_timestamp: Option<u64>,
        end_timestamp: Option<u64>,
    ) -> Res<DeribitDeliveryPrices> {
        self.call(&DeribitRequest::get_delivery_prices(
            index_name,
            offset,
            count,
            start_timestamp,
            end_timestamp,
        ))
        .await
    }

    pub async fn get_last_settlements_by_currency(
        &self,
        currency: &str,
        filter: DeribitSettlementFilter<'_>,
    ) -> Res<DeribitSettlementsPage> {
        self.call(&DeribitRequest::get_last_settlements_by_currency(
            currency, filter,
        ))
        .await
    }

    pub async fn get_last_settlements_by_instrument(
        &self,
        instrument_name: &str,
        filter: DeribitSettlementFilter<'_>,
    ) -> Res<DeribitSettlementsPage> {
        self.call(&DeribitRequest::get_last_settlements_by_instrument(
            instrument_name,
            filter,
        ))
        .await
    }

    pub async fn get_contract_size(&self, instrument_name: &str) -> Res<DeribitContractSize> {
        self.call(&DeribitRequest::get_contract_size(instrument_name))
            .await
    }

    pub async fn get_mark_price_history(
        &self,
        instrument_name: &str,
        start_timestamp: u64,
        end_timestamp: u64,
    ) -> Res<Vec<DeribitTimeValue>> {
        self.call(&DeribitRequest::get_mark_price_history(
            instrument_name,
            start_timestamp,
            end_timestamp,
        ))
        .await
    }

    pub async fn get_supported_index_names(&self, index_type: Option<&str>) -> Res<Vec<String>> {
        self.call(&DeribitRequest::get_supported_index_names(index_type))
            .await
    }

    pub async fn get_expirations(
        &self,
        currency: Option<&str>,
        kind: Option<&str>,
        currency_pair: Option<&str>,
    ) -> Res<DeribitExpirations> {
        self.call(&DeribitRequest::get_expirations(
            currency,
            kind,
            currency_pair,
        ))
        .await
    }

    pub async fn get_apr_history(
        &self,
        currency: &str,
        limit: Option<u32>,
        before: Option<u64>,
    ) -> Res<DeribitAprHistory> {
        self.call(&DeribitRequest::get_apr_history(currency, limit, before))
            .await
    }

    pub async fn get_trade_volumes(&self, extended: Option<bool>) -> Res<Vec<DeribitTradeVolume>> {
        self.call(&DeribitRequest::get_trade_volumes(extended))
            .await
    }

    pub async fn get_combos(&self, currency: &str) -> Res<Vec<DeribitCombo>> {
        self.call(&DeribitRequest::get_combos(currency)).await
    }

    pub async fn get_combo_ids(&self, currency: &str, state: Option<&str>) -> Res<Vec<String>> {
        self.call(&DeribitRequest::get_combo_ids(currency, state))
            .await
    }

    pub async fn get_combo_details(&self, combo_id: &str) -> Res<DeribitCombo> {
        self.call(&DeribitRequest::get_combo_details(combo_id))
            .await
    }

    pub async fn get_block_rfq_trades(
        &self,
        currency: &str,
        continuation: Option<&str>,
        count: Option<u32>,
    ) -> Res<DeribitBlockRfqTradesPage> {
        self.call(&DeribitRequest::get_block_rfq_trades(
            currency,
            continuation,
            count,
        ))
        .await
    }

    pub async fn status(&self) -> Res<DeribitStatus> {
        self.call(&DeribitRequest::status()).await
    }

    /// Server time, ms since the UNIX epoch.
    pub async fn get_time(&self) -> Res<u64> {
        self.call(&DeribitRequest::get_time()).await
    }

    pub async fn test(&self, expected_result: Option<&str>) -> Res<DeribitVersion> {
        self.call(&DeribitRequest::test(expected_result)).await
    }
}
