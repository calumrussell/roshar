# roshar-binance

Binance USDⓈ-M futures public market data: REST (`https://fapi.binance.com`) and websocket subscribe
messages for roshar-ws-mgr. The trade and book-ticker message types also parse the spot streams.

## API docs

| Area | Reference |
|---|---|
| REST | https://developers.binance.com/en/docs/catalog/core-trading-derivatives-trading-usd-s-m-futures/api/rest-api/market-data |
| WebSocket | https://developers.binance.com/en/docs/products/derivatives-trading-usds-futures/websocket-market-streams/Connect, streams: https://developers.binance.com/en/docs/catalog/core-trading-derivatives-trading-usd-s-m-futures/api/ws-streams/public and https://developers.binance.com/en/docs/catalog/core-trading-derivatives-trading-usd-s-m-futures/api/ws-streams/market |
| WS URL split | https://developers.binance.com/en/docs/products/derivatives-trading-usds-futures/websocket-market-streams/Important-WebSocket-Change-Notice |
| Spot WebSocket | https://developers.binance.com/en/docs/products/spot/web-socket-streams, streams: https://developers.binance.com/en/docs/catalog/core-trading-spot-trading/api/ws-streams/~ |
| Rate limits | https://developers.binance.com/en/docs/products/derivatives-trading-usds-futures/general-info (weights are on each REST endpoint) |
| Changelog | https://developers.binance.com/en/docs/products/derivatives-trading-usds-futures/change-log, spot: https://developers.binance.com/en/docs/products/spot/CHANGELOG |

Checked against these on 2026-11-16. Old `developers.binance.com/docs/derivatives/...` URLs now
redirect to `/en/docs/products/...`. The docs site is a JS app. Use a browser, or append `.md` to the
`/products/...` pages.

From a UK IP, every REST endpoint below and the `/public` and `/market` websockets work. There was no
geo-block.

## Coverage

All public USDⓈ-M market-data endpoints and market streams listed in the docs (cross-checked with
the endpoint list of Binance's `binance-connector-js` USDⓈ-M client on 2026-11-16). Every REST
endpoint was called live and every stream was captured live, except `!contractInfo` (no push in a
200 s capture; parsed from the doc example). Captures are in `tests/fixtures/`.

| Item | REST (`BinanceApi`) |
|---|---|
| Ping, server time | `GET /fapi/v1/ping`, `GET /fapi/v1/time` |
| Exchange info (symbols, filters, rate limits, assets) | `GET /fapi/v1/exchangeInfo` |
| Order book | `GET /fapi/v1/depth` (`limit` 5/10/20/50/100/500/1000) |
| RPI order book | `GET /fapi/v1/rpiDepth` (`limit` 1000 only, checked live) |
| Recent trades | `GET /fapi/v1/trades` |
| Historical trades | `GET /fapi/v1/historicalTrades` (needs an API key in `X-MBX-APIKEY`, no signature; the caller passes it) |
| Aggregate trades | `GET /fapi/v1/aggTrades` (`limit` max 1000, last 48 h only) |
| Klines | `GET /fapi/v1/klines`, `continuousKlines`, `indexPriceKlines`, `markPriceKlines`, `premiumIndexKlines` (`limit` max 1500) |
| Mark price / index / funding | `GET /fapi/v1/premiumIndex` (weight 10 without a symbol) |
| Funding history | `GET /fapi/v1/fundingRate` (one page, `limit` max 1000; 500 req / 5 min / IP) |
| Funding info | `GET /fapi/v1/fundingInfo` |
| Tickers | `GET /fapi/v1/ticker/24hr` (weight 40 without a symbol), `GET /fapi/v1/ticker/price`, `GET /fapi/v2/ticker/price`, `GET /fapi/v1/ticker/bookTicker` |
| Open interest | `GET /fapi/v1/openInterest`, `GET /futures/data/openInterestHist` |
| Statistics | `GET /futures/data/topLongShortAccountRatio`, `topLongShortPositionRatio`, `globalLongShortAccountRatio`, `takerlongshortRatio`, `basis` |
| Delivery price | `GET /futures/data/delivery-price` |
| Indexes | `GET /fapi/v1/indexInfo` (composite), `GET /fapi/v1/assetIndex` (multi-assets), `GET /fapi/v1/constituents` |
| Insurance fund | `GET /fapi/v1/insuranceBalance` |
| ADL risk | `GET /fapi/v1/symbolAdlRisk` |
| TradFi sessions | `GET /fapi/v1/tradingSchedule` |

Endpoints that take an optional `symbol` return one object with it and an array without it; the
client returns a `Vec` in both cases. A non-2xx reply becomes an error carrying Binance's `code` and
`msg`.

| Route | Streams (`BinanceStream`) |
|---|---|
| `/public` | `<symbol>@depth[@500ms\|@100ms]`, `<symbol>@depth<5\|10\|20>[@500ms\|@100ms]`, `<symbol>@rpiDepth@500ms`, `<symbol>@bookTicker`, `!bookTicker`, `<symbol>@trade` |
| `/market` | `<symbol>@aggTrade`, `<symbol>@markPrice[@1s]`, `!markPrice@arr[@1s]`, `<symbol>@kline_<interval>`, `<pair>_<contractType>@continuousKline_<interval>`, `<symbol>@miniTicker`, `!miniTicker@arr`, `<symbol>@ticker`, `!ticker@arr`, `<symbol>@forceOrder`, `!forceOrder@arr`, `<symbol>@compositeIndex`, `!contractInfo`, `<assetSymbol>@assetIndex`, `!assetIndex@arr`, `tradingSession` |

`BinanceWsEvent::parse` parses every frame of both routes (raw or combined `{"stream","data"}`
form): data events by `e`, `!…@arr` arrays, `{"result","id"}` replies and `{"error":{code,msg},"id"}`
errors. An unknown event type is an error. Binance replies `{"result":null}` even to unknown stream
names, so a bad subscription is silent (checked live). `!forceOrder@arr` pushes single objects, not
arrays. The `tradingSession` event name is `<market>Update` (e.g. `FXUpdate`).

Websockets: USDⓈ-M streams are split by route. `BinanceClient::ws_public_config()`
(`wss://fstream.binance.com/public/ws`, connection `binance-public`) and `ws_market_config()`
(`wss://fstream.binance.com/market/ws`, connection `binance-market`). Add both to the manager.
`subscribe`/`unsubscribe` send each stream to its route's connection, one batched message per
route. The unrouted `wss://fstream.binance.com/ws` was decommissioned on 2026-04-23. Since then it
acts like `/public`, so market streams on it get no data (checked live). A connection accepts at
most 10 incoming messages per second and carries at most 1024 streams.

`<symbol>@trade` is not in the USDⓈ-M stream docs, but `/public` serves it (checked live). Since the
COIN-M merge, futures payloads carry `st` (`1` USDⓈ-M, `2` COIN-M) and most carry `ps` (pair).
The trade and book-ticker types also parse the spot streams, so their futures-only fields (`X`,
`st`, `e`, `E`, `T`, `ps`) are `Option`. `BINANCE_SPOT_WSS_URL` is the spot base. The spot
diff-depth payload has no `pu`, so `BinanceDepthDiffMessage` is futures-only.

The REST rate limiter counts requests, not weight. Binance's limit is 2400 weight per minute per IP.
Choose `requests_per_second` with the endpoint weights in mind.

## Not covered yet

- COIN-M (`https://dapi.binance.com`, `dstream`): a separate API with its own paths (`/dapi/v1/…`),
  contract-denominated quantities, `pair`-keyed responses and arrays where USDⓈ-M returns objects
  (e.g. `premiumIndex?symbol=` returns an array). Not trivial; needs its own client and types.
- Spot REST and the spot streams other than trade and book ticker: out of scope for this crate.
- `!contractInfo` has no live capture (no push during 200 s); its type follows the docs.
- No trading, account or user data streams.
