# roshar-bitget

Bitget public market data, classic (v2) API: futures (`/api/v2/mix/market/*`) and spot
(`/api/v2/spot/*`) REST, plus the public websocket (`wss://ws.bitget.com/v2/ws/public`) with
subscribe builders and typed frame parsing. `BitgetProductType` selects `USDT-FUTURES`,
`COIN-FUTURES` or `USDC-FUTURES`; `BitgetWsInstType` adds `SPOT` for the websocket.

## API docs

| Area | Reference |
|---|---|
| REST (futures market) | https://www.bitget.com/docs/catalog/classic-contract-market/classic-contract-market |
| REST (spot market) | https://www.bitget.com/docs/catalog/classic-spot-market/classic-spot-market |
| REST (server time) | https://www.bitget.com/docs/catalog/classic-common-public |
| WebSocket | https://www.bitget.com/docs/classic/websocket/intro; futures [Tickers](https://www.bitget.com/docs/classic/websocket/contract/public/Tickers-Channel), [Candlesticks](https://www.bitget.com/docs/classic/websocket/contract/public/Candlesticks-Channel), [Order book](https://www.bitget.com/docs/classic/websocket/contract/public/Order-Book-Channel), [Trades](https://www.bitget.com/docs/classic/websocket/contract/public/New-Trades-Channel); spot [Tickers](https://www.bitget.com/docs/classic/websocket/spot/public/Tickers-Channel), [Candlesticks](https://www.bitget.com/docs/classic/websocket/spot/public/Candlesticks-Channel), [Depth](https://www.bitget.com/docs/classic/websocket/spot/public/Depth-Channel), [Auction](https://www.bitget.com/docs/classic/websocket/spot/public/Auction-Channel), [Trades](https://www.bitget.com/docs/classic/websocket/spot/public/Trades-Channel) |
| Rate limits | https://www.bitget.com/docs/classic/rest-api (overall 8000/IP/min; per-endpoint limit on each endpoint's entry) |
| Changelog | https://www.bitget.com/docs/classic/changelog/2026-11 |
| v2 → v3 (UTA) mapping | https://www.bitget.com/docs/classic/uta-api-upgrade-guide |

Checked against these on 2026-11-16. The old `bitget.com/api-doc/...` URLs now all redirect to the
UTA intro page.

## Coverage

All methods are on the `BitgetApi` trait (implemented by `BitgetClient`). Futures and spot share
one rate limiter (default 20 req/s).

| Item | Endpoint / channel |
|---|---|
| `get_server_time()` | `GET /api/v2/public/time` |
| `get_contracts(product_type, symbol?)` | `GET /api/v2/mix/market/contracts` (20/s) |
| `get_tickers(product_type)` | `GET /api/v2/mix/market/tickers` (20/s) |
| `get_ticker(product_type, symbol)` | `GET /api/v2/mix/market/ticker` (20/s) |
| `get_merge_depth(product_type, symbol, precision?, limit?)` | `GET /api/v2/mix/market/merge-depth` (20/s) |
| `get_recent_trades(product_type, symbol, limit?)` | `GET /api/v2/mix/market/fills` (20/s) |
| `get_trades_history(product_type, symbol, &BitgetFillsHistoryParams)` | `GET /api/v2/mix/market/fills-history` (10/s) |
| `get_candles(product_type, symbol, granularity, kline_type?, BitgetCandleParams)` | `GET /api/v2/mix/market/candles` (market/mark/index, 20/s) |
| `get_history_candles(...)` | `GET /api/v2/mix/market/history-candles` (20/s) |
| `get_history_index_candles(...)` | `GET /api/v2/mix/market/history-index-candles` (20/s) |
| `get_history_mark_candles(...)` | `GET /api/v2/mix/market/history-mark-candles` (20/s) |
| `get_open_interest(product_type, symbol)` | `GET /api/v2/mix/market/open-interest` (20/s) |
| `get_current_funding_rates(product_type, symbol?)` | `GET /api/v2/mix/market/current-fund-rate` (20/s) |
| `get_historical_funding_rates(product_type, symbol, page_size, page_no)` | `GET /api/v2/mix/market/history-fund-rate` (20/s, `pageSize` ≤ 100) |
| `get_next_funding_time(product_type, symbol)` | `GET /api/v2/mix/market/funding-time` (20/s) |
| `get_symbol_price(product_type, symbol)` | `GET /api/v2/mix/market/symbol-price` (last/index/mark, 20/s) |
| `get_oi_limits(product_type, symbol?)` | `GET /api/v2/mix/market/oi-limit` (10/s) |
| `get_spot_symbols(symbol?)` | `GET /api/v2/spot/public/symbols` (20/s) |
| `get_spot_tickers(symbol?)` | `GET /api/v2/spot/market/tickers` (20/s) |
| `get_spot_orderbook(symbol, step?, limit?)` | `GET /api/v2/spot/market/orderbook` (20/s) |
| `get_spot_merge_depth(symbol, precision?, limit?)` | `GET /api/v2/spot/market/merge-depth` (20/s) |
| `get_spot_recent_trades(symbol, limit?)` | `GET /api/v2/spot/market/fills` (10/s) |
| `get_spot_trades_history(symbol, &BitgetFillsHistoryParams)` | `GET /api/v2/spot/market/fills-history` (10/s) |
| `get_spot_candles(symbol, granularity, start?, end?, limit?)` | `GET /api/v2/spot/market/candles` (20/s) |
| `get_spot_history_candles(symbol, granularity, end_time, limit?)` | `GET /api/v2/spot/market/history-candles` (20/s) |
| `get_spot_auction(symbol)` | `GET /api/v2/spot/market/auction` (20/s) |
| `BitgetClient::ws_config()` | `wss://ws.bitget.com/v2/ws/public`, text `ping` every 30 s |
| `subscribe_depth` / `subscribe_books(.., BitgetBookChannel, ..)` | `books`, `books1`, `books5`, `books15` (futures and spot) |
| `subscribe_trades` | `trade` (futures and spot) |
| `subscribe_tickers` | `ticker` (futures and spot) |
| `subscribe_candles(.., interval, ..)` | `candle<interval>` (futures and spot) |
| `subscribe_auction` | spot `auction` |
| `BitgetWsIncoming::parse(text)` | `pong`, subscribe/unsubscribe acks, `error` events, and every channel above |

Each `subscribe_*` has an `unsubscribe_*`. `BitgetWssMessage::subscribe`/`unsubscribe` build a
request for any channel name.

REST replies are parsed strictly. A `code` other than `00000` is returned as an error with
Bitget's `msg` (error replies carry `data: null`). Websocket `error` events become
`BitgetWsIncoming::Error` with `code`/`msg` (the code arrives as a JSON number live, a string in
the docs; both are accepted). Unknown events or channels are parse errors.

Notes from live checks (UK IP, 2026-11-16):

- `COIN-FUTURES` contracts and tickers return `00000` with an empty `data` list. USDT-M and USDC-M
  work.
- `merge-depth` levels are JSON numbers (parsed as `f64`). Spot `orderbook` levels are strings
  for `type=step0` and JSON numbers for `step1`..`step5`; both are read into strings. Websocket
  levels are strings.
- Spot `orderbook` and `merge-depth` for the tokenized-stock symbols (`R...USDT`) return `40502
  Please upgrade to access the V3 API.` on v2.
- `kLineType` must be lowercase (`market`, `mark`, `index`). Bitget treats uppercase values as
  `market` without an error.
- `fills` returns `side` lowercase, `fills-history` returns `Buy`/`Sell`.
- Fields that are always sent but can be `null` are `Option` and the key is still required:
  `oi-limit` `singlePositionRatio`/`totalPositionRatio` (not in the docs, `null` on all 934
  contracts), `current-fund-rate` `min/maxFundingRate` (`null` on 21 of 955), the futures ticker
  `deliveryStartTime`/`deliveryTime` (`null` on perpetuals), spot ticker `bidSz`/`askSz` (`null`
  on about 20 of 3666 symbols with an empty book side), and every `auction` field (`null` without
  an auction).
- Fields that can be absent are `Option` with a default: the futures `ticker` push
  `deliveryPrice` (documented, but absent from all 11144 perpetual pushes captured), and the spot
  `ticker` push `bidPr`/`bidSz`/`askPr`/`askSz` (left out when that book side is empty).
- From 2026-11-17 (live), Bitget stops sending a snapshot after subscribing to `trade` and
  `candle*` (futures and spot); only `update` pushes arrive. On 2026-11-16 the snapshot was still
  sent. Use `get_recent_trades`/`get_candles` for history.

## v2 (classic) vs v3 (UTA)

This crate uses the classic v2 API. v2 is still documented and live, and its public market data
works without an account. v3 (`/api/v3/market/*`, websocket `instType`/`topic` scheme) is a
separate API generation with a different request and push format, so it would be a second
protocol in this crate. The v3 public market endpoints that duplicate v2 (instruments, tickers,
orderbook, candles, fills, funding, open interest, OI limit, time) are not added.

## Not covered yet

- v3-only public market data, which has no v2 equivalent: REST `liquidations`,
  `open-interest-history`, `risk-reserve`/`risk-reserve-all`/`risk-reserve-hour` (insurance fund),
  `position-tier`, `index-components`, `rpi-orderbook`/`rpi-symbols`, and the v3 websocket
  `liquidation`, `funding-rate`, `instruments`, `open-interest-limit`, `position-tier` and RPI
  order book topics. Adding these means implementing the v3 protocol (see above).
- VIP fee rates (`/api/v2/mix/market/vip-fee-rate`, `/api/v2/spot/market/vip-fee-rate`): fee
  schedule, not market data.
- Union-margin collateral and interest data (`/api/v2/mix/market/discount-rate`, `exchange-rate`,
  `union-interest-rate-history`): borrowing/lending.
- Spot coin info (`/api/v2/spot/public/coins`): deposit/withdrawal chain configuration.
- The symbol rename notice push (changelog 2026-11, `ANTHROPICUSDT` → `ANTUSDT`): the frame format
  is not documented and could not be captured, so `BitgetWsIncoming::parse` returns an error for
  it.
- Margin and all private (signed) REST and websocket endpoints.
