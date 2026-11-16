//! Authenticated REST: JWT auth, order entry, open orders, positions, balances, fills.
//!
//! `ParadexExchange::new(PARADEX_API_URL)` wraps the private endpoints of
//! https://docs.paradex.trade/api/prod. Call `auth` for a JWT (Paradex JWTs expire after 5 minutes
//! and cannot be extended; refreshing is the caller's job) and pass it to the other calls.
//! Orders are signed with `ParadexSigner` before they are sent (`NewOrder::sign`,
//! `ModifyOrder::sign`); the same signed requests go over the websocket (`ws_private`).
//!
//! Responses are parsed strictly. Paradex's OpenAPI marks every field optional; fields here are
//! required where the websocket channel schemas list them as required (the REST and websocket
//! payloads are the same objects) or every doc example carries them, and `Option` where the
//! schemas leave them out. Non-2xx replies are returned as `Err` with the exchange's body
//! (`{"error": CODE, "message": ..}`).
//!
//! Not covered: onboarding (`POST /onboarding`; accounts created in the Paradex app are already
//! onboarded), stop / take-profit / stop-loss orders, batch create/cancel, isolated-market
//! accounts (`on_behalf_of_account`, `PARADEX-AUTHORIZE-ISOLATED-MARKETS`), MMP.

use anyhow::{bail, Context, Result};
use reqwest::{Method, RequestBuilder};
use roshar_http::RateLimitedClient;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::signer::{OrderMessage, OrderSide, OrderType, ParadexSigner};
use crate::types::{ParadexListResponse, ParadexPageQuery};

pub const PARADEX_API_URL: &str = "https://api.prod.paradex.trade/v1";
pub const PARADEX_TESTNET_API_URL: &str = "https://api.testnet.paradex.trade/v1";

/// Paradex's per-IP limit, 1500 requests a minute, which every private endpoint also counts
/// against. It is lower than the per-account order limit (800/s or 17250/min), so it is the one
/// a single client hits first.
pub const DEFAULT_REQUESTS_PER_SECOND: u32 = 25;
/// Paradex's per-account limit for private `GET`s: 120/s or 600/min.
pub const DEFAULT_READS_PER_SECOND: u32 = 10;

// ── Enums ────────────────────────────────────────────────────────────────────

/// `token_usage` query parameter of `POST /auth`. An interactive token makes the account's
/// orders Retail Orders (speed bump, lower rate limits, retail fees and RPI access).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenUsage {
    Interactive,
}

impl TokenUsage {
    pub fn as_str(self) -> &'static str {
        match self {
            TokenUsage::Interactive => "interactive",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Instruction {
    Gtc,
    PostOnly,
    Ioc,
    Rpi,
}

/// Self-trade prevention. Unset on a new order means `EXPIRE_TAKER`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Stp {
    ExpireMaker,
    ExpireTaker,
    ExpireBoth,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OrderFlag {
    ReduceOnly,
    StopConditionBelowTrigger,
    StopConditionAboveTrigger,
    Interactive,
    TargetStrategyVwap,
    Mmp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OrderStatus {
    New,
    Untriggered,
    Open,
    Closed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PositionSide {
    Long,
    Short,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PositionStatus {
    Open,
    Closed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FillType {
    Fill,
    Liquidation,
    UnwindTransfer,
    SettleMarket,
    Rpi,
    BlockTrade,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Liquidity {
    Maker,
    Taker,
}

/// Fill flags; Paradex sends these in lowercase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FillFlag {
    Interactive,
    Rpi,
    Fastfill,
}

// ── Order requests ───────────────────────────────────────────────────────────

/// `MARKET` orders carry no price (signed as 0); `LIMIT` orders carry a decimal price string.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderKind<'a> {
    Market,
    Limit { price: &'a str },
}

/// A new order before signing. `size` and `price` are decimal strings (`digits[.digits]`); they
/// are sent as given and signed as integers with 8 decimals, truncated (see `signer::quantize`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NewOrder<'a> {
    pub market: &'a str,
    pub side: OrderSide,
    pub kind: OrderKind<'a>,
    pub size: &'a str,
    pub instruction: Instruction,
    pub client_id: Option<&'a str>,
    pub reduce_only: bool,
    /// Unset: the exchange default, `EXPIRE_TAKER`.
    pub stp: Option<Stp>,
    /// The order is rejected unless received within this many ms of `signature_timestamp_ms`
    /// (minimum 10).
    pub recv_window_ms: Option<u64>,
    /// Unix ms. Signed; Paradex uses it as the order nonce.
    pub signature_timestamp_ms: u64,
}

impl<'a> NewOrder<'a> {
    pub fn order_type(&self) -> OrderType {
        match self.kind {
            OrderKind::Market => OrderType::Market,
            OrderKind::Limit { .. } => OrderType::Limit,
        }
    }

    fn price(&self) -> Option<&'a str> {
        match self.kind {
            OrderKind::Market => None,
            OrderKind::Limit { price } => Some(price),
        }
    }

    /// The signed fields of this order.
    pub fn message(&self) -> OrderMessage<'a> {
        OrderMessage {
            timestamp_ms: self.signature_timestamp_ms,
            market: self.market,
            side: self.side,
            order_type: self.order_type(),
            size: self.size,
            price: self.price(),
        }
    }

    /// Sign and build the `POST /orders` (and websocket `order.create`) body.
    pub fn sign(&self, signer: &ParadexSigner) -> Result<OrderRequest> {
        let signature = signer.sign_order(&self.message())?.flattened();
        Ok(OrderRequest {
            market: self.market.to_string(),
            side: self.side,
            size: self.size.to_string(),
            order_type: self.order_type(),
            client_id: self.client_id.map(str::to_string),
            instruction: self.instruction,
            signature,
            signature_timestamp: self.signature_timestamp_ms,
            recv_window: self.recv_window_ms,
            stp: self.stp,
            price: self.price().map(str::to_string),
            flags: if self.reduce_only {
                vec![OrderFlag::ReduceOnly]
            } else {
                Vec::new()
            },
        })
    }
}

/// Signed `requests.OrderRequest`. Field order and content follow paradex-py's
/// `Order.dump_to_dict`, except that unset optional fields are left out rather than sent as
/// `null` / `""`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OrderRequest {
    market: String,
    side: OrderSide,
    size: String,
    #[serde(rename = "type")]
    order_type: OrderType,
    #[serde(skip_serializing_if = "Option::is_none")]
    client_id: Option<String>,
    instruction: Instruction,
    signature: String,
    signature_timestamp: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    recv_window: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stp: Option<Stp>,
    #[serde(skip_serializing_if = "Option::is_none")]
    price: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    flags: Vec<OrderFlag>,
}

impl OrderRequest {
    pub fn signature(&self) -> &str {
        &self.signature
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("OrderRequest serialises")
    }
}

/// A change to an open `LIMIT` order: the new (or unchanged) price and size, with the order's
/// existing market and side. Signed as `ModifyOrder`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModifyOrder<'a> {
    pub order_id: &'a str,
    pub market: &'a str,
    pub side: OrderSide,
    pub size: &'a str,
    pub price: &'a str,
    /// Unix ms.
    pub signature_timestamp_ms: u64,
}

impl<'a> ModifyOrder<'a> {
    pub fn message(&self) -> OrderMessage<'a> {
        OrderMessage {
            timestamp_ms: self.signature_timestamp_ms,
            market: self.market,
            side: self.side,
            order_type: OrderType::Limit,
            size: self.size,
            price: Some(self.price),
        }
    }

    /// Sign and build the `PUT /orders/{id}` (and websocket `order.modify`) body.
    pub fn sign(&self, signer: &ParadexSigner) -> Result<ModifyOrderRequest> {
        let signature = signer
            .sign_modify_order(&self.message(), self.order_id)?
            .flattened();
        Ok(ModifyOrderRequest {
            id: self.order_id.to_string(),
            market: self.market.to_string(),
            price: self.price.to_string(),
            side: self.side,
            signature,
            signature_timestamp: self.signature_timestamp_ms,
            size: self.size.to_string(),
            order_type: OrderType::Limit,
        })
    }
}

/// Signed `requests.ModifyOrderRequest`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ModifyOrderRequest {
    id: String,
    market: String,
    price: String,
    side: OrderSide,
    signature: String,
    signature_timestamp: u64,
    size: String,
    #[serde(rename = "type")]
    order_type: OrderType,
}

impl ModifyOrderRequest {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("ModifyOrderRequest serialises")
    }
}

// ── Responses (REST and websocket JSON) ──────────────────────────────────────

/// `responses.RequestInfo`: outcome of a modify request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParadexRequestInfo {
    pub id: String,
    pub message: String,
    pub request_type: String,
    pub status: String,
}

/// `responses.OrderResp`: REST order replies and the `orders.{market}` channel.
/// Strings that have no value (e.g. `cancel_reason` of an open order) are `""`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParadexOrder {
    pub id: String,
    pub account: String,
    pub market: String,
    pub side: OrderSide,
    #[serde(rename = "type")]
    pub order_type: OrderType,
    pub status: OrderStatus,
    pub instruction: Instruction,
    pub stp: Stp,
    pub flags: Vec<OrderFlag>,
    pub client_id: String,
    /// `"0"` for `MARKET` orders.
    pub price: String,
    pub size: String,
    pub remaining_size: String,
    pub avg_fill_price: String,
    pub trigger_price: String,
    pub cancel_reason: String,
    /// Signature timestamp, ms.
    pub timestamp: i64,
    pub created_at: i64,
    pub received_at: i64,
    pub published_at: i64,
    pub last_updated_at: i64,
    pub seq_no: i64,
    /// Present on orders with a modify request.
    pub request_info: Option<ParadexRequestInfo>,
}

/// `responses.PositionResp`: `GET /positions` and the `positions` channel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParadexPosition {
    pub id: String,
    pub account: String,
    pub market: String,
    pub side: PositionSide,
    pub status: PositionStatus,
    /// Signed: negative when short.
    pub size: String,
    pub average_entry_price: String,
    pub average_entry_price_usd: String,
    pub average_exit_price: String,
    pub cost: String,
    pub cost_usd: String,
    pub cached_funding_index: String,
    pub leverage: String,
    pub liquidation_price: String,
    pub unrealized_pnl: String,
    pub unrealized_funding_pnl: String,
    pub realized_positional_pnl: String,
    pub realized_positional_funding_pnl: String,
    pub last_fill_id: String,
    pub seq_no: i64,
    pub created_at: Option<i64>,
    pub closed_at: Option<i64>,
    pub last_updated_at: Option<i64>,
}

/// `responses.BalanceResp`: one token of `GET /balance`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParadexBalance {
    pub token: String,
    pub size: String,
    pub average_entry_price: String,
    pub average_entry_price_usd: String,
    pub cost: String,
    pub cost_usd: String,
    pub last_updated_at: i64,
}

/// `responses.FillResult`: `GET /fills` and the `fills.{market}` channel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParadexFill {
    pub id: String,
    pub account: String,
    pub market: String,
    pub order_id: String,
    pub client_id: String,
    pub fill_type: FillType,
    pub liquidity: Liquidity,
    /// Taker side.
    pub side: OrderSide,
    pub price: String,
    pub size: String,
    pub remaining_size: String,
    pub fee: String,
    pub fee_currency: String,
    pub realized_pnl: String,
    pub realized_funding: String,
    pub underlying_price: String,
    pub flags: Vec<FillFlag>,
    pub created_at: i64,
    /// Omitted for spot, options, liquidations and older fills.
    pub leverage: Option<String>,
    /// Order book sequence number after the fill; 0 for position transfers.
    pub orderbook_seq_no: Option<i64>,
}

/// Filters of `GET /fills`. Unset fields are left to the exchange (page size 100).
#[derive(Debug, Clone, Default)]
pub struct FillsQuery<'a> {
    pub market: Option<&'a str>,
    /// `PERP`, `PERP_OPTION`, `OPTION` or `SPOT`.
    pub asset_kind: Option<&'a str>,
    pub page: ParadexPageQuery,
}

#[derive(Deserialize)]
struct Results<T> {
    results: Vec<T>,
}

#[derive(Debug, Deserialize)]
struct Jwt {
    jwt_token: String,
}

// ── Client ───────────────────────────────────────────────────────────────────

pub struct ParadexExchange {
    base: String,
    /// Every request: the per-IP limit.
    requests: RateLimitedClient,
    /// Private GETs in addition: the per-account read limit.
    reads: RateLimitedClient,
}

impl ParadexExchange {
    /// `base_url` including `/v1`, e.g. `PARADEX_API_URL`. Rate limits are Paradex's documented
    /// defaults (`DEFAULT_REQUESTS_PER_SECOND`, `DEFAULT_READS_PER_SECOND`).
    pub fn new(base_url: &str) -> ParadexExchange {
        Self::with_limits(
            base_url,
            DEFAULT_REQUESTS_PER_SECOND,
            DEFAULT_READS_PER_SECOND,
        )
    }

    /// `requests_per_second` applies to every request, `reads_per_second` additionally to
    /// private GETs.
    pub fn with_limits(
        base_url: &str,
        requests_per_second: u32,
        reads_per_second: u32,
    ) -> ParadexExchange {
        ParadexExchange {
            base: base_url.trim_end_matches('/').to_string(),
            requests: RateLimitedClient::new(requests_per_second, 1),
            reads: RateLimitedClient::new(reads_per_second, 1),
        }
    }

    async fn write(&self, method: Method, path: &str, jwt: &str) -> RequestBuilder {
        self.requests
            .request(method, &format!("{}/{path}", self.base))
            .await
            .bearer_auth(jwt)
    }

    async fn read(&self, path: &str, jwt: &str) -> RequestBuilder {
        self.reads.acquire().await;
        self.write(Method::GET, path, jwt).await
    }

    /// `POST /auth/{public_key}`: a JWT for the signer's account. The signature covers
    /// `timestamp_s` and `expiry_s` (unix seconds; Paradex accepts an expiry of at most a week,
    /// default 30 min). The path carries the signing key's public key, which is how Paradex
    /// tells a subkey from the account's main key.
    pub async fn auth(
        &self,
        signer: &ParadexSigner,
        timestamp_s: u64,
        expiry_s: u64,
        token_usage: Option<TokenUsage>,
    ) -> Result<String> {
        let signature = signer.sign_auth(timestamp_s, expiry_s)?.flattened();
        let url = format!("{}/auth/{}", self.base, signer.public_key_hex());
        let mut rb = self
            .requests
            .request(Method::POST, &url)
            .await
            .header("PARADEX-STARKNET-ACCOUNT", signer.account_hex())
            .header("PARADEX-STARKNET-SIGNATURE", signature)
            .header("PARADEX-TIMESTAMP", timestamp_s.to_string())
            .header("PARADEX-SIGNATURE-EXPIRATION", expiry_s.to_string());
        if let Some(t) = token_usage {
            rb = rb.query(&[("token_usage", t.as_str())]);
        }
        Ok(send::<Jwt>(rb, "auth").await?.jwt_token)
    }

    /// `POST /orders`. The reply is the order as accepted (status `NEW`).
    pub async fn create_order(&self, jwt: &str, order: &OrderRequest) -> Result<ParadexOrder> {
        let rb = self.write(Method::POST, "orders", jwt).await.json(order);
        send(rb, "create order").await
    }

    /// `PUT /orders/{id}`.
    pub async fn modify_order(
        &self,
        jwt: &str,
        order: &ModifyOrderRequest,
    ) -> Result<ParadexOrder> {
        let path = format!("orders/{}", path_segment(order.id()));
        let rb = self.write(Method::PUT, &path, jwt).await.json(order);
        send(rb, "modify order").await
    }

    /// `DELETE /orders/{id}`: queues the cancel; confirmation arrives on the `orders` channel.
    pub async fn cancel_order(&self, jwt: &str, order_id: &str) -> Result<()> {
        let path = format!("orders/{}", path_segment(order_id));
        send_empty(self.write(Method::DELETE, &path, jwt).await, "cancel order").await
    }

    /// `DELETE /orders/by_client_id/{client_id}[?market=]`.
    pub async fn cancel_order_by_client_id(
        &self,
        jwt: &str,
        client_id: &str,
        market: Option<&str>,
    ) -> Result<()> {
        let path = format!("orders/by_client_id/{}", path_segment(client_id));
        let mut rb = self.write(Method::DELETE, &path, jwt).await;
        if let Some(m) = market {
            rb = rb.query(&[("market", m)]);
        }
        send_empty(rb, "cancel order by client id").await
    }

    /// `DELETE /orders[?market=]`: every open order, or those of one market.
    pub async fn cancel_all_orders(&self, jwt: &str, market: Option<&str>) -> Result<()> {
        let mut rb = self.write(Method::DELETE, "orders", jwt).await;
        if let Some(m) = market {
            rb = rb.query(&[("market", m)]);
        }
        send_empty(rb, "cancel all orders").await
    }

    /// `GET /orders[?market=]`: open orders.
    pub async fn open_orders(&self, jwt: &str, market: Option<&str>) -> Result<Vec<ParadexOrder>> {
        let mut rb = self.read("orders", jwt).await;
        if let Some(m) = market {
            rb = rb.query(&[("market", m)]);
        }
        Ok(send::<Results<ParadexOrder>>(rb, "open orders")
            .await?
            .results)
    }

    /// `GET /positions`.
    pub async fn positions(&self, jwt: &str) -> Result<Vec<ParadexPosition>> {
        let rb = self.read("positions", jwt).await;
        Ok(send::<Results<ParadexPosition>>(rb, "positions")
            .await?
            .results)
    }

    /// `GET /balance`.
    pub async fn balances(&self, jwt: &str) -> Result<Vec<ParadexBalance>> {
        let rb = self.read("balance", jwt).await;
        Ok(send::<Results<ParadexBalance>>(rb, "balances")
            .await?
            .results)
    }

    /// `GET /fills`: one page, newest first. Pass `next` back as `page.cursor` for the next.
    pub async fn fills(
        &self,
        jwt: &str,
        query: &FillsQuery<'_>,
    ) -> Result<ParadexListResponse<ParadexFill>> {
        let mut q: Vec<(&str, String)> = Vec::new();
        if let Some(m) = query.market {
            q.push(("market", m.to_string()));
        }
        if let Some(k) = query.asset_kind {
            q.push(("asset_kind", k.to_string()));
        }
        if let Some(v) = query.page.start_at {
            q.push(("start_at", v.to_string()));
        }
        if let Some(v) = query.page.end_at {
            q.push(("end_at", v.to_string()));
        }
        if let Some(v) = query.page.page_size {
            q.push(("page_size", v.to_string()));
        }
        if let Some(c) = &query.page.cursor {
            q.push(("cursor", c.clone()));
        }
        let rb = self.read("fills", jwt).await.query(&q);
        send(rb, "fills").await
    }
}

/// Percent-encode everything outside RFC 3986 unreserved characters.
fn path_segment(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

async fn send<T: DeserializeOwned>(rb: RequestBuilder, what: &str) -> Result<T> {
    let r = rb.send().await.with_context(|| format!("Paradex {what}"))?;
    let status = r.status().as_u16();
    let body = r.text().await.with_context(|| format!("Paradex {what}"))?;
    parse_body(status, &body, what)
}

async fn send_empty(rb: RequestBuilder, what: &str) -> Result<()> {
    let r = rb.send().await.with_context(|| format!("Paradex {what}"))?;
    let status = r.status().as_u16();
    if !(200..300).contains(&status) {
        let body = r.text().await.unwrap_or_default();
        bail!("Paradex {what} failed (HTTP {status}): {body}");
    }
    Ok(())
}

fn parse_body<T: DeserializeOwned>(status: u16, body: &str, what: &str) -> Result<T> {
    if !(200..300).contains(&status) {
        bail!("Paradex {what} failed (HTTP {status}): {body}");
    }
    serde_json::from_str(body)
        .with_context(|| format!("Paradex {what}: unexpected response: {body}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Response examples from the docs pages of each endpoint.
    const ORDER: &str = r#"{"account":"0x4638e3041366aa71720be63e32e53e1223316c7f0d56f7aa617542ed1e7512x","avg_fill_price":"26000","cancel_reason":"NOT_ENOUGH_MARGIN","client_id":"x1234","created_at":1681493746016,"flags":["REDUCE_ONLY"],"id":"123456","instruction":"GTC","last_updated_at":1681493746016,"market":"BTC-USD-PERP","price":"26000","published_at":1681493746016,"received_at":1681493746016,"remaining_size":"0","request_info":{"id":"string","message":"string","request_type":"string","status":"string"},"seq_no":1681471234972000000,"side":"BUY","size":"0.05","status":"NEW","stp":"EXPIRE_MAKER","timestamp":1681493746016,"trigger_price":"26000","type":"MARKET"}"#;

    #[test]
    fn parses_doc_examples() {
        let o: ParadexOrder = parse_body(201, ORDER, "create order").unwrap();
        assert_eq!(o.order_type, OrderType::Market);
        assert_eq!(o.flags, vec![OrderFlag::ReduceOnly]);
        assert_eq!(o.seq_no, 1681471234972000000);

        let open: Results<ParadexOrder> =
            parse_body(200, &format!("{{\"results\":[{ORDER}]}}"), "open orders").unwrap();
        assert_eq!(open.results.len(), 1);

        let p: Results<ParadexPosition> = parse_body(200, r#"{"results":[{"account":"string","average_entry_price":"29001.34","average_entry_price_usd":"29001.34","average_exit_price":"29001.34","cached_funding_index":"1234.3","closed_at":1681493939981,"cost":"-10005.4623","cost_usd":"-10005.4623","created_at":1681493939981,"id":"1234234","last_fill_id":"1234234","last_updated_at":1681493939981,"leverage":"string","liquidation_price":"string","market":"BTC-USD-PERP","realized_positional_funding_pnl":"12.234","realized_positional_pnl":"-123.23","seq_no":1681471234972000000,"side":"SHORT","size":"-0.345","status":"OPEN","unrealized_funding_pnl":"12.234","unrealized_pnl":"-123.23"}]}"#, "positions").unwrap();
        assert_eq!(p.results[0].side, PositionSide::Short);

        let b: Results<ParadexBalance> = parse_body(200, r#"{"results":[{"average_entry_price":"2901.34","average_entry_price_usd":"2902.18","cost":"105.4623","cost_usd":"105.8311","last_updated_at":1681462770114,"size":"123003.620","token":"USDC"}]}"#, "balances").unwrap();
        assert_eq!(b.results[0].token, "USDC");

        let f: ParadexListResponse<ParadexFill> = parse_body(200, r#"{"next":"eyJmaWx0ZXIiMsIm1hcmtlciI6eyJtYXJrZXIiOiIxNjc1NjUwMDE3NDMxMTAxNjk5N=","prev":"eyJmaWx0ZXIiOnsiTGltaXQiOjkwfSwidGltZSI6MTY4MTY3OTgzNzk3MTMwOTk1MywibWFya2VyIjp7Im1zMjExMD==","results":[{"account":"0x978532f46745d7fFBa1CDfa1D8C8202D09D89C9E","client_id":"x1234","created_at":1681375176910,"fee":"7.56","fee_currency":"USDC","fill_type":"FILL","flags":["interactive","interactive"],"id":"8615262148007718462","leverage":"20","liquidity":"TAKER","market":"BTC-USD-PERP","order_id":"1681462103821101699438490000","orderbook_seq_no":12345,"price":"30000.12","realized_funding":"7.56","realized_pnl":"7.56","remaining_size":"0.5","side":"BUY","size":"0.5","underlying_price":"7.56"}]}"#, "fills").unwrap();
        assert_eq!(f.results[0].fill_type, FillType::Fill);
        assert_eq!(f.results[0].leverage.as_deref(), Some("20"));

        let jwt: Jwt = parse_body(200, r#"{"jwt_token":"eyJ"}"#, "auth").unwrap();
        assert_eq!(jwt.jwt_token, "eyJ");
    }

    #[test]
    fn errors_and_missing_fields() {
        let err = parse_body::<Jwt>(
            401,
            r#"{"error":"NOT_ONBOARDED","message":"User has never called /onboarding endpoint"}"#,
            "auth",
        )
        .unwrap_err();
        assert!(err.to_string().contains("NOT_ONBOARDED"));
        let without_stp = ORDER.replace(r#""stp":"EXPIRE_MAKER","#, "");
        assert!(parse_body::<ParadexOrder>(200, &without_stp, "order").is_err());
    }

    #[test]
    fn path_segments_are_escaped() {
        assert_eq!(path_segment("abc-1_2.3~"), "abc-1_2.3~");
        assert_eq!(path_segment("a/b c"), "a%2Fb%20c");
    }
}
