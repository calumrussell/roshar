use super::{parse_data, BoxError};
use crate::types::{
    PacificaFundingRate, PacificaFundingRatePage, PacificaKline, PacificaMarket, PacificaOrderBook,
    PacificaSpotAsset, PacificaTicker, PacificaTrade,
};
use roshar_http::RateLimitedClient;
use serde::Deserialize;
use std::collections::HashMap;

pub struct MarketApi {
    client: RateLimitedClient,
    base_url: &'static str,
}

/// `GET /funding_rate/history` reply: the page fields sit next to `data`, outside it.
#[derive(Deserialize)]
struct FundingHistoryReply {
    has_more: bool,
    next_cursor: Option<String>,
}

impl MarketApi {
    /// Market data from mainnet or testnet, matching `TradingApi`'s `is_mainnet`. Pass a clone of
    /// the `RateLimitedClient` the trading API uses: Pacifica's credit quota is per IP / account.
    pub fn new(client: RateLimitedClient, is_mainnet: bool) -> Self {
        Self {
            client,
            base_url: super::base_url(is_mainnet),
        }
    }

    async fn get_text(&self, url: &str) -> Result<(bool, String, String), BoxError> {
        let response = self.client.get(url).await?;
        let status = response.status();
        let text = response.text().await?;
        Ok((status.is_success(), status.to_string(), text))
    }

    /// Market specifications for every symbol.
    /// `GET /api/v1/info`
    pub async fn get_markets(&self) -> Result<Vec<PacificaMarket>, BoxError> {
        let (ok, status, text) = self.get_text(&format!("{}/info", self.base_url)).await?;
        parse_data(ok, status, &text, "info")
    }

    /// Mark, mid, oracle and funding for every symbol, keyed by symbol.
    /// `GET /api/v1/info/prices`
    pub async fn get_tickers(&self) -> Result<HashMap<String, PacificaTicker>, BoxError> {
        let (ok, status, text) = self
            .get_text(&format!("{}/info/prices", self.base_url))
            .await?;
        let tickers: Vec<PacificaTicker> = parse_data(ok, status, &text, "info/prices")?;
        Ok(tickers.into_iter().map(|t| (t.symbol.clone(), t)).collect())
    }

    /// One page of settled funding for `symbol`, newest first. Pass the previous page's
    /// `next_cursor` to continue. `limit` is sent only when given (exchange default 200, max 4000).
    /// `GET /api/v1/funding_rate/history?symbol=...&limit=...&cursor=...`
    pub async fn get_funding_rate_history(
        &self,
        symbol: &str,
        limit: Option<u32>,
        cursor: Option<&str>,
    ) -> Result<PacificaFundingRatePage, BoxError> {
        let mut url = format!("{}/funding_rate/history?symbol={symbol}", self.base_url);
        if let Some(lim) = limit {
            url.push_str(&format!("&limit={lim}"));
        }
        if let Some(c) = cursor {
            url.push_str(&format!("&cursor={c}"));
        }
        let (ok, status, text) = self.get_text(&url).await?;
        let rates: Vec<PacificaFundingRate> =
            parse_data(ok, &status, &text, "funding_rate/history")?;
        let page: FundingHistoryReply = serde_json::from_str(&text)
            .map_err(|e| format!("Failed to parse Pacifica funding_rate/history page: {e}"))?;
        Ok(PacificaFundingRatePage {
            rates,
            next_cursor: page.next_cursor,
            has_more: page.has_more,
        })
    }

    /// Order book for a symbol, up to 10 levels per side. `agg_level` is sent only when given
    /// (exchange default 1).
    /// `GET /api/v1/book?symbol=...&agg_level=...`
    pub async fn get_orderbook(
        &self,
        symbol: &str,
        agg_level: Option<u32>,
    ) -> Result<PacificaOrderBook, BoxError> {
        let mut url = format!("{}/book?symbol={symbol}", self.base_url);
        if let Some(agg) = agg_level {
            url.push_str(&format!("&agg_level={agg}"));
        }
        let (ok, status, text) = self.get_text(&url).await?;
        parse_data(ok, status, &text, "book")
    }

    /// Recent trade events for `symbol`, or across all markets when `None` (the entries carry
    /// no symbol).
    /// `GET /api/v1/trades?symbol=...`
    pub async fn get_recent_trades(
        &self,
        symbol: Option<&str>,
    ) -> Result<Vec<PacificaTrade>, BoxError> {
        let mut url = format!("{}/trades", self.base_url);
        if let Some(s) = symbol {
            url.push_str(&format!("?symbol={s}"));
        }
        let (ok, status, text) = self.get_text(&url).await?;
        parse_data(ok, status, &text, "trades")
    }

    /// Candles for a symbol from `start_time` (ms). `end_time` and `limit` are sent only when
    /// given (exchange defaults: now, and 4000 candles, the maximum).
    /// `GET /api/v1/kline?symbol=...&interval=...&start_time=...&end_time=...&limit=...`
    pub async fn get_klines(
        &self,
        symbol: &str,
        interval: &str,
        start_time: i64,
        end_time: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<PacificaKline>, BoxError> {
        self.klines("kline", symbol, interval, start_time, end_time, limit)
            .await
    }

    /// Mark price candles for a symbol from `start_time` (ms). `end_time` and `limit` are sent
    /// only when given (exchange defaults: now, and 4000 candles, the maximum).
    /// `GET /api/v1/kline/mark?symbol=...&interval=...&start_time=...&end_time=...&limit=...`
    pub async fn get_mark_price_klines(
        &self,
        symbol: &str,
        interval: &str,
        start_time: i64,
        end_time: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<PacificaKline>, BoxError> {
        self.klines("kline/mark", symbol, interval, start_time, end_time, limit)
            .await
    }

    async fn klines(
        &self,
        path: &str,
        symbol: &str,
        interval: &str,
        start_time: i64,
        end_time: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<PacificaKline>, BoxError> {
        let mut url = format!(
            "{}/{path}?symbol={symbol}&interval={interval}&start_time={start_time}",
            self.base_url
        );
        if let Some(end) = end_time {
            url.push_str(&format!("&end_time={end}"));
        }
        if let Some(lim) = limit {
            url.push_str(&format!("&limit={lim}"));
        }
        let (ok, status, text) = self.get_text(&url).await?;
        parse_data(ok, status, &text, path)
    }

    /// Spot assets, sorted by symbol. Each flag is sent only when given (exchange default for
    /// both: false).
    /// `GET /api/v1/spot_assets?include_inactive=...&collateral_enabled_only=...`
    pub async fn get_spot_assets(
        &self,
        include_inactive: Option<bool>,
        collateral_enabled_only: Option<bool>,
    ) -> Result<Vec<PacificaSpotAsset>, BoxError> {
        let mut query = Vec::new();
        if let Some(v) = include_inactive {
            query.push(format!("include_inactive={v}"));
        }
        if let Some(v) = collateral_enabled_only {
            query.push(format!("collateral_enabled_only={v}"));
        }
        let mut url = format!("{}/spot_assets", self.base_url);
        if !query.is_empty() {
            url.push('?');
            url.push_str(&query.join("&"));
        }
        let (ok, status, text) = self.get_text(&url).await?;
        parse_data(ok, status, &text, "spot_assets")
    }
}
