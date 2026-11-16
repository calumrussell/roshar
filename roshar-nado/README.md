# roshar-nado

Nado client (Ink chain perps and spot order book; successor to Vertex). Covers public market data
(gateway queries, archive indexer, V2 REST, subscriptions websocket), EIP-712 signing of executes,
REST execute submission, and the account reads a trader needs.

## Principles

These are the same as in the other roshar crates (see `roshar-hyperliquid/README.md`):

- **Mirror the exchange.** Payloads, EIP-712 digests and signatures are byte-identical to the
  official Python SDK. Replies are parsed strictly. x18 values are integers in strings and are
  parsed as `I128`, `U64` or `U128`. A missing field, an unparseable number, or `status: failure`
  is an error. The error carries the exchange's `error_code` and message (`NadoApiError`).
- **No caller policy.** The library does not choose nonces, `recv_time`s, expirations, client ids,
  retries, products or depths. The key is passed in. Nothing is read from the environment or the
  clock.

## Modules

| Module | What it does |
|---|---|
| `signer` | `Signer` (secp256k1 EIP-712), `Subaccount`, `Order`, `PlaceOrder`, `CancelOrders`, `CancelProductOrders`, `LinkSigner`, `Appendix`, `order_nonce`, `order_verifying_contract`. Each `sign_*` call returns `SignedExecute { kind, digests, payload }`. |
| `rest::MarketApi` (`NadoMarketApi`) | Gateway queries, edge (cached) queries, archive queries and V2 REST market data. |
| `rest::TradingApi` (`NadoTradingApi`) | Executes: `place_order`, `place_orders`, `cancel_orders`, `cancel_product_orders`, `cancel_and_place`, `link_signer`. Account queries: `nonces`, `subaccount_info`, `order`, `subaccount_orders`, `orders`, `fee_rates`, `linked_signer`, `max_order_size`. Archive: `matches`, `archive_orders`, `interest_and_funding`. |
| `types` | Reply types per API (`gateway`, `archive`, `v2`, `ws`). `NadoStream` builds subscribe and unsubscribe frames. `NadoWsMessage::parse` splits events from replies. |
| crate root | `NadoClient` holds the market REST client and `ws_config()` for roshar-ws-mgr, with `subscribe_*` and `unsubscribe_*`. Also `NadoApi` (async trait), `NadoNetwork` and the URL constants. |

## API docs

| Area | Reference |
|---|---|
| Endpoints | https://docs.nado.xyz/developer-resources/api/endpoints |
| Gateway REST / WS | https://docs.nado.xyz/developer-resources/api/gateway, `/gateway/executes/*`, `/gateway/queries/*` (incl. `edge-all-products`, `health-groups`, `insurance`), https://docs.nado.xyz/developer-resources/api/gateway/websocket-v2 |
| Edge queries | https://docs.nado.xyz/developer-resources/api/gateway/edge (`/edge/query`: `cached_*`, `cached_bbo_history`, `ping`, `time`) |
| Archive (indexer) | https://docs.nado.xyz/developer-resources/api/archive-indexer (candlesticks, edge-candlesticks, product-snapshots, funding-rate, funding-rate-history, perp-prices, oracle-price, oracle-snapshots, market-snapshots, edge-market-snapshots, matches-and-liquidations, liquidation-feed, quote-price, sequencer-backlog, matches, orders, interest-and-funding-payments) |
| V2 REST | https://docs.nado.xyz/developer-resources/api/v2 (assets, pairs, apr, orderbook, tickers, contracts, trades, symbols) |
| Subscriptions | https://docs.nado.xyz/developer-resources/api/subscriptions, `/subscriptions/streams`, `/subscriptions/events`, `/subscriptions/authentication` |
| Auth / signing | https://docs.nado.xyz/developer-resources/api/gateway/signing, `/signing/q-and-a`, https://docs.nado.xyz/developer-resources/api/order-appendix |
| Rate limits | https://docs.nado.xyz/developer-resources/api/rate-limits, https://docs.nado.xyz/developer-resources/api/subscriptions/rate-limits |
| Errors | https://docs.nado.xyz/developer-resources/api/errors |
| Changelog | https://docs.nado.xyz/developer-resources/api/api-changelog (latest entry 2026-10-26) |
| SDK used for vectors | `nado-protocol` 2.0.0 on PyPI (https://nadohq.github.io/nado-python-sdk/). The TypeScript SDK is `@nadohq/client` 0.73.0 (https://github.com/nadohq/nado-typescript-sdk); it was not used. |

Checked against these on 2026-11-15; edge, snapshot and feed pages re-checked on 2026-11-16.

UK access: the Restricted Territories page (https://docs.nado.xyz/legal/legal/restricted-territories)
fully blocks Belarus, Cuba, Iran, North Korea, Panama, Russia, Syria and three occupied Ukrainian
regions. It sets Canada and the USA to view-only. The UK is not listed. The API/SDK terms PDF linked
from https://docs.nado.xyz/developer-resources/api-sdk-terms-of-use was not reviewed. From a UK IP,
every public REST endpoint and both websockets worked. Unauthenticated executes reached the engine
and got business errors (2024 "no previous deposits", 2012 recv_time), not a geo-block.

## Transport requirements (checked live)

- **HTTP** requires `Accept-Encoding: gzip|br|deflate`. Without it Cloudflare returns 403
  `{"reason":"Invalid compression headers: ...","block":true}`. This crate turns on reqwest 0.11's
  `gzip` feature. That feature is shared with roshar-http's client, so every request sends the
  header and the reply is decoded.
- **Subscriptions websocket** on `api.prod.nado.xyz` and `gateway.prod.nado.xyz` returns 403 to
  upgrades without `Sec-WebSocket-Extensions: permessage-deflate`. roshar-ws-mgr (tungstenite) does
  not negotiate deflate, so `NadoClient::ws_config()` defaults to the documented direct host
  `wss://direct-gateway.prod.nado-backend.xyz/v1/subscribe`, which accepts uncompressed
  connections. Change `ws_config_mut().url` to use another host.
- The gateway execute/query websockets (`/v1/ws`, `/ws/v2`) accept uncompressed upgrades.
- Keepalive: protocol ping frames at least every 30 s. The config pings every 20 s.

## Coverage

| Item | Endpoint / channel |
|---|---|
| Instruments | gateway `symbols`, `all_products`, `edge_all_products` (per chain), `contracts`, `health_groups`; V2 `assets`, `pairs`, `symbols` (archive); archive `products` (one product's snapshot history), `product_snapshots` (latest per product) |
| Tickers | V2 `tickers` (24h), `contracts` (perp OI, index/mark, funding); gateway `market_price(s)` |
| Order book | gateway `market_liquidity` (x18, ≤100 levels); V2 `orderbook` (decimals) |
| Trades | V2 `trades` (archive); archive `matches_and_liquidations` by `product_ids` |
| Candles | archive `candlesticks`, `edge_candlesticks` (all chains) |
| Funding / OI | archive `funding_rate`, `funding_rates` (24h predicted), `funding_rate_history` (realized hourly), `market_snapshots` / `edge_market_snapshots` (OI, funding, volume, deposits/borrows history); V2 `contracts` |
| Prices | archive `price`, `perp_prices`, `oracle_price`, `oracle_snapshots` (history), `quote_price` (USDT0/USD); V2 `apr` |
| Liquidations / insurance | archive `matches_and_liquidations`, `liquidation_feed` (liquidatable subaccounts); gateway `insurance`; WS `liquidation` |
| Status | gateway `status`; archive `backlog` (sequencer backlog) |
| Edge (cached) | `cached_prices`, `cached_symbols`, `cached_contracts`, `cached_status`, `cached_all_products`, `cached_edge_all_products`, `cached_health_groups`, `cached_bbo_history`; control `ping`, `time` (`parse_edge_control_reply`) |
| Public WS | `book_depth`, `trade`, `best_bid_offer`, `latest_candlestick`, `funding_rate`, `funding_payment`, `liquidation`, `all_bbo`; control `list`, `ping`, `time`; `subscribe_multi` / `unsubscribe_multi` frames (see below) |
| Executes | `place_order`, `place_orders`, `cancel_orders` (by digest), `cancel_product_orders` (cancel-all), `cancel_and_place` (modify), `link_signer` |
| Account | gateway `nonces`, `subaccount_info` (balances, positions, health), `order`, `subaccount_orders`, `orders`, `fee_rates`, `linked_signer`, `max_order_size`; archive `matches` (fills), `orders`, `interest_and_funding` |
| Private WS | `authenticate` (`StreamAuthentication`), `order_update`; `fill` and `position_change` (these need no auth) |

All payloads in `tests/fixtures` were captured live from mainnet, except the `order_update`,
`liquidation` and `funding_payment` events and the execute success bodies. Those are taken from
the docs and are inline in `tests/parse.rs`.

Rate limits: queries are limited per IP to 2400 weight a minute (400 per 10 s), with gateway and
archive counted separately. Edge queries have their own bucket of 12000 weight a minute (2000 per
10 s, weight 1 each). Executes are limited per wallet to 600 weight a minute. `NadoClient`
defaults to 20 requests/s, applied separately to gateway, edge and archive. `TradingApi::new`
takes separate query and execute rates.

`subscribe_multi` / `unsubscribe_multi` (`{"method":"subscribe_multi","streams":[..],"id"}`) are
not on the docs site; the frame is the one built by `nado-protocol`'s `subscribe_multi_message`.
On 2026-11-16 the direct host answered every such frame from this IP with
`{"result":null,"error":"multi subscriptions not enabled for this IP: <ip>","id":..}` (fixture
`ws_multi_not_enabled.json`), so a successful multi subscription has not been observed. The
builders (`subscribe_multi_message`, `NadoClient::subscribe_multi`) are provided for allowlisted
IPs.

## Signing

- Domain: `EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)`
  with name `Nado` and version `0.0.1`. `chainId` and the endpoint address come from the
  `contracts` query. On 2026-11-15 they were mainnet 57073 / `0x05ec92d78ed421f3d3ada77ffde167106565974e`
  and testnet 763373 / `0x698d87105274292b5673367dec81874ce3633ac2`.
- `verifyingContract` is `address(product_id)` for `Order`. Every other struct uses the endpoint
  address.
- Types: `Order(bytes32 sender,int128 priceX18,int128 amount,uint64 expiration,uint64 nonce,uint128 appendix)`,
  `Cancellation(bytes32 sender,uint32[] productIds,bytes32[] digests,uint64 nonce)`,
  `CancellationProducts(bytes32 sender,uint32[] productIds,uint64 nonce)`,
  `LinkSigner(bytes32 sender,bytes32 signer,uint64 nonce)`,
  `StreamAuthentication(bytes32 sender,uint64 expiration)`.
- Signature: secp256k1 RFC 6979 over the EIP-712 digest, encoded as `0x || r || s || v` with
  `v` 27 or 28. Uses `k256` and `sha3`. The order digest is the order id on the exchange.
- `sender` is the owner address (20 bytes) followed by the subaccount name (12 bytes, e.g.
  `default`). The key can be the owner's or a linked signer's (set with `link_signer`, which
  needs at least 5 USDT0 of account value).
- Order and cancel nonces are `recv_time_ms << 20 | random20`. The engine accepts cancels only
  when `now < recv_time ≤ now + 100 s`. `link_signer` uses the sequential `tx_nonce`.
- The appendix packs version 1, the isolated bit, order type (default/IOC/FOK/post-only),
  reduce-only, trigger type, builder id and fee, and a 64-bit value (isolated margin or TWAP).
  The client `id` is not signed.

### Parity

`tests/gen_vectors.py` runs `nado-protocol` 2.0.0's `EngineExecuteClient` with a fresh random key.
`_execute` is stubbed out, so it makes no network calls and records the exact `/execute` body. It
also calls the SDK's `build_appendix`, `subaccount_to_hex`, `get_order_digest`, `build_digest` and
`ws.messages.authenticate_message`.

The output is `tests/vectors.json`. It has 10 appendices, 7 `place_order` cases (post-only, IOC
reduce-only without spot leverage, isolated FOK with builder, i128/u64/u128 extremes, negative
price, testnet), a `place_orders` batch, 3 `cancel_orders`, 3 `cancel_product_orders` (one with an
empty list, i.e. all products), `cancel_and_place`, 3 `link_signer` (one a revoke) and 2 stream
authentications.

Both sides use deterministic RFC 6979 signatures. `tests/vectors.rs` therefore asserts byte-equal
payloads (the JSON `Value`, including signatures) and equal digests. Never regenerate the vectors
to make a change pass. To regenerate:

```sh
uv venv /tmp/nado-venv --python 3.12
VIRTUAL_ENV=/tmp/nado-venv uv pip install nado-protocol==2.0.0 "setuptools<81"
/tmp/nado-venv/bin/python -W ignore tests/gen_vectors.py > tests/vectors.json
```

## Usage

```rust
use roshar_nado::{signer::*, NadoNetwork, NadoTradingApi};

let api = NadoTradingApi::new(NadoNetwork::Mainnet, 20, 10);
let signer = Signer::from_hex(&key_hex, 57073, "0x05ec92d78ed421f3d3ada77ffde167106565974e")?;
let sender = Subaccount::new(owner_address, "default")?;

let order = PlaceOrder {
    product_id: 2,                                      // BTC-PERP
    order: Order {
        sender,
        price_x18: 65_000 * 10i128.pow(18),
        amount: 15 * 10i128.pow(16),                    // +0.15 buys; negative sells
        expiration: now_s + 3600,
        nonce: order_nonce(now_ms + 5_000, random20)?,
        appendix: Appendix::new(OrderType::PostOnly).encode()?,
    },
    id: Some(1),
    spot_leverage: None,
};
let signed = signer.sign_place_order(&order)?;
let digest = api.place_order(&signed).await?.digest;   // or send signed.to_json() over NADO_WS_URL

let cancel_all = CancelProductOrders { sender, product_ids: vec![], nonce: order_nonce(now_ms + 5_000, r)?, id: None };
api.cancel_product_orders(&signer.sign_cancel_product_orders(&cancel_all)?).await?;

// Private stream: authenticate the subscriptions connection, then subscribe.
let auth = signer.stream_auth_message(&sender, now_ms + 60_000, 1)?;
client.authenticate(&manager, auth)?;
client.subscribe(&manager, &NadoStream::OrderUpdate { subaccount: sender.to_hex(), product_id: None }, Some(2))?;
```

## Not covered yet

- Deposits (on-chain), `withdraw_collateral` (v1/v2), `transfer_quote` (v1/v2),
  `liquidate_subaccount`, `mint_nlp` and `burn_nlp`. Add them to `gen_vectors.py` first.
- Trigger service (`/trigger/v1`: stop, TP/SL, TWAP orders and `ListTriggerOrders` signing). The
  appendix trigger and TWAP bits are encoded, but nothing submits to the trigger service.
- Some gateway queries that are account or vault data, not market data: `isolated_positions`,
  `max_withdrawable`, `dynamic_fee_quote`, NLP queries (and edge `cached_nlp_pool_info`),
  `validate_order`, `subaccount_info` simulation (`txns`, `pre_state`).
- Archive queries that are account, vault or analytics data: `events`, `positions`,
  `portfolio*`, `subaccounts`, `subaccount_snapshots`, `linked_signers`, `linked_signer_rate_limit`,
  `isolated_subaccounts`, `signatures`, `fast_withdrawal_signature`, NLP payments / snapshots,
  `tx_hashes`, `direct_deposit_address`, `market_net_fees` (fee analytics). The rewards API is
  not covered.
- A client for the gateway execute/query websocket. Payloads are identical:
  `SignedExecute::to_json()` gives the frame, but no reply correlation is provided.
- A permessage-deflate subscriptions connection, which needs roshar-ws-mgr support.
