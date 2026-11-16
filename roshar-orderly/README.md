# roshar-orderly

Orderly Network client: public market data (REST + websocket config), Orderly-key (ed25519)
request signing, the signed trading and account API, private websocket auth, and the wallet
(EIP-712) signature that adds an Orderly key to an account. Orderly is one order book shared by
every builder ("broker") front-end; the broker id is a caller parameter.

## Principles

Same as the other roshar crates (see `roshar-hyperliquid/README.md`):

- **Mirror the exchange.** Signed paths, bodies and signatures are byte-identical to the official
  Python SDK. Responses are parsed strictly; `{"success": false, "code", "message"}` becomes
  `OrderlyError::Api` (Orderly sends it with HTTP 200 as well as 4xx).
- **No caller policy.** Every signed call takes the caller's `timestamp_ms`; expirations, client
  order ids, websocket request ids, broker id and chain id are caller inputs. Credentials are passed
  in. Nothing reads the environment or the clock.

## API docs

| Area | Reference |
|---|---|
| REST | https://orderly.network/docs/build-on-omnichain/introduction (per-endpoint OpenAPI, e.g. https://orderly.network/docs/build-on-omnichain/restful-api/private/create-order); index https://orderly.network/docs/llms.txt |
| WebSocket | https://orderly.network/docs/build-on-omnichain/websocket-api/introduction, `/websocket-api/authentication`, `/websocket-api/ping-pong`, `/websocket-api/error-response`, per-topic pages under `/websocket-api/public/` and `/websocket-api/private/` |
| Auth / signing | https://orderly.network/docs/build-on-omnichain/api-authentication, https://orderly.network/docs/build-on-omnichain/user-flows/wallet-authentication, https://orderly.network/docs/build-on-omnichain/user-flows/accounts |
| Errors | https://orderly.network/docs/build-on-omnichain/error-codes |
| Rate limits | Per endpoint, on each endpoint page (public market data: 10 req/s per IP; most private: 10 req/s per key; batch create 1 req/s). Counted per Orderly key; HTTP 429 past it. |
| Changelog | https://orderly.network/docs/release-notes/ |
| SDK used for vectors | https://github.com/OrderlyNetwork/orderly-evm-connector-python at `fb7def39` (2026-08-07, pyproject 0.2.8). PyPI 0.2.9 has identical `api.py`, `_account.py` and signing in `lib/utils.py`. |

Checked against these on 2026-11-16.

UK access: Orderly's terms (https://orderly.network/docs/introduction/terms-of-service, §4
"Prohibited Locations") exclude US persons and sanctioned "Restricted Persons" only; the UK is not
named. Individual builder front-ends may add their own restrictions. From a UK IP every public
endpoint and the public websocket answered normally; `GET /v1/ip_info` reported
`"region":"United Kingdom","checked":true`. `GET /v1/product_restricted_areas?product_type=BINARY`
returned empty country/city lists (other product types are rejected with -1014).

## Modules

| Module | What it does |
|---|---|
| crate root | `OrderlyClient` (public REST via the `OrderlyApi` trait, `ws_config()` / `subscribe` / `unsubscribe` / `request_orderbook` / `pong`), `OrderlyPrivateWs` (`auth`, `subscribe`, `unsubscribe`, `pong`), `ORDERLY_API_URL`, `DEFAULT_REQUESTS_PER_SECOND`. |
| `signer` | `OrderlyKey` (ed25519: `sign_request`, `auth_headers`, `ws_auth_message`), `WalletSigner` (secp256k1 EIP-712 `sign_add_orderly_key`), `account_id(address, broker_id)`, `request_message`. |
| `request` | `PreparedRequest` builders for every signed endpoint (method, path+query, exact body). `OrderlyOrderRequest`, `OrderlyEditOrderRequest`, query structs. |
| `OrderlyTradingApi` | Signs and sends `PreparedRequest`s with a key and account id. |
| `OrderlyAccountApi` | Unsigned `get_account`, `get_orderly_key`, and `add_orderly_key` (wallet-signed). |
| `OrderlyMarketApi` | Public REST, as used by `OrderlyClient`. |
| `ws` | URLs, `PING`/`PONG`, `OrderlyTopic`, `OrderlyPrivateTopic`, frame builders. |
| `types` | Market, Public Info API (`OrderlyQuery*`), websocket and (`types::trading`) private response types. `OrderlyWsMessage::parse` turns any public websocket frame into a typed variant. |

## Coverage

| Item | Endpoint / channel |
|---|---|
| Symbols / order rules | `GET /v1/public/info`, `/v1/public/info/{symbol}` |
| Tickers (mark, index, funding, OI, 24 h) | `GET /v1/public/futures`, `/v1/public/futures/{symbol}` |
| Recent trades | `GET /v1/public/market_trades` |
| Funding | `GET /v1/public/funding_rates`, `/v1/public/funding_rate/{symbol}`, `/v1/public/funding_rate_history`, `/v1/public/market_info/funding_history` |
| Other market info | `GET /v1/public/market_info/price_changes`, `/traders_open_interests`, `/history_charts`, `/v1/public/system_info` |
| Index prices | `GET /v1/public/index_price` (1 s history), `/v1/public/index_price_source` |
| Liquidations, insurance fund | `GET /v1/public/liquidated_positions`, `/v1/public/liquidation`, `/v1/public/insurancefund` |
| Exchange config | `GET /v1/public/config` (leverage choices), `/v1/public/leverage`, `/v1/public/token` (collateral), `/v1/public/chain_info`, `/v1/tv/symbol_info`, `/v1/tv/config` |
| Orderly Predict rounds | `GET /v1/public/binary/info`, `/v1/public/binary/info/{symbol}` |
| Candles | `GET /v1/tv/kline_history` and `/v1/tv/history` (public); `GET /v1/kline` (Orderly key) |
| Order book snapshot | `GET /v1/orderbook/{symbol}` (Orderly key), WS `request` orderbook, Public Info `orderbook` |
| Public Info API | `POST /v1/public/query` market types: `marketSummary`, `marketDetail`, `orderbook`, `candles`, `marketTrades`, `liquidations`, `fundingRateHistory`, `fundingComparison`. Errors (string codes) → `OrderlyError::Query`. |
| Public WS | `{s}@orderbook`, `{s}@orderbookupdate`, `{s}@trade`, `{s}@ticker`, `tickers`, `{broker}${s}@ticker`, `{broker}$tickers`, `{s}@bbo`, `bbos`, `{s}@kline_{i}`, `{s}@estfundingrate`, `{s}@markprice`, `markprices`, `{s}@indexprice`, `indexprices`, `{s}@openinterest`, `price_changes`, `traders_open_interests`, `history_charts_{i}`, `liquidation`, `maintenance_status`; all parsed by `OrderlyWsMessage` |
| Orders | `POST /v1/order` (LIMIT, MARKET, IOC, FOK, POST_ONLY, ASK, BID; reduce-only, client order id, hidden/iceberg, slippage, level, post_only_adjust, margin mode, order tag), `POST /v1/batch-order`, `PUT /v1/order` |
| Cancels | `DELETE /v1/order`, `/v1/client/order`, `/v1/orders` (all / per symbol), `/v1/batch-order`, `/v1/client/batch-order`, `POST /v1/order/cancel_all_after` |
| Reads | `GET /v1/orders`, `/v1/order/{id}`, `/v1/client/order/{cid}`, `/v1/order/{id}/trades`, `/v1/trades`, `/v1/trade/{id}`, `/v1/positions`, `/v1/position/{s}`, `/v1/client/holding`, `/v1/client/info`, `/v1/account_info`, `/v1/funding_fee/history`, `/v1/client/key_info` |
| Keys | `GET /v1/get_account`, `GET /v1/get_orderly_key`, `POST /v1/orderly_key` |
| Private WS | `auth`, `executionreport` (optionally per symbols), `position`, `balance`, `account` |

`OrderlyClient` and `OrderlyTradingApi` rate-limit to 10 requests a second by default
(`DEFAULT_REQUESTS_PER_SECOND`): the market endpoints allow 10 a second per IP and most private
endpoints 10 a second per key. Share one `RateLimitedClient` between APIs that use the same key.

The websocket path carries an account id: `wss://ws-evm.orderly.org/ws/stream/{account_id}`. The
server closes public connections whose id it does not recognise (tested: random 40-char and 32-byte
hex ids are closed with 1000; the SDK's default id `OqdphuyCtYWxwzhxyLLjOWNdFP7sQt8RPWzmb5xY` is
accepted). The server pings with `{"event":"ping"}` every 10 s; `ws_config()` sends
`{"event":"ping"}` every 10 s as text. Text pongs are not protocol pongs, so answer server pings
with `pong()` if you need to.

## Signing

- **Request auth.** Message `{timestamp_ms}{METHOD}{path?query}{body}` signed with the ed25519
  Orderly key. Headers: `orderly-timestamp`, `orderly-account-id`, `orderly-key` (`ed25519:` +
  base58 public key), `orderly-signature`. The docs say base64url; the official SDK sends standard
  base64, and this crate follows the SDK. Requests more than 300 s from server time are rejected.
- **Body encoding.** The server checks the signature against the bytes it receives, so the crate
  signs the exact text it sends. It writes bodies as the SDK does (Python `json.dumps`: `", "` /
  `": "` separators, ASCII escaping, SDK key order). Prices and sizes are decimal strings written
  verbatim as JSON numbers, so the caller controls precision. Query values are not
  percent-encoded; values with `& = # ? % +`, spaces or non-ASCII are rejected.
- **Websocket auth.** `{"id", "event": "auth", "params": {"orderly_key", "sign", "timestamp"}}`;
  `sign` is the ed25519 signature of the decimal timestamp alone.
- **Add Orderly key.** EIP-712 `AddOrderlyKey(string brokerId,uint256 chainId,string orderlyKey,string scope,uint64 timestamp,uint64 expiration)`
  in domain `{name "Orderly", version "1", chainId, verifyingContract 0xCcCCccccCCCCcCCCCCCcCcCccCcCCCcCcccccccC}`,
  signed by the wallet with `k256`; signature `0x` + r‖s‖v (v = 27/28). Expiration at most 365 days.
- **Account id.** `keccak256(abi.encode(address, keccak256(broker_id)))`.

### Parity

`tests/vectors.json` was produced by `tests/gen_vectors.py` from the official SDK with throwaway
random keys and no network (the SDK's HTTP dispatch is replaced by a recorder, its clock by fixed
values). ed25519 and RFC 6979 ECDSA are deterministic, so `tests/vectors.rs` checks byte-identical
method, path, body, signed message and signature for 37 requests (every order type, batch create,
edit, all cancels, every read), the websocket auth frame, two `AddOrderlyKey` bodies and signatures
(chains 42161 and 8453), the public key, the wallet address and the account id. Fields the SDK's
`create_order` does not take (`slippage`, `level`, `post_only_adjust`, `margin_mode`) are signed
through the SDK's own `_sign_request`. Never regenerate the vectors to make a change pass.

Private REST responses and private websocket pushes are typed from the documented schemas and
examples (`tests/fixtures/docs_ws_*.json`); no live account was available to capture them.

## Usage

```rust
use roshar_orderly::*;
use roshar_http::RateLimitedClient;

let key = OrderlyKey::from_secret(&orderly_secret)?;          // "ed25519:<base58 seed>"
let account = signer::account_id(&wallet_address, &broker_id)?;
let http = RateLimitedClient::new(DEFAULT_REQUESTS_PER_SECOND, 1);
let trading = OrderlyTradingApi::new(http.clone(), ORDERLY_API_URL, account.clone(), key);

let mut order = OrderlyOrderRequest::new("PERP_ETH_USDC", OrderlyOrderType::PostOnly, OrderlySide::Buy);
order.order_price = Some("1800.5".into());
order.order_quantity = Some("0.1".into());
order.client_order_id = Some("my-0001".into());
let ack = trading.create_order(now_ms, &order).await?;
trading.cancel_order_by_client_order_id(now_ms, "my-0001", "PERP_ETH_USDC").await?;
trading.cancel_all_orders(now_ms, Some("PERP_ETH_USDC")).await?;

// Private websocket (after every connect):
let private = OrderlyPrivateWs::new(&account);
manager.new_conn("orderly-private", private.ws_config())?;
private.auth(&manager, trading.key(), "auth-1", now_ms)?;
private.subscribe(&manager, "er-1", &OrderlyPrivateTopic::ExecutionReport(None))?;

// Adding a key with the wallet (once):
let wallet = WalletSigner::from_hex(&wallet_private_key)?;
let new_key = OrderlyKey::from_seed(seed);
let signed = wallet.sign_add_orderly_key(&AddOrderlyKey {
    broker_id: &broker_id, chain_id: 42161, orderly_key: new_key.public_key(),
    scope: "read,trading", timestamp: now_ms, expiration: now_ms + 30 * 86_400_000,
})?;
OrderlyAccountApi::new(http, ORDERLY_API_URL).add_orderly_key(&signed).await?;
```

## Not covered yet

Account registration (`GET /v1/registration_nonce`, EIP-712 `Registration`,
`POST /v1/register_account`), deposits, withdrawals, PnL settlement, internal transfers,
delegate signers and Solana-wallet (`chainType: SOL`) key registration. Algo orders
(stop/TP/SL/bracket: `/v1/algo/*`, `algoexecutionreport`), leverage and margin-mode settings,
position-margin adjustment, removing keys and key IP restrictions, sub-accounts, position history,
`positions_lite`, maintenance config. Each needs vectors from `gen_vectors.py` where it is signed.

Public, out of market-data scope: `announcement` websocket topic and announcement / campaign /
referral / builder-stats / staking / vault / Valor / faucet / restricted-area / IP-info REST
endpoints; the Public Info API account and platform types (`accounts`, `accountState`,
`agentContext`, `feeRate`, `fundingPayments`, `historicalOrders`, `openOrders`, `orderStatus`,
`portfolio`, `positionContext`, `trades`, `userDepositsWithdrawals`, `whaleContext`,
`topAddresses`, `platformPositions`) and `rateLimitStatus`.

Live vs docs (checked 2026-11-16 from the UK):

- `GET /v1/public/market_info/history_charts` needs `time_interval` (docs: optional; -1005
  without it). `1h`, `4h`, `1d`, `1w` answer; `1m`, `5m`, `15m` give -1000. The
  `history_charts_{i}` topic accepts `1h` and rejects `1m` ("invalid topic").
- `GET /v1/public/index_price` returns `data: {"rows": [...]}`, not the documented bare array.
- `GET /v1/tv/config` returns the TradingView datafeed config (`supported_resolutions`,
  `supports_*`), not the documented `{s, d}` terminal config; `locale` and `group`
  (`/v1/tv/symbol_info`) make no difference. `/v1/tv/symbol_info` sends 15 of the documented
  columns, including none of the ones the docs mark required; only the sent ones are typed.
- The `traders_open_interests` topic is spelled `traders_open_interest` in the docs' parameter
  table; the server rejects that spelling.
- `GET /v1/public/liquidation` and the `liquidation` / `maintenance_status` topics had no data
  while capturing; their types come from the documented examples
  (`tests/fixtures/docs_*liquidation*.json`, `docs_ws_maintenance_status.json`).
