# roshar-pacifica

Pacifica (Solana perps DEX) client: public market data over REST, websocket subscription messages
for `roshar-ws-mgr`, and Ed25519-signed limit order create / cancel / edit plus open orders and
positions.

## API docs

| Area | Reference |
|---|---|
| REST | https://docs.pacifica.fi/api-documentation/api/rest-api |
| WebSocket | https://docs.pacifica.fi/api-documentation/api/websocket |
| Auth / signing | https://docs.pacifica.fi/api-documentation/api/signing/implementation (reference: https://github.com/pacifica-fi/python-sdk `common/utils.py`) |
| Rate limits | https://docs.pacifica.fi/api-documentation/api/rate-limits |
| Errors | https://docs.pacifica.fi/api-documentation/api/error-codes |
| Changelog | https://docs.pacifica.fi/api-documentation/changelog |

Checked against these on 2026-11-16 (page index: https://docs.pacifica.fi/llms.txt).

| Network | REST | WebSocket |
|---|---|---|
| Mainnet | `https://api.pacifica.fi/api/v1` | `wss://ws.pacifica.fi/ws` |
| Testnet | `https://test-api.pacifica.fi/api/v1` | `wss://test-ws.pacifica.fi/ws` |

## Coverage

| Item | Endpoint / channel |
|---|---|
| `get_markets` | `GET /info` |
| `get_tickers` | `GET /info/prices` (keyed by symbol) |
| `get_funding_rate_history` | `GET /funding_rate/history` (`limit`, `cursor` pagination) |
| `get_orderbook` | `GET /book` (`agg_level`) |
| `get_recent_trades` | `GET /trades` (`symbol` optional: `None` = all markets) |
| `get_klines` | `GET /kline` (`start_time` required) |
| `get_mark_price_klines` | `GET /kline/mark` (`start_time` required) |
| `get_spot_assets` | `GET /spot_assets` (`include_inactive`, `collateral_enabled_only`) |
| `create_limit_order` | `POST /orders/create`, type `create_order` |
| `cancel_order` | `POST /orders/cancel`, type `cancel_order` (by `order_id`) |
| `edit_order` | `POST /orders/edit`, type `edit_order` (by `order_id`) |
| `get_open_orders` | `GET /orders?account=` |
| `get_positions` | `GET /positions?account=` |
| `subscribe_depth` / `unsubscribe_depth` | `book` (`symbol`, `agg_level` required) |
| `subscribe_trades` / `unsubscribe_trades` | `trades` |
| `subscribe_bbo` / `unsubscribe_bbo` | `bbo` |
| `subscribe_candles` / `unsubscribe_candles` | `candle` |
| `subscribe_mark_price_candles` / `unsubscribe_mark_price_candles` | `mark_price_candle` |
| `subscribe_prices` / `unsubscribe_prices` | `prices` (all symbols) |
| ping | `{"method":"ping"}` every 30 s (server drops after 60 s idle) |
| `PacificaWsMessage::parse` | `subscribe` / `unsubscribe` acks, `pong`, `book`, `bbo`, `trades`, `candle`, `mark_price_candle`, `prices`, and both error shapes (`{"error"}`, `{"code","err","t"}`) |
| `use_ws_deflate` + `PacificaInflater` | `?compress=deflate`: binary frames of raw DEFLATE with state shared across the connection. Feed every `Message::BinaryMessage` to one inflater, `reset()` it on `SuccessfulHandshake` |

Every REST reply is the `{"success","data","error","code"}` envelope; a non-2xx status,
`success: false` or missing `data` is an error.

## Signing

The message is `{"data": <payload>, "expiry_window", "timestamp", "type"}` with keys sorted at every
level, compact separators and Python `json.dumps` ASCII escaping, signed with Ed25519 and sent as
base58. The request body is `account`, `signature`, `timestamp`, `expiry_window`, plus the payload
fields at the top level. `agent_wallet` is added only when the key is an API agent key: the
exchange rejects `agent_wallet == account`. `expiry_window_ms` is the caller's choice
(`new_with_credentials`); the exchange's default when omitted is 30 000.

`tests/vectors.json` holds messages, signatures and request bodies produced by the official Python
SDK's `sign_message` (commit `126ad919`) with throwaway keys and no network, via
`tests/gen_vectors.py`. The unit tests in `src/rest/trading.rs` check the message, signature, request
body and payload builders byte-for-byte. Never regenerate the vectors to make a change pass.

## Rate limits

Pacifica charges credits per 60 s: 100 for an unidentified IP, 300 with an API config key, more by
fee tier. A cancel costs 0.5, heavy GETs 3-12. The default client allows 5 requests per 3 s (100 per
60 s), shared by market and trading calls; `new_with_rate_limit(requests, interval_secs, ..)` sets
another rate. The limiter counts requests, not credits.

## Not covered yet

- Market, stop and TP/SL orders, cancel-all, cancel by `client_order_id`, batch orders, order
  strategies, TWAP, Print orders; `order_expiry` and `builder_code` on limit orders.
- Account info / settings, leverage and margin mode, trade / funding / order history, withdrawals,
  subaccounts, vaults (lakes), spot balances, API agent key binding.
- Account websocket channels and websocket trading operations (private).
- `PF-API-KEY` API config key header (higher rate limits).
- `last_order_id` (top-level sibling of `data` in `GET /trades`) is not returned.
- Fee levels, loan pool, bridge info / parameters: not market data under this repo's scope.
