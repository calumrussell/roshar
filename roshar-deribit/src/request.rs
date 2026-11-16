//! JSON-RPC method calls shared by REST (`GET /api/v2/{method}?{params}`) and the websocket
//! (`{"jsonrpc":"2.0","method":...,"params":...,"id":...}`). Every optional parameter is an
//! `Option` and is omitted when `None`.

use serde_json::{Map, Value};

/// One public JSON-RPC call: method name plus named parameters.
#[derive(Debug, Clone, PartialEq)]
pub struct DeribitRequest {
    pub method: &'static str,
    pub params: Map<String, Value>,
}

/// Optional time/size/order filters shared by the `get_last_trades_by_*` methods.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DeribitTradeFilter<'a> {
    /// Milliseconds since the UNIX epoch.
    pub start_timestamp: Option<u64>,
    /// Milliseconds since the UNIX epoch.
    pub end_timestamp: Option<u64>,
    /// 1..=1000 (server default 10).
    pub count: Option<u32>,
    /// `asc`, `desc` or `default`.
    pub sorting: Option<&'a str>,
}

/// Optional filters shared by the `get_last_settlements_by_*` methods.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DeribitSettlementFilter<'a> {
    /// `settlement`, `delivery` or `bankruptcy`.
    pub settlement_type: Option<&'a str>,
    /// 1..=1000 (server default 20).
    pub count: Option<u32>,
    /// `continuation` from the previous page.
    pub continuation: Option<&'a str>,
    /// Milliseconds since the UNIX epoch; the search walks back from here.
    pub search_start_timestamp: Option<u64>,
}

impl DeribitRequest {
    fn new(method: &'static str) -> Self {
        Self {
            method,
            params: Map::new(),
        }
    }

    fn with(mut self, key: &str, value: impl Into<Value>) -> Self {
        self.params.insert(key.to_string(), value.into());
        self
    }

    fn with_opt<V: Into<Value>>(self, key: &str, value: Option<V>) -> Self {
        match value {
            Some(v) => self.with(key, v),
            None => self,
        }
    }

    fn with_trade_filter(self, filter: DeribitTradeFilter<'_>) -> Self {
        self.with_opt("start_timestamp", filter.start_timestamp)
            .with_opt("end_timestamp", filter.end_timestamp)
            .with_opt("count", filter.count)
            .with_opt("sorting", filter.sorting)
    }

    fn with_settlement_filter(self, filter: DeribitSettlementFilter<'_>) -> Self {
        self.with_opt("type", filter.settlement_type)
            .with_opt("count", filter.count)
            .with_opt("continuation", filter.continuation)
            .with_opt("search_start_timestamp", filter.search_start_timestamp)
    }

    /// Query string for the REST transport (`k=v&...`, values percent-encoded).
    /// Array parameters (only `channels`, which is websocket-only) are not representable.
    pub fn query_string(&self) -> String {
        let mut out = String::new();
        for (key, value) in &self.params {
            if !out.is_empty() {
                out.push('&');
            }
            out.push_str(key);
            out.push('=');
            match value {
                Value::String(s) => percent_encode(s, &mut out),
                other => percent_encode(&other.to_string(), &mut out),
            }
        }
        out
    }

    // ---- Market data ----

    /// `public/get_instruments`. `currency` may be `any`; `kind` is `future`, `option`, `spot`,
    /// `future_combo`, `option_combo` or `any`.
    pub fn get_instruments(
        currency: &str,
        kind: Option<&str>,
        expired: Option<bool>,
        extended: Option<bool>,
    ) -> Self {
        Self::new("public/get_instruments")
            .with("currency", currency)
            .with_opt("kind", kind)
            .with_opt("expired", expired)
            .with_opt("extended", extended)
    }

    /// `public/get_instrument`.
    pub fn get_instrument(instrument_name: &str) -> Self {
        Self::new("public/get_instrument").with("instrument_name", instrument_name)
    }

    /// `public/get_currencies`.
    pub fn get_currencies() -> Self {
        Self::new("public/get_currencies")
    }

    /// `public/get_book_summary_by_currency`.
    pub fn get_book_summary_by_currency(currency: &str, kind: Option<&str>) -> Self {
        Self::new("public/get_book_summary_by_currency")
            .with("currency", currency)
            .with_opt("kind", kind)
    }

    /// `public/get_book_summary_by_instrument`.
    pub fn get_book_summary_by_instrument(instrument_name: &str) -> Self {
        Self::new("public/get_book_summary_by_instrument").with("instrument_name", instrument_name)
    }

    /// `public/get_order_book`. `depth` is one of 1, 5, 10, 20, 50, 100, 1000, 10000.
    pub fn get_order_book(instrument_name: &str, depth: Option<u32>) -> Self {
        Self::new("public/get_order_book")
            .with("instrument_name", instrument_name)
            .with_opt("depth", depth)
    }

    /// `public/get_order_book_by_instrument_id`. `depth` as for `get_order_book`.
    pub fn get_order_book_by_instrument_id(instrument_id: u64, depth: Option<u32>) -> Self {
        Self::new("public/get_order_book_by_instrument_id")
            .with("instrument_id", instrument_id)
            .with_opt("depth", depth)
    }

    /// `public/ticker`.
    pub fn ticker(instrument_name: &str) -> Self {
        Self::new("public/ticker").with("instrument_name", instrument_name)
    }

    /// `public/get_last_trades_by_instrument`, optionally bounded by trade sequence numbers.
    pub fn get_last_trades_by_instrument(
        instrument_name: &str,
        start_seq: Option<u64>,
        end_seq: Option<u64>,
        filter: DeribitTradeFilter<'_>,
    ) -> Self {
        Self::new("public/get_last_trades_by_instrument")
            .with("instrument_name", instrument_name)
            .with_opt("start_seq", start_seq)
            .with_opt("end_seq", end_seq)
            .with_trade_filter(filter)
    }

    /// `public/get_last_trades_by_instrument_and_time`.
    pub fn get_last_trades_by_instrument_and_time(
        instrument_name: &str,
        filter: DeribitTradeFilter<'_>,
    ) -> Self {
        Self::new("public/get_last_trades_by_instrument_and_time")
            .with("instrument_name", instrument_name)
            .with_trade_filter(filter)
    }

    /// `public/get_last_trades_by_currency`, optionally bounded by trade ids. `kind` also
    /// accepts `combo` and `any`. Without a `kind`, currencies with Coinbase-routed spot
    /// return error 11060.
    pub fn get_last_trades_by_currency(
        currency: &str,
        kind: Option<&str>,
        start_id: Option<&str>,
        end_id: Option<&str>,
        filter: DeribitTradeFilter<'_>,
    ) -> Self {
        Self::new("public/get_last_trades_by_currency")
            .with("currency", currency)
            .with_opt("kind", kind)
            .with_opt("start_id", start_id)
            .with_opt("end_id", end_id)
            .with_trade_filter(filter)
    }

    /// `public/get_last_trades_by_currency_and_time`.
    pub fn get_last_trades_by_currency_and_time(
        currency: &str,
        kind: Option<&str>,
        filter: DeribitTradeFilter<'_>,
    ) -> Self {
        Self::new("public/get_last_trades_by_currency_and_time")
            .with("currency", currency)
            .with_opt("kind", kind)
            .with_trade_filter(filter)
    }

    /// `public/get_tradingview_chart_data`. `resolution` is minutes (`1`, `3`, `5`, `10`,
    /// `15`, `30`, `60`, `120`, `180`, `360`, `720`) or `1D`.
    pub fn get_tradingview_chart_data(
        instrument_name: &str,
        start_timestamp: u64,
        end_timestamp: u64,
        resolution: &str,
    ) -> Self {
        Self::new("public/get_tradingview_chart_data")
            .with("instrument_name", instrument_name)
            .with("start_timestamp", start_timestamp)
            .with("end_timestamp", end_timestamp)
            .with("resolution", resolution)
    }

    /// `public/get_funding_rate_history` (hourly records).
    pub fn get_funding_rate_history(
        instrument_name: &str,
        start_timestamp: u64,
        end_timestamp: u64,
    ) -> Self {
        Self::new("public/get_funding_rate_history")
            .with("instrument_name", instrument_name)
            .with("start_timestamp", start_timestamp)
            .with("end_timestamp", end_timestamp)
    }

    /// `public/get_funding_rate_value`: accumulated funding over the period.
    pub fn get_funding_rate_value(
        instrument_name: &str,
        start_timestamp: u64,
        end_timestamp: u64,
    ) -> Self {
        Self::new("public/get_funding_rate_value")
            .with("instrument_name", instrument_name)
            .with("start_timestamp", start_timestamp)
            .with("end_timestamp", end_timestamp)
    }

    /// `public/get_funding_chart_data`. `length` is `8h`, `24h` or `1m`.
    pub fn get_funding_chart_data(instrument_name: &str, length: &str) -> Self {
        Self::new("public/get_funding_chart_data")
            .with("instrument_name", instrument_name)
            .with("length", length)
    }

    /// `public/get_historical_volatility`.
    pub fn get_historical_volatility(currency: &str) -> Self {
        Self::new("public/get_historical_volatility").with("currency", currency)
    }

    /// `public/get_volatility_index_data`. `resolution` is seconds (`1`, `60`, `3600`,
    /// `43200`) or `1D`.
    pub fn get_volatility_index_data(
        currency: &str,
        start_timestamp: u64,
        end_timestamp: u64,
        resolution: &str,
    ) -> Self {
        Self::new("public/get_volatility_index_data")
            .with("currency", currency)
            .with("start_timestamp", start_timestamp)
            .with("end_timestamp", end_timestamp)
            .with("resolution", resolution)
    }

    /// `public/get_index_price`.
    pub fn get_index_price(index_name: &str) -> Self {
        Self::new("public/get_index_price").with("index_name", index_name)
    }

    /// `public/get_index_price_names`. `extended: Some(true)` changes the result from names
    /// to objects (`DeribitIndexPriceName`).
    pub fn get_index_price_names(extended: Option<bool>) -> Self {
        Self::new("public/get_index_price_names").with_opt("extended", extended)
    }

    /// `public/get_index_chart_data`. `range` is `1h`, `1d`, `2d`, `1m`, `1y` or `all`.
    pub fn get_index_chart_data(index_name: &str, range: &str) -> Self {
        Self::new("public/get_index_chart_data")
            .with("index_name", index_name)
            .with("range", range)
    }

    /// `public/get_basket_index_composition` (currently only `COIN50`).
    pub fn get_basket_index_composition(index_name: &str) -> Self {
        Self::new("public/get_basket_index_composition").with("index_name", index_name)
    }

    /// `public/get_delivery_prices`.
    pub fn get_delivery_prices(
        index_name: &str,
        offset: Option<u32>,
        count: Option<u32>,
        start_timestamp: Option<u64>,
        end_timestamp: Option<u64>,
    ) -> Self {
        Self::new("public/get_delivery_prices")
            .with("index_name", index_name)
            .with_opt("offset", offset)
            .with_opt("count", count)
            .with_opt("start_timestamp", start_timestamp)
            .with_opt("end_timestamp", end_timestamp)
    }

    /// `public/get_last_settlements_by_currency`.
    pub fn get_last_settlements_by_currency(
        currency: &str,
        filter: DeribitSettlementFilter<'_>,
    ) -> Self {
        Self::new("public/get_last_settlements_by_currency")
            .with("currency", currency)
            .with_settlement_filter(filter)
    }

    /// `public/get_last_settlements_by_instrument`.
    pub fn get_last_settlements_by_instrument(
        instrument_name: &str,
        filter: DeribitSettlementFilter<'_>,
    ) -> Self {
        Self::new("public/get_last_settlements_by_instrument")
            .with("instrument_name", instrument_name)
            .with_settlement_filter(filter)
    }

    /// `public/get_contract_size`.
    pub fn get_contract_size(instrument_name: &str) -> Self {
        Self::new("public/get_contract_size").with("instrument_name", instrument_name)
    }

    /// `public/get_mark_price_history` (5-minute marks; only options used by the volatility
    /// index return data, everything else returns `[]`).
    pub fn get_mark_price_history(
        instrument_name: &str,
        start_timestamp: u64,
        end_timestamp: u64,
    ) -> Self {
        Self::new("public/get_mark_price_history")
            .with("instrument_name", instrument_name)
            .with("start_timestamp", start_timestamp)
            .with("end_timestamp", end_timestamp)
    }

    /// `public/get_supported_index_names`. `index_type` is `all`, `spot` or `derivative`.
    pub fn get_supported_index_names(index_type: Option<&str>) -> Self {
        Self::new("public/get_supported_index_names").with_opt("type", index_type)
    }

    /// `public/get_expirations`. `kind` is `future`, `option` or `any`.
    pub fn get_expirations(
        currency: Option<&str>,
        kind: Option<&str>,
        currency_pair: Option<&str>,
    ) -> Self {
        Self::new("public/get_expirations")
            .with_opt("currency", currency)
            .with_opt("kind", kind)
            .with_opt("currency_pair", currency_pair)
    }

    /// `public/get_apr_history`. `currency` is `usde`, `steth`, `usdc` or `build`; `before`
    /// is the `continuation` day from the previous page.
    pub fn get_apr_history(currency: &str, limit: Option<u32>, before: Option<u64>) -> Self {
        Self::new("public/get_apr_history")
            .with("currency", currency)
            .with_opt("limit", limit)
            .with_opt("before", before)
    }

    /// `public/get_trade_volumes`. `extended: Some(true)` adds 7- and 30-day volumes.
    pub fn get_trade_volumes(extended: Option<bool>) -> Self {
        Self::new("public/get_trade_volumes").with_opt("extended", extended)
    }

    /// `public/get_combos`. `currency` may be `any`.
    pub fn get_combos(currency: &str) -> Self {
        Self::new("public/get_combos").with("currency", currency)
    }

    /// `public/get_combo_ids`. `state` is `active` or `inactive`.
    pub fn get_combo_ids(currency: &str, state: Option<&str>) -> Self {
        Self::new("public/get_combo_ids")
            .with("currency", currency)
            .with_opt("state", state)
    }

    /// `public/get_combo_details`.
    pub fn get_combo_details(combo_id: &str) -> Self {
        Self::new("public/get_combo_details").with("combo_id", combo_id)
    }

    /// `public/get_block_rfq_trades`. `count` is 10..=50; `currency` may be `any`.
    pub fn get_block_rfq_trades(
        currency: &str,
        continuation: Option<&str>,
        count: Option<u32>,
    ) -> Self {
        Self::new("public/get_block_rfq_trades")
            .with("currency", currency)
            .with_opt("continuation", continuation)
            .with_opt("count", count)
    }

    // ---- Supporting ----

    /// `public/status`.
    pub fn status() -> Self {
        Self::new("public/status")
    }

    /// `public/get_time`.
    pub fn get_time() -> Self {
        Self::new("public/get_time")
    }

    /// `public/test`. `expected_result: Some("exception")` makes the server return an error.
    /// Also the required reply to a heartbeat `test_request`.
    pub fn test(expected_result: Option<&str>) -> Self {
        Self::new("public/test").with_opt("expected_result", expected_result)
    }

    // ---- Websocket only (REST returns error 10030 `must_be_websocket_request`) ----

    /// `public/hello`: introduce the client; the result is the API version.
    pub fn hello(client_name: &str, client_version: &str) -> Self {
        Self::new("public/hello")
            .with("client_name", client_name)
            .with("client_version", client_version)
    }

    /// `public/subscribe`.
    pub fn subscribe(channels: Vec<String>) -> Self {
        Self::new("public/subscribe").with("channels", channels)
    }

    /// `public/unsubscribe`.
    pub fn unsubscribe(channels: Vec<String>) -> Self {
        Self::new("public/unsubscribe").with("channels", channels)
    }

    /// `public/unsubscribe_all`.
    pub fn unsubscribe_all() -> Self {
        Self::new("public/unsubscribe_all")
    }

    /// `public/set_heartbeat`. `interval` must be 10..=300 seconds (else `-32602`).
    pub fn set_heartbeat(interval: u32) -> Self {
        Self::new("public/set_heartbeat").with("interval", interval)
    }

    /// `public/disable_heartbeat`.
    pub fn disable_heartbeat() -> Self {
        Self::new("public/disable_heartbeat")
    }
}

/// Percent-encodes everything but RFC 3986 unreserved characters.
fn percent_encode(s: &str, out: &mut String) {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push('%');
            out.push(HEX[(b >> 4) as usize] as char);
            out.push(HEX[(b & 0xF) as usize] as char);
        }
    }
}

/// Public websocket channels. `interval` is `100ms`, `agg2` or `raw` (`raw` needs an
/// authenticated connection; unauthenticated subscriptions fail with error 13778).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeribitChannel<'a> {
    /// `book.{instrument_name}.{interval}`: snapshot then incremental changes.
    Book {
        instrument_name: &'a str,
        interval: &'a str,
    },
    /// `book.{instrument_name}.{group}.{depth}.{interval}`: full grouped top-of-book each time.
    /// `group` is `none` or a price grouping (e.g. `1`, `2`, `5`, `10`, `25`, `100`, `250`);
    /// `depth` is 1, 10 or 20; `interval` is `100ms` or `agg2`.
    GroupedBook {
        instrument_name: &'a str,
        group: &'a str,
        depth: u32,
        interval: &'a str,
    },
    /// `ticker.{instrument_name}.{interval}`.
    Ticker {
        instrument_name: &'a str,
        interval: &'a str,
    },
    /// `incremental_ticker.{instrument_name}`: snapshot then changed fields only.
    IncrementalTicker { instrument_name: &'a str },
    /// `trades.{instrument_name}.{interval}`.
    Trades {
        instrument_name: &'a str,
        interval: &'a str,
    },
    /// `trades.{kind}.{currency}.{interval}`. `kind` and `currency` accept `any`.
    TradesByKind {
        kind: &'a str,
        currency: &'a str,
        interval: &'a str,
    },
    /// `chart.trades.{instrument_name}.{resolution}`. `resolution` is minutes (`1` .. `720`)
    /// or `1D`.
    ChartTrades {
        instrument_name: &'a str,
        resolution: &'a str,
    },
    /// `quote.{instrument_name}`: best bid/ask.
    Quote { instrument_name: &'a str },
    /// `deribit_price_index.{index_name}`.
    PriceIndex { index_name: &'a str },
    /// `deribit_price_ranking.{index_name}`: index constituents.
    PriceRanking { index_name: &'a str },
    /// `deribit_price_statistics.{index_name}`: 24-hour index statistics.
    PriceStatistics { index_name: &'a str },
    /// `deribit_volatility_index.{index_name}` (DVOL).
    VolatilityIndex { index_name: &'a str },
    /// `estimated_expiration_price.{index_name}`.
    EstimatedExpirationPrice { index_name: &'a str },
    /// `markprice.options.{index_name}`: option mark prices and IVs.
    MarkPriceOptions { index_name: &'a str },
    /// `perpetual.{instrument_name}.{interval}`: perpetual interest rate.
    Perpetual {
        instrument_name: &'a str,
        interval: &'a str,
    },
    /// `instrument.state.{kind}.{currency}`. Both accept `any`.
    InstrumentState { kind: &'a str, currency: &'a str },
    /// `instrument.creation.{kind}.{currency}`. Both accept `any`.
    InstrumentCreation { kind: &'a str, currency: &'a str },
    /// `platform_state`: per-index lock state.
    PlatformState,
    /// `platform_state.public_methods_state`.
    PlatformPublicMethodsState,
    /// `block_rfq.trades.{currency}`. `currency` accepts `any`.
    BlockRfqTrades { currency: &'a str },
}

impl DeribitChannel<'_> {
    pub fn name(&self) -> String {
        match self {
            Self::Book {
                instrument_name,
                interval,
            } => format!("book.{instrument_name}.{interval}"),
            Self::GroupedBook {
                instrument_name,
                group,
                depth,
                interval,
            } => format!("book.{instrument_name}.{group}.{depth}.{interval}"),
            Self::Ticker {
                instrument_name,
                interval,
            } => format!("ticker.{instrument_name}.{interval}"),
            Self::IncrementalTicker { instrument_name } => {
                format!("incremental_ticker.{instrument_name}")
            }
            Self::Trades {
                instrument_name,
                interval,
            } => format!("trades.{instrument_name}.{interval}"),
            Self::TradesByKind {
                kind,
                currency,
                interval,
            } => format!("trades.{kind}.{currency}.{interval}"),
            Self::ChartTrades {
                instrument_name,
                resolution,
            } => format!("chart.trades.{instrument_name}.{resolution}"),
            Self::Quote { instrument_name } => format!("quote.{instrument_name}"),
            Self::PriceIndex { index_name } => format!("deribit_price_index.{index_name}"),
            Self::PriceRanking { index_name } => format!("deribit_price_ranking.{index_name}"),
            Self::PriceStatistics { index_name } => {
                format!("deribit_price_statistics.{index_name}")
            }
            Self::VolatilityIndex { index_name } => {
                format!("deribit_volatility_index.{index_name}")
            }
            Self::EstimatedExpirationPrice { index_name } => {
                format!("estimated_expiration_price.{index_name}")
            }
            Self::MarkPriceOptions { index_name } => format!("markprice.options.{index_name}"),
            Self::Perpetual {
                instrument_name,
                interval,
            } => format!("perpetual.{instrument_name}.{interval}"),
            Self::InstrumentState { kind, currency } => {
                format!("instrument.state.{kind}.{currency}")
            }
            Self::InstrumentCreation { kind, currency } => {
                format!("instrument.creation.{kind}.{currency}")
            }
            Self::PlatformState => "platform_state".to_string(),
            Self::PlatformPublicMethodsState => "platform_state.public_methods_state".to_string(),
            Self::BlockRfqTrades { currency } => format!("block_rfq.trades.{currency}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Query parameters as a sorted list (`serde_json`'s `preserve_order` feature, which any
    /// crate in a build can enable, changes the map's iteration order).
    fn query_parts(r: &DeribitRequest) -> Vec<String> {
        let mut parts: Vec<String> = r.query_string().split('&').map(str::to_string).collect();
        parts.sort();
        parts
    }

    #[test]
    fn test_optional_params_omitted() {
        let r = DeribitRequest::get_instruments("BTC", None, None, None);
        assert_eq!(r.method, "public/get_instruments");
        assert_eq!(r.query_string(), "currency=BTC");
        let r = DeribitRequest::get_order_book("BTC-PERPETUAL", Some(5));
        assert_eq!(
            query_parts(&r),
            ["depth=5", "instrument_name=BTC-PERPETUAL"]
        );
    }

    #[test]
    fn test_query_string_encodes_and_formats() {
        let r = DeribitRequest::get_block_rfq_trades("BTC", Some("1794809861426:61026"), Some(10));
        assert_eq!(
            query_parts(&r),
            [
                "continuation=1794809861426%3A61026",
                "count=10",
                "currency=BTC"
            ]
        );
        let r = DeribitRequest::get_instruments("BTC", Some("future"), Some(false), None);
        assert_eq!(
            query_parts(&r),
            ["currency=BTC", "expired=false", "kind=future"]
        );
    }

    #[test]
    fn test_trade_filter_params() {
        let r = DeribitRequest::get_last_trades_by_instrument(
            "BTC-PERPETUAL",
            Some(5),
            None,
            DeribitTradeFilter {
                count: Some(2),
                sorting: Some("desc"),
                ..Default::default()
            },
        );
        assert_eq!(r.params["start_seq"], 5);
        assert_eq!(r.params["count"], 2);
        assert_eq!(r.params["sorting"], "desc");
        assert!(!r.params.contains_key("end_seq"));
        assert!(!r.params.contains_key("start_timestamp"));
    }

    #[test]
    fn test_channel_names() {
        let cases = [
            (
                DeribitChannel::GroupedBook {
                    instrument_name: "BTC-PERPETUAL",
                    group: "none",
                    depth: 20,
                    interval: "agg2",
                },
                "book.BTC-PERPETUAL.none.20.agg2",
            ),
            (
                DeribitChannel::TradesByKind {
                    kind: "option",
                    currency: "any",
                    interval: "raw",
                },
                "trades.option.any.raw",
            ),
            (
                DeribitChannel::ChartTrades {
                    instrument_name: "BTC-PERPETUAL",
                    resolution: "1",
                },
                "chart.trades.BTC-PERPETUAL.1",
            ),
            (
                DeribitChannel::MarkPriceOptions {
                    index_name: "btc_usd",
                },
                "markprice.options.btc_usd",
            ),
            (
                DeribitChannel::PlatformPublicMethodsState,
                "platform_state.public_methods_state",
            ),
            (
                DeribitChannel::BlockRfqTrades { currency: "any" },
                "block_rfq.trades.any",
            ),
        ];
        for (channel, name) in cases {
            assert_eq!(channel.name(), name);
        }
    }
}
