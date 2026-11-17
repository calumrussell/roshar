# roshar-gmx

GMX v2 client: GMX API and Oracle API reads (markets, tickers, prices, candles, funding and
borrowing rates, positions, orders, trades), the GMX API stream config, and express (gasless)
order trading: prepare through the GMX API, EIP-712 signing here, submit to GMX Relay.

## Principles

Same as the other roshar crates (see `roshar-hyperliquid/README.md`):

- **Mirror the exchange.** Signatures are byte-identical to `@gmx-io/sdk`. Replies are parsed
  strictly against the published OpenAPI spec and live captures; a non-2xx reply is a
  `GmxApiError` with the API's `code` / `message` / `details`.
- **No caller policy.** Hosts, rate limits, slippage, execution-fee buffers, retries and polling
  are the caller's. Keys are passed in. Nothing reads the environment or the clock.

## Trading path

The GMX docs list two programmatic order paths: SDK v2 express orders through the GMX API, and
RPC-owned transactions (SDK v1 or direct `ExchangeRouter` calls). This crate covers the express
path, which the SDK v2 docs present as the current one:

1. `POST /v2/orders/txns/prepare` (or `edit/prepare`, `cancel/prepare`). The API picks the market,
   prices, acceptable price, execution fee, relay fee and relay nonce, and returns EIP-712 typed
   data (`Batch` over `GmxBaseGelatoRelayRouter` v1) plus `relayParams` / `batchParams`.
2. Validate and sign the typed data (`signer`).
3. `POST /v2/orders/txns/submit`. GMX Relay sends the relay-router transaction, so the account
   needs no gas. Poll `POST /v2/orders/txns/status`.

`mode: classic` prepares return an `ExchangeRouter.multicall` transaction (`to`, `data`, `value`,
`gasLimit`). This crate parses it but does not sign or broadcast transactions (see Not covered).

The SDK sends order transactions to `/v2/...` and reads to `/v1/...`. The OpenAPI spec lists the
order routes under `/v1`. Those routes still answer, but they bind the typed data to the previous
GelatoRelayRouter (`0x5503…4e87`), which SDK 2.9.1 rejects. This crate uses `/v2`.

## Modules

| Module | What it does |
|---|---|
| crate root | `GmxClient::new(api_url, wss_url)`: `GmxApi` reads and `ws_config()` / `subscribe_*` for roshar-ws-mgr. Host constants (`GMX_{ARBITRUM,AVALANCHE,MEGAETH}_API_URL[_AI]`, `GMX_ARBITRUM_WSS_URL`, `GMX_{ARBITRUM,AVALANCHE}_ORACLE_URL`) and chain ids. `GmxOracleApi` handles Oracle API reads. |
| `exchange` | `OrderTxApi::new(api_url, rps)`: `prepare_order`, `prepare_edit_order`, `prepare_cancel_order`, `submit_order`, `order_status`. |
| `signer` | Generic EIP-712 hashing (`hash_typed_data`, `encode_type`, `hash_struct`), `validate_order_typed_data` (the SDK's pre-sign checks), `GmxSigner` (secp256k1, `sign_prepared_order`, `sign_typed_data`), relay router allow-lists per chain. |
| `types` | OpenAPI-derived read models, order transaction requests and responses, stream frames, `GmxApiError`. |

## API docs

| Area | Reference |
|---|---|
| Overview, hosts | https://docs.gmx.io/docs/api/overview, https://docs.gmx.io/docs/api/gmx-api/gmx-io-gmx-public-api |
| REST (GMX API) | OpenAPI https://arbitrum.gmxapi.io/swagger.json; workflows https://docs.gmx.io/docs/api/integration-guide |
| REST (Oracle API) | https://docs.gmx.io/docs/api/rest-api/oracle-prices, https://docs.gmx.io/docs/api/rest-api/markets |
| Orders, signing | https://docs.gmx.io/docs/sdk/v2/readme, https://docs.gmx.io/docs/sdk/v2/examples, https://docs.gmx.io/docs/api/contracts/advanced-entrypoints, https://docs.gmx.io/docs/api/contracts/addresses |
| WebSocket | Not in the docs. `wss://arbitrum.gmxapi.io/v1/stream`, from `@gmx-io/sdk` 2.9.1 `utils/stream` (`WsStreamClient`), confirmed live. |
| Rate limits | None published. The integration guide only says to back off on `429` ("Retries, timeouts, and fallback URLs"). |
| Changelog | https://docs.gmx.io/docs/sdk/changelog (stops at 2.4.0; npm has 2.9.1); contract and API announcements on Telegram `@GMX_Technical_Announcements` (https://docs.gmx.io/docs/api/updates-support) |
| SDK used for vectors | `@gmx-io/sdk` 2.9.1 (npm, gitHead `896a4a9b`, published 2026-11-13) with viem 2.57.20 |

Checked against these on 2026-11-15; market-data coverage re-checked on 2026-11-16.

UK access: GMX's terms (https://gmx.io/terms-and-conditions, source
`gmx-io/gmx-interface` `landing/src/pages/TermsAndConditions`, "Last modified: August 20th, 2026")
exclude U.S. persons and residents of "any country to which the United States, the United Kingdom,
or the European Union embargoes goods or imposes similar sanctions". They do not exclude the UK
itself. From a UK IP, the GMX API (`.io` and `.ai`), the stream, the Oracle API and `prepare`
all answered normally, with no geo-block.

## Coverage

| Item | Endpoint / channel |
|---|---|
| Markets | `GET /v1/markets`, `/v1/markets/info` (config + live pool, OI, funding, borrowing), `/v1/markets/config` and `/v1/markets/values` (the two halves of info), `/v1/risk-oracle/markets`; Oracle `GET /markets`, `/markets/info` (OI, liquidity, pool amounts, rates), `/markets/state` (open/closed) |
| Tickers | `GET /v1/markets/tickers` (`addresses`, `symbols` filters), `/v1/markets/trading-capacity` |
| Tokens, prices | `GET /v1/tokens`, `/v1/tokens/info`, `/v1/prices/tickers`; Oracle `GET /prices/tickers`, `/tokens`, `/signed_prices/latest` |
| Candles | `GET /v1/prices/ohlcv`; Oracle `GET /prices/candles` |
| Funding / borrowing | Current: tickers and markets info/values. History: `GET /v1/rates` (hourly snapshots). `GET /v1/pairs`, `/v1/pairs/solana` (hourly funding % per pair) |
| Trades | `POST /v1/trades/search` (`forAllAccounts: true` is the protocol-wide trade tape; market/direction and order-event filters, cursor paging) |
| Server status | Oracle `GET /ping` |
| Account reads | `GET /v1/positions`, `/v1/positions/{key}`, `/v1/orders`, `/v1/orders/{key}`, `/v1/trades`, `/v1/balances/wallet` |
| Orders | `POST /v2/orders/txns/prepare`, `/edit/prepare`, `/cancel/prepare` (by key or `all`), `/submit`, `/status` |
| Stream | `prices`, `candles:{tokenSymbol}:{1m,5m,15m,1h,4h,1d}`; `{"op":"subscribe"/"unsubscribe","channels":[..]}`; `ack` / `error` / `snapshot` frames parsed by `GmxWsFrame::parse` |

GMX has no order book. Prices are oracle min/max, so there is no depth endpoint to cover. The
stream has only the two channels the SDK uses (`prices`, `candles`); other names (`markets`,
`tickers`, `trades`, `funding`, `rates`, `tokens`) are refused with "unknown channel(s)
ignored". It has no private stream: track orders with `order_status` and the account reads.

Live notes: `/v1/trades/search` rejects `limit` above 300 (the spec says 1000).
`/v1/pairs/solana` answers 503 on Avalanche. Oracle `/ping` returns `{"message":"ok"}` and
`/signed_prices/latest` rows carry an ISO `createdAt`, `minPriceFull` / `maxPriceFull` and a
Data Streams `blob` with the legacy signer fields null; the docs show other shapes, and the
types follow the live replies.

Order types: market, limit, stop-market, take-profit and stop-loss increases and decreases, swaps,
TWAP and TP/SL sidecars. Express orders have no post-only, IOC or client order id. Market orders
fill at the oracle price within `slippage`, and limit or trigger orders rest until their trigger
price. `GmxOrder.order_type` is the contract enum (0 MarketSwap … 8 StopIncrease).

`GmxClient` and `OrderTxApi` rate-limit to 5 requests/s by default. No limit is published, so
this value is a choice, not a documented figure. The stream closed a connection with 1008
"message rate limit exceeded" after about 25 client messages in one second, so put many channels
in one `subscribe`. Only `arbitrum.gmxapi.io` serves the stream; the other hosts return 404.

## Signing

- Typed data comes from the API: the domain is `{name: "GmxBaseGelatoRelayRouter", version: "1",
  chainId, verifyingContract: <relay router>}` and the primary type is `Batch(address account,
  CreateOrderParams[] createOrderParamsList, UpdateOrderParams[] updateOrderParamsList,
  bytes32[] cancelOrderKeys, bytes32 relayParams, bytes32 subaccountApproval)`.
  `hash_typed_data` implements EIP-712 for any typed data: structs, arrays (dynamic and fixed),
  `string`, `bytes`, `bytesN`, `bool`, `address` (EIP-55 enforced on mixed case), `uintN` and
  `intN` (decimal, `0x` or JSON integer, range-checked).
- The response carries no `primaryType`. The SDK uses the first key of `types`, but this crate
  uses the one struct no other struct references. The two rules agree for every GMX payload.
- The signature is secp256k1 over `keccak256(0x1901 || domainSeparator || hashStruct(Batch))`,
  with RFC 6979 and low-s. Its format is `0x || r || s || v` (v 27/28), as in viem `signTypedData`.
- `validate_order_typed_data` runs the checks of SDK `validateOrderTypedData`: domain name, version
  and chain id; `verifyingContract` in a caller-supplied allow-list (`ARBITRUM_RELAY_ROUTERS` etc.,
  copied from SDK 2.9.1 `configs/contracts`); and order `receiver` / `cancellationReceiver` set
  to zero, the signer or the given account. The docs' addresses page still lists the old
  GelatoRelayRouter, so the allow-lists follow the SDK. `relayParams` reaches the signer only as a
  hash, and the SDK does not check it either.

### Parity

`tests/gen_vectors.mjs` reads the captured `prepare` responses in `tests/fixtures` (increase limit,
increase market with SL, decrease market, edit, cancel, cancel-all). It signs them with
`signPreparedOrder` and a throwaway random `PrivateKeySigner`, without network access, and records
viem's `hashDomain` / `hashStruct` / `hashTypedData` for each. It also records 10 validation cases
(SDK accept or reject) and a synthetic typed-data message that covers the other EIP-712 types.
`tests/vectors.rs` checks that the hashes and signatures are byte-identical and that the
validation outcomes match. Signing is deterministic, so the test needs no nonce recovery. Never
regenerate the vectors to make a change pass.

```
mkdir -p /tmp/gmx-sdk && cd /tmp/gmx-sdk && npm init -y && npm install @gmx-io/sdk@2.9.1
node /path/to/roshar-gmx/tests/gen_vectors.mjs
```

### API

```rust
use roshar_gmx::{OrderTxApi, GMX_ARBITRUM_API_URL, ARBITRUM_CHAIN_ID};
use roshar_gmx::signer::{GmxSigner, ARBITRUM_RELAY_ROUTERS};
use roshar_gmx::types::*;

let signer = GmxSigner::from_hex(&private_key)?;
let account = signer.address();
let api = OrderTxApi::new(GMX_ARBITRUM_API_URL, 5);          // one host per order chain

let mut req = GmxPrepareOrderRequest::new(
    GmxOrderKind::Increase, "ETH/USD [WETH-USDC]", GmxOrderType::Limit,
    "100000000000000000000000000000000",                      // $100, 30 decimals
    GmxTxMode::Express, account.clone(),
);
req.direction = Some(GmxDirection::Long);
req.trigger_price = Some("2000000000000000000000000000000000".into());
req.collateral_token = Some("USDC".into());
req.collateral_to_pay = Some(GmxTokenAmount { token: "USDC".into(), amount: "10000000".into() });
let prepared = api.prepare_order(&req).await?;               // inspect prepared.estimates

let sig = signer.sign_prepared_order(&prepared, ARBITRUM_CHAIN_ID, None, ARBITRUM_RELAY_ROUTERS)?;
let submit = GmxSubmitOrderRequest::express(&prepared, sig, account.clone(), None).unwrap();
let sent = api.submit_order(&submit).await?;
let status = api.order_status(&sent.request_id).await?;      // poll until terminal

let cancel = GmxCancelOrderRequest {
    order_ids: Some(vec![order_key]), all: None, mode: GmxTxMode::Express,
    from: account.clone(), subaccount_address: None, subaccount_approval: None,
};
let prepared = api.prepare_cancel_order(&cancel).await?;     // then sign + submit as above
```

| Item | Notes |
|---|---|
| Units | `size`, `trigger_price`, acceptable prices: 30-decimal USD. Collateral amounts: token decimals. Orders read back store prices in contract format (`price30 / 10^indexTokenDecimals`). |
| Collateral | Express orders take collateral and the relay fee from the wallet through the `Router` allowance, or from a GMX Account balance. The allowance has to be granted on-chain once (ERC-20 `approve`). |
| Subaccounts | Pass `subaccount_address` / `subaccount_approval` to prepare, sign with the subaccount's `GmxSigner` and `account_address = Some(main)`, and put the same approval in `GmxSubmitOrderRequest::express`. |
| Hosts | Prepared requests exist only on the host that served them. The SDK says: after a 5xx or a status 404, prepare again (check open orders first) rather than switch host. |
| Expiry | `prepared.expires_at` (unix s). The API chooses the relay `deadline` and `userNonce`. |

## Not covered yet

- Signing and broadcasting classic `ExchangeRouter` transactions: EIP-1559 signing, RPC nonce
  and gas handling, `eth_sendRawTransaction`. Express orders cover trading without them.
- Onboarding and funding: ERC-20 approvals (`/v1/allowances`), GMX Account deposits and
  withdrawals, subaccount generation and approval signing (`/v2/subaccounts/*`), collateral
  deposit and withdraw prepares (`/v2/orders/txns/collateral/prepare`), and the generic
  `/v1/relay/*` routes.
- Reads that are not market data: `/v1/apy`, `/v1/performance/*`, `/v1/yield/*`, `/v1/stats/*`,
  `/v1/jit/*` (GLV shift liquidity), `/v1/staking/*`, `/v1/paxos/*`, `/v1/whitelists`,
  `/v1/incentives/*`, `/v1/buyback/*`, the Oracle API's `/apy`, and GraphQL (Subsquid) history.
- Oracle `GET /markets/tickers`: listed in the docs' operational notes, but answers 404 on
  Arbitrum (with and without `symbols` / `addresses`) as of 2026-11-16. The GMX API
  `/v1/markets/tickers` is covered.
- Typed `relayParams` / `batchParams` / `gasPaymentParams`. They are echoed back as opaque JSON.
