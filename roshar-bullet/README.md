# roshar-bullet

Bullet (bullet.xyz, ex-Zeta; a Solana network-extension rollup) client: public market data and
account reads over the Binance-FAPI-shaped REST API, websocket topics, and ed25519-signed borsh
transactions for trading, submitted over REST or the websocket.

## Principles

Same as the other roshar crates:

- **Mirror the exchange.** Transaction bytes and signatures are byte-identical to the official
  Rust SDK. REST and websocket replies are parsed strictly; a missing field is an error. `Option`
  is used only where the OpenAPI spec marks a field optional/nullable or the docs say it is absent
  on some messages. Non-2xx replies surface the API's `{"status","message","details"?}` envelope.
- **No caller policy.** The library does not choose uniqueness values, fees, gas, client order
  ids, request ids, retries or refresh. Keys and the chain hash are passed in. Nothing is read
  from the environment or the clock.

## Modules

| Module | What it does |
|---|---|
| crate root | `BulletClient::new(BulletNetwork)`: REST (`BulletApi` trait, or `market()` / `account()`), `ws_config()` for roshar-ws-mgr, `subscribe_*` / `unsubscribe_*`, `send_tx`. URL constants for mainnet, testnet, staging. |
| `signer` | `UnsignedTransaction`, `UserAction` (place, amend, cancel, cancel market, cancel all, cancel-and-place, max leverage), `Signer` (ed25519; secret, hex or 64-byte Solana keypair), `SignedTransaction`, `parse_chain_hash`, `decode_address`, `verify`. |
| `ws_tx` | `tx_request(id, BulletTxMethod, &tx)`: `order.place`, `order.cancel`, `order.amend`, `order.cancelAll`, `submit`. |
| `types` | REST payloads, `BulletApiError`. `types::ws`: `BulletTopic`, `subscribe` / `unsubscribe` / `ping` / `list_subscriptions` builders, `BulletWsMessage::parse`. |

## API docs

| Area | Reference |
|---|---|
| REST | https://tradingapi.bullet.xyz/docs/rest (OpenAPI: https://tradingapi.bullet.xyz/docs/rest/openapi.json, version 0.1.0) |
| WebSocket | https://tradingapi.bullet.xyz/docs/ws/index.html, https://tradingapi.bullet.xyz/docs/consistency-guarantee.html |
| Auth / signing | https://tradingapi.bullet.xyz/docs/tx-signing.html, https://tradingapi.bullet.xyz/docs/tx-fields.html, https://tradingapi.bullet.xyz/docs/delegate-accounts.html, schema `GET /rollup/schema` |
| Rate limits | https://tradingapi.bullet.xyz/docs/connectivity.html |
| Changelog | https://tradingapi.bullet.xyz/docs/changelog.html (latest entry 2026-11-13) |
| All docs in one page | https://tradingapi.bullet.xyz/docs/print.html, protocol docs https://docs.bullet.xyz |
| SDK used for vectors | `bullet-rust-sdk` 0.0.48 from crates.io (repo https://github.com/bulletxyz/bullet-rust-sdk HEAD `79e25490`, 2026-11-10, is also 0.0.48) with `bullet-exchange-interface` 0.16.3 |

Checked against these on 2026-11-15.

UK access: Bullet's terms (https://docs.bullet.xyz/legals/terms-and-conditions, PDF dated
2026-07-06, §4) restrict persons in "the United States of America or Singapore" and sanctioned
jurisdictions; the UK is not listed (it appears only as a sanctions-list authority). From a UK IP
every public REST endpoint, the websocket (`wss://tradingapi.bullet.xyz/ws`) and `POST /tx/submit`
answered normally; no geo-block response.

## Coverage

| Item | Endpoint / channel |
|---|---|
| Ping, server time (µs) | `GET /fapi/v1/ping`, `/fapi/v1/time` |
| Markets, assets, chain hash | `GET /fapi/v1/exchangeInfo` |
| Order book | `GET /fapi/v1/depth` |
| Recent trades | `GET /fapi/v1/trades` |
| Tickers | `GET /fapi/v1/ticker/24hr`, `/fapi/v1/ticker/price` |
| Mark, index, current funding | `GET /fapi/v1/premiumIndex` |
| Funding history, funding config | `GET /fapi/v1/fundingRate`, `/fapi/v1/fundingInfo` |
| Open interest, symbol config, brackets | `GET /fapi/v1/openInterest`, `/fapi/v1/symbolConfig`, `/fapi/v1/leverageBracket` |
| Oracle prices, insurance fund | `GET /api/v1/oraclePrices`, `/fapi/v1/insuranceBalance` |
| Borrow/lend pools, risk engine config | `GET /fapi/v1/borrowLendPools`, `/api/v1/riskEngineConfig` |
| Listing feeds (contracts, order book) | `GET /api/v1/cmc/contracts`, `/api/v1/cmc/orderbook/{ticker_id}`, `/api/v1/coingecko/contracts`, `/api/v1/coingecko/orderbook` |
| Account, balances, positions | `GET /fapi/v3/account`, `/fapi/v3/balance`, `/fapi/v2/positionRisk` |
| Orders | `GET /fapi/v1/openOrders`, `/fapi/v1/openOrder`, `/fapi/v1/allOrders` (cursor) |
| Fills, funding paid, fees | `GET /fapi/v1/userTrades`, `/fapi/v1/userFundingPayments` (cursor), `/fapi/v1/commissionRate` |
| Delegate lookup | `GET /api/v1/delegateOf` |
| Submit | `POST /tx/submit`; WS `order.place`, `order.cancel`, `order.amend`, `order.cancelAll`, `submit` |
| Public WS | `SYMBOL@depth{,5,10,20,100}`, `@aggTrade`, `@bookTicker`, `@markPrice`, `@liquidations`, `@ticker`, `@kline_*`; `bookTickers`, `markPrices`, `liquidations`, `marketStatus` |
| Private WS | `ADDRESS@user.orders` (orders and fills). No auth: topics are keyed by the main account address. |

Account reads are public and keyed by the main account address (for a delegate key, the
delegator's). There is no REST candle endpoint (none in the OpenAPI spec, 2026-11-16). `@kline_*`,
`@ticker` and `tickers` are still "coming soon" in the docs; on 2026-11-16 the server acknowledged
a subscription to `BTC-USD@ticker`, `SOL-USD@ticker`, `@kline_1m` and `tickers` and published
nothing in 150 s, so the builders stay and their payloads are not modelled.

`BulletClient` rate-limits REST to 50 requests/s, the documented sustained per-IP limit (burst 150,
websocket upgrades included). Transactions are metered separately per signing key by the sequencer
(240 tx/s; HTTP 503 or WS -1016 past it).

Live checks (2026-11-15; oracle, insurance, borrow/lend, risk engine and listing feeds
2026-11-16): every endpoint above with `curl`, captures in `tests/fixtures`. Account
endpoints were read for a public vault address. Websocket: `subscribe` / `unsubscribe` /
`list_subscriptions` / `ping`, depth, bookTicker, markPrice, aggTrade, marketStatus and
`user.orders` (NEW, TRADE, CANCELED) messages captured; an invalid topic returns -1004; every tx
method with garbage bytes returns -1008. `POST /tx/submit` with a vector transaction returned
`400 user_variants not found` for the throwaway key (signature accepted), and with one signature
byte flipped `401 invalid ed25519 signature`. No real order was placed.

## Signing

- Wire: borsh `Transaction::V0` = `0u8 ‖ signature[64] ‖ pub_key[32] ‖ UnsignedTransaction`.
- `UnsignedTransaction` = `RuntimeCall::Exchange` (7) `CallMessage::User` (0) `UserAction` (place
  20, amend 21, cancel 22, cancel market 23, cancel all 29, cancel-and-place 30, max leverage 10),
  then `UniquenessData` (`Nonce` 0 / `Generation` 1 / `Window` 2, `u64`), `TxDetails`
  (`max_priority_fee_bips: u64`, `max_fee: u128`, `gas_limit: Option<[u64; 2]>`,
  `chain_hash_fragment: u64`) and `address_override: Option<[u8; 32]>`.
- Signed bytes: borsh `SigningPayload::V0` = `0u8 ‖ UnsignedTransaction ‖ chain_hash[32]`, ed25519.
  This is the accounts-hard-fork format (changelog 2026-09-28): `chain_hash_fragment` (first 8
  bytes of the chain hash, little-endian) replaces `chain_id`, and `address_override` was added.
- Prices and sizes are `PositiveDecimal`: rust_decimal `{flags, hi, lo, mid}` as `u32`s, with the
  scale as written (`50000.0` ≠ `50000`). Negative values are rejected.
- The chain hash is `chainHash` from `GET /fapi/v1/exchangeInfo`; it can rotate. A transaction
  signed over a stale hash fails with 401.
- The docs' `TpslLeg { trigger_price, order_price: Option, trigger_condition }` is stale: the live
  `/rollup/schema` and the SDK use `Tpsl { order_price, trigger_price, price_condition, order_type }`,
  which this crate follows.

### Parity

`tests/vectors.json` holds 14 transactions built and signed by the official Rust SDK
(`Transaction::builder` / `UnsignedTransaction::builder`) with a throwaway random key and no
network: `tests/gen_vectors` points the SDK client at an in-process mock serving the SDK's own
schema and the live mainnet and testnet chain hashes. Cases cover every covered action, all three
uniqueness kinds, gas limits, extreme fee values, sub-accounts, `address_override`, TP/SL legs,
every order type and decimal scales up to 27 digits. ed25519 is deterministic, so
`tests/vectors.rs` asserts signing bytes, signature and base64 wire bytes match exactly. Regenerate
with `cd tests/gen_vectors && cargo run --release > ../vectors.json`; never regenerate to make a
change pass.

### Usage

```rust
use roshar_bullet::signer::*;
use roshar_bullet::ws_tx::BulletTxMethod;
use roshar_bullet::{BulletApi, BulletClient, BulletNetwork};
use rust_decimal::Decimal;

let client = BulletClient::new(BulletNetwork::Mainnet);
let info = client.get_exchange_info().await?;
let chain_hash = parse_chain_hash(info.chain_hash.as_deref().ok_or(anyhow!("no chainHash"))?)?;
let btc = info.symbols.iter().find(|s| s.symbol == "BTC-USD").unwrap().market_id;

let signer = Signer::from_hex(&delegate_secret_hex)?;          // registered delegate key
let tx = UnsignedTransaction {
    action: UserAction::PlaceOrders {
        market_id: btc,
        orders: vec![NewOrderArgs {
            price: Decimal::from_str_exact("64900")?, size: Decimal::from_str_exact("0.001")?,
            side: Side::Bid, order_type: OrderType::PostOnly, reduce_only: false,
            client_order_id: Some(my_cid), pending_tpsl_pair: None,
        }],
        replace: false,
        sub_account_index: None,
    },
    uniqueness: Uniqueness::Window(now_ms),                    // you own uniqueness
    details: TxDetails { max_priority_fee_bips: 0, max_fee: 1 << 48, gas_limit: None },
    address_override: None,
};
let signed = signer.sign(&tx, &chain_hash)?;
let reply = client.submit_tx(&signed).await?;                  // or over the websocket:
client.send_tx(&manager, Some(10), BulletTxMethod::for_action(&tx.action), &signed)?;

let orders = client.get_open_orders(&main_account_address, Some("BTC-USD")).await?;
client.subscribe_user_orders(&manager, Some(1), &main_account_address)?;
```

`Window` uniqueness must be unique per key within the rollup's window (the docs suggest
millisecond time); `Nonce` must be exactly the key's current nonce + 1. A `dropped` status means
the order was not placed.

## Not covered yet

Trading: TP/SL on a position (`CreatePositionTpsl`), spot trigger orders (`CreateTriggerOrders`,
`CancelTriggerOrders`), TWAP (`CreateTwapOrder`, `CancelTwapOrder`, `GET /fapi/v1/twapHistory`,
`/twapFills`), `SetPerpLedgerTradingMode`, `SetAccountGroup`, Degen trades (web-app only), Solana
off-chain / Ledger envelopes (`POST /api/v1/solanaOffchainTx`), multisig, `POST /rollup/simulate`.
Add each with vectors from `gen_vectors`.

Funds and accounts: transfers, deposits, withdrawals, delegate registration, sub-accounts, vaults,
borrow/lend, bridge (`/api/v1/bridge/*`) and their history endpoints (`userTransfers`,
`userDeposits`, `userWithdrawals`, `userLiquidations`).

Data: `GET /fapi/v1/orderHistory` (web-app feed; `allOrders` covers order history), `/api/v1/account`
and `/api/v1/balance` (non-Binance account views), `/rollup/constants` (chain id/name, gas token,
Hyperlane domain, address prefix: not market data, and the same values are in
`exchangeInfo.chainInfo`), Degen endpoints and topics (`@degenResults`: documented but nothing
published in 150 s on BTC/SOL/ETH, so no live capture to type against; `@user.degenPositions`),
and payloads of the "coming soon" `@ticker` / `tickers` / `@kline_*` topics. Liquidation history
is only per account (`/fapi/v1/userLiquidations`); public liquidations are the websocket topics.
