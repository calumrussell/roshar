# roshar-grvt

GRVT client: public market data (REST + websocket), API-key and wallet login, EIP-712 order
signing, trading REST, and the private trading websocket.

## Principles

Same as the other roshar crates (see `roshar-hyperliquid/README.md`):

- **Mirror the exchange.** Signed order and wallet-login bodies, including `r`/`s`/`v`, are
  identical to GRVT's Python signers. Replies are parsed strictly; a non-2xx reply surfaces the
  exchange's `code`/`message` as `GrvtApiError`.
- **No caller policy.** Nonces, expirations, client order ids, JSON-RPC ids, rates, depths and
  session refresh are the caller's. Keys and API keys are passed in; nothing reads the environment
  or the clock.

UK access: GRVT's restricted-jurisdictions list (US, Canada, Singapore, Hong Kong, sanctioned
countries and regions) does not include the UK
(https://help.grvt.io/en/articles/9711621-restricted-jurisdictions). Onboarding requires KYC
("Onboarding requires KYC verification through ComplyCube", https://grvt.io/blog/how-to-trade-tokenized-etfs/),
and API keys are created in the UI of a KYC'd account. From a UK IP on 2026-11-15, every public
REST and websocket endpoint answered. Auth and trading endpoints returned normal errors (401
`Invalid Credentials`, code 1000), not a geo-block. Cloudflare returns 403 to plain `curl` on
`api-docs.grvt.io` and `grvt.io`, but not to the API hosts.

## Modules

| Module | What it does |
|---|---|
| crate root | `GrvtClient` (market REST via `GrvtApi`, `ws_config()` for roshar-ws-mgr, public `subscribe_*`/`unsubscribe_*`, websocket JSON-RPC `query_*`), `GrvtTradingStream` (private websocket config with session headers, private subscriptions, `v1/create_order`/`cancel_order`/`cancel_all_orders` JSON-RPC), URL constants for mainnet and testnet, `GrvtTradingApi`, `GrvtMarketApi`, `GrvtApiError`. |
| `signer` | `Signer` (secp256k1 key + chain id): `sign_order` → `GrvtOrderRequest`, `sign_wallet_login` → `GrvtWalletLoginRequest`, plus `domain_separator`, `order_struct_hash`, digests, `scale_decimal`. |
| `types` | `full`-encoding wire types, market-data request bodies (`Grvt*Request`, shared by REST and JSON-RPC), JSON-RPC request/response wrappers, stream names, `selector::*` feed selector builders. |

`GrvtTradingApi::new(GRVT_EDGE_URL, GRVT_TRADES_URL)` provides `login_api_key`, `login_wallet`,
`create_order`, `cancel_order`, `cancel_all_orders`, `bulk_orders`, `cancel_on_disconnect`,
`get_order`, `open_orders`, `order_history`, `fill_history`, `funding_payment_history`,
`positions` and `account_summary`.

## API docs

| Area | Reference |
|---|---|
| Overview, auth, websocket format, chain ids | https://api-docs.grvt.io/ |
| Market data REST | https://api-docs.grvt.io/market_data_api/ |
| Market data websocket | https://api-docs.grvt.io/market_data_streams/ |
| Trading REST | https://api-docs.grvt.io/trading_api/ (schema pages, e.g. https://api-docs.grvt.io/schemas/api_sub_account_summary_response) |
| Trading websocket | https://api-docs.grvt.io/trading_streams/ |
| Docs source | https://github.com/gravity-technologies/api-spec (`artifacts/apidocs`, `9e6dc15`, 2026-09-21; older than the live site, used for sections the live page truncates) |
| Auth / signing | https://github.com/gravity-technologies/grvt-pysdk (`src/pysdk/grvt_raw_signing.py`, `grvt_raw_base.py`); wallet login: https://github.com/gravity-technologies/builder-examples (`python/wallet_login.py`, `docs/WALLET_LOGIN_README.md`) |
| Rate limits | https://help.grvt.io/en/articles/9636566-what-are-the-rate-limitations-on-grvt |
| Changelog | None published. |
| SDKs used for vectors | `grvt-pysdk` at `bb77839` (2026-03-16; the first commit with `OrderWithBuilderFee`; PyPI's latest, 0.2.1, predates it) with `eth-account` 0.14.0; `builder-examples` at `6ad03de` (2026-05-20). |

Checked against these on 2026-11-15; market-data REST and websocket queries rechecked on 2026-11-16.

## Coverage

| Item | Endpoint / channel |
|---|---|
| Instruments | `full/v1/instrument`, `full/v1/all_instruments`, `full/v1/instruments` |
| Currencies, margin tiers | `full/v1/currency`, `full/v1/margin_rules` |
| Supported assets per wallet, collateral config | `full/v1/supported_assets`, `full/v1/get_all_collateral_asset_info` |
| Tickers (incl. current funding rate) | `full/v1/mini`, `full/v1/ticker` |
| Order book | `full/v1/book` (depth 10/50/100/500) |
| Trades | `full/v1/trade`, `full/v1/trade_history` |
| Candles | `full/v1/kline` |
| Funding history | `full/v1/funding` |
| Server time | `GET /time` |
| Public WS (`/ws/full`) | `v1.book.s`, `v1.book.d`, `v1.trade`, `v1.mini.s`/`.d`, `v1.ticker.s`/`.d`, `v1.candle` (funding is on the ticker stream) |
| Public WS JSON-RPC queries | every market-data REST method above as `v1/<method>` (`GrvtClient::query_*`); the reply is `GrvtRpcResponse<R>` with the REST reply type `R` and `method` echoed. Without an `id` the server answers with `"id":0`. |
| Auth | `edge/auth/api_key/login`, `edge/auth/wallet/login` → `GrvtSession` (`gravity` cookie, `X-Grvt-Account-Id`) |
| Orders | `create_order`, `cancel_order`, `cancel_all_orders`, `v2/bulk_orders` (bulk create / cancel / cancel-replace; GRVT has no amend), `cancel_on_disconnect`, `order`, `open_orders`, `order_history` |
| Account | `positions`, `fill_history`, `funding_payment_history`, `account_summary` |
| Private WS (`trades/ws/full`) | `v1.order`, `v1.state`, `v1.fill`, `v1.position`, `v1.cancel`; JSON-RPC `v1/create_order`, `v1/cancel_order`, `v1/cancel_all_orders` |

Public payload types were built from live captures (`tests/fixtures`). Trading payload types were
built from the docs and the SDK; no private reply was captured.

`GrvtClient` rate-limits REST to 8 requests/s by default: market data allows 500 requests/min for
most endpoints. `GrvtTradingApi` is not rate-limited. GRVT limits per main account: creates
260/10 s per instrument, cancels 2600/10 s, reads 25–225/10 s by tier.

GRVT has no JSON-RPC ping (`"ping"` returns code 1200). The server answers protocol pings, and
the websocket configs send them.

## Signing

EIP-712, domain `{name: "GRVT Exchange", version: "0", chainId}`, no verifying contract.
`MAINNET_CHAIN_ID` = 325, `TESTNET_CHAIN_ID` = 326.

| Type | Fields |
|---|---|
| `Order` | `uint64 subAccountID, bool isMarket, uint8 timeInForce, bool postOnly, bool reduceOnly, OrderLeg[] legs, uint32 nonce, int64 expiration` |
| `OrderWithBuilderFee` | as `Order`, plus `address builder, uint32 builderFee` before `nonce`; used when the order has a builder |
| `OrderLeg` | `uint256 assetID` (instrument `instrument_hash`), `uint64 contractSize` (size × 10^`base_decimals`), `uint64 limitPrice` (price × 10^9), `bool isBuyingContract` |
| `WalletLogin` | `address signer, uint32 nonce, int64 expiration` (ns, at most 5 minutes ahead) |

`timeInForce` is signed as GTT 1, AON 2, IOC 3, FOK 4. `RETAIL_PRICE_IMPROVEMENT` has no
signing value in the SDK and is rejected. The builder fee is signed as fee (percent) × 10^4. The
SDK truncates digits beyond the scale. `scale_decimal` rejects them instead, because the exchange
would see a signed value that differs from the decimal string. Order signatures omit
`signature.chain_id` as the SDK does; wallet login sends it as `builder-examples` does.

### Parity

`tests/vectors.json` comes from `tests/gen_vectors.py`: `grvt_raw_signing.sign_order` for
orders and `wallet_login.build_eip712_payload` + `grvt_common.sign_eip712` for logins, with a
throwaway random key and no network. ECDSA here is deterministic (RFC 6979, low-s), so
`tests/vectors.rs` checks the domain separator, struct hash and digest, and that the
whole request body matches exactly, including `r`, `s` and `v`. The SDK's body is its
`create_order_v1` serialisation with null fields dropped. Cases: GTT post-only, IOC reduce-only,
FOK market, AON on testnet with u64-max sub account and nonce 0, a 6-decimal `STABLE_PERP`, a
`SPOT_SWAP`, two legs, and two builder-fee orders. Never regenerate the vectors to make a change
pass.

## Usage

```rust
use roshar_grvt::signer::{NewOrder, NewOrderLeg, Signer, MAINNET_CHAIN_ID};
use roshar_grvt::types::*;
use roshar_grvt::{GrvtApi, GrvtClient, GrvtTradingApi, GrvtTradingStream, GRVT_EDGE_URL, GRVT_TRADES_URL, GRVT_TRADES_WSS_URL};

let market = GrvtClient::new();
let btc = market.get_instrument("BTC_USDT_Perp").await?;

let api = GrvtTradingApi::new(GRVT_EDGE_URL, GRVT_TRADES_URL);
let session = api.login_api_key(&api_key).await?;          // refresh before session.cookie_expires
let signer = Signer::from_hex(&signing_key, MAINNET_CHAIN_ID)?;

let order = signer.sign_order(&NewOrder {
    sub_account_id, is_market: false, time_in_force: GrvtTimeInForce::GoodTillTime,
    post_only: true, reduce_only: false,
    legs: vec![NewOrderLeg {
        instrument: btc.instrument.clone(), instrument_hash: btc.instrument_hash.clone(),
        base_decimals: btc.base_decimals as u32, size: "0.001".into(), limit_price: "64000".into(),
        is_buying_asset: true,
    }],
    client_order_id: 9_300_000_000_000_000_001, builder: None,
    nonce, expiration_ns: now_ns + 3_600_000_000_000,           // caller picks both
})?;
let placed = api.create_order(&session, &order).await?;

api.cancel_order(&session, &GrvtCancelOrderRequest {
    sub_account_id: sub_account_id.to_string(), order_id: None,
    client_order_id: Some("9300000000000000001".into()), time_to_live_ms: None,
}).await?;

let stream = GrvtTradingStream::new(GRVT_TRADES_WSS_URL, &session); // stream.ws_config() → Manager
stream.subscribe_fills(&manager, sub_account_id, None, Some(1))?;
```

## Not covered yet

- Trading: TP/SL and other trigger metadata (`metadata.trigger`), `slippage_bps`, broker tags,
  ECN/RFQ (`create_rfq`, `cancel_rfq`, `set_indicative_prices`, `ecn_from_broker`,
  `v1.rfq`/`v1.quote`/`v1.ecn_to_broker`). Also `RETAIL_PRICE_IMPROVEMENT` orders, which the SDK
  cannot sign.
- Account: `set_position_config` (leverage and margin type; it needs an EIP-712 type that no
  official SDK implements), deprecated `set_initial_leverage`/`get_all_initial_leverage`,
  `position_history`, `interest_payment_history`, `add_position_margin`,
  `get_position_margin_limits`, `get_collateral_preference`, `spot_account_summary`,
  `aggregated_account_summary`, `funding_account_summary`, `get_sub_accounts`,
  `set_derisk_mm_ratio`, builder and vault endpoints, referral data.
- Funds: deposits, transfers, withdrawals (`transfer`, `withdrawal`, `*_history`,
  `withdrawal_fee`) and their streams (`v1.deposit`, `v1.transfer`, `v1.withdrawal`).
- Market data: the `lite` encoding (same data with abbreviated keys) and the legacy `/ws`
  subscribe format; both duplicate what `full` already covers. `GET /time` has no JSON-RPC form.
