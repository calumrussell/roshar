# roshar-lighter

Lighter client: public market data (REST + websocket config), native transaction signing, REST
transaction submission, and the websocket messages for trading.

## Principles

Same as the other roshar crates (see `roshar-hyperliquid/README.md`):

- **Mirror the exchange.** Transaction JSON, hashes and signatures are byte-identical to the official
  signer. REST replies are parsed strictly. A missing field, or a `code` other than 200, is an error.
- **No caller policy.** The library does not choose nonces, expiries, client order ids, retries or
  channels. Credentials are passed in. Nothing is read from the environment or the clock.

## Modules

| Module | What it does |
|---|---|
| `signer` | Native Lighter signing: Poseidon2 hashing and Schnorr signatures over ECgFp5. `ApiKey`, `Signer`, the tx structs, `SignedTx`, `auth_token`. |
| `exchange` | `TxApi::new(LIGHTER_API_URL)` provides `send_tx`, `send_tx_batch`, `next_nonce`, `api_key`, `account`, `order_book_detail` and `order_book_orders_raw`. |
| `ws_tx` | Message builders for the websocket: `send_tx`, `send_tx_batch` (`jsonapi/sendtx[batch]`) and `subscribe_account_orders/market/tx` (authenticated channels). |
| crate root | `LighterClient` (market REST + `ws_config()` for roshar-ws-mgr), `LIGHTER_WSS_URL`, `LIGHTER_READONLY_WSS_URL`. `types::LIGHTER_WS_PING` is the keepalive frame. |

## API docs

`apidocs.lighter.xyz` now redirects to `docs.lighter.xyz/api`.

| Area | Reference |
|---|---|
| REST | https://docs.lighter.xyz/api (per-endpoint OpenAPI, e.g. https://docs.lighter.xyz/api/orders/orderbooks, https://docs.lighter.xyz/api/transactions/sendtx) |
| WebSocket | https://docs.lighter.xyz/api/websocket |
| Endpoints, chain ids | https://docs.lighter.xyz/developers/getting-started/environments |
| Auth / signing | https://docs.lighter.xyz/developers/integration-guides/signing-transactions, https://docs.lighter.xyz/developers/integration-guides/api-keys (auth token), reference signer https://github.com/elliottech/lighter-go |
| Tx types, errors | https://docs.lighter.xyz/developers/reference/data-structures-constants-and-errors |
| Rate limits | https://docs.lighter.xyz/developers/reference/rate-limits |
| Changelog | None published; API changes are announced in Discord `#api-updates`. |

Checked against these on 2026-11-15. The signer matches `lighter-sdk` 1.1.8 (latest on PyPI) and
lighter-go up to `74e2b996` (2026-11-13). Later lighter-go commits did not change the create,
cancel, cancel-all or modify tx formats.

Lighter's terms exclude UK residents. From a UK IP, `wss://…/stream` and `POST /api/v1/sendTx`
return HTTP 400 with code 20558 ("restricted jurisdiction"). Public REST GETs and
`LIGHTER_READONLY_WSS_URL` still work.

## Coverage

| Item | Endpoint / channel |
|---|---|
| Status / server time | `GET /` |
| Markets (perp, spot, binary options) | `GET /api/v1/orderBooks` |
| Market details (perp + spot) | `GET /api/v1/orderBookDetails` (`market_id`, `filter` optional) |
| Resting orders | `GET /api/v1/orderBookOrders` (`limit` 1..=250, required) |
| Recent trades | `GET /api/v1/recentTrades` (`limit` 1..=100, required) |
| Trade history | `GET /api/v1/trades` (public filters: `market_id`, `market_type`, `type`, `ask_filter`, `from`, `cursor`; `sort_by` + `limit` required) |
| Candles | `GET /api/v1/candles`, `GET /api/v1/markPriceCandles`, `GET /api/v1/marketPriceCharts` |
| Funding | `GET /api/v1/fundings` (history), `GET /api/v1/funding-rates` (cross-venue current) |
| Exchange info | `GET /api/v1/exchangeStats`, `/assetDetails`, `/priceOracleInfo` (index/mark sources), `/feeTiers` |
| Binary options | `GET /api/v1/events`, `GET /api/v1/upDownCandles` (timestamps in **seconds**: ms is rejected with 20001 despite the docs) |
| Public WS | `order_book/{m}`, `ticker/{m}`, `trade/{m}`, `market_stats/{m}`, `market_stats/all`, `market_stats` + `market_ids`, `spot_market_stats` (same three forms), `partial` updates, `bo_market_stats/{m}`, `bo_market_stats/all`, `candle/{m}/{r}`, `mark_price_candle/{m}/{r}`, `up_down_candle/{s}/{d}/{r}`, `height`. Builders: `types::LighterWssMessage`; parsing: `types::LighterWsMessage::parse` (or `parse_partial` on sockets with `partial` subscriptions: the wire does not mark partial frames) |
| Tx submission | `POST /api/v1/sendTx`, `POST /api/v1/sendTxBatch`, WS `jsonapi/sendtx`, `jsonapi/sendtxbatch` |
| Account reads | `GET /api/v1/nextNonce`, `/apikeys`, `/account`, `/orderBookDetails?market_id=` (perp or spot) |
| Private WS | `account_orders/{m}/{a}`, `account_market/{m}/{a}`, `account_tx/{a}` (auth token) |

`LighterClient` rate-limits its REST calls to 2 every 3 s by default: the Standard tier allows 60
requests a minute, and `recentTrades` (weight 600) only 40. `TxApi` is not rate-limited.

## Signing

The signing code is ported from Lighter's Go signer: `github.com/elliottech/lighter-go`, built into
`lighter-sdk` 1.1.8, and `github.com/elliottech/poseidon_crypto` v0.0.15. There is no FFI.

- A transaction is hashed as a list of Goldilocks field elements:
  `[chain_id, tx_type, nonce, expired_at, account_index, api_key_index, <tx fields>]`. The hash is
  Poseidon2 (width 12, rate 8, no padding), and the first 5 output lanes form a GF(p^5) element.
- With attributes (here only cancel-all's market index), the hash is
  `H(tx_hash || H(sorted (type, value) pairs, padded to 4))`.
- Signing is Schnorr over ECgFp5: `r = k*G`, `e = H(enc(r) || m) mod n`, `s = k - e*sk`. The
  signature is `s || e`, each 40 bytes little-endian. `tx_info.Sig` is that signature in base64.
  `tx_hash` is the message hash in hex.
- The auth token is `"{deadline}:{account}:{key}:" + hex(sig)`. It signs the Poseidon2 hash of the
  ASCII string `"{deadline}:{account}:{key}"`, read as 8-byte little-endian limbs.
- `k` is drawn fresh for every signature, so signatures are not deterministic. The same goes for the
  Go signer.

### Parity

`tests/vectors.json` holds transactions signed by the Python SDK's signer library, with a throwaway
random key and no network. They were generated by `tests/gen_vectors.py`. The vectors cover create
order (post-only, reduce-only IOC market, GTT, extreme values), cancel, cancel-all (one market, all
markets, scheduled, abort), modify, the auth token and the public key, on chain ids 304 and 300.

The library's `k` is random, so each test recovers it from the recorded signature
(`k = s + e*sk`), signs again with that `k`, and checks that `tx_info` (with `Sig`), `tx_hash` and
`tx_type` match exactly. It also checks that freshly signed (random `k`) transactions verify. Never
regenerate the vectors to make a change pass.

### API

```rust
use roshar_lighter::signer::*;
use roshar_lighter::exchange::{TxApi, LIGHTER_API_URL};

let signer = Signer::new(ApiKey::from_hex(&private_key)?, account_index, api_key_index, MAINNET_CHAIN_ID)?;
let api = TxApi::new(LIGHTER_API_URL)?;
assert_eq!(api.api_key(account_index, api_key_index).await?.public_key, signer.public_key_hex());

let mut nonce = api.next_nonce(account_index, api_key_index).await?;   // you own the nonce
let opts = |n| TxOpts { nonce: n, expired_at_ms: now_ms + DEFAULT_TX_LIFETIME_MS };

let order = CreateOrder {
    market_index: 45, client_order_index: 1763200000001, base_amount: 3500, price: 2946,
    is_ask: false, order_type: OrderType::Limit, time_in_force: TimeInForce::PostOnly,
    reduce_only: false, trigger_price: 0, order_expiry: now_ms + 330_000,
};
let tx = signer.sign_create_order(&order, opts(nonce))?;
nonce += 1;
let reply = api.send_tx(&tx).await?;                       // or ws_tx::send_tx(id, &tx) over the WS

let cancel_all = CancelAllOrders { time_in_force: CancelAllTif::Immediate, time: 0, market_index: Some(45) };
api.send_tx(&signer.sign_cancel_all_orders(&cancel_all, opts(nonce))?).await?;

let auth = signer.auth_token(now_s + 8 * 3600)?;           // private WS channels
let sub = roshar_lighter::ws_tx::subscribe_account_orders(45, account_index, &auth);
```

| Item | Notes |
|---|---|
| `ApiKey::from_hex(key)` | 40-byte hex, with or without `0x`. `public_key_hex()` returns the registered public key. |
| `Signer::new(key, account_index, api_key_index, chain_id)` | `MAINNET_CHAIN_ID` = 304, `TESTNET_CHAIN_ID` = 300. |
| `sign_create_order`, `sign_cancel_order`, `sign_cancel_all_orders`, `sign_modify_order` | Each takes the tx and `TxOpts { nonce, expired_at_ms }`, and returns `SignedTx { tx_type, tx_info, tx_hash }`. Rejects what the Go `Validate` rejects. |
| `auth_token(deadline_unix_s)` | Lighter allows at most 8 h. |
| `CreateOrder.order_expiry` | Absolute ms. It must be 0 for IOC and market orders, and non-zero for resting limit orders. The SDK's `-1` default means `now + 28 days`; pass that explicitly. |
| `CancelAllOrders.market_index` | `Some(m)` limits an immediate cancel-all to one market. `None` cancels everywhere. |

Nonces are per API key and strictly sequential. A tx the API rejects does not consume its nonce, so
every later in-flight nonce on that key fails too. Re-fetch `next_nonce` after any rejection. Two
processes must never share an API key.

## Not covered yet

Grouped orders (OTO/OCO), transfers, withdrawals, leverage and margin updates, pool transactions and
change-pub-key (which needs an L1 signature). Integrator and self-trade attributes, and the
skip-nonce attribute, are not exposed either. Add each with vectors from `gen_vectors.py`.

Market data: `/api/v1/syntheticSpotInfo` (RWA pricing parameters; SPY, XAU, EURUSD, NVDA and
BRENTOIL all return 20001 "market does not have a synthetic spot info", so no live shape to build from).
Deliberately out of scope: `exchangeMetrics`/`executeStats` (analytics), `tokenlist` (logos and
marketing metadata), `systemConfig`/`info`/`layer1BasicInfo` (pool and L1 contract config),
`export/historicalTrades` (paid, authenticated), the `pool_data`/`pool_info` channels (vaults)
and the whitelist-only `rfq` channel. Account channels other than the three above
(`account_all*`, `user_stats`, `notification`, `pool_*`) and the `sendTx` `price_protection`
form field are not exposed.
