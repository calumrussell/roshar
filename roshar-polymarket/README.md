# roshar-polymarket

Polymarket prediction-market public market data: Gamma metadata (`GammaApi`), CLOB order books
and prices (`ClobApi`), Data API v2 market-level reads (`DataApi`) and the CLOB market websocket
(`PolymarketMarketWs` + `types::MarketWs*`). Everything is unauthenticated; there is no trading.

## API docs

| Area | Reference |
|---|---|
| REST overview, base URLs | https://docs.polymarket.com/api-reference/predictions/overview |
| Docs index (all endpoint pages) | https://docs.polymarket.com/_llms/en/api-reference/predictions.txt |
| Gamma schema (OpenAPI) | https://docs.polymarket.com/api-spec/gamma-openapi.yaml |
| CLOB schema (OpenAPI) | https://docs.polymarket.com/api-spec/clob-openapi.yaml |
| Data API v2 schema (OpenAPI) | https://data-api.polymarket.com/v2/openapi.json |
| Data API v1 to v2 migration | https://docs.polymarket.com/migrate/data-api-v1-to-v2 |
| Market websocket | https://docs.polymarket.com/api-reference/wss/market (AsyncAPI: https://docs.polymarket.com/asyncapi.json) |
| Real-time feeds guide | https://docs.polymarket.com/market-data/realtime-data |
| Rate limits | https://docs.polymarket.com/api-reference/rate-limits |
| Geographic restrictions | https://docs.polymarket.com/api-reference/geoblock |
| Changelog | https://docs.polymarket.com/changelog/predictions |

Checked against these on 2026-11-16.

## Coverage

### Gamma (`GammaApi`, `https://gamma-api.polymarket.com`)

| Item | Endpoint |
|---|---|
| `status() -> String` | `GET /status` (plain text `OK`) |
| `list_events(&ListEventsParams) -> EventsPage` | `GET /events/keyset` (pass `next_cursor` back as `after_cursor`) |
| `list_events_paginated(&GammaPageParams, recurrence) -> GammaEventsPagination` | `GET /events/pagination` |
| `event(id)`, `event_by_slug(slug) -> PolymarketEvent` | `GET /events/{id}`, `GET /events/slug/{slug}` |
| `event_tags(id) -> Vec<PolymarketTag>` | `GET /events/{id}/tags` |
| `list_markets(&GammaListMarketsParams) -> GammaMarketsPage` | `GET /markets/keyset` |
| `market(id, include_tag)`, `market_by_slug(slug, include_tag) -> PolymarketMarket` | `GET /markets/{id}`, `GET /markets/slug/{slug}` |
| `market_tags(id)`, `market_description(id) -> GammaMarketDescription` | `GET /markets/{id}/tags`, `GET /markets/{id}/description` |
| `markets_information(&GammaMarketsInformationBody)`, `markets_abridged(..) -> Vec<PolymarketMarket>` | `POST /markets/information`, `POST /markets/abridged` |
| `series(&GammaSeriesParams)`, `series_by_id(id) -> PolymarketSeries` | `GET /series`, `GET /series/{id}` |
| `series_summary(id)`, `series_summary_by_slug(slug) -> GammaSeriesSummary` | `GET /series-summary/{id}`, `GET /series-summary/slug/{slug}` |
| `tags(&GammaTagsParams)`, `tag(id)`, `tag_by_slug(slug) -> PolymarketTag` | `GET /tags`, `GET /tags/{id}`, `GET /tags/slug/{slug}` |
| `related_tags(id, ..)`, `related_tags_by_slug(slug, ..) -> Vec<GammaRelatedTag>` | `GET /tags/{id}/related-tags`, `GET /tags/slug/{slug}/related-tags` |
| `related_tag_tags(id, ..)`, `related_tag_tags_by_slug(slug, ..) -> Vec<PolymarketTag>` | `GET /tags/{id}/related-tags/tags`, `GET /tags/slug/{slug}/related-tags/tags` |
| `search(q, &GammaSearchParams) -> GammaSearch` | `GET /public-search` (events and tags; profiles ignored) |
| `sports() -> Vec<GammaSport>`, `sports_market_types() -> GammaSportsMarketTypes` | `GET /sports`, `GET /sports/market-types` |
| `teams(&GammaTeamsParams)`, `team(id) -> GammaTeam` | `GET /teams`, `GET /teams/{id}` |

### CLOB (`ClobApi`, `https://clob.polymarket.com`)

| Item | Endpoint |
|---|---|
| `server_time() -> u64` | `GET /time` |
| `markets(cursor)`, `sampling_markets(cursor) -> ClobMarketsPage<ClobMarket>` | `GET /markets`, `GET /sampling-markets` |
| `simplified_markets(cursor)`, `sampling_simplified_markets(cursor) -> ClobMarketsPage<ClobSimplifiedMarket>` | `GET /simplified-markets`, `GET /sampling-simplified-markets` |
| `market(condition_id) -> ClobMarket` | `GET /markets/{condition_id}` |
| `clob_market_info(condition_id) -> ClobMarketInfo` | `GET /clob-markets/{condition_id}` |
| `market_by_token(token_id) -> ClobMarketByToken` | `GET /markets-by-token/{token_id}` |
| `live_activity_market(condition_id)`, `live_activity_markets(&[..]) -> ClobLiveActivityMarket` | `GET /markets/live-activity/{condition_id}`, `POST /markets/live-activity` |
| `book(token_id) -> ClobBook`, `books(&[ClobBookRequest]) -> Vec<ClobBook>` | `GET /book`, `POST /books` |
| `price(token_id, side) -> ClobPrice`, `prices(..) -> ClobTokenPrices` | `GET /price`, `POST /prices` |
| `midpoint(token_id) -> ClobMidpoint`, `midpoints(..) -> ClobTokenValues` | `GET /midpoint`, `POST /midpoints` |
| `spread(token_id) -> ClobSpread`, `spreads(..) -> ClobTokenValues` | `GET /spread`, `POST /spreads` |
| `last_trade_price(token_id) -> ClobLastTradePrice`, `last_trades_prices(..) -> Vec<ClobLastTradePriceItem>` | `GET /last-trade-price`, `POST /last-trades-prices` |
| `tick_size`, `neg_risk`, `fee_rate` (token_id) | `GET /tick-size/{token_id}`, `GET /neg-risk/{token_id}`, `GET /fee-rate/{token_id}` |
| `prices_history(token_id, &ClobPricesHistoryParams) -> ClobPricesHistory` | `GET /prices-history` |
| `batch_prices_history(&ClobBatchPricesHistoryRequest) -> ClobBatchPricesHistory` | `POST /batch-prices-history` |

### Data API v2 (`DataApi`, `https://data-api.polymarket.com`)

| Item | Endpoint |
|---|---|
| `trades(&DataTradesParams) -> DataPage<DataTrade>` | `GET /v2/trades` (market, event or global feed) |
| `holders(conditions, &DataHoldersParams) -> DataPage<DataTokenHolders>` | `GET /v2/holders` |
| `open_interest(conditions) -> DataEnvelope<Vec<DataOpenInterest>>` | `GET /v2/oi` |
| `live_volume(event_ids) -> DataEnvelope<DataLiveVolume>` | `GET /v2/live-volume` |
| `prices_history(token_id, &DataPriceWindow, &DataPricesHistoryParams) -> DataPage<DataPricePoint>` | `GET /v2/prices-history` |
| `resolutions(&DataResolutionSelector) -> DataEnvelope<Vec<DataResolution>>` | `GET /v2/resolutions` |
| `tokens(&DataTokenSelector) -> DataEnvelope<Vec<DataToken>>` | `GET /v2/tokens` |
| `status() -> DataEnvelope<DataStatus>` | `GET /v2/status` (data freshness) |

### Market websocket (`wss://ws-subscriptions-clob.polymarket.com/ws/market`)

| Item | Frame |
|---|---|
| `PolymarketMarketWs::ws_config()` | `roshar_ws_mgr::Config` (see ping note below) |
| `subscribe(&MarketWsSubscription)` | `{"type": "market", "assets_ids", "initial_dump"?, "level"?, "custom_feature_enabled"?}` |
| `add_assets`, `remove_assets`, `update(&MarketWsUpdate)` | `{"operation": "subscribe" \| "unsubscribe", "assets_ids", ...}` |
| `MarketWsMessage::parse(text)` | `PONG`, `INVALID OPERATION`, a JSON array (`Batch`) or one event (`Event`) |
| `MarketWsEvent::{Book, PriceChange, LastTradePrice, TickSizeChange}` | `book`, `price_change`, `last_trade_price`, `tick_size_change` |
| `MarketWsEvent::{BestBidAsk, NewMarket, MarketResolved}` | `best_bid_ask`, `new_market`, `market_resolved` (need `custom_feature_enabled: true`) |

### Notes

| Note | Detail |
|---|---|
| Errors | All REST calls return `ApiResult<T>` (`Box<dyn Error + Send + Sync>`, unchanged from the original `GammaApi`). Exchange failures are a `PolymarketApiError` (`Status` with the raw body, or `Decode`); the bodies parse as `GammaError` (`{type, error}`), `ClobError` (`{error}`) or `DataError` (`{error, code, parameter, retryable, trace_id}`). |
| Compression | Gamma pages with `limit` above 50 are served gzip-only (HTTP 406 `gzip_required` otherwise). The crate turns on reqwest's `gzip` feature. |
| Deprecated / retired | Gamma `GET /events` and `GET /markets` (offset paging) send `Deprecation: true`, `Sunset: 1 May 2026` and `Warning: use /events/keyset` / `use /markets/keyset`; not exposed. Data API v1 (`/trades`, `/holders`, `/oi`, `/live-volume`, ...) answers HTTP 410 `Data API v1 has been retired` (retired 2 Nov 2026); only v2 is implemented. |
| Gamma query arrays | Array filters are sent as repeated keys (`id=1&id=2`); a comma list gets 422. `/markets/keyset` `closed=any` (`GammaClosedFilter::Any`) is accepted only with an identifier filter. `/tags`, `/teams` cap pages at 100 rows. |
| Gamma optional fields | The spec marks nearly every field nullable; live responses omit them instead. Only `Event.id`, `Event.markets`, `Market.id`, `Market.conditionId`, `Tag.id/label/slug` (and the `Gamma*` types' required fields) were present on every live row. `Market.liquidity`/`volume`/`lowerBound`/`upperBound` are decimal strings; `outcomes`, `outcomePrices`, `clobTokenIds`, `umaResolutionStatuses` are JSON arrays inside a string. Markets returned by `/markets*` carry their parent `events` (`GammaEventRef`, no markets). |
| CLOB batch routes | The documented `GET /books`, `GET /prices`, `GET /midpoints` and `GET /last-trades-prices` query forms answered HTTP 400 `Invalid payload` for every query shape tried (comma list, repeated keys, JSON array); `GET /spreads` is 405. Only the `POST` (JSON body) forms are exposed. Tokens without a book are left out of batch answers. |
| CLOB vs spec | `GET /markets` and `GET /markets/{condition_id}` are live but no longer in the OpenAPI spec. `/midpoint` returns `mid` (spec: `mid_price`); `/price` and `/prices` return decimal strings (spec: numbers). Book `bids` are ascending and `asks` descending (best level last; the spec says the reverse). `last_trade_price` is `""` before the first trade, and `/last-trade-price` returns `{"price": "0.5", "side": ""}` for such tokens; both map to `None`. `next_cursor == "LTE="` (`CLOB_END_CURSOR`) ends market paging; sending it back is an error. `/clob-markets` omits most keys on some markets, and sends undocumented `nr`, `aot`, `sd`, `cbos`, `ao`, `r.moas`, `r.e`, `v`. `/prices-history` needs `interval` or `startTs`/`endTs`. |
| Data API | `/v2/trades` exposes market, event and global feeds; the `user` filter (wallet feed) and its user-only `start`/`end` are not exposed. Nullable `TokenRef`/`ServiceStatus` keys are always sent (`null`), so they are key-required `Option`s. |
| Market websocket | The first frame must be the subscription; anything else (including `PING`) closes with 1008 `invalid subscription payload`. The reply is one JSON array of `book` snapshots (`[]` if no id is known). A second `type: "market"` frame gets `INVALID OPERATION`; use `MarketWsUpdate`. Snapshot books carry `tick_size` and `last_trade_price`; post-trade books do not. `new_market` is sent for every new market, not only subscribed ids. After the last subscribed asset resolves the server closes with 1000 `all subscribed assets resolved`. A client that drains too slowly is closed with 1013 `slow consumer: send buffer full` (seen with a few dozen busy 5-minute crypto markets). `level` 1, 2 and 3 produced the same frames. |
| Market websocket ping | The docs ask for a text `PING` every 10 s (answered `PONG`). Because the text must not precede the subscription and ws-mgr's first ping fires on connect, the `Config` sends protocol ping frames every 10 s instead (`use_text_ping: false`); the server answers them with pong frames, and a subscribed connection with no pings at all stayed open for 150 s. |
| Rate limits | Callers choose `requests_per_second`. Documented per-IP limits: Gamma 4,000 / 10 s (`/events` 500, `/markets` 300, `/tags` 200, `/public-search` 350); CLOB 9,000 / 10 s (`/book`, `/price`, `/midpoint` 1,500; `/books`, `/prices`, `/midpoints` 500; `/prices-history` 1,000; tick size 200); Data API v2 800 / 10 s (`/v2/trades` 300, `/v2/prices-history` 200, `/v2/status` 100). Cloudflare throttles instead of rejecting. |
| UK | All public reads above (REST and websocket) work from the UK. Polymarket lists GB as close-only for orders on both the frontend and the API; `GET https://polymarket.com/api/geoblock` returned `blocked: true` for a GB IP. |

## Not covered yet

- Trading and accounts: CLOB orders, cancels, heartbeats, L1/L2 auth and signing, user trades,
  notifications, balances, rewards and rebates (all authenticated or account-specific), the CLOB
  user websocket, `GET /builder/trades` (builder-attributed). Out of scope for market data.
- Data API wallet/portfolio routes (`/v2/positions`, `/v2/activity`, `/v2/value`, `/v2/user-*`,
  `/v2/approvals`, combos) and boards (`/v2/leaderboard`, `/v2/biggest-winners`, `/v2/builders/*`):
  account-specific or leaderboards. Bridge and Relayer: deposits/transactions.
- Gamma comments, profiles, tweet counts and comment counts (social, not market data);
  `/events/creators` (event-creator profiles); `GET /events/results`, which returned id-only
  objects (`[{"id": "2890"}, ...]`) live; `/events/keyset` relations added by `include_chat`,
  `include_template`, `include_best_lines` and `partner_slug`.
- Rarely-sent or admin `Market`/`Event` fields (e.g. image optimisation on markets, `marketMetadata`,
  `eventMetadata`, curation and deployment flags). Unknown fields are ignored.
- Sports websocket (`wss://sports-api.polymarket.com/ws`): the server sends a text `ping` every 5 s
  and closes unless each is answered with `pong` within 10 s; roshar-ws-mgr cannot answer
  server-initiated text pings.
- PolyBolt / RTDS live-data websocket (crypto, equity and TWAP reference prices) and combo/RFQ
  markets: separate products not covered by this crate.
- Polymarket Perps (`https://api.perpetuals.polymarket.com`) is a separate API, covered by
  `roshar-polymarket-perps`.
