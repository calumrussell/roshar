# roshar-kraken

Kraken Futures (Kraken Derivatives) client: public market data over REST (derivatives v3, charts
and analytics, market history) and the Unified WebSocket, and signed REST order management and
leverage preferences. Kraken Spot is not modelled; every websocket request carries
`asset_class: "derivatives"`.

UK note: Kraken offers derivatives to UK residents only if they qualify as Professional Clients
(FCA rules, arranged by Crypto Facilities Ltd). No public endpoint was geo-blocked from the UK on
2026-11-16.

## API docs

| Area | Reference |
|---|---|
| REST (Derivatives v3) | https://docs.kraken.com/api-reference/market-data/get-tickers (one page per endpoint; spec: https://docs.kraken.com/openapi/futures-rest.yaml) |
| REST (charts, analytics) | https://docs.kraken.com/api-reference/candles/market-candles, https://docs.kraken.com/api-reference/analytics/market-analytics (spec: https://docs.kraken.com/openapi/futures-charts-rest.yaml) |
| REST (market history) | https://docs.kraken.com/api-reference/market-history/get-public-execution-events (spec: https://docs.kraken.com/openapi/futures-history-rest.yaml) |
| WebSocket | https://docs.kraken.com/exchange/api-reference/unified-websocket, channels under https://docs.kraken.com/exchange/api-reference/unified-ws/ (`book`, `trade`, `ticker`, `ohlc`, `instrument`, `rfqs`, `status`, `heartbeat`, `ping`) |
| Auth / signing | https://docs.kraken.com/exchange/guides/futures/rest |
| Rate limits | https://docs.kraken.com/exchange/guides/futures/ratelimits |
| Changelog | https://docs.kraken.com/exchange/changelog |

Checked against these on 2026-11-16.

## Coverage

REST market data (`KrakenApi`, implemented by `KrakenClient`). Optional parameters are `Option`
and omitted when `None`; paging (`lastTime`, `continuation_token`, `since`/`before`, `count`) is
left to the caller.

| Method | Endpoint |
|---|---|
| `get_tickers` | `GET /derivatives/api/v3/tickers?contractType=&symbol=` (options tickers carry `greeks`, `extrinsicValue`, `markIv`) |
| `get_ticker` | `GET /derivatives/api/v3/tickers/{symbol}` |
| `get_instruments` | `GET /derivatives/api/v3/instruments?contractType=&expired=` |
| `get_instruments_status` | `GET /derivatives/api/v3/instruments/status?contractType=` |
| `get_instrument_status` | `GET /derivatives/api/v3/instruments/{symbol}/status` |
| `get_orderbook` | `GET /derivatives/api/v3/orderbook?symbol=` |
| `get_trade_history` | `GET /derivatives/api/v3/history?symbol=&lastTime=&includeMTFData=` |
| `get_historical_funding_rates` | `GET /derivatives/api/v3/historical-funding-rates?symbol=` |
| `get_candles` | `GET https://futures.kraken.com/api/charts/v1/{tick_type}/{symbol}/{resolution}?from=&to=&count=` |
| `get_chart_tick_types` | `GET /api/charts/v1/` |
| `get_chart_markets` | `GET /api/charts/v1/{tick_type}` |
| `get_chart_resolutions` | `GET /api/charts/v1/{tick_type}/{symbol}` |
| `get_analytics` | `GET /api/charts/v1/analytics/{symbol}/{analytics_type}?since=&interval=&to=` (all 16 types: open interest, aggressor differential, trade volume/count, liquidation volume, rolling volatility, long/short ratio and info, CVD, top traders, orderbook, spreads, liquidity, slippage, future basis, funding) |
| `get_liquidity_pool` | `GET /api/charts/v1/analytics/liquidity-pool?since=&interval=&to=` |
| `get_market_executions` | `GET https://futures.kraken.com/api/history/v3/market/{tradeable}/executions` |
| `get_market_order_events` | `GET /api/history/v3/market/{tradeable}/orders` |
| `get_market_mark_price_events` | `GET /api/history/v3/market/{tradeable}/price` |

Unified WebSocket (`wss://uws.kraken.com/v2/public`), `KrakenClient` helpers built on
`KrakenWssMessage`:

| Helpers | Channel |
|---|---|
| `subscribe_depth` / `unsubscribe_depth` | `book`, caller-chosen depth and snapshot |
| `subscribe_trades` / `unsubscribe_trades` | `trade` |
| `subscribe_ticker` / `unsubscribe_ticker` | `ticker` |
| `subscribe_ohlc` / `unsubscribe_ohlc` | `ohlc`, caller-chosen interval and tick type (`trade`/`mark`) |
| `subscribe_instruments` / `unsubscribe_instruments` | `instrument` |
| `subscribe_rfqs` / `unsubscribe_rfqs` | `rfqs` (options RFQs) |
| `ws_config` | protocol ping frames every 20 s (answered with pong frames) |

`KrakenWssEvent::parse` turns any frame into `Book`, `Trade`, `Ticker`, `Ohlc`, `Instrument`,
`Rfqs`, `Status`, `Heartbeat`, `Pong` (application `ping` reply) or `Ack` (subscribe/unsubscribe
replies; `success: false` carries Kraken's `error`). Unknown channels are errors.

Signed REST:

| Item | Endpoint |
|---|---|
| `OrderManagementApi::create_order` | `POST /derivatives/api/v3/sendorder` |
| `OrderManagementApi::modify_order` | `POST /derivatives/api/v3/editorder` |
| `OrderManagementApi::cancel_order` | `POST /derivatives/api/v3/cancelorder` |
| `OrderManagementApi::get_open_orders` | `GET /derivatives/api/v3/openorders` |
| `OrderManagementApi::get_order_status` | `POST /derivatives/api/v3/orders/status` |
| `OrderManagementApi::dead_mans_switch` | `POST /derivatives/api/v3/cancelallordersafter` |
| `MultiCollateralApi::get_leverage` / `set_leverage` | `GET` / `PUT /derivatives/api/v3/leveragepreferences` |

Order events are the tagged enum `KrakenOrderEvent` (`PLACE`/`CANCEL`/`EDIT`/`REJECT`/`EXECUTION`).

Signing: `Authent = base64(HMAC-SHA512(base64decode(secret), SHA256(postData + nonce + path)))`,
with `/derivatives` stripped from the path and `postData` signed exactly as sent (url-encoded),
as krakenfx/api-go `derivatives.Sign` does. Tested against the documented sign-challenge vector,
which uses the same pipeline.

A derivatives reply with `"result": "error"` is returned as an error carrying Kraken's error;
charts and history errors carry the HTTP status and body, analytics errors Kraken's `errors`
list. Rate limiting uses the caller's `RateLimitedClient`; Kraken budgets `/derivatives` calls by
cost (500 per 10 s, e.g. `sendorder` 10, `openorders` 2) and public endpoints are free.

Live differences from the docs, as modelled (2026-11-16): `history` sends `size` as a number and
omits the MTF fields unless `includeMTFData=true`; analytics `cvd` keys are snake_case, `funding`
values are strings and its timestamps milliseconds; the WS `status` message sends
`futures_exchange`/`spot_exchange` instead of `system`; WS tickers carry mark/index/premium,
open interest and funding fields; `ohlc` with `tick_type: spot` is rejected, and an `ohlc`
`mark` subscription is only removed when the unsubscribe also carries `tick_type: mark`.

## Not covered yet

- Kraken Spot (REST and `asset_class: spot` websocket): this crate models Futures only.
- Index tickers (`in_*`/`rr_*`) in `tickers`: documented, but none are returned and
  `tickers/rr_xbtusd` answers `contractNotFound` (2026-11-16).
- Unified WS `level3`: requires an authentication token (an unauthenticated subscribe is
  rejected with `request not permitted on this instance`); private channels and WS order entry.
- `GET /feeschedules`: public but fee data, not market data.
- The deprecated Derivatives WebSocket `wss://futures.kraken.com/ws/v1` (shut down 2027-03-01) is no
  longer used.
- `sendorder` parameters `trailingStopMaxDeviation`, `trailingStopDeviationUnit`,
  `limitPriceOffsetValue`, `limitPriceOffsetUnit`, `processBefore`, `broker`; `editorder`
  `qtyMode` and trailing-stop parameters; the `algoId` header.
- `batchorder`, `cancelallorders`, accounts, positions, fills, `trading/instruments`, transfers,
  `pnlpreferences`, `rfq-assignment/max-leverage`, account history.
