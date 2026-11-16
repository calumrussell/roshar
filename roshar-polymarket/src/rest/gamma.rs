use crate::rest::{get, post, ApiResult, PolymarketApiError, Query};
use crate::types::{
    EventsPage, GammaEventsPagination, GammaListMarketsParams, GammaMarketDescription,
    GammaMarketsInformationBody, GammaMarketsPage, GammaPageParams, GammaRelatedTag,
    GammaRelatedTagsParams, GammaSearch, GammaSearchParams, GammaSeriesParams, GammaSeriesSummary,
    GammaSport, GammaSportsMarketTypes, GammaTagsParams, GammaTeam, GammaTeamsParams,
    ListEventsParams, PolymarketEvent, PolymarketMarket, PolymarketSeries, PolymarketTag,
};
use roshar_http::RateLimitedClient;

/// Polymarket Gamma API client (public market metadata).
///
/// Gamma serves `/events/keyset` pages with `limit` above 50 gzip-only (HTTP 406 otherwise); this
/// crate enables reqwest's `gzip` feature so requests carry `Accept-Encoding: gzip`.
///
/// Errors come back as [`PolymarketApiError::Status`] whose body parses as
/// [`crate::types::GammaError`] (e.g. 404 `{"type": "not found error", "error": "id not found"}`).
///
/// Offset-paged `GET /events` and `GET /markets` are deprecated (`Deprecation: true`,
/// `Sunset: 1 May 2026`, `Warning: use /events/keyset` / `use /markets/keyset`) and not exposed.
pub struct GammaApi {
    client: RateLimitedClient,
}

impl GammaApi {
    const BASE_URL: &'static str = "https://gamma-api.polymarket.com";

    /// `requests_per_second` is the caller's budget. Polymarket documents 4,000 req / 10 s for
    /// Gamma in general, `/events` 500, `/markets` 300, `/tags` 200 and `/public-search` 350 per
    /// 10 s, per IP (https://docs.polymarket.com/api-reference/rate-limits).
    pub fn new(requests_per_second: u32) -> Self {
        Self {
            client: RateLimitedClient::new(requests_per_second, 1),
        }
    }

    pub fn new_with_client(client: RateLimitedClient) -> Self {
        Self { client }
    }

    async fn get<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, String)],
    ) -> ApiResult<T> {
        get(&self.client, Self::BASE_URL, path, query).await
    }

    /// `GET /status`: health check; the body is the plain text `OK`.
    pub async fn status(&self) -> ApiResult<String> {
        let url = format!("{}/status", Self::BASE_URL);
        let response = self
            .client
            .request(reqwest::Method::GET, &url)
            .await
            .send()
            .await?;
        let status = response.status();
        let body = response.text().await?;
        if !status.is_success() {
            return Err(Box::new(PolymarketApiError::Status {
                method: reqwest::Method::GET,
                path: "/status".to_string(),
                status: status.as_u16(),
                body,
            }));
        }
        Ok(body)
    }

    /// `GET /events/keyset`: one page of events with their markets.
    ///
    /// To page, set `params.after_cursor` to the returned `next_cursor`; it is `None` on the last
    /// page. Offset paging (`GET /events`) is deprecated by Polymarket and not exposed.
    ///
    /// ```no_run
    /// use roshar_polymarket::GammaApi;
    /// use roshar_polymarket::types::ListEventsParams;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    ///     let api = GammaApi::new(10);
    ///     let mut params = ListEventsParams::new(100);
    ///     params.closed = Some(false);
    ///     loop {
    ///         let page = api.list_events(&params).await?;
    ///         println!("{} events", page.events.len());
    ///         match page.next_cursor {
    ///             Some(cursor) => params.after_cursor = Some(cursor),
    ///             None => break,
    ///         }
    ///     }
    ///     Ok(())
    /// }
    /// ```
    pub async fn list_events(&self, params: &ListEventsParams) -> ApiResult<EventsPage> {
        self.get("/events/keyset", &params.query_pairs()).await
    }

    /// `GET /events/pagination`: offset-paged events with a `{hasMore, totalResults}` envelope.
    pub async fn list_events_paginated(
        &self,
        page: &GammaPageParams,
        recurrence: Option<&str>,
    ) -> ApiResult<GammaEventsPagination> {
        let mut q = page.query();
        q.opt("recurrence", &recurrence);
        self.get("/events/pagination", &q.0).await
    }

    /// `GET /events/{id}`.
    pub async fn event(&self, id: u64) -> ApiResult<PolymarketEvent> {
        self.get(&format!("/events/{id}"), &[]).await
    }

    /// `GET /events/slug/{slug}`.
    pub async fn event_by_slug(&self, slug: &str) -> ApiResult<PolymarketEvent> {
        self.get(&format!("/events/slug/{slug}"), &[]).await
    }

    /// `GET /events/{id}/tags`.
    pub async fn event_tags(&self, id: u64) -> ApiResult<Vec<PolymarketTag>> {
        self.get(&format!("/events/{id}/tags"), &[]).await
    }

    /// `GET /markets/keyset`: one page of markets. Page by passing `next_cursor` back as
    /// `after_cursor`; it is `None` on the last page.
    pub async fn list_markets(
        &self,
        params: &GammaListMarketsParams,
    ) -> ApiResult<GammaMarketsPage> {
        self.get("/markets/keyset", &params.query().0).await
    }

    /// `GET /markets/{id}`.
    pub async fn market(&self, id: u64, include_tag: Option<bool>) -> ApiResult<PolymarketMarket> {
        let mut q = Query::default();
        q.opt("include_tag", &include_tag);
        self.get(&format!("/markets/{id}"), &q.0).await
    }

    /// `GET /markets/slug/{slug}`.
    pub async fn market_by_slug(
        &self,
        slug: &str,
        include_tag: Option<bool>,
    ) -> ApiResult<PolymarketMarket> {
        let mut q = Query::default();
        q.opt("include_tag", &include_tag);
        self.get(&format!("/markets/slug/{slug}"), &q.0).await
    }

    /// `GET /markets/{id}/tags`.
    pub async fn market_tags(&self, id: u64) -> ApiResult<Vec<PolymarketTag>> {
        self.get(&format!("/markets/{id}/tags"), &[]).await
    }

    /// `GET /markets/{id}/description`.
    pub async fn market_description(&self, id: u64) -> ApiResult<GammaMarketDescription> {
        self.get(&format!("/markets/{id}/description"), &[]).await
    }

    /// `POST /markets/information`: full markets matching body filters.
    pub async fn markets_information(
        &self,
        body: &GammaMarketsInformationBody,
    ) -> ApiResult<Vec<PolymarketMarket>> {
        post(&self.client, Self::BASE_URL, "/markets/information", body).await
    }

    /// `POST /markets/abridged`: same filters, fewer fields per market.
    pub async fn markets_abridged(
        &self,
        body: &GammaMarketsInformationBody,
    ) -> ApiResult<Vec<PolymarketMarket>> {
        post(&self.client, Self::BASE_URL, "/markets/abridged", body).await
    }

    /// `GET /series`.
    pub async fn series(&self, params: &GammaSeriesParams) -> ApiResult<Vec<PolymarketSeries>> {
        self.get("/series", &params.query().0).await
    }

    /// `GET /series/{id}`.
    pub async fn series_by_id(&self, id: u64) -> ApiResult<PolymarketSeries> {
        self.get(&format!("/series/{id}"), &[]).await
    }

    /// `GET /series-summary/{id}`: event dates and weeks of a series.
    pub async fn series_summary(&self, id: u64) -> ApiResult<GammaSeriesSummary> {
        self.get(&format!("/series-summary/{id}"), &[]).await
    }

    /// `GET /series-summary/slug/{slug}`.
    pub async fn series_summary_by_slug(&self, slug: &str) -> ApiResult<GammaSeriesSummary> {
        self.get(&format!("/series-summary/slug/{slug}"), &[]).await
    }

    /// `GET /tags`.
    pub async fn tags(&self, params: &GammaTagsParams) -> ApiResult<Vec<PolymarketTag>> {
        let mut q = params.page.query();
        q.opt("include_template", &params.include_template)
            .opt("is_carousel", &params.is_carousel);
        self.get("/tags", &q.0).await
    }

    /// `GET /tags/{id}`.
    pub async fn tag(&self, id: u64) -> ApiResult<PolymarketTag> {
        self.get(&format!("/tags/{id}"), &[]).await
    }

    /// `GET /tags/slug/{slug}`.
    pub async fn tag_by_slug(&self, slug: &str) -> ApiResult<PolymarketTag> {
        self.get(&format!("/tags/slug/{slug}"), &[]).await
    }

    /// `GET /tags/{id}/related-tags`: relationship rows.
    pub async fn related_tags(
        &self,
        id: u64,
        params: &GammaRelatedTagsParams,
    ) -> ApiResult<Vec<GammaRelatedTag>> {
        self.get(&format!("/tags/{id}/related-tags"), &params.query().0)
            .await
    }

    /// `GET /tags/slug/{slug}/related-tags`.
    pub async fn related_tags_by_slug(
        &self,
        slug: &str,
        params: &GammaRelatedTagsParams,
    ) -> ApiResult<Vec<GammaRelatedTag>> {
        self.get(
            &format!("/tags/slug/{slug}/related-tags"),
            &params.query().0,
        )
        .await
    }

    /// `GET /tags/{id}/related-tags/tags`: the related tags themselves.
    pub async fn related_tag_tags(
        &self,
        id: u64,
        params: &GammaRelatedTagsParams,
    ) -> ApiResult<Vec<PolymarketTag>> {
        self.get(&format!("/tags/{id}/related-tags/tags"), &params.query().0)
            .await
    }

    /// `GET /tags/slug/{slug}/related-tags/tags`.
    pub async fn related_tag_tags_by_slug(
        &self,
        slug: &str,
        params: &GammaRelatedTagsParams,
    ) -> ApiResult<Vec<PolymarketTag>> {
        self.get(
            &format!("/tags/slug/{slug}/related-tags/tags"),
            &params.query().0,
        )
        .await
    }

    /// `GET /public-search`: events (with markets) and, with `search_tags`, tags matching `q`.
    pub async fn search(&self, q: &str, params: &GammaSearchParams) -> ApiResult<GammaSearch> {
        self.get("/public-search", &params.query(q).0).await
    }

    /// `GET /sports`: league metadata.
    pub async fn sports(&self) -> ApiResult<Vec<GammaSport>> {
        self.get("/sports", &[]).await
    }

    /// `GET /sports/market-types`: valid `sports_market_types` values.
    pub async fn sports_market_types(&self) -> ApiResult<GammaSportsMarketTypes> {
        self.get("/sports/market-types", &[]).await
    }

    /// `GET /teams`.
    pub async fn teams(&self, params: &GammaTeamsParams) -> ApiResult<Vec<GammaTeam>> {
        self.get("/teams", &params.query().0).await
    }

    /// `GET /teams/{id}`.
    pub async fn team(&self, id: u64) -> ApiResult<GammaTeam> {
        self.get(&format!("/teams/{id}"), &[]).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Hits the live API.
    async fn test_list_events_pages() {
        let api = GammaApi::new(10);
        let mut params = ListEventsParams::new(100);
        params.closed = Some(false);
        let first = api.list_events(&params).await.unwrap();
        assert_eq!(first.events.len(), 100);
        params.after_cursor = Some(first.next_cursor.expect("full page has a cursor"));
        let second = api.list_events(&params).await.unwrap();
        assert!(!second.events.is_empty());
        assert_ne!(first.events[0].id, second.events[0].id);
    }

    #[tokio::test]
    #[ignore] // Hits the live API.
    async fn test_list_events_by_id() {
        let api = GammaApi::new(10);
        let params = ListEventsParams {
            id: Some(vec![2890, 3933]),
            ..Default::default()
        };
        let page = api.list_events(&params).await.unwrap();
        let ids: Vec<_> = page.events.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(ids, ["2890", "3933"]);
    }

    #[tokio::test]
    #[ignore] // Hits the live API.
    async fn test_markets_keyset_and_lookups() {
        let api = GammaApi::new(10);
        assert_eq!(api.status().await.unwrap(), "OK");
        let page = api
            .list_markets(&GammaListMarketsParams {
                limit: Some(5),
                ..Default::default()
            })
            .await
            .unwrap();
        let first = &page.markets[0];
        let id: u64 = first.id.parse().unwrap();
        let by_id = api.market(id, Some(true)).await.unwrap();
        assert_eq!(by_id.id, first.id);
        api.sports().await.unwrap();
        api.tags(&GammaTagsParams::default()).await.unwrap();
    }
}
