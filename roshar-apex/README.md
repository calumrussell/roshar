# roshar-apex

ApeX Omni client: public market data (REST + websocket config), API-key authenticated trading REST,
the private account websocket, and native zkLink order signing.

## Principles

Same as the other roshar crates (see `roshar-hyperliquid/README.md`):

- **Mirror the exchange.** Order payloads, zk signatures and API-key signatures are byte-identical to
  the official Python SDK. REST replies are parsed strictly; an error envelope (`{"code","msg"}`,
  sent with HTTP 200) is returned as `types::ApexError`.
- **No caller policy.** The library does not choose client ids, expirations, fee rates, prices,
  timestamps, retries or topics. Credentials and zk seeds are passed in. Nothing is read from the
  environment or the clock: every private call takes `timestamp_ms`.

UK access: ApeX's terms bar US persons, sanctioned countries and "any other persons ... whose use of
APEX Software is contrary to any applicable law"; the UK is not listed
(https://apex-pro.gitbook.io/apex-pro/about/terms-of-use, "3. Restricted Persons"). From a UK IP on
2026-11-15 every public REST endpoint and both websockets answered normally; private endpoints
returned credential errors, not a geo-block.

## API docs

| Area | Reference |
|---|---|
| REST, WebSocket, Auth/signing, Rate limits | https://api-docs.omni.apex.exchange (single page: "General", "Trade", "Account", "PublicApi", "Public Websocket", "Private Websocket", "Rate Limits") |
| Changelog | None for the API. SDK changelog: https://github.com/ApeX-Protocol/apexpro-openapi/blob/main/CHANGELOG.md |
| SDK used for vectors | `apexomni` 3.4.2 (PyPI, latest), repo https://github.com/ApeX-Protocol/apexpro-openapi at `11e63a0a` (2026-09-21) |
| zk signer source | https://github.com/zkLinkProtocol/zklink_sdk branch `apex-devnet` (`b5be87f`), https://github.com/zkLinkProtocol/franklin-crypto branch `poseidon` (`3bafa7f3`) |
| Node SDK (typed models) | https://github.com/ApeX-Protocol/apexomni-connector-node (`ca1d7e54`) |
| Terms | https://apex-pro.gitbook.io/apex-pro/about/terms-of-use |

Checked against these on 2026-11-15; public market-data sections re-checked on 2026-11-16 (the
"PublicApi" section documents exactly the seven REST endpoints below, "Public Websocket" exactly
the five channels below; all are covered).

## Modules

| Module | What it does |
|---|---|
| crate root | `ApexClient` (market REST via the `ApexApi` trait, `ws_config()` / `ws_private_config()` for roshar-ws-mgr, subscribe/unsubscribe, `login_private`, `send_pong`, `trading_api(creds)`), URL constants. |
| `rest::MarketApi` | `GET /v3/time`, `/v3/symbols`, `/v3/depth`, `/v3/trades`, `/v3/klines`, `/v3/ticker`, `/v3/history-funding`. |
| `rest::TradingApi` | Orders, cancels, open/history orders, fills, account, balance, funding payments, worst price, leverage. |
| `order` | `sign_order` builds the zkLink `Contract` message, signs it, computes `limitFee` and the form fields. Also `contract_ids`, `pack_token_amount`, `scale_decimal`, `fee_rate_bps`, `limit_fee`. |
| `zk` | `ZkSigner`: key from `seeds`, `l2Key`, public key hash, `sign_musig`. |
| `auth` | `ApexApiCredentials`: `APEX-SIGNATURE` HMAC and the private websocket login frame. |
| `types` | REST and websocket wire types, `ApexTopic` topic builders, `ApexWsRequest` (subscribe, ping, pong), `ApexWsPublicMessage::parse` (every public websocket frame: pushes, server ping, success/error replies). |

## Coverage

| Item | Endpoint / channel |
|---|---|
| Instruments, assets, `l2PairId`, fee assets | `GET /v3/symbols` |
| Ticker + current/predicted funding | `GET /v3/ticker?symbol=BTCUSDT` |
| Order book | `GET /v3/depth?symbol=BTCUSDT[&limit]` |
| Recent trades | `GET /v3/trades?symbol=BTCUSDT[&limit]` (≤ 500) |
| Candles | `GET /v3/klines?symbol&interval[&start&end&limit]` (seconds, ≤ 200) |
| Funding history | `GET /v3/history-funding?symbol=BTC-USDT[&limit&page&beginTimeInclusive&endTimeExclusive]` |
| Public WS | `orderBook{25,200}.H.{sym}`, `recentlyTrade.H.{sym}`, `instrumentInfo.{H,M}.{sym}` (ticker + funding), `instrumentInfo.all`, `candle.{interval}.{sym}`; typed via `ApexWsPublicMessage`. The `M` frequency for `orderBook*` and `recentlyTrade` is rejected live (`error:handler not found`, 2026-11-16); the topic builder still accepts it since it is documented. |
| Place order | `POST /v3/order` (limit, market, stop/take-profit; GTC, FOK, IOC, POST_ONLY; reduce-only; `clientId`) |
| Cancel | `POST /v3/delete-order` (id), `/v3/delete-client-order-id` (client id), `/v3/delete-open-orders` (all / by symbols) |
| Orders | `GET /v3/open-orders`, `/v3/history-orders`, `/v3/order?id`, `/v3/order-by-client-order-id?id` |
| Fills | `GET /v3/fills` |
| Account, positions, balances | `GET /v3/account`, `/v3/account-balance` |
| Funding payments | `GET /v3/funding` |
| Market-order price | `GET /v3/get-worst-price` |
| Leverage | `POST /v3/set-initial-margin-rate` |
| Private WS | login + `ws_zk_accounts_v3` (orders, fills, positions, wallets), `ws_notify_v1` (subscribe only) |

Symbols come in two forms: `BTC-USDT` (`symbol`: orders, account, funding history) and `BTCUSDT`
(`crossSymbolName`: depth, trades, klines, ticker, websocket topics). The exchange does not modify
orders; cancel and replace.

`ApexClient` rate-limits REST to 10 requests/s by default: the documented IP limit is 600 requests
per 60 s. Private endpoints are also limited per account to 600 GETs and 300 POSTs per 60 s, and
open orders to 200 per user; `trading_api()` shares the client's limiter.

### Websocket keepalive

The server sends `{"op":"ping","args":["<ms>"]}` every 15 s and closes the connection about 150 s
later unless each is answered with `{"op":"pong","args":[same]}`. Client pings (protocol or
`{"op":"ping"}`) are answered but do not keep the session alive (checked live: a connection sending
fresh client pings every 15 s was closed after 153 s; one answering the server's pings was still
open after 200 s). `ws_config()` uses protocol pings for liveness; the caller must answer server
pings: `ApexWsRequest::pong_reply(text)` returns the pong for a ping frame, `ApexClient::send_pong`
sends it.

## Signing

Two signatures per order:

- **API key** (every private request): `APEX-SIGNATURE = base64(HMAC-SHA256(key =
  base64(secret), timestamp + METHOD + path + data))`. `path` includes `/api` and, for GETs, the
  query string; `data` is the POST form sorted by key, `k=v` joined by `&`. Headers: `APEX-API-KEY`,
  `APEX-PASSPHRASE`, `APEX-TIMESTAMP` (ms). The websocket login signs `timestamp + "GET" +
  "/ws/accounts"`.
- **zkLink** (order body `signature`): musig over Alt-BabyJubjub (twisted Edwards over BN254 `Fr`,
  generator `FixedGenerators::SpendingKeyGenerator`) with a Rescue (BN254, width 3, rate 2, 22
  rounds, `x^(1/5)`/`x^5` S-boxes) challenge.
  - Key: `sk = sha256(sha256(seeds))`, re-hashed until below the subgroup order. `l2Key` = packed
    `sk * G` (`y` little-endian, top bit = parity of `x`).
  - Message: zkLink `Contract`, 36 bytes: `0xfe || account_id u32 || sub_account u8 || slot u16 ||
    nonce u24 || pair u8 || is_long u8 || size (35-bit mantissa, 5-bit decimal exponent) || price
    u120 || maker_fee_bps u8 || taker_fee_bps u8 || has_subsidy u8`, all big-endian.
  - Ids: `n = sha256(clientId)`, `nonce = n mod (2^32-1)`, `slot = (n mod (2^64-1)) / (2^32-1)` (the
    SDK's float division, reproduced exactly), `account_id = id mod (2^32-1)`. Size and price are
    scaled by `10^decimals` of the contract's settle asset (18 for USDT perps) and truncated; fee
    rates are `ceil(rate * 10000)`.
  - Signature: `h = rescue_hash(bits(msg) padded to 736)`, `k = HMAC-SHA256 DRBG(sk, sha256(h))`,
    `R = k*G`, `e = rescue(pk.x || R.x || h)` (low 125 bits of two squeezes), `s = k + e*sk`;
    `pack(R) || s_le`, hex with `0x`.
  - `limitFee = size * price * takerFeeRate`, rounded up to 6 decimals. `expiration` (ms) is not
    signed; the SDK sends `now + 28 days`.

The zk signer is a port of `zklink_sdk`'s `ZkLinkSigner` and the franklin-crypto code it calls, with
no new dependency: `zklink_sdk` and the zkLink franklin-crypto fork are not published on crates.io
(git-only, pulling ethers, starknet-rs and uniffi), so depending on them was not an option. Field and
curve arithmetic is plain 4-limb Montgomery; Rescue round constants and MDS were generated from the
franklin-crypto parameter derivation and are in `src/zk/constants.rs`.

### Parity

`tests/vectors.json` was produced by `tests/gen_vectors.py` with `apexomni` 3.4.2's native signer,
throwaway seeds and API credentials, and no network (`create_order_v3` runs against an in-memory
config and a captured `_post`). The signature scheme is deterministic, so `tests/vectors.rs`
compares bytes directly:

- `l2Key` and public key hash for 3 seeds; `sign_musig` for 12 (seed, message) pairs, including the
  92-byte maximum.
- 6 orders (limit GTC buy, post-only reduce-only sell, IOC market with a client id where the SDK's
  float slot rounding differs from integer division, FOK stop-market with trigger, DOGE size/price
  extremes, a 6-decimal settle asset): contract fields, contract bytes, zk signature, `limitFee`,
  the form, and its `APEX-SIGNATURE`.
- `APEX-SIGNATURE` for GET and POST requests, and the websocket login signature.

The SDK posts `slLimitFee=None` and `tpLimitFee=None` (Python's `str(None)`) on orders without
attached TP/SL; this crate omits them. Never regenerate the vectors to make a change pass.

## Usage

```rust
use roshar_apex::{ApexClient, ApexApi};
use roshar_apex::auth::ApexApiCredentials;
use roshar_apex::order::*;
use roshar_apex::zk::ZkSigner;

let client = ApexClient::new();
let symbols = client.get_symbols().await?;
let btc = symbols.contract_config.perpetual_contract.iter().find(|c| c.symbol == "BTC-USDT").unwrap();
let decimals = symbols.contract_config.assets.iter().find(|a| a.token == btc.settle_asset_id).unwrap().decimals;

let trading = client.trading_api(ApexApiCredentials { key, secret, passphrase });
let account = trading.get_account(now_ms).await?;
let signer = ZkSigner::from_seeds_hex(&seeds)?;
assert_eq!(signer.l2_key_hex(), account.l2_key);

let ctx = ApexOrderContext {
    account_id: &account.id, l2_pair_id: btc.l2_pair_id_u16()?, settle_decimals: decimals,
    taker_fee_rate: &account.contract_account.taker_fee_rate,
    maker_fee_rate: &account.contract_account.maker_fee_rate,
};
let order = sign_order(&signer, &ctx, &ApexOrderRequest {
    symbol: "BTC-USDT", side: ApexSide::Buy, order_type: ApexOrderType::Limit,
    size: "0.01", price: "60000.0", time_in_force: ApexTimeInForce::PostOnly,
    reduce_only: false, client_id: &my_client_id, expiration_ms: now_ms + 28 * 86_400_000,
    trigger_price: None, trigger_price_type: None, trailing_percent: None, is_position_tpsl: false,
})?;
let placed = trading.create_order(&order, now_ms).await?;
trading.cancel_order_by_client_id(&my_client_id, now_ms).await?;

// Websocket: client.ws_config() / ws_private_config() into roshar-ws-mgr, then
client.subscribe_depth(&manager, &["BTCUSDT"], ApexBookDepth::D25, ApexWsFrequency::High)?;
client.login_private(&manager, &creds, now_ms, &[APEX_TOPIC_ACCOUNT])?;
// on every text frame: if let Some(pong) = ApexWsRequest::pong_reply(&text) { send it back }
```

## Not covered yet

- Onboarding and key management: `generate-nonce`, `onboarding`, zk key derivation from an
  Ethereum signature, `change-pub-key`, API-key create/delete, `modify-user`.
- Deposits, withdrawals, transfers (fund ↔ contract), repayment, withdrawal fees and limits.
- Open TP/SL legs on `POST /v3/order` (`isOpenTpslOrder`, `sl*`/`tp*` fields), `brokerId`,
  `sourceFlag`, batch cancels (`delete-orders`, `delete-client-order-ids`), `order-fills`.
- Historical PnL, yesterday PnL, historical asset value.
- RWA (stock) sub-accounts and their API key flow; spot.
- Typed `ws_notify_v1` contents; private push collections other than orders, fills, positions and
  contract wallets.
- The SDK's client-side tick-size check on price (the exchange rejects off-tick prices).

Private response types follow the documented examples (no account was available to capture live
private replies); fields the docs mark optional or that differ between examples are `Option`.
