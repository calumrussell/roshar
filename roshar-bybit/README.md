# roshar-bybit

ByBit V5 public market data for every category (`spot`, `linear`, `inverse`, `option`) plus spread
trading: REST market endpoints and system status, and websocket config, subscribe/unsubscribe
requests and typed message parsing for roshar-ws-mgr.

## API docs

| Area | Reference |
|---|---|
| REST (base URLs, regional hosts) | https://bybit-exchange.github.io/docs/v5/guide |
| REST market endpoints (one page each, sidebar "Market") | https://bybit-exchange.github.io/docs/v5/market/time |
| REST spread market | https://bybit-exchange.github.io/docs/v5/spread/market/instrument |
| REST system status | https://bybit-exchange.github.io/docs/v5/system-status |
| WebSocket (URLs, ping, subscribe) | https://bybit-exchange.github.io/docs/v5/ws/connect |
| WebSocket public channels (one page each) | https://bybit-exchange.github.io/docs/v5/websocket/public/orderbook |
| WebSocket spread channels | https://bybit-exchange.github.io/docs/v5/spread/websocket/public/orderbook |
| WebSocket system status | https://bybit-exchange.github.io/docs/v5/websocket/system/system-status |
| Rate limits | https://bybit-exchange.github.io/docs/v5/rate-limit |
| Changelog | https://bybit-exchange.github.io/docs/changelog/v5 |
| Docs source (markdown) | https://github.com/bybit-exchange/docs |

Checked against these on 2026-11-16. All endpoints below answered from a UK IP (ByBit restricts US
and Mainland China IPs with 403).

## Coverage

REST (`ByBitMarketApi`, reachable via `ByBitClient::market()`). Every method returns one page /
one response; paging (`cursor`, `startTime`/`endTime`) and limits are caller parameters, optional
ones are omitted when `None`. Non-zero `retCode` is returned as `ByBit API error {code}: {msg}`.

| Method | Endpoint |
|---|---|
| `get_server_time` | `GET /v5/market/time` |
| `get_system_status` | `GET /v5/system/status` |
| `get_kline` | `GET /v5/market/kline` (spot, linear, inverse) |
| `get_mark_price_kline` | `GET /v5/market/mark-price-kline` (linear, inverse, option) |
| `get_index_price_kline` | `GET /v5/market/index-price-kline` |
| `get_premium_index_price_kline` | `GET /v5/market/premium-index-price-kline` |
| `get_instruments_info` / `get_spot_instruments_info` / `get_option_instruments_info` | `GET /v5/market/instruments-info` with `symbol`, `symbolType`, `status`, `baseCoin`, `limit`, `cursor` (`ByBitInstrumentsQuery`) |
| `get_instrument_summary` | `GET /v5/market/instrument-summary` |
| `get_option_base_coins` | `GET /v5/market/option-base-coins` |
| `get_corporate_actions` | `GET /v5/market/corporate-action` |
| `get_orderbook` | `GET /v5/market/orderbook` (all categories) |
| `get_full_orderbook` | `GET /v5/market/full_orderbook` |
| `get_rpi_orderbook` | `GET /v5/market/rpi_orderbook` |
| `get_tickers` / `get_spot_tickers` / `get_option_tickers` | `GET /v5/market/tickers` |
| `get_funding_rate_history` | `GET /v5/market/funding/history` |
| `get_recent_trades` | `GET /v5/market/recent-trade` (incl. option `mP`/`iP`/`mIv`/`iv`) |
| `get_open_interest` | `GET /v5/market/open-interest` |
| `get_historical_volatility` | `GET /v5/market/historical-volatility` |
| `get_insurance` | `GET /v5/market/insurance` |
| `get_adl_alert` | `GET /v5/market/adlAlert` |
| `get_risk_limit` | `GET /v5/market/risk-limit` |
| `get_delivery_price` / `get_new_delivery_price` | `GET /v5/market/delivery-price`, `/v5/market/new-delivery-price` |
| `get_long_short_ratio` | `GET /v5/market/account-ratio` |
| `get_index_price_components` | `GET /v5/market/index-price-components` |
| `get_order_price_limit` | `GET /v5/market/price-limit` |
| `get_spread_instruments` / `get_spread_orderbook` / `get_spread_recent_trades` / `get_spread_tickers` | `GET /v5/spread/{instrument,orderbook,recent-trade,tickers}` |
| `ByBitApi::get_realtime_funding_rates` | linear tickers keyed by symbol |

WebSocket. `ByBitClient::new_for_stream(ByBitWsStream, rps)` builds the `Config` for one stream
(`new()` = linear): `wss://stream.bybit.com/v5/public/{spot,linear,inverse,option,spread,misc/status}`,
`{"req_id":"100001","op":"ping"}` every 20 s. `subscribe` / `unsubscribe` take `ByBitTopic`s
(one request per topic); `ByBitWssMessage::{subscribe,unsubscribe,ping}` build the raw requests.
`ByBitWsMessage::parse(stream, text)` / `ByBitClient::parse_ws_message` return a typed message;
unknown topics and missing fields are errors.

| `ByBitTopic` | Topic | Parsed as |
|---|---|---|
| `Orderbook` | `orderbook.{1,25,50,100,200,1000}.{symbol}` | `Orderbook(ByBitDepthMessage)` |
| `OrderbookRpi` | `orderbook.rpi.{symbol}` | `OrderbookRpi(ByBitRpiDepthMessage)` |
| `OrderbookFull` | `orderbook.full.{symbol}` (delta only) | `Orderbook(ByBitDepthMessage)` |
| `PublicTrade` | `publicTrade.{symbol}` (option: base coin) | `Trades(ByBitTradesMessage)` |
| `Ticker` | `tickers.{symbol}` | `ContractTicker` (linear/inverse, snapshot + delta), `SpotTicker`, `OptionTicker`, `SpreadTicker` |
| `Kline` | `kline.{interval}.{symbol}` | `Kline(ByBitCandleMessage)` |
| `AllLiquidation` | `allLiquidation.{symbol}` | `Liquidation(ByBitLiquidationMessage)` |
| `InsurancePool` | `insurance.{USDT,USDC,inverse}` | `InsurancePool(ByBitInsurancePoolMessage)` |
| `AdlAlert` | `adlAlert.{USDT,USDC,inverse}` | `AdlAlert(ByBitAdlAlertMessage)` |
| `PriceLimit` | `priceLimit.{symbol}` | `PriceLimit(ByBitPriceLimitMessage)` |
| `SystemStatus` | `system.status` | `SystemStatus(ByBitSystemStatusMessage)` |
| — | control frames | `Response` (spot/linear/inverse), `CommandResponse` + `Pong` (option/spread/system status) |

Fixtures: `tests/fixtures/rest` and `tests/fixtures/ws` are live captures from 2026-11-16, trimmed
to a few rows. `docs_*` files are the documented examples, used where nothing was live: system
status (no maintenance record), spread trades (no spread trades during capture).

Default REST rate is 10 req/s per client, well inside the documented 600 requests / 5 s per IP; pass
another rate to `new_with_rate_limit` / `new_for_stream` or share a `RateLimitedClient` via
`ByBitMarketApi::new_with_client`.

## Not covered yet

- `GET /v5/market/fee-group-info` (contract fee-rate groups): fee schedule, not market data.
- `GET /v5/announcements/*`: out of scope.
- RFQ block-trade data: `GET /v5/rfq/public-trades` needs an API key (live: `retCode 10001 apiKey is
  missing`), and the `wss://stream.bybit.com/v5/public/rfq` stream belongs to that workflow.
- Event Trading stream (`/v5/public/event`): documented as market-maker only.
- Private REST/WS, order entry and HMAC/RSA signing.
