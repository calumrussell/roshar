# roshar-synthetix

Synthetix perps client (offchain orderbook, Ethereum mainnet settlement): public market data (REST
+ websocket frames), EIP-712 request signing, `POST /v1/trade` submission, and the trade-websocket
frames for auth, signed actions and `subAccountUpdates`.

## Principles

Same as the other roshar crates (see `roshar-hyperliquid/README.md`):

- **Mirror the exchange.** Request bodies, EIP-712 digests and signatures are byte-identical to the
  official SDK. Replies are parsed strictly; a missing field or `"status":"error"` is an error that
  carries the exchange's `code` and `message`.
- **No caller policy.** The library does not choose nonces, expiries, client order ids, symbols,
  retries or request ids. The key and the `User-Agent` are passed in. Nothing reads the environment
  or the clock.

The exchange returns HTTP 403 (REST and websocket upgrade) without a `User-Agent`, so every client
takes one. The websocket configs send it through `roshar_ws_mgr::Config::headers`.

## API docs

| Area | Reference |
|---|---|
| REST | https://developers.synthetix.io/developer-resources/api/rest-api, info actions https://developers.synthetix.io/developer-resources/api/rest-api/info, trade actions https://developers.synthetix.io/developer-resources/api/rest-api/trade, objects https://developers.synthetix.io/developer-resources/api/objects |
| WebSocket | https://developers.synthetix.io/developer-resources/api/ws-api, https://developers.synthetix.io/developer-resources/api/ws-api/subscriptions, https://developers.synthetix.io/developer-resources/api/ws-api/trade-websocket/authentication |
| Endpoints | https://developers.synthetix.io/environments |
| Auth / signing | https://developers.synthetix.io/developer-resources/api/authentication/eip-712-signing, https://developers.synthetix.io/developer-resources/api/authentication/nonce-management |
| Rate limits | https://developers.synthetix.io/developer-resources/api/rate-limits |
| Errors | https://developers.synthetix.io/developer-resources/api/error-handling |
| Changelog | https://developers.synthetix.io/changelog (latest entry: 2026-07-07), SDK https://github.com/Synthetixio/synthetix-sdk/blob/main/CHANGELOG.md |
| Full export | https://developers.synthetix.io/llms-full.txt (the docs say there is no OpenAPI spec) |
| SDK used for vectors | `synthetix-sdk` 0.7.0 (PyPI, latest), https://github.com/Synthetixio/synthetix-sdk at `8a9c7447` (2026-08-17), with `eth-account` 0.13.7 |

Checked against these on 2026-11-15 and again on 2026-11-16 (public market data). The SDK is the
one the docs link as the official Python SDK.

UK access: the exchange terms (https://synthetix.io/exchange-terms, "1.2. Service Limitations",
last modified 19 December 2025) exclude sanctioned persons and residents of Cuba, Iran, North Korea,
Sudan and occupied Donetsk/Luhansk/Crimea, "or where your use of the Services would otherwise
violate any applicable law". The UK is not named. From a UK IP every public REST endpoint and both
websockets answered normally; private endpoints returned only auth errors.

## Modules

| Module | What it does |
|---|---|
| crate root | `SynthetixClient::new(user_agent)`: market REST via the `SynthetixApi` trait, `ws_config()` (`/v1/ws/info`) and `trade_ws_config()` (`/v1/ws/trade`) for roshar-ws-mgr, `subscribe_*` / `unsubscribe_*`, `post_info` (unsigned info-socket queries), `authenticate`, `post`. |
| `signer` | `Signer` (secp256k1 key), `sign_place_orders`, `sign_cancel_orders`, `sign_cancel_orders_by_cloid`, `sign_cancel_all_orders`, `sign_modify_order`, `sign_modify_order_by_cloid`, `sign_update_leverage`, `sign_schedule_cancel`, `sign_query`, delegated-signer actions, `sign_ws_auth`. Returns `SignedRequest` (`rest_body()`, `ws_post(id)`) or `WsAuth` (`ws_message(id)`). |
| `TradeApi` | `POST /v1/trade` for a `SignedRequest`: `place_orders`, `cancel_orders`, `cancel_all_orders`, `modify_order`, `update_leverage`, `schedule_cancel`, `query::<Q>`, delegation calls, or generic `send::<T>`. |
| `ws` | URLs, `SYNTHETIX_WS_PING`, and `{"id","method","params"}` frames for `orderbook`, `trades`, `marketPrices`, `candles`, `subAccountUpdates`, and info-socket `post` queries (`InfoPost`, `post_info`). |
| `types` | Envelope (`SynthetixEnvelope`, `SynthetixApiError`), info and aggregator types, websocket pushes. `types::trading`: query structs (`GetOpenOrders`, `GetPositions`, `GetSubAccount`, `GetTrades`, `GetOrderHistory`, `GetFundingPayments`, `GetRateLimits`, `GetDelegatedSigners`) and trade responses. |

## Coverage

| Item | Endpoint / channel |
|---|---|
| Markets | `getMarkets` (`activeOnly`), `GET /v1/info/contract_specs` |
| Tickers | `getMarketPrices`, `getMids`, `getOpenInterest`, `GET /v1/info/contracts` |
| Order book | `getOrderbook` (`limit`), `GET /v1/info/orderbook` (`ticker_id`, `depth`) |
| Trades | `getLastTrades` |
| Candles | `getCandles` |
| Funding | `getFundingRate`, `getFundingRateHistory` |
| Reference | `getCollaterals`, `getTiers`, `getIsWhitelisted`, `getSubAccountIds` (with and without `includeDelegations`) |
| Status | `GET /v1/status` (what the SDK polls), `GET /v1/exchange/status`, `GET /v1/ws/exchange/status` (same status from the websocket service) |
| Public WS | `orderbook` (format/depth/frequency), `trades`, `marketPrices`, `candles`; typed pushes `orderbook_depth_update`, `trade`, `market_price_update` (with the undocumented `fundingRate`/`nextFundingTime` on `mark` updates), `candle_update`; `ping` replies |
| Info WS `post` | `ws::InfoPost` for `getMarkets`, `getMarketPrices`, `getMids`, `getOpenInterest`, `getOrderbook`, `getLastTrades`, `getCandles`, `getFundingRate`, `getCollaterals` (answered live, not in the websocket docs), `getIsWhitelisted`, `getSubAccountIds` (with and without `includeDelegations`). `SynthetixWsReply::into_post_response::<T>()` returns the action's REST type or the `SynthetixWsError`. `getFundingRateHistory` and `getTiers` are REST-only: the socket answers "Unsupported action". |
| Orders | `placeOrders` (all order types, `postOnly`, `reduceOnly`, `clientOrderId`, GTD `expiresAt`, trigger, TWAP, groupings), `cancelOrders` by id / cloid, `cancelAllOrders`, `modifyOrder` by id / cloid, `scheduleCancel` |
| Account | `getOpenOrders`, `getOrderHistory`, `getPositions`, `getSubAccount` (balances, margin), `getTrades` (fills), `getFundingPayments`, `getRateLimits`, `updateLeverage` |
| Delegation | `addDelegatedSigner`, `removeDelegatedSigner`, `removeAllDelegatedSigners`, `getDelegatedSigners` |
| Private WS | `auth`, signed actions as `method: "post"`, `subAccountUpdates` (`SubAccountUpdate`, `data` kept as JSON) |

`SynthetixClient` rate-limits market REST calls to 4 a second by default. The per-IP budget is
10,000 tokens per 10 s shared by REST and websocket; info actions cost 50 to 250 tokens
(`getFundingRateHistory` 1,000). `TradeApi` is not rate-limited: trade actions also draw on a
per-subaccount budget that depends on the fee tier, so pacing is the caller's.

## Signing

EIP-712 over secp256k1, hand-rolled with `k256` + `sha3`, as in `roshar-hyperliquid`.

- Domain: `{name "Synthetix", version "1", chainId 1, verifyingContract 0x0}` for every action.
- One primary type per action, e.g.
  `PlaceOrders(uint256 subAccountId,Order[] orders,string grouping,uint256 nonce,uint256 expiresAfter)`
  with `Order(string symbol,string side,string orderType,string price,string triggerPrice,string quantity,bool reduceOnly,bool isTriggerMarket,string clientOrderId,bool closePosition)`;
  `CancelOrders` (`uint256[] orderIds`), `CancelOrdersByCloid`, `CancelAllOrders`, `ModifyOrder`,
  `ModifyOrderByCloid`, `UpdateLeverage`, `ScheduleCancel`, `AddDelegatedSigner`,
  `RemoveDelegatedSigner`, `RemoveAllDelegatedSigners`, and `SubAccountAction(uint256 subAccountId,string action,uint256 nonce,uint256 expiresAfter)`
  for read-only queries. Type strings are the SDK's (`synthetix/utils/constants.py`).
- `postOnly`, `expiresAt`, `triggerPriceType` and the TWAP fields are sent but not signed.
- Signature: RFC 6979, low-s, `v` 27/28, sent as `{"v","r","s"}`. Websocket auth signs
  `AuthMessage(uint256 subAccountId,uint256 timestamp,string action)` and sends the typed data as a
  JSON string with the signature as `0x` + r‖s‖v.
- Request body: `{"signature","nonce","expiresAfter","params"}` with
  `params = {action, subaccountId, walletAddress, …}`, as the SDK sends it.

### Parity

`tests/vectors.json` holds 33 signed requests and 2 websocket auths produced by the SDK's own
`Signer` with a throwaway random key and no network (`tests/gen_vectors.py`). They cover every
order type and grouping, batches, cancels by id and cloid, cancel-all, modify by id and cloid,
leverage, dead-man switch, eight query actions and the delegation actions. Signing is deterministic,
so `tests/vectors.rs` checks that the EIP-712 digest, the REST body and the websocket `post` frame
match byte for byte, as do the websocket auth message and signature. Never regenerate the vectors
to make a change pass.

### API

```rust
use roshar_synthetix::signer::*;
use roshar_synthetix::types::trading::GetOpenOrders;
use roshar_synthetix::TradeApi;

let signer = Signer::from_hex(&private_key)?;              // owner or delegated signer
let api = TradeApi::mainnet("my-bot/1.0");
let opts = ReqOpts { nonce, expires_after: now_ms + 60_000 }; // you own both

let order = Order {
    symbol: "BTC-USDT", side: Side::Buy, order_type: OrderType::LimitGtc,
    price: "64990.5", trigger_price: "", quantity: "0.0015",
    reduce_only: false, is_trigger_market: false,
    client_order_id: "0x0123456789abcdef0123456789abcdef", close_position: false,
    post_only: true, expires_at: None, trigger_price_type: None, twap: None,
};
let placed = api.place_orders(&signer.sign_place_orders(subaccount_id, &[order], Grouping::Na, opts)?).await?;

let q = GetOpenOrders { symbol: Some("BTC-USDT"), ..Default::default() };
let open = api.query::<GetOpenOrders>(&signer.sign_query(subaccount_id, &q, next_opts)?).await?;

// Trade websocket: connect client.trade_ws_config(), then within 30 s:
let auth = signer.sign_ws_auth(subaccount_id, now_s)?;
client.authenticate(&manager, "auth-1", &auth)?;
client.subscribe_sub_account_updates(&manager, "sub-1", subaccount_id)?;
```

| Item | Notes |
|---|---|
| `ReqOpts.nonce` | Positive, increasing and unique per request. The SDK uses Unix ms, bumped on collision. |
| `ReqOpts.expires_after` | Unix ms deadline (the SDK uses `nonce + 60_000`); `0` = none. Past values are rejected ("expiresAfter must be a future unix timestamp"). |
| `Order.client_order_id` | `""` when unset (the SDK always sends the key). Docs: `0x` + 32 hex. |
| `sign_cancel_all_orders` | `["*"]` for all markets. The exchange rejects `[]`. |
| `sign_query` | Signs `SubAccountAction` with a nonce, as the SDK does. The docs' nonce-less form (changelog 2026-01-06) is not implemented. |
| `sign_ws_auth(sub, timestamp)` | The docs ask for Unix seconds within ±60 s; the SDK sends Unix ms. Both were sent live and reached the account-ownership check. |
| `SignedRequest::ws_post(id)` | Flattens `params` with `signature`, `nonce`, `expiresAfter`, like the SDK's websocket `post`. |

## Live checks (2026-11-15 and 2026-11-16, UK IP)

- Every `POST /v1/info` action above and the `GET` endpoints: 200; responses saved in
  `tests/fixtures/` (files without a `docs_` prefix). Without a `User-Agent`: HTTP 403 HTML page.
- `POST /v1/trade` with a bad signature: 401 `UNAUTHORIZED` ("signature verification failed").
  SDK-signed requests from a random key: 401 "wallet … is not authorized to act on this subaccount",
  so the signature recovered to the signing address.
- Info websocket: `ping`, subscribe/unsubscribe for all four channels and `post` confirmed; pushes
  saved as `ws_*.json`. On 2026-11-16 every `InfoPost` action was sent live and the replies saved as
  `ws_post_*.json` (arrays and maps trimmed to two entries), plus an unsupported action and an
  unknown market; `GET /v1/ws/exchange/status` answered like `/v1/exchange/status`
  (`ws_exchange_status.json`). Trade websocket `auth` reached the same ownership error.
- Private response types (`docs_*.json`) come from the documentation examples; there was no funded
  account to capture them.

## Not covered yet

Deposits (onchain), `withdrawCollateral`, `transferCollateral`, `exchangeCollateral`,
`voluntaryCollateralExchange`, `createSubaccount`, `updateSubAccountName`, referrals, Snaxpot
actions and `getSnaxpotEpoch`, and the queries `getSubAccounts`, `getPositionHistory`,
`getTradesForPosition`, `getBalanceUpdates`, `getTransfers`, `getPerformanceHistory`,
`getFeeRate`, `getWithdrawableAmounts`, `getDelegationsForDelegate`, `getPortfolio`. Gnosis Safe
manager flows. Typed parsing of the individual `subAccountUpdates` event types and of trade-websocket
`post` replies.
