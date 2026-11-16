# roshar-hyperliquid

Hyperliquid client: L1 action signing, order posting over websocket or REST, user event streams,
nonces, `/info` queries, tick rounding and the market-data websocket config.

The order path is the one the live PUMP maker runs (moved from saoirse `hl_exchange`). The golden
vectors in `tests/golden.json` were recorded from that code before the move. Never regenerate them to
make a change pass: a changed vector means a changed signature.

## API docs

| Area | Reference |
|---|---|
| REST (overview) | https://hyperliquid.gitbook.io/hyperliquid-docs/for-developers/api |
| Info endpoint | https://hyperliquid.gitbook.io/hyperliquid-docs/for-developers/api/info-endpoint, `/perpetuals`, `/spot` |
| Exchange endpoint | https://hyperliquid.gitbook.io/hyperliquid-docs/for-developers/api/exchange-endpoint |
| Asset ids (perp, spot, HIP-3, outcomes) | https://hyperliquid.gitbook.io/hyperliquid-docs/for-developers/api/asset-ids |
| Tick and lot size | https://hyperliquid.gitbook.io/hyperliquid-docs/for-developers/api/tick-and-lot-size |
| WebSocket | https://hyperliquid.gitbook.io/hyperliquid-docs/for-developers/api/websocket (`/subscriptions`, `/post-requests`, `/timeouts-and-heartbeats`) |
| Auth / signing | https://hyperliquid.gitbook.io/hyperliquid-docs/for-developers/api/signing; reference implementation https://github.com/hyperliquid-dex/hyperliquid-python-sdk (`hyperliquid/utils/signing.py`) |
| Priority fees (`{p}` grouping) | https://hyperliquid.gitbook.io/hyperliquid-docs/for-developers/api/priority-fees |
| Rate limits | https://hyperliquid.gitbook.io/hyperliquid-docs/for-developers/api/rate-limits-and-user-limits |
| Changelog | none published; track https://github.com/hyperliquid-dex/hyperliquid-python-sdk/commits/master |

Checked against these on 2026-11-15.

## Principles

- **Mirror the exchange.** Return what Hyperliquid sends, parsed strictly. A missing or unparseable
  field is an error, never a default. Statuses are the exchange's own (`OrderStatus::{Resting, Filled,
  Success, Rejected}`) with `u64` oids.
- **No caller policy.** The library does not choose coins, sizes, retries, rounding direction,
  caching or refresh intervals. Credentials are passed in; nothing is read from the environment.
- **Wire types live in `roshar_hyperliquid::types::exchange`** and are re-exported here: `Action`,
  `OrderReq`, `OrderRef`, `Tif`, `Cloid`, `cloid_hex`, `OrderStatus`, `L2Snapshot`, `Level`, `Side`, and for
  trigger orders `TypedOrderReq`, `OrderType`, `Tpsl`, `Grouping`.

## Modules

| Module | What it does |
|---|---|
| `signer` | `Signer::from_hex(key, vault: Option<&str>, mainnet)`. `sign(&Action, nonce)` gives the `/exchange` payload. `float_to_wire` gives exact decimal strings. Pass `vault = Some(subaccount)` to trade a sub-account or vault, `None` for the key's own account or its master. |
| `Action` variants | `Order` (limit orders, grouping `na` or priority `{p}`), `BatchModify`, `Modify`, `Cancel`, `CancelByCloid`. `OrderTyped { orders: Vec<TypedOrderReq>, grouping }` takes limit or trigger orders (`OrderType::Trigger { is_market, trigger_px, tpsl }`) with grouping `na`, `normalTpsl` or `positionTpsl`. `UpdateLeverage { asset, is_cross, leverage }`. Every variant is checked against Python SDK vectors (`tests/vectors.json`, `tests/trigger_vectors.json`). |
| `nonce` | `NonceGen`: strictly increasing millisecond nonces. |
| `poster` | `WsPoster::connect(url)`. `post(payload)` returns a `PendingPost` whose `rx` oneshot always resolves to a `PostResult { sent_ns, recv_ns, status }`. A transport problem (timeout or disconnect) is `status: Err`. `spawn_fan_writer` races one action over several connections. `decode_status` decodes the first status of a response. `now_ns` is the clock used by all `*_ns` fields. |
| `rest` | `RestPoster::new(HL_EXCHANGE_URL)`. Same `PostResult` over HTTP. |
| `user_stream` | `UserStream::spawn(ws_url, user, tx)`. Emits `UserEvent::{Connected, Reconnected, Disconnected, Orders { updates, .. }, Fills { snapshot, fills, .. }}` from `orderUpdates` and `userFills`. |
| `info` | `Info::new()` (mainnet), `Info::testnet()`, `.with_timeout(..)`, `.with_rate_limit(n, interval)`. See below. |
| `tick` | `to_tick(px, sz_decimals, up)`: valid perp price, rounded in the direction you choose. |
| `ws` | `HL_WSS_URL`, `HL_TESTNET_WSS_URL`. `ws_config(mainnet)` builds the roshar-ws-mgr config (feature `ws-mgr`). Subscription messages are `roshar_hyperliquid::types::HyperliquidWssMessage`: `all_mids`, `all_mids_dex(dex)`, `l2_book`, `l2_book_fast`, `l2_book_with(coin, n_sig_figs, mantissa, fast)`, `candle(coin, interval)`, `trades`, `bbo`, `active_asset_ctx(coin)`, `all_dexs_asset_ctxs`, `outcome_meta_updates`, `fast_asset_ctxs`, `user_fills`, `order_updates`, each with an `_unsub`. Incoming frames parse with `HyperliquidWsEvent::parse(text)` (tagged by `channel`; unknown channels are an error); decode `FastAssetCtxs` payloads with `decode_fast_asset_ctxs`. |

### `Info` endpoints

`dex` is the perp dex name; `""` is the first perp dex. HIP-3 coins are named `{dex}:{coin}`
(e.g. `xyz:XYZ100`); `perp_dex_of(coin)` returns the dex part.

**Wire types:**
- `meta_and_asset_ctxs(dex)`, `meta(dex)`, `all_perp_metas()`, `perp_dexs()`
- `perp_dex_limits(dex)`, `perp_dex_status(dex)`, `perps_at_open_interest_cap(dex)`
- `perp_annotation(coin)` (`None` if unannotated), `perp_categories()`, `perp_concise_annotations()`
- `all_mids(dex)`, `l2_book(coin, n_sig_figs, mantissa)`, `recent_trades(coin)`
- `predicted_fundings()`
- `spot_meta_and_asset_ctxs`, `spot_meta()`, `token_details(token_id)`
- `outcome_meta()`, `outcome_templates()`, `settled_outcome(outcome)` (`None` if unknown)
- `candle_snapshot(coin, interval, start, end)`
- `funding_history(coin, start, end)`, which follows pages of 500 entries
- `clearinghouse_state(user, dex)`
- `spot_clearinghouse_state(user)`

**Parsed to numbers:**
- `l2_snapshot(coin)` returns an `L2Snapshot`
- `asset_meta(coin)` returns `(asset id, szDecimals)`; HIP-3 coins get `100000 + perp_dex_index * 10000 + index`
- `candle_closes(coin, interval, start, end)`
- `position_szi(user, coin)`; 0 means no position. HIP-3 coins query their own dex.
- `open_orders(user, dex)`, from `frontendOpenOrders`
- `fills_since(user, start)`, one `userFillsByTime` request (at most 2000 fills)
- `order_status(user, cloid)`
- `master_of(subaccount)`
- `user_rate_limit(user)`

Spot assets use index `10000 + spot index`. Read the spot universe from `spot_meta_and_asset_ctxs`.

Calls that `/info` answers with `null` for an unknown coin/dex or invalid parameters (`l2_book`,
`meta`, `perp_dex_limits`, `perp_dex_status`, `token_details`) return an error.

### Market-data coverage

Checked against the docs and live on 2026-11-16 (captures in `tests/fixtures/`).

| Data | REST (`/info` type) | WebSocket |
|---|---|---|
| Perp markets / specs | `meta`, `metaAndAssetCtxs`, `allPerpMetas`, `perpDexs`, `perpDexLimits`, `perpDexStatus`, `perpAnnotation`, `perpCategories`, `perpConciseAnnotations` | — |
| Spot markets / tokens | `spotMeta`, `spotMetaAndAssetCtxs`, `tokenDetails` | — |
| Outcome markets | `outcomeMeta`, `outcomeTemplates`, `settledOutcome` | `outcomeMetaUpdates` |
| Mids | `allMids` (+`dex`) | `allMids` (+`dex`) |
| Asset contexts (mark, oracle, funding, OI, volume) | `metaAndAssetCtxs`, `spotMetaAndAssetCtxs` | `activeAssetCtx` (perp and spot), `allDexsAssetCtxs`, `fastAssetCtxs` |
| Order book | `l2Book` (+`nSigFigs`, `mantissa`) | `l2Book` (+`nSigFigs`, `mantissa`, `fast`), `bbo` |
| Trades | `recentTrades` (undocumented) | `trades` |
| Candles | `candleSnapshot` | `candle` |
| Funding | `fundingHistory`, `predictedFundings` | — |
| Open-interest caps | `perpsAtOpenInterestCap` | — |

## Example: place and cancel one ALO order

```rust
use roshar_hyperliquid::*;

let info = Info::new()?;
let (asset, sz_dec) = info.asset_meta("ETH").await?;
let signer = Signer::from_hex(&api_wallet_key, Some(&subaccount), true)?;
let nonces = NonceGen::new();
let ws = WsPoster::connect(HL_WSS_URL).await?;

let px = to_tick(mid * 0.99, sz_dec, false);
let order = OrderReq { asset, is_buy: true, px, sz: 0.01, reduce_only: false, tif: Tif::Alo, cloid: None };
let action = Action::Order { orders: vec![order], priority: None };
let res = ws.post(signer.sign(&action, nonces.next())?).await?.rx.await?; // always resolves
if let Ok(OrderStatus::Resting { oid }) = res.status {
    let cancel = Action::Cancel { cancels: vec![(asset, oid)], fast: false };
    ws.post(signer.sign(&cancel, nonces.next())?).await?;
}
```

## Not covered yet

- `scheduleCancel` (dead-man's switch), `updateIsolatedMargin`, transfers and vault actions. Add them as
  new `Action` variants, with signing vectors checked against the Python SDK (see
  `tests/gen_trigger_vectors.py` for an offline generator).
- `expiresAfter` (optional on L1 actions; changes the hash), the order `builder` field, and the
  documented `a` (always_place) flag on `batchModify`.
- `decode_status` does not recognise the bare-string statuses a trigger order can return (e.g.
  `"waitingForTrigger"`, `"waitingForFill"`); they come back as `Err(raw response)`. Callers placing
  trigger orders should inspect that response themselves.
- The integer-price exemption from the 5-significant-figure rule (see `tick.rs`); `to_tick` output is
  valid but coarser than needed for prices >= 100000.
- `fills_since` does not page past 2000 fills.
- Private/user info types and WS feeds (`clearinghouseState`, `openOrders`, `userEvents`,
  `webData3`, `activeAssetData`, `spotState`, ...) other than the ones listed above.
- Spot-scoped calls take no `dex`: the docs list none, and `spotMetaAndAssetCtxs` with `dex: "xyz"`
  returns the same universe as without (checked 2026-11-16).
- Not provided by Hyperliquid: open-interest history, liquidation feed, insurance fund, server
  time. Deploy auctions (`perpDeployAuctionStatus`, `spotDeployState`, `spotPairDeployAuctionStatus`)
  and `outcomeDeployerLimits` are deployer tooling, not market data.
- Invalid `l2Book` aggregation on the websocket (e.g. `nSigFigs: 4, mantissa: 2`) makes the server
  close the connection without an error frame; the builders do not validate.
- `outcomeMetaUpdates` sent nothing during capture; its parsing follows the documented shapes.
- Outcome asset ids (`100000000 + encoding`): only the `+`/`#` coin helpers exist.
- `WsPoster` does not enforce the 100 in-flight post limit per IP.
