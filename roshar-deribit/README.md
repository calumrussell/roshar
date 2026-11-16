# roshar-deribit

Deribit public market data: every public JSON-RPC market-data method over REST
(`GET /api/v2/public/*`, through `roshar_http::RateLimitedClient`) and as websocket request
builders, the public websocket channels with subscribe/unsubscribe builders, heartbeat control,
and strict types for every result and notification.

## API docs

| Area | Reference |
|---|---|
| Overview, endpoints | https://docs.deribit.com/ |
| JSON-RPC over WebSocket / HTTP | https://docs.deribit.com/articles/json-rpc-overview |
| OpenAPI (all methods, params, results) | https://docs.deribit.com/specifications/deribit_openapi.json |
| AsyncAPI (all channels) | https://docs.deribit.com/specifications/deribit_asyncapi.json |
| Market data methods | https://docs.deribit.com/api-reference/market-data/public-get_instruments (and siblings) |
| Combo books | https://docs.deribit.com/api-reference/combo-books/public-get_combos |
| Block RFQ trades | https://docs.deribit.com/api-reference/block-rfq/public-get_block_rfq_trades |
| Supporting (`status`, `get_time`, `hello`, `test`) | https://docs.deribit.com/api-reference/supporting/public-status |
| Subscription / session management | https://docs.deribit.com/api-reference/subscription-management/public-subscribe |
| Subscriptions | https://docs.deribit.com/articles/notifications |
| Rate limits | https://docs.deribit.com/articles/rate-limits |
| Connection management | https://docs.deribit.com/articles/connection-management-best-practices |
| Changelog | https://docs.deribit.com/changelogs/jsonrpc |
| Restricted jurisdictions | https://support.deribit.com/hc/en-us/articles/25944487427741-Restricted-Jurisdictions |

Checked against these on 2026-11-16. Live API version was `1.2.26`.

## Coverage

Every REST method below is a `DeribitApi` method on `DeribitClient` (and on `DeribitMarketApi`),
and a `DeribitRequest` builder that `DeribitClient::send_request` sends over the websocket; the
reply is a `DeribitWsIncoming::Response` whose `result_as::<T>()` takes the same result type.

| Method (`public/…`) | Result |
|---|---|
| `get_instruments` | `Vec<DeribitInstrument>` |
| `get_instrument` | `DeribitInstrument` |
| `get_currencies` | `Vec<DeribitCurrency>` |
| `get_book_summary_by_currency`, `get_book_summary_by_instrument` | `Vec<DeribitBookSummary>` |
| `get_order_book`, `get_order_book_by_instrument_id` | `DeribitOrderBookSnapshot` |
| `ticker` | `DeribitTickerData` |
| `get_last_trades_by_instrument`, `_by_instrument_and_time`, `_by_currency`, `_by_currency_and_time` | `DeribitTradesPage` |
| `get_tradingview_chart_data` | `DeribitTradingViewChartData` |
| `get_funding_rate_history` | `Vec<DeribitFundingRateHistory>` |
| `get_funding_rate_value` | `f64` |
| `get_funding_chart_data` | `DeribitFundingChartData` |
| `get_historical_volatility`, `get_index_chart_data`, `get_mark_price_history` | `Vec<DeribitTimeValue>` |
| `get_volatility_index_data` | `DeribitVolatilityIndexData` |
| `get_index_price` | `DeribitIndexPrice` |
| `get_index_price_names` | `Vec<String>`; `extended=true`: `Vec<DeribitIndexPriceName>` |
| `get_supported_index_names` | `Vec<String>` |
| `get_basket_index_composition` | `DeribitBasketIndexComposition` |
| `get_delivery_prices` | `DeribitDeliveryPrices` |
| `get_last_settlements_by_currency`, `_by_instrument` | `DeribitSettlementsPage` |
| `get_contract_size` | `DeribitContractSize` |
| `get_expirations` | `DeribitExpirations` |
| `get_apr_history` | `DeribitAprHistory` |
| `get_trade_volumes` | `Vec<DeribitTradeVolume>` |
| `get_combos`, `get_combo_details` | `Vec<DeribitCombo>`, `DeribitCombo` |
| `get_combo_ids` | `Vec<String>` |
| `get_block_rfq_trades` | `DeribitBlockRfqTradesPage` |
| `status` | `DeribitStatus` |
| `get_time` | `u64` (ms) |
| `test` | `DeribitVersion` |
| `hello` (websocket only) | `DeribitVersion` |
| `subscribe`, `unsubscribe` (websocket only) | `Vec<String>` |
| `unsubscribe_all`, `set_heartbeat`, `disable_heartbeat` (websocket only) | `String` (`"ok"`) |

| `DeribitChannel` | Channel | `DeribitData` |
|---|---|---|
| `Book` | `book.{instrument_name}.{interval}` | `Book(DeribitOrderBookData)` |
| `GroupedBook` | `book.{instrument_name}.{group}.{depth}.{interval}` | `GroupedBook(DeribitGroupedBookData)` |
| `Ticker` | `ticker.{instrument_name}.{interval}` | `Ticker(DeribitTickerData)` |
| `IncrementalTicker` | `incremental_ticker.{instrument_name}` | `IncrementalTicker(DeribitIncrementalTickerData)` |
| `Trades` | `trades.{instrument_name}.{interval}` | `Trades(Vec<DeribitTradeData>)` |
| `TradesByKind` | `trades.{kind}.{currency}.{interval}` | `Trades(Vec<DeribitTradeData>)` |
| `ChartTrades` | `chart.trades.{instrument_name}.{resolution}` | `ChartTrades(DeribitChartTradesData)` |
| `Quote` | `quote.{instrument_name}` | `Quote(DeribitQuoteData)` |
| `PriceIndex` | `deribit_price_index.{index_name}` | `PriceIndex(DeribitPriceIndexData)` |
| `PriceRanking` | `deribit_price_ranking.{index_name}` | `PriceRanking(Vec<DeribitPriceRankingEntry>)` |
| `PriceStatistics` | `deribit_price_statistics.{index_name}` | `PriceStatistics(DeribitPriceStatisticsData)` |
| `VolatilityIndex` | `deribit_volatility_index.{index_name}` | `VolatilityIndex(DeribitVolatilityIndexTick)` |
| `EstimatedExpirationPrice` | `estimated_expiration_price.{index_name}` | `EstimatedExpirationPrice(…)` |
| `MarkPriceOptions` | `markprice.options.{index_name}` | `MarkPriceOptions(Vec<DeribitMarkPriceOption>)` |
| `Perpetual` | `perpetual.{instrument_name}.{interval}` | `Perpetual(DeribitPerpetualData)` |
| `InstrumentState` | `instrument.state.{kind}.{currency}` | `InstrumentState(DeribitInstrumentStateData)` |
| `InstrumentCreation` | `instrument.creation.{kind}.{currency}` | `InstrumentCreation(Box<DeribitInstrument>)` |
| `PlatformState` | `platform_state` | `PlatformState(DeribitPlatformStateData)` |
| `PlatformPublicMethodsState` | `platform_state.public_methods_state` | `PublicMethodsState(…)` |
| `BlockRfqTrades` | `block_rfq.trades.{currency}` | `BlockRfqTrade(Box<DeribitBlockRfq>)` (one per notification) |

| Item | Detail |
|---|---|
| `DERIBIT_REST_URL`, `DERIBIT_TESTNET_REST_URL` | `https://www.deribit.com/api/v2`, `https://test.deribit.com/api/v2` (`DeribitMarketApi::with_base_url`) |
| `DERIBIT_WSS_URL`, `DERIBIT_TESTNET_WSS_URL` | `wss://www.deribit.com/ws/api/v2`, `wss://test.deribit.com/ws/api/v2` |
| `DeribitWsIncoming::parse` | Any server frame: `Subscription` (data typed by channel), `Heartbeat`, or `Response` |
| keepalive (`ws_config`) | `public/test` as a text frame every 30 s, id `DERIBIT_KEEPALIVE_ID` |

| Note | Detail |
|---|---|
| Errors | Exchange errors (REST: HTTP 400 with the JSON-RPC envelope) become `DeribitRpcError { code, message, data }`. |
| Request ids | Every websocket request takes a caller-chosen JSON-RPC `id`. The keepalive uses `DERIBIT_KEEPALIVE_ID` (1). |
| Intervals | `100ms`, `agg2` or `raw`. The builders accept `raw`, but unauthenticated `raw` subscriptions fail with error 13778 (`raw_subscriptions_not_available_for_unauthorized`) and this crate does not authenticate. |
| Heartbeat | `interval` is 10..=300 s. Reply to each `test_request` with `DeribitClient::send_test`. Only JSON text messages count as client activity, not websocket ping frames. After `2 * interval + 0.5` s of silence the server closes with code 4000. |
| Book | `book.*.{interval}`: first notification is a full snapshot (`type: snapshot`, no `prev_change_id`); each later one has `prev_change_id` equal to the previous `change_id`. On a gap, resubscribe. The grouped book sends the whole grouped top `depth` levels every time. |
| Incremental ticker | First notification is a full snapshot; later ones only carry changed fields (also inside `stats` and `greeks`), so every field is optional and nullable ones are `Option<Option<_>>`. |
| Ticker | `open_interest` and `estimated_delivery_price` are absent for spot. `open_interest` is 8 hours delayed. `best_*` and `last_price` are always present but can be `null`. |
| Trades | Delayed Block RFQ trades keep their original `timestamp` and `trade_seq`, so they can arrive out of sequence. `get_last_trades_by_currency*` without `kind` returns error 11060 for currencies with Coinbase-routed spot (BTC included); pass a `kind`. |
| Mark price history | Only options used in the volatility index have data; everything else returns `[]`. |
| Expirations | `currency=any` returns the kinds at top level; otherwise they are keyed by currency or pair (`DeribitExpirations::{Kinds, ByCurrency}`). |
| Rate limits | Unauthenticated limits are per IP. Default methods: 500 credits each, 50,000 pool, 10,000/s refill (20 req/s, burst 100). `get_instruments`: 1 req/s, burst 50. `public/subscribe`: about 3.3 calls/s, burst 10; put many channels in one call. `DeribitClient::new()` uses one REST bucket of 1 request/s (safe for `get_instruments`); use `new_with_rate_limit` to change it. |
| UK | Deribit does not allow UK retail clients. Public market data is reachable from the UK (no geo-block seen). |

## Not covered yet

- Authentication (`public/auth`, `exchange_token`, `fork_token`) and so all private methods and
  channels, and `raw` delivery of public channels (needs an authenticated session).
- `public/get_announcements` and the `announcements` channel (out of scope: announcements).
- The Starbase REST gateway (`/api/v2/public/get_instruments` on Starbase IPs), FIX and the Starbase
  binary APIs: reachable only via colocation, cross-connect or AWS PrivateLink.
- A client-side `public/subscribe` credit limiter.
