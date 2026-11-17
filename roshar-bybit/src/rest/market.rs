use super::types::*;
use roshar_http::RateLimitedClient;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// For fields ByBit always sends but may set to `null`: the key must be present.
fn required_nullable<'de, D, T>(d: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(d)
}

/// Row of `GET /v5/market/instruments-info` for `linear` / `inverse`.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ByBitInstrumentInfo {
    pub symbol: String,
    #[serde(rename = "symbolId")]
    pub symbol_id: u64,
    #[serde(rename = "contractType")]
    pub contract_type: String,
    pub status: String,
    #[serde(rename = "baseCoin")]
    pub base_coin: String,
    #[serde(rename = "quoteCoin")]
    pub quote_coin: String,
    #[serde(rename = "symbolType")]
    pub symbol_type: String,
    #[serde(rename = "launchTime")]
    pub launch_time: String,
    /// Expiry for futures, delisting time for perpetuals; "0" when none.
    #[serde(rename = "deliveryTime")]
    pub delivery_time: String,
    #[serde(rename = "deliveryFeeRate")]
    pub delivery_fee_rate: String,
    #[serde(rename = "priceScale")]
    pub price_scale: String,
    #[serde(rename = "leverageFilter")]
    pub leverage_filter: ByBitLeverageFilter,
    #[serde(rename = "priceFilter")]
    pub price_filter: ByBitPriceFilter,
    #[serde(rename = "lotSizeFilter")]
    pub lot_size_filter: ByBitLotSizeFilter,
    #[serde(rename = "unifiedMarginTrade")]
    pub unified_margin_trade: bool,
    /// Funding interval in minutes.
    #[serde(rename = "fundingInterval")]
    pub funding_interval: i32,
    #[serde(rename = "settleCoin")]
    pub settle_coin: String,
    #[serde(rename = "copyTrading")]
    pub copy_trading: String,
    #[serde(rename = "upperFundingRate")]
    pub upper_funding_rate: String,
    #[serde(rename = "lowerFundingRate")]
    pub lower_funding_rate: String,
    #[serde(rename = "displayName")]
    pub display_name: String,
    #[serde(rename = "fullName")]
    pub full_name: String,
    #[serde(rename = "marketRegion")]
    pub market_region: String,
    #[serde(rename = "underlyingTicker")]
    pub underlying_ticker: String,
    pub tags: Vec<String>,
    #[serde(rename = "forbidUplWithdrawal")]
    pub forbid_upl_withdrawal: bool,
    #[serde(rename = "riskParameters")]
    pub risk_parameters: ByBitRiskParameters,
    #[serde(rename = "isPreListing")]
    pub is_pre_listing: bool,
    /// `null` when `isPreListing` is false (every live row on 2026-11-16); the key is always sent.
    #[serde(rename = "preListingInfo", deserialize_with = "required_nullable")]
    pub pre_listing_info: Option<ByBitPreListingInfo>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ByBitLeverageFilter {
    #[serde(rename = "minLeverage")]
    pub min_leverage: String,
    #[serde(rename = "maxLeverage")]
    pub max_leverage: String,
    #[serde(rename = "leverageStep")]
    pub leverage_step: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ByBitPriceFilter {
    #[serde(rename = "minPrice")]
    pub min_price: String,
    #[serde(rename = "maxPrice")]
    pub max_price: String,
    #[serde(rename = "tickSize")]
    pub tick_size: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ByBitLotSizeFilter {
    /// Maximum quantity for Limit and PostOnly orders.
    #[serde(rename = "maxOrderQty")]
    pub max_order_qty: String,
    /// Maximum quantity for Market orders.
    #[serde(rename = "maxMktOrderQty")]
    pub max_mkt_order_qty: String,
    #[serde(rename = "minOrderQty")]
    pub min_order_qty: String,
    #[serde(rename = "qtyStep")]
    pub qty_step: String,
    #[serde(rename = "minNotionalValue")]
    pub min_notional_value: String,
    /// Maximum quantity for PostOnly orders.
    #[serde(rename = "postOnlyMaxOrderQty")]
    pub post_only_max_order_qty: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ByBitRiskParameters {
    #[serde(rename = "priceLimitRatioX")]
    pub price_limit_ratio_x: String,
    #[serde(rename = "priceLimitRatioY")]
    pub price_limit_ratio_y: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ByBitPreListingInfo {
    #[serde(rename = "curAuctionPhase")]
    pub cur_auction_phase: String,
    pub phases: Vec<ByBitPreListingPhase>,
    #[serde(rename = "auctionFeeInfo")]
    pub auction_fee_info: ByBitAuctionFeeInfo,
    #[serde(rename = "skipCallAuction")]
    pub skip_call_auction: bool,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ByBitPreListingPhase {
    pub phase: String,
    #[serde(rename = "startTime")]
    pub start_time: String,
    /// Empty for the open-ended `ContinuousTrading` phase.
    #[serde(rename = "endTime")]
    pub end_time: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ByBitAuctionFeeInfo {
    #[serde(rename = "auctionFeeRate")]
    pub auction_fee_rate: String,
    #[serde(rename = "takerFeeRate")]
    pub taker_fee_rate: String,
    #[serde(rename = "makerFeeRate")]
    pub maker_fee_rate: String,
}

/// Row of `GET /v5/market/tickers` for `linear` / `inverse`.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ByBitTickerData {
    pub symbol: String,
    #[serde(rename = "lastPrice")]
    pub last_price: String,
    #[serde(rename = "indexPrice")]
    pub index_price: String,
    #[serde(rename = "markPrice")]
    pub mark_price: String,
    #[serde(rename = "prevPrice24h")]
    pub prev_price_24h: String,
    #[serde(rename = "price24hPcnt")]
    pub price_24h_pcnt: String,
    #[serde(rename = "highPrice24h")]
    pub high_price_24h: String,
    #[serde(rename = "lowPrice24h")]
    pub low_price_24h: String,
    #[serde(rename = "prevPrice1h")]
    pub prev_price_1h: String,
    #[serde(rename = "openInterest")]
    pub open_interest: String,
    #[serde(rename = "openInterestValue")]
    pub open_interest_value: String,
    #[serde(rename = "singleOpenInterest")]
    pub single_open_interest: String,
    #[serde(rename = "singleOpenInterestValue")]
    pub single_open_interest_value: String,
    #[serde(rename = "turnover24h")]
    pub turnover_24h: String,
    #[serde(rename = "volume24h")]
    pub volume_24h: String,
    /// Empty for expiry futures.
    #[serde(rename = "fundingRate")]
    pub funding_rate: String,
    #[serde(rename = "nextFundingTime")]
    pub next_funding_time: String,
    #[serde(rename = "predictedDeliveryPrice")]
    pub predicted_delivery_price: String,
    #[serde(rename = "basisRate")]
    pub basis_rate: String,
    #[serde(rename = "deliveryFeeRate")]
    pub delivery_fee_rate: String,
    #[serde(rename = "deliveryTime")]
    pub delivery_time: String,
    #[serde(rename = "ask1Size")]
    pub ask1_size: String,
    #[serde(rename = "bid1Price")]
    pub bid1_price: String,
    #[serde(rename = "ask1Price")]
    pub ask1_price: String,
    #[serde(rename = "bid1Size")]
    pub bid1_size: String,
    #[serde(rename = "basis")]
    pub basis: String,
    #[serde(rename = "preOpenPrice")]
    pub pre_open_price: String,
    #[serde(rename = "preQty")]
    pub pre_qty: String,
    #[serde(rename = "curPreListingPhase")]
    pub cur_pre_listing_phase: String,
    /// Whole hours; empty for expiry futures.
    #[serde(rename = "fundingIntervalHour")]
    pub funding_interval_hour: String,
    /// Empty for expiry futures.
    #[serde(rename = "fundingCap")]
    pub funding_cap: String,
    /// Empty for perpetuals.
    #[serde(rename = "basisRateYear")]
    pub basis_rate_year: String,
}

/// `{retCode, retMsg}` head of every ByBit REST response.
#[derive(Deserialize)]
struct Envelope {
    #[serde(rename = "retCode")]
    ret_code: i64,
    #[serde(rename = "retMsg")]
    ret_msg: String,
}

#[derive(Deserialize)]
struct Payload<T> {
    result: T,
}

/// Checks `retCode` (surfacing ByBit's code and message) and then parses `result` as `T`.
/// Failed requests carry `result: {}` or `null`, so the envelope is checked first.
pub(crate) fn parse_response<T: DeserializeOwned>(body: &str) -> Result<T, BoxError> {
    let envelope: Envelope = serde_json::from_str(body)
        .map_err(|e| format!("Failed to parse ByBit response envelope: {e}: {body}"))?;
    if envelope.ret_code != 0 {
        return Err(format!(
            "ByBit API error {}: {}",
            envelope.ret_code, envelope.ret_msg
        )
        .into());
    }
    let payload: Payload<T> = serde_json::from_str(body)
        .map_err(|e| format!("Failed to parse ByBit response result: {e}"))?;
    Ok(payload.result)
}

/// Query string builder: required params always, optional params only when `Some`.
#[derive(Default)]
struct Query(Vec<(&'static str, String)>);

impl Query {
    fn req(mut self, key: &'static str, value: impl ToString) -> Self {
        self.0.push((key, value.to_string()));
        self
    }

    fn opt<V: ToString>(mut self, key: &'static str, value: Option<V>) -> Self {
        if let Some(v) = value {
            self.0.push((key, v.to_string()));
        }
        self
    }

    /// Values are appended verbatim: ByBit symbols/coins are URL-safe and `cursor` values are
    /// returned already URL-encoded (re-encoding would break them).
    fn to_url(&self, path: &str) -> String {
        let mut url = format!("{}{}", super::BYBIT_REST_URL, path);
        for (i, (k, v)) in self.0.iter().enumerate() {
            url.push(if i == 0 { '?' } else { '&' });
            url.push_str(k);
            url.push('=');
            url.push_str(v);
        }
        url
    }
}

/// ByBit V5 public market data REST API.
pub struct MarketApi {
    client: Arc<RateLimitedClient>,
}

impl MarketApi {
    pub fn new(requests_per_second: u32) -> Self {
        Self {
            client: Arc::new(RateLimitedClient::new(requests_per_second, 1)),
        }
    }

    pub fn new_with_client(client: Arc<RateLimitedClient>) -> Self {
        Self { client }
    }

    async fn get<T: DeserializeOwned>(&self, path: &str, query: Query) -> Result<T, BoxError> {
        let url = query.to_url(path);
        let response = self.client.get(&url).await?;
        let status = response.status();
        let body = response.text().await?;
        if !status.is_success() {
            return Err(format!("ByBit {path} failed with status {status}: {body}").into());
        }
        parse_response(&body)
    }

    /// `GET /v5/market/time`.
    pub async fn get_server_time(&self) -> Result<ByBitServerTime, BoxError> {
        self.get("/v5/market/time", Query::default()).await
    }

    /// `GET /v5/system/status`: platform maintenance / incident records.
    pub async fn get_system_status(
        &self,
        id: Option<&str>,
        state: Option<&str>,
    ) -> Result<ByBitSystemStatusResult, BoxError> {
        let q = Query::default().opt("id", id).opt("state", state);
        self.get("/v5/system/status", q).await
    }

    /// `GET /v5/market/kline` (`spot`, `linear`, `inverse`; ByBit defaults to `linear`).
    pub async fn get_kline(
        &self,
        category: Option<ByBitCategory>,
        symbol: &str,
        interval: crate::ByBitKlineInterval,
        start: Option<u64>,
        end: Option<u64>,
        limit: Option<u32>,
    ) -> Result<ByBitKlineResult, BoxError> {
        let q = kline_query(category, symbol, interval, start, end, limit);
        self.get("/v5/market/kline", q).await
    }

    /// `GET /v5/market/mark-price-kline` (`linear`, `inverse`, `option`).
    pub async fn get_mark_price_kline(
        &self,
        category: Option<ByBitCategory>,
        symbol: &str,
        interval: crate::ByBitKlineInterval,
        start: Option<u64>,
        end: Option<u64>,
        limit: Option<u32>,
    ) -> Result<ByBitPriceKlineResult, BoxError> {
        let q = kline_query(category, symbol, interval, start, end, limit);
        self.get("/v5/market/mark-price-kline", q).await
    }

    /// `GET /v5/market/index-price-kline` (`linear`, `inverse`).
    pub async fn get_index_price_kline(
        &self,
        category: Option<ByBitContractCategory>,
        symbol: &str,
        interval: crate::ByBitKlineInterval,
        start: Option<u64>,
        end: Option<u64>,
        limit: Option<u32>,
    ) -> Result<ByBitPriceKlineResult, BoxError> {
        let q = Query::default()
            .opt("category", category.map(|c| c.as_str()))
            .req("symbol", symbol)
            .req("interval", interval.as_str())
            .opt("start", start)
            .opt("end", end)
            .opt("limit", limit);
        self.get("/v5/market/index-price-kline", q).await
    }

    /// `GET /v5/market/premium-index-price-kline` (linear perpetuals; category is always
    /// `linear`, which is also ByBit's default, so it is not sent).
    pub async fn get_premium_index_price_kline(
        &self,
        symbol: &str,
        interval: crate::ByBitKlineInterval,
        start: Option<u64>,
        end: Option<u64>,
        limit: Option<u32>,
    ) -> Result<ByBitPriceKlineResult, BoxError> {
        let q = kline_query(None, symbol, interval, start, end, limit);
        self.get("/v5/market/premium-index-price-kline", q).await
    }

    /// `GET /v5/market/instruments-info` for `linear` / `inverse`: one page.
    pub async fn get_instruments_info(
        &self,
        category: ByBitContractCategory,
        query: &ByBitInstrumentsQuery,
    ) -> Result<ByBitInstrumentsResult<ByBitInstrumentInfo>, BoxError> {
        let q = instruments_query(category.as_str(), query);
        self.get("/v5/market/instruments-info", q).await
    }

    /// `GET /v5/market/instruments-info?category=spot` (not paginated by ByBit).
    pub async fn get_spot_instruments_info(
        &self,
        query: &ByBitInstrumentsQuery,
    ) -> Result<ByBitSpotInstrumentsResult, BoxError> {
        let q = instruments_query("spot", query);
        self.get("/v5/market/instruments-info", q).await
    }

    /// `GET /v5/market/instruments-info?category=option`: one page.
    pub async fn get_option_instruments_info(
        &self,
        query: &ByBitInstrumentsQuery,
    ) -> Result<ByBitInstrumentsResult<ByBitOptionInstrumentInfo>, BoxError> {
        let q = instruments_query("option", query);
        self.get("/v5/market/instruments-info", q).await
    }

    /// `GET /v5/market/instrument-summary` (`linear`, `option`): one page.
    #[allow(clippy::too_many_arguments)]
    pub async fn get_instrument_summary(
        &self,
        category: ByBitCategory,
        symbol: Option<&str>,
        symbol_type: Option<&str>,
        market_region: Option<&str>,
        base_coin: Option<&str>,
        limit: Option<u32>,
        cursor: Option<&str>,
    ) -> Result<ByBitInstrumentSummaryResult, BoxError> {
        let q = Query::default()
            .req("category", category.as_str())
            .opt("symbol", symbol)
            .opt("symbolType", symbol_type)
            .opt("marketRegion", market_region)
            .opt("baseCoin", base_coin)
            .opt("limit", limit)
            .opt("cursor", cursor);
        self.get("/v5/market/instrument-summary", q).await
    }

    /// `GET /v5/market/option-base-coins`. `underlying_type` is ByBit's comma list, e.g. `0,2`.
    pub async fn get_option_base_coins(
        &self,
        underlying_type: Option<&str>,
    ) -> Result<ByBitOptionBaseCoinsResult, BoxError> {
        let q = Query::default().opt("underlyingType", underlying_type);
        self.get("/v5/market/option-base-coins", q).await
    }

    /// `GET /v5/market/corporate-action`: one page.
    pub async fn get_corporate_actions(
        &self,
        category: Option<ByBitCategory>,
        base_coin: Option<&str>,
        action_type: Option<&str>,
        limit: Option<u32>,
        cursor: Option<&str>,
    ) -> Result<ByBitCorporateActionResult, BoxError> {
        let q = Query::default()
            .opt("category", category.map(|c| c.as_str()))
            .opt("baseCoin", base_coin)
            .opt("actionType", action_type)
            .opt("limit", limit)
            .opt("cursor", cursor);
        self.get("/v5/market/corporate-action", q).await
    }

    /// `GET /v5/market/orderbook` (all categories).
    pub async fn get_orderbook(
        &self,
        category: ByBitCategory,
        symbol: &str,
        limit: Option<u32>,
    ) -> Result<ByBitOrderbook, BoxError> {
        let q = Query::default()
            .req("category", category.as_str())
            .req("symbol", symbol)
            .opt("limit", limit);
        self.get("/v5/market/orderbook", q).await
    }

    /// `GET /v5/market/full_orderbook` (`spot`, `linear`, `inverse`; up to 10,000 levels, no RPI).
    pub async fn get_full_orderbook(
        &self,
        category: ByBitCategory,
        symbol: &str,
    ) -> Result<ByBitOrderbook, BoxError> {
        let q = Query::default()
            .req("category", category.as_str())
            .req("symbol", symbol);
        self.get("/v5/market/full_orderbook", q).await
    }

    /// `GET /v5/market/rpi_orderbook` (`spot`, `linear`, `inverse`). `limit` is required by ByBit.
    pub async fn get_rpi_orderbook(
        &self,
        category: Option<ByBitCategory>,
        symbol: &str,
        limit: u32,
    ) -> Result<ByBitRpiOrderbook, BoxError> {
        let q = Query::default()
            .opt("category", category.map(|c| c.as_str()))
            .req("symbol", symbol)
            .req("limit", limit);
        self.get("/v5/market/rpi_orderbook", q).await
    }

    /// `GET /v5/market/tickers` for `linear` / `inverse`.
    pub async fn get_tickers(
        &self,
        category: ByBitContractCategory,
        symbol: Option<&str>,
    ) -> Result<ByBitTickersResult<ByBitTickerData>, BoxError> {
        let q = Query::default()
            .req("category", category.as_str())
            .opt("symbol", symbol);
        self.get("/v5/market/tickers", q).await
    }

    /// `GET /v5/market/tickers?category=spot`.
    pub async fn get_spot_tickers(
        &self,
        symbol: Option<&str>,
    ) -> Result<ByBitTickersResult<ByBitSpotTicker>, BoxError> {
        let q = Query::default()
            .req("category", "spot")
            .opt("symbol", symbol);
        self.get("/v5/market/tickers", q).await
    }

    /// `GET /v5/market/tickers?category=option`. ByBit requires `symbol` or `base_coin`;
    /// `exp_date` looks like `25DEC22`.
    pub async fn get_option_tickers(
        &self,
        symbol: Option<&str>,
        base_coin: Option<&str>,
        exp_date: Option<&str>,
    ) -> Result<ByBitTickersResult<ByBitOptionTicker>, BoxError> {
        let q = Query::default()
            .req("category", "option")
            .opt("symbol", symbol)
            .opt("baseCoin", base_coin)
            .opt("expDate", exp_date);
        self.get("/v5/market/tickers", q).await
    }

    /// `GET /v5/market/funding/history`: one page, newest first.
    pub async fn get_funding_rate_history(
        &self,
        category: ByBitContractCategory,
        symbol: &str,
        start_time: Option<u64>,
        end_time: Option<u64>,
        limit: Option<u32>,
    ) -> Result<ByBitFundingHistoryResult, BoxError> {
        let q = Query::default()
            .req("category", category.as_str())
            .req("symbol", symbol)
            .opt("startTime", start_time)
            .opt("endTime", end_time)
            .opt("limit", limit);
        self.get("/v5/market/funding/history", q).await
    }

    /// `GET /v5/market/recent-trade` (all categories).
    pub async fn get_recent_trades(
        &self,
        category: ByBitCategory,
        query: &ByBitRecentTradesQuery,
    ) -> Result<ByBitRecentTradesResult, BoxError> {
        let q = Query::default()
            .req("category", category.as_str())
            .opt("symbol", query.symbol.as_deref())
            .opt("baseCoin", query.base_coin.as_deref())
            .opt("optionType", query.option_type.as_deref())
            .opt("limit", query.limit);
        self.get("/v5/market/recent-trade", q).await
    }

    /// `GET /v5/market/open-interest`: one page.
    #[allow(clippy::too_many_arguments)]
    pub async fn get_open_interest(
        &self,
        category: ByBitContractCategory,
        symbol: &str,
        interval_time: ByBitDataPeriod,
        start_time: Option<u64>,
        end_time: Option<u64>,
        limit: Option<u32>,
        cursor: Option<&str>,
    ) -> Result<ByBitOpenInterestResult, BoxError> {
        let q = Query::default()
            .req("category", category.as_str())
            .req("symbol", symbol)
            .req("intervalTime", interval_time.as_str())
            .opt("startTime", start_time)
            .opt("endTime", end_time)
            .opt("limit", limit)
            .opt("cursor", cursor);
        self.get("/v5/market/open-interest", q).await
    }

    /// `GET /v5/market/historical-volatility?category=option`. ByBit wants `start_time` and
    /// `end_time` together (window <= 30 days) or neither.
    pub async fn get_historical_volatility(
        &self,
        base_coin: Option<&str>,
        quote_coin: Option<&str>,
        period: Option<u32>,
        start_time: Option<u64>,
        end_time: Option<u64>,
    ) -> Result<Vec<ByBitHistoricalVolatility>, BoxError> {
        let q = Query::default()
            .req("category", "option")
            .opt("baseCoin", base_coin)
            .opt("quoteCoin", quote_coin)
            .opt("period", period)
            .opt("startTime", start_time)
            .opt("endTime", end_time);
        self.get("/v5/market/historical-volatility", q).await
    }

    /// `GET /v5/market/insurance`.
    pub async fn get_insurance(
        &self,
        coin: Option<&str>,
    ) -> Result<ByBitInsuranceResult, BoxError> {
        let q = Query::default().opt("coin", coin);
        self.get("/v5/market/insurance", q).await
    }

    /// `GET /v5/market/adlAlert`.
    pub async fn get_adl_alert(
        &self,
        symbol: Option<&str>,
    ) -> Result<ByBitAdlAlertResult, BoxError> {
        let q = Query::default().opt("symbol", symbol);
        self.get("/v5/market/adlAlert", q).await
    }

    /// `GET /v5/market/risk-limit`: one page.
    pub async fn get_risk_limit(
        &self,
        category: ByBitContractCategory,
        symbol: Option<&str>,
        cursor: Option<&str>,
    ) -> Result<ByBitRiskLimitResult, BoxError> {
        let q = Query::default()
            .req("category", category.as_str())
            .opt("symbol", symbol)
            .opt("cursor", cursor);
        self.get("/v5/market/risk-limit", q).await
    }

    /// `GET /v5/market/delivery-price` (`linear`, `inverse`, `option`): one page.
    pub async fn get_delivery_price(
        &self,
        category: ByBitCategory,
        symbol: Option<&str>,
        base_coin: Option<&str>,
        settle_coin: Option<&str>,
        limit: Option<u32>,
        cursor: Option<&str>,
    ) -> Result<ByBitDeliveryPriceResult, BoxError> {
        let q = Query::default()
            .req("category", category.as_str())
            .opt("symbol", symbol)
            .opt("baseCoin", base_coin)
            .opt("settleCoin", settle_coin)
            .opt("limit", limit)
            .opt("cursor", cursor);
        self.get("/v5/market/delivery-price", q).await
    }

    /// `GET /v5/market/new-delivery-price?category=option`.
    pub async fn get_new_delivery_price(
        &self,
        base_coin: &str,
        settle_coin: Option<&str>,
    ) -> Result<ByBitNewDeliveryPriceResult, BoxError> {
        let q = Query::default()
            .req("category", "option")
            .req("baseCoin", base_coin)
            .opt("settleCoin", settle_coin);
        self.get("/v5/market/new-delivery-price", q).await
    }

    /// `GET /v5/market/account-ratio` (long/short account ratio): one page.
    #[allow(clippy::too_many_arguments)]
    pub async fn get_long_short_ratio(
        &self,
        category: ByBitContractCategory,
        symbol: &str,
        period: ByBitDataPeriod,
        start_time: Option<u64>,
        end_time: Option<u64>,
        limit: Option<u32>,
        cursor: Option<&str>,
    ) -> Result<ByBitLongShortRatioResult, BoxError> {
        let q = Query::default()
            .req("category", category.as_str())
            .req("symbol", symbol)
            .req("period", period.as_str())
            .opt("startTime", start_time)
            .opt("endTime", end_time)
            .opt("limit", limit)
            .opt("cursor", cursor);
        self.get("/v5/market/account-ratio", q).await
    }

    /// `GET /v5/market/index-price-components`.
    pub async fn get_index_price_components(
        &self,
        index_name: &str,
    ) -> Result<ByBitIndexPriceComponents, BoxError> {
        let q = Query::default().req("indexName", index_name);
        self.get("/v5/market/index-price-components", q).await
    }

    /// `GET /v5/market/price-limit` (`spot`, `linear`, `inverse`; ByBit defaults to `linear`).
    pub async fn get_order_price_limit(
        &self,
        category: Option<ByBitCategory>,
        symbol: &str,
    ) -> Result<ByBitOrderPriceLimit, BoxError> {
        let q = Query::default()
            .opt("category", category.map(|c| c.as_str()))
            .req("symbol", symbol);
        self.get("/v5/market/price-limit", q).await
    }

    /// `GET /v5/spread/instrument`: one page.
    pub async fn get_spread_instruments(
        &self,
        symbol: Option<&str>,
        base_coin: Option<&str>,
        limit: Option<u32>,
        cursor: Option<&str>,
    ) -> Result<ByBitSpreadInstrumentsResult, BoxError> {
        let q = Query::default()
            .opt("symbol", symbol)
            .opt("baseCoin", base_coin)
            .opt("limit", limit)
            .opt("cursor", cursor);
        self.get("/v5/spread/instrument", q).await
    }

    /// `GET /v5/spread/orderbook`.
    pub async fn get_spread_orderbook(
        &self,
        symbol: &str,
        limit: Option<u32>,
    ) -> Result<ByBitOrderbook, BoxError> {
        let q = Query::default().req("symbol", symbol).opt("limit", limit);
        self.get("/v5/spread/orderbook", q).await
    }

    /// `GET /v5/spread/recent-trade`.
    pub async fn get_spread_recent_trades(
        &self,
        symbol: &str,
        limit: Option<u32>,
    ) -> Result<ByBitSpreadRecentTradesResult, BoxError> {
        let q = Query::default().req("symbol", symbol).opt("limit", limit);
        self.get("/v5/spread/recent-trade", q).await
    }

    /// `GET /v5/spread/tickers`.
    pub async fn get_spread_tickers(
        &self,
        symbol: &str,
    ) -> Result<ByBitSpreadTickersResult, BoxError> {
        let q = Query::default().req("symbol", symbol);
        self.get("/v5/spread/tickers", q).await
    }
}

fn kline_query(
    category: Option<ByBitCategory>,
    symbol: &str,
    interval: crate::ByBitKlineInterval,
    start: Option<u64>,
    end: Option<u64>,
    limit: Option<u32>,
) -> Query {
    Query::default()
        .opt("category", category.map(|c| c.as_str()))
        .req("symbol", symbol)
        .req("interval", interval.as_str())
        .opt("start", start)
        .opt("end", end)
        .opt("limit", limit)
}

fn instruments_query(category: &str, query: &ByBitInstrumentsQuery) -> Query {
    Query::default()
        .req("category", category)
        .opt("symbol", query.symbol.as_deref())
        .opt("symbolType", query.symbol_type.as_deref())
        .opt("status", query.status.as_deref())
        .opt("baseCoin", query.base_coin.as_deref())
        .opt("limit", query.limit)
        .opt("cursor", query.cursor.as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_omits_unset_params() {
        let url = Query::default()
            .req("category", "linear")
            .opt("symbol", None::<&str>)
            .opt("limit", Some(5))
            .to_url("/v5/market/tickers");
        assert_eq!(
            url,
            "https://api.bybit.com/v5/market/tickers?category=linear&limit=5"
        );
        assert_eq!(
            Query::default().to_url("/v5/market/time"),
            "https://api.bybit.com/v5/market/time"
        );
    }

    #[tokio::test]
    async fn live_funding_rate_history_page() {
        let api = MarketApi::new(10);
        let page = api
            .get_funding_rate_history(
                ByBitContractCategory::Linear,
                "BTCUSDT",
                None,
                None,
                Some(5),
            )
            .await
            .expect("funding history");
        assert_eq!(page.list.len(), 5);
        let t0: u64 = page.list[0].funding_rate_timestamp.parse().unwrap();
        let t1: u64 = page.list[1].funding_rate_timestamp.parse().unwrap();
        assert!(t0 > t1, "expected newest first");
    }

    #[tokio::test]
    async fn live_invalid_symbol_surfaces_ret_code() {
        let api = MarketApi::new(10);
        let err = api
            .get_orderbook(ByBitCategory::Linear, "NOPEUSDT", None)
            .await
            .expect_err("invalid symbol must fail");
        assert!(err.to_string().contains("10001"), "{err}");
    }
}
