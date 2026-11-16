# roshar-paradex

Paradex (Starknet appchain perps, options and spot) client: public market data (REST + websocket
config), SBE decoding of websocket payloads, Stark-curve signing, authenticated REST trading and
private websocket channels.

## API docs

| Area | Reference |
|---|---|
| REST | https://docs.paradex.trade/api/general-information/api-urls (base URL); endpoint pages under https://docs.paradex.trade/api/prod/markets/get-markets, https://docs.paradex.trade/api/prod/system/get-config, https://docs.paradex.trade/api/prod/insurance/get-insurance-fund, https://docs.paradex.trade/api/prod/orders/new, https://docs.paradex.trade/api/prod/account/list-fills; index https://docs.paradex.trade/llms.txt |
| WebSocket | https://docs.paradex.trade/ws/general-information/introduction, https://docs.paradex.trade/ws/general-information/subscription-channels, https://docs.paradex.trade/ws/web-socket-channels/markets-summary-market-symbol/markets-summary-market-symbol, https://docs.paradex.trade/ws/web-socket-channels/modify-subscription/modify-subscription, https://docs.paradex.trade/ws/web-socket-channels/orders-market-symbol/orders-market-symbol |
| WebSocket encoding (SBE) | https://docs.paradex.trade/ws/general-information/binary-encoding-sbe; schema https://github.com/tradeparadex/paradex-py/blob/main/paradex_py/api/sbe/paradex_1_0.xml (pinned in `schema/`) |
| Auth / signing | https://docs.paradex.trade/api/general-information/authentication, https://docs.paradex.trade/api/prod/auth/get-jwt, https://docs.paradex.trade/ws/general-information/authentication; reference signer https://github.com/tradeparadex/paradex-py |
| Rate limits | https://docs.paradex.trade/api/general-information/rate-limits/api, https://docs.paradex.trade/ws/general-information/rate-limits |
| Changelog | https://docs.paradex.trade/releases/changelog |

Checked against these on 2026-11-16.

## Principles

- **Mirror the exchange.** Typed-data hashes and signatures match paradex-py byte for byte. REST
  replies and SBE frames are parsed strictly: a missing field, unknown enum value or truncated
  frame is an error.
- **No caller policy.** Signature timestamps, JWT expiries, client order ids, request ids, JWT
  refresh and retries belong to the caller. Credentials are passed in. Nothing is read from the
  environment or the clock.

## Modules

| Module | What it does |
|---|---|
| crate root | `ParadexClient` (market REST via `ParadexApi`, `ws_config()` for roshar-ws-mgr, public subscribe helpers), `PARADEX_WSS_URL`, `PARADEX_TESTNET_WSS_URL`. |
| `types` | Public REST payloads, `ParadexWssMessage` (subscribe / unsubscribe / `modify_subscription` builders), `ParadexBookFeed`, `ParadexPageQuery`. |
| `sbe` | `decode(frame)` for binary websocket frames: trades, bbo, order book, markets summary, funding data, funding rate comparison, heartbeat, subscribed, and the private order/fill/position/account templates. `Decimal` keeps mantissa and exponent (no f64). |
| `ws_control` | `parse(text)` for JSON-RPC text frames: subscribe / unsubscribe / modify acks (incl. scoped `markets`, `add`, `remove`), results (incl. `auth`), errors, JSON `subscription` notifications with typed `system_status()` and `market_event()`. |
| `signer` | `ParadexSigner`: legacy StarkNet typed data (Pedersen), auth / order / modify-order message hashes, Stark ECDSA, `quantize` (8-decimal chain integers). |
| `exchange` | `ParadexExchange::new(PARADEX_API_URL)`: `auth`, `create_order`, `modify_order`, `cancel_order`, `cancel_order_by_client_id`, `cancel_all_orders`, `open_orders`, `positions`, `balances`, `fills`. `NewOrder` / `ModifyOrder` build the signed bodies. |
| `ws_private` | Builders for `auth`, private `subscribe`/`unsubscribe`, `order.create`, `order.modify`, `order.cancel`, `order.cancel_all`; `parse_private` for the JSON form of private channel payloads. |

## Signing

Ported from paradex-py 0.9.0 (`message/auth.py`, `message/order.py`, `account/typed_data.py`,
`account/utils.py`).

- Typed data is legacy revision 0: `H = pedersen_chain([shortstring("StarkNet Message"),
  struct_hash(StarkNetDomain{name:"Paradex", chainId, version:"1"}), account, struct_hash(msg)])`,
  where `pedersen_chain` is `h(h(h(0,a0),a1)…,n)` and type hashes are `starknet_keccak` of the
  encoded type string.
- `chainId` is the shortstring of `starknet_chain_id` from `GET /v1/system/config`
  (`PRIVATE_SN_PARACLEAR_MAINNET`, `PRIVATE_SN_PARACLEAR_TESTNET`).
- Auth signs `Request{method:"POST", path:"/v1/auth", body:"", timestamp, expiration}` (seconds).
  Orders sign `Order{timestamp(ms), market, side(1 buy, 2 sell), orderType, size, price}` with size
  and price as integers with 8 decimals, truncated (`int(Decimal(s).scaleb(8))`); `MARKET` signs
  price 0. Modify signs `ModifyOrder` with the extra `id`.
- `k` is RFC 6979 with seed 32, as in starknet-crypto-py, so signatures are deterministic.
  The signature is sent as `["r","s"]` in decimal.

### Parity

`tests/vectors.json` was generated by `tests/gen_vectors.py` with the official paradex-py SDK,
a throwaway random key and no network: auth, limit/market orders in both directions, every
instruction, reduce-only, stp, recv_window, truncation and rounding edge cases, and modify, on
mainnet and testnet chain ids. `tests/vectors.rs` asserts message hashes, `r`, `s`, the flattened
signature, the public key and the order body exactly. Never regenerate the vectors to make a change
pass.

### API

```rust
use roshar_paradex::signer::{ParadexSigner, OrderSide, MAINNET_CHAIN_NAME};
use roshar_paradex::exchange::*;

let signer = ParadexSigner::from_hex(&l2_private_key, &l2_account, MAINNET_CHAIN_NAME)?;
let api = ParadexExchange::new(PARADEX_API_URL);
let jwt = api.auth(&signer, now_s, now_s + 3600, None).await?;   // you own refresh

let order = NewOrder {
    market: "BTC-USD-PERP", side: OrderSide::Buy, kind: OrderKind::Limit { price: "60000" },
    size: "0.001", instruction: Instruction::PostOnly, client_id: Some("c1"),
    reduce_only: false, stp: None, recv_window_ms: None, signature_timestamp_ms: now_ms,
};
let placed = api.create_order(&jwt, &order.sign(&signer)?).await?;
api.cancel_order(&jwt, &placed.id).await?;
```

## Coverage

| Item | Endpoint / channel |
|---|---|
| `get_markets(market, asset_kind, base_asset)` | `GET /v1/markets` |
| `get_markets_summary(market, asset_kind, base_asset)` | `GET /v1/markets/summary?market={market\|ALL}` |
| `get_orderbook(market, depth, price_tick)` | `GET /v1/orderbook/{market}` |
| `get_trades(market, &ParadexPageQuery)` | `GET /v1/trades?market=` (one page; follow `next`) |
| `get_klines(market, resolution, start_at, end_at, price_kind)` | `GET /v1/markets/klines` |
| `get_funding_history(market, &ParadexPageQuery)` | `GET /v1/funding/data?market=` (one page; follow `next`) |
| `get_bbo(market)` / `get_bbo_interactive(market)` | `GET /v1/bbo/{market}`, `GET /v1/bbo/{market}/interactive` |
| `get_orderbook_interactive(market, depth, price_tick)` | `GET /v1/orderbook/{market}/interactive` |
| `get_impact_price(market, side, size)` | `GET /v1/orderbook/{market}/impact-price` |
| `get_markets_history(market)` | `GET /v1/markets/history` (last 7 days) |
| `get_settlement_price(market)` | `GET /v1/markets/settlement-price` |
| `get_insurance_fund()` | `GET /v1/insurance` |
| `get_system_config()`, `get_system_state()`, `get_system_time()`, `get_exchange_stats()` | `GET /v1/system/config`, `/v1/system/state`, `/v1/system/time`, `/v1/system/stats` |
| `get_margin_config(base_asset)` | `GET /v1/system/margin-config` |
| `subscribe_depth` / `unsubscribe_depth` (`ParadexBookFeed`) | `order_book.{market}.deltas`, `.interactive_deltas`, `.snapshot@15@{50ms\|100ms}[@tick]`, `.interactive@15@{100ms\|200ms\|400ms}[@tick]` |
| `subscribe_trades` / `unsubscribe_trades` | `trades.{market}` |
| `subscribe_bbo` / `unsubscribe_bbo` | `bbo.{market}` |
| `subscribe_funding` / `unsubscribe_funding` | `funding_data.{market}` |
| `subscribe_markets_summary` / `unsubscribe_markets_summary` | `markets_summary`, `markets_summary.{market}` |
| `subscribe_markets_summary_scoped` / `modify_subscription` | scoped `markets_summary` (`params.markets`), `modify_subscription` (`add` / `remove`) |
| `subscribe_funding_rate_comparison` / `unsubscribe_…` | `funding_rate_comparison.ALL@{500ms\|1000ms}` (SBE template 6) |
| `subscribe_markets` / `unsubscribe_markets` | `markets`, `markets.{market}` (JSON; `ParadexWsNotification::market_event`) |
| `subscribe_system_status` / `unsubscribe_system_status` | `system_status` (JSON; `ParadexWsNotification::system_status`) |
| `ws_config()` | `PARADEX_WSS_URL` (`?sbeSchemaId=1&sbeSchemaVersion=3`), protocol-level ping/pong |
| `sbe::decode` | SBE templates 1-6, 20-23, 40, 41 (schema 1, versions 1-3) |
| `ws_control::parse` | JSON-RPC acks, results, errors, notifications |
| `exchange::auth` | `POST /v1/auth/{public_key}` (PARADEX-STARKNET-* headers; optional `token_usage=interactive`) |
| `create_order` / `modify_order` | `POST /v1/orders` / `PUT /v1/orders/{id}` (LIMIT and MARKET; GTC, POST_ONLY, IOC, RPI; reduce-only; client_id; stp; recv_window) |
| `cancel_order` / `cancel_order_by_client_id` / `cancel_all_orders` | `DELETE /v1/orders/{id}`, `/v1/orders/by_client_id/{id}`, `/v1/orders` |
| `open_orders`, `positions`, `balances`, `fills` | `GET /v1/orders`, `/v1/positions`, `/v1/balance`, `/v1/fills` |
| `ws_private::auth` / `subscribe(PrivateChannel)` | `auth`; `orders.{market\|ALL}`, `fills.{market\|ALL}`, `positions`, `account`, `balance_events` |
| `ws_private::order_*` | `order.create`, `order.modify`, `order.cancel`, `order.cancel_all` |

Notes:

- Public WS payloads are SBE binary frames (required since 2026-09-21; a connection without the
  SBE query parameters is rejected with HTTP 400). The docs page still says version 1; the server's
  own error, paradex-py and the pinned schema use 3, and live v3 frames match it.
- On an SBE connection `orders`, `fills`, `positions` and `account` arrive as SBE or JSON
  depending on the account: decode binary frames with `sbe::decode`, text with
  `ws_private::parse_private`. `balance_events` is always JSON.
- The schema requires unknown `AssetKind` and `FundingRateSource` values to map to `Unspecified`;
  every other enum errors on unknown values.
- Only one order book feed per market can share an SBE connection.
- Paradex has no JSON-RPC ping method. The server pings every 55 s and needs a pong within 5 s.
- Default REST limits: 25 req/s (1500 req/min per IP) and, for private GETs, 10 req/s (600/min per
  account). Unfiltered `/markets` and `/markets/summary?market=ALL` also draw on a per-client budget.
- `system_status` is delivered as a JSON `subscription` notification on an SBE connection (live).
  `markets` has no SBE template either, so it is parsed as JSON with the documented payload (same
  schema as `/markets/history` results); the subscription is acked live, but no event was
  published during a 58-minute capture, so that payload is untested against a live frame.
- `funding_rate_comparison` is not on the docs site; the channel name and refresh rates come from
  paradex-py and were accepted live.
- `impact-price` requires `side` (`BUY`/`SELL`) although the docs list only `size`; an empty
  book side returns HTTP 500 `filled size is zero`.
- `bbo` on an empty book returns `"0"` prices and sizes with `seq_no` 0; `markets/history`
  `changed_fields` carries only the fields that changed, with spec numbers as JSON numbers.
- Klines are `[ts_ms, open, high, low, close, volume]` JSON numbers; the docs give no response
  schema, so the order was read from live data.
- Onboarding is not needed for an existing account: export the L2 key (or a subkey) from the app
  and call `auth`.

## Not covered yet

Onboarding (`POST /v1/onboarding`) and EVM/SIWE auth; L1-key to L2-key derivation; stop, take-profit
and stop-loss orders (`trigger_price`); batch order endpoints and `order.cancel_on_disconnect`;
isolated-market accounts, MMP, block trades and RFQ; typed replies for WS `order.*`;
transfers and withdrawals.

Public endpoints left out as not market data: `GET /v1/system/announcements`,
`/v1/system/volume-tiers` (fee tiers), `/v1/system/portfolio-margin-config` (deprecated, superseded
by `margin-config`), referrals and vaults.
