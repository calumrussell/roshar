use crate::types::{
    AdlRisk, AssetIndex, Basis, BinanceAggTrade, BinanceApiError, BinanceContractType,
    BinanceHistoricalFundingRate, BinanceKline, BinanceKlineInterval, BinanceOrderBookSnapshot,
    BinancePremiumIndex, BinanceStatsPeriod, BinanceTrade, BookTicker, CompositeIndexInfo,
    DeliveryPrice, ExchangeInfo, FundingInfo, IndexConstituents, InsuranceBalance, LongShortRatio,
    OpenInterestData, OpenInterestHist, PriceTicker, ServerTime, TakerBuySellVolume, TickerData,
    TradingSchedule,
};
use roshar_http::RateLimitedClient;
use serde::de::DeserializeOwned;

const BASE_URL: &str = "https://fapi.binance.com";

pub type BoxError = Box<dyn std::error::Error + Send + Sync>;

type Query<'a> = [(&'a str, Option<String>)];

/// Window parameters shared by kline and statistics endpoints.
fn window(
    start_time: Option<i64>,
    end_time: Option<i64>,
    limit: Option<u32>,
) -> [(&'static str, Option<String>); 3] {
    [
        ("startTime", start_time.map(|v| v.to_string())),
        ("endTime", end_time.map(|v| v.to_string())),
        ("limit", limit.map(|v| v.to_string())),
    ]
}

fn opt<T: ToString>(v: Option<T>) -> Option<String> {
    v.map(|v| v.to_string())
}

/// Binance USDⓈ-M futures REST market data client.
pub struct BinanceRestClient {
    client: RateLimitedClient,
}

impl BinanceRestClient {
    /// `requests_per_second` counts requests, not Binance request weight.
    pub fn new(requests_per_second: u32) -> Self {
        Self {
            client: RateLimitedClient::new(requests_per_second, 1),
        }
    }

    fn url(path: &str, query: &Query<'_>) -> String {
        let mut url = format!("{BASE_URL}{path}");
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
        url
    }

    /// GET returning the body; a non-2xx status becomes an error carrying Binance's `code`/`msg`.
    async fn get_text(
        &self,
        path: &str,
        query: &Query<'_>,
        api_key: Option<&str>,
    ) -> Result<String, BoxError> {
        let url = Self::url(path, query);
        let response = match api_key {
            Some(key) => {
                self.client
                    .request(reqwest::Method::GET, &url)
                    .await
                    .header("X-MBX-APIKEY", key)
                    .send()
                    .await?
            }
            None => self.client.get(&url).await?,
        };
        let status = response.status();
        let body = response.text().await?;
        if !status.is_success() {
            return Err(match serde_json::from_str::<BinanceApiError>(&body) {
                Ok(err) => format!(
                    "Binance API error {} on {path} (HTTP {status}): {}",
                    err.code, err.msg
                ),
                Err(_) => format!("Binance API error on {path} (HTTP {status}): {body}"),
            }
            .into());
        }
        Ok(body)
    }

    async fn get<T: DeserializeOwned>(&self, path: &str, query: &Query<'_>) -> Result<T, BoxError> {
        let body = self.get_text(path, query, None).await?;
        serde_json::from_str(&body)
            .map_err(|e| format!("Failed to parse Binance {path} response: {e}").into())
    }

    /// Endpoints that return one object with `symbol` and an array without it.
    async fn get_one_or_all<T: DeserializeOwned>(
        &self,
        path: &str,
        symbol: Option<&str>,
    ) -> Result<Vec<T>, BoxError> {
        let query = [("symbol", symbol.map(str::to_string))];
        if symbol.is_some() {
            Ok(vec![self.get::<T>(path, &query).await?])
        } else {
            self.get::<Vec<T>>(path, &query).await
        }
    }

    /// `GET /fapi/v1/ping`.
    pub async fn ping(&self) -> Result<(), BoxError> {
        let _: serde_json::Map<String, serde_json::Value> = self.get("/fapi/v1/ping", &[]).await?;
        Ok(())
    }

    /// `GET /fapi/v1/time`.
    pub async fn get_server_time(&self) -> Result<ServerTime, BoxError> {
        self.get("/fapi/v1/time", &[]).await
    }

    /// `GET /fapi/v1/exchangeInfo` (weight 1).
    pub async fn get_exchange_info(&self) -> Result<ExchangeInfo, BoxError> {
        self.get("/fapi/v1/exchangeInfo", &[]).await
    }

    /// `GET /fapi/v1/depth`. `limit`: 5, 10, 20, 50, 100, 500, 1000.
    pub async fn get_depth_snapshot(
        &self,
        symbol: &str,
        limit: Option<u32>,
    ) -> Result<BinanceOrderBookSnapshot, BoxError> {
        let query = [("symbol", Some(symbol.to_string())), ("limit", opt(limit))];
        self.get("/fapi/v1/depth", &query).await
    }

    /// `GET /fapi/v1/rpiDepth`: book including RPI orders. `limit`: 1000 only.
    pub async fn get_rpi_depth_snapshot(
        &self,
        symbol: &str,
        limit: Option<u32>,
    ) -> Result<BinanceOrderBookSnapshot, BoxError> {
        let query = [("symbol", Some(symbol.to_string())), ("limit", opt(limit))];
        self.get("/fapi/v1/rpiDepth", &query).await
    }

    /// `GET /fapi/v1/trades`: recent market trades (max 1000).
    pub async fn get_recent_trades(
        &self,
        symbol: &str,
        limit: Option<u32>,
    ) -> Result<Vec<BinanceTrade>, BoxError> {
        let query = [("symbol", Some(symbol.to_string())), ("limit", opt(limit))];
        self.get("/fapi/v1/trades", &query).await
    }

    /// `GET /fapi/v1/historicalTrades`. Needs an API key in `X-MBX-APIKEY` (no signature).
    pub async fn get_historical_trades(
        &self,
        api_key: &str,
        symbol: &str,
        limit: Option<u32>,
        from_id: Option<i64>,
    ) -> Result<Vec<BinanceTrade>, BoxError> {
        let path = "/fapi/v1/historicalTrades";
        let query = [
            ("symbol", Some(symbol.to_string())),
            ("limit", opt(limit)),
            ("fromId", opt(from_id)),
        ];
        let body = self.get_text(path, &query, Some(api_key)).await?;
        serde_json::from_str(&body)
            .map_err(|e| format!("Failed to parse Binance {path} response: {e}").into())
    }

    /// `GET /fapi/v1/aggTrades`. `limit` max 1000; only the last 48 hours are queryable and a
    /// `startTime`/`endTime` window must be under 1 hour.
    pub async fn get_agg_trades(
        &self,
        symbol: &str,
        start_time: Option<i64>,
        end_time: Option<i64>,
        from_id: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<BinanceAggTrade>, BoxError> {
        let query = [
            ("symbol", Some(symbol.to_string())),
            ("fromId", opt(from_id)),
            ("startTime", opt(start_time)),
            ("endTime", opt(end_time)),
            ("limit", opt(limit)),
        ];
        self.get("/fapi/v1/aggTrades", &query).await
    }

    async fn get_klines_at(
        &self,
        path: &str,
        leading: &Query<'_>,
        interval: BinanceKlineInterval,
        start_time: Option<i64>,
        end_time: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<BinanceKline>, BoxError> {
        let mut query: Vec<(&str, Option<String>)> = leading.to_vec();
        query.push(("interval", Some(interval.as_str().to_string())));
        query.extend(window(start_time, end_time, limit));
        self.get(path, &query).await
    }

    /// `GET /fapi/v1/klines`. `limit` max 1500.
    pub async fn get_klines(
        &self,
        symbol: &str,
        interval: BinanceKlineInterval,
        start_time: Option<i64>,
        end_time: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<BinanceKline>, BoxError> {
        let leading = [("symbol", Some(symbol.to_string()))];
        self.get_klines_at(
            "/fapi/v1/klines",
            &leading,
            interval,
            start_time,
            end_time,
            limit,
        )
        .await
    }

    /// `GET /fapi/v1/continuousKlines`.
    pub async fn get_continuous_klines(
        &self,
        pair: &str,
        contract_type: BinanceContractType,
        interval: BinanceKlineInterval,
        start_time: Option<i64>,
        end_time: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<BinanceKline>, BoxError> {
        let leading = [
            ("pair", Some(pair.to_string())),
            ("contractType", Some(contract_type.as_str().to_string())),
        ];
        self.get_klines_at(
            "/fapi/v1/continuousKlines",
            &leading,
            interval,
            start_time,
            end_time,
            limit,
        )
        .await
    }

    /// `GET /fapi/v1/indexPriceKlines` (keyed by `pair`).
    pub async fn get_index_price_klines(
        &self,
        pair: &str,
        interval: BinanceKlineInterval,
        start_time: Option<i64>,
        end_time: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<BinanceKline>, BoxError> {
        let leading = [("pair", Some(pair.to_string()))];
        self.get_klines_at(
            "/fapi/v1/indexPriceKlines",
            &leading,
            interval,
            start_time,
            end_time,
            limit,
        )
        .await
    }

    /// `GET /fapi/v1/markPriceKlines`.
    pub async fn get_mark_price_klines(
        &self,
        symbol: &str,
        interval: BinanceKlineInterval,
        start_time: Option<i64>,
        end_time: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<BinanceKline>, BoxError> {
        let leading = [("symbol", Some(symbol.to_string()))];
        self.get_klines_at(
            "/fapi/v1/markPriceKlines",
            &leading,
            interval,
            start_time,
            end_time,
            limit,
        )
        .await
    }

    /// `GET /fapi/v1/premiumIndexKlines`.
    pub async fn get_premium_index_klines(
        &self,
        symbol: &str,
        interval: BinanceKlineInterval,
        start_time: Option<i64>,
        end_time: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<BinanceKline>, BoxError> {
        let leading = [("symbol", Some(symbol.to_string()))];
        self.get_klines_at(
            "/fapi/v1/premiumIndexKlines",
            &leading,
            interval,
            start_time,
            end_time,
            limit,
        )
        .await
    }

    /// `GET /fapi/v1/premiumIndex`: mark/index price and funding. Weight 10 without a symbol.
    pub async fn get_premium_index(
        &self,
        symbol: Option<&str>,
    ) -> Result<Vec<BinancePremiumIndex>, BoxError> {
        self.get_one_or_all("/fapi/v1/premiumIndex", symbol).await
    }

    /// `GET /fapi/v1/fundingRate`: one page (`limit` max 1000), ascending by time.
    /// Shares a 500 requests / 5 min / IP limit with `fundingInfo`.
    pub async fn get_funding_rate_history(
        &self,
        symbol: Option<&str>,
        start_time: Option<i64>,
        end_time: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<BinanceHistoricalFundingRate>, BoxError> {
        let mut query = vec![("symbol", symbol.map(str::to_string))];
        query.extend(window(start_time, end_time, limit));
        self.get("/fapi/v1/fundingRate", &query).await
    }

    /// `GET /fapi/v1/fundingInfo`: symbols with adjusted funding cap/floor or interval.
    pub async fn get_funding_info(&self) -> Result<Vec<FundingInfo>, BoxError> {
        self.get("/fapi/v1/fundingInfo", &[]).await
    }

    /// `GET /fapi/v1/ticker/24hr`. Weight 1 with a symbol, 40 without.
    pub async fn get_24hr_ticker(&self, symbol: Option<&str>) -> Result<Vec<TickerData>, BoxError> {
        self.get_one_or_all("/fapi/v1/ticker/24hr", symbol).await
    }

    /// `GET /fapi/v1/ticker/price` (v1; Binance recommends v2).
    pub async fn get_price_ticker(
        &self,
        symbol: Option<&str>,
    ) -> Result<Vec<PriceTicker>, BoxError> {
        self.get_one_or_all("/fapi/v1/ticker/price", symbol).await
    }

    /// `GET /fapi/v2/ticker/price`.
    pub async fn get_price_ticker_v2(
        &self,
        symbol: Option<&str>,
    ) -> Result<Vec<PriceTicker>, BoxError> {
        self.get_one_or_all("/fapi/v2/ticker/price", symbol).await
    }

    /// `GET /fapi/v1/ticker/bookTicker`.
    pub async fn get_book_ticker(&self, symbol: Option<&str>) -> Result<Vec<BookTicker>, BoxError> {
        self.get_one_or_all("/fapi/v1/ticker/bookTicker", symbol)
            .await
    }

    /// `GET /fapi/v1/openInterest`.
    pub async fn get_open_interest(&self, symbol: &str) -> Result<OpenInterestData, BoxError> {
        self.get(
            "/fapi/v1/openInterest",
            &[("symbol", Some(symbol.to_string()))],
        )
        .await
    }

    /// `GET /futures/data/delivery-price`: settlement prices of quarterly contracts.
    pub async fn get_delivery_price(&self, pair: &str) -> Result<Vec<DeliveryPrice>, BoxError> {
        self.get(
            "/futures/data/delivery-price",
            &[("pair", Some(pair.to_string()))],
        )
        .await
    }

    async fn get_stats<T: DeserializeOwned>(
        &self,
        path: &str,
        symbol: &str,
        period: BinanceStatsPeriod,
        limit: Option<u32>,
        start_time: Option<i64>,
        end_time: Option<i64>,
    ) -> Result<Vec<T>, BoxError> {
        let mut query = vec![
            ("symbol", Some(symbol.to_string())),
            ("period", Some(period.as_str().to_string())),
        ];
        query.extend(window(start_time, end_time, limit));
        self.get(path, &query).await
    }

    /// `GET /futures/data/openInterestHist`. `limit` max 500; last 1 month only.
    pub async fn get_open_interest_hist(
        &self,
        symbol: &str,
        period: BinanceStatsPeriod,
        limit: Option<u32>,
        start_time: Option<i64>,
        end_time: Option<i64>,
    ) -> Result<Vec<OpenInterestHist>, BoxError> {
        let path = "/futures/data/openInterestHist";
        self.get_stats(path, symbol, period, limit, start_time, end_time)
            .await
    }

    /// `GET /futures/data/topLongShortAccountRatio`.
    pub async fn get_top_long_short_account_ratio(
        &self,
        symbol: &str,
        period: BinanceStatsPeriod,
        limit: Option<u32>,
        start_time: Option<i64>,
        end_time: Option<i64>,
    ) -> Result<Vec<LongShortRatio>, BoxError> {
        let path = "/futures/data/topLongShortAccountRatio";
        self.get_stats(path, symbol, period, limit, start_time, end_time)
            .await
    }

    /// `GET /futures/data/topLongShortPositionRatio`.
    pub async fn get_top_long_short_position_ratio(
        &self,
        symbol: &str,
        period: BinanceStatsPeriod,
        limit: Option<u32>,
        start_time: Option<i64>,
        end_time: Option<i64>,
    ) -> Result<Vec<LongShortRatio>, BoxError> {
        let path = "/futures/data/topLongShortPositionRatio";
        self.get_stats(path, symbol, period, limit, start_time, end_time)
            .await
    }

    /// `GET /futures/data/globalLongShortAccountRatio`.
    pub async fn get_global_long_short_account_ratio(
        &self,
        symbol: &str,
        period: BinanceStatsPeriod,
        limit: Option<u32>,
        start_time: Option<i64>,
        end_time: Option<i64>,
    ) -> Result<Vec<LongShortRatio>, BoxError> {
        let path = "/futures/data/globalLongShortAccountRatio";
        self.get_stats(path, symbol, period, limit, start_time, end_time)
            .await
    }

    /// `GET /futures/data/takerlongshortRatio`.
    pub async fn get_taker_buy_sell_volume(
        &self,
        symbol: &str,
        period: BinanceStatsPeriod,
        limit: Option<u32>,
        start_time: Option<i64>,
        end_time: Option<i64>,
    ) -> Result<Vec<TakerBuySellVolume>, BoxError> {
        let path = "/futures/data/takerlongshortRatio";
        self.get_stats(path, symbol, period, limit, start_time, end_time)
            .await
    }

    /// `GET /futures/data/basis`.
    pub async fn get_basis(
        &self,
        pair: &str,
        contract_type: BinanceContractType,
        period: BinanceStatsPeriod,
        limit: Option<u32>,
        start_time: Option<i64>,
        end_time: Option<i64>,
    ) -> Result<Vec<Basis>, BoxError> {
        let mut query = vec![
            ("pair", Some(pair.to_string())),
            ("contractType", Some(contract_type.as_str().to_string())),
            ("period", Some(period.as_str().to_string())),
        ];
        query.extend(window(start_time, end_time, limit));
        self.get("/futures/data/basis", &query).await
    }

    /// `GET /fapi/v1/indexInfo`: composite index symbols.
    pub async fn get_composite_index_info(
        &self,
        symbol: Option<&str>,
    ) -> Result<Vec<CompositeIndexInfo>, BoxError> {
        self.get_one_or_all("/fapi/v1/indexInfo", symbol).await
    }

    /// `GET /fapi/v1/assetIndex`: multi-assets mode asset index (`symbol` like `BTCUSD`).
    pub async fn get_asset_index(&self, symbol: Option<&str>) -> Result<Vec<AssetIndex>, BoxError> {
        self.get_one_or_all("/fapi/v1/assetIndex", symbol).await
    }

    /// `GET /fapi/v1/constituents`: index price constituents.
    pub async fn get_index_constituents(
        &self,
        symbol: &str,
    ) -> Result<IndexConstituents, BoxError> {
        self.get(
            "/fapi/v1/constituents",
            &[("symbol", Some(symbol.to_string()))],
        )
        .await
    }

    /// `GET /fapi/v1/insuranceBalance`.
    pub async fn get_insurance_balance(
        &self,
        symbol: Option<&str>,
    ) -> Result<Vec<InsuranceBalance>, BoxError> {
        self.get_one_or_all("/fapi/v1/insuranceBalance", symbol)
            .await
    }

    /// `GET /fapi/v1/symbolAdlRisk`.
    pub async fn get_adl_risk(&self, symbol: Option<&str>) -> Result<Vec<AdlRisk>, BoxError> {
        self.get_one_or_all("/fapi/v1/symbolAdlRisk", symbol).await
    }

    /// `GET /fapi/v1/tradingSchedule`: TradFi market sessions.
    pub async fn get_trading_schedule(&self) -> Result<TradingSchedule, BoxError> {
        self.get("/fapi/v1/tradingSchedule", &[]).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_url_omits_none() {
        let url = BinanceRestClient::url(
            "/fapi/v1/aggTrades",
            &[
                ("symbol", Some("BTCUSDT".to_string())),
                ("fromId", None),
                ("limit", Some("5".to_string())),
            ],
        );
        assert_eq!(
            url,
            "https://fapi.binance.com/fapi/v1/aggTrades?symbol=BTCUSDT&limit=5"
        );
        assert_eq!(
            BinanceRestClient::url("/fapi/v1/time", &[("symbol", None)]),
            "https://fapi.binance.com/fapi/v1/time"
        );
    }

    #[tokio::test]
    async fn test_get_funding_rate_history_page() {
        let client = BinanceRestClient::new(10);
        let rates = client
            .get_funding_rate_history(Some("BTCUSDT"), None, None, Some(5))
            .await
            .expect("funding rate history");
        assert_eq!(rates.len(), 5);
        for window in rates.windows(2) {
            assert!(window[0].funding_time <= window[1].funding_time);
        }
    }
}
