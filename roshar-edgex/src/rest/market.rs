use super::{build_query, parse_data, read_reply, BoxError};
use crate::types::{
    EdgexContractKlines, EdgexDepth, EdgexDepthLevel, EdgexFundingRate, EdgexKline, EdgexKlineType,
    EdgexMarketStatus, EdgexMetaData, EdgexPage, EdgexPriceType, EdgexServerTime, EdgexTicker,
};
use roshar_http::RateLimitedClient;
use serde::de::DeserializeOwned;

type ApiResult<T> = Result<T, BoxError>;

/// `GET /api/v2/public/quote/getKline` query. Every field is optional per the docs.
#[derive(Debug, Clone, Default)]
pub struct EdgexKlineQuery {
    pub contract_id: Option<String>,
    pub price_type: Option<EdgexPriceType>,
    pub kline_type: Option<EdgexKlineType>,
    /// 1..=1000; exchange default 1000.
    pub size: Option<u32>,
    pub offset_data: Option<String>,
    /// Results are newest first.
    pub filter_begin_kline_time_inclusive: Option<u64>,
    pub filter_end_kline_time_exclusive: Option<u64>,
}

/// `GET /api/v2/public/quote/getMultiContractKline` query.
#[derive(Debug, Clone, Default)]
pub struct EdgexMultiKlineQuery {
    pub contract_ids: Vec<String>,
    pub price_type: Option<EdgexPriceType>,
    pub kline_type: Option<EdgexKlineType>,
    /// 1..=200; exchange default 200.
    pub size: Option<u32>,
    pub filter_begin_kline_time_inclusive: Option<u64>,
    pub filter_end_kline_time_exclusive: Option<u64>,
}

/// `GET /api/v2/public/funding/getFundingRatePage` query.
#[derive(Debug, Clone, Default)]
pub struct EdgexFundingRateQuery {
    pub contract_id: Option<String>,
    /// 1..=100; exchange default 100.
    pub size: Option<u32>,
    pub offset_data: Option<String>,
    /// Only settlement records (the per-minute predicted rates are excluded).
    pub filter_settlement_funding_rate: Option<bool>,
    pub filter_begin_time_inclusive: Option<u64>,
    pub filter_end_time_exclusive: Option<u64>,
}

pub struct MarketApi {
    client: RateLimitedClient,
    base_url: String,
}

fn push<T: ToString>(params: &mut Vec<(&'static str, String)>, key: &'static str, v: Option<T>) {
    if let Some(v) = v {
        params.push((key, v.to_string()));
    }
}

impl MarketApi {
    pub fn new(client: RateLimitedClient, base_url: &str) -> Self {
        Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
        }
    }

    async fn get<T: DeserializeOwned>(
        &self,
        path: &str,
        params: &[(&str, String)],
        what: &str,
    ) -> ApiResult<T> {
        let url = if params.is_empty() {
            format!("{}{path}", self.base_url)
        } else {
            format!("{}{path}?{}", self.base_url, build_query(params))
        };
        let resp = self.client.get(&url).await?;
        let (status, text) = read_reply(resp).await?;
        parse_data(status, &text, what)
    }

    pub async fn get_server_time(&self) -> ApiResult<EdgexServerTime> {
        self.get("/api/v2/public/meta/getServerTime", &[], "getServerTime")
            .await
    }

    pub async fn get_meta_data(&self) -> ApiResult<EdgexMetaData> {
        self.get("/api/v2/public/meta/getMetaData", &[], "getMetaData")
            .await
    }

    /// An unknown `contract_id` returns an empty list.
    pub async fn get_depth(
        &self,
        contract_id: &str,
        level: EdgexDepthLevel,
    ) -> ApiResult<Vec<EdgexDepth>> {
        let params = [
            ("contractId", contract_id.to_string()),
            ("level", level.as_u32().to_string()),
        ];
        self.get("/api/v2/public/quote/getDepth", &params, "getDepth")
            .await
    }

    pub async fn get_kline(&self, q: &EdgexKlineQuery) -> ApiResult<EdgexPage<EdgexKline>> {
        let mut p = Vec::new();
        push(&mut p, "contractId", q.contract_id.as_deref());
        push(&mut p, "priceType", q.price_type);
        push(&mut p, "klineType", q.kline_type);
        push(&mut p, "size", q.size);
        push(&mut p, "offsetData", q.offset_data.as_deref());
        push(
            &mut p,
            "filterBeginKlineTimeInclusive",
            q.filter_begin_kline_time_inclusive,
        );
        push(
            &mut p,
            "filterEndKlineTimeExclusive",
            q.filter_end_kline_time_exclusive,
        );
        self.get("/api/v2/public/quote/getKline", &p, "getKline")
            .await
    }

    pub async fn get_multi_contract_kline(
        &self,
        q: &EdgexMultiKlineQuery,
    ) -> ApiResult<Vec<EdgexContractKlines>> {
        let mut p = Vec::new();
        if !q.contract_ids.is_empty() {
            p.push(("contractIdList", q.contract_ids.join(",")));
        }
        push(&mut p, "priceType", q.price_type);
        push(&mut p, "klineType", q.kline_type);
        push(&mut p, "size", q.size);
        push(
            &mut p,
            "filterBeginKlineTimeInclusive",
            q.filter_begin_kline_time_inclusive,
        );
        push(
            &mut p,
            "filterEndKlineTimeExclusive",
            q.filter_end_kline_time_exclusive,
        );
        self.get(
            "/api/v2/public/quote/getMultiContractKline",
            &p,
            "getMultiContractKline",
        )
        .await
    }

    /// 24 h ticker. Live, omitting `contract_id` returns an empty list.
    pub async fn get_ticker(&self, contract_id: Option<&str>) -> ApiResult<Vec<EdgexTicker>> {
        let mut p = Vec::new();
        push(&mut p, "contractId", contract_id);
        self.get("/api/v2/public/quote/getTicker", &p, "getTicker")
            .await
    }

    pub async fn get_market_status(
        &self,
        contract_id: Option<&str>,
    ) -> ApiResult<EdgexMarketStatus> {
        let mut p = Vec::new();
        push(&mut p, "contractId", contract_id);
        self.get(
            "/api/v2/public/quote/getMarketStatus",
            &p,
            "getMarketStatus",
        )
        .await
    }

    /// Latest (predicted) funding rate per contract. Live, an empty list returns no rows.
    pub async fn get_latest_funding_rate(
        &self,
        contract_ids: &[&str],
    ) -> ApiResult<Vec<EdgexFundingRate>> {
        let mut p = Vec::new();
        if !contract_ids.is_empty() {
            p.push(("contractId", contract_ids.join(",")));
        }
        self.get(
            "/api/v2/public/funding/getLatestFundingRate",
            &p,
            "getLatestFundingRate",
        )
        .await
    }

    pub async fn get_funding_rate_page(
        &self,
        q: &EdgexFundingRateQuery,
    ) -> ApiResult<EdgexPage<EdgexFundingRate>> {
        let mut p = Vec::new();
        push(&mut p, "contractId", q.contract_id.as_deref());
        push(&mut p, "size", q.size);
        push(&mut p, "offsetData", q.offset_data.as_deref());
        push(
            &mut p,
            "filterSettlementFundingRate",
            q.filter_settlement_funding_rate,
        );
        push(
            &mut p,
            "filterBeginTimeInclusive",
            q.filter_begin_time_inclusive,
        );
        push(
            &mut p,
            "filterEndTimeExclusive",
            q.filter_end_time_exclusive,
        );
        self.get(
            "/api/v2/public/funding/getFundingRatePage",
            &p,
            "getFundingRatePage",
        )
        .await
    }
}
