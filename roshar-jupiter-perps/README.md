# roshar-jupiter-perps

Jupiter Perpetuals (Solana pool perps, SOL/ETH/BTC against the JLP pool) through the Jupiter Perps
API: market data, wallet reads, transaction building, signing of the returned Solana transactions,
and submission.

Jupiter Perps has no order book, candles, funding rate or websocket. Prices come from oracles;
positions pay an hourly borrow rate (`pool-info`) instead of funding. Every write is a Solana
transaction the API builds and the wallet signs.

## Principles

- **Mirror the exchange.** Types follow the OpenAPI document and live replies. Missing fields are
  errors. Non-2xx replies become `JupiterPerpsError { status, code, message, metadata }`.
- **No caller policy.** No hard-coded markets, sizes, slippage, priority fees or retries. Keys are
  passed in. Nothing reads the environment or the clock.

UK access: Jupiter's Terms of Use exclude the United States, Singapore and other listed countries
and "any other state, country or region that is subject to sanctions enforced by the United States,
the United Kingdom or the European Union". The UK is not on the list
(https://developers.jup.ag/docs/legal/terms-of-use). From a UK IP every endpoint answered normally
(CloudFront LHR). There was no geo-block.

## API docs

| Area | Reference |
|---|---|
| REST | https://perps-api.jup.ag/v2/docs (Redoc; the OpenAPI JSON is embedded in the page) |
| WebSocket | None. https://developers.jup.ag/docs/perps describes the Perps API as "still a work in progress" and lists no stream. |
| Auth / signing | No API keys. Writes return an unsigned transaction. The official CLI signs it with `@solana/kit`: https://github.com/jup-ag/cli/blob/main/src/lib/Signer.ts, flow in https://github.com/jup-ag/cli/blob/main/src/clients/PerpsClient.ts and https://github.com/jup-ag/cli/blob/main/docs/perps.md |
| On-chain accounts | https://developers.jup.ag/docs/perps/position-account, `position-request-account`, `pool-account`, `custody-account` |
| Rate limits | perps-api.jup.ag publishes none. The Jupiter API's keyless tier is 0.5 req/s (https://developers.jup.ag/docs/portal/rate-limits). |
| Changelog | https://developers.jup.ag/changelog (no Perps API entries) |
| SDK for vectors | `@solana/kit` 6.10.0, the library `@jup-ag/cli` 0.10.1 (commit `d8d73866`, 2026-06-02) signs with |

Checked against these on 2026-11-15. The OpenAPI `servers` entry says `/v1`. The docs are served
under `/v2`, and the CLI calls `/v2`. Both answer identically. The crate uses
`https://perps-api.jup.ag/v2`.

## Modules

| Module | What it does |
|---|---|
| crate root | `JupiterPerpsClient` (`market()`, `trading()`, `JupiterPerpsApi` trait for market data), `JUPITER_PERPS_API_URL` |
| `MarketApi` | `market_stats`, `pool_info`, `jlp_info`, `top_traders`, `trader_stats` |
| `TradingApi` | Wallet reads: `positions`, `limit_orders`, `expired_orders`, `trades`, `collateral_limits`. Transaction builders: `increase_position`, `decrease_position`, `close_all`, `create/update/cancel_limit_order`, `create/update/cancel_tpsl`. Submission: `execute` |
| `signer` | `Keypair` (seed, 64-byte keypair, base58), `sign_transaction[_base64]` → `SignedTransaction` |
| `types` | Request/response bodies, enums (`Asset`, `Token`, `Side`, `ExecuteAction`, …), market mints, `ErrorBody`, `JupiterPerpsError` |

## Coverage

| Item | Endpoint |
|---|---|
| Market price, 24 h stats | `GET /market-stats?mint=` |
| Liquidity, utilisation, borrow rates, fees | `GET /pool-info?mint=` |
| JLP pool and custodies | `GET /jlp-info` |
| Leaderboard, per-wallet weekly stats | `GET /top-traders`, `GET /trader-stats` |
| Positions (with TP/SL), closed positions | `GET /positions` |
| Open limit orders | `GET /orders/limit?walletAddress=` (the OpenAPI document says body, but the API and CLI use the query string) |
| Expired limit orders (close txs) | `GET /orders/expired` |
| Trade history (fills, triggers, liquidations) | `GET /trades` |
| Collateral deposit/withdraw bounds | `GET /positions/collateral-limits` |
| Market open/increase, deposit collateral (+ TP/SL) | `POST /positions/increase` |
| Market reduce/close, withdraw collateral | `POST /positions/decrease`, `POST /positions/close-all` |
| Limit order create / update / cancel | `POST`, `PATCH`, `DELETE /orders/limit` |
| TP/SL create / update / cancel | `POST`, `PATCH`, `DELETE /tpsl` |
| Submit signed tx | `POST /transaction/execute` |

Order types are what the program offers: market (increase/decrease with `maxSlippageBps`), limit
(trigger price) and TP/SL triggers. There is no post-only, IOC, reduce-only flag or client order
id. A decrease is reduce-only by construction. Orders are identified by their position-request
account (`positionRequestPubkey`).

Rate limit: `JupiterPerpsClient::new()` allows 1 request every 2 s (the documented keyless Jupiter
limit, 30 per 60 s). Market and trading calls share it. In testing, 40 sequential requests
in 10 s all succeeded. Use `new_with_rate_limit(refill, interval_secs)` to change it.

Live checks (2026-11-15): every GET above was captured into `tests/fixtures/`. The transaction
builders were called with a throwaway wallet, or with public addresses of active traders (building a
transaction moves no funds). `POST /transaction/execute` was sent an unsigned transaction and
returned 422 `simulation_error`. Error bodies: 400 `invalid_argument`, 422 `insufficient_funds` /
`invalid_sl_price` (with `metadata`), and 500 `internal_server_error` (also returned for unknown
positions and invalid wallet addresses).

## Signing

- The API returns a complete Solana wire transaction. So far it has always been a v0 message, with
  the wallet as fee payer (signer 0). Market, limit and instant-TP/SL transactions have 3 signer
  slots: the wallet, the keeper `Ag28fGtw…` (filled at `/transaction/execute` when
  `requireKeeperSignature` is true), and `perpSnt3…`, which the API has already signed.
- `Keypair::sign_transaction` reads the signature count, the message header and the account keys.
  It finds the wallet among the first `num_required_signatures` keys, then writes
  `ed25519(message bytes)` into that slot. Other slots are left untouched. A wallet that is not a
  required signer is an error. This is `partiallySignTransactionWithSigners` in `@solana/kit`.
  Legacy and v0 messages are accepted. Other versions are rejected.
- Dependencies: `ed25519-dalek`, `bs58` and `base64`. No `solana-sdk`, because the transaction is
  never built or changed locally.

### Parity

`tests/gen_vectors.mjs` uses `@solana/kit` 6.10.0 with a throwaway key (seed in the file, never
funded) and no network. It signs three recorded `POST /orders/limit` transactions
(`tests/fixtures/create_limit_order*.json`, requested for that key), a kit-built legacy
transaction, and a kit-built v0 transaction in which the key is signer 1. It also records that kit
refuses two recorded transactions belonging to other wallets. `tests/vectors.rs` checks
that the address, signed wire bytes (base64) and signature (base58) are byte-identical, that
signing is idempotent, and that non-signers and malformed bytes are rejected. Ed25519 is
deterministic, so no nonce recovery is needed. Never regenerate the vectors to make a change pass.

## Usage

```rust
use roshar_jupiter_perps::{JupiterPerpsClient, signer::Keypair, types::*};

let client = JupiterPerpsClient::new();
let key = Keypair::from_base58(&keypair_b58)?;

let built = client.trading().create_limit_order(&CreateLimitOrderRequest {
    asset: Asset::Eth, input_token: Token::Usdc, input_token_amount: "10000000".into(),
    leverage: Some("3".into()), side: Side::Long, size_usd_delta: None,
    trigger_price: "1500000000".into(), include_serialized_tx: Some(true),
    wallet_address: Some(key.address()),
}).await?;
let signed = key.sign_transaction_base64(built.serialized_tx_base64.as_deref().ok_or("no tx")?)?;
let reply = client.trading().execute(&ExecuteRequest {
    action: ExecuteAction::CreateLimitOrder,
    serialized_tx_base64: signed.to_base64(),
}).await?;
assert_eq!(reply.txid, signed.signature_base58());
```

Close-all and expired-order replies hold one transaction per position. Sign each one. The
documented `action` list has no close-all entry, so submit those with `DecreasePosition` (the CLI
does the same per transaction) or send them through your own RPC.

## Not covered yet

- JLP mint/burn (`POST /jlp/mint`, `/jlp/burn`) and JLP loans (`/loans/*`). These are not perps trading.
- Portfolio PnL (`GET /wallet-pnl/{wallet}`, SSE `/wallet-pnl/stream`). These need a dedicated
  Portfolio bearer credential.
- Submission through a Solana RPC (`sendTransaction`) or `tx.jup.ag`. Keeper-co-signed
  transactions must go through `/transaction/execute` anyway.
- Reading the on-chain Pool/Custody/Position accounts directly (e.g. custody funding-rate state)
  through an RPC and the Anchor IDL.
- Typed parsing of `ExecuteResponse` was not live-verified. No order was placed, and the shape
  comes from the OpenAPI document.
