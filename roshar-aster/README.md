# roshar-aster

Aster client for public market data, no authenticated endpoints and no signing:

- Perpetual futures: REST `https://fapi.asterdex.com` (V3 paths), WebSocket
  `wss://fstream.asterdex.com/ws` (raw) and `/stream` (combined). `AsterClient`, `AsterApi`.
- Spot: REST `https://sapi.asterdex.com` (`/api/v3`), WebSocket `wss://sstream.asterdex.com/ws`
  and `/stream`. `spot::AsterSpotClient`.

## API docs

| Area | Reference |
|---|---|
| Futures REST (market data) | https://asterdex.github.io/aster-api-website/futures-v3/market-data/ |
| Futures WebSocket | https://asterdex.github.io/aster-api-website/futures-v3/websocket-market-streams/ |
| Futures rate limits, general info | https://asterdex.github.io/aster-api-website/futures-v3/general-info/ |
| Futures source (markdown) | https://github.com/asterdex/api-docs/blob/master/V3(Recommended)/EN/aster-finance-futures-api-v3.md |
| Spot source (markdown) | https://github.com/asterdex/api-docs/blob/master/V3(Recommended)/EN/aster-finance-spot-api-v3.md |
| Changelog | https://github.com/asterdex/api-docs/blob/master/CHANGELOG.md |

Checked against these on 2026-11-16. Use the V3 docs only. The changelog entry of 2026-11-02
removed the futures V1 docs. `docs.asterdex.com/for-developers/aster-api/api-documentation` still
describes V1 and is out of date.

## Coverage: futures REST (`AsterApi` on `AsterClient`)

| Method | Endpoint |
|---|---|
| `ping()` | `GET /fapi/v3/ping` |
| `get_server_time()` | `GET /fapi/v3/time` |
| `get_exchange_info()` | `GET /fapi/v3/exchangeInfo`. Filters are the typed `AsterSymbolFilter`; an unknown `filterType` is an error. |
| `get_orderbook(symbol, limit)` | `GET /fapi/v3/depth`. `None` omits `limit` (exchange default 500). |
| `get_recent_trades(symbol, limit)` | `GET /fapi/v3/trades` |
| `get_historical_trades(symbol, limit, from_id)` | `GET /fapi/v3/historicalTrades` (no API key needed live) |
| `get_agg_trades(symbol, from_id, start, end, limit)` | `GET /fapi/v3/aggTrades` |
| `get_klines(symbol, interval, start, end, limit)` | `GET /fapi/v3/klines` |
| `get_index_price_klines(pair, interval, start, end, limit)` | `GET /fapi/v3/indexPriceKlines` |
| `get_mark_price_klines(symbol, interval, start, end, limit)` | `GET /fapi/v3/markPriceKlines` |
| `get_premium_index_klines(symbol, interval, start, end, limit)` | `GET /fapi/v3/premiumIndexKlines` (live, not in the docs) |
| `get_mark_prices()` / `get_mark_price(symbol)` | `GET /fapi/v3/premiumIndex` (all / one symbol) |
| `get_historical_funding_rates(symbol, start, end)` | `GET /fapi/v3/fundingRate`. Pages by `startTime` with `limit=1000` (documented max). |
| `get_funding_info(symbol)` | `GET /fapi/v3/fundingInfo`. `None` returns every symbol. |
| `get_tickers()` / `get_ticker(symbol)` | `GET /fapi/v3/ticker/24hr` (all, weight 40 / one, weight 1) |
| `get_price_tickers()` / `get_price_ticker(symbol)` | `GET /fapi/v3/ticker/price` |
| `get_book_tickers()` / `get_book_ticker(symbol)` | `GET /fapi/v3/ticker/bookTicker` |
| `get_index_references(symbol)` | `GET /fapi/v3/indexreferences` |
| `get_remaining_openable_notional(symbol, leverage)` | `GET /fapi/v3/remainingOpenableNotionalValue` (weight 50) |
| `get_open_interest(symbol)` | `GET /fapi/v3/openInterest` (live, not in the docs) |

Non-2xx responses return an error with the exchange `code` and `msg` (for example
`-1121 Invalid symbol.`).

Differences between the docs and the live API (2026-11-16):

- `indexPriceKlines` and `markPriceKlines` carry a docs note that they "could not be verified" and
  that only `marketKlines` exists. Live, both answer, and `marketKlines` returns `-1121`.
- `ticker/24hr` does not send the documented `prevClosePrice`.
- `ticker/bookTicker` also sends `lastUpdateId`. `exchangeInfo` sends more fields than the docs
  (`createTime`, `symbolType`, `tradingMode`, `name`, `channel`, `sequenceNo`, `twapMinNotional`,
  `imn`, `tags`, asset `assetType`, `PERCENT_PRICE.ltMultiplierUp/Down`), sends `multiplierDecimal`
  and `autoAssetExchange` as strings, and has `contractType: ""` on symbols not trading yet.
- `fundingInfo` sends `fundingFeeCap`/`fundingFeeFloor` as JSON numbers (`f64`).

## Coverage: futures WebSocket (`AsterClient`)

| Methods | Stream | Parsed as |
|---|---|---|
| `subscribe_depth` / `unsubscribe_depth` | `<symbol>@depth[@500ms\|@100ms]` | `AsterWsMessage::Depth` |
| `subscribe_partial_depth` / `unsubscribe_partial_depth` | `<symbol>@depth<5\|10\|20>[@500ms\|@100ms]` | `AsterWsMessage::Depth` |
| `subscribe_trades` / `unsubscribe_trades` | `<symbol>@aggTrade` | `AsterWsMessage::AggTrade` |
| `subscribe_mark_price` / `unsubscribe_mark_price` | `<symbol>@markPrice[@1s]` | `AsterWsMessage::MarkPrice` |
| `subscribe_all_mark_prices` / `unsubscribe_all_mark_prices` | `!markPrice@arr[@1s]` | `AsterWsMessage::MarkPriceArr` |
| `subscribe_klines` / `unsubscribe_klines` | `<symbol>@kline_<interval>` | `AsterWsMessage::Kline` |
| `subscribe_mini_tickers` / `unsubscribe_mini_tickers` | `<symbol>@miniTicker` | `AsterWsMessage::MiniTicker` |
| `subscribe_all_mini_tickers` / `unsubscribe_all_mini_tickers` | `!miniTicker@arr` | `AsterWsMessage::MiniTickerArr` |
| `subscribe_tickers` / `unsubscribe_tickers` | `<symbol>@ticker` | `AsterWsMessage::Ticker` |
| `subscribe_all_tickers` / `unsubscribe_all_tickers` | `!ticker@arr` | `AsterWsMessage::TickerArr` |
| `subscribe_book_tickers` / `unsubscribe_book_tickers` | `<symbol>@bookTicker` | `AsterWsMessage::BookTicker` |
| `subscribe_all_book_tickers` / `unsubscribe_all_book_tickers` | `!bookTicker` | `AsterWsMessage::BookTicker` |
| `subscribe_liquidations` / `unsubscribe_liquidations` | `<symbol>@forceOrder` | `AsterWsMessage::ForceOrder` |
| `subscribe_all_liquidations` / `unsubscribe_all_liquidations` | `!forceOrder@arr` | `AsterWsMessage::ForceOrder` (one object per event live, not an array) |
| `list_subscriptions`, `get_combined`, `set_combined` | `LIST_SUBSCRIPTIONS`, `GET_PROPERTY`, `SET_PROPERTY` | `AsterWsMessage::Response` |
| `AsterClient::with_combined_stream()`, `types::combined_stream_url` | `/stream`, `/stream?streams=a/b` | `AsterWsMessage::Combined { stream, data }` |

`AsterWsMessage::parse` also returns `Error { id, code, msg }`. Live errors are nested
(`{"id":5,"error":{"code":2,"msg":..}}`), not flat as in the docs. A `SUBSCRIBE` to an unknown
stream is acknowledged with `result: null`.

| WebSocket rule (from docs) | How the crate handles it |
|---|---|
| Bare host rejects the connection (close 3001 "illegal request") | `ASTER_WSS_URL` is `/ws`; `ASTER_COMBINED_WSS_URL` is `/stream`. Both accept `SUBSCRIBE` with no streams in the URL. |
| 10 incoming messages/s per connection; 200 streams per connection | Each `subscribe_*` call sends one `SUBSCRIBE` with every symbol in `params`. The caller must keep the total under 200 streams. |
| The server sends ping frames, and unsolicited pongs are allowed. There is no JSON `ping` method. | `ws_config()` sends WebSocket ping frames (`use_text_ping: false`). |
| Connections are closed at 24 h | `roshar-ws-mgr` reconnects. The caller must subscribe again. |
| Request `id` | `subscribe_*` always send `id: 1`; the control helpers take the `id`. |

| Rate limit | Value |
|---|---|
| `REQUEST_WEIGHT` per IP | 2400 per minute (`/fapi/v3/exchangeInfo`, live) |
| `ORDERS` | 1200 per minute and 300 per 10 s. Not used: the crate places no orders. |

`AsterClient::new()` allows 10 requests per second. `RateLimitedClient` counts requests, not
weight. The heavy calls are `get_tickers` (weight 40), `get_remaining_openable_notional`
(weight 50), `get_historical_trades` and `get_agg_trades` (weight 20), depth with limit 1000
(weight 20) and klines with limit over 1000 (weight 10). Use `new_with_rate_limit` if you make many
of them.

## Coverage: spot (`spot::AsterSpotClient`)

| Item | Endpoint / channel | Methods |
|---|---|---|
| Exchange info, ping, time | `GET /api/v3/exchangeInfo`, `/api/v3/ping`, `/api/v3/time` | `get_exchange_info`, `ping`, `get_server_time` |
| Order book | `GET /api/v3/depth` | `get_orderbook` |
| Trades | `GET /api/v3/trades`, `/api/v3/historicalTrades`, `/api/v3/aggTrades` | `get_trades`, `get_historical_trades`, `get_agg_trades` |
| Klines | `GET /api/v3/klines` | `get_klines` |
| Tickers | `GET /api/v3/ticker/24hr`, `/ticker/price`, `/ticker/bookTicker` (one + all) | `get_ticker_24hr`/`get_tickers_24hr`, `get_price_ticker`/`get_price_tickers`, `get_book_ticker`/`get_book_tickers` |
| WS trades | `<symbol>@aggTrade`, `<symbol>@trade` | `subscribe_agg_trades`, `subscribe_trades` |
| WS klines | `<symbol>@kline_<interval>` | `subscribe_klines` |
| WS tickers | `<symbol>@miniTicker`, `!miniTicker@arr`, `<symbol>@ticker`, `!ticker@arr`, `<symbol>@bookTicker`, `!bookTicker` | `subscribe_mini_tickers`/`subscribe_all_mini_tickers`, `subscribe_tickers`/`subscribe_all_tickers`, `subscribe_book_tickers`/`subscribe_all_book_tickers` |
| WS depth | `<symbol>@depth<5\|10\|20>[@100ms]`, `<symbol>@depth[@100ms]` | `subscribe_partial_depth`, `subscribe_depth` |
| WS control | `LIST_SUBSCRIPTIONS`, `GET_PROPERTY`, `SET_PROPERTY` | `list_subscriptions`, `get_combined`, `set_combined` |
| WS parsing | raw and combined (`/stream?streams=`) frames, request results and errors | `AsterSpotWsMessage::parse`, `AsterSpotWsCombinedMessage::parse`, `spot::combined_stream_url` |

Every `subscribe_*` has an `unsubscribe_*`. Spot live differences: `depth` also sends `E`, `T`,
`symbol`; klines rows have 12 elements (docs 11); partial depth uses `b`/`a` (docs `bids`/`asks`);
`bookTicker` events carry `e`/`E`/`T`; `ticker/24hr` (all) omits `baseAsset`, `quoteAsset` and
bid/ask fields for symbols not in `exchangeInfo`, so those are `Option`.

## Not covered yet

- Futures `GET /fapi/v3/continuousKlines` answers live but is not in the V3 docs; for perpetuals it
  matches `klines`.
- Spot `GET /api/v3/commissionRate`: needs a signed request (unauthenticated call returns `-1102`
  missing `nonce`), so it is account data.
- Spot `GET /api/v3/ticker/opt/24hr`: undocumented; live returns a different CoinGecko-style schema
  (`ticker_id`, `base_volume`, ...).
- Open interest history, insurance fund and REST liquidation history: not in the V3 docs, and
  `openInterestHist`, `futures/data/openInterestHist`, `allForceOrders` and `insuranceBalance` return
  404 live.
- Everything authenticated. This includes trading, account data and user data streams, which use
  V3 agent-wallet EIP-712 signing (`user`, `signer`, `nonce`). Since 2026-09-01 these also need the
  main wallet to have made a deposit.
