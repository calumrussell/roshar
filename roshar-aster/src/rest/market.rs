use crate::types::{
    AsterAggTrade, AsterApiError, AsterBookTicker, AsterExchangeInfo, AsterFundingInfo,
    AsterFundingRate, AsterIndexReferences, AsterKline, AsterKlineInterval, AsterMarkPrice,
    AsterOpenInterest, AsterOrderBook, AsterPriceKline, AsterPriceTicker,
    AsterRemainingOpenableNotional, AsterServerTime, AsterTicker24hr, AsterTrade,
};
use roshar_http::RateLimitedClient;
use serde::de::DeserializeOwned;
use std::collections::HashMap;

const BASE_URL: &str = "https://fapi.asterdex.com";

type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// Appends `&name=value` when `value` is `Some`.
fn push_opt<T: std::fmt::Display>(url: &mut String, name: &str, value: Option<T>) {
    if let Some(v) = value {
        url.push_str(&format!("&{name}={v}"));
    }
}

/// `?symbol=<symbol>` or nothing.
fn symbol_query(symbol: Option<&str>) -> String {
    symbol.map(|s| format!("?symbol={s}")).unwrap_or_default()
}

pub struct MarketApi {
    client: RateLimitedClient,
}

impl MarketApi {
    pub fn new(requests_per_second: u32) -> Self {
        Self {
            client: RateLimitedClient::new(requests_per_second, 1),
        }
    }

    /// GETs `BASE_URL + path` and parses the body as `T`. Non-2xx bodies are parsed as
    /// `{"code":..,"msg":..}` when possible so the exchange error is surfaced.
    async fn get<T: DeserializeOwned>(&self, path: &str, what: &str) -> Result<T, BoxError> {
        let url = format!("{BASE_URL}{path}");
        let response = self.client.get(&url).await?;
        let status = response.status();
        let text = response.text().await?;
        if !status.is_success() {
            return Err(match serde_json::from_str::<AsterApiError>(&text) {
                Ok(e) => format!("Aster {what} failed ({status}): code {} {}", e.code, e.msg),
                Err(_) => format!("Aster {what} failed ({status}): {text}"),
            }
            .into());
        }
        serde_json::from_str(&text).map_err(|e| format!("Failed to parse Aster {what}: {e}").into())
    }

    /// `GET /fapi/v3/ping`. Returns `Ok(())` when the server answers `{}`.
    pub async fn ping(&self) -> Result<(), BoxError> {
        let _: serde_json::Map<String, serde_json::Value> =
            self.get("/fapi/v3/ping", "ping").await?;
        Ok(())
    }

    /// `GET /fapi/v3/time`.
    pub async fn get_server_time(&self) -> Result<AsterServerTime, BoxError> {
        self.get("/fapi/v3/time", "time").await
    }

    /// `GET /fapi/v3/exchangeInfo`.
    pub async fn get_exchange_info(&self) -> Result<AsterExchangeInfo, BoxError> {
        self.get("/fapi/v3/exchangeInfo", "exchangeInfo").await
    }

    /// Fetch an order book snapshot for a symbol.
    /// `GET /fapi/v3/depth?symbol=...[&limit=...]`
    /// Valid limits: 5, 10, 20, 50, 100, 500, 1000. `None` sends no limit (exchange default 500).
    /// Note: the diff-depth WS stream (`{symbol}@depth@100ms`) needs an initial
    /// snapshot from this endpoint before diffs can be applied correctly.
    pub async fn get_orderbook(
        &self,
        symbol: &str,
        limit: Option<u32>,
    ) -> Result<AsterOrderBook, BoxError> {
        let mut path = format!("/fapi/v3/depth?symbol={symbol}");
        push_opt(&mut path, "limit", limit);
        self.get(&path, "depth").await
    }

    /// `GET /fapi/v3/trades`. `limit`: default 500, max 1000.
    pub async fn get_recent_trades(
        &self,
        symbol: &str,
        limit: Option<u32>,
    ) -> Result<Vec<AsterTrade>, BoxError> {
        let mut path = format!("/fapi/v3/trades?symbol={symbol}");
        push_opt(&mut path, "limit", limit);
        self.get(&path, "trades").await
    }

    /// `GET /fapi/v3/historicalTrades` (weight 20). `from_id` `None` returns the most recent.
    pub async fn get_historical_trades(
        &self,
        symbol: &str,
        limit: Option<u32>,
        from_id: Option<u64>,
    ) -> Result<Vec<AsterTrade>, BoxError> {
        let mut path = format!("/fapi/v3/historicalTrades?symbol={symbol}");
        push_opt(&mut path, "limit", limit);
        push_opt(&mut path, "fromId", from_id);
        self.get(&path, "historicalTrades").await
    }

    /// `GET /fapi/v3/aggTrades` (weight 20). All filters `None` returns the most recent.
    pub async fn get_agg_trades(
        &self,
        symbol: &str,
        from_id: Option<u64>,
        start_time: Option<i64>,
        end_time: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<AsterAggTrade>, BoxError> {
        let mut path = format!("/fapi/v3/aggTrades?symbol={symbol}");
        push_opt(&mut path, "fromId", from_id);
        push_opt(&mut path, "startTime", start_time);
        push_opt(&mut path, "endTime", end_time);
        push_opt(&mut path, "limit", limit);
        self.get(&path, "aggTrades").await
    }

    /// `GET /fapi/v3/klines`. `limit`: default 500, max 1500.
    pub async fn get_klines(
        &self,
        symbol: &str,
        interval: AsterKlineInterval,
        start_time: Option<i64>,
        end_time: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<AsterKline>, BoxError> {
        let mut path = format!(
            "/fapi/v3/klines?symbol={symbol}&interval={}",
            interval.as_str()
        );
        push_opt(&mut path, "startTime", start_time);
        push_opt(&mut path, "endTime", end_time);
        push_opt(&mut path, "limit", limit);
        self.get(&path, "klines").await
    }

    /// Shared by the price-only kline endpoints. `key` is `pair` or `symbol`.
    #[allow(clippy::too_many_arguments)]
    async fn get_price_klines(
        &self,
        endpoint: &str,
        key: &str,
        value: &str,
        interval: AsterKlineInterval,
        start_time: Option<i64>,
        end_time: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<AsterPriceKline>, BoxError> {
        let mut path = format!(
            "/fapi/v3/{endpoint}?{key}={value}&interval={}",
            interval.as_str()
        );
        push_opt(&mut path, "startTime", start_time);
        push_opt(&mut path, "endTime", end_time);
        push_opt(&mut path, "limit", limit);
        self.get(&path, endpoint).await
    }

    /// `GET /fapi/v3/indexPriceKlines?pair=...`. The docs flag this path as unverified;
    /// it answered live on 2026-11-16.
    pub async fn get_index_price_klines(
        &self,
        pair: &str,
        interval: AsterKlineInterval,
        start_time: Option<i64>,
        end_time: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<AsterPriceKline>, BoxError> {
        self.get_price_klines(
            "indexPriceKlines",
            "pair",
            pair,
            interval,
            start_time,
            end_time,
            limit,
        )
        .await
    }

    /// `GET /fapi/v3/markPriceKlines?symbol=...`. The docs flag this path as unverified;
    /// it answered live on 2026-11-16.
    pub async fn get_mark_price_klines(
        &self,
        symbol: &str,
        interval: AsterKlineInterval,
        start_time: Option<i64>,
        end_time: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<AsterPriceKline>, BoxError> {
        self.get_price_klines(
            "markPriceKlines",
            "symbol",
            symbol,
            interval,
            start_time,
            end_time,
            limit,
        )
        .await
    }

    /// `GET /fapi/v3/premiumIndexKlines?symbol=...`. Live but not in the V3 docs.
    pub async fn get_premium_index_klines(
        &self,
        symbol: &str,
        interval: AsterKlineInterval,
        start_time: Option<i64>,
        end_time: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<AsterPriceKline>, BoxError> {
        self.get_price_klines(
            "premiumIndexKlines",
            "symbol",
            symbol,
            interval,
            start_time,
            end_time,
            limit,
        )
        .await
    }

    /// Fetch mark price and current funding rate for all symbols.
    /// `GET /fapi/v3/premiumIndex`
    pub async fn get_mark_prices(&self) -> Result<Vec<AsterMarkPrice>, BoxError> {
        self.get("/fapi/v3/premiumIndex", "premiumIndex").await
    }

    /// `GET /fapi/v3/premiumIndex?symbol=...`.
    pub async fn get_mark_price(&self, symbol: &str) -> Result<AsterMarkPrice, BoxError> {
        self.get(
            &format!("/fapi/v3/premiumIndex?symbol={symbol}"),
            "premiumIndex",
        )
        .await
    }

    /// `GET /fapi/v3/fundingInfo[?symbol=...]`. Always an array, also for one symbol.
    pub async fn get_funding_info(
        &self,
        symbol: Option<&str>,
    ) -> Result<Vec<AsterFundingInfo>, BoxError> {
        self.get(
            &format!("/fapi/v3/fundingInfo{}", symbol_query(symbol)),
            "fundingInfo",
        )
        .await
    }

    /// Fetch 24-hour ticker statistics for all symbols.
    /// `GET /fapi/v3/ticker/24hr`
    pub async fn get_tickers(&self) -> Result<HashMap<String, AsterTicker24hr>, BoxError> {
        let tickers: Vec<AsterTicker24hr> = self.get("/fapi/v3/ticker/24hr", "ticker/24hr").await?;
        Ok(tickers.into_iter().map(|t| (t.symbol.clone(), t)).collect())
    }

    /// `GET /fapi/v3/ticker/24hr?symbol=...` (weight 1).
    pub async fn get_ticker(&self, symbol: &str) -> Result<AsterTicker24hr, BoxError> {
        self.get(
            &format!("/fapi/v3/ticker/24hr?symbol={symbol}"),
            "ticker/24hr",
        )
        .await
    }

    /// `GET /fapi/v3/ticker/price` (all symbols, weight 2).
    pub async fn get_price_tickers(&self) -> Result<Vec<AsterPriceTicker>, BoxError> {
        self.get("/fapi/v3/ticker/price", "ticker/price").await
    }

    /// `GET /fapi/v3/ticker/price?symbol=...`.
    pub async fn get_price_ticker(&self, symbol: &str) -> Result<AsterPriceTicker, BoxError> {
        self.get(
            &format!("/fapi/v3/ticker/price?symbol={symbol}"),
            "ticker/price",
        )
        .await
    }

    /// `GET /fapi/v3/ticker/bookTicker` (all symbols, weight 2).
    pub async fn get_book_tickers(&self) -> Result<Vec<AsterBookTicker>, BoxError> {
        self.get("/fapi/v3/ticker/bookTicker", "ticker/bookTicker")
            .await
    }

    /// `GET /fapi/v3/ticker/bookTicker?symbol=...`.
    pub async fn get_book_ticker(&self, symbol: &str) -> Result<AsterBookTicker, BoxError> {
        self.get(
            &format!("/fapi/v3/ticker/bookTicker?symbol={symbol}"),
            "ticker/bookTicker",
        )
        .await
    }

    /// `GET /fapi/v3/indexreferences?symbol=...`.
    pub async fn get_index_references(
        &self,
        symbol: &str,
    ) -> Result<AsterIndexReferences, BoxError> {
        self.get(
            &format!("/fapi/v3/indexreferences?symbol={symbol}"),
            "indexreferences",
        )
        .await
    }

    /// `GET /fapi/v3/remainingOpenableNotionalValue?symbol=...&leverage=...` (weight 50).
    pub async fn get_remaining_openable_notional(
        &self,
        symbol: &str,
        leverage: u32,
    ) -> Result<AsterRemainingOpenableNotional, BoxError> {
        self.get(
            &format!("/fapi/v3/remainingOpenableNotionalValue?symbol={symbol}&leverage={leverage}"),
            "remainingOpenableNotionalValue",
        )
        .await
    }

    /// `GET /fapi/v3/openInterest?symbol=...`. Live but not in the V3 docs.
    pub async fn get_open_interest(&self, symbol: &str) -> Result<AsterOpenInterest, BoxError> {
        self.get(
            &format!("/fapi/v3/openInterest?symbol={symbol}"),
            "openInterest",
        )
        .await
    }

    /// Fetch historical funding rates for a symbol, paging through
    /// `GET /fapi/v3/fundingRate?symbol=...&startTime=...&endTime=...` at the documented max page size.
    pub async fn get_historical_funding_rates(
        &self,
        symbol: &str,
        start_time: i64,
        end_time: i64,
    ) -> Result<Vec<AsterFundingRate>, BoxError> {
        let mut all_rates: Vec<AsterFundingRate> = Vec::new();
        let mut current_start = start_time;
        const LIMIT: u32 = 1000;

        loop {
            let path = format!(
                "/fapi/v3/fundingRate?symbol={symbol}&startTime={current_start}&endTime={end_time}&limit={LIMIT}"
            );
            let rates: Vec<AsterFundingRate> = self.get(&path, "fundingRate").await?;

            let Some(last) = rates.last() else {
                break;
            };
            let last_time = last.funding_time;
            let batch_size = rates.len();
            all_rates.extend(rates);

            if batch_size < LIMIT as usize || last_time >= end_time {
                break;
            }

            current_start = last_time + 1;
        }

        Ok(all_rates)
    }
}
