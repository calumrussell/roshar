# roshar-kucoin

KuCoin Futures (Classic API) public market data: contracts, tickers, order books, trades,
klines, mark/index/interest/premium indices, funding, server time/status, open interest (UTA
endpoint), and the public websocket (token, every public channel, typed frame parsing). No
trading, no signing.

## API docs

| Area | Reference |
|---|---|
| REST: market data | https://www.kucoin.com/docs-new/rest/futures-trading/market-data/get-symbol (and the sibling pages under Futures Trading > Market Data) |
| REST: funding | https://www.kucoin.com/docs-new/rest/futures-trading/funding-fees/get-current-funding-rate, https://www.kucoin.com/docs-new/rest/futures-trading/funding-fees/get-public-funding-history |
| REST: open interest (UTA) | https://www.kucoin.com/docs-new/v2/rest/ua/get-futures-open-interest |
| WebSocket | https://www.kucoin.com/docs-new/websocket-api/base-info/introduction |
| WebSocket token | https://www.kucoin.com/docs-new/websocket-api/base-info/get-public-token-futures |
| WebSocket channels | https://www.kucoin.com/docs-new/3470080w0 (Ticker V2), 3470081w0 (Ticker V1), 3470082w0 (Orderbook - Increment), 3470083w0 (Level 5), 3470097w0 (Level 50), 3470084w0 (Trade), 3470086w0 (Klines), 3470087w0 (Instrument), 3470088w0 (Funding Fee Settlement), 3470089w0 (Symbol Snapshot) |
| Index of all pages | https://www.kucoin.com/docs-new/llms.txt |
| Rate limits | https://www.kucoin.com/docs-new/rate-limit-rule-classic |
| Changelog | https://www.kucoin.com/docs-new/change-log |

Checked against these on 2026-11-16.

## Coverage

REST (`KuCoinApi`, base `https://api-futures.kucoin.com` unless stated; weights from the docs):

| Item | Endpoint |
|---|---|
| `get_contracts` | `GET /api/v1/contracts/active` (3) |
| `get_contract` | `GET /api/v1/contracts/{symbol}` (3) |
| `get_ticker` | `GET /api/v1/ticker?symbol` (2) |
| `get_all_tickers` | `GET /api/v1/allTickers` (5) |
| `get_full_order_book` | `GET /api/v1/level2/snapshot?symbol` (3), seeds `level2` via `sequence` |
| `get_part_order_book` | `GET /api/v1/level2/depth{20,100}?symbol` (5) |
| `get_trade_history` | `GET /api/v1/trade/history?symbol` (5), latest 100 |
| `get_klines` | `GET /api/v1/kline/query?symbol&granularity[&from&to]` (3). Contract, index (`indexSymbol`) and premium (`premiumsSymbol1M/8H`) symbols |
| `get_mark_price` | `GET /api/v1/mark-price/{symbol}/current` (3) |
| `get_spot_index_price` | `GET /api/v1/index/query` (2), `KuCoinIndexQuery` paging params |
| `get_interest_rate_index` | `GET /api/v1/interest/query` (5) |
| `get_premium_index` | `GET /api/v1/premium/query` (3) |
| `get_current_funding_rate` | `GET /api/v1/funding-rate/{symbol}/current` (2) |
| `get_historical_funding_rates` | `GET /api/v1/contract/funding-rates?symbol&from&to` (5 per page) |
| `get_server_time` | `GET /api/v1/timestamp` (2) |
| `get_service_status` | `GET /api/v1/status` (4) |
| `get_open_interest` | `GET https://api.kucoin.com/api/ua/v2/market/open-interest[?symbol=a,b]` (10), real-time |
| `get_open_interest_history` | same, `?symbol&interval[&startAt&endAt&pageSize]` (10) |

WebSocket (`KuCoinClient`; frames parsed by `types::KuCoinWsEvent::parse`):

| Item | Topic / behaviour |
|---|---|
| `get_ws_url` / `get_ws_token` | `POST /api/v1/bullet-public` (10) |
| `apply_ws_server(token, server)` | Explicit: sets `url` and `ping_duration`/`ping_timeout` from a bullet-public instance server |
| `subscribe` / `unsubscribe(channel, symbols)` | `KuCoinChannel`: `tickerV2`, `ticker`, `level2`, `level2Depth5`, `level2Depth50`, `execution`, `/contract/instrument`, `snapshot`; one request with comma-separated symbols |
| `subscribe_depth` / `subscribe_trades` (+ unsub) | Shorthands for `level2` / `execution` |
| `subscribe_candles` / `unsubscribe_candles` | `/contractMarket/limitCandle:{symbol}_{type},...` |
| `subscribe_funding_settlement` / unsub | `/contract/announcement` (`None`) or `/contract/announcement:{symbols}` |
| `ws_config()` | Text ping `{"id":..,"type":"ping"}` every 18 s, 10 s timeout (current bullet-public values) |
| `KuCoinWsEvent` | `welcome`, `ack`, `pong`, `error` (code + message), and typed data for every channel/subject above |

Notes:

- REST envelope: success is `{"code":"200000","data":...}`. Errors are `{"code":"<other>","msg":...}`
  with no `data`, and they can arrive with HTTP 200. The `code` is checked before `data` is read.
- Funding history returns a capped page (100 records observed; the cap is not documented) of the
  newest records in `[from, to]` (inclusive), newest first, and `data: null` for an empty window.
  The crate pages backwards and returns ascending order.
- `KUCOIN_FUTURES_WSS_URL` is the untokenised host. Connecting needs the URL from `get_ws_url`; set
  it with `ws_config_mut().url`, or call `get_ws_token` then `apply_ws_server` to also take the
  server's ping interval/timeout. The token is valid for 24 h, and the server drops the connection
  after that. `roshar-ws-mgr` reconnects with the stored URL, so fetch a new URL and recreate the
  connection instead.
- Default REST rate: 6 req/s. The Public pool is 2000 weight per 30 s per IP, and the heaviest
  endpoints (bullet-public, open interest) weigh 10.
- UK: the United Kingdom is not one of the Restricted Locations in the
  [Terms of Use](https://www.kucoin.com/legal/terms-of-use) (updated 2026-10-29). KuCoin is not
  FCA-registered, and the FCA bans the sale of crypto derivatives to UK retail clients. All
  endpoints above answered from a UK IP without a geo-block.
- Classic vs UTA: the changelog puts Classic *account* compatibility in the UTA API into
  maintenance mode and recommends either staying on the Classic API for Classic accounts or
  moving to UTA. It does not deprecate the Classic Futures market-data endpoints; all of them
  are still listed under "Classic REST > Futures Trading > Market Data" and answer live. This
  crate stays on Classic. The one exception is open interest: Classic only exposes the current
  value (`openInterest` on contracts), so current and historical OI come from the UTA REST V2
  endpoint (same `code`/`data` envelope, Public pool).
- Multi-symbol topics are sent as one request; the caller keeps each list within 100 symbols.
- `level2Depth50` pushes can hold fewer than 50 levels per side (32 bids observed).

## Not covered yet

- Trading, account, position and private websocket channels (these need HMAC signing).
- `GET /api/v1/trade-statistics` (Get 24hr stats, platform turnover): documented as public
  (`security: []`), but answers `400001` "Please check the header of your request for
  KC-API-KEY..." without credentials (fixture `trade_statistics_error.json`). Per-symbol 24 h
  stats come from `get_contract`/`get_contracts` and the `snapshot` channel.
- Mark-price klines: not documented for Classic (`kline/query` lists contract, index and premium
  symbols only).
- Funding-settlement (`/contract/announcement`) pushes arrive only at settlement time; the
  subscription is acknowledged live but the data type is built from the documented example.
- The rest of the UTA API (`api.kucoin.com/api/ua/...`, `x-push-futures.kucoin.com`): a second
  API for the same markets, not needed for Classic market data (see above).
