use crate::rest::{get, post, ApiResult, Query};
use crate::types::{
    ClobBatchPricesHistory, ClobBatchPricesHistoryRequest, ClobBook, ClobBookRequest, ClobFeeRate,
    ClobLastTradePrice, ClobLastTradePriceItem, ClobLiveActivityMarket, ClobMarket,
    ClobMarketByToken, ClobMarketInfo, ClobMarketsPage, ClobMidpoint, ClobNegRisk, ClobPrice,
    ClobPricesHistory, ClobPricesHistoryParams, ClobSide, ClobSimplifiedMarket, ClobSpread,
    ClobTickSize, ClobTokenPrices, ClobTokenValues,
};
use roshar_http::RateLimitedClient;

/// Polymarket CLOB API client, public market-data reads only (no orders, auth or rewards).
///
/// Errors come back as [`crate::PolymarketApiError::Status`] with a [`crate::types::ClobError`]
/// body, e.g. HTTP 404 `No orderbook exists for the requested token id` for a token without a
/// book, or 404 `market not found`.
///
/// Batch routes: the documented `GET /books`, `GET /prices`, `GET /midpoints` and
/// `GET /last-trades-prices` query forms answered HTTP 400 `Invalid payload` for every query shape
/// tried live (comma list, repeated keys, JSON array), and there is no `GET /spreads` (405).
/// Only the `POST` (JSON body) forms are exposed.
///
/// Rate limits (per IP, https://docs.polymarket.com/api-reference/rate-limits): 9,000 req / 10 s
/// for the CLOB in general; `/book`, `/price`, `/midpoint` 1,500 / 10 s; `/books`, `/prices`,
/// `/midpoints` 500 / 10 s; `/prices-history` 1,000 / 10 s; tick size 200 / 10 s.
pub struct ClobApi {
    client: RateLimitedClient,
}

impl ClobApi {
    const BASE_URL: &'static str = "https://clob.polymarket.com";

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

    async fn post<T: serde::de::DeserializeOwned, B: serde::Serialize + ?Sized>(
        &self,
        path: &str,
        body: &B,
    ) -> ApiResult<T> {
        post(&self.client, Self::BASE_URL, path, body).await
    }

    fn cursor(next_cursor: Option<&str>) -> Vec<(&'static str, String)> {
        let mut q = Query::default();
        q.opt("next_cursor", &next_cursor);
        q.0
    }

    /// `GET /time`: server time, epoch seconds.
    pub async fn server_time(&self) -> ApiResult<u64> {
        self.get("/time", &[]).await
    }

    /// `GET /markets`: one page (1,000 rows) of all CLOB markets. Not in the current OpenAPI spec
    /// but live. Pass the previous `next_cursor` back; [`crate::types::CLOB_END_CURSOR`] marks
    /// the end.
    pub async fn markets(
        &self,
        next_cursor: Option<&str>,
    ) -> ApiResult<ClobMarketsPage<ClobMarket>> {
        self.get("/markets", &Self::cursor(next_cursor)).await
    }

    /// `GET /markets/{condition_id}`: one market. Not in the current OpenAPI spec but live.
    pub async fn market(&self, condition_id: &str) -> ApiResult<ClobMarket> {
        self.get(&format!("/markets/{condition_id}"), &[]).await
    }

    /// `GET /simplified-markets`: one page of all markets, reduced fields.
    pub async fn simplified_markets(
        &self,
        next_cursor: Option<&str>,
    ) -> ApiResult<ClobMarketsPage<ClobSimplifiedMarket>> {
        self.get("/simplified-markets", &Self::cursor(next_cursor))
            .await
    }

    /// `GET /sampling-markets`: one page of markets in the liquidity-rewards sample.
    pub async fn sampling_markets(
        &self,
        next_cursor: Option<&str>,
    ) -> ApiResult<ClobMarketsPage<ClobMarket>> {
        self.get("/sampling-markets", &Self::cursor(next_cursor))
            .await
    }

    /// `GET /sampling-simplified-markets`.
    pub async fn sampling_simplified_markets(
        &self,
        next_cursor: Option<&str>,
    ) -> ApiResult<ClobMarketsPage<ClobSimplifiedMarket>> {
        self.get("/sampling-simplified-markets", &Self::cursor(next_cursor))
            .await
    }

    /// `GET /clob-markets/{condition_id}`: compact CLOB parameters (tokens, tick, fees, rewards).
    pub async fn clob_market_info(&self, condition_id: &str) -> ApiResult<ClobMarketInfo> {
        self.get(&format!("/clob-markets/{condition_id}"), &[])
            .await
    }

    /// `GET /markets-by-token/{token_id}`: the condition and both token ids of a token's market.
    pub async fn market_by_token(&self, token_id: &str) -> ApiResult<ClobMarketByToken> {
        self.get(&format!("/markets-by-token/{token_id}"), &[])
            .await
    }

    /// `GET /markets/live-activity/{condition_id}`: minimal market card.
    pub async fn live_activity_market(
        &self,
        condition_id: &str,
    ) -> ApiResult<ClobLiveActivityMarket> {
        self.get(&format!("/markets/live-activity/{condition_id}"), &[])
            .await
    }

    /// `POST /markets/live-activity`: minimal market cards for several condition ids.
    pub async fn live_activity_markets(
        &self,
        condition_ids: &[&str],
    ) -> ApiResult<Vec<ClobLiveActivityMarket>> {
        self.post("/markets/live-activity", condition_ids).await
    }

    /// `GET /book`: order book summary of one token.
    pub async fn book(&self, token_id: &str) -> ApiResult<ClobBook> {
        self.get("/book", &[("token_id", token_id.to_string())])
            .await
    }

    /// `POST /books`: books of several tokens. Tokens without a book are left out.
    pub async fn books(&self, tokens: &[ClobBookRequest]) -> ApiResult<Vec<ClobBook>> {
        self.post("/books", tokens).await
    }

    /// `GET /price`: best price on `side` (`BUY` = best bid, `SELL` = best ask).
    pub async fn price(&self, token_id: &str, side: ClobSide) -> ApiResult<ClobPrice> {
        self.get(
            "/price",
            &[
                ("token_id", token_id.to_string()),
                ("side", side.as_str().to_string()),
            ],
        )
        .await
    }

    /// `POST /prices`: best prices; every item needs a `side`.
    pub async fn prices(&self, tokens: &[ClobBookRequest]) -> ApiResult<ClobTokenPrices> {
        self.post("/prices", tokens).await
    }

    /// `GET /midpoint`.
    pub async fn midpoint(&self, token_id: &str) -> ApiResult<ClobMidpoint> {
        self.get("/midpoint", &[("token_id", token_id.to_string())])
            .await
    }

    /// `POST /midpoints`.
    pub async fn midpoints(&self, tokens: &[ClobBookRequest]) -> ApiResult<ClobTokenValues> {
        self.post("/midpoints", tokens).await
    }

    /// `GET /spread`: best ask minus best bid.
    pub async fn spread(&self, token_id: &str) -> ApiResult<ClobSpread> {
        self.get("/spread", &[("token_id", token_id.to_string())])
            .await
    }

    /// `POST /spreads`.
    pub async fn spreads(&self, tokens: &[ClobBookRequest]) -> ApiResult<ClobTokenValues> {
        self.post("/spreads", tokens).await
    }

    /// `GET /last-trade-price`.
    pub async fn last_trade_price(&self, token_id: &str) -> ApiResult<ClobLastTradePrice> {
        self.get("/last-trade-price", &[("token_id", token_id.to_string())])
            .await
    }

    /// `POST /last-trades-prices`: at most 500 tokens.
    pub async fn last_trades_prices(
        &self,
        tokens: &[ClobBookRequest],
    ) -> ApiResult<Vec<ClobLastTradePriceItem>> {
        self.post("/last-trades-prices", tokens).await
    }

    /// `GET /tick-size/{token_id}` (the `?token_id=` form returns the same body).
    pub async fn tick_size(&self, token_id: &str) -> ApiResult<ClobTickSize> {
        self.get(&format!("/tick-size/{token_id}"), &[]).await
    }

    /// `GET /neg-risk/{token_id}` (the `?token_id=` form returns the same body).
    pub async fn neg_risk(&self, token_id: &str) -> ApiResult<ClobNegRisk> {
        self.get(&format!("/neg-risk/{token_id}"), &[]).await
    }

    /// `GET /fee-rate/{token_id}` (the `?token_id=` form returns the same body).
    pub async fn fee_rate(&self, token_id: &str) -> ApiResult<ClobFeeRate> {
        self.get(&format!("/fee-rate/{token_id}"), &[]).await
    }

    /// `GET /prices-history`: price series of one token (`market` = token id).
    pub async fn prices_history(
        &self,
        token_id: &str,
        params: &ClobPricesHistoryParams,
    ) -> ApiResult<ClobPricesHistory> {
        let mut q = Query::default();
        q.req("market", &token_id)
            .opt("startTs", &params.start_ts)
            .opt("endTs", &params.end_ts)
            .opt("interval", &params.interval)
            .opt("fidelity", &params.fidelity);
        self.get("/prices-history", &q.0).await
    }

    /// `POST /batch-prices-history`: price series of up to 20 tokens.
    pub async fn batch_prices_history(
        &self,
        request: &ClobBatchPricesHistoryRequest,
    ) -> ApiResult<ClobBatchPricesHistory> {
        self.post("/batch-prices-history", request).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::CLOB_END_CURSOR;

    #[tokio::test]
    #[ignore] // Hits the live API.
    async fn test_markets_and_book() {
        let api = ClobApi::new(5);
        assert!(api.server_time().await.unwrap() > 1_700_000_000);
        let page = api.sampling_markets(None).await.unwrap();
        assert_ne!(page.next_cursor, CLOB_END_CURSOR);
        let token = page
            .data
            .iter()
            .find(|m| m.accepting_orders)
            .and_then(|m| m.tokens.first())
            .unwrap()
            .token_id
            .clone();
        let book = api.book(&token).await.unwrap();
        assert_eq!(book.asset_id, token);
        let books = api.books(&[ClobBookRequest::new(&token)]).await.unwrap();
        assert_eq!(books.len(), 1);
    }
}
