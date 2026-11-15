# roshar-hyperliquid

Hyperliquid client: L1 action signing, order posting over websocket or REST, user event streams,
nonces, `/info` queries, tick rounding and the market-data websocket config.

The order path is the one the live PUMP maker runs (moved from saoirse `hl_exchange`). The golden
vectors in `tests/golden.json` were recorded from that code before the move. Never regenerate them to
make a change pass: a changed vector means a changed signature.

## Principles

- **Mirror the exchange.** Return what Hyperliquid sends, parsed strictly. A missing or unparseable
  field is an error, never a default. Statuses are the exchange's own (`OrderStatus::{Resting, Filled,
  Success, Rejected}`) with `u64` oids.
- **No caller policy.** The library does not choose coins, sizes, retries, rounding direction,
  caching or refresh intervals. Credentials are passed in; nothing is read from the environment.
- **Wire types live in `roshar_types::hyperliquid::exchange`** and are re-exported here: `Action`,
  `OrderReq`, `OrderRef`, `Tif`, `Cloid`, `cloid_hex`, `OrderStatus`, `L2Snapshot`, `Level`, `Side`.

## Modules

| Module | What it does |
|---|---|
| `signer` | `Signer::from_hex(key, vault: Option<&str>, mainnet)`. `sign(&Action, nonce)` gives the `/exchange` payload. `float_to_wire` gives exact decimal strings. Pass `vault = Some(subaccount)` to trade a sub-account or vault, `None` for the key's own account or its master. |
| `nonce` | `NonceGen`: strictly increasing millisecond nonces. |
| `poster` | `WsPoster::connect(url)`. `post(payload)` returns a `PendingPost` whose `rx` oneshot always resolves to a `PostResult { sent_ns, recv_ns, status }`. A transport problem (timeout or disconnect) is `status: Err`. `spawn_fan_writer` races one action over several connections. `decode_status` decodes the first status of a response. `now_ns` is the clock used by all `*_ns` fields. |
| `rest` | `RestPoster::new(HL_EXCHANGE_URL)`. Same `PostResult` over HTTP. |
| `user_stream` | `UserStream::spawn(ws_url, user, tx)`. Emits `UserEvent::{Order(OrderUpdate), Fill(UserFill), ...}` from `orderUpdates` and `userFills`. |
| `info` | `Info::new()` (mainnet), `Info::testnet()`, `.with_timeout(..)`, `.with_rate_limit(n, interval)`. See below. |
| `tick` | `to_tick(px, sz_decimals, up)`: valid perp price, rounded in the direction you choose. |
| `ws` | `HL_WSS_URL`, `HL_TESTNET_WSS_URL`. `ws_config(mainnet)` builds the roshar-ws-mgr config (feature `ws-mgr`). Subscription messages are `roshar_types::HyperliquidWssMessage`. |

### `Info` endpoints

**Wire types:**
- `meta_and_asset_ctxs`
- `spot_meta_and_asset_ctxs`
- `candle_snapshot(coin, interval, start, end)`
- `funding_history(coin, start, end)`, which follows pages of 500 entries
- `clearinghouse_state(user)`
- `spot_clearinghouse_state(user)`

**Parsed to numbers:**
- `l2_snapshot(coin)` returns an `L2Snapshot`
- `asset_meta(coin)` returns `(asset index, szDecimals)`
- `candle_closes(coin, interval, start, end)`
- `position_szi(user, coin)`; 0 means no position
- `open_orders(user)`, from `frontendOpenOrders`
- `fills_since(user, start)`
- `order_status(user, cloid)`
- `master_of(subaccount)`
- `user_rate_limit(user)`

Spot assets use index `10000 + spot index`. Read the spot universe from `spot_meta_and_asset_ctxs`.

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

- Leverage updates (`updateLeverage`), `scheduleCancel` (dead-man's switch), transfers and vault
  actions. Add them as new `Action` variants, with signing vectors checked against the Python SDK.
- The integer-price exemption from the 5-significant-figure rule (see the TODO in `tick.rs`).
