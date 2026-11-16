# roshar

Exchange clients for crypto venues: one crate per exchange, plus websocket and HTTP transport.

| Crate | What |
|---|---|
| `roshar-<exchange>` | Client for one exchange: wire types (`roshar_<exchange>::types`), REST, websocket config, and signing where supported. Each crate's `README.md` lists the official API docs it was checked against, what it covers and what it does not. |
| `roshar-ws` | Low-level websocket client. |
| `roshar-ws-mgr` | Websocket connection manager: reconnects, pings, fans out messages. `Config.headers` adds HTTP headers to the upgrade request. |
| `roshar-http` | Pooled HTTP client with leaky-bucket rate limiting. |

## Venues

Trading means signed order placement, checked against vectors from the venue's official SDK. UK
access is what the venue's own terms say about UK residents, read on 2026-11-15; each crate's README
quotes the clause. It is not legal advice, and terms change.

| Crate | Venue | Kind | Trading | UK access |
|---|---|---|---|---|
| `roshar-apex` | ApeX Omni | perp DEX | yes | not restricted |
| `roshar-bullet` | Bullet (ex-Zeta) | perp DEX | yes | not restricted |
| `roshar-edgex` | edgeX | perp DEX | yes | not restricted |
| `roshar-gmx` | GMX v2 | perp DEX (pool) | yes (express relay) | not restricted |
| `roshar-grvt` | GRVT | perp DEX (hybrid) | yes | not restricted; KYC required |
| `roshar-hyperliquid` | Hyperliquid | perp/spot DEX | yes | not restricted |
| `roshar-jupiter-perps` | Jupiter Perps | perp DEX (pool) | yes | not restricted |
| `roshar-nado` | Nado | perp/spot DEX | yes | not restricted |
| `roshar-orderly` | Orderly Network | perp DEX | yes | not restricted (each broker front-end has its own terms) |
| `roshar-paradex` | Paradex | perp/option DEX | yes | not restricted |
| `roshar-polymarket-perps` | Polymarket Perps | perp DEX (hybrid) | yes | not restricted |
| `roshar-synthetix` | Synthetix | perp DEX | yes | not restricted |
| `roshar-variational` | Variational Omni | perp RFQ | no (venue has no trading API) | not restricted |
| `roshar-aster` | Aster | perp DEX | no | restricted |
| `roshar-derive` | Derive | option/perp DEX | no | professional clients and high-net-worth companies only |
| `roshar-lighter` | Lighter | perp DEX | yes | restricted; `sendTx` and the main websocket refuse UK IPs |
| `roshar-pacifica` | Pacifica | perp DEX | yes | restricted |
| `roshar-phoenix` | Phoenix | perp DEX | no | restricted |
| `roshar-polymarket` | Polymarket (Gamma) | prediction markets | no | close-only |
| `roshar-binance` | Binance | CEX | no | see crate README |
| `roshar-bitget` | Bitget | CEX | no | see crate README |
| `roshar-bybit` | Bybit | CEX | no | see crate README |
| `roshar-coinbase` | Coinbase Advanced Trade | CEX | no | see crate README |
| `roshar-deribit` | Deribit | CEX | no | no UK retail clients |
| `roshar-kraken` | Kraken Futures | CEX | yes | derivatives for professional clients only |
| `roshar-kucoin` | KuCoin | CEX | no | see crate README |
| `roshar-okx` | OKX | CEX | no | see crate README |

## Principles

- **Exchange-specific.** A crate models one exchange's API as that exchange defines it. There are no
  cross-exchange abstractions or normalised order types; callers map between exchanges themselves.
- **No caller leakage.** No hard-coded coins, sizes, intervals, depths, filters, caching or refresh
  policies chosen for one caller. Credentials are passed in, never read from the environment.
- **Strict parsing.** A missing or unparseable field is an error, not a silent default.
- **Signing is tested against vectors** from the exchange's reference SDK. Each signing crate keeps
  its generator (`tests/gen_vectors.*`) next to `tests/vectors.json`. Never regenerate vectors to
  make a change pass.

## Build

```
cargo build --workspace
cargo test --workspace     # some tests hit live public endpoints
```

`roshar-ws`'s `io_uring` feature builds only on Linux.
