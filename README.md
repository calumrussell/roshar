# roshar

Exchange clients for crypto venues: one crate per exchange, plus websocket and HTTP transport.

| Crate | What |
|---|---|
| `roshar-<exchange>` | Client for one exchange: wire types (`roshar_<exchange>::types`), REST, websocket config, and signing where supported. Exchanges: aster, binance, bitget, bybit, coinbase, deribit, derive, hyperliquid, kraken, kucoin, lighter, okx, pacifica, paradex, phoenix, polymarket. |
| `roshar-ws` | Low-level websocket client. |
| `roshar-ws-mgr` | Websocket connection manager: reconnects, pings, fans out messages. |
| `roshar-http` | Pooled HTTP client with leaky-bucket rate limiting. |

## Principles

- **Exchange-specific.** A crate models one exchange's API as that exchange defines it. There are no
  cross-exchange abstractions or normalised order types; callers map between exchanges themselves.
- **No caller leakage.** No hard-coded coins, sizes, intervals, depths, filters, caching or refresh
  policies chosen for one caller. Credentials are passed in, never read from the environment.
- **Strict parsing.** A missing or unparseable field is an error, not a silent default.
- **Signing is tested against vectors** from the exchange's reference SDK (see
  `roshar-hyperliquid/tests`).

`roshar-hyperliquid/README.md` documents the Hyperliquid client, which is the most complete one.

## Build

```
cargo build --workspace
cargo test --workspace     # some tests hit live public endpoints
```

`roshar-ws`'s `io_uring` feature builds only on Linux.
