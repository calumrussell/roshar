# roshar-coinbase

Coinbase Advanced Trade (Spot & US Derivatives venue, `api.coinbase.com/api/v3/brokerage`): public
product metadata over REST, and subscribe messages plus `roshar-ws-mgr` config for the public market
data WebSocket. No authentication, no trading. This is not the Coinbase Exchange API
(`api.exchange.coinbase.com`) and not Coinbase International / Global Derivatives.

## API docs

| Area | Reference |
|---|---|
| REST | https://docs.cdp.coinbase.com/coinbase-app/advanced-trade-apis/rest-api |
| REST (OpenAPI spec) | https://docs.cdp.coinbase.com/api-reference/advanced-trade-api/rest-api/advanced-trade-spec.yaml |
| WebSocket | https://docs.cdp.coinbase.com/coinbase-app/advanced-trade-apis/websocket/websocket-endpoints |
| WebSocket protocol | https://docs.cdp.coinbase.com/coinbase-app/advanced-trade-apis/websocket/websocket-overview |
| Rate limits (WebSocket) | https://docs.cdp.coinbase.com/coinbase-app/advanced-trade-apis/websocket/websocket-rate-limits |
| Rate limits (REST) | Not published for the public Advanced Trade endpoints. The default of 10/s per IP is the last figure Coinbase published. |
| Changelog | https://docs.cdp.coinbase.com/coinbase-app/introduction/changelog |

Checked against these, the AsyncAPI spec
(https://docs.cdp.coinbase.com/api-reference/advanced-trade-api/advanced-trade-asyncapi.json) and
live responses on 2026-11-16.

## Coverage

REST (`MarketApi` and the `CoinbaseApi` trait on `CoinbaseClient`; all unauthenticated):

| Item | Endpoint |
|---|---|
| `get_products(&CoinbaseProductsQuery)` | `GET /api/v3/brokerage/market/products` ([ref](https://docs.cdp.coinbase.com/api-reference/advanced-trade-api/rest-api/public/list-public-products)). Every documented query parameter: `limit`, `offset`, `product_type`, `product_ids`, `contract_expiry_type`, `expiring_contract_status`, `get_all_products`, `products_sort_order`, `cursor`, `futures_underlying_type`, `user_country_code`, `expired`. Returns the page with `num_products` and `pagination`. With no parameters the server returns SPOT only. |
| `get_product` | `GET /api/v3/brokerage/market/products/{product_id}` ([ref](https://docs.cdp.coinbase.com/api-reference/advanced-trade-api/rest-api/public/get-public-product)) |
| `get_product_book` | `GET /api/v3/brokerage/market/product_book` (`limit`, `aggregation_price_increment`) |
| `get_market_trades` | `GET /api/v3/brokerage/market/products/{product_id}/ticker`: recent trades plus best bid/ask (`limit`, `start`, `end`) |
| `get_candles` | `GET /api/v3/brokerage/market/products/{product_id}/candles` (`granularity`, `start`, `end`, `limit`) |
| `get_server_time` | `GET /api/v3/brokerage/time` |

`CoinbaseProduct` carries `future_product_details` (with `perpetual_details`, margin rates,
funding, open interest, index/settlement price, `deribit_product_details`),
`fcm_trading_session_details` and `equity_product_details`. Top-level fields that the OpenAPI spec
marks `required` are plain types, the others are `Option`. The spec marks nothing required inside
the detail objects, so there fields that live data always returns are plain types and fields seen
as `null` are `Option`. Undocumented fields (e.g. equity `fundamentals`) are not mapped.
Non-2xx responses return Coinbase's `error`/`message`.

WebSocket (`wss://advanced-trade-ws.coinbase.com`, `CoinbaseClient::ws_config`, WebSocket ping
frames). Subscribe helpers on `CoinbaseClient`, message builders on `CoinbaseWssMessage`, parsing
via `CoinbaseWsMessage::parse` into `CoinbaseWsChannelMessage` (one variant per channel, common
envelope with `timestamp`, `sequence_num`, `events`) or `CoinbaseWsMessage::Error`:

| Subscribe / unsubscribe | Channel | Parsed as |
|---|---|---|
| `subscribe_depth` / `unsubscribe_depth` | `level2` ([ref](https://docs.cdp.coinbase.com/api-reference/advanced-trade-api/websocket/level2)) | `L2Data` (data arrives on channel `l2_data`) |
| `subscribe_trades` / `unsubscribe_trades` | `market_trades` ([ref](https://docs.cdp.coinbase.com/api-reference/advanced-trade-api/websocket/market-trades)) | `MarketTrades` |
| `subscribe_ticker` / `unsubscribe_ticker` | `ticker` ([ref](https://docs.cdp.coinbase.com/api-reference/advanced-trade-api/websocket/ticker)) | `Ticker` |
| `subscribe_ticker_batch` / `unsubscribe_ticker_batch` | `ticker_batch` ([ref](https://docs.cdp.coinbase.com/api-reference/advanced-trade-api/websocket/ticker-batch)) | `TickerBatch` |
| `subscribe_candles` / `unsubscribe_candles` | `candles` ([ref](https://docs.cdp.coinbase.com/api-reference/advanced-trade-api/websocket/candles)) | `Candles` |
| `subscribe_status` / `unsubscribe_status` | `status` (AsyncAPI spec; its doc page 404s) | `Status` |
| `subscribe_heartbeats` / `unsubscribe_heartbeats` | `heartbeats` ([ref](https://docs.cdp.coinbase.com/api-reference/advanced-trade-api/websocket/heartbeats)). Coinbase closes most channels after 60-90 s with no updates. | `Heartbeats` |
| (reply to every subscribe/unsubscribe) | `subscriptions` | `Subscriptions` |

Each subscribe call sends one message for all products. Unauthenticated WebSocket messages are
limited to 8/s per IP. Coinbase closes the connection if no subscribe arrives within 5 s. An
unknown channel, or a product channel without `product_ids`, gets
`{"type":"error","message":"authentication failure"}`; unknown product ids are dropped silently.

Fixtures under `tests/fixtures/` are live captures from 2026-11-16; `tests/parse.rs` parses them.

## Not covered yet

- `GET /api/v3/brokerage/best_bid_ask`: needs authentication (401 without a JWT). The public
  `get_market_trades` returns best bid/ask.
- JWT (CDP key, ES256) auth, private REST (orders, accounts, portfolios), and the user WebSocket
  (`wss://advanced-trade-ws-user.coinbase.com`, channels `user`, `futures_balance_summary`).
- Global Derivatives (`https://drb.coinbase.com/api/v2`, `wss://streams.drb.coinbase.com/ws/api/v2`,
  [overview](https://docs.cdp.coinbase.com/coinbase-app/advanced-trade-apis/guides/derivatives/overview)).
  International perpetuals moved there from INTX on 2026-10-01. Its public market data is the
  Deribit API v2 (JSON-RPC `public/*` methods, Deribit channels) on Coinbase hosts. On 2026-11-16
  `public/get_instruments?kind=future` returned the same 221 instruments with the same fields on
  `drb.coinbase.com` and `www.deribit.com`, and `public/ticker` returned the same book. It is one
  order book, so use `roshar-deribit` for this market data instead of a second copy of the
  Deribit client here. The legacy INTX perpetuals (`*-PERP-INTX`) still appear in
  `get_products` with `product_type` FUTURE, with funding frozen at 2026-10-01.
