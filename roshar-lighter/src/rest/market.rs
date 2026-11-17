use crate::types::{
    LighterAsset, LighterAssetDetails, LighterCandles, LighterEvents, LighterExchangeStats,
    LighterFeeTiers, LighterFundingRate, LighterFundingRatesResponse, LighterFundings,
    LighterMarkPriceCandles, LighterMarket, LighterMarketPriceCharts, LighterMarketsResponse,
    LighterOrderBook, LighterOrderBookDetails, LighterPriceOracleInfo, LighterStatus, LighterTrade,
    LighterTradesResponse, LighterUpDownCandles,
};
use roshar_http::RateLimitedClient;
use serde::de::DeserializeOwned;
use serde::Deserialize;

const HOST: &str = "https://mainnet.zklighter.elliot.ai";
const BASE_URL: &str = "https://mainnet.zklighter.elliot.ai/api/v1";

type BoxError = Box<dyn std::error::Error + Send + Sync>;
type Query = Vec<(&'static str, String)>;

/// Query parameters of `GET /api/v1/candles` and `GET /api/v1/markPriceCandles` that both
/// share. All are required by the API.
#[derive(Debug, Clone)]
pub struct LighterCandleRange<'a> {
    pub market_id: u32,
    /// `1m`, `5m`, `15m`, `30m`, `1h`, `4h`, `12h` or `1d`.
    pub resolution: &'a str,
    /// Unix ms (seconds are accepted too). Inclusive.
    pub start_timestamp: i64,
    /// Unix ms (seconds are accepted too).
    pub end_timestamp: i64,
    /// Window size hint used when `start_timestamp` is 0.
    pub count_back: i64,
}

impl LighterCandleRange<'_> {
    fn query(&self) -> Query {
        vec![
            ("market_id", self.market_id.to_string()),
            ("resolution", self.resolution.to_string()),
            ("start_timestamp", self.start_timestamp.to_string()),
            ("end_timestamp", self.end_timestamp.to_string()),
            ("count_back", self.count_back.to_string()),
        ]
    }
}

/// Public filters of `GET /api/v1/trades` (account filters are not exposed). `sort_by` and
/// `limit` (1..=100) are required; the rest are sent only when `Some`.
#[derive(Debug, Clone, Default)]
pub struct LighterTradesQuery<'a> {
    /// `block_height`, `timestamp` or `trade_id`. Results are always descending.
    pub sort_by: &'a str,
    pub limit: u32,
    pub market_id: Option<u32>,
    /// `all`, `spot`, `perp`, `bo`, or several comma-separated.
    pub market_type: Option<&'a str>,
    /// `next_cursor` of the previous page.
    pub cursor: Option<&'a str>,
    /// Exclusive start value in the `sort_by` field (Unix ms for `timestamp`).
    pub from: Option<i64>,
    /// `-1` both sides, `0` buys, `1` sells.
    pub ask_filter: Option<i32>,
    /// `all`, `trade`, `liquidation`, `deleverage` or `market-settlement`.
    pub trade_type: Option<&'a str>,
}

/// Filters of `GET /api/v1/events`, each sent only when `Some`. `event_ids` and `market_ids` are
/// mutually exclusive (at most 50 each).
#[derive(Debug, Clone, Default)]
pub struct LighterEventsQuery<'a> {
    pub event_ids: Option<&'a [i64]>,
    pub market_ids: Option<&'a [u32]>,
    pub creator_account_index: Option<i64>,
    pub cursor: Option<i64>,
    pub active: Option<bool>,
    /// 1..=100.
    pub limit: Option<u32>,
    pub tag: Option<&'a str>,
    /// `one_off` or `recurrent`.
    pub event_kind: Option<&'a str>,
    /// Inclusive `start_at` lower bound, Unix ms.
    pub start_from: Option<i64>,
    /// Exclusive `start_at` upper bound, Unix ms.
    pub start_before: Option<i64>,
    /// `id` or `start_at`.
    pub sort: Option<&'a str>,
    /// `asc` or `desc`.
    pub order: Option<&'a str>,
}

/// Lighter wraps most replies in `{code, message?, …}`: a `code` other than 200 is an error.
#[derive(Deserialize)]
struct Code {
    code: Option<i64>,
    message: Option<String>,
}

fn push<T: ToString>(q: &mut Query, key: &'static str, v: Option<T>) {
    if let Some(v) = v {
        q.push((key, v.to_string()));
    }
}

fn join<T: ToString>(xs: &[T]) -> String {
    xs.iter().map(T::to_string).collect::<Vec<_>>().join(",")
}

pub struct MarketApi {
    client: RateLimitedClient,
}

impl MarketApi {
    /// `refill` requests every `interval_secs` seconds (see `roshar_http::RateLimitedClient`).
    pub fn new(refill: u32, interval_secs: u64) -> Self {
        Self {
            client: RateLimitedClient::new(refill, interval_secs),
        }
    }

    async fn get<T: DeserializeOwned>(
        &self,
        name: &str,
        url: &str,
        query: &[(&'static str, String)],
    ) -> Result<T, BoxError> {
        self.client.acquire().await;
        let response = self.client.inner().get(url).query(query).send().await?;
        let status = response.status();
        let text = response.text().await?;
        if !status.is_success() {
            return Err(format!("Lighter {name} failed ({status}): {text}").into());
        }
        if let Ok(Code {
            code: Some(code),
            message,
        }) = serde_json::from_str::<Code>(&text)
        {
            if code != 200 {
                let message = message.unwrap_or_default();
                return Err(format!("Lighter {name} error {code}: {message}").into());
            }
        }
        serde_json::from_str(&text)
            .map_err(|e| format!("Failed to parse Lighter {name}: {e}: {text}").into())
    }

    /// Exchange status and server time. `GET /`
    pub async fn get_status(&self) -> Result<LighterStatus, BoxError> {
        self.get("status", &format!("{HOST}/"), &[]).await
    }

    /// List all markets (perp, spot and binary options) with fee and size metadata.
    /// `GET /api/v1/orderBooks`
    pub async fn get_markets(&self) -> Result<Vec<LighterMarket>, BoxError> {
        let url = format!("{BASE_URL}/orderBooks");
        let r: LighterMarketsResponse = self.get("orderBooks", &url, &[]).await?;
        Ok(r.order_books)
    }

    /// Perp and spot market details with daily stats and prices.
    /// `GET /api/v1/orderBookDetails?market_id=&filter=`
    pub async fn get_market_details(
        &self,
        market_id: Option<u32>,
        filter: Option<&str>,
    ) -> Result<LighterOrderBookDetails, BoxError> {
        let mut q = Query::new();
        push(&mut q, "market_id", market_id);
        push(&mut q, "filter", filter);
        let url = format!("{BASE_URL}/orderBookDetails");
        self.get("orderBookDetails", &url, &q).await
    }

    /// Resting orders of a market, `limit` (1..=250, required by the API) per side.
    /// `GET /api/v1/orderBookOrders?market_id=&limit=`
    pub async fn get_orderbook(
        &self,
        market_id: u32,
        limit: u32,
    ) -> Result<LighterOrderBook, BoxError> {
        let q = vec![
            ("market_id", market_id.to_string()),
            ("limit", limit.to_string()),
        ];
        let url = format!("{BASE_URL}/orderBookOrders");
        self.get("orderBookOrders", &url, &q).await
    }

    /// Most recent `limit` (1..=100, required by the API) trades of a market.
    /// `GET /api/v1/recentTrades?market_id=&limit=`
    pub async fn get_recent_trades(
        &self,
        market_id: u32,
        limit: u32,
    ) -> Result<Vec<LighterTrade>, BoxError> {
        let q = vec![
            ("market_id", market_id.to_string()),
            ("limit", limit.to_string()),
        ];
        let url = format!("{BASE_URL}/recentTrades");
        let r: LighterTradesResponse = self.get("recentTrades", &url, &q).await?;
        Ok(r.trades)
    }

    /// Paged trade history. `GET /api/v1/trades`
    pub async fn get_trades(
        &self,
        query: &LighterTradesQuery<'_>,
    ) -> Result<LighterTradesResponse, BoxError> {
        let mut q = vec![
            ("sort_by", query.sort_by.to_string()),
            ("limit", query.limit.to_string()),
        ];
        push(&mut q, "market_id", query.market_id);
        push(&mut q, "market_type", query.market_type);
        push(&mut q, "cursor", query.cursor);
        push(&mut q, "from", query.from);
        push(&mut q, "ask_filter", query.ask_filter);
        push(&mut q, "type", query.trade_type);
        let url = format!("{BASE_URL}/trades");
        self.get("trades", &url, &q).await
    }

    /// Trade candles, at most 500 per call. `GET /api/v1/candles`
    pub async fn get_candles(
        &self,
        range: &LighterCandleRange<'_>,
        set_timestamp_to_end: Option<bool>,
        merge_mark_price: Option<bool>,
    ) -> Result<LighterCandles, BoxError> {
        let mut q = range.query();
        push(&mut q, "set_timestamp_to_end", set_timestamp_to_end);
        push(&mut q, "merge_mark_price", merge_mark_price);
        let url = format!("{BASE_URL}/candles");
        self.get("candles", &url, &q).await
    }

    /// Mark price candles. `GET /api/v1/markPriceCandles`
    pub async fn get_mark_price_candles(
        &self,
        range: &LighterCandleRange<'_>,
        connect_opens: Option<bool>,
    ) -> Result<LighterMarkPriceCandles, BoxError> {
        let mut q = range.query();
        push(&mut q, "connect_opens", connect_opens);
        let url = format!("{BASE_URL}/markPriceCandles");
        self.get("markPriceCandles", &url, &q).await
    }

    /// Last 24 hourly prices per market (all markets when `None`).
    /// `GET /api/v1/marketPriceCharts?market_ids=`
    pub async fn get_market_price_charts(
        &self,
        market_ids: Option<&[u32]>,
    ) -> Result<LighterMarketPriceCharts, BoxError> {
        let mut q = Query::new();
        push(&mut q, "market_ids", market_ids.map(join));
        let url = format!("{BASE_URL}/marketPriceCharts");
        self.get("marketPriceCharts", &url, &q).await
    }

    /// Funding payment history, at most 750 entries. `resolution`: `1h` or `1d`.
    /// `GET /api/v1/fundings`
    pub async fn get_fundings(
        &self,
        market_id: u32,
        resolution: &str,
        start_timestamp: i64,
        end_timestamp: i64,
        count_back: i64,
    ) -> Result<LighterFundings, BoxError> {
        let q = vec![
            ("market_id", market_id.to_string()),
            ("resolution", resolution.to_string()),
            ("start_timestamp", start_timestamp.to_string()),
            ("end_timestamp", end_timestamp.to_string()),
            ("count_back", count_back.to_string()),
        ];
        let url = format!("{BASE_URL}/fundings");
        self.get("fundings", &url, &q).await
    }

    /// Current funding rates of every market on Lighter, Binance, Bybit and Hyperliquid.
    /// `GET /api/v1/funding-rates`
    pub async fn get_funding_rates(&self) -> Result<Vec<LighterFundingRate>, BoxError> {
        let url = format!("{BASE_URL}/funding-rates");
        let r: LighterFundingRatesResponse = self.get("funding-rates", &url, &[]).await?;
        Ok(r.funding_rates)
    }

    /// 24h exchange totals and per-market stats. `GET /api/v1/exchangeStats`
    pub async fn get_exchange_stats(&self) -> Result<LighterExchangeStats, BoxError> {
        let url = format!("{BASE_URL}/exchangeStats");
        self.get("exchangeStats", &url, &[]).await
    }

    /// One asset, or all when `None`. `GET /api/v1/assetDetails?asset_id=`
    pub async fn get_asset_details(
        &self,
        asset_id: Option<u32>,
    ) -> Result<Vec<LighterAsset>, BoxError> {
        let mut q = Query::new();
        push(&mut q, "asset_id", asset_id);
        let url = format!("{BASE_URL}/assetDetails");
        let r: LighterAssetDetails = self.get("assetDetails", &url, &q).await?;
        Ok(r.asset_details)
    }

    /// Index and mark price sources, for one symbol or all markets.
    /// `GET /api/v1/priceOracleInfo?symbol=`
    pub async fn get_price_oracle_info(
        &self,
        symbol: Option<&str>,
    ) -> Result<LighterPriceOracleInfo, BoxError> {
        let mut q = Query::new();
        push(&mut q, "symbol", symbol);
        let url = format!("{BASE_URL}/priceOracleInfo");
        self.get("priceOracleInfo", &url, &q).await
    }

    /// Fee tiers. `filter`: `all`, `bo`, `perps` or `spot`. `GET /api/v1/feeTiers?filter=`
    pub async fn get_fee_tiers(&self, filter: Option<&str>) -> Result<LighterFeeTiers, BoxError> {
        let mut q = Query::new();
        push(&mut q, "filter", filter);
        let url = format!("{BASE_URL}/feeTiers");
        self.get("feeTiers", &url, &q).await
    }

    /// Binary-options events and their markets. `GET /api/v1/events`
    pub async fn get_events(
        &self,
        query: &LighterEventsQuery<'_>,
    ) -> Result<LighterEvents, BoxError> {
        let mut q = Query::new();
        push(&mut q, "event_ids", query.event_ids.map(join));
        push(&mut q, "market_ids", query.market_ids.map(join));
        push(&mut q, "creator_account_index", query.creator_account_index);
        push(&mut q, "cursor", query.cursor);
        push(&mut q, "active", query.active);
        push(&mut q, "limit", query.limit);
        push(&mut q, "tag", query.tag);
        push(&mut q, "event_kind", query.event_kind);
        push(&mut q, "start_from", query.start_from);
        push(&mut q, "start_before", query.start_before);
        push(&mut q, "sort", query.sort);
        push(&mut q, "order", query.order);
        let url = format!("{BASE_URL}/events");
        self.get("events", &url, &q).await
    }

    /// Underlying-price candles of an Up/Down series (e.g. `eth`, `15m`), oldest first, over at
    /// most 24 h. Timestamps are Unix seconds (the API rejects ms here despite the docs).
    /// `resolution`: `1s` or `30s`; `limit`: 1..=3600. `GET /api/v1/upDownCandles`
    #[allow(clippy::too_many_arguments)]
    pub async fn get_up_down_candles(
        &self,
        symbol: &str,
        duration: &str,
        start_timestamp: i64,
        end_timestamp: i64,
        resolution: Option<&str>,
        limit: Option<u32>,
    ) -> Result<LighterUpDownCandles, BoxError> {
        let mut q = vec![
            ("symbol", symbol.to_string()),
            ("duration", duration.to_string()),
            ("start_timestamp", start_timestamp.to_string()),
            ("end_timestamp", end_timestamp.to_string()),
        ];
        push(&mut q, "resolution", resolution);
        push(&mut q, "limit", limit);
        let url = format!("{BASE_URL}/upDownCandles");
        self.get("upDownCandles", &url, &q).await
    }
}
