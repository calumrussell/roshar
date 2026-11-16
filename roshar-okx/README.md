# roshar-okx

OKX (Global site) public market data over REST, the public WebSocket (`/ws/v5/public`) and the
business WebSocket (`/ws/v5/business`), with configs for roshar-ws-mgr. No auth, no trading.

## API docs

| Area | Reference |
|---|---|
| REST market data | https://www.okx.com/docs-v5/en/#order-book-trading-market-data |
| REST public data | https://www.okx.com/docs-v5/en/#public-data-rest-api |
| System status | https://www.okx.com/docs-v5/en/#status |
| WebSocket | https://www.okx.com/docs-v5/en/#overview-websocket-connect, https://www.okx.com/docs-v5/en/#order-book-trading-market-data-ws-order-book-channel, https://www.okx.com/docs-v5/en/#public-data-websocket |
| Endpoints / domains | https://www.okx.com/docs-v5/en/#overview-production-trading-services |
| Rate limits | https://www.okx.com/docs-v5/en/#overview-rate-limits (per-endpoint limits are on each endpoint) |
| Changelog | https://www.okx.com/docs-v5/log_en/ |
| UK availability | https://www.okx.com/en-gb/help/okx-what-can-i-use-in-the-united-kingdom |

Checked against these on 2026-11-16.

| Setting | Value |
|---|---|
| REST base | `https://openapi.okx.com` (recommended since 2026-05-20; `www.okx.com` still works) |
| `OKX_WSS_URL` | `wss://ws.okx.com/ws/v5/public` (port 8443 retired 2026-10-31) |
| `OKX_BUSINESS_WSS_URL` | `wss://ws.okx.com/ws/v5/business` (candles, `trades-all`) |
| `OKX_PRIVATE_WSS_URL` | `wss://ws.okx.com/ws/v5/private` (constant only; nothing in the crate uses it) |
| Keepalive | Text `ping` every 20 s, expect `pong`. OKX drops a connection after 30 s without data. |
| REST rate limit | One bucket, 10 req/s by default. OKX limits are per endpoint and per IP (or IP + instrument), from 1 to 50 req/s. Most are 20 / 2 s; funding-rate(-history), mark-price, time, insurance-fund, position-tiers, estimated-price/settlement, books-full, history-index-candles and event-contract are 10 / 2 s; instrument-tick-bands 5 / 2 s; platform-24-volume 2 / 2 s. Use `new_with_rate_limit` for a lower rate when bursting those. |

Regional domains: US/AU accounts must use `us.okx.com` and EEA accounts must use `eea.okx.com`.
This crate targets the Global domain only. OKX's UK page lists derivatives, margin and the API as
not available to UK customers. The public market data here is still served to UK IPs (checked live
2026-11-16).

## Coverage

REST: every method is on `OkxApi` (implemented by `OkxClient`) and on `OkxMarketApi`. Optional
parameters are `Option` and omitted when `None`. A non-`"0"` OKX `code` is returned as an error
with the code and message.

| Method | Endpoint |
|---|---|
| `get_instruments_info(inst_type, inst_family, inst_id, series_id)` | `GET /api/v5/public/instruments` (incl. `EVENTS`, which needs `series_id`) |
| `get_tickers` / `get_ticker` | `GET /api/v5/market/tickers`, `GET /api/v5/market/ticker` |
| `get_order_book` / `get_order_book_full` / `get_order_book_rpi` | `GET /api/v5/market/books`, `books-full`, `books-rpi` |
| `get_trades` / `get_history_trades` | `GET /api/v5/market/trades`, `history-trades` |
| `get_candles` / `get_history_candles` | `GET /api/v5/market/candles`, `history-candles` (with `adjust`) |
| `get_mark_price_candles` / `get_history_mark_price_candles` | `GET /api/v5/market/mark-price-candles`, `history-mark-price-candles` |
| `get_index_candles` / `get_history_index_candles` | `GET /api/v5/market/index-candles`, `history-index-candles` |
| `get_index_tickers` / `get_index_components` | `GET /api/v5/market/index-tickers`, `index-components` |
| `get_exchange_rate` / `get_platform_24_volume` | `GET /api/v5/market/exchange-rate`, `platform-24-volume` |
| `get_call_auction_details` | `GET /api/v5/market/call-auction-details` |
| `get_option_family_trades` / `get_option_trades` | `GET /api/v5/market/option/instrument-family-trades`, `GET /api/v5/public/option-trades` |
| `get_opt_summary` | `GET /api/v5/public/opt-summary` |
| `get_funding_rate` / `get_funding_rate_history` | `GET /api/v5/public/funding-rate` (`inst_id` may be `ANY`), `funding-rate-history` |
| `get_premium_history` | `GET /api/v5/public/premium-history` |
| `get_open_interest` | `GET /api/v5/public/open-interest` |
| `get_price_limit` / `get_mark_price` | `GET /api/v5/public/price-limit`, `mark-price` |
| `get_estimated_price` / `get_estimated_settlement_info` | `GET /api/v5/public/estimated-price`, `estimated-settlement-info` (data only 30 min before settlement; OKX error 51001 otherwise) |
| `get_delivery_exercise_history` / `get_settlement_history` | `GET /api/v5/public/delivery-exercise-history`, `settlement-history` |
| `get_insurance_fund` | `GET /api/v5/public/insurance-fund` |
| `get_underlying` / `get_position_tiers` / `get_instrument_tick_bands` | `GET /api/v5/public/underlying`, `position-tiers`, `instrument-tick-bands` |
| `get_event_series` / `get_events` / `get_event_markets` | `GET /api/v5/public/event-contract/series`, `events`, `markets` |
| `get_system_time` / `get_system_status` | `GET /api/v5/public/time`, `GET /api/v5/system/status` |

WebSocket: build channel args with `OkxWssArg` constructors, send them with
`OkxClient::subscribe` / `unsubscribe` (each arg goes to the public or business connection per
`OkxWssArg::is_business`), and parse every inbound frame with `OkxWsMessage::parse` (acks, errors
and notices are `OkxWsMessage::Event`; unknown channels are an error). Add both
`ws_config()` and `business_ws_config()` to the manager if you use channels from both sockets.

| Socket | Channels (`OkxWssArg::…` → `OkxWsMessage::…`) |
|---|---|
| public | `books` / `books_rpi` → `Books` (snapshot + updates with `seq_id`/`prev_seq_id`; deprecated `checksum` not modelled), `books5` → `Books5`, `bbo_tbt` / `bbo_rpi_tbt` → `Bbo`, `trades` → `Trades`, `tickers`, `index_tickers`, `mark_price`, `funding_rate`, `open_interest`, `price_limit`, `estimated_price`, `liquidation_orders`, `instruments`, `opt_summary`, `option_trades`, `call_auction_details`, `status`, `event_contract_markets` → `EventContractMarkets` |
| public, login + VIP4 | `books_l2_tbt`, `books50_l2_tbt`: builders and parsing only (`Books`). OKX answers error 60011 "Please log in" without login; this crate does not log in. |
| business | `candle(bar, ..)` → `Candle`, `mark_price_candle(bar, ..)` → `MarkPriceCandle`, `index_candle(bar, ..)` → `IndexCandle`, `trades_all` → `TradesAll` |

Fixtures under `tests/fixtures/{rest,ws}` are trimmed live captures from 2026-11-16 and back the
offline parse tests.

## Not covered yet

- Spread trading (`/api/v5/market/sprd-*`, `sprd-*` channels) and block trading
  (`block-ticker(s)`, `public/block-trades`, `block-tickers`, `public-block-trades`,
  `public-struc-block-trades`): separate OKX products with their own instruments.
- `GET /api/v5/market/books-sbe` and the SBE (`public-sbe`) feed: binary encoding.
- `GET /api/v5/public/market-data-history`: bulk-download file links, not market data itself.
- `GET /api/v5/rubik/*` trading statistics (long/short ratios, taker volume, OI history from
  rubik): analytics.
- `economic-calendar` (REST and WS): requires login.
- `adl-warning` WS: subscribe succeeds but no frame arrived in 5 minutes of capture, so there is no
  live sample to type it from; account-risk oriented rather than market data.
- `GET /api/v5/public/convert-contract-coin`, `discount-rate-interest-free-quota`,
  `interest-rate-loan-quota`, `delta-hedge-currencies`, `corporate-actions`,
  `mm-instrument-types`: conversion calculators / borrowing / account configuration, not market
  data.
- Regional domains (`us.okx.com`, `eea.okx.com`) and the demo environment (`wspap.okx.com`).
- All private endpoints: auth/signing, account, trading.
