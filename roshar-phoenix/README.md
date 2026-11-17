# roshar-phoenix

Phoenix (Ellipsis Labs, Solana perps; `perp-api.phoenix.trade`) client: public market data and
trader read endpoints over REST, plus websocket subscription messages and typed payloads.
Read-only. No signing, no transaction building.

## API docs

| Area | Reference |
|---|---|
| REST | https://docs.phoenix.trade/api (per-endpoint pages, e.g. https://docs.phoenix.trade/api/exchange/list-markets). Machine-readable: https://docs.phoenix.trade/openapi/phoenix-public-api.json |
| WebSocket | https://docs.phoenix.trade/api/websocket |
| Auth | https://docs.phoenix.trade/sdk/auth (JWT sessions; not used by this crate) |
| Rate limits | https://docs.phoenix.trade/sdk/ws-api-best-practices (qualitative only; no numbers published) |
| Reference SDK | https://github.com/Ellipsis-Labs/rise-public (`rust/types` is the wire-type reference) |
| Changelog | https://github.com/Ellipsis-Labs/rise-public/blob/master/rust/CHANGELOG.md (SDK changelog; the API has none) |

Checked against these on 2026-11-15.

Where the OpenAPI schema, the SDK and live responses disagree, the types follow live responses:

- `commodityMetadata` price fields are `{value, decimals, ui}` objects. OpenAPI says string.
- `JsSafeU64` fields (`openInterestCapBaseLots`, ...) arrive as decimal strings. OpenAPI says
  integer. Both forms are accepted, as in the SDK.
- PnL points include `cumulativeMakerFee`. OpenAPI omits it. Their times are unix seconds, not ms.
- Trader-state subaccounts carry `margin`. OpenAPI omits it.
- Order-history `status`: OpenAPI enumerates `active|cancelled|filled`, the SDK
  `open|filled|cancelled|expired`. It is kept as a string.
- Subscription acks arrive as `{"channel":"subscriptionStatus","status":"subscribed"|"unsubscribed",...}`,
  not the documented `subscriptionConfirmed`.

From a UK IP every endpoint used here returned 200 and the websocket connected. No geo-block
was seen.

## Coverage

| Item | Endpoint / channel |
|---|---|
| Exchange keys + markets | `GET /v1/view/exchange` |
| Exchange keys | `GET /v1/view/exchange/keys` |
| Exchange status | `GET /v1/view/exchange/status` |
| Exchange snapshot | `GET /v1/exchange/snapshot` (exchange state + full market parameters, ~0.5 MB; same shape as the WS `exchange` snapshot) |
| Markets | `GET /v1/view/exchange/markets`, `GET /v1/view/exchange/market/{symbol}` (symbol is case-insensitive server-side) |
| Orderbook | `GET /v1/view/orderbook/{symbol}?includeSplines=&bypassExecutionBand=` |
| Mark price | `GET /v1/market/{symbol}/mark-price` |
| Market stats history | `GET /v1/market/{symbol}/stats?start_time=&end_time=&limit=&timeframe=` (RFC 3339 times; snake_case params) |
| Market calendars | `GET /v1/market/{symbol}/market-calendar`, `GET /v1/market-calendar/{id}`, `GET /v1/market/commodity-calendar`, `GET /v1/market/{symbol}/next-market-calendar-transition`, `GET /v1/market/next-commodity-market-transition` |
| Funding rates | `GET /v1/funding/{symbol}/rates?startTime=&endTime=&limit=`, `GET /v1/funding/overview?startTime=&endTime=&perMarketLimit=` (ms) |
| Market fills | `GET /v1/trades/{symbol}/fills?limit=&cursor=&startTime=&endTime=` (ms) |
| Candles | `GET /v1/candles/{symbol}?timeframe=&startTime=&endTime=&limit=&enableExternalSource=` (ms) |
| Trader state | `GET /v1/trader/state/{authority}?traderPdaIndex=` (positions, open orders, margin per subaccount) |
| PnL | `GET /v1/users/{authority}/pnl?resolution=` (`1m,5m,15m,1h,4h,1d,1w,1M`) |
| Order history | `GET /v1/trader/{authority}/order-history?limit=` (`limit` required) |
| Trade history | `GET /v1/trader/{authority}/trades-history?pdaIndex=` |
| Funding history | `GET /v1/trader/{authority}/funding-history?traderPdaIndex=` |
| Collateral history | `GET /v1/trader/{authority}/collateral-history?pdaIndex=&limit=&nextCursor=` (`limit` required) |
| WS subscribe/unsubscribe | `allMids`, `exchange` (optional `encoding`), `fundingRate`, `orderbook` (optional `bypassExecutionBand`), `market`, `trades`, `candles`, `traderState` |
| WS payload types | `PhoenixWsServerMessage` (tagged by `channel`): `PhoenixAllMids`, `PhoenixFundingRate`, `PhoenixL2BookUpdate`, `PhoenixMarketStats`, `PhoenixTradesMessage`, `PhoenixCandleUpdate` (server channel `candle`), `PhoenixExchangeMessage` (`snapshot` / `encodedSnapshot` / `delta`), `PhoenixSubscriptionStatus`, `PhoenixWsError`; `traderState` stays `serde_json::Value` |
| Keepalive | WebSocket ping/pong frames. The server rejects a JSON `{"type":"ping"}` with a 400 error frame. |

REST is rate-limited to 2 req/s by default (`PhoenixClient::new_with_rate_limit` overrides this).
Phoenix publishes no number. Measured unauthenticated on 2026-11-15: 2 req/s sustained passes,
while 3 req/s and above returns 429 with `retry-after: 2`, after a burst of about 20.

`tests/parse.rs` deserializes every REST and WS type from live payloads captured on 2026-11-15
and 2026-11-16 (`tests/fixtures/`; long arrays truncated, values unedited). It also checks that
removing a documented-required field is an error and that removing a documented-optional field
is not. Exchange-channel deltas are tested against the documented example only: none arrived
during capture (they fire only on exchange/market parameter changes).

More disagreements between docs and live (2026-11-16):

- Orderbook query params: the server honours `includeSplines` / `bypassExecutionBand`; the
  documented `include_splines` / `bypass_execution_band` are silently ignored. Spline amounts are
  `{value, decimals, ui}` objects, not strings.
- Market stats params are snake_case (`start_time`, ...); camelCase is ignored.
- Funding rate / overview `timestamp` is unix seconds (integer), not an ISO string.
- Snapshot commodity price bands are `{lower, upper}`; REST market configs use `{min, max}`.
- `subscriptionError` / `subscriptionConfirmed` are never sent; rejections arrive on the `error`
  channel with `code` and an echoed `subscription`. Re-requesting an `exchange` snapshot within
  ~15 s is rejected.

## Not covered yet

- Typed `traderState` websocket payloads (trader-specific).
- Snapshot `spotCollaterals` and the `spotCollateralsUpdated` delta payload (spot collateral config, not market data; kept untyped/ignored).
- Decoding the `base64+zstd` exchange snapshot payload (would need a zstd dependency); use `encoding: "json"`.
- Trader-specific REST: `/v1/view/trader/{pubkey}`, `/v1/view/trader-capabilities`, `trades_v2`/`orders_v2`, `/v1/traders/{pubkey}/pnl*`,
  portfolio values, user/trader collateral history variants, liquidation history (`/v1/users/{pubkey}/liquidation-history`
  is per user; there is no market-wide feed), hourly funding (`/v1/users/{pubkey}/funding-hourly`, per user), notifications.
- Trader-state triggers (TP/SL), splines, spot collateral, locked deposit, escrow requests (fields are ignored).
- Order-history `orderStatus` filter, collateral-history `prevCursor`.
- Auth (JWT) for higher rate limits; transaction building (`/v1/ix/*`), invites/referrals, and signing.
