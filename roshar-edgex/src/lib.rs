//! edgeX (V2 API) client: public market data, websocket config, HMAC request auth, EIP-712 L2
//! order signing and private REST.

/// Wire and message types (REST payloads, websocket frames, request bodies).
pub mod types;

/// HMAC request auth and EIP-712 L2 signing.
pub mod signer;

pub(crate) mod rest;

pub use rest::market::{EdgexFundingRateQuery, EdgexKlineQuery, EdgexMultiKlineQuery};
pub use rest::MarketApi as EdgexMarketApi;
pub use rest::{SignedRequest, TradingApi};

use crate::signer::ApiCredentials;
use crate::types::{
    EdgexChannel, EdgexContractKlines, EdgexDepth, EdgexDepthLevel, EdgexFundingRate, EdgexKline,
    EdgexKlineType, EdgexMarketStatus, EdgexMetaData, EdgexPage, EdgexPriceType, EdgexServerTime,
    EdgexTicker, EdgexWsRequest,
};
use anyhow::Result;
use async_trait::async_trait;
use roshar_http::RateLimitedClient;
use roshar_ws_mgr::{Config, Manager, Message};
use std::sync::Arc;

/// REST host for everything except the asset (deposit/withdraw) API.
pub const EDGEX_API_URL: &str = "https://edgex-prod-v2.edgex.exchange";
/// Public market-data websocket. The path keeps `/api/v1/` for compatibility (per the docs).
pub const EDGEX_PUBLIC_WSS_URL: &str = "wss://edgex-quote-prod-v2.edgex.exchange/api/v1/public/ws";
/// Private account websocket; query `accountId` and `timestamp` are required.
pub const EDGEX_PRIVATE_WSS_URL: &str =
    "wss://edgex-quote-prod-v2.edgex.exchange/api/v1/private/ws";
/// Request path signed for the private websocket handshake.
pub const EDGEX_PRIVATE_WS_PATH: &str = "/api/v1/private/ws";

type ApiResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// Websocket keepalive shared by both connections. The server sends a JSON `ping` every 10 s and
/// answers both protocol pings and JSON `{"type":"ping"}`; a connection left without pongs for 75 s
/// stayed open in testing. roshar-ws-mgr sends protocol pings and the server's protocol pongs drive
/// the timeout. Callers that want to answer the JSON pings can use [`EdgexClient::send_pong`].
fn base_ws_config(name: &str, url: String, headers: Vec<(String, String)>) -> Config {
    Config {
        name: name.to_string(),
        url,
        ping_duration: 10,
        ping_message: String::new(),
        ping_timeout: 30,
        reconnect_timeout: 5000,
        use_text_ping: Some(false),
        read_buffer_size: None,
        write_buffer_size: None,
        max_message_size: None,
        max_frame_size: None,
        tcp_recv_buffer_size: None,
        tcp_send_buffer_size: None,
        tcp_nodelay: Some(true),
        broadcast_channel_size: None,
        headers,
    }
}

/// Private websocket config: URL `?accountId={id}&timestamp={ts}` and the four `X-edgeX-*`
/// headers signed over `ts + "GET" + "/api/v1/private/ws" + "accountId={id}&timestamp={ts}"`.
/// The timestamp is fixed in the config, so build a fresh config (new `timestamp_ms`) before each
/// reconnect. No subscribe message is needed.
pub fn private_ws_config(
    credentials: &ApiCredentials,
    account_id: u64,
    timestamp_ms: u64,
) -> Config {
    let query = format!("accountId={account_id}&timestamp={timestamp_ms}");
    let headers = credentials.headers(timestamp_ms, "GET", EDGEX_PRIVATE_WS_PATH, &query);
    base_ws_config(
        "edgex-private",
        format!("{EDGEX_PRIVATE_WSS_URL}?{query}"),
        headers,
    )
}

pub struct EdgexClient {
    market_api: rest::MarketApi,
    http: RateLimitedClient,
    ws_config: Config,
}

impl Default for EdgexClient {
    fn default() -> Self {
        Self::new()
    }
}

impl EdgexClient {
    /// edgeX publishes no numeric REST limit (the docs say only "Public API: higher rate limits",
    /// HTTP 429 when exceeded). 5 req/s matches the "50 requests per 10 seconds" that third-party
    /// clients observe; override with `new_with_rate_limit`.
    const DEFAULT_REQUESTS_PER_SECOND: u32 = 5;
    const WS_CONN_NAME: &str = "edgex";

    pub fn new() -> Self {
        Self::new_with_rate_limit(Self::DEFAULT_REQUESTS_PER_SECOND)
    }

    pub fn new_with_rate_limit(requests_per_second: u32) -> Self {
        let http = RateLimitedClient::new(requests_per_second, 1);
        Self {
            market_api: rest::MarketApi::new(http.clone(), EDGEX_API_URL),
            http,
            ws_config: base_ws_config(Self::WS_CONN_NAME, EDGEX_PUBLIC_WSS_URL.to_string(), vec![]),
        }
    }

    /// The rate-limited HTTP client, to share with a [`TradingApi`].
    pub fn http_client(&self) -> RateLimitedClient {
        self.http.clone()
    }

    pub fn ws_config(&self) -> roshar_ws_mgr::Config {
        self.ws_config.clone()
    }

    pub fn ws_config_mut(&mut self) -> &mut roshar_ws_mgr::Config {
        &mut self.ws_config
    }

    fn send(&self, manager: &Arc<Manager>, msg: EdgexWsRequest) -> Result<()> {
        manager.write(
            &self.ws_config.name,
            Message::TextMessage(self.ws_config.name.clone(), msg.to_json()),
        )?;
        Ok(())
    }

    fn subscribe_each(
        &self,
        manager: &Arc<Manager>,
        channels: impl IntoIterator<Item = String>,
        subscribe: bool,
    ) -> Result<()> {
        for channel in channels {
            let msg = if subscribe {
                EdgexWsRequest::subscribe(channel)
            } else {
                EdgexWsRequest::unsubscribe(channel)
            };
            self.send(manager, msg)?;
        }
        Ok(())
    }

    /// Answer a server `{"type":"ping","time":t}` with `{"type":"pong","time":t}`.
    pub fn send_pong(&self, manager: &Arc<Manager>, time: &str) -> Result<()> {
        self.send(manager, EdgexWsRequest::pong(time))
    }

    /// `ticker.{contractId}`.
    pub fn subscribe_ticker(&self, manager: &Arc<Manager>, contract_ids: &[&str]) -> Result<()> {
        self.subscribe_each(
            manager,
            contract_ids.iter().map(|c| EdgexChannel::ticker(c)),
            true,
        )
    }

    pub fn unsubscribe_ticker(&self, manager: &Arc<Manager>, contract_ids: &[&str]) -> Result<()> {
        self.subscribe_each(
            manager,
            contract_ids.iter().map(|c| EdgexChannel::ticker(c)),
            false,
        )
    }

    /// `ticker.all.1s`.
    pub fn subscribe_all_tickers(&self, manager: &Arc<Manager>) -> Result<()> {
        self.subscribe_each(manager, [EdgexChannel::ticker_all()], true)
    }

    pub fn unsubscribe_all_tickers(&self, manager: &Arc<Manager>) -> Result<()> {
        self.subscribe_each(manager, [EdgexChannel::ticker_all()], false)
    }

    /// `depth.{contractId}.{level}`: a `Snapshot` then `changed` deltas.
    pub fn subscribe_depth(
        &self,
        manager: &Arc<Manager>,
        contract_ids: &[&str],
        level: EdgexDepthLevel,
    ) -> Result<()> {
        self.subscribe_each(
            manager,
            contract_ids.iter().map(|c| EdgexChannel::depth(c, level)),
            true,
        )
    }

    pub fn unsubscribe_depth(
        &self,
        manager: &Arc<Manager>,
        contract_ids: &[&str],
        level: EdgexDepthLevel,
    ) -> Result<()> {
        self.subscribe_each(
            manager,
            contract_ids.iter().map(|c| EdgexChannel::depth(c, level)),
            false,
        )
    }

    /// `trades.{contractId}`.
    pub fn subscribe_trades(&self, manager: &Arc<Manager>, contract_ids: &[&str]) -> Result<()> {
        self.subscribe_each(
            manager,
            contract_ids.iter().map(|c| EdgexChannel::trades(c)),
            true,
        )
    }

    pub fn unsubscribe_trades(&self, manager: &Arc<Manager>, contract_ids: &[&str]) -> Result<()> {
        self.subscribe_each(
            manager,
            contract_ids.iter().map(|c| EdgexChannel::trades(c)),
            false,
        )
    }

    /// `kline.{priceType}.{contractId}.{interval}`.
    pub fn subscribe_klines(
        &self,
        manager: &Arc<Manager>,
        contract_ids: &[&str],
        price_type: EdgexPriceType,
        interval: EdgexKlineType,
    ) -> Result<()> {
        self.subscribe_each(
            manager,
            contract_ids
                .iter()
                .map(|c| EdgexChannel::kline(c, price_type, interval)),
            true,
        )
    }

    pub fn unsubscribe_klines(
        &self,
        manager: &Arc<Manager>,
        contract_ids: &[&str],
        price_type: EdgexPriceType,
        interval: EdgexKlineType,
    ) -> Result<()> {
        self.subscribe_each(
            manager,
            contract_ids
                .iter()
                .map(|c| EdgexChannel::kline(c, price_type, interval)),
            false,
        )
    }

    /// `fundingRate.{contractId}`.
    pub fn subscribe_funding(&self, manager: &Arc<Manager>, contract_ids: &[&str]) -> Result<()> {
        self.subscribe_each(
            manager,
            contract_ids.iter().map(|c| EdgexChannel::funding_rate(c)),
            true,
        )
    }

    pub fn unsubscribe_funding(&self, manager: &Arc<Manager>, contract_ids: &[&str]) -> Result<()> {
        self.subscribe_each(
            manager,
            contract_ids.iter().map(|c| EdgexChannel::funding_rate(c)),
            false,
        )
    }

    /// `fundingRate.all`.
    pub fn subscribe_all_funding(&self, manager: &Arc<Manager>) -> Result<()> {
        self.subscribe_each(manager, [EdgexChannel::funding_rate_all()], true)
    }

    pub fn unsubscribe_all_funding(&self, manager: &Arc<Manager>) -> Result<()> {
        self.subscribe_each(manager, [EdgexChannel::funding_rate_all()], false)
    }

    /// `metadata`.
    pub fn subscribe_metadata(&self, manager: &Arc<Manager>) -> Result<()> {
        self.subscribe_each(manager, [EdgexChannel::metadata()], true)
    }

    pub fn unsubscribe_metadata(&self, manager: &Arc<Manager>) -> Result<()> {
        self.subscribe_each(manager, [EdgexChannel::metadata()], false)
    }
}

#[async_trait]
pub trait EdgexApi {
    async fn get_server_time(&self) -> ApiResult<EdgexServerTime>;
    async fn get_meta_data(&self) -> ApiResult<EdgexMetaData>;
    async fn get_depth(
        &self,
        contract_id: &str,
        level: EdgexDepthLevel,
    ) -> ApiResult<Vec<EdgexDepth>>;
    async fn get_kline(&self, query: &EdgexKlineQuery) -> ApiResult<EdgexPage<EdgexKline>>;
    async fn get_multi_contract_kline(
        &self,
        query: &EdgexMultiKlineQuery,
    ) -> ApiResult<Vec<EdgexContractKlines>>;
    async fn get_ticker(&self, contract_id: Option<&str>) -> ApiResult<Vec<EdgexTicker>>;
    async fn get_market_status(&self, contract_id: Option<&str>) -> ApiResult<EdgexMarketStatus>;
    async fn get_latest_funding_rate(
        &self,
        contract_ids: &[&str],
    ) -> ApiResult<Vec<EdgexFundingRate>>;
    async fn get_funding_rate_page(
        &self,
        query: &EdgexFundingRateQuery,
    ) -> ApiResult<EdgexPage<EdgexFundingRate>>;
}

#[async_trait]
impl EdgexApi for EdgexClient {
    async fn get_server_time(&self) -> ApiResult<EdgexServerTime> {
        self.market_api.get_server_time().await
    }

    async fn get_meta_data(&self) -> ApiResult<EdgexMetaData> {
        self.market_api.get_meta_data().await
    }

    async fn get_depth(
        &self,
        contract_id: &str,
        level: EdgexDepthLevel,
    ) -> ApiResult<Vec<EdgexDepth>> {
        self.market_api.get_depth(contract_id, level).await
    }

    async fn get_kline(&self, query: &EdgexKlineQuery) -> ApiResult<EdgexPage<EdgexKline>> {
        self.market_api.get_kline(query).await
    }

    async fn get_multi_contract_kline(
        &self,
        query: &EdgexMultiKlineQuery,
    ) -> ApiResult<Vec<EdgexContractKlines>> {
        self.market_api.get_multi_contract_kline(query).await
    }

    async fn get_ticker(&self, contract_id: Option<&str>) -> ApiResult<Vec<EdgexTicker>> {
        self.market_api.get_ticker(contract_id).await
    }

    async fn get_market_status(&self, contract_id: Option<&str>) -> ApiResult<EdgexMarketStatus> {
        self.market_api.get_market_status(contract_id).await
    }

    async fn get_latest_funding_rate(
        &self,
        contract_ids: &[&str],
    ) -> ApiResult<Vec<EdgexFundingRate>> {
        self.market_api.get_latest_funding_rate(contract_ids).await
    }

    async fn get_funding_rate_page(
        &self,
        query: &EdgexFundingRateQuery,
    ) -> ApiResult<EdgexPage<EdgexFundingRate>> {
        self.market_api.get_funding_rate_page(query).await
    }
}
