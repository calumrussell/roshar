use crate::types::*;
use roshar_http::RateLimitedClient;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

pub type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// OKX REST envelope: `{"code": "0", "msg": "", "data": ...}`. A non-"0" `code` is an error.
#[derive(Debug, Deserialize, Serialize)]
pub struct OkxResponse<T> {
    pub code: String,
    pub msg: String,
    pub data: T,
}

#[derive(Deserialize)]
struct OkxErrorEnvelope {
    code: String,
    msg: String,
}

pub type OkxInstrumentsResponse = OkxResponse<Vec<OkxInstrumentInfo>>;

// OKX Tickers API Response
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct OkxTickerData {
    /// Instrument type (e.g. "SWAP")
    #[serde(rename = "instType")]
    pub inst_type: String,
    /// Instrument ID
    #[serde(rename = "instId")]
    pub inst_id: String,
    /// Last traded price ("" when the instrument has not traded)
    pub last: String,
    /// Last traded size. "0" when there is no trading volume
    #[serde(rename = "lastSz")]
    pub last_sz: String,
    /// Best ask price ("" when the ask side is empty)
    #[serde(rename = "askPx")]
    pub ask_px: String,
    /// Best ask size
    #[serde(rename = "askSz")]
    pub ask_sz: String,
    /// Best bid price ("" when the bid side is empty)
    #[serde(rename = "bidPx")]
    pub bid_px: String,
    /// Best bid size
    #[serde(rename = "bidSz")]
    pub bid_sz: String,
    /// Open price in the past 24 hours
    #[serde(rename = "open24h")]
    pub open_24h: String,
    /// Highest price in the past 24 hours
    #[serde(rename = "high24h")]
    pub high_24h: String,
    /// Lowest price in the past 24 hours
    #[serde(rename = "low24h")]
    pub low_24h: String,
    /// 24h volume in currency: base currency for derivatives, quote currency for SPOT/MARGIN
    #[serde(rename = "volCcy24h")]
    pub vol_ccy_24h: String,
    /// 24h volume: contracts for derivatives, base currency for SPOT/MARGIN
    #[serde(rename = "vol24h")]
    pub vol_24h: String,
    /// Open price at UTC 0
    #[serde(rename = "sodUtc0")]
    pub sod_utc0: String,
    /// Open price at UTC 8
    #[serde(rename = "sodUtc8")]
    pub sod_utc8: String,
    /// Ticker generation time (ms)
    pub ts: String,
}

pub type OkxTickersResponse = OkxResponse<Vec<OkxTickerData>>;

/// Parse an OKX REST body: surfaces `code`/`msg` when `code != "0"`, otherwise decodes `data`
/// strictly as `T`.
pub(crate) fn parse_response<T: DeserializeOwned>(path: &str, text: &str) -> Result<T, BoxError> {
    let envelope: OkxErrorEnvelope = serde_json::from_str(text)
        .map_err(|e| format!("Failed to parse OKX {path} response: {e}"))?;
    if envelope.code != "0" {
        return Err(format!(
            "OKX API error {} on {path}: {}",
            envelope.code, envelope.msg
        )
        .into());
    }
    let response: OkxResponse<T> = serde_json::from_str(text)
        .map_err(|e| format!("Failed to parse OKX {path} response: {e}"))?;
    Ok(response.data)
}

/// Endpoints that return a one-element `data` array.
fn single<T>(path: &str, data: Vec<T>) -> Result<T, BoxError> {
    data.into_iter()
        .next()
        .ok_or_else(|| format!("OKX {path} returned no data").into())
}

/// OKX Market API for public data endpoints
pub struct MarketApi {
    client: Arc<RateLimitedClient>,
}

impl MarketApi {
    pub fn new(requests_per_second: u32) -> Self {
        Self {
            client: Arc::new(RateLimitedClient::new(requests_per_second, 1)),
        }
    }

    /// GET `/api/v5/{path}` with the `Some` query parameters; `None` parameters are omitted.
    async fn get<T: DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, Option<String>)],
    ) -> Result<T, BoxError> {
        let mut url = format!("{}/api/v5/{}", super::OKX_REST_URL, path);
        let mut sep = '?';
        for (key, value) in query {
            if let Some(value) = value {
                url.push(sep);
                url.push_str(key);
                url.push('=');
                url.push_str(value);
                sep = '&';
            }
        }

        let response = self.client.get(&url).await?;
        let status = response.status();
        let text = response.text().await?;
        match parse_response(path, &text) {
            Ok(data) => Ok(data),
            Err(e) if status.is_success() => Err(e),
            Err(e) => Err(format!("OKX API request failed with status {status}: {e}").into()),
        }
    }

    /// `GET /api/v5/public/instruments`. Map of instId -> instrument (all states; caller filters).
    /// `inst_type`: SPOT, MARGIN, SWAP, FUTURES, OPTION or EVENTS. `inst_family` is required by OKX for OPTION; `series_id` is required for EVENTS.
    pub async fn get_instruments_info(
        &self,
        inst_type: &str,
        inst_family: Option<&str>,
        inst_id: Option<&str>,
        series_id: Option<&str>,
    ) -> Result<HashMap<String, OkxInstrumentInfo>, BoxError> {
        let data: Vec<OkxInstrumentInfo> = self
            .get(
                "public/instruments",
                &[
                    ("instType", Some(inst_type.to_string())),
                    ("instFamily", inst_family.map(str::to_string)),
                    ("instId", inst_id.map(str::to_string)),
                    ("seriesId", series_id.map(str::to_string)),
                ],
            )
            .await?;
        Ok(data
            .into_iter()
            .map(|item| (item.inst_id.clone(), item))
            .collect())
    }

    /// `GET /api/v5/market/tickers`. Map of instId -> ticker.
    /// `inst_type`: SPOT, SWAP, FUTURES, OPTION or EVENTS; `inst_family` applies to FUTURES/SWAP/OPTION.
    pub async fn get_tickers(
        &self,
        inst_type: &str,
        inst_family: Option<&str>,
    ) -> Result<HashMap<String, OkxTickerData>, BoxError> {
        let data: Vec<OkxTickerData> = self
            .get(
                "market/tickers",
                &[
                    ("instType", Some(inst_type.to_string())),
                    ("instFamily", inst_family.map(str::to_string)),
                ],
            )
            .await?;
        Ok(data
            .into_iter()
            .map(|item| (item.inst_id.clone(), item))
            .collect())
    }

    /// `GET /api/v5/market/ticker`: one instrument.
    pub async fn get_ticker(&self, inst_id: &str) -> Result<OkxTickerData, BoxError> {
        let data: Vec<OkxTickerData> = self
            .get("market/ticker", &[("instId", Some(inst_id.to_string()))])
            .await?;
        single("market/ticker", data)
    }

    /// `GET /api/v5/market/books`. `sz`: depth per side (OKX max 400, default 1).
    pub async fn get_order_book(
        &self,
        inst_id: &str,
        sz: Option<u32>,
    ) -> Result<OkxOrderBook, BoxError> {
        let data: Vec<OkxOrderBook> = self
            .get(
                "market/books",
                &[
                    ("instId", Some(inst_id.to_string())),
                    ("sz", sz.map(|v| v.to_string())),
                ],
            )
            .await?;
        single("market/books", data)
    }

    /// `GET /api/v5/market/books-full`. `sz`: depth per side (OKX max 5000, default 1).
    pub async fn get_order_book_full(
        &self,
        inst_id: &str,
        sz: Option<u32>,
    ) -> Result<OkxOrderBookFull, BoxError> {
        let data: Vec<OkxOrderBookFull> = self
            .get(
                "market/books-full",
                &[
                    ("instId", Some(inst_id.to_string())),
                    ("sz", sz.map(|v| v.to_string())),
                ],
            )
            .await?;
        single("market/books-full", data)
    }

    /// `GET /api/v5/market/books-rpi`: book including Retail Price Improvement orders; level index 2 is the size excluding RPI. `sz`: depth per side (OKX max 400, default 1).
    pub async fn get_order_book_rpi(
        &self,
        inst_id: &str,
        sz: Option<u32>,
    ) -> Result<OkxOrderBook, BoxError> {
        let data: Vec<OkxOrderBook> = self
            .get(
                "market/books-rpi",
                &[
                    ("instId", Some(inst_id.to_string())),
                    ("sz", sz.map(|v| v.to_string())),
                ],
            )
            .await?;
        single("market/books-rpi", data)
    }

    /// `GET /api/v5/market/trades`: most recent trades, newest first. `limit`: OKX max 500, default 100.
    pub async fn get_trades(
        &self,
        inst_id: &str,
        limit: Option<u32>,
    ) -> Result<Vec<OkxTrade>, BoxError> {
        let data: Vec<OkxTrade> = self
            .get(
                "market/trades",
                &[
                    ("instId", Some(inst_id.to_string())),
                    ("limit", limit.map(|v| v.to_string())),
                ],
            )
            .await?;
        Ok(data)
    }

    /// `GET /api/v5/market/history-trades` (last 3 months). `pagination_type`: 1 = by tradeId (OKX default), 2 = by timestamp. `after`/`before` are tradeIds or (after only) ms timestamps per `pagination_type`. `limit`: OKX max/default 100.
    pub async fn get_history_trades(
        &self,
        inst_id: &str,
        pagination_type: Option<u32>,
        after: Option<&str>,
        before: Option<&str>,
        limit: Option<u32>,
    ) -> Result<Vec<OkxTrade>, BoxError> {
        let data: Vec<OkxTrade> = self
            .get(
                "market/history-trades",
                &[
                    ("instId", Some(inst_id.to_string())),
                    ("type", pagination_type.map(|v| v.to_string())),
                    ("after", after.map(str::to_string)),
                    ("before", before.map(str::to_string)),
                    ("limit", limit.map(|v| v.to_string())),
                ],
            )
            .await?;
        Ok(data)
    }

    /// `GET /api/v5/market/candles`: roughly the latest 1,440 bars, newest first. `bar` defaults to 1m at OKX; `after`/`before` are ms timestamps; `limit` max 300 (default 100); `adjust` = "forward" for equity perpetuals.
    pub async fn get_candles(
        &self,
        inst_id: &str,
        bar: Option<&str>,
        after: Option<i64>,
        before: Option<i64>,
        limit: Option<u32>,
        adjust: Option<&str>,
    ) -> Result<Vec<OkxCandle>, BoxError> {
        let data: Vec<OkxCandle> = self
            .get(
                "market/candles",
                &[
                    ("instId", Some(inst_id.to_string())),
                    ("bar", bar.map(str::to_string)),
                    ("after", after.map(|v| v.to_string())),
                    ("before", before.map(|v| v.to_string())),
                    ("limit", limit.map(|v| v.to_string())),
                    ("adjust", adjust.map(str::to_string)),
                ],
            )
            .await?;
        Ok(data)
    }

    /// `GET /api/v5/market/history-candles`: older bars than `/market/candles`. Same parameters.
    pub async fn get_history_candles(
        &self,
        inst_id: &str,
        bar: Option<&str>,
        after: Option<i64>,
        before: Option<i64>,
        limit: Option<u32>,
        adjust: Option<&str>,
    ) -> Result<Vec<OkxCandle>, BoxError> {
        let data: Vec<OkxCandle> = self
            .get(
                "market/history-candles",
                &[
                    ("instId", Some(inst_id.to_string())),
                    ("bar", bar.map(str::to_string)),
                    ("after", after.map(|v| v.to_string())),
                    ("before", before.map(|v| v.to_string())),
                    ("limit", limit.map(|v| v.to_string())),
                    ("adjust", adjust.map(str::to_string)),
                ],
            )
            .await?;
        Ok(data)
    }

    /// `GET /api/v5/market/mark-price-candles` (recent mark-price bars). `bar` defaults to 1m at OKX; `after`/`before` are ms timestamps; `limit` max/default 100.
    pub async fn get_mark_price_candles(
        &self,
        inst_id: &str,
        bar: Option<&str>,
        after: Option<i64>,
        before: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<OkxPriceCandle>, BoxError> {
        let data: Vec<OkxPriceCandle> = self
            .get(
                "market/mark-price-candles",
                &[
                    ("instId", Some(inst_id.to_string())),
                    ("bar", bar.map(str::to_string)),
                    ("after", after.map(|v| v.to_string())),
                    ("before", before.map(|v| v.to_string())),
                    ("limit", limit.map(|v| v.to_string())),
                ],
            )
            .await?;
        Ok(data)
    }

    /// `GET /api/v5/market/history-mark-price-candles`. `bar` defaults to 1m at OKX; `after`/`before` are ms timestamps; `limit` max/default 100.
    pub async fn get_history_mark_price_candles(
        &self,
        inst_id: &str,
        bar: Option<&str>,
        after: Option<i64>,
        before: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<OkxPriceCandle>, BoxError> {
        let data: Vec<OkxPriceCandle> = self
            .get(
                "market/history-mark-price-candles",
                &[
                    ("instId", Some(inst_id.to_string())),
                    ("bar", bar.map(str::to_string)),
                    ("after", after.map(|v| v.to_string())),
                    ("before", before.map(|v| v.to_string())),
                    ("limit", limit.map(|v| v.to_string())),
                ],
            )
            .await?;
        Ok(data)
    }

    /// `GET /api/v5/market/index-candles` (`inst_id` is an index, e.g. "BTC-USD"). `bar` defaults to 1m at OKX; `after`/`before` are ms timestamps; `limit` max/default 100.
    pub async fn get_index_candles(
        &self,
        inst_id: &str,
        bar: Option<&str>,
        after: Option<i64>,
        before: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<OkxPriceCandle>, BoxError> {
        let data: Vec<OkxPriceCandle> = self
            .get(
                "market/index-candles",
                &[
                    ("instId", Some(inst_id.to_string())),
                    ("bar", bar.map(str::to_string)),
                    ("after", after.map(|v| v.to_string())),
                    ("before", before.map(|v| v.to_string())),
                    ("limit", limit.map(|v| v.to_string())),
                ],
            )
            .await?;
        Ok(data)
    }

    /// `GET /api/v5/market/history-index-candles` (`inst_id` is an index). `bar` defaults to 1m at OKX; `after`/`before` are ms timestamps; `limit` max/default 100.
    pub async fn get_history_index_candles(
        &self,
        inst_id: &str,
        bar: Option<&str>,
        after: Option<i64>,
        before: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<OkxPriceCandle>, BoxError> {
        let data: Vec<OkxPriceCandle> = self
            .get(
                "market/history-index-candles",
                &[
                    ("instId", Some(inst_id.to_string())),
                    ("bar", bar.map(str::to_string)),
                    ("after", after.map(|v| v.to_string())),
                    ("before", before.map(|v| v.to_string())),
                    ("limit", limit.map(|v| v.to_string())),
                ],
            )
            .await?;
        Ok(data)
    }

    /// `GET /api/v5/market/index-tickers`. OKX requires one of `quote_ccy` or `inst_id`.
    pub async fn get_index_tickers(
        &self,
        quote_ccy: Option<&str>,
        inst_id: Option<&str>,
    ) -> Result<Vec<OkxIndexTicker>, BoxError> {
        let data: Vec<OkxIndexTicker> = self
            .get(
                "market/index-tickers",
                &[
                    ("quoteCcy", quote_ccy.map(str::to_string)),
                    ("instId", inst_id.map(str::to_string)),
                ],
            )
            .await?;
        Ok(data)
    }

    /// `GET /api/v5/market/index-components` (e.g. index "BTC-USDT").
    pub async fn get_index_components(&self, index: &str) -> Result<OkxIndexComponents, BoxError> {
        let data: OkxIndexComponents = self
            .get(
                "market/index-components",
                &[("index", Some(index.to_string()))],
            )
            .await?;
        Ok(data)
    }

    /// `GET /api/v5/market/exchange-rate` (USD/CNY, averaged over two weeks).
    pub async fn get_exchange_rate(&self) -> Result<OkxExchangeRate, BoxError> {
        let data: Vec<OkxExchangeRate> = self.get("market/exchange-rate", &[]).await?;
        single("market/exchange-rate", data)
    }

    /// `GET /api/v5/market/platform-24-volume`.
    pub async fn get_platform_24_volume(&self) -> Result<OkxPlatformVolume, BoxError> {
        let data: Vec<OkxPlatformVolume> = self.get("market/platform-24-volume", &[]).await?;
        single("market/platform-24-volume", data)
    }

    /// `GET /api/v5/market/call-auction-details`.
    pub async fn get_call_auction_details(
        &self,
        inst_id: &str,
    ) -> Result<OkxCallAuctionDetails, BoxError> {
        let data: Vec<OkxCallAuctionDetails> = self
            .get(
                "market/call-auction-details",
                &[("instId", Some(inst_id.to_string()))],
            )
            .await?;
        single("market/call-auction-details", data)
    }

    /// `GET /api/v5/market/option/instrument-family-trades`: latest option trades grouped by call/put.
    pub async fn get_option_family_trades(
        &self,
        inst_family: &str,
    ) -> Result<Vec<OkxOptionFamilyTrades>, BoxError> {
        let data: Vec<OkxOptionFamilyTrades> = self
            .get(
                "market/option/instrument-family-trades",
                &[("instFamily", Some(inst_family.to_string()))],
            )
            .await?;
        Ok(data)
    }

    /// `GET /api/v5/public/funding-rate`. `inst_id` may be "ANY" for every perpetual.
    pub async fn get_funding_rate(&self, inst_id: &str) -> Result<Vec<OkxFundingRate>, BoxError> {
        let data: Vec<OkxFundingRate> = self
            .get(
                "public/funding-rate",
                &[("instId", Some(inst_id.to_string()))],
            )
            .await?;
        Ok(data)
    }

    /// `GET /api/v5/public/funding-rate-history`. `before`/`after` are fundingTime ms; `limit` max/default 400.
    pub async fn get_funding_rate_history(
        &self,
        inst_id: &str,
        before: Option<i64>,
        after: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<OkxFundingRateHistory>, BoxError> {
        let data: Vec<OkxFundingRateHistory> = self
            .get(
                "public/funding-rate-history",
                &[
                    ("instId", Some(inst_id.to_string())),
                    ("before", before.map(|v| v.to_string())),
                    ("after", after.map(|v| v.to_string())),
                    ("limit", limit.map(|v| v.to_string())),
                ],
            )
            .await?;
        Ok(data)
    }

    /// `GET /api/v5/public/open-interest`. `inst_type`: SWAP, FUTURES, OPTION or EVENTS; `inst_family` required for OPTION.
    pub async fn get_open_interest(
        &self,
        inst_type: &str,
        inst_family: Option<&str>,
        inst_id: Option<&str>,
    ) -> Result<Vec<OkxOpenInterest>, BoxError> {
        let data: Vec<OkxOpenInterest> = self
            .get(
                "public/open-interest",
                &[
                    ("instType", Some(inst_type.to_string())),
                    ("instFamily", inst_family.map(str::to_string)),
                    ("instId", inst_id.map(str::to_string)),
                ],
            )
            .await?;
        Ok(data)
    }

    /// `GET /api/v5/public/price-limit`.
    pub async fn get_price_limit(&self, inst_id: &str) -> Result<OkxPriceLimit, BoxError> {
        let data: Vec<OkxPriceLimit> = self
            .get(
                "public/price-limit",
                &[("instId", Some(inst_id.to_string()))],
            )
            .await?;
        single("public/price-limit", data)
    }

    /// `GET /api/v5/public/mark-price`. `inst_type`: MARGIN, SWAP, FUTURES, OPTION or EVENTS.
    pub async fn get_mark_price(
        &self,
        inst_type: &str,
        inst_family: Option<&str>,
        inst_id: Option<&str>,
    ) -> Result<Vec<OkxMarkPrice>, BoxError> {
        let data: Vec<OkxMarkPrice> = self
            .get(
                "public/mark-price",
                &[
                    ("instType", Some(inst_type.to_string())),
                    ("instFamily", inst_family.map(str::to_string)),
                    ("instId", inst_id.map(str::to_string)),
                ],
            )
            .await?;
        Ok(data)
    }

    /// `GET /api/v5/public/estimated-price` (FUTURES/OPTION/EVENTS; published one hour before delivery/exercise).
    pub async fn get_estimated_price(&self, inst_id: &str) -> Result<OkxEstimatedPrice, BoxError> {
        let data: Vec<OkxEstimatedPrice> = self
            .get(
                "public/estimated-price",
                &[("instId", Some(inst_id.to_string()))],
            )
            .await?;
        single("public/estimated-price", data)
    }

    /// `GET /api/v5/public/estimated-settlement-info` (FUTURES; data only in the 30 minutes before settlement, OKX error 51001 otherwise).
    pub async fn get_estimated_settlement_info(
        &self,
        inst_id: &str,
    ) -> Result<OkxEstimatedSettlement, BoxError> {
        let data: Vec<OkxEstimatedSettlement> = self
            .get(
                "public/estimated-settlement-info",
                &[("instId", Some(inst_id.to_string()))],
            )
            .await?;
        single("public/estimated-settlement-info", data)
    }

    /// `GET /api/v5/public/delivery-exercise-history`. `inst_type`: FUTURES or OPTION; `after`/`before` ms; `limit` max/default 100.
    pub async fn get_delivery_exercise_history(
        &self,
        inst_type: &str,
        inst_family: &str,
        after: Option<i64>,
        before: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<OkxDeliveryExercise>, BoxError> {
        let data: Vec<OkxDeliveryExercise> = self
            .get(
                "public/delivery-exercise-history",
                &[
                    ("instType", Some(inst_type.to_string())),
                    ("instFamily", Some(inst_family.to_string())),
                    ("after", after.map(|v| v.to_string())),
                    ("before", before.map(|v| v.to_string())),
                    ("limit", limit.map(|v| v.to_string())),
                ],
            )
            .await?;
        Ok(data)
    }

    /// `GET /api/v5/public/settlement-history` (futures settlements, last 3 months).
    pub async fn get_settlement_history(
        &self,
        inst_family: &str,
        after: Option<i64>,
        before: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<OkxSettlement>, BoxError> {
        let data: Vec<OkxSettlement> = self
            .get(
                "public/settlement-history",
                &[
                    ("instFamily", Some(inst_family.to_string())),
                    ("after", after.map(|v| v.to_string())),
                    ("before", before.map(|v| v.to_string())),
                    ("limit", limit.map(|v| v.to_string())),
                ],
            )
            .await?;
        Ok(data)
    }

    /// `GET /api/v5/public/insurance-fund`. `kind` is OKX `type`; `inst_family` required for FUTURES/SWAP/OPTION, `ccy` for MARGIN.
    pub async fn get_insurance_fund(
        &self,
        inst_type: &str,
        kind: Option<&str>,
        inst_family: Option<&str>,
        ccy: Option<&str>,
        before: Option<i64>,
        after: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<OkxInsuranceFund>, BoxError> {
        let data: Vec<OkxInsuranceFund> = self
            .get(
                "public/insurance-fund",
                &[
                    ("instType", Some(inst_type.to_string())),
                    ("type", kind.map(str::to_string)),
                    ("instFamily", inst_family.map(str::to_string)),
                    ("ccy", ccy.map(str::to_string)),
                    ("before", before.map(|v| v.to_string())),
                    ("after", after.map(|v| v.to_string())),
                    ("limit", limit.map(|v| v.to_string())),
                ],
            )
            .await?;
        Ok(data)
    }

    /// `GET /api/v5/public/premium-history` (SWAP; last 6 months).
    pub async fn get_premium_history(
        &self,
        inst_id: &str,
        after: Option<i64>,
        before: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<OkxPremium>, BoxError> {
        let data: Vec<OkxPremium> = self
            .get(
                "public/premium-history",
                &[
                    ("instId", Some(inst_id.to_string())),
                    ("after", after.map(|v| v.to_string())),
                    ("before", before.map(|v| v.to_string())),
                    ("limit", limit.map(|v| v.to_string())),
                ],
            )
            .await?;
        Ok(data)
    }

    /// `GET /api/v5/public/opt-summary`. `exp_time` is "YYMMDD".
    pub async fn get_opt_summary(
        &self,
        inst_family: &str,
        exp_time: Option<&str>,
    ) -> Result<Vec<OkxOptionSummary>, BoxError> {
        let data: Vec<OkxOptionSummary> = self
            .get(
                "public/opt-summary",
                &[
                    ("instFamily", Some(inst_family.to_string())),
                    ("expTime", exp_time.map(str::to_string)),
                ],
            )
            .await?;
        Ok(data)
    }

    /// `GET /api/v5/public/option-trades`. OKX requires `inst_id` or `inst_family`; `opt_type` is "C" or "P".
    pub async fn get_option_trades(
        &self,
        inst_id: Option<&str>,
        inst_family: Option<&str>,
        opt_type: Option<&str>,
    ) -> Result<Vec<OkxOptionTrade>, BoxError> {
        let data: Vec<OkxOptionTrade> = self
            .get(
                "public/option-trades",
                &[
                    ("instId", inst_id.map(str::to_string)),
                    ("instFamily", inst_family.map(str::to_string)),
                    ("optType", opt_type.map(str::to_string)),
                ],
            )
            .await?;
        Ok(data)
    }

    /// `GET /api/v5/public/underlying`. `inst_type`: SWAP, FUTURES or OPTION.
    pub async fn get_underlying(&self, inst_type: &str) -> Result<Vec<String>, BoxError> {
        let data: Vec<Vec<String>> = self
            .get(
                "public/underlying",
                &[("instType", Some(inst_type.to_string()))],
            )
            .await?;
        Ok(data.into_iter().next().unwrap_or_default())
    }

    /// `GET /api/v5/public/position-tiers`. `td_mode`: cross or isolated; see OKX docs for which of `inst_family`/`inst_id`/`ccy` each `inst_type` needs.
    pub async fn get_position_tiers(
        &self,
        inst_type: &str,
        td_mode: &str,
        inst_family: Option<&str>,
        inst_id: Option<&str>,
        ccy: Option<&str>,
        tier: Option<&str>,
    ) -> Result<Vec<OkxPositionTier>, BoxError> {
        let data: Vec<OkxPositionTier> = self
            .get(
                "public/position-tiers",
                &[
                    ("instType", Some(inst_type.to_string())),
                    ("tdMode", Some(td_mode.to_string())),
                    ("instFamily", inst_family.map(str::to_string)),
                    ("instId", inst_id.map(str::to_string)),
                    ("ccy", ccy.map(str::to_string)),
                    ("tier", tier.map(str::to_string)),
                ],
            )
            .await?;
        Ok(data)
    }

    /// `GET /api/v5/public/instrument-tick-bands`. `inst_type`: OPTION or EVENTS.
    pub async fn get_instrument_tick_bands(
        &self,
        inst_type: &str,
        inst_family: Option<&str>,
    ) -> Result<Vec<OkxInstrumentTickBands>, BoxError> {
        let data: Vec<OkxInstrumentTickBands> = self
            .get(
                "public/instrument-tick-bands",
                &[
                    ("instType", Some(inst_type.to_string())),
                    ("instFamily", inst_family.map(str::to_string)),
                ],
            )
            .await?;
        Ok(data)
    }

    /// `GET /api/v5/public/event-contract/series` (all series when `series_id` is None).
    pub async fn get_event_series(
        &self,
        series_id: Option<&str>,
    ) -> Result<Vec<OkxEventSeries>, BoxError> {
        let data: Vec<OkxEventSeries> = self
            .get(
                "public/event-contract/series",
                &[("seriesId", series_id.map(str::to_string))],
            )
            .await?;
        Ok(data)
    }

    /// `GET /api/v5/public/event-contract/events`. `state`: preopen, live, settling or expired; `before`/`after` are expTime ms; `limit` max/default 100.
    pub async fn get_events(
        &self,
        series_id: &str,
        event_id: Option<&str>,
        state: Option<&str>,
        limit: Option<u32>,
        before: Option<i64>,
        after: Option<i64>,
    ) -> Result<Vec<OkxEvent>, BoxError> {
        let data: Vec<OkxEvent> = self
            .get(
                "public/event-contract/events",
                &[
                    ("seriesId", Some(series_id.to_string())),
                    ("eventId", event_id.map(str::to_string)),
                    ("state", state.map(str::to_string)),
                    ("limit", limit.map(|v| v.to_string())),
                    ("before", before.map(|v| v.to_string())),
                    ("after", after.map(|v| v.to_string())),
                ],
            )
            .await?;
        Ok(data)
    }

    /// `GET /api/v5/public/event-contract/markets`. Same filters as `get_events` plus `inst_id`.
    pub async fn get_event_markets(
        &self,
        series_id: &str,
        event_id: Option<&str>,
        inst_id: Option<&str>,
        state: Option<&str>,
        limit: Option<u32>,
        before: Option<i64>,
        after: Option<i64>,
    ) -> Result<Vec<OkxEventMarket>, BoxError> {
        let data: Vec<OkxEventMarket> = self
            .get(
                "public/event-contract/markets",
                &[
                    ("seriesId", Some(series_id.to_string())),
                    ("eventId", event_id.map(str::to_string)),
                    ("instId", inst_id.map(str::to_string)),
                    ("state", state.map(str::to_string)),
                    ("limit", limit.map(|v| v.to_string())),
                    ("before", before.map(|v| v.to_string())),
                    ("after", after.map(|v| v.to_string())),
                ],
            )
            .await?;
        Ok(data)
    }

    /// `GET /api/v5/public/time`.
    pub async fn get_system_time(&self) -> Result<OkxSystemTime, BoxError> {
        let data: Vec<OkxSystemTime> = self.get("public/time", &[]).await?;
        single("public/time", data)
    }

    /// `GET /api/v5/system/status`. `state`: scheduled, ongoing, pre_open, completed or canceled (OKX default: scheduled + ongoing + pre_open).
    pub async fn get_system_status(
        &self,
        state: Option<&str>,
    ) -> Result<Vec<OkxSystemStatus>, BoxError> {
        let data: Vec<OkxSystemStatus> = self
            .get("system/status", &[("state", state.map(str::to_string))])
            .await?;
        Ok(data)
    }
}

#[cfg(test)]
mod fixture_tests {
    use super::*;
    use serde_json::Value;

    macro_rules! fixture {
        ($name:literal) => {
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/rest/",
                $name,
                ".json"
            ))
        };
    }

    /// Removes `field` from the first object in `data` and re-serialises the body.
    fn without(text: &str, field: &str) -> String {
        let mut v: Value = serde_json::from_str(text).unwrap();
        let data = &mut v["data"];
        let obj = if data.is_array() { &mut data[0] } else { data };
        assert!(
            obj.as_object_mut().unwrap().remove(field).is_some(),
            "{field} not in fixture"
        );
        v.to_string()
    }

    /// Drops the last element of the first row of an array-of-arrays `data`.
    fn short_row(text: &str) -> String {
        let mut v: Value = serde_json::from_str(text).unwrap();
        v["data"][0].as_array_mut().unwrap().pop();
        v.to_string()
    }

    #[test]
    fn parses_ticker() {
        let data: Vec<OkxTickerData> = parse_response("ticker", fixture!("ticker")).unwrap();
        assert!(!data.is_empty());
        assert!(parse_response::<Vec<OkxTickerData>>(
            "ticker",
            &without(fixture!("ticker"), "instId")
        )
        .is_err());
    }

    #[test]
    fn parses_books() {
        let data: Vec<OkxOrderBook> = parse_response("books", fixture!("books")).unwrap();
        assert!(!data.is_empty());
        assert!(
            parse_response::<Vec<OkxOrderBook>>("books", &without(fixture!("books"), "seqId"))
                .is_err()
        );
    }

    #[test]
    fn parses_books_full() {
        let data: Vec<OkxOrderBookFull> =
            parse_response("books_full", fixture!("books_full")).unwrap();
        assert!(!data.is_empty());
        assert!(parse_response::<Vec<OkxOrderBookFull>>(
            "books_full",
            &without(fixture!("books_full"), "seqId")
        )
        .is_err());
    }

    #[test]
    fn parses_books_rpi() {
        let data: Vec<OkxOrderBook> = parse_response("books_rpi", fixture!("books_rpi")).unwrap();
        assert!(!data.is_empty());
        assert!(parse_response::<Vec<OkxOrderBook>>(
            "books_rpi",
            &without(fixture!("books_rpi"), "asks")
        )
        .is_err());
    }

    #[test]
    fn parses_trades() {
        let data: Vec<OkxTrade> = parse_response("trades", fixture!("trades")).unwrap();
        assert!(!data.is_empty());
        assert!(
            parse_response::<Vec<OkxTrade>>("trades", &without(fixture!("trades"), "tradeId"))
                .is_err()
        );
    }

    #[test]
    fn parses_history_trades() {
        let data: Vec<OkxTrade> =
            parse_response("history_trades", fixture!("history_trades")).unwrap();
        assert!(!data.is_empty());
        assert!(parse_response::<Vec<OkxTrade>>(
            "history_trades",
            &without(fixture!("history_trades"), "source")
        )
        .is_err());
    }

    #[test]
    fn parses_candles() {
        let data: Vec<OkxCandle> = parse_response("candles", fixture!("candles")).unwrap();
        assert!(!data.is_empty());
        assert!(
            parse_response::<Vec<OkxCandle>>("candles", &short_row(fixture!("candles"))).is_err()
        );
    }

    #[test]
    fn parses_mark_price_candles() {
        let data: Vec<OkxPriceCandle> =
            parse_response("mark_price_candles", fixture!("mark_price_candles")).unwrap();
        assert!(!data.is_empty());
        assert!(parse_response::<Vec<OkxPriceCandle>>(
            "mark_price_candles",
            &short_row(fixture!("mark_price_candles"))
        )
        .is_err());
    }

    #[test]
    fn parses_index_candles() {
        let data: Vec<OkxPriceCandle> =
            parse_response("index_candles", fixture!("index_candles")).unwrap();
        assert!(!data.is_empty());
        assert!(parse_response::<Vec<OkxPriceCandle>>(
            "index_candles",
            &short_row(fixture!("index_candles"))
        )
        .is_err());
    }

    #[test]
    fn parses_index_tickers() {
        let data: Vec<OkxIndexTicker> =
            parse_response("index_tickers", fixture!("index_tickers")).unwrap();
        assert!(!data.is_empty());
        assert!(parse_response::<Vec<OkxIndexTicker>>(
            "index_tickers",
            &without(fixture!("index_tickers"), "idxPx")
        )
        .is_err());
    }

    #[test]
    fn parses_funding_rate() {
        let data: Vec<OkxFundingRate> =
            parse_response("funding_rate", fixture!("funding_rate")).unwrap();
        assert!(!data.is_empty());
        assert!(parse_response::<Vec<OkxFundingRate>>(
            "funding_rate",
            &without(fixture!("funding_rate"), "fundingRate")
        )
        .is_err());
    }

    #[test]
    fn parses_funding_rate_history() {
        let data: Vec<OkxFundingRateHistory> =
            parse_response("funding_rate_history", fixture!("funding_rate_history")).unwrap();
        assert!(!data.is_empty());
        assert!(parse_response::<Vec<OkxFundingRateHistory>>(
            "funding_rate_history",
            &without(fixture!("funding_rate_history"), "realizedRate")
        )
        .is_err());
    }

    #[test]
    fn parses_open_interest() {
        let data: Vec<OkxOpenInterest> =
            parse_response("open_interest", fixture!("open_interest")).unwrap();
        assert!(!data.is_empty());
        assert!(parse_response::<Vec<OkxOpenInterest>>(
            "open_interest",
            &without(fixture!("open_interest"), "oi")
        )
        .is_err());
    }

    #[test]
    fn parses_price_limit() {
        let data: Vec<OkxPriceLimit> =
            parse_response("price_limit", fixture!("price_limit")).unwrap();
        assert!(!data.is_empty());
        assert!(parse_response::<Vec<OkxPriceLimit>>(
            "price_limit",
            &without(fixture!("price_limit"), "enabled")
        )
        .is_err());
    }

    #[test]
    fn parses_mark_price() {
        let data: Vec<OkxMarkPrice> = parse_response("mark_price", fixture!("mark_price")).unwrap();
        assert!(!data.is_empty());
        assert!(parse_response::<Vec<OkxMarkPrice>>(
            "mark_price",
            &without(fixture!("mark_price"), "markPx")
        )
        .is_err());
    }

    #[test]
    fn parses_estimated_price() {
        let data: Vec<OkxEstimatedPrice> =
            parse_response("estimated_price", fixture!("estimated_price")).unwrap();
        assert!(!data.is_empty());
        assert!(parse_response::<Vec<OkxEstimatedPrice>>(
            "estimated_price",
            &without(fixture!("estimated_price"), "settlePx")
        )
        .is_err());
    }

    #[test]
    fn parses_delivery_exercise_history() {
        let data: Vec<OkxDeliveryExercise> = parse_response(
            "delivery_exercise_history",
            fixture!("delivery_exercise_history"),
        )
        .unwrap();
        assert!(!data.is_empty());
        assert!(parse_response::<Vec<OkxDeliveryExercise>>(
            "delivery_exercise_history",
            &without(fixture!("delivery_exercise_history"), "details")
        )
        .is_err());
    }

    #[test]
    fn parses_insurance_fund() {
        let data: Vec<OkxInsuranceFund> =
            parse_response("insurance_fund", fixture!("insurance_fund")).unwrap();
        assert!(!data.is_empty());
        assert!(parse_response::<Vec<OkxInsuranceFund>>(
            "insurance_fund",
            &without(fixture!("insurance_fund"), "total")
        )
        .is_err());
    }

    #[test]
    fn parses_time() {
        let data: Vec<OkxSystemTime> = parse_response("time", fixture!("time")).unwrap();
        assert!(!data.is_empty());
        assert!(
            parse_response::<Vec<OkxSystemTime>>("time", &without(fixture!("time"), "ts")).is_err()
        );
    }

    #[test]
    fn parses_system_status() {
        let data: Vec<OkxSystemStatus> =
            parse_response("system_status", fixture!("system_status")).unwrap();
        assert!(!data.is_empty());
        assert!(parse_response::<Vec<OkxSystemStatus>>(
            "system_status",
            &without(fixture!("system_status"), "state")
        )
        .is_err());
    }

    #[test]
    fn parses_instruments_events() {
        let data: Vec<OkxInstrumentInfo> =
            parse_response("instruments_events", fixture!("instruments_events")).unwrap();
        assert!(!data.is_empty());
        assert!(parse_response::<Vec<OkxInstrumentInfo>>(
            "instruments_events",
            &without(fixture!("instruments_events"), "lotSz")
        )
        .is_err());
    }

    #[test]
    fn parses_instruments_spot() {
        let data: Vec<OkxInstrumentInfo> =
            parse_response("instruments_spot", fixture!("instruments_spot")).unwrap();
        assert!(!data.is_empty());
        assert!(parse_response::<Vec<OkxInstrumentInfo>>(
            "instruments_spot",
            &without(fixture!("instruments_spot"), "tickSz")
        )
        .is_err());
    }

    #[test]
    fn parses_opt_summary() {
        let data: Vec<OkxOptionSummary> =
            parse_response("opt_summary", fixture!("opt_summary")).unwrap();
        assert!(!data.is_empty());
        assert!(parse_response::<Vec<OkxOptionSummary>>(
            "opt_summary",
            &without(fixture!("opt_summary"), "markVol")
        )
        .is_err());
    }

    #[test]
    fn parses_option_trades() {
        let data: Vec<OkxOptionTrade> =
            parse_response("option_trades", fixture!("option_trades")).unwrap();
        assert!(!data.is_empty());
        assert!(parse_response::<Vec<OkxOptionTrade>>(
            "option_trades",
            &without(fixture!("option_trades"), "fillVol")
        )
        .is_err());
    }

    #[test]
    fn parses_option_family_trades() {
        let data: Vec<OkxOptionFamilyTrades> =
            parse_response("option_family_trades", fixture!("option_family_trades")).unwrap();
        assert!(!data.is_empty());
        assert!(parse_response::<Vec<OkxOptionFamilyTrades>>(
            "option_family_trades",
            &without(fixture!("option_family_trades"), "tradeInfo")
        )
        .is_err());
    }

    #[test]
    fn parses_premium_history() {
        let data: Vec<OkxPremium> =
            parse_response("premium_history", fixture!("premium_history")).unwrap();
        assert!(!data.is_empty());
        assert!(parse_response::<Vec<OkxPremium>>(
            "premium_history",
            &without(fixture!("premium_history"), "premium")
        )
        .is_err());
    }

    #[test]
    fn parses_settlement_history() {
        let data: Vec<OkxSettlement> =
            parse_response("settlement_history", fixture!("settlement_history")).unwrap();
        assert!(!data.is_empty());
        assert!(parse_response::<Vec<OkxSettlement>>(
            "settlement_history",
            &without(fixture!("settlement_history"), "details")
        )
        .is_err());
    }

    #[test]
    fn parses_underlying() {
        let data: Vec<Vec<String>> = parse_response("underlying", fixture!("underlying")).unwrap();
        assert!(!data.is_empty());
    }

    #[test]
    fn parses_position_tiers() {
        let data: Vec<OkxPositionTier> =
            parse_response("position_tiers", fixture!("position_tiers")).unwrap();
        assert!(!data.is_empty());
        assert!(parse_response::<Vec<OkxPositionTier>>(
            "position_tiers",
            &without(fixture!("position_tiers"), "mmr")
        )
        .is_err());
    }

    #[test]
    fn parses_index_components() {
        let data: OkxIndexComponents =
            parse_response("index_components", fixture!("index_components")).unwrap();
        let _ = data;
        assert!(parse_response::<OkxIndexComponents>(
            "index_components",
            &without(fixture!("index_components"), "components")
        )
        .is_err());
    }

    #[test]
    fn parses_exchange_rate() {
        let data: Vec<OkxExchangeRate> =
            parse_response("exchange_rate", fixture!("exchange_rate")).unwrap();
        assert!(!data.is_empty());
        assert!(parse_response::<Vec<OkxExchangeRate>>(
            "exchange_rate",
            &without(fixture!("exchange_rate"), "usdCny")
        )
        .is_err());
    }

    #[test]
    fn parses_call_auction_details() {
        let data: Vec<OkxCallAuctionDetails> =
            parse_response("call_auction_details", fixture!("call_auction_details")).unwrap();
        assert!(!data.is_empty());
        assert!(parse_response::<Vec<OkxCallAuctionDetails>>(
            "call_auction_details",
            &without(fixture!("call_auction_details"), "state")
        )
        .is_err());
    }

    #[test]
    fn parses_event_series() {
        let data: Vec<OkxEventSeries> =
            parse_response("event_series", fixture!("event_series")).unwrap();
        assert!(!data.is_empty());
        assert!(parse_response::<Vec<OkxEventSeries>>(
            "event_series",
            &without(fixture!("event_series"), "settlement")
        )
        .is_err());
    }

    #[test]
    fn parses_event_events() {
        let data: Vec<OkxEvent> = parse_response("event_events", fixture!("event_events")).unwrap();
        assert!(!data.is_empty());
        assert!(parse_response::<Vec<OkxEvent>>(
            "event_events",
            &without(fixture!("event_events"), "eventId")
        )
        .is_err());
    }

    #[test]
    fn parses_event_markets() {
        let data: Vec<OkxEventMarket> =
            parse_response("event_markets", fixture!("event_markets")).unwrap();
        assert!(!data.is_empty());
        assert!(parse_response::<Vec<OkxEventMarket>>(
            "event_markets",
            &without(fixture!("event_markets"), "outcome")
        )
        .is_err());
    }

    #[test]
    fn parses_tick_bands() {
        let data: Vec<OkxInstrumentTickBands> =
            parse_response("tick_bands", fixture!("tick_bands")).unwrap();
        assert!(!data.is_empty());
        assert!(parse_response::<Vec<OkxInstrumentTickBands>>(
            "tick_bands",
            &without(fixture!("tick_bands"), "tickBand")
        )
        .is_err());
    }

    #[test]
    fn parses_platform_24_volume() {
        let data: Vec<OkxPlatformVolume> =
            parse_response("platform_24_volume", fixture!("platform_24_volume")).unwrap();
        assert!(!data.is_empty());
        assert!(parse_response::<Vec<OkxPlatformVolume>>(
            "platform_24_volume",
            &without(fixture!("platform_24_volume"), "volUsd")
        )
        .is_err());
    }

    #[test]
    fn values_from_fixtures() {
        let book: Vec<OkxOrderBook> = parse_response("books", fixture!("books")).unwrap();
        assert_eq!(book[0].seq_id, 354420275084);
        assert_eq!(book[0].asks[0][0], "64572");
        let full: Vec<OkxOrderBookFull> =
            parse_response("books_full", fixture!("books_full")).unwrap();
        assert_eq!(full[0].bids[0], ["64602.9", "0.61191", "5"]);
        let candles: Vec<OkxPriceCandle> =
            parse_response("mark_price_candles", fixture!("mark_price_candles")).unwrap();
        assert_eq!(candles[0].ts, 1794826980000);
        assert_eq!(candles[1].confirm, 1);
        let limit: Vec<OkxPriceLimit> =
            parse_response("price_limit", fixture!("price_limit")).unwrap();
        assert!(limit[0].enabled);
        let status: Vec<OkxSystemStatus> =
            parse_response("system_status", fixture!("system_status")).unwrap();
        assert!(status[0].ts.is_none());
        let underlying: Vec<Vec<String>> =
            parse_response("underlying", fixture!("underlying")).unwrap();
        assert_eq!(underlying[0][0], "BLEND-USDT");
        let events: Vec<OkxInstrumentInfo> =
            parse_response("instruments_events", fixture!("instruments_events")).unwrap();
        assert_eq!(events[0].series_id.as_deref(), Some("BTC-UPDOWN-5MIN"));
        let spot: Vec<OkxInstrumentInfo> =
            parse_response("instruments_spot", fixture!("instruments_spot")).unwrap();
        // Live: some preopen SPOT rows omit seriesId.
        assert_eq!(spot[0].series_id.as_deref(), Some(""));
        assert!(spot[1].series_id.is_none());
    }

    #[test]
    fn surfaces_exchange_errors() {
        let err =
            parse_response::<Vec<OkxInstrumentInfo>>("instruments", fixture!("instruments_error"))
                .unwrap_err()
                .to_string();
        assert!(err.contains("50014") && err.contains("seriesId"), "{err}");
        let err = parse_response::<Vec<OkxEstimatedSettlement>>(
            "estimated-settlement-info",
            fixture!("estimated_settlement_info_error"),
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("51001"), "{err}");
    }

    #[test]
    fn candle_rejects_extra_element() {
        let row = r#"["1","1","1","1","1","1","extra"]"#;
        assert!(serde_json::from_str::<OkxPriceCandle>(row).is_err());
    }
}

#[cfg(test)]
mod live_tests {
    use super::*;

    #[tokio::test]
    async fn test_get_instruments_info() {
        let api = MarketApi::new(10);
        let instruments = api
            .get_instruments_info("SWAP", None, None, None)
            .await
            .expect("instruments");
        let btc = instruments.get("BTC-USDT-SWAP").expect("BTC-USDT-SWAP");
        assert_eq!(btc.inst_type, "SWAP");
        assert_eq!(btc.state, "live");
    }

    #[tokio::test]
    async fn test_get_candles() {
        let api = MarketApi::new(10);
        let candles = api
            .get_candles("BTC-USDT-SWAP", Some("1m"), None, None, Some(5), None)
            .await
            .expect("candles");
        assert!(!candles.is_empty() && candles.len() <= 5);
        assert!(candles[0].high >= candles[0].low);
    }

    #[tokio::test]
    async fn test_get_history_candles() {
        let api = MarketApi::new(10);
        // Three months back: beyond /market/candles' window.
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;
        let after = now - 90 * 24 * 3_600_000;
        let candles = api
            .get_history_candles(
                "BTC-USDT-SWAP",
                Some("1m"),
                Some(after),
                None,
                Some(100),
                None,
            )
            .await
            .expect("history candles");
        assert_eq!(candles.len(), 100);
        assert!(candles.iter().all(|c| c.ts < after));
    }

    #[tokio::test]
    async fn test_get_tickers() {
        let api = MarketApi::new(10);
        let tickers = api.get_tickers("SWAP", None).await.expect("tickers");
        assert!(!tickers["BTC-USDT-SWAP"].last.is_empty());
    }

    #[tokio::test]
    async fn test_public_snapshots() {
        let api = MarketApi::new(10);
        let inst = "BTC-USDT-SWAP";
        api.get_ticker(inst).await.expect("ticker");
        let book = api.get_order_book(inst, Some(5)).await.expect("books");
        assert_eq!(book.asks.len(), 5);
        api.get_order_book_full("BTC-USDT", Some(5))
            .await
            .expect("books-full");
        api.get_order_book_rpi(inst, Some(5))
            .await
            .expect("books-rpi");
        api.get_trades(inst, Some(5)).await.expect("trades");
        api.get_history_trades(inst, None, None, None, Some(5))
            .await
            .expect("history-trades");
        api.get_mark_price_candles(inst, None, None, None, Some(2))
            .await
            .expect("mark candles");
        api.get_history_index_candles("BTC-USDT", None, None, None, Some(2))
            .await
            .expect("index candles");
        api.get_index_tickers(None, Some("BTC-USDT"))
            .await
            .expect("index-tickers");
        api.get_funding_rate(inst).await.expect("funding-rate");
        api.get_funding_rate_history(inst, None, None, Some(2))
            .await
            .expect("funding history");
        api.get_open_interest("SWAP", None, Some(inst))
            .await
            .expect("open-interest");
        api.get_price_limit(inst).await.expect("price-limit");
        api.get_mark_price("SWAP", None, Some(inst))
            .await
            .expect("mark-price");
        api.get_insurance_fund("SWAP", None, Some("BTC-USDT"), None, None, None, Some(1))
            .await
            .expect("insurance");
        api.get_system_time().await.expect("time");
        api.get_system_status(None).await.expect("status");
        api.get_event_series(None).await.expect("event series");
    }
}
