# roshar-derive

Derive (formerly Lyra) v3: public market data over JSON-RPC HTTP POST, and `roshar-ws-mgr` config plus
subscribe/unsubscribe messages for public order book, trade and slim-ticker channels. No auth or trading.

## API docs

| Area | Reference |
|---|---|
| REST | https://docs.derive.xyz/getting-started/introduction (endpoints), https://docs.derive.xyz/json-rpc (envelope), https://docs.derive.xyz/openapi.json (machine-readable) |
| WebSocket | https://docs.derive.xyz/connecting, https://docs.derive.xyz/subscriptions, https://docs.derive.xyz/subscriptions.asyncapi.json |
| Auth / signing | https://docs.derive.xyz/authentication/action-signing (not implemented here) |
| Rate limits | https://docs.derive.xyz/rate-limits |
| Changelog | https://docs.derive.xyz/changelog, v2 → v3: https://docs.derive.xyz/migrating/breaking-changes |
| Reference SDK | https://github.com/derivexyz/derive-rs |

Checked against these on 2026-11-16.

## Coverage

| Item | Endpoint / channel |
|---|---|
| REST base | `https://api.derive.xyz/v3`. POST `/{method}`, body is the `params` object. Replies are `{result}` or `{error}`, both HTTP 200. An `error` is returned as `Err`. |
| `get_all_instruments` | `public/get_all_instruments`: one page plus `pagination`. Caller sets `page`/`page_size` (max 1000). |
| `get_instrument` | `public/get_instrument` |
| `get_all_live_instruments` | `public/get_all_live_instruments`: names only. |
| `get_ticker` / `get_tickers` | `public/get_ticker`, `public/get_tickers`: slim tickers. Options need `currency` and `expiry_date` (YYYYMMDD) on `get_tickers`. |
| `get_trade_history` | `public/get_trade_history`: one page; filters in `DeriveTradeHistoryParams`. |
| `get_all_currencies` / `get_currency` | `public/get_all_currencies`, `public/get_currency` |
| `get_risk_universes` | `public/get_risk_universes` |
| `get_tradingview_chart_data` | `public/get_tradingview_chart_data`: traded-price OHLCV, UTC seconds. |
| `get_index_chart_data` | `public/get_index_chart_data`: spot index OHLC, UTC seconds. |
| `get_funding_rate_history` | `public/get_funding_rate_history`: OHLC candles. `period` is in seconds. |
| `get_interest_rate_history` | `public/get_interest_rate_history`: lending APY candles, UTC ms. |
| `get_option_settlement_prices` | `public/get_option_settlement_prices` |
| `get_perp_impact_twap` | `public/get_perp_impact_twap` |
| `get_latest_signed_feeds` | `public/get_latest_signed_feeds`: oracle spot/forward/vol/rate/perp/funding feeds. |
| `get_liquidation_history` / `get_live_auctions` | `public/get_liquidation_history`, `public/get_live_auctions` |
| `get_time` | `public/get_time` |
| `get_live_incidents` | `public/get_live_incidents`: exchange status. |
| `DERIVE_WSS_URL` / `ws_config()` | `wss://api.derive.xyz/v3/ws`. Protocol-level Ping every 30 s, Pong timeout 60 s. The server drops idle sockets after about 180 s. |
| `DeriveWsMessage::parse` | Typed frames: subscribe/unsubscribe results, errors, and every channel below. |
| `subscribe_trades` / `unsubscribe_trades` | `trades.{instrument_name}` |
| `subscribe_trades_by_type` / `unsubscribe_trades_by_type` | `trades.{instrument_type}.{currency}` |
| `subscribe_depth` / `unsubscribe_depth` | `orderbook.{instrument_name}.{group}.{depth}`: group 1/10/100, depth 1/10/20/100 |
| `subscribe_tickers` / `unsubscribe_tickers` | `ticker_slim.{instrument_name}.{interval}`: interval 100/1000 ms |
| `subscribe_spot_feed` / `unsubscribe_spot_feed` | `spot_feed.{currency}` |
| `subscribe_auctions_watch` / `subscribe_margin_watch` (+ unsubscribe) | `auctions.watch`, `margin.watch` |
| Default rate limit | 5 req/s. This is the documented public per-IP budget (5 TPS, 25 points per 5 s window). |

## Not covered yet

- `auctions.watch` and `margin.watch` published nothing during a 7-minute capture; their payloads follow the asyncapi
  spec (single object or array accepted) and `margin.watch` positions/collaterals stay raw JSON (untyped in the spec).
- `public/getRateLimits` (caller's own rate budget, not market data), `public/get_maker_program*`, referral, vault,
  wallet/subaccount, transaction, deposit and debug methods.
- `block_proposals` channel and block-proposal methods (RFQ/block trading).
- Authentication (`public/login`, `X-Derive*` headers, session keys), EIP-712 action signing, private
  channels, trading, RFQ and vaults.
