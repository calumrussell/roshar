# roshar-polymarket-perps

Polymarket Perps client (`api.perpetuals.polymarket.com`): public market data over REST, websocket
config and message builders, EIP-712 signing of trading ops with a proxy key, signed REST trading
routes and authenticated account reads. Separate from `roshar-polymarket` (Gamma prediction markets).

## Principles

Same as the other roshar crates:

- **Mirror the exchange.** Signed payloads, hashes, signatures and request bodies are
  byte-identical to the official TypeScript SDK. Replies are parsed strictly. A non-2xx reply is a
  `PolymarketPerpsApiError` carrying the exchange's `error` identifier and `ref`.
- **No caller policy.** The library does not choose salts, timestamps, command deadlines, client
  order ids, request ids, instruments or retries. Keys and the proxy secret are passed in. Nothing
  is read from the environment or the clock.

## Modules

| Module | What it does |
|---|---|
| `signer` | `Signer` (secp256k1 key + Polymarket EIP-712 domain), `Op` (every signed operation), `SignedCommand` (REST body + WS `post` frame), `SignedCreateProxy`. |
| `rest::trading` | `TradingApi`: signed `/v1/trade/*` routes, `createProxy` / `deleteProxy`, and `/v1/account/*` reads with `POLYMARKET-PROXY` / `POLYMARKET-SECRET` headers. `ProxyCredentials`. |
| `rest::market` | `MarketApi`: every public `/v1/info/*` market-data endpoint below. |
| `types` | REST and WS payloads; WS builders `subscribe`, `unsubscribe`, `ping`, `auth`, `*_channel`, `CHANNEL_*`; `WsFrame` / `WsUpdate<T>` / `WsResponse<T>`. |
| crate root | `PolymarketPerpsClient` (`PolymarketPerpsApi` trait for market REST, `trading_api()`, `ws_config()`, `subscribe_*` / `unsubscribe_*`, `authenticate`, `subscribe_private`, `send_command`), `POLYMARKET_PERPS_API_URL`, `POLYMARKET_PERPS_WSS_URL`. |

## API docs

| Area | Reference |
|---|---|
| REST | https://docs.polymarket.com/perps/market-data, https://docs.polymarket.com/perps/trading, https://docs.polymarket.com/perps/account-management, OpenAPI https://docs.polymarket.com/api-spec/perps-openapi.json |
| WebSocket | https://docs.polymarket.com/perps/realtime-updates, https://docs.polymarket.com/api-reference/wss/perps-book, AsyncAPI https://docs.polymarket.com/asyncapi-perps.json |
| Auth / signing | https://docs.polymarket.com/perps/authenticated-sessions (createProxy, headers, WS auth), https://docs.polymarket.com/perps/trading (Op typed data, compact ops) |
| Rate limits | https://docs.polymarket.com/api-reference/perps/rate-limits |
| Errors | https://docs.polymarket.com/perps/errors |
| Changelog | https://docs.polymarket.com/changelog/perps (latest entry Nov 10, 2026), SDK: https://docs.polymarket.com/changelog/sdks |
| SDK used for vectors | `@polymarket/client` 0.18.0, https://github.com/Polymarket/ts-sdk at `3f0d9b85532d745b513596f4715aa49647acea1d` (2026-11-09), `packages/client/src` |

Checked against these on 2026-11-15.

**UK access.** The Perps geographic-restrictions page
(https://docs.polymarket.com/api-reference/perps/geographic-restrictions, and the FAQ
https://docs.polymarket.com/perps/faq) says: "Order placement is not permitted from the United
States, Canada, Cuba, Iran, North Korea, Syria, Crimea, Donetsk, or Luhansk", and "This list can
change". The UK is not listed. (The separate prediction-markets list,
https://docs.polymarket.com/api-reference/geoblock, puts GB in close-only, and
`https://polymarket.com/api/geoblock` returns `{"blocked":true,"country":"GB"}` from this machine;
that endpoint is for prediction markets.) From a UK IP, every public REST endpoint and the public
WS channels work, and signed trading requests reach signature verification (a valid signature from
an unregistered proxy returns `400 account not found for proxy`). No geo-block response was seen.
Order placement from a registered UK account was not tested.

## Coverage

| Item | Endpoint / channel |
|---|---|
| Exchange, time, ping, assets | `GET /v1/info/exchange` (EIP-712 domain, `cancel_only`), `/time`, `/ping`, `/assets` |
| Instruments | `GET /v1/info/instruments` (`instrument_id`, `instrument_type`, `category`) |
| Tickers, funding rate | `GET /v1/info/tickers` (mark, index, last, mid, OI, `funding_rate`, `next_funding`) |
| 24 h statistics | `GET /v1/info/statistics`, `GET /v1/info/exchange-stats` |
| Book, BBO | `GET /v1/info/book` (`depth` 10/100/500/1000), `GET /v1/info/bbo` |
| Trades | `GET /v1/info/trades` (≤100, newest first) |
| Candles, mark history | `GET /v1/info/klines`, `GET /v1/info/mark-history` (≤1000) |
| Funding history | `GET /v1/info/funding` (≤100) |
| Index, fees, limit tiers, registration | `GET /v1/info/index`, `/fees`, `/limit-tiers`, `/registered` |
| Public WS | `bbo::{iid}`, `book::{iid}[::50]`, `trades::{iid}`, `tickers::{iid\|all}` (also the public funding stream), `statistics::{iid\|all}`, `klines::{iid}::{interval}`; each has a typed payload (`WsBbo`, `WsBook`, `WsTrade`, `WsTicker`, `WsStatistics`, `Kline`) and every one has a live-capture parse test |
| Proxy credentials | `POST /v1/account/proxy` (owner-signed `CreateProxy`), `DELETE /v1/account/proxy` (owner-signed `deleteProxy` op), `GET /v1/account/credentials` |
| Orders | `POST /v1/trade/orders` (gtc / gtd / ioc / fok, post-only, reduce-only, client order id, batch), `DELETE /v1/trade/orders`, `DELETE /v1/trade/orders-coid`, `PATCH /v1/trade/orders`, `PATCH /v1/trade/orders-coid`, `DELETE /v1/trade/orders/all` |
| Dead man's switch | `PATCH /v1/trade/auto-cancel`, `GET /v1/account/auto-cancel` |
| Leverage, margin | `PATCH /v1/trade/leverage`, `/leverage/batch`, `/margin`, `GET /v1/account/config` |
| Account reads | `GET /v1/account/orders`, `/open-orders`, `/balances`, `/portfolio`, `/fills`, `/funding` |
| Private WS | `auth`, `orders`, `fills`, `portfolio`, `balances`, `funding`; every signed op also as a WS `post` frame (`SignedCommand::ws_frame`) |

`PolymarketPerpsClient` rate-limits REST to 16 requests/s: the IP budget is 1,000 weighted tokens
a minute and light reads weigh 1 (deep books, unfiltered open orders and account orders weigh up
to 20). `trading_api()` shares the same limiter. Order actions also draw on a per-account action
budget (5,000 tokens a minute by default).

The WS config sends the documented application ping (`{"req":"post","op":{"type":"ping"}}`) every
25 s as a text frame; the server closes idle connections after 60 s.

## Signing

| Item | Value |
|---|---|
| Curve / hash | secp256k1 ECDSA (RFC 6979, low-S), keccak256; signature `0x` r‖s‖v, v = 27/28 |
| Domain | `EIP712Domain(string name,string version,uint256 chainId)` = `{"Polymarket", "1", 137}`, no verifying contract (`GET /v1/info/exchange` reports the same `name`, `version`, `chain_id`) |
| Trading ops | `Op(bytes32 data,uint64 salt,uint64 ts)`, `data = keccak256(msgpack(compact op))`, signed by the proxy key |
| Compact op | `[type, args]` with every absent array entry removed, e.g. `["createOrders", [[iid, buy, p?, qty, tif, po, ro?, c?, gtd_expiry?]]]`, `["cancelAll", []]`, `["autoCancel", [time]]`, `["updateLeverage", [iid, lev, cross]]`, `["modifyOrders", [[oid, p, qty]]]` |
| Proxy registration | `CreateProxy(address addr,uint64 exp,uint64 salt,uint64 ts)`, signed by the account owner |
| Proxy revocation | `Op` over `["deleteProxy", [proxy]]`, signed by the account owner |
| Body | `{"salt","sig","ts","exp"?,"op":{...}}`; WS adds `"id"` and `"req":"post"`. `exp` (command deadline) is not signed. |

msgpack integers use the smallest encoding, as `@msgpack/msgpack` does for JS numbers. The SDK
holds ids and times as JS numbers, so values above 2^53 − 1 are rejected.

### Parity

`tests/vectors.json` is produced by `tests/gen_vectors.mts`, which imports the SDK source
(`signPerpsOp`, `createPerpsOpTypedDataPayload`, `toPerpsCommandBodyOp`, the trading actions,
`openPerpsSession` and `revokePerpsCredentials`) with fresh throwaway keys and no network. The
trading actions build the compact op and body from SDK-level requests; createProxy / deleteProxy
are captured from the SDK's own flows with the viem signer. RFC 6979 makes signatures
deterministic, so `tests/vectors.rs` checks msgpack bytes, `data`, EIP-712 digest, signature, REST
body and WS frame byte for byte. Cases: gtc / gtd / ioc (with and without price) / fok orders,
post-only, reduce-only, client order ids, batches, `exp`, chain 80002, cancels by id and coid,
cancel-all (all / one instrument), auto-cancel arm / disarm, leverage (single and batch), isolated
margin, createProxy (with and without label) and deleteProxy.

`modifyOrders` / `modifyOrdersCOID` are not implemented in the SDK yet ("SDK support is coming
soon"). Their vectors use the documented compact ops and bodies, hashed and signed with the SDK's
`signPerpsOp`.

Never regenerate the vectors to make a change pass. To regenerate, follow the header of
`tests/gen_vectors.mts` (clone the SDK at the commit above, `pnpm install`, build `types` and
`bindings`, run with `tsx`).

### API

```rust
use roshar_polymarket_perps::signer::*;
use roshar_polymarket_perps::types::TimeInForce;
use roshar_polymarket_perps::{PolymarketPerpsClient, ProxyCredentials};

let client = PolymarketPerpsClient::new();
let trading = client.trading_api();

// Once: the account owner registers a proxy key; keep the returned secret.
let owner = Signer::from_hex(&owner_key, POLYGON_CHAIN_ID)?;
let proxy = Signer::from_hex(&proxy_key, POLYGON_CHAIN_ID)?;
let reg = owner.sign_create_proxy(&proxy.address(), expiry_ms, salt, now_ms, Some("bot"))?;
let secret = trading.create_proxy(&reg).await?;            // CreateProxyResult::Ok { secret }

// Trading: the proxy signs each op; you supply salt and ts.
let order = CreateOrder {
    instrument_id: 6, buy: true, price: Some("65000".into()), quantity: "0.01".into(),
    tif: TimeInForce::Gtc, post_only: true, reduce_only: false,
    client_order_id: Some("7f9e4a2b6c8d0e1f1234567890abcdef".into()), gtd_expiry: None,
};
let cmd = proxy.sign_op(Op::CreateOrders(vec![order]), None, salt, now_ms)?;
let acks = trading.create_orders(&cmd).await?;             // or client.send_command(&manager, id, &cmd)

let creds = ProxyCredentials { proxy: proxy.address(), secret };
let open = trading.get_open_orders(&creds, Some(6)).await?;
client.authenticate(&manager, Some(1), &creds)?;           // then subscribe_private
```

## Not covered yet

- Deposits, withdrawals (owner-signed `Withdraw` with verifying contract), internal transfers,
  deposit/withdrawal/transfer history and channels.
- TP/SL triggers (`tr`, `grp`), trailing stops, builder attribution (`builder`), chases, TWAPs:
  the signer has no fields for them yet.
- WebSocket `cancelOnDisconnect` (added 2026-11-10; not in the SDK or the OpenAPI yet).
- Equity / PnL history, backstops, account stats / limits / rewards, referral, invites,
  notifications, builder endpoints, competitions, leaderboard, and the `tpsl`,
  `notifications`, `builderFills` channels: account data, out of scope.
- `GET /v1/info/portfolio`, `/fills`, `/position-fills` and `POST /v1/info/position-snapshots`
  are unauthenticated but keyed by `address`: they are one account's portfolio / fills /
  share-card data, not market data. Public fills share the `AccountFills` shape (tested).
- `backstop` channel: AsyncAPI marks it auth-required and restricted to Backstop Liquidity
  Provider accounts (others get `unauthorized`), so it is not a public market channel.
- `GET /v1/info/largest-positions?instrument_id=` answers live (ranked accounts with size, entry,
  PnL, leverage) but is not in the OpenAPI or docs; it is a per-account ranking (leaderboard
  style), so it is not implemented.
- `exp` is only accepted on the ops the OpenAPI documents it for; the SDK also passes it on
  auto-cancel.
- Private response parse tests use the documented examples (`tests/fixtures/docs/`): there is no
  funded account to capture them live.
