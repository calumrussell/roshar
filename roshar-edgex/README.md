# roshar-edgex

edgeX perpetuals client for the **V2 API**, which is the current one. The docs label V2 "Current"
and V1 "Legacy", `edgex-python-sdk` 2.x is "v2 only", and the Go SDK's v1 branch is deprecated.
The crate covers public market data (REST + websocket config), HMAC request auth, EIP-712 L2 order
signing, and private REST for an onboarded account.

## Principles

Same as the other roshar crates:

- **Mirror the exchange.** Bodies, HMAC headers, typed-data digests and `l2Signature` are
  byte-identical to the official Python SDK. REST replies are parsed strictly. A missing field, or
  a `code` other than `SUCCESS`, is an error that carries the exchange's `code`, `msg` and
  `errorParam`.
- **No caller policy.** Every private call takes the request timestamp. Client order ids, expiries,
  the fee bound and the market-order price bound are inputs. Nothing is read from the environment
  or the clock.

## Modules

| Module | What it does |
|---|---|
| crate root | `EdgexClient` (market REST via the `EdgexApi` trait, `ws_config()`/`ws_config_mut()`, `subscribe_*`/`unsubscribe_*`, `send_pong`), `private_ws_config(creds, account_id, ts)`, `EDGEX_API_URL`, `EDGEX_PUBLIC_WSS_URL`, `EDGEX_PRIVATE_WSS_URL`. |
| `signer` | `ApiCredentials` (HMAC headers), `canonical_value`/`canonical_query` (the signed payload), `L2Signer` (trading key → EIP-712 `LimitOrderParams` / `SetMarginPreferenceParams`), `L2Domain`, `calc_nonce`, `l2_order_amounts`, `market_order_l2_price`. |
| `TradingApi` | Private REST: create order, cancel by id / client id / all, open orders, order history, fills, orders by id, max order size, account asset (balances + positions), account, positions, collateral, leverage, margin mode. Also has `signed_get` and `signed_post` for offline-built requests. |
| `types` | Envelope `EdgexResponse<T>` / `EdgexApiError`, `EdgexPage<T>`, market records, order, fill and account records, request bodies, and public and private websocket frames. |

## API docs

| Area | Reference |
|---|---|
| Overview, hosts, envelope | https://edgex-1.gitbook.io/edgex-documentation/api-v2 |
| REST (public) | https://edgex-1.gitbook.io/edgex-documentation/api-v2/public-api/metadata-api, …/public-api/quote-api, …/public-api/funding-api |
| REST (private) | https://edgex-1.gitbook.io/edgex-documentation/api-v2/private-api/order-api, …/private-api/account-api |
| WebSocket | https://edgex-1.gitbook.io/edgex-documentation/api-v2/websocket-api/public-websocket, …/websocket-api/private-websocket |
| Auth / signing | https://edgex-1.gitbook.io/edgex-documentation/api-v2/authentication (HMAC), https://edgex-1.gitbook.io/edgex-documentation/api-v2/sign (EIP-712) |
| Rate limits | No number published. The overview says only "Public API: Higher rate limits; Private API: based on API key tier", with HTTP 429 when exceeded. |
| Changelog | None. See the "Version History" section of the overview page (last updated 2026-06-11) and https://github.com/edgex-org/edgex-gitbook (`d0d73853`). |
| SDK used for vectors | `edgex-python-sdk` 2.0.1 (PyPI), https://github.com/edgex-Tech/edgex-python-sdk `487274d9` (2026-09-10). Cross-read against https://github.com/edgex-Tech/edgex-golang-sdk `a25b92ed` (2026-09-02). |
| API access | https://pro.edgex.exchange/en-US/edgex-api: API keys need 1,000 EDGE staked. |

Checked against these on 2026-11-15.

**UK access:** the Terms of Use (https://edgex-1.gitbook.io/edgex-documentation/about/terms-of-use)
block "persons … in the United States of America or Canada" and "Restricted Territories" (Iran,
Cuba, North Korea, Syria, Myanmar, Crimea/Donetsk/Luhansk, and countries under comprehensive US
sanctions). The UK is not named. From a UK IP (CloudFront LHR), every public REST and websocket
call worked. Private endpoints returned auth errors (`GATEWAY_HEADER_REQUIRED`, `INVALID_API_KEY`,
WS HTTP 400/401), not a geo-block.

## Coverage

| Item | Endpoint / channel |
|---|---|
| Server time, metadata (coins, contracts, signing domain) | `GET /api/v2/public/meta/getServerTime`, `/getMetaData` |
| Order book | `GET /api/v2/public/quote/getDepth` (`level` 15 or 200 only) |
| Candles | `GET /api/v2/public/quote/getKline`, `/getMultiContractKline` |
| 24 h ticker | `GET /api/v2/public/quote/getTicker`. With no `contractId` it returns `[]` live. |
| Market status / price limits | `GET /api/v2/public/quote/getMarketStatus` |
| Funding, current + history | `GET /api/v2/public/funding/getLatestFundingRate`, `/getFundingRatePage` |
| Public WS | `ticker.{id}`, `ticker.all.1s`, `depth.{id}.{15\|200}`, `trades.{id}`, `kline.{priceType}.{id}.{interval}`, `fundingRate.{id}`, `fundingRate.all`, `metadata` |
| Orders | `POST /api/v2/private/order/createOrder`, `cancelOrderById`, `cancelOrderByClientOrderId`, `cancelAllOrder`, `getMaxCreateOrderSize`; `GET getActiveOrderPage`, `getHistoryOrderPage`, `getHistoryOrderFillTransactionPage`, `getOrderById`, `getOrderByClientOrderId` |
| Account | `GET /api/v2/private/account/getAccountAsset`, `getAccountById`, `getPositionByContractId`, `getCollateralByCoinId`; `POST updateLeverageSetting`, `setMarginMode` |
| Private WS | `wss://…/api/v1/private/ws?accountId=&timestamp=` with HMAC headers. Typed frames: `connected`, `ping`, `trade-event` (`Snapshot`, `ORDER_UPDATE`, asset and funding events). |

There is no order-modify endpoint and no public REST trades endpoint (recent trades come only from
`trades.{id}`). The V2 docs (checked 2026-11-16) list no public open-interest history, long/short
ratio, liquidation or insurance-fund endpoint or channel; current open interest is on the ticker.

Live findings that differ from the docs, all taken from real responses:

- Contract ids are `3000xxxx` (BTCUSDC = `30000001`). The docs' examples use the older `1000xxxx`
  ids.
- Funding records carry `fundingIndex` and `markPrice`, not `starkExFundingIndex`. Websocket
  funding items (`EdgexWsFundingRate`) never carry `forecastFundingRate` and call the settlement
  flag `settlement` (REST: `isSettlement`).
- Websocket `dataType` is `Snapshot` first and then `changed` (lower case). Public payload ids are
  strings, and items have the REST shapes (depth levels are `{price,size}`, not tuples).
  `ticker.*` items carry `bestAskPrice`/`bestBidPrice` only some of the time.
- `metadata` items are not the REST `getMetaData` shape (typed as `EdgexWsMetaData`, docs give
  only `"data": []`). They carry a V1-era `global` (different `contractAddress`/`chainId`, a
  `collateralCoin` object, no signing-domain fields), contracts with `oraclePriceSigner` /
  `oraclePriceSignedAssetId`, and a `multiChain` deposit/withdraw chain list. Only a `Snapshot`
  arrived in 40 s. Use REST `getMetaData` for the L2 domain.
- The server sends `{"type":"ping","time":…}` every 10 s. It answers protocol pings and JSON pings.
  A client that never answered stayed connected for 75 s. `ws_config()` uses protocol pings for
  keepalive; answering the JSON ping (`send_pong`) is up to the caller.

`EdgexClient` rate-limits REST to 5 req/s by default. edgeX publishes no figure; 5 req/s matches
the 50 requests per 10 s that third-party clients observe (e.g. https://github.com/realnaka/edgex-cli).

## Signing

**REST / private WS auth (HMAC-SHA256).** The headers are `X-edgeX-Api-Key`, `X-edgeX-Passphrase`,
`X-edgeX-Timestamp` (ms) and `X-edgeX-Signature`. The signature is
`hex(HMAC_SHA256(key = base64(api_secret), ts + METHOD + path + payload))`.

- For GET, `payload` is the query sorted by key (`k=v&…`, raw values). Lists are comma-joined.
- For POST, `payload` is the SDK's `get_value(body)`: object keys sorted as `k=v` joined by `&`,
  arrays joined by `&`, `null` empty, booleans lower case.
- The private websocket signs `GET` + `/api/v1/private/ws` + `accountId={id}&timestamp={ts}`.

**L2 (EIP-712, secp256k1).** The key is the trading "Signer Key" from API Management → Perps V2 →
SDK Signer. The domain is `{name:"EdgeX", version:"1", chainId: global.nativeChainId (3343),
verifyingContract: global.contractAddress}`.

- `LimitOrderParams(OrderBase base, int256 amountSynthetic, int256 amountCollateral, uint256
  amountFee, uint64 assetIdSynthetic, uint64 assetIdCollateral, bool isBuyingSynthetic, bytes32
  parentOrderHash, uint256 subOrderIndex)`, where `OrderBase(uint256 nonce, address signer, uint64
  accountId, uint256 expirationTimestamp)`. It signs every order type, including market and
  conditional orders.
- `nonce` = the first 32 bits of `sha256(clientOrderId)`. `expirationTimestamp` = `expireTime /
  1000`. `l2ExpireTime` = `expireTime + 8 days`.
- `l2Value` = `l2_price × size`. `l2LimitFee` = `ceil(l2Value × fee_rate)`.
  `amountSynthetic` = `size × contract.resolution`, `amountCollateral` = `l2Value × quote.resolution`,
  `amountFee` = `l2LimitFee × quote.resolution` (all truncated to integers). `parentOrderHash` is
  0 and `subOrderIndex` is 0.
- The SDK uses `fee_rate = max(defaultTakerFeeRate, defaultMakerFeeRate)`. For market orders it
  uses `l2_price` = `market_order_l2_price` (buy: `ceil(oracle × 10 / tick) × tick`, sell: `tick`)
  and sends body `price` `"0"`.
- `SetMarginPreferenceParams(uint64 accountId, uint64 assetId, uint8 marginMode, uint256 nonce,
  address signer)`.
- The signature is `r‖s‖v` (v = 27/28) as hex without `0x`. RFC 6979 makes it deterministic, so it
  is compared exactly.

### Parity

`tests/gen_vectors.py` runs `edgex-python-sdk` 2.0.1 offline. It fakes the HTTP session, metadata,
oracle quote and websocket connect, pins `time.time`, and uses a random throwaway trading key, API
key, secret and passphrase. Its output is `tests/vectors.json`. `tests/vectors.rs` asserts the
following, byte for byte:

- the signer address and `calc_nonce`;
- 10 `createOrder` cases: the full body including `l2Signature`, the typed-data digest, and the
  HMAC headers. The cases are GTC, POST_ONLY + reduce-only with an explicit expiry, IOC, FOK, a
  7-decimal-tick contract, a zero fee rate, market buy/sell, STOP_LIMIT with an `ORACLE_PRICE`
  trigger, and TAKE_PROFIT_MARKET;
- the market-order price bound;
- 2 `setMarginMode` requests;
- 17 other private requests (all the cancel forms, order, fill and account queries, leverage):
  query or body, and headers;
- the private websocket URL and headers.

Never regenerate the vectors to make a change pass.

Not verified against a live account (none available): private reply shapes come from the docs'
examples, so fields some examples omit are `Option`. The Python SDK also sends the auth headers as
a base64 websocket subprotocol for browsers; `roshar-ws-mgr` sends only the headers, as the docs
specify.

## Usage

```rust
use roshar_edgex::signer::*;
use roshar_edgex::types::*;
use roshar_edgex::{EdgexApi, EdgexClient, TradingApi, EDGEX_API_URL};

let client = EdgexClient::new();
let meta = client.get_meta_data().await?;
let btc = meta.contract_list.iter().find(|c| c.contract_id == "30000001").unwrap();
let usdc = meta.coin_list.iter().find(|c| c.coin_id == btc.quote_coin_id).unwrap();

let domain = L2Domain::from_global(&meta.global)?;
let signer = L2Signer::from_hex(&trading_key)?;
let creds = ApiCredentials::new(api_key, passphrase, api_secret);
let api = TradingApi::new(client.http_client(), EDGEX_API_URL, creds, account_id);

let order = NewOrder {
    contract_id: "30000001", side: EdgexSide::Buy, order_type: EdgexOrderType::Limit,
    time_in_force: EdgexTimeInForce::PostOnly, price: "64000.0", size: "0.001",
    client_order_id: "1794780000000000001", expire_time_ms: now_ms + 22 * 86_400_000,
    reduce_only: false, trigger: None,
};
let l2 = OrderL2Inputs {
    l2_price: "64000.0", fee_rate: "0.00045",
    contract_resolution: &btc.resolution, quote_resolution: usdc.resolution.as_deref().unwrap(),
    quote_coin_id: &btc.quote_coin_id,
};
let body = signer.create_order_body(&domain, account_id, &order, &l2)?;
let id = api.create_order(&body, now_ms).await?.order_id;
api.cancel_order_by_client_order_id(&["1794780000000000001"], now_ms).await?;
api.cancel_all_orders(&CancelAllFilter::default(), now_ms).await?;

let ws = roshar_edgex::private_ws_config(&ApiCredentials::new(/* … */), account_id, now_ms);
```

## Not covered yet

- Deposits, withdrawals and transfers: the asset API on `spot.edgex.exchange` (wallet-key EIP-712
  over server-supplied typed data) and the `TransferParams` signature.
- Open TP/SL attached to an order (`openTp`/`openSl`, which need their own L2 signatures), position
  TP/SL, and TWAP.
- Position, collateral and asset-snapshot history pages (`getPositionTransactionPage`,
  `getPositionTermPage`, `getCollateralTransactionPage`, `getAccountAssetSnapshotPage`,
  `getPositionOrders`, the `…ById` variants), `getAccountPage` and `getAccountDeleverageLight`.
- Typed parsing of private `collateralTransaction`, `positionTransaction`, and
  deposit/withdraw/transfer records (kept as `serde_json::Value`).
- `getMultiContractKline` returns up to 200 per contract; paging beyond that is up to the caller.
