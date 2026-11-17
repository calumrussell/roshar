//! Signed-endpoint request builders.
//!
//! Each builder returns the exact method, path (with query) and body that is both signed and sent.
//! Field order and encoding follow the official Python SDK, so the signatures match its vectors
//! (`tests/vectors.rs`). Prices and sizes are decimal text written verbatim as JSON numbers.

use crate::wire::{path_segment, py_array, PathQuery, PyObject};
use anyhow::{bail, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
    Put,
    Delete,
}

impl Method {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Delete => "DELETE",
        }
    }

    /// `Content-Type` Orderly expects for the method.
    pub fn content_type(self) -> &'static str {
        match self {
            Self::Get | Self::Delete => "application/x-www-form-urlencoded",
            Self::Post | Self::Put => "application/json",
        }
    }
}

/// A request ready to sign: `path` includes the query string, `body` is the exact JSON text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedRequest {
    pub method: Method,
    pub path: String,
    pub body: Option<String>,
}

/// `order_type` of `POST /v1/order`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderlyOrderType {
    Limit,
    Market,
    /// Fill what crosses at `order_price`, cancel the rest.
    Ioc,
    /// Fill entirely at `order_price` or cancel.
    Fok,
    /// Cancelled if it would take liquidity.
    PostOnly,
    /// Priced at the best ask when accepted (`level` picks ask0..ask4).
    Ask,
    /// Priced at the best bid when accepted (`level` picks bid0..bid4).
    Bid,
}

impl OrderlyOrderType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Limit => "LIMIT",
            Self::Market => "MARKET",
            Self::Ioc => "IOC",
            Self::Fok => "FOK",
            Self::PostOnly => "POST_ONLY",
            Self::Ask => "ASK",
            Self::Bid => "BID",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderlySide {
    Buy,
    Sell,
}

impl OrderlySide {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Buy => "BUY",
            Self::Sell => "SELL",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderlyMarginMode {
    Cross,
    Isolated,
}

impl OrderlyMarginMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Cross => "CROSS",
            Self::Isolated => "ISOLATED",
        }
    }
}

/// `POST /v1/order` (and one item of `POST /v1/batch-order`). Unset fields are omitted and take
/// the exchange default.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderlyOrderRequest {
    pub symbol: String,
    pub order_type: OrderlyOrderType,
    pub side: OrderlySide,
    /// Up to 36 characters, unique among the account's open orders; must not start with `-`.
    pub client_order_id: Option<String>,
    /// Decimal text. Required except for `MARKET`/`ASK`/`BID`.
    pub order_price: Option<String>,
    /// Decimal text, base units.
    pub order_quantity: Option<String>,
    /// Decimal text, quote units; `MARKET`/`ASK`/`BID` only, instead of `order_quantity`.
    pub order_amount: Option<String>,
    pub reduce_only: Option<bool>,
    /// Decimal text; `0` hides the order. Not for `MARKET`/`IOC`/`FOK`.
    pub visible_quantity: Option<String>,
    /// Referral code or `enum:<broker_id>:<enum_id>`; fixed once placed.
    pub order_tag: Option<String>,
    /// Decimal text; `MARKET` only: fills beyond this slippage are not executed.
    pub slippage: Option<String>,
    /// `0..=4`; `ASK`/`BID` only: price at level N of the book.
    pub level: Option<u8>,
    /// `POST_ONLY` only: re-price one tick inside the best price instead of rejecting.
    pub post_only_adjust: Option<bool>,
    pub margin_mode: Option<OrderlyMarginMode>,
}

impl OrderlyOrderRequest {
    /// An order with only the required fields set.
    pub fn new(symbol: impl Into<String>, order_type: OrderlyOrderType, side: OrderlySide) -> Self {
        Self {
            symbol: symbol.into(),
            order_type,
            side,
            client_order_id: None,
            order_price: None,
            order_quantity: None,
            order_amount: None,
            reduce_only: None,
            visible_quantity: None,
            order_tag: None,
            slippage: None,
            level: None,
            post_only_adjust: None,
            margin_mode: None,
        }
    }

    /// Body text, keys in the SDK's `create_order` order, then the documented fields the SDK does
    /// not take.
    fn json(&self) -> Result<String> {
        let mut o = PyObject::new();
        o.str("symbol", &self.symbol)
            .str("order_type", self.order_type.as_str())
            .str("side", self.side.as_str())
            .opt_str("client_order_id", self.client_order_id.as_deref());
        o.opt_number("order_price", self.order_price.as_deref())?
            .opt_number("order_quantity", self.order_quantity.as_deref())?
            .opt_number("order_amount", self.order_amount.as_deref())?;
        o.opt_bool("reduce_only", self.reduce_only);
        o.opt_number("visible_quantity", self.visible_quantity.as_deref())?;
        o.opt_str("order_tag", self.order_tag.as_deref());
        o.opt_number("slippage", self.slippage.as_deref())?;
        if let Some(level) = self.level {
            o.uint("level", u64::from(level));
        }
        o.opt_bool("post_only_adjust", self.post_only_adjust)
            .opt_str(
                "margin_mode",
                self.margin_mode.map(OrderlyMarginMode::as_str),
            );
        Ok(o.finish())
    }
}

/// `PUT /v1/order`. Only price and quantity can change; the other fields must repeat the order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderlyEditOrderRequest {
    pub order_id: u64,
    pub symbol: String,
    pub order_type: OrderlyOrderType,
    pub side: OrderlySide,
    pub client_order_id: Option<String>,
    pub order_price: Option<String>,
    pub order_quantity: Option<String>,
    pub order_amount: Option<String>,
    pub reduce_only: Option<bool>,
    pub visible_quantity: Option<String>,
}

impl OrderlyEditOrderRequest {
    /// Body text in the SDK's `edit_order` key order. `order_id` is a JSON string, as documented.
    fn json(&self) -> Result<String> {
        let mut o = PyObject::new();
        o.str("order_id", &self.order_id.to_string());
        o.opt_number("order_price", self.order_price.as_deref())?
            .opt_number("order_quantity", self.order_quantity.as_deref())?;
        o.str("order_type", self.order_type.as_str())
            .str("side", self.side.as_str())
            .str("symbol", &self.symbol)
            .opt_str("client_order_id", self.client_order_id.as_deref());
        o.opt_number("order_amount", self.order_amount.as_deref())?;
        o.opt_bool("reduce_only", self.reduce_only);
        o.opt_number("visible_quantity", self.visible_quantity.as_deref())?;
        Ok(o.finish())
    }
}

/// Filters of `GET /v1/orders`, sent as given. Unset filters are omitted.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OrderlyOrdersQuery<'a> {
    pub symbol: Option<&'a str>,
    /// `LIMIT` / `MARKET`.
    pub order_type: Option<&'a str>,
    /// `BUY` / `SELL`.
    pub side: Option<&'a str>,
    /// `NEW`, `CANCELLED`, `PARTIAL_FILLED`, `FILLED`, `REJECTED`, `INCOMPLETE` (open) or
    /// `COMPLETED`.
    pub status: Option<&'a str>,
    /// Unix ms.
    pub start_t: Option<u64>,
    /// Unix ms.
    pub end_t: Option<u64>,
    /// From 1.
    pub page: Option<u32>,
    /// Max 500.
    pub size: Option<u32>,
    pub order_tag: Option<&'a str>,
    /// `CREATED_TIME_DESC`, `CREATED_TIME_ASC`, `UPDATED_TIME_DESC` or `UPDATED_TIME_ASC`.
    pub sort_by: Option<&'a str>,
    /// `PERP` or `BINARY`.
    pub product_type: Option<&'a str>,
}

/// Filters of `GET /v1/trades`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OrderlyTradesQuery<'a> {
    pub symbol: Option<&'a str>,
    pub start_t: Option<u64>,
    pub end_t: Option<u64>,
    pub page: Option<u32>,
    pub size: Option<u32>,
    /// `PERP` or `BINARY`.
    pub product_type: Option<&'a str>,
}

/// Filters of `GET /v1/funding_fee/history`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OrderlyFundingFeeQuery<'a> {
    /// Unset returns all symbols.
    pub symbol: Option<&'a str>,
    pub start_t: Option<u64>,
    pub end_t: Option<u64>,
    pub page: Option<u32>,
    pub size: Option<u32>,
}

fn join<T: std::fmt::Display>(field: &str, items: &[T]) -> Result<String> {
    if items.is_empty() {
        bail!("{field}: at least one id is required");
    }
    let mut out = String::new();
    for (i, item) in items.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&item.to_string());
    }
    Ok(out)
}

impl PreparedRequest {
    fn get(path: String) -> Self {
        Self {
            method: Method::Get,
            path,
            body: None,
        }
    }

    fn delete(path: String) -> Self {
        Self {
            method: Method::Delete,
            path,
            body: None,
        }
    }

    fn post(path: &str, body: String) -> Self {
        Self {
            method: Method::Post,
            path: path.to_string(),
            body: Some(body),
        }
    }

    /// `POST /v1/order`
    pub fn create_order(order: &OrderlyOrderRequest) -> Result<Self> {
        Ok(Self::post("/v1/order", order.json()?))
    }

    /// `POST /v1/batch-order` — `{"orders": [...]}`; the exchange accepts at most 10.
    pub fn batch_create_orders(orders: &[OrderlyOrderRequest]) -> Result<Self> {
        if orders.is_empty() {
            bail!("orders: at least one order is required");
        }
        let items = orders
            .iter()
            .map(OrderlyOrderRequest::json)
            .collect::<Result<Vec<_>>>()?;
        let mut o = PyObject::new();
        o.raw("orders", &py_array(&items));
        Ok(Self::post("/v1/batch-order", o.finish()))
    }

    /// `PUT /v1/order`
    pub fn edit_order(edit: &OrderlyEditOrderRequest) -> Result<Self> {
        Ok(Self {
            method: Method::Put,
            path: "/v1/order".to_string(),
            body: Some(edit.json()?),
        })
    }

    /// `DELETE /v1/order?order_id=..&symbol=..`
    pub fn cancel_order(order_id: u64, symbol: &str) -> Result<Self> {
        let path = PathQuery::new("/v1/order")
            .param("order_id", order_id)?
            .param("symbol", symbol)?
            .finish();
        Ok(Self::delete(path))
    }

    /// `DELETE /v1/client/order?client_order_id=..&symbol=..`
    pub fn cancel_order_by_client_order_id(client_order_id: &str, symbol: &str) -> Result<Self> {
        let path = PathQuery::new("/v1/client/order")
            .param("client_order_id", client_order_id)?
            .param("symbol", symbol)?
            .finish();
        Ok(Self::delete(path))
    }

    /// `DELETE /v1/orders[?symbol=..]` — every open order, or every open order on `symbol`.
    pub fn cancel_all_orders(symbol: Option<&str>) -> Result<Self> {
        let path = PathQuery::new("/v1/orders").opt("symbol", symbol)?.finish();
        Ok(Self::delete(path))
    }

    /// `DELETE /v1/batch-order?order_ids=a,b,..`; the exchange accepts at most 10.
    pub fn batch_cancel_orders(order_ids: &[u64]) -> Result<Self> {
        let path = PathQuery::new("/v1/batch-order")
            .param("order_ids", join("order_ids", order_ids)?)?
            .finish();
        Ok(Self::delete(path))
    }

    /// `DELETE /v1/client/batch-order?client_order_ids=a,b,..`; at most 10.
    pub fn batch_cancel_orders_by_client_order_id(client_order_ids: &[&str]) -> Result<Self> {
        let path = PathQuery::new("/v1/client/batch-order")
            .param(
                "client_order_ids",
                join("client_order_ids", client_order_ids)?,
            )?
            .finish();
        Ok(Self::delete(path))
    }

    /// `POST /v1/order/cancel_all_after` — dead man's switch. `trigger_after_ms` is 5 000..=900 000,
    /// or 0 to clear the timer. The exchange enables this endpoint per account on request.
    pub fn cancel_all_after(trigger_after_ms: u64) -> Result<Self> {
        let mut o = PyObject::new();
        o.uint("trigger_after", trigger_after_ms);
        Ok(Self::post("/v1/order/cancel_all_after", o.finish()))
    }

    /// `GET /v1/orders`
    pub fn get_orders(q: &OrderlyOrdersQuery<'_>) -> Result<Self> {
        let path = PathQuery::new("/v1/orders")
            .opt("symbol", q.symbol)?
            .opt("order_type", q.order_type)?
            .opt("side", q.side)?
            .opt("status", q.status)?
            .opt("start_t", q.start_t)?
            .opt("end_t", q.end_t)?
            .opt("page", q.page)?
            .opt("size", q.size)?
            .opt("order_tag", q.order_tag)?
            .opt("sort_by", q.sort_by)?
            .opt("product_type", q.product_type)?
            .finish();
        Ok(Self::get(path))
    }

    /// `GET /v1/order/{order_id}`
    pub fn get_order(order_id: u64) -> Result<Self> {
        Ok(Self::get(format!("/v1/order/{order_id}")))
    }

    /// `GET /v1/client/order/{client_order_id}`
    pub fn get_order_by_client_order_id(client_order_id: &str) -> Result<Self> {
        path_segment("client_order_id", client_order_id)?;
        Ok(Self::get(format!("/v1/client/order/{client_order_id}")))
    }

    /// `GET /v1/order/{order_id}/trades`
    pub fn get_order_trades(order_id: u64) -> Result<Self> {
        Ok(Self::get(format!("/v1/order/{order_id}/trades")))
    }

    /// `GET /v1/trades`
    pub fn get_trades(q: &OrderlyTradesQuery<'_>) -> Result<Self> {
        let path = PathQuery::new("/v1/trades")
            .opt("symbol", q.symbol)?
            .opt("start_t", q.start_t)?
            .opt("end_t", q.end_t)?
            .opt("page", q.page)?
            .opt("size", q.size)?
            .opt("product_type", q.product_type)?
            .finish();
        Ok(Self::get(path))
    }

    /// `GET /v1/trade/{trade_id}`
    pub fn get_trade(trade_id: u64) -> Result<Self> {
        Ok(Self::get(format!("/v1/trade/{trade_id}")))
    }

    /// `GET /v1/positions[?product_type=..]`
    pub fn get_positions(product_type: Option<&str>) -> Result<Self> {
        let path = PathQuery::new("/v1/positions")
            .opt("product_type", product_type)?
            .finish();
        Ok(Self::get(path))
    }

    /// `GET /v1/position/{symbol}`
    pub fn get_position(symbol: &str) -> Result<Self> {
        path_segment("symbol", symbol)?;
        Ok(Self::get(format!("/v1/position/{symbol}")))
    }

    /// `GET /v1/client/holding[?all=true|false]` — `all` includes zero balances.
    pub fn get_holding(all: Option<bool>) -> Result<Self> {
        let path = PathQuery::new("/v1/client/holding")
            .opt("all", all)?
            .finish();
        Ok(Self::get(path))
    }

    /// `GET /v1/client/info`
    pub fn get_client_info() -> Result<Self> {
        Ok(Self::get("/v1/client/info".to_string()))
    }

    /// `GET /v1/account_info`
    pub fn get_account_info() -> Result<Self> {
        Ok(Self::get("/v1/account_info".to_string()))
    }

    /// `GET /v1/funding_fee/history`
    pub fn get_funding_fee_history(q: &OrderlyFundingFeeQuery<'_>) -> Result<Self> {
        let path = PathQuery::new("/v1/funding_fee/history")
            .opt("symbol", q.symbol)?
            .opt("start_t", q.start_t)?
            .opt("end_t", q.end_t)?
            .opt("page", q.page)?
            .opt("size", q.size)?
            .finish();
        Ok(Self::get(path))
    }

    /// `GET /v1/client/key_info[?key_status=ACTIVE|REMOVING|REMOVED]`
    pub fn get_key_info(key_status: Option<&str>) -> Result<Self> {
        let path = PathQuery::new("/v1/client/key_info")
            .opt("key_status", key_status)?
            .finish();
        Ok(Self::get(path))
    }

    /// `GET /v1/orderbook/{symbol}[?max_level=..]` — needs an Orderly key; unset `max_level` is
    /// the exchange default (100).
    pub fn get_orderbook(symbol: &str, max_level: Option<u32>) -> Result<Self> {
        path_segment("symbol", symbol)?;
        let path = PathQuery::new(format!("/v1/orderbook/{symbol}"))
            .opt("max_level", max_level)?
            .finish();
        Ok(Self::get(path))
    }

    /// `GET /v1/kline?symbol=..&type=..[&limit=..]` — needs an Orderly key. `kline_type` is
    /// `1m`, `5m`, `15m`, `30m`, `1h`, `4h`, `12h`, `1d`, `1w`, `1mon` or `1y`; `limit` max 1000.
    pub fn get_kline(symbol: &str, kline_type: &str, limit: Option<u32>) -> Result<Self> {
        let path = PathQuery::new("/v1/kline")
            .param("symbol", symbol)?
            .param("type", kline_type)?
            .opt("limit", limit)?
            .finish();
        Ok(Self::get(path))
    }
}
