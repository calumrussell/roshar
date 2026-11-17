use crate::rest::{get, ApiResult, Query};
use crate::types::{
    DataEnvelope, DataHoldersParams, DataLiveVolume, DataOpenInterest, DataPage, DataPricePoint,
    DataPriceWindow, DataPricesHistoryParams, DataResolution, DataResolutionSelector, DataStatus,
    DataToken, DataTokenHolders, DataTokenSelector, DataTrade, DataTradesParams,
};
use roshar_http::RateLimitedClient;

/// Polymarket Data API v2 client, market-level public routes only (no wallet, portfolio or
/// leaderboard routes).
///
/// Errors come back as [`crate::PolymarketApiError::Status`] whose body parses as
/// [`crate::types::DataError`]. Polymarket documents 800 req / 10 s for all `/v2` routes,
/// 300 / 10 s for `/v2/trades`, 200 / 10 s for `/v2/prices-history` and 100 / 10 s for
/// `/v2/status`, per IP (https://docs.polymarket.com/api-reference/rate-limits).
pub struct DataApi {
    client: RateLimitedClient,
}

impl DataApi {
    const BASE_URL: &'static str = "https://data-api.polymarket.com";

    pub fn new(requests_per_second: u32) -> Self {
        Self {
            client: RateLimitedClient::new(requests_per_second, 1),
        }
    }

    pub fn new_with_client(client: RateLimitedClient) -> Self {
        Self { client }
    }

    /// `GET /v2/trades`: market, event or (with no filter) global trade feed, newest first.
    pub async fn trades(&self, params: &DataTradesParams) -> ApiResult<DataPage<DataTrade>> {
        get(
            &self.client,
            Self::BASE_URL,
            "/v2/trades",
            &params.query().0,
        )
        .await
    }

    /// `GET /v2/holders`: top holders per outcome token of `condition` ids (1..=20; exactly one
    /// with `include_pnl`).
    pub async fn holders(
        &self,
        condition: &[&str],
        params: &DataHoldersParams,
    ) -> ApiResult<DataPage<DataTokenHolders>> {
        get(
            &self.client,
            Self::BASE_URL,
            "/v2/holders",
            &params.query(condition).0,
        )
        .await
    }

    /// `GET /v2/oi`: open interest per condition id (at most 20). `None` returns one row with
    /// `condition_id == "GLOBAL"`. Ids that do not resolve to a servable market are absent.
    pub async fn open_interest(
        &self,
        condition: Option<&[&str]>,
    ) -> ApiResult<DataEnvelope<Vec<DataOpenInterest>>> {
        let mut q = Query::default();
        if let Some(c) = condition {
            q.req("condition", &c.join(","));
        }
        get(&self.client, Self::BASE_URL, "/v2/oi", &q.0).await
    }

    /// `GET /v2/live-volume`: cumulative taker volume per market of the given Gamma event ids
    /// (1..=20).
    pub async fn live_volume(&self, event_id: &[u64]) -> ApiResult<DataEnvelope<DataLiveVolume>> {
        let mut q = Query::default();
        q.joined("event_id", &Some(event_id.to_vec()));
        get(&self.client, Self::BASE_URL, "/v2/live-volume", &q.0).await
    }

    /// `GET /v2/prices-history`: price series of one outcome token, oldest first.
    pub async fn prices_history(
        &self,
        token_id: &str,
        window: &DataPriceWindow,
        params: &DataPricesHistoryParams,
    ) -> ApiResult<DataPage<DataPricePoint>> {
        get(
            &self.client,
            Self::BASE_URL,
            "/v2/prices-history",
            &params.query(token_id, window).0,
        )
        .await
    }

    /// `GET /v2/resolutions`: UMA / on-chain resolution state. Misses return an empty list.
    pub async fn resolutions(
        &self,
        selector: &DataResolutionSelector,
    ) -> ApiResult<DataEnvelope<Vec<DataResolution>>> {
        let mut q = Query::default();
        match selector {
            DataResolutionSelector::QuestionId(id) => {
                q.req("question_id", id);
            }
            DataResolutionSelector::Conditions(c) => {
                q.req("condition", &c.join(","));
            }
            DataResolutionSelector::EventIds(e) => {
                q.joined("event_id", &Some(e.clone()));
            }
        }
        get(&self.client, Self::BASE_URL, "/v2/resolutions", &q.0).await
    }

    /// `GET /v2/tokens`: token reference data (condition, outcome, neg-risk index, event).
    /// Unknown ids are simply absent.
    pub async fn tokens(
        &self,
        selector: &DataTokenSelector,
    ) -> ApiResult<DataEnvelope<Vec<DataToken>>> {
        let mut q = Query::default();
        match selector {
            DataTokenSelector::TokenIds(t) => {
                q.req("token_id", &t.join(","));
            }
            DataTokenSelector::Conditions(c) => {
                q.req("condition", &c.join(","));
            }
        }
        get(&self.client, Self::BASE_URL, "/v2/tokens", &q.0).await
    }

    /// `GET /v2/status`: how far behind chain head the served data is.
    pub async fn status(&self) -> ApiResult<DataEnvelope<DataStatus>> {
        get(&self.client, Self::BASE_URL, "/v2/status", &[]).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Hits the live API.
    async fn test_global_trades_and_oi() {
        let api = DataApi::new(5);
        let page = api
            .trades(&DataTradesParams {
                limit: Some(10),
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(page.data.len(), 10);
        let oi = api.open_interest(None).await.unwrap();
        assert_eq!(oi.data[0].condition_id, "GLOBAL");
        api.status().await.unwrap();
    }
}
