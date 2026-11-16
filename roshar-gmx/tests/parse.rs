//! Offline parse tests against live captures in tests/fixtures (2026-11-15, Arbitrum).

use roshar_gmx::types::*;
use serde::de::DeserializeOwned;

fn parse<T: DeserializeOwned>(name: &str, raw: &str) -> T {
    serde_json::from_str(raw).unwrap_or_else(|e| panic!("{name}: {e}"))
}

macro_rules! fixture {
    ($name:literal) => {
        include_str!(concat!("fixtures/", $name))
    };
}

#[test]
fn markets() {
    let m: Vec<GmxMarket> = parse("markets", fixture!("markets.json"));
    assert_eq!(m.len(), 3);
    assert_eq!(m[0].symbol, "APT/USD [WETH-USDC]");
    assert!(!m[0].leverage_tiers.is_empty());
    assert!(m.iter().any(|x| x.is_spot_only));
}

#[test]
fn markets_info() {
    let m: Vec<GmxMarketInfo> = parse("markets info", fixture!("markets_info.json"));
    assert_eq!(m.len(), 2);
    assert!(!m[0].funding_factor_per_second.is_empty());
    assert!(m.iter().any(|x| x.is_spot_only));
}

#[test]
fn markets_tickers_and_capacity() {
    let t: Vec<GmxMarketTicker> = parse("tickers", fixture!("markets_tickers.json"));
    assert_eq!(t.len(), 2);
    assert_eq!(
        t[0].capacity_long.limiting_factor,
        GmxLimitingFactor::OpenInterest
    );
    let c: GmxTradingCapacity = parse("capacity", fixture!("trading_capacity.json"));
    assert_eq!(c.market_data_status, GmxMarketDataStatus::Fresh);
    assert_eq!(c.jit_data_status, GmxJitDataStatus::Available);
}

#[test]
fn tokens() {
    let t: Vec<GmxToken> = parse("tokens", fixture!("tokens.json"));
    assert_eq!(t[0].symbol, "ETH");
    assert_eq!(t[0].decimals, 18);
    assert_eq!(t[0].is_native, Some(true));
    let i: Vec<GmxTokenInfo> = parse("tokens info", fixture!("tokens_info.json"));
    assert!(!i[0].prices.max_price.is_empty());
}

#[test]
fn prices_and_candles() {
    let p: GmxTokenPrices = parse("prices", fixture!("prices_tickers.json"));
    assert_eq!(p.prices.len(), 3);
    assert!(p.origin_ts > 1_700_000_000_000);
    let c: Vec<GmxOhlcvCandle> = parse("ohlcv", fixture!("ohlcv.json"));
    assert_eq!(c.len(), 3);
    assert!(c[0].timestamp > c[1].timestamp);
}

#[test]
fn rates_and_pairs() {
    let r: Vec<GmxMarketRates> = parse("rates", fixture!("rates.json"));
    assert_eq!(r[0].rates_snapshots.len(), 3);
    let p: Vec<GmxPair> = parse("pairs", fixture!("pairs.json"));
    assert_eq!(p[0].product_type, "Perpetual");
    assert_eq!(p[0].funding_interval_minutes, Some(60));
    assert!(p[1].funding_rate.is_none());
}

#[test]
fn account_reads() {
    let p: Vec<GmxPosition> = parse("positions", fixture!("positions.json"));
    assert_eq!(p.len(), 2);
    let related = p[0]
        .related_orders
        .as_ref()
        .expect("relatedOrders requested");
    assert_eq!(related[0].order_type, 6);
    assert!(related[0].request_id.is_none());
    let o: Vec<GmxOrder> = parse("orders", fixture!("orders.json"));
    assert!(!o.is_empty());
    let t: GmxTradesList = parse("trades", fixture!("trades.json"));
    assert!(t.has_more);
    assert_eq!(t.next_cursor.as_deref(), Some("5"));
    assert!(t
        .trades
        .iter()
        .any(|x| x.event_name == "OrderExecuted" && x.execution_price.is_some()));
    let b: Vec<GmxWalletBalance> = parse("balances", fixture!("balances_wallet.json"));
    assert_eq!(b.len(), 3);
}

#[test]
fn oracle() {
    let t: Vec<GmxOracleTicker> = parse("oracle tickers", fixture!("oracle_tickers.json"));
    assert_eq!(t.len(), 3);
    let c: GmxOracleCandles = parse("oracle candles", fixture!("oracle_candles.json"));
    assert_eq!(c.period, GmxCandlePeriod::H1);
    assert_eq!(c.candles.len(), 3);
    assert!(c.candles[0].timestamp() > c.candles[1].timestamp());
}

#[test]
fn prepare_express() {
    for (name, raw, creates, updates, cancels) in [
        (
            "increase limit",
            fixture!("prepare_increase_limit.json"),
            1,
            0,
            0,
        ),
        (
            "increase market + SL",
            fixture!("prepare_increase_market.json"),
            2,
            0,
            0,
        ),
        (
            "decrease market",
            fixture!("prepare_decrease_market.json"),
            1,
            0,
            0,
        ),
        ("edit", fixture!("prepare_edit.json"), 0, 1, 0),
        ("cancel", fixture!("prepare_cancel.json"), 0, 0, 1),
        ("cancel all", fixture!("prepare_cancel_all.json"), 0, 0, 1),
    ] {
        let p: GmxPreparedOrder = parse(name, raw);
        assert_eq!(p.mode, GmxTxMode::Express, "{name}");
        let e = p
            .express()
            .unwrap_or_else(|| panic!("{name}: not typed data"));
        assert_eq!(e.typed_data.domain.name, "GmxBaseGelatoRelayRouter");
        assert_eq!(e.typed_data.domain.chain_id, 42161);
        assert_eq!(
            e.relay_router_address,
            e.typed_data.domain.verifying_contract
        );
        let msg = &e.typed_data.message;
        let len = |k: &str| msg[k].as_array().map(Vec::len).unwrap();
        assert_eq!(len("createOrderParamsList"), creates, "{name}");
        assert_eq!(len("updateOrderParamsList"), updates, "{name}");
        assert_eq!(len("cancelOrderKeys"), cancels, "{name}");
        assert_eq!(p.estimates.is_some(), creates > 0, "{name}");

        let submit = GmxSubmitOrderRequest::express(&p, "0xsig".into(), "0xacc".into(), None)
            .expect("express");
        let body = serde_json::to_value(&submit).unwrap();
        assert_eq!(body["requestId"], p.request_id.as_str());
        assert_eq!(body["mode"], "express");
        assert_eq!(body["eip712Data"]["relayParams"], e.relay_params);
        assert_eq!(body["eip712Data"]["batchParams"], e.batch_params);
        assert!(body["eip712Data"].get("subaccountApproval").is_none());
    }
}

#[test]
fn prepare_classic() {
    let p: GmxPreparedOrder = parse("classic", fixture!("prepare_classic.json"));
    assert_eq!(p.mode, GmxTxMode::Classic);
    let tx = p.classic().expect("transaction payload");
    assert_eq!(tx.to, "0x6776c2b52179BC737193DE3bc75262681006AA27");
    assert!(tx.data.starts_with("0xac9650d8")); // multicall(bytes[])
    assert!(GmxSubmitOrderRequest::express(&p, String::new(), String::new(), None).is_none());
}

#[test]
fn prepare_request_shape() {
    let mut req = GmxPrepareOrderRequest::new(
        GmxOrderKind::Increase,
        "ETH/USD [WETH-USDC]",
        GmxOrderType::Limit,
        "10000000000000000000000000000000",
        GmxTxMode::Express,
        "0x1111111111111111111111111111111111111111",
    );
    req.direction = Some(GmxDirection::Long);
    req.trigger_price = Some("1000000000000000000000000000000000".into());
    req.collateral_token = Some("USDC".into());
    req.collateral_to_pay = Some(GmxTokenAmount {
        token: "USDC".into(),
        amount: "2000000".into(),
    });
    req.tpsl = Some(vec![GmxTpsl {
        kind: GmxTpslType::StopLoss,
        trigger_price: "900000000000000000000000000000000".into(),
        size: None,
    }]);
    let v = serde_json::to_value(&req).unwrap();
    assert_eq!(
        v,
        serde_json::json!({
            "kind": "increase",
            "symbol": "ETH/USD [WETH-USDC]",
            "direction": "long",
            "orderType": "limit",
            "size": "10000000000000000000000000000000",
            "triggerPrice": "1000000000000000000000000000000000",
            "collateralToken": "USDC",
            "collateralToPay": {"token": "USDC", "amount": "2000000"},
            "tpsl": [{"type": "stop-loss", "triggerPrice": "900000000000000000000000000000000"}],
            "mode": "express",
            "from": "0x1111111111111111111111111111111111111111"
        })
    );
    let cancel = GmxCancelOrderRequest {
        order_ids: None,
        all: Some(true),
        mode: GmxTxMode::Express,
        from: "0xabc".into(),
        subaccount_address: None,
        subaccount_approval: None,
    };
    assert_eq!(
        serde_json::to_value(&cancel).unwrap(),
        serde_json::json!({"all": true, "mode": "express", "from": "0xabc"})
    );
    assert_eq!(
        serde_json::to_value(GmxOrderType::StopMarket).unwrap(),
        serde_json::json!("stop-market")
    );
}

#[test]
fn error_envelopes() {
    let e = GmxApiError::from_body(400, fixture!("error_invalid_params.json"));
    assert!(e.message.contains("'orderType' is required"));
    assert!(e.code.is_none());
    let e = GmxApiError::from_body(404, fixture!("error_not_found.json"));
    assert!(e.message.starts_with("Order 0x"));
    let e = GmxApiError::from_body(400, fixture!("error_submit_rejected.json"));
    assert_eq!(e.message, "The relay rejected the operation.");
    let e = GmxApiError::from_body(400, fixture!("oracle_error.json"));
    assert!(e.message.starts_with("unsupported period"));
    let e = GmxApiError::from_body(
        400,
        r#"{"code":"INSUFFICIENT_LIQUIDITY","message":"too big","details":{"requestedSizeUsd":"1"}}"#,
    );
    assert_eq!(e.code.as_deref(), Some("INSUFFICIENT_LIQUIDITY"));
    assert!(e.details.is_some());
    let e = GmxApiError::from_body(504, "<html>gateway</html>");
    assert_eq!(e.message, "<html>gateway</html>");
}

#[test]
fn stream_frames() {
    let ack = r#"{"op":"ack","channels":["prices","candles:ETH:1m"]}"#;
    match GmxWsFrame::parse(ack).unwrap() {
        GmxWsFrame::Control(GmxWsControl::Ack { channels }) => assert_eq!(channels.len(), 2),
        other => panic!("{other:?}"),
    }
    let err = r#"{"op":"error","message":"unknown channel(s) ignored","channels":["candles:BTC/USD [BTC-USDC]:1h","bogus"]}"#;
    match GmxWsFrame::parse(err).unwrap() {
        GmxWsFrame::Control(GmxWsControl::Error { message, channels }) => {
            assert_eq!(message, "unknown channel(s) ignored");
            assert_eq!(channels.len(), 2);
        }
        other => panic!("{other:?}"),
    }
    let prices = r#"{"ch":"prices","type":"snapshot","serverTs":1794780113590,"originTs":1794780112000,"data":{"0x7f9FBf9bDd3F4105C478b996B648FE6e828a1e98":{"minPrice":"80090803802833770000000000000","maxPrice":"80148973715300370000000000000"}}}"#;
    match GmxWsFrame::parse(prices).unwrap() {
        GmxWsFrame::Snapshot(s) => {
            assert_eq!(s.origin_ts, Some(1794780112000));
            assert_eq!(s.prices().unwrap().len(), 1);
        }
        other => panic!("{other:?}"),
    }
    let candle = r#"{"ch":"candles:ETH:1m","type":"snapshot","serverTs":1794780114241,"data":{"timestamp":1794780060000,"open":"1818.17","high":"1819.4","low":"1818.12","close":"1819.08"}}"#;
    match GmxWsFrame::parse(candle).unwrap() {
        GmxWsFrame::Snapshot(s) => {
            assert!(s.origin_ts.is_none());
            assert_eq!(s.candle().unwrap().close, "1819.08");
        }
        other => panic!("{other:?}"),
    }

    let sub = GmxWsRequest::subscribe(&[
        GmxWsChannel::Prices,
        GmxWsChannel::Candles {
            symbol: "ETH",
            period: GmxCandlePeriod::M1,
        },
    ]);
    assert_eq!(
        sub.to_json(),
        r#"{"op":"subscribe","channels":["prices","candles:ETH:1m"]}"#
    );
    assert_eq!(
        GmxWsRequest::unsubscribe(&[GmxWsChannel::Prices]).to_json(),
        r#"{"op":"unsubscribe","channels":["prices"]}"#
    );
}

// ── Captures of 2026-11-16 (Arbitrum) ───────────────────────────────────────────────────────

/// Removes `key` from the first object of the top-level array (or of `container`'s array) and
/// asserts that parsing then fails.
fn assert_strict<T: DeserializeOwned>(raw: &str, container: Option<&str>, key: &str) {
    let mut v: serde_json::Value = serde_json::from_str(raw).unwrap();
    let rows = match container {
        Some(c) => v.get_mut(c).unwrap(),
        None => &mut v,
    };
    let removed = rows[0].as_object_mut().unwrap().remove(key);
    assert!(removed.is_some(), "fixture lacks {key}");
    assert!(
        serde_json::from_value::<T>(v).is_err(),
        "parsed without required {key}"
    );
}

#[test]
fn markets_config_and_values() {
    let c: Vec<GmxMarketConfig> = parse("markets config", fixture!("markets_config.json"));
    assert_eq!(c.len(), 2);
    assert_eq!(c[0].name, "ASTER/USD [BTC-USDC]");
    assert!(c[1].is_spot_only);
    assert!(c[0].max_pnl_factor_for_deposits_long.is_some());
    assert_strict::<Vec<GmxMarketConfig>>(fixture!("markets_config.json"), None, "fundingFactor");

    let v: Vec<GmxMarketValues> = parse("markets values", fixture!("markets_values.json"));
    assert_eq!(v.len(), 2);
    assert_eq!(v[0].updated_at, Some(1794826492527));
    assert_eq!(v[1].updated_at, None);
    assert!(!v[0].funding_factor_per_second.is_empty());
    assert_strict::<Vec<GmxMarketValues>>(fixture!("markets_values.json"), None, "updatedAt");
    assert_strict::<Vec<GmxMarketValues>>(fixture!("markets_values.json"), None, "poolValueMax");
}

#[test]
fn risk_oracle_markets_and_solana_pairs() {
    let m: Vec<GmxRiskOracleMarket> = parse("risk oracle", fixture!("risk_oracle_markets.json"));
    assert_eq!(m.len(), 3);
    assert_eq!(m[0].name, "BTC/USD [WBTC-USDC]");
    assert_eq!(
        m[0].market_token,
        "0x47c031236e19d024b42f8AE6780E44A573170703"
    );
    assert_strict::<Vec<GmxRiskOracleMarket>>(
        fixture!("risk_oracle_markets.json"),
        None,
        "market_token",
    );

    let p: Vec<GmxPair> = parse("solana pairs", fixture!("pairs_solana.json"));
    assert_eq!(p.len(), 2);
    assert_eq!(p[0].ticker_id, "AAPL/USD[USDC-USDC]");
    assert_eq!(p[0].funding_interval_minutes, Some(60));
    assert_strict::<Vec<GmxPair>>(fixture!("pairs_solana.json"), None, "last_price");
}

#[test]
fn trades_search() {
    let t: GmxTradesList = parse("trades search", fixture!("trades_search.json"));
    assert!(t.has_more);
    assert_eq!(t.next_cursor.as_deref(), Some("100"));
    assert_eq!(t.trades.len(), 2);
    assert_eq!(t.trades[0].event_name, "OrderExecuted");
    assert!(t.trades[0].execution_price.is_some());
    // Cancelled rows omit the execution fields.
    assert_eq!(t.trades[1].event_name, "OrderCancelled");
    assert!(t.trades[1].execution_price.is_none());
    assert_eq!(t.trades[1].reason.as_deref(), Some("AUTO_CANCEL"));
    assert_strict::<GmxTradesList>(fixture!("trades_search.json"), Some("trades"), "orderKey");
    // `nextCursor` is nullable but required.
    let mut v: serde_json::Value = serde_json::from_str(fixture!("trades_search.json")).unwrap();
    v.as_object_mut().unwrap().remove("nextCursor").unwrap();
    assert!(serde_json::from_value::<GmxTradesList>(v).is_err());

    let req = GmxTradesSearchRequest {
        for_all_accounts: Some(true),
        markets_directions: Some(vec![GmxMarketDirectionFilter {
            market_address: "0x70d95587d40A2caf56bd97485aB3Eec10Bee6336".into(),
            direction: GmxTradeDirection::Long,
            collateral_address: None,
        }]),
        order_event_combinations: Some(vec![GmxOrderEventCombination {
            order_type: Some(2),
            event_name: Some("OrderExecuted".into()),
            ..Default::default()
        }]),
        from_timestamp: Some(1794800000),
        limit: Some(2),
        ..Default::default()
    };
    assert_eq!(
        serde_json::to_value(&req).unwrap(),
        serde_json::json!({
            "forAllAccounts": true,
            "fromTimestamp": 1794800000u64,
            "marketsDirections": [{
                "marketAddress": "0x70d95587d40A2caf56bd97485aB3Eec10Bee6336",
                "direction": "long"
            }],
            "orderEventCombinations": [{"orderType": 2, "eventName": "OrderExecuted"}],
            "limit": 2
        })
    );
    assert_eq!(
        serde_json::to_string(&GmxTradesSearchRequest::default()).unwrap(),
        "{}"
    );
}

#[test]
fn oracle_markets_tokens_state() {
    let m: GmxOracleMarkets = parse("oracle markets", fixture!("oracle_markets.json"));
    assert_eq!(m.markets[0].name, "ARB/USD [ARB-USDC]");
    assert!(m.markets[0].is_listed);
    assert_strict::<GmxOracleMarkets>(
        fixture!("oracle_markets.json"),
        Some("markets"),
        "listingDate",
    );

    let i: GmxOracleMarketsInfo =
        parse("oracle markets info", fixture!("oracle_markets_info.json"));
    assert_eq!(i.markets.len(), 2);
    assert!(i.markets[0].funding_rate_long.is_some());
    assert_eq!(i.markets[1].name, "SWAP-ONLY [USDC-USDC.e]");
    assert!(i.markets[1].net_rate_short.is_none());
    assert_strict::<GmxOracleMarketsInfo>(
        fixture!("oracle_markets_info.json"),
        Some("markets"),
        "openInterestLong",
    );

    let s: GmxOracleMarketsState = parse(
        "oracle markets state",
        fixture!("oracle_markets_state.json"),
    );
    assert_eq!(s.markets[0].status, GmxOracleMarketStatus::Open);
    assert!(!s.markets[0].is_market_closed);
    assert!(s.markets[0].closing_price.is_none());
    assert_strict::<GmxOracleMarketsState>(
        fixture!("oracle_markets_state.json"),
        Some("markets"),
        "closingPrice",
    );

    let t: GmxOracleTokens = parse("oracle tokens", fixture!("oracle_tokens.json"));
    assert_eq!(t.tokens[0].symbol, "APE");
    assert_eq!(t.tokens[0].synthetic, None);
    assert_eq!(t.tokens[1].synthetic, Some(true));
    assert_strict::<GmxOracleTokens>(fixture!("oracle_tokens.json"), Some("tokens"), "decimals");

    let p: GmxOraclePing = parse("oracle ping", fixture!("oracle_ping.json"));
    assert_eq!(p.message, "ok");
}

#[test]
fn oracle_signed_prices() {
    let s: GmxOracleSignedPrices = parse("signed prices", fixture!("oracle_signed_prices.json"));
    assert_eq!(s.signed_prices.len(), 2);
    let p = &s.signed_prices[0];
    assert_eq!(p.token_symbol, "USDG");
    assert_eq!(p.min_price_full, "999846215000000000000000");
    assert!(p.signature.is_none());
    assert!(p.is_valid);
    assert_eq!(p.feed_version, "v3");
    assert!(p.blob.starts_with("0x"));
    // Nullable fields must still be present.
    assert_strict::<GmxOracleSignedPrices>(
        fixture!("oracle_signed_prices.json"),
        Some("signedPrices"),
        "signature",
    );
    assert_strict::<GmxOracleSignedPrices>(
        fixture!("oracle_signed_prices.json"),
        Some("signedPrices"),
        "maxPriceFull",
    );
}
