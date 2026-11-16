use super::{ApiResult, Transport};
use crate::types::{
    NadoAllProducts, NadoApr, NadoAsset, NadoBacklog, NadoBboHistory, NadoBboHistoryQuery,
    NadoCandlesticks, NadoCandlesticksQuery, NadoContracts, NadoContractsSummary,
    NadoEdgeAllProducts, NadoEdgeMarketSnapshots, NadoEdgeTime, NadoFundingRate,
    NadoFundingRateHistory, NadoFundingRateHistoryQuery, NadoFundingRates, NadoHealthGroups,
    NadoInsurance, NadoLiquidatable, NadoMarketLiquidity, NadoMarketPrice, NadoMarketPrices,
    NadoMarketSnapshots, NadoMatches, NadoMatchesQuery, NadoOraclePrices, NadoOracleSnapshots,
    NadoOrderbook, NadoPair, NadoPerpPrices, NadoPerpPricesMap, NadoProductHistory,
    NadoProductHistoryQuery, NadoProductSnapshots, NadoProductType, NadoQuotePrice,
    NadoSnapshotsQuery, NadoSymbols, NadoTickers, NadoTrade, NadoV2Symbols,
};
use crate::NadoNetwork;
use roshar_http::RateLimitedClient;
use serde_json::json;
use std::fmt::Write as _;

/// Public market data: gateway queries, edge (cached) queries, archive (indexer) queries and V2
/// REST.
pub struct MarketApi {
    gateway: Transport,
    edge: Transport,
    archive: Transport,
}

fn push_param(url: &mut String, first: &mut bool, key: &str, value: impl std::fmt::Display) {
    url.push(if *first { '?' } else { '&' });
    *first = false;
    let _ = write!(url, "{key}={value}");
}

fn product_type_str(t: NadoProductType) -> &'static str {
    match t {
        NadoProductType::Spot => "spot",
        NadoProductType::Perp => "perp",
    }
}

impl MarketApi {
    /// Gateway queries, edge queries and archive queries draw on separate per-IP weight budgets
    /// (gateway and archive 2400 / min, edge 12000 / min), so each gets its own limiter at
    /// `requests_per_second`.
    pub fn new(network: NadoNetwork, requests_per_second: u32) -> Self {
        Self {
            gateway: Transport::new(RateLimitedClient::new(requests_per_second, 1), network),
            edge: Transport::new(RateLimitedClient::new(requests_per_second, 1), network),
            archive: Transport::new(RateLimitedClient::new(requests_per_second, 1), network),
        }
    }

    pub fn network(&self) -> NadoNetwork {
        self.gateway.network()
    }

    // ── Gateway queries (`POST /gateway/v1/query`) ──────────────────────────────────────────

    /// `status`: `"active"` or `"failed"`.
    pub async fn status(&self) -> ApiResult<String> {
        self.gateway
            .query(&json!({"type": "status"}), "status")
            .await
    }

    /// `contracts`: chain id and endpoint address (EIP-712 verifying contract).
    pub async fn contracts(&self) -> ApiResult<NadoContracts> {
        self.gateway
            .query(&json!({"type": "contracts"}), "contracts")
            .await
    }

    /// `symbols`: product config keyed by symbol, optionally filtered.
    pub async fn symbols(
        &self,
        product_ids: Option<&[u32]>,
        product_type: Option<NadoProductType>,
    ) -> ApiResult<NadoSymbols> {
        let mut body = json!({"type": "symbols"});
        if let Some(ids) = product_ids {
            body["product_ids"] = json!(ids);
        }
        if let Some(t) = product_type {
            body["product_type"] = json!(product_type_str(t));
        }
        self.gateway.query(&body, "symbols").await
    }

    /// `all_products`: oracle prices, risk weights, interest/funding state and book info.
    pub async fn all_products(&self) -> ApiResult<NadoAllProducts> {
        self.gateway
            .query(&json!({"type": "all_products"}), "all_products")
            .await
    }

    /// `market_liquidity`: order book snapshot, `depth` levels per side (max 100).
    pub async fn market_liquidity(
        &self,
        product_id: u32,
        depth: u32,
    ) -> ApiResult<NadoMarketLiquidity> {
        self.gateway
            .query(
                &json!({"type": "market_liquidity", "product_id": product_id, "depth": depth}),
                "market_liquidity",
            )
            .await
    }

    /// `market_price`: best bid / ask.
    pub async fn market_price(&self, product_id: u32) -> ApiResult<NadoMarketPrice> {
        self.gateway
            .query(
                &json!({"type": "market_price", "product_id": product_id}),
                "market_price",
            )
            .await
    }

    /// `market_prices`: best bid / ask for several products.
    pub async fn market_prices(&self, product_ids: &[u32]) -> ApiResult<NadoMarketPrices> {
        self.gateway
            .query(
                &json!({"type": "market_prices", "product_ids": product_ids}),
                "market_prices",
            )
            .await
    }

    /// `edge_all_products`: `all_products` for every chain, keyed by chain id.
    pub async fn edge_all_products(&self) -> ApiResult<NadoEdgeAllProducts> {
        self.gateway
            .query(&json!({"type": "edge_all_products"}), "edge_all_products")
            .await
    }

    /// `health_groups`: `[spot_product_id, perp_product_id]` pairs margined together.
    pub async fn health_groups(&self) -> ApiResult<NadoHealthGroups> {
        self.gateway
            .query(&json!({"type": "health_groups"}), "health_groups")
            .await
    }

    /// `insurance`: insurance fund value.
    pub async fn insurance(&self) -> ApiResult<NadoInsurance> {
        self.gateway
            .query(&json!({"type": "insurance"}), "insurance")
            .await
    }

    // ── Edge queries (`POST /gateway/v1/edge/query`): cached, eventually consistent ─────────

    /// `cached_prices`: best bid / ask; `None` returns every cached product. Products without a
    /// cached BBO are omitted.
    pub async fn cached_prices(&self, product_ids: Option<&[u32]>) -> ApiResult<NadoMarketPrices> {
        let mut body = json!({"type": "cached_prices"});
        if let Some(ids) = product_ids {
            body["product_ids"] = json!(ids);
        }
        self.edge.edge_query(&body, "cached_prices").await
    }

    /// `cached_symbols`, optionally filtered.
    pub async fn cached_symbols(
        &self,
        product_ids: Option<&[u32]>,
        product_type: Option<NadoProductType>,
    ) -> ApiResult<NadoSymbols> {
        let mut body = json!({"type": "cached_symbols"});
        if let Some(ids) = product_ids {
            body["product_ids"] = json!(ids);
        }
        if let Some(t) = product_type {
            body["product_type"] = json!(product_type_str(t));
        }
        self.edge.edge_query(&body, "cached_symbols").await
    }

    /// `cached_contracts`.
    pub async fn cached_contracts(&self) -> ApiResult<NadoContracts> {
        self.edge
            .edge_query(&json!({"type": "cached_contracts"}), "cached_contracts")
            .await
    }

    /// `cached_status`: e.g. `"active"`.
    pub async fn cached_status(&self) -> ApiResult<String> {
        self.edge
            .edge_query(&json!({"type": "cached_status"}), "cached_status")
            .await
    }

    /// `cached_all_products`.
    pub async fn cached_all_products(&self) -> ApiResult<NadoAllProducts> {
        self.edge
            .edge_query(
                &json!({"type": "cached_all_products"}),
                "cached_all_products",
            )
            .await
    }

    /// `cached_edge_all_products`.
    pub async fn cached_edge_all_products(&self) -> ApiResult<NadoEdgeAllProducts> {
        self.edge
            .edge_query(
                &json!({"type": "cached_edge_all_products"}),
                "cached_edge_all_products",
            )
            .await
    }

    /// `cached_health_groups`.
    pub async fn cached_health_groups(&self) -> ApiResult<NadoHealthGroups> {
        self.edge
            .edge_query(
                &json!({"type": "cached_health_groups"}),
                "cached_health_groups",
            )
            .await
    }

    /// `cached_bbo_history`: sampled BBO time series (30-minute retention).
    pub async fn cached_bbo_history(
        &self,
        query: &NadoBboHistoryQuery,
    ) -> ApiResult<NadoBboHistory> {
        let mut body = json!(query);
        body["type"] = json!("cached_bbo_history");
        self.edge.edge_query(&body, "cached_bbo_history").await
    }

    /// Edge `ping`: echoes `client_time_ms` with the server time.
    pub async fn edge_ping(
        &self,
        id: Option<u64>,
        client_time_ms: Option<u64>,
    ) -> ApiResult<NadoEdgeTime> {
        let mut body = json!({"type": "ping"});
        if let Some(id) = id {
            body["id"] = json!(id);
        }
        if let Some(t) = client_time_ms {
            body["client_time"] = json!(t.to_string());
        }
        self.edge.edge_control(&body, "ping").await
    }

    /// Edge `time`: server time.
    pub async fn edge_time(&self, id: Option<u64>) -> ApiResult<NadoEdgeTime> {
        let mut body = json!({"type": "time"});
        if let Some(id) = id {
            body["id"] = json!(id);
        }
        self.edge.edge_control(&body, "time").await
    }

    // ── Archive queries (`POST /archive/v1`) ────────────────────────────────────────────────

    /// `candlesticks`, newest first.
    pub async fn candlesticks(&self, query: &NadoCandlesticksQuery) -> ApiResult<NadoCandlesticks> {
        self.archive
            .archive(&json!({"candlesticks": query}), "candlesticks")
            .await
    }

    /// `edge_candlesticks`: candles aggregated across all chains, newest first.
    pub async fn edge_candlesticks(
        &self,
        query: &NadoCandlesticksQuery,
    ) -> ApiResult<NadoCandlesticks> {
        self.archive
            .archive(&json!({"edge_candlesticks": query}), "edge_candlesticks")
            .await
    }

    /// `products`: snapshots of one product with their transactions, newest first.
    pub async fn product_history(
        &self,
        query: &NadoProductHistoryQuery,
    ) -> ApiResult<NadoProductHistory> {
        self.archive
            .archive(&json!({"products": query}), "products")
            .await
    }

    /// `product_snapshots`: latest snapshot (at or before `max_time`, unix seconds) per product.
    pub async fn product_snapshots(
        &self,
        product_ids: &[u32],
        max_time: Option<u64>,
    ) -> ApiResult<NadoProductSnapshots> {
        let mut q = json!({"product_ids": product_ids});
        if let Some(t) = max_time {
            q["max_time"] = json!(t);
        }
        self.archive
            .archive(&json!({"product_snapshots": q}), "product_snapshots")
            .await
    }

    /// `oracle_snapshots`: historical oracle prices, newest first.
    pub async fn oracle_snapshots(
        &self,
        query: &NadoSnapshotsQuery,
    ) -> ApiResult<NadoOracleSnapshots> {
        self.archive
            .archive(&json!({"oracle_snapshots": query}), "oracle_snapshots")
            .await
    }

    /// `market_snapshots`: OI, funding, volume, fees, deposits / borrows history, newest first.
    pub async fn market_snapshots(
        &self,
        query: &NadoSnapshotsQuery,
    ) -> ApiResult<NadoMarketSnapshots> {
        self.archive
            .archive(&json!({"market_snapshots": query}), "market_snapshots")
            .await
    }

    /// `matches_and_liquidations`: fills plus liquidations (entries whose tx is
    /// `liquidate_subaccount`, with a zeroed digest and order), newest first. Filter by
    /// `product_ids` for public market history.
    pub async fn matches_and_liquidations(
        &self,
        query: &NadoMatchesQuery,
    ) -> ApiResult<NadoMatches> {
        self.archive
            .archive(
                &json!({"matches_and_liquidations": query}),
                "matches_and_liquidations",
            )
            .await
    }

    /// `edge_market_snapshots`: `market_snapshots` keyed by chain id.
    pub async fn edge_market_snapshots(
        &self,
        query: &NadoSnapshotsQuery,
    ) -> ApiResult<NadoEdgeMarketSnapshots> {
        self.archive
            .archive(
                &json!({"edge_market_snapshots": query}),
                "edge_market_snapshots",
            )
            .await
    }

    /// `liquidation_feed`: subaccounts currently eligible for liquidation.
    pub async fn liquidation_feed(&self) -> ApiResult<Vec<NadoLiquidatable>> {
        self.archive
            .archive(&json!({"liquidation_feed": {}}), "liquidation_feed")
            .await
    }

    /// `quote_price`: USDT0 price in USD.
    pub async fn quote_price(&self) -> ApiResult<NadoQuotePrice> {
        self.archive
            .archive(&json!({"quote_price": {}}), "quote_price")
            .await
    }

    /// `backlog`: off-chain sequencer backlog.
    pub async fn backlog(&self) -> ApiResult<NadoBacklog> {
        self.archive
            .archive(&json!({"backlog": {}}), "backlog")
            .await
    }

    /// `funding_rate`: latest predicted 24h funding rate.
    pub async fn funding_rate(&self, product_id: u32) -> ApiResult<NadoFundingRate> {
        self.archive
            .archive(
                &json!({"funding_rate": {"product_id": product_id}}),
                "funding_rate",
            )
            .await
    }

    /// `funding_rates`: latest predicted 24h funding rates, keyed by product id.
    pub async fn funding_rates(&self, product_ids: &[u32]) -> ApiResult<NadoFundingRates> {
        self.archive
            .archive(
                &json!({"funding_rates": {"product_ids": product_ids}}),
                "funding_rates",
            )
            .await
    }

    /// `funding_rate_history`: realized hourly rates, oldest first.
    pub async fn funding_rate_history(
        &self,
        query: &NadoFundingRateHistoryQuery,
    ) -> ApiResult<NadoFundingRateHistory> {
        self.archive
            .archive(
                &json!({"funding_rate_history": query}),
                "funding_rate_history",
            )
            .await
    }

    /// `price`: index and mark price of one perp.
    pub async fn perp_price(&self, product_id: u32) -> ApiResult<NadoPerpPrices> {
        self.archive
            .archive(&json!({"price": {"product_id": product_id}}), "price")
            .await
    }

    /// `perp_prices`: index and mark prices, keyed by product id.
    pub async fn perp_prices(&self, product_ids: &[u32]) -> ApiResult<NadoPerpPricesMap> {
        self.archive
            .archive(
                &json!({"perp_prices": {"product_ids": product_ids}}),
                "perp_prices",
            )
            .await
    }

    /// `oracle_price`: latest oracle prices.
    pub async fn oracle_prices(&self, product_ids: &[u32]) -> ApiResult<NadoOraclePrices> {
        self.archive
            .archive(
                &json!({"oracle_price": {"product_ids": product_ids}}),
                "oracle_price",
            )
            .await
    }

    // ── V2 REST (`GET /gateway/v2/*`, `GET /archive/v2/*`) ──────────────────────────────────

    /// `GET /gateway/v2/assets`.
    pub async fn assets(&self) -> ApiResult<Vec<NadoAsset>> {
        let url = format!("{}/assets", self.network().gateway_v2_url());
        self.gateway.get_v2(&url, "v2 assets").await
    }

    /// `GET /gateway/v2/pairs[?market=spot|perp]`.
    pub async fn pairs(&self, market: Option<NadoProductType>) -> ApiResult<Vec<NadoPair>> {
        let mut url = format!("{}/pairs", self.network().gateway_v2_url());
        if let Some(m) = market {
            url.push_str("?market=");
            url.push_str(product_type_str(m));
        }
        self.gateway.get_v2(&url, "v2 pairs").await
    }

    /// `GET /gateway/v2/apr`: spot deposit / borrow APRs.
    pub async fn apr(&self) -> ApiResult<Vec<NadoApr>> {
        let url = format!("{}/apr", self.network().gateway_v2_url());
        self.gateway.get_v2(&url, "v2 apr").await
    }

    /// `GET /gateway/v2/orderbook?ticker_id=..&depth=..` (e.g. `BTC-PERP_USDT0`).
    pub async fn orderbook(&self, ticker_id: &str, depth: u32) -> ApiResult<NadoOrderbook> {
        let url = format!(
            "{}/orderbook?ticker_id={ticker_id}&depth={depth}",
            self.network().gateway_v2_url()
        );
        self.gateway.get_v2(&url, "v2 orderbook").await
    }

    /// `GET /archive/v2/tickers[?market=..][&edge=..]`: 24h stats keyed by ticker id.
    pub async fn tickers(
        &self,
        market: Option<NadoProductType>,
        edge: Option<bool>,
    ) -> ApiResult<NadoTickers> {
        let mut url = format!("{}/tickers", self.network().archive_v2_url());
        let mut first = true;
        if let Some(m) = market {
            push_param(&mut url, &mut first, "market", product_type_str(m));
        }
        if let Some(e) = edge {
            push_param(&mut url, &mut first, "edge", e);
        }
        self.archive.get_v2(&url, "v2 tickers").await
    }

    /// `GET /archive/v2/contracts[?edge=..]`: perp summary (OI, index / mark, funding) keyed by
    /// ticker id.
    pub async fn contracts_summary(&self, edge: Option<bool>) -> ApiResult<NadoContractsSummary> {
        let mut url = format!("{}/contracts", self.network().archive_v2_url());
        if let Some(e) = edge {
            let _ = write!(url, "?edge={e}");
        }
        self.archive.get_v2(&url, "v2 contracts").await
    }

    /// `GET /archive/v2/trades?ticker_id=..[&limit=..][&max_trade_id=..]`, newest first.
    pub async fn trades(
        &self,
        ticker_id: &str,
        limit: Option<u32>,
        max_trade_id: Option<u64>,
    ) -> ApiResult<Vec<NadoTrade>> {
        let mut url = format!(
            "{}/trades?ticker_id={ticker_id}",
            self.network().archive_v2_url()
        );
        let mut first = false;
        if let Some(l) = limit {
            push_param(&mut url, &mut first, "limit", l);
        }
        if let Some(m) = max_trade_id {
            push_param(&mut url, &mut first, "max_trade_id", m);
        }
        self.archive.get_v2(&url, "v2 trades").await
    }

    /// `GET /archive/v2/symbols`: symbols with market hours, price feeds and boosts.
    pub async fn v2_symbols(&self) -> ApiResult<NadoV2Symbols> {
        let url = format!("{}/symbols", self.network().archive_v2_url());
        self.archive.get_v2(&url, "v2 symbols").await
    }
}
