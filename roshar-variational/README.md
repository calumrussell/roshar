# roshar-variational

Variational Omni (RFQ perpetuals and swaps, settled on Arbitrum) client: the public read-only
statistics API. Variational has no trading API, order book, trades, candles, funding history or
websocket to cover. The docs say: "The trading API is still in development, and is not yet available
to any users". The roadmap lists "Enable API trading" as not done.

## Principles

Same as the other roshar crates (see `roshar-hyperliquid/README.md`):

- **Mirror the exchange.** Replies are parsed strictly. Decimals stay as the strings the exchange
  sends. Fields are `Option` only where the docs say they are omitted or nullable. Nullable fields
  that are always sent (`funding`, swap `last`/`next`/`next_timestamp`, perpetual `next`) must
  still be present.
- **No caller policy.** No retries, caching or refresh.

## Modules

| Module | What it does |
|---|---|
| crate root | `VariationalClient` (rate-limited REST) implementing `VariationalApi`: `get_stats`, `get_stats_v1`. `VARIATIONAL_API_URL`, `VariationalMarketApi`. |
| `types` | `VariationalStats` (v2) and `VariationalStatsV1` (legacy), listings, `VariationalContract` (`contract_type` tag → perpetual or swap `funding`), quotes, `VariationalErrorResponse`, `VariationalApiError`. |

## API docs

| Area | Reference |
|---|---|
| REST | https://docs.variational.io/technical-documentation/api (markdown: `.../api.md`) |
| WebSocket | None documented |
| Auth / signing | None. The trading API is "not yet available to any users" (same page, "Trading API"). |
| Rate limits | https://docs.variational.io/technical-documentation/api#rate-limits |
| Roadmap | https://docs.variational.io/getting-started/roadmap ("Enable read-only Omni API" done, "Enable API trading" open) |
| Trade/transfer history | https://docs.variational.io/technical-documentation/trade-and-transfer-history (CSV download in the web UI only, no API) |
| Changelog | None published |
| SDK used for vectors | None. Nothing is signed. |

Checked against these on 2026-11-15.

UK access: the UK is not a restricted territory. https://docs.variational.io/legal/restricted-persons
lists sanctioned regions, the US and its territories, Canada and Taiwan. The Terms of Use
(https://docs.variational.io/legal/terms-of-use §4) exclude persons subject to HM Treasury
sanctions. From a UK IP, both endpoints returned HTTP 200 with no geo-block.

## Coverage

| Item | Endpoint |
|---|---|
| Platform metrics (24h volume, cumulative volume, TVL, OI, market count) and per-listing mark price, 24h volume, long/short OI, funding (perpetual: last/next rate, interval, next time; swap: long/short rates), spread, indicative bid/ask at base/$1k/$100k/$1m | `GET /metadata/v2/stats` |
| Legacy v1 of the same (unsuffixed tickers, `funding_rate` + `funding_interval_s`, `"0"` for swaps) | `GET /metadata/stats` |

There is one response containing all listings. It has no per-market query parameters. `num_markets`
can be larger than `listings.len()` (both were 589 on 2026-11-15). Quotes may be cached for up to
600 s. While swap markets are closed, `quotes`, `base_spread_bps` and swap `funding.next` are absent
or `null`. The live fixtures were captured on a Sunday and show this.

`VariationalClient` rate-limits to 1 request/s by default. The documented limit is 10 requests per
10 s per IP, shared by v1 and v2, plus 1000 requests per minute globally. Over the limit, the API
returns HTTP 429 with
`{"error":"rate_limited","message":"Service is busy; try again in 1 seconds","retry_after_seconds":1}`.
This becomes `VariationalApiError` (downcast the boxed error to read `retry_after_seconds`). We
got these 429s on 2026-11-15 even at about 1 request per 2.6 s. The "Service is busy" wording
suggests the global limit, so callers should expect occasional 429s at any rate. A 404 has an empty
body.

```rust
use roshar_variational::{VariationalApi, VariationalClient};
use roshar_variational::types::VariationalContract;

let client = VariationalClient::new();
let stats = client.get_stats().await?;
for l in &stats.listings {
    match &l.contract {
        VariationalContract::Perpetual { funding: Some(f) } => println!("{} {} {}", l.ticker, l.mark_price, f.last.annualized_rate),
        VariationalContract::Swap { funding: Some(f) } => println!("{} next {:?}", l.ticker, f.next),
        _ => println!("{} no funding data", l.ticker),
    }
}
```

## Signing

None. There is no authenticated API.

## Tests

`src/types/mod.rs` parses the fixtures in `tests/fixtures/`:

- `stats_v2.json` and `stats_v1.json`: live captures from 2026-11-15, trimmed to `BTC-PERP`,
  `TNSR-PERP` (no `size_1m`) and two closed swaps.
- `stats_v2_docs_sample.json`: the docs sample, which shows an open swap with `next` published.
- `error_rate_limited.json`: a live 429 body.

The tests also check that a missing required or always-sent nullable key is an error, and that a
`contract_type` must match its `funding` shape.

## Not covered yet

Trading, account data and private streams, because Variational has not released a trading API. The
web app's internal endpoints are undocumented and not covered. Trade and transfer history is
available only as CSV downloads from the web UI.
