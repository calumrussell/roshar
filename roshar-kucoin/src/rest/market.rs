use crate::types::{
    KuCoinContract, KuCoinCurrentFundingRate, KuCoinFundingRate, KuCoinIndexPage, KuCoinIndexQuery,
    KuCoinIndexValue, KuCoinKline, KuCoinMarkPrice, KuCoinOpenInterest, KuCoinOpenInterestPoint,
    KuCoinOrderBook, KuCoinResponse, KuCoinServiceStatus, KuCoinSpotIndex, KuCoinTicker,
    KuCoinTrade, KuCoinWsTokenData,
};
use roshar_http::RateLimitedClient;
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::sync::Arc;

type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// KuCoin success code; every other `code` is an error.
const SUCCESS_CODE: &str = "200000";

/// Decode a KuCoin REST envelope.
///
/// KuCoin returns error envelopes (`{"code":"404000","msg":"..."}`, no `data`) with both
/// HTTP 200 and non-2xx statuses, so the envelope `code` is checked before `data` is used.
/// Returns `data` as sent: `None` when the exchange sent it as `null` or omitted it.
fn decode<T: DeserializeOwned>(
    status_ok: bool,
    status: impl std::fmt::Display,
    text: &str,
    what: &str,
) -> Result<Option<T>, BoxError> {
    let parsed: KuCoinResponse<T> = match serde_json::from_str(text) {
        Ok(parsed) => parsed,
        Err(e) if status_ok => {
            return Err(format!("Failed to parse KuCoin {what} response: {e}").into())
        }
        Err(_) => {
            return Err(format!("KuCoin {what} request failed with status {status}: {text}").into())
        }
    };

    if parsed.code != SUCCESS_CODE {
        return Err(format!(
            "KuCoin {what} error {} (HTTP {status}): {}",
            parsed.code,
            parsed.msg.as_deref().unwrap_or("")
        )
        .into());
    }
    if !status_ok {
        return Err(format!("KuCoin {what} request failed with status {status}: {text}").into());
    }

    Ok(parsed.data)
}

/// KuCoin Futures public market data REST client.
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

    /// Fetch a public WebSocket token and return the full connection URL
    /// (`<instanceServers[0].endpoint>?token=<token>`).
    ///
    /// Endpoint: POST /api/v1/bullet-public (Get Public Token - Classic Futures)
    /// Rate limit weight: 10
    ///
    /// The token is valid for 24 hours. Set the result on the client's
    /// `ws_config_mut().url` before `Manager::new_conn`; a reconnect after the token
    /// expires needs a fresh URL.
    pub async fn get_ws_url(&self) -> Result<String, BoxError> {
        self.get_ws_token().await?.url()
    }

    /// Get all contracts returned by the active-contracts endpoint, whatever their `status`.
    /// Returns a map of symbol -> contract info. Filter on `status == "Open"` to keep only
    /// tradable contracts.
    ///
    /// Endpoint: GET /api/v1/contracts/active (Get All Symbols)
    /// Rate limit weight: 3
    pub async fn get_contracts(&self) -> Result<HashMap<String, KuCoinContract>, BoxError> {
        let url = format!("{}/api/v1/contracts/active", super::KUCOIN_FUTURES_REST_URL);
        let response = self.client.get(&url).await?;
        let status = response.status();
        let text = response.text().await?;
        let contracts: Vec<KuCoinContract> =
            decode(status.is_success(), status, &text, "contracts")?
                .ok_or("KuCoin contracts response has null data")?;

        Ok(contracts
            .into_iter()
            .map(|c| (c.symbol.clone(), c))
            .collect())
    }

    /// Get historical funding rates for a symbol within `[start_time, end_time]`
    /// (milliseconds, both inclusive).
    ///
    /// Endpoint: GET /api/v1/contract/funding-rates (Get Public Funding History)
    /// Rate limit weight: 5 per request
    ///
    /// The exchange returns a capped page of the newest records in the window, newest
    /// first, and `data: null` when the window holds none. This method pages backwards
    /// by moving `to` below the oldest record received until the window is exhausted,
    /// and returns the records in ascending `timepoint` order.
    pub async fn get_historical_funding_rates(
        &self,
        symbol: &str,
        start_time: i64,
        end_time: i64,
    ) -> Result<Vec<KuCoinFundingRate>, BoxError> {
        let mut all_rates: Vec<KuCoinFundingRate> = Vec::new();
        let mut current_to = end_time;

        while current_to >= start_time {
            let url = format!(
                "{}/api/v1/contract/funding-rates?symbol={}&from={}&to={}",
                super::KUCOIN_FUTURES_REST_URL,
                symbol,
                start_time,
                current_to,
            );

            let response = self.client.get(&url).await?;
            let status = response.status();
            let text = response.text().await?;
            let batch: Vec<KuCoinFundingRate> =
                match decode(status.is_success(), status, &text, "funding rates")? {
                    Some(batch) => batch,
                    None => break,
                };

            let Some(oldest_timepoint) = batch.iter().map(|r| r.timepoint).min() else {
                break;
            };
            all_rates.extend(batch);

            // Continue strictly below the oldest record received.
            current_to = oldest_timepoint - 1;
        }

        all_rates.sort_by_key(|r| r.timepoint);
        Ok(all_rates)
    }

    /// GET `url` and require non-null `data`.
    async fn get_data<T: DeserializeOwned>(&self, url: &str, what: &str) -> Result<T, BoxError> {
        let response = self.client.get(url).await?;
        let status = response.status();
        let text = response.text().await?;
        decode(status.is_success(), status, &text, what)?
            .ok_or_else(|| format!("KuCoin {what} response has null data").into())
    }

    fn url(path: &str) -> String {
        format!("{}{}", super::KUCOIN_FUTURES_REST_URL, path)
    }

    /// Endpoint: GET /api/v1/contracts/{symbol} (Get Symbol). Weight 3.
    pub async fn get_contract(&self, symbol: &str) -> Result<KuCoinContract, BoxError> {
        self.get_data(
            &Self::url(&format!("/api/v1/contracts/{symbol}")),
            "contract",
        )
        .await
    }

    /// Endpoint: GET /api/v1/ticker?symbol (Get Ticker). Weight 2.
    pub async fn get_ticker(&self, symbol: &str) -> Result<KuCoinTicker, BoxError> {
        self.get_data(
            &Self::url(&format!("/api/v1/ticker?symbol={symbol}")),
            "ticker",
        )
        .await
    }

    /// Endpoint: GET /api/v1/allTickers (Get All Tickers). Weight 5.
    pub async fn get_all_tickers(&self) -> Result<Vec<KuCoinTicker>, BoxError> {
        self.get_data(&Self::url("/api/v1/allTickers"), "all tickers")
            .await
    }

    /// Endpoint: GET /api/v1/level2/snapshot?symbol (Get Full OrderBook). Weight 3.
    /// Seeds `/contractMarket/level2` increments via `sequence`.
    pub async fn get_full_order_book(&self, symbol: &str) -> Result<KuCoinOrderBook, BoxError> {
        self.get_data(
            &Self::url(&format!("/api/v1/level2/snapshot?symbol={symbol}")),
            "full order book",
        )
        .await
    }

    /// Endpoint: GET /api/v1/level2/depth{size}?symbol (Get Part OrderBook). Weight 5.
    /// Documented sizes: 20, 100 (others answer `404`).
    pub async fn get_part_order_book(
        &self,
        symbol: &str,
        size: u32,
    ) -> Result<KuCoinOrderBook, BoxError> {
        self.get_data(
            &Self::url(&format!("/api/v1/level2/depth{size}?symbol={symbol}")),
            "part order book",
        )
        .await
    }

    /// Endpoint: GET /api/v1/trade/history?symbol (Get Trade History, latest 100). Weight 5.
    pub async fn get_trade_history(&self, symbol: &str) -> Result<Vec<KuCoinTrade>, BoxError> {
        self.get_data(
            &Self::url(&format!("/api/v1/trade/history?symbol={symbol}")),
            "trade history",
        )
        .await
    }

    /// Endpoint: GET /api/v1/kline/query (Get Klines). Weight 3.
    ///
    /// `symbol` may be a contract, its `indexSymbol` (index klines) or a premium symbol
    /// (`premiumsSymbol1M`/`premiumsSymbol8H`). `granularity` is minutes: 1, 5, 15, 30, 60,
    /// 120, 240, 480, 720, 1440, 10080. `from`/`to` are milliseconds.
    pub async fn get_klines(
        &self,
        symbol: &str,
        granularity: u32,
        from: Option<i64>,
        to: Option<i64>,
    ) -> Result<Vec<KuCoinKline>, BoxError> {
        let mut path = format!("/api/v1/kline/query?symbol={symbol}&granularity={granularity}");
        if let Some(from) = from {
            path.push_str(&format!("&from={from}"));
        }
        if let Some(to) = to {
            path.push_str(&format!("&to={to}"));
        }
        self.get_data(&Self::url(&path), "klines").await
    }

    /// Endpoint: GET /api/v1/mark-price/{symbol}/current (Get Mark Price). Weight 3.
    pub async fn get_mark_price(&self, symbol: &str) -> Result<KuCoinMarkPrice, BoxError> {
        self.get_data(
            &Self::url(&format!("/api/v1/mark-price/{symbol}/current")),
            "mark price",
        )
        .await
    }

    /// Endpoint: GET /api/v1/index/query (Get Spot Index Price). Weight 2.
    /// `symbol` is the contract's `indexSymbol`, e.g. `.KXBTUSDT`.
    pub async fn get_spot_index_price(
        &self,
        symbol: &str,
        query: &KuCoinIndexQuery,
    ) -> Result<KuCoinIndexPage<KuCoinSpotIndex>, BoxError> {
        let path = format!("/api/v1/index/query?symbol={symbol}{}", query.to_query());
        self.get_data(&Self::url(&path), "spot index").await
    }

    /// Endpoint: GET /api/v1/interest/query (Get Interest Rate Index). Weight 5.
    /// `symbol` is the contract's `fundingBaseSymbol`/`fundingQuoteSymbol`, e.g. `.XBTINT8H`.
    pub async fn get_interest_rate_index(
        &self,
        symbol: &str,
        query: &KuCoinIndexQuery,
    ) -> Result<KuCoinIndexPage<KuCoinIndexValue>, BoxError> {
        let path = format!("/api/v1/interest/query?symbol={symbol}{}", query.to_query());
        self.get_data(&Self::url(&path), "interest rate index")
            .await
    }

    /// Endpoint: GET /api/v1/premium/query (Get Premium Index). Weight 3.
    /// `symbol` is a premium symbol, e.g. `.XBTUSDTMPI8H`.
    pub async fn get_premium_index(
        &self,
        symbol: &str,
        query: &KuCoinIndexQuery,
    ) -> Result<KuCoinIndexPage<KuCoinIndexValue>, BoxError> {
        let path = format!("/api/v1/premium/query?symbol={symbol}{}", query.to_query());
        self.get_data(&Self::url(&path), "premium index").await
    }

    /// Endpoint: GET /api/v1/funding-rate/{symbol}/current (Get Current Funding Rate).
    /// Weight 2. Delivery contracts answer error 415000.
    pub async fn get_current_funding_rate(
        &self,
        symbol: &str,
    ) -> Result<KuCoinCurrentFundingRate, BoxError> {
        self.get_data(
            &Self::url(&format!("/api/v1/funding-rate/{symbol}/current")),
            "current funding rate",
        )
        .await
    }

    /// Endpoint: GET /api/v1/timestamp (Get Server Time). Weight 2. Milliseconds.
    pub async fn get_server_time(&self) -> Result<i64, BoxError> {
        self.get_data(&Self::url("/api/v1/timestamp"), "server time")
            .await
    }

    /// Endpoint: GET /api/v1/status (Get Service Status). Weight 4.
    pub async fn get_service_status(&self) -> Result<KuCoinServiceStatus, BoxError> {
        self.get_data(&Self::url("/api/v1/status"), "service status")
            .await
    }

    /// Real-time open interest. Endpoint: GET https://api.kucoin.com/api/ua/v2/market/open-interest
    /// (UTA REST V2, Get Futures Open Interest). `None` returns every symbol; the exchange
    /// accepts up to 10 symbols. Weight 10.
    pub async fn get_open_interest(
        &self,
        symbols: Option<&[&str]>,
    ) -> Result<Vec<KuCoinOpenInterest>, BoxError> {
        let mut url = format!(
            "{}/api/ua/v2/market/open-interest",
            super::KUCOIN_UTA_REST_URL
        );
        if let Some(symbols) = symbols {
            url.push_str(&format!("?symbol={}", symbols.join(",")));
        }
        self.get_data(&url, "open interest").await
    }

    /// Open-interest history, newest first. Same endpoint in interval mode.
    /// `interval`: `5min, 15min, 30min, 1hour, 4hour, 1day`. `end_at` doubles as the paging
    /// cursor. `start_at`/`end_at` are milliseconds; `page_size` defaults to 30 (max 200).
    /// Weight 10.
    pub async fn get_open_interest_history(
        &self,
        symbol: &str,
        interval: &str,
        start_at: Option<i64>,
        end_at: Option<i64>,
        page_size: Option<u32>,
    ) -> Result<Vec<KuCoinOpenInterestPoint>, BoxError> {
        let mut url = format!(
            "{}/api/ua/v2/market/open-interest?symbol={symbol}&interval={interval}",
            super::KUCOIN_UTA_REST_URL
        );
        if let Some(v) = start_at {
            url.push_str(&format!("&startAt={v}"));
        }
        if let Some(v) = end_at {
            url.push_str(&format!("&endAt={v}"));
        }
        if let Some(v) = page_size {
            url.push_str(&format!("&pageSize={v}"));
        }
        self.get_data(&url, "open interest history").await
    }

    /// POST /api/v1/bullet-public, returning the token and every instance server
    /// (endpoint, `pingInterval`, `pingTimeout`).
    pub async fn get_ws_token(&self) -> Result<KuCoinWsTokenData, BoxError> {
        let url = format!("{}/api/v1/bullet-public", super::KUCOIN_FUTURES_REST_URL);
        let response = self.client.post(&url).await.send().await?;
        let status = response.status();
        let text = response.text().await?;
        decode(status.is_success(), status, &text, "bullet-public")?
            .ok_or_else(|| "KuCoin bullet-public response has null data".into())
    }
}
