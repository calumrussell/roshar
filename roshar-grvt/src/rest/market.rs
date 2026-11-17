//! Market-data REST (`https://market-data.grvt.io/full/v1/*`). Every endpoint is a JSON `POST`.

use super::{parse_reply, ApiResult};
use crate::types::{
    GrvtAllInstrumentsRequest, GrvtBookRequest, GrvtCandle, GrvtCandleInterval, GrvtCandleType,
    GrvtCollateralAssets, GrvtCurrency, GrvtEmptyRequest, GrvtFundingAggregation, GrvtFundingRate,
    GrvtFundingRequest, GrvtInstrument, GrvtInstrumentRequest, GrvtInstrumentsQuery, GrvtKind,
    GrvtKlineRequest, GrvtMarginRules, GrvtMiniTicker, GrvtOrderbook, GrvtPage, GrvtPageQuery,
    GrvtResult, GrvtServerTime, GrvtSupportedAssets, GrvtTicker, GrvtTrade,
    GrvtTradeHistoryRequest, GrvtTradeRequest,
};
use roshar_http::RateLimitedClient;
use serde::de::DeserializeOwned;
use serde::Serialize;

pub struct MarketApi {
    client: RateLimitedClient,
    base_url: String,
}

impl MarketApi {
    /// `base_url`: [`crate::GRVT_MARKET_DATA_URL`] or [`crate::GRVT_TESTNET_MARKET_DATA_URL`].
    pub fn new(base_url: &str, requests_per_second: u32) -> Self {
        Self {
            client: RateLimitedClient::new(requests_per_second, 1),
            base_url: base_url.trim_end_matches('/').to_string(),
        }
    }

    async fn post<B: Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
        what: &str,
    ) -> ApiResult<T> {
        let url = format!("{}/full/v1/{path}", self.base_url);
        let response = self
            .client
            .post(&url)
            .await
            .header("Content-Type", "application/json")
            .body(serde_json::to_string(body)?)
            .send()
            .await?;
        let status = response.status().as_u16();
        let text = response.text().await?;
        parse_reply(what, status, &text)
    }

    /// `full/v1/instrument`.
    pub async fn get_instrument(&self, instrument: &str) -> ApiResult<GrvtInstrument> {
        let r: GrvtResult<GrvtInstrument> = self
            .post(
                "instrument",
                &GrvtInstrumentRequest { instrument },
                "instrument",
            )
            .await?;
        Ok(r.result)
    }

    /// `full/v1/all_instruments`. Unset `kinds` uses the exchange default (`PERPETUAL` only).
    pub async fn get_all_instruments(
        &self,
        is_active: Option<bool>,
        kinds: Option<&[GrvtKind]>,
    ) -> ApiResult<Vec<GrvtInstrument>> {
        let r: GrvtResult<Vec<GrvtInstrument>> = self
            .post(
                "all_instruments",
                &GrvtAllInstrumentsRequest { is_active, kinds },
                "all instruments",
            )
            .await?;
        Ok(r.result)
    }

    /// `full/v1/instruments` (filtered; unset filters mean all kinds/bases/quotes, limit 500).
    pub async fn get_instruments(
        &self,
        query: &GrvtInstrumentsQuery,
    ) -> ApiResult<Vec<GrvtInstrument>> {
        let r: GrvtResult<Vec<GrvtInstrument>> =
            self.post("instruments", query, "instruments").await?;
        Ok(r.result)
    }

    /// `full/v1/currency`.
    pub async fn get_currencies(&self) -> ApiResult<Vec<GrvtCurrency>> {
        let r: GrvtResult<Vec<GrvtCurrency>> = self
            .post("currency", &GrvtEmptyRequest {}, "currency")
            .await?;
        Ok(r.result)
    }

    /// `full/v1/margin_rules`.
    pub async fn get_margin_rules(&self, instrument: &str) -> ApiResult<GrvtMarginRules> {
        self.post(
            "margin_rules",
            &GrvtInstrumentRequest { instrument },
            "margin rules",
        )
        .await
    }

    /// `full/v1/supported_assets`: assets per wallet type (funding, spot, futures by mode).
    pub async fn get_supported_assets(&self) -> ApiResult<GrvtSupportedAssets> {
        self.post("supported_assets", &GrvtEmptyRequest {}, "supported assets")
            .await
    }

    /// `full/v1/get_all_collateral_asset_info`: platform-wide collateral configuration.
    pub async fn get_all_collateral_asset_info(&self) -> ApiResult<GrvtCollateralAssets> {
        self.post(
            "get_all_collateral_asset_info",
            &GrvtEmptyRequest {},
            "collateral asset info",
        )
        .await
    }

    /// `full/v1/mini`.
    pub async fn get_mini_ticker(&self, instrument: &str) -> ApiResult<GrvtMiniTicker> {
        let r: GrvtResult<GrvtMiniTicker> = self
            .post("mini", &GrvtInstrumentRequest { instrument }, "mini ticker")
            .await?;
        Ok(r.result)
    }

    /// `full/v1/ticker` (includes the current funding rate and next funding time).
    pub async fn get_ticker(&self, instrument: &str) -> ApiResult<GrvtTicker> {
        let r: GrvtResult<GrvtTicker> = self
            .post("ticker", &GrvtInstrumentRequest { instrument }, "ticker")
            .await?;
        Ok(r.result)
    }

    /// `full/v1/book`. `depth` must be 10, 50, 100 or 500 (else code 3031).
    pub async fn get_orderbook(&self, instrument: &str, depth: u32) -> ApiResult<GrvtOrderbook> {
        let r: GrvtResult<GrvtOrderbook> = self
            .post("book", &GrvtBookRequest { instrument, depth }, "orderbook")
            .await?;
        Ok(r.result)
    }

    /// `full/v1/trade`: up to `limit` (max 1000) most recent public trades.
    pub async fn get_trades(&self, instrument: &str, limit: u32) -> ApiResult<Vec<GrvtTrade>> {
        let r: GrvtResult<Vec<GrvtTrade>> = self
            .post("trade", &GrvtTradeRequest { instrument, limit }, "trades")
            .await?;
        Ok(r.result)
    }

    /// `full/v1/trade_history` (last three months, newest first).
    pub async fn get_trade_history(
        &self,
        instrument: &str,
        page: &GrvtPageQuery,
    ) -> ApiResult<GrvtPage<GrvtTrade>> {
        self.post(
            "trade_history",
            &GrvtTradeHistoryRequest { instrument, page },
            "trade history",
        )
        .await
    }

    /// `full/v1/kline`, newest first.
    pub async fn get_candles(
        &self,
        instrument: &str,
        interval: GrvtCandleInterval,
        kind: GrvtCandleType,
        page: &GrvtPageQuery,
    ) -> ApiResult<GrvtPage<GrvtCandle>> {
        self.post(
            "kline",
            &GrvtKlineRequest {
                instrument,
                interval,
                kind,
                page,
            },
            "candles",
        )
        .await
    }

    /// `full/v1/funding`: historical funding rates, newest first. Unset `agg_type` uses the
    /// exchange default (`FUNDING_INTERVAL`).
    pub async fn get_funding_history(
        &self,
        instrument: &str,
        agg_type: Option<GrvtFundingAggregation>,
        page: &GrvtPageQuery,
    ) -> ApiResult<GrvtPage<GrvtFundingRate>> {
        self.post(
            "funding",
            &GrvtFundingRequest {
                instrument,
                page,
                agg_type,
            },
            "funding history",
        )
        .await
    }

    /// `GET /time`.
    pub async fn get_server_time(&self) -> ApiResult<GrvtServerTime> {
        let url = format!("{}/time", self.base_url);
        let response = self.client.get(&url).await?;
        let status = response.status().as_u16();
        let text = response.text().await?;
        parse_reply("server time", status, &text)
    }
}
