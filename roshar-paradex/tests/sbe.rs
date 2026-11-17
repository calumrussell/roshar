//! SBE decoder tests against frames captured from prod
//! (`wss://ws.api.prod.paradex.trade/v1?sbeSchemaId=1&sbeSchemaVersion=3`,
//! plus versions 1 and 2 in `older_versions.hex`) and synthetic private
//! frames encoded per the schema. Expected values in `*.expected.json` are the
//! paradex-py 62c0c31 decoder's `model_dump()` of the same bytes.
//!
//! Representation mapping, Rust → paradex-py:
//! - timestamps: Rust keeps μs; the SDK floors to ms (`us // 1000`).
//! - `Decimal` `Display` == SDK `_f8`/`_f12` strings; null → `None`/null.
//! - enums: SDK name strings; `NonRepresentable` → null. AssetKind and
//!   FundingRateSource carry the SDK's `ASSET_KIND_`/`SOURCE_` prefixes.
//! - `AccountAddress` `Display` == SDK `_addr`.
//! - OrderEvent request fields → SDK `request_info` (null when both enums are
//!   `Unspecified` or absent).
//! - PositionEvent: Rust keeps the absolute size and `Side`; the SDK signs the
//!   size (negative when `Sell` and non-zero) and names the side LONG/SHORT.

use roshar_paradex::sbe::{
    decode, decode_header, AccountAddress, AccountStatus, AssetKind, BookLevel, Decimal, FillFlags,
    FillType, FundingRateSource, LiquiditySide, OrderFlags, OrderStatus, OrderType,
    ParadexSbeMessage, PkgType, PositionStatus, RequestStatus, RequestType, Side, StpMode,
    TimeInForce,
};
use serde_json::{json, Value};

fn hex(s: &str) -> Vec<u8> {
    let s = s.trim();
    assert!(s.len().is_multiple_of(2), "odd hex length");
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("hex digit"))
        .collect()
}

fn frames(file: &str) -> Vec<Vec<u8>> {
    file.lines()
        .filter(|l| !l.trim().is_empty())
        .map(hex)
        .collect()
}

fn d(x: &Decimal) -> Value {
    Value::String(x.to_string())
}

fn od(x: &Option<Decimal>) -> Value {
    x.as_ref().map(d).unwrap_or(Value::Null)
}

fn ms(us: i64) -> Value {
    json!(us.div_euclid(1000))
}

fn side(x: Side) -> Value {
    match x {
        Side::Buy => json!("BUY"),
        Side::Sell => json!("SELL"),
        Side::NonRepresentable => Value::Null,
    }
}

fn fill_type(x: FillType) -> Value {
    match x {
        FillType::Fill => json!("FILL"),
        FillType::Liquidation => json!("LIQUIDATION"),
        FillType::UnwindTransfer => json!("UNWIND_TRANSFER"),
        FillType::SettleMarket => json!("SETTLE_MARKET"),
        FillType::Rpi => json!("RPI"),
        FillType::BlockTrade => json!("BLOCK_TRADE"),
        FillType::NonRepresentable => Value::Null,
    }
}

fn order_type(x: OrderType) -> Value {
    match x {
        OrderType::Limit => json!("LIMIT"),
        OrderType::Market => json!("MARKET"),
        OrderType::StopLimit => json!("STOP_LIMIT"),
        OrderType::StopMarket => json!("STOP_MARKET"),
        OrderType::TakeProfitLimit => json!("TAKE_PROFIT_LIMIT"),
        OrderType::TakeProfitMarket => json!("TAKE_PROFIT_MARKET"),
        OrderType::StopLossLimit => json!("STOP_LOSS_LIMIT"),
        OrderType::StopLossMarket => json!("STOP_LOSS_MARKET"),
        OrderType::NonRepresentable => Value::Null,
    }
}

fn order_status(x: OrderStatus) -> Value {
    match x {
        OrderStatus::New => json!("NEW"),
        OrderStatus::Untriggered => json!("UNTRIGGERED"),
        OrderStatus::Open => json!("OPEN"),
        OrderStatus::Closed => json!("CLOSED"),
        OrderStatus::NonRepresentable => Value::Null,
    }
}

fn tif(x: TimeInForce) -> Value {
    match x {
        TimeInForce::Gtc => json!("GTC"),
        TimeInForce::Ioc => json!("IOC"),
        TimeInForce::PostOnly => json!("POST_ONLY"),
        TimeInForce::Rpi => json!("RPI"),
        TimeInForce::NonRepresentable => Value::Null,
    }
}

fn stp(x: StpMode) -> Value {
    match x {
        StpMode::None => json!("NONE"),
        StpMode::ExpireMaker => json!("EXPIRE_MAKER"),
        StpMode::ExpireTaker => json!("EXPIRE_TAKER"),
        StpMode::ExpireBoth => json!("EXPIRE_BOTH"),
        StpMode::NonRepresentable => Value::Null,
    }
}

fn liquidity(x: LiquiditySide) -> Value {
    match x {
        LiquiditySide::Maker => json!("MAKER"),
        LiquiditySide::Taker => json!("TAKER"),
        LiquiditySide::NonRepresentable => Value::Null,
    }
}

fn pkg_type(x: PkgType) -> Value {
    match x {
        PkgType::Snapshot => json!("SNAPSHOT"),
        PkgType::Delta => json!("DELTA"),
    }
}

fn asset_kind(x: AssetKind) -> Value {
    match x {
        AssetKind::Unspecified => json!("ASSET_KIND_UNSPECIFIED"),
        AssetKind::PerpetualFuture => json!("ASSET_KIND_PERPETUAL_FUTURE"),
        AssetKind::Option => json!("ASSET_KIND_OPTION"),
        AssetKind::Spot => json!("ASSET_KIND_SPOT"),
    }
}

fn source(x: FundingRateSource) -> Value {
    let name = match x {
        FundingRateSource::Unspecified => "UNSPECIFIED",
        FundingRateSource::Paradex => "PARADEX",
        FundingRateSource::Binance => "BINANCE",
        FundingRateSource::Bybit => "BYBIT",
        FundingRateSource::Okx => "OKX",
        FundingRateSource::Deribit => "DERIBIT",
        FundingRateSource::Derive => "DERIVE",
        FundingRateSource::Hyperliquid => "HYPERLIQUID",
        FundingRateSource::Lighter => "LIGHTER",
        FundingRateSource::Databento => "DATABENTO",
        FundingRateSource::Publisher => "PUBLISHER",
        FundingRateSource::Pyth => "PYTH",
        FundingRateSource::Stork => "STORK",
    };
    json!(format!("SOURCE_{}", name))
}

fn position_status(x: PositionStatus) -> Value {
    match x {
        PositionStatus::Open => json!("OPEN"),
        PositionStatus::Closed => json!("CLOSED"),
        PositionStatus::Unspecified => json!("UNSPECIFIED"),
        PositionStatus::NonRepresentable => Value::Null,
    }
}

fn account_status(x: AccountStatus) -> Value {
    match x {
        AccountStatus::Active => json!("ACTIVE"),
        AccountStatus::Liquidation => json!("LIQUIDATION"),
        AccountStatus::NonRepresentable => Value::Null,
    }
}

fn request_status(x: RequestStatus) -> Value {
    match x {
        RequestStatus::Unspecified => json!("UNSPECIFIED"),
        RequestStatus::Pending => json!("PENDING"),
        RequestStatus::Processed => json!("PROCESSED"),
        RequestStatus::Rejected => json!("REJECTED"),
        RequestStatus::Success => json!("SUCCESS"),
        RequestStatus::NonRepresentable => Value::Null,
    }
}

fn request_type(x: RequestType) -> Value {
    match x {
        RequestType::Unspecified => json!("UNSPECIFIED"),
        RequestType::ModifyOrder => json!("MODIFY_ORDER"),
        RequestType::NonRepresentable => Value::Null,
    }
}

fn order_flags(f: OrderFlags) -> Value {
    let mut out = Vec::new();
    for (set, name) in [
        (f.reduce_only, "REDUCE_ONLY"),
        (
            f.stop_condition_below_trigger,
            "STOP_CONDITION_BELOW_TRIGGER",
        ),
        (
            f.stop_condition_above_trigger,
            "STOP_CONDITION_ABOVE_TRIGGER",
        ),
        (f.interactive, "INTERACTIVE"),
        (f.target_strategy_vwap, "TARGET_STRATEGY_VWAP"),
        (f.mmp, "MMP"),
    ] {
        if set {
            out.push(json!(name));
        }
    }
    Value::Array(out)
}

fn fill_flags(f: FillFlags) -> Value {
    let mut out = Vec::new();
    for (set, name) in [
        (f.interactive, "interactive"),
        (f.rpi, "rpi"),
        (f.fastfill, "fastfill"),
        (f.unknown, "unknown"),
    ] {
        if set {
            out.push(json!(name));
        }
    }
    Value::Array(out)
}

fn levels(levels: &[BookLevel]) -> Value {
    Value::Array(
        levels
            .iter()
            .map(|l| json!([l.price.to_string(), l.size.to_string()]))
            .collect(),
    )
}

fn addr(a: &AccountAddress) -> Value {
    json!(a.to_string())
}

/// Render a decoded message in paradex-py `model_dump()` shape.
fn sdk_view(msg: &ParadexSbeMessage) -> Value {
    match msg {
        ParadexSbeMessage::Trade(m) => json!({
            "timestamp": ms(m.ts),
            "seq_no": m.seq,
            "trade_id": m.trade_id,
            "side": side(m.side),
            "price": d(&m.price),
            "size": d(&m.size),
            "created_at": ms(m.created_at),
            "trade_type": fill_type(m.trade_type),
            "market": m.market,
            "trade_id_str": m.trade_id_str,
        }),
        ParadexSbeMessage::Bbo(m) => json!({
            "timestamp": ms(m.ts),
            "seq_no": m.seq,
            "bid_price": od(&m.bid_price),
            "bid_size": od(&m.bid_size),
            "ask_price": od(&m.ask_price),
            "ask_size": od(&m.ask_size),
            "market": m.market,
        }),
        ParadexSbeMessage::Book(m) => json!({
            "timestamp": ms(m.ts),
            "seq_no": m.seq,
            "pkg_type": pkg_type(m.pkg_type),
            "best_bid_price": od(&m.best_bid_price),
            "best_bid_size": od(&m.best_bid_size),
            "best_ask_price": od(&m.best_ask_price),
            "best_ask_size": od(&m.best_ask_size),
            "relative_spread": od(&m.relative_spread),
            "best_bid_api_price": od(&m.best_bid_api_price),
            "best_bid_api_size": od(&m.best_bid_api_size),
            "best_ask_api_price": od(&m.best_ask_api_price),
            "best_ask_api_size": od(&m.best_ask_api_size),
            "bids": levels(&m.bids),
            "asks": levels(&m.asks),
            "market": m.market,
        }),
        ParadexSbeMessage::MarketSummary(m) => json!({
            "timestamp": ms(m.ts),
            "seq_no": m.seq,
            "mark_price": d(&m.mark_price),
            "index_price": d(&m.index_price),
            "last_price": d(&m.last_price),
            "volume_usd24h": d(&m.volume_usd_24h),
            "open_interest": d(&m.open_interest),
            "funding_rate": d(&m.funding_rate),
            "bid": od(&m.bid),
            "ask": od(&m.ask),
            "total_volume": d(&m.total_volume),
            "price_change_rate24h": d(&m.price_change_rate_24h),
            "mark_iv": od(&m.mark_iv),
            "bid_iv": od(&m.bid_iv),
            "ask_iv": od(&m.ask_iv),
            "last_iv": od(&m.last_iv),
            "delta": od(&m.delta),
            "gamma": od(&m.gamma),
            "vega": od(&m.vega),
            "theta": od(&m.theta),
            "rho": od(&m.rho),
            "volga": od(&m.volga),
            "vanna": od(&m.vanna),
            "future_funding_rate": od(&m.future_funding_rate),
            "external_fair_price": od(&m.external_fair_price),
            "bid_size": od(&m.bid_size),
            "ask_size": od(&m.ask_size),
            "forward_rate": od(&m.forward_rate),
            "risk_free_rate": od(&m.risk_free_rate),
            "funding_rate_precise": od(&m.funding_rate_precise),
            "settlement_twap_estimate": od(&m.settlement_twap_estimate),
            "market": m.market,
        }),
        ParadexSbeMessage::FundingData(m) => json!({
            "timestamp": ms(m.ts),
            "seq_no": m.seq,
            "funding_rate": d(&m.funding_rate),
            "funding_index": d(&m.funding_index),
            "created_at": ms(m.created_at),
            "funding_premium": d(&m.funding_premium),
            "funding_rate8h": d(&m.funding_rate_8h),
            "funding_period_hours": m.funding_period_hours,
            "impact_premium_rate": od(&m.impact_premium_rate),
            "market": m.market,
        }),
        ParadexSbeMessage::FundingRateComparison(m) => json!({
            "timestamp": ms(m.ts),
            "last_updated_at": ms(m.last_updated_at),
            "hourly_funding_rate": d(&m.hourly_funding_rate),
            "asset_kind": asset_kind(m.asset_kind),
            "source": source(m.source),
            "market": m.market,
        }),
        ParadexSbeMessage::Order(m) => {
            let request_info = match (m.request_status, m.request_type) {
                (Some(RequestStatus::Unspecified), Some(RequestType::Unspecified)) => Value::Null,
                (Some(s), Some(t)) => json!({
                    "id": m.request_id.clone().unwrap_or_default(),
                    "message": m.request_message.clone().unwrap_or_default(),
                    "request_type": request_type(t),
                    "status": request_status(s),
                }),
                _ => Value::Null,
            };
            json!({
                "timestamp": ms(m.ts),
                "seq_no": m.seq,
                "status": order_status(m.status),
                "side": side(m.side),
                "order_type": order_type(m.order_type),
                "time_in_force": tif(m.time_in_force),
                "price": od(&m.price),
                "trigger_price": od(&m.trigger_price),
                "size": d(&m.size),
                "size_open": d(&m.size_open),
                "avg_fill_price": od(&m.avg_fill_price),
                "created_at": ms(m.created_at),
                "updated_at": ms(m.updated_at),
                "account": addr(&m.account),
                "received_at": ms(m.received_at),
                "published_at": ms(m.published_at),
                "stp": stp(m.stp),
                "flags": order_flags(m.flags),
                "order_id": m.order_id,
                "client_order_id": m.client_order_id,
                "market": m.market,
                "cancel_reason": m.cancel_reason,
                "request_info": request_info,
            })
        }
        ParadexSbeMessage::Fill(m) => json!({
            "timestamp": ms(m.ts),
            "seq_no": m.seq,
            "fill_type": fill_type(m.fill_type),
            "side": side(m.side),
            "liquidity": liquidity(m.liquidity),
            "price": d(&m.price),
            "size": d(&m.size),
            "fee": d(&m.fee),
            "realized_pnl": od(&m.realized_pnl),
            "created_at": ms(m.created_at),
            "account": addr(&m.account),
            "underlying_price": d(&m.underlying_price),
            "realized_funding": d(&m.realized_funding),
            "flags": m.flags.map(fill_flags).unwrap_or(Value::Null),
            "orderbook_seq_no": m.orderbook_seq_no,
            "fill_id": m.fill_id,
            "order_id": m.order_id,
            "client_order_id": m.client_order_id,
            "trade_id": m.trade_id,
            "market": m.market,
            "fee_currency": m.fee_currency,
        }),
        ParadexSbeMessage::Position(m) => {
            let signed_size = if m.side == Side::Sell && m.size.mantissa > 0 {
                Decimal {
                    mantissa: -m.size.mantissa,
                    exponent: m.size.exponent,
                }
            } else {
                m.size
            };
            let side = match m.side {
                Side::Buy => json!("LONG"),
                Side::Sell => json!("SHORT"),
                Side::NonRepresentable => Value::Null,
            };
            json!({
                "timestamp": ms(m.ts),
                "seq_no": m.seq,
                "side": side,
                "size": d(&signed_size),
                "avg_entry_price": d(&m.avg_entry_price),
                "unrealized_pnl": d(&m.unrealized_pnl),
                "realized_pnl": d(&m.realized_pnl),
                "mark_price": od(&m.mark_price),
                "liquidation_price": od(&m.liquidation_price),
                "leverage": d(&m.leverage),
                "updated_at": ms(m.updated_at),
                "account": addr(&m.account),
                "avg_entry_price_usd": d(&m.avg_entry_price_usd),
                "cost": d(&m.cost),
                "cost_usd": d(&m.cost_usd),
                "cached_funding_index": d(&m.cached_funding_index),
                "unrealized_funding_pnl": d(&m.unrealized_funding_pnl),
                "status": position_status(m.status),
                "market": m.market,
                "last_fill_id": m.last_fill_id,
            })
        }
        ParadexSbeMessage::Account(m) => json!({
            "timestamp": ms(m.ts),
            "seq_no": m.seq,
            "total_collateral": d(&m.total_collateral),
            "free_collateral": d(&m.free_collateral),
            "initial_margin_req": d(&m.initial_margin_req),
            "maintenance_margin_req": d(&m.maintenance_margin_req),
            "account_value": d(&m.account_value),
            "unrealized_pnl": od(&m.unrealized_pnl),
            "updated_at": ms(m.updated_at),
            "account": addr(&m.account),
            "margin_cushion": d(&m.margin_cushion),
            "status": account_status(m.status),
            "last_seen_notification": m.last_seen_notification.map(|t| t.div_euclid(1000)),
            "settlement_asset": m.settlement_asset,
        }),
        ParadexSbeMessage::Heartbeat(_) | ParadexSbeMessage::Subscribed(_) => {
            panic!("paradex-py discards heartbeat/subscribed frames; no SDK view")
        }
    }
}

fn check_fixture(name: &str, hex_file: &str, expected_file: &str, template: u16) {
    let frames = frames(hex_file);
    let expected: Vec<Value> = serde_json::from_str(expected_file).expect("expected json");
    assert_eq!(frames.len(), expected.len(), "{name}: fixture count");
    assert!(!frames.is_empty(), "{name}: empty fixture");
    for (i, (frame, exp)) in frames.iter().zip(expected.iter()).enumerate() {
        let header = decode_header(frame).expect("header");
        assert_eq!(header.template_id, template, "{name}[{i}] template");
        assert_eq!(
            json!({
                "block_length": header.block_length,
                "template_id": header.template_id,
                "schema_id": header.schema_id,
                "version": header.version,
            }),
            exp["header"],
            "{name}[{i}] header"
        );
        let msg = decode(frame).unwrap_or_else(|e| panic!("{name}[{i}] decode: {e:#}"));
        assert_eq!(sdk_view(&msg), exp["data"], "{name}[{i}] data");
    }
}

macro_rules! fixture_test {
    ($test:ident, $file:literal, $template:expr) => {
        #[test]
        fn $test() {
            check_fixture(
                $file,
                include_str!(concat!("fixtures/", $file, ".hex")),
                include_str!(concat!("fixtures/", $file, ".expected.json")),
                $template,
            );
        }
    };
}

fixture_test!(trades, "trades", 1);
fixture_test!(bbo, "bbo", 2);
fixture_test!(order_book_deltas, "order_book_deltas", 3);
fixture_test!(order_book_snapshot, "order_book_snapshot", 3);
fixture_test!(order_book_snapshot_tick, "order_book_snapshot_tick", 3);
fixture_test!(order_book_interactive, "order_book_interactive", 3);
fixture_test!(
    order_book_interactive_deltas,
    "order_book_interactive_deltas",
    3
);
fixture_test!(markets_summary, "markets_summary", 4);
fixture_test!(funding_data, "funding_data", 5);
fixture_test!(funding_rate_comparison, "funding_rate_comparison", 6);

#[test]
fn older_versions() {
    let frames = frames(include_str!("fixtures/older_versions.hex"));
    let expected: Vec<Value> =
        serde_json::from_str(include_str!("fixtures/older_versions.expected.json")).unwrap();
    assert_eq!(frames.len(), expected.len());
    for (i, (frame, exp)) in frames.iter().zip(expected.iter()).enumerate() {
        let header = decode_header(frame).unwrap();
        assert_eq!(json!(header.version), exp["header"]["version"], "[{i}]");
        assert_eq!(
            json!(header.block_length),
            exp["header"]["block_length"],
            "[{i}]"
        );
        let msg = decode(frame).unwrap_or_else(|e| panic!("[{i}] decode: {e:#}"));
        assert_eq!(sdk_view(&msg), exp["data"], "older_versions[{i}]");
        if let ParadexSbeMessage::MarketSummary(m) = &msg {
            // 240-byte block (versions 1 and 2) predates settlementTwapEstimate.
            assert_eq!(m.settlement_twap_estimate, None);
            assert!(m.funding_rate_precise.is_some());
        }
    }
}

#[test]
fn private_synthetic() {
    let frames = frames(include_str!("fixtures/private_synthetic.hex"));
    let expected: Vec<Value> =
        serde_json::from_str(include_str!("fixtures/private_synthetic.expected.json")).unwrap();
    assert_eq!(frames.len(), expected.len());
    for (i, (frame, exp)) in frames.iter().zip(expected.iter()).enumerate() {
        let msg = decode(frame).unwrap_or_else(|e| panic!("[{i}] decode: {e:#}"));
        assert_eq!(sdk_view(&msg), exp["data"], "private_synthetic[{i}]");
    }
}

fn all_fixture_frames() -> Vec<Vec<u8>> {
    [
        include_str!("fixtures/trades.hex"),
        include_str!("fixtures/bbo.hex"),
        include_str!("fixtures/order_book_deltas.hex"),
        include_str!("fixtures/order_book_snapshot.hex"),
        include_str!("fixtures/order_book_snapshot_tick.hex"),
        include_str!("fixtures/order_book_interactive.hex"),
        include_str!("fixtures/order_book_interactive_deltas.hex"),
        include_str!("fixtures/markets_summary.hex"),
        include_str!("fixtures/funding_data.hex"),
        include_str!("fixtures/funding_rate_comparison.hex"),
        include_str!("fixtures/older_versions.hex"),
        include_str!("fixtures/private_synthetic.hex"),
    ]
    .iter()
    .flat_map(|f| frames(f))
    .collect()
}

#[test]
fn truncated_frames_error() {
    for frame in all_fixture_frames() {
        let full = decode(&frame).unwrap();
        let template = decode_header(&frame).unwrap().template_id;
        for len in 0..frame.len() {
            match decode(&frame[..len]) {
                Err(_) => {}
                // Var-data appended in schema version 2 (TradeEvent.tradeIdStr,
                // OrderEvent.requestId/requestMessage, FillEvent.feeCurrency)
                // is read as absent when the frame ends before it, as the
                // schema prescribes; such a prefix decodes to a different
                // message rather than an error.
                Ok(msg) => {
                    assert!(
                        matches!(template, 1 | 20 | 21),
                        "prefix of {} / {} bytes of template {} decoded",
                        len,
                        frame.len(),
                        template
                    );
                    assert_ne!(msg, full, "prefix of {} bytes decoded identically", len);
                }
            }
        }
    }
}

#[test]
fn trailing_bytes_error_on_known_version() {
    let mut frame = frames(include_str!("fixtures/trades.hex")).remove(0);
    frame.push(0);
    assert!(decode(&frame).is_err());
}

#[test]
fn unknown_template_and_schema_error() {
    let frame = frames(include_str!("fixtures/bbo.hex")).remove(0);
    let mut bad = frame.clone();
    bad[2..4].copy_from_slice(&99u16.to_le_bytes());
    let err = decode(&bad).unwrap_err().to_string();
    assert!(err.contains("templateId 99"), "{err}");

    let mut bad = frame;
    bad[4..6].copy_from_slice(&2u16.to_le_bytes());
    let err = decode(&bad).unwrap_err().to_string();
    assert!(err.contains("schemaId 2"), "{err}");
}

#[test]
fn unknown_enum_value_errors() {
    let mut frame = frames(include_str!("fixtures/trades.hex")).remove(0);
    // TradeEvent.side at root offset 24.
    frame[8 + 24] = 9;
    assert!(decode(&frame).is_err());
}

#[test]
fn null_in_required_field_errors() {
    let mut frame = frames(include_str!("fixtures/trades.hex")).remove(0);
    // TradeEvent.price at root offset 25.
    frame[8 + 25..8 + 33].copy_from_slice(&i64::MIN.to_le_bytes());
    assert!(decode(&frame).is_err());
}

#[test]
fn newer_version_with_longer_block_decodes() {
    // A future version appending 8 bytes to the root block and var-data after
    // the known fields: the header blockLength skips the unknown field.
    let original = frames(include_str!("fixtures/markets_summary.hex")).remove(0);
    let header = decode_header(&original).unwrap();
    let block_end = 8 + header.block_length as usize;
    let mut frame = Vec::new();
    frame.extend_from_slice(&(header.block_length + 8).to_le_bytes());
    frame.extend_from_slice(&original[2..6]);
    frame.extend_from_slice(&(header.version + 1).to_le_bytes());
    frame.extend_from_slice(&original[8..block_end]);
    frame.extend_from_slice(&[0xAB; 8]);
    frame.extend_from_slice(&original[block_end..]);
    frame.extend_from_slice(&[3, b'x', b'y', b'z']);
    assert_eq!(decode(&frame).unwrap(), decode(&original).unwrap());
}

#[test]
fn heartbeat_and_subscribed() {
    // Synthetic frames per schema: paradex-py discards these templates and no
    // live frame was observed in captures.
    let mut hb = Vec::new();
    for v in [16u16, 40, 1, 3] {
        hb.extend_from_slice(&v.to_le_bytes());
    }
    hb.extend_from_slice(&1_794_780_123_456_789i64.to_le_bytes());
    hb.extend_from_slice(&7i64.to_le_bytes());
    match decode(&hb).unwrap() {
        ParadexSbeMessage::Heartbeat(h) => {
            assert_eq!(h.ts, 1_794_780_123_456_789);
            assert_eq!(h.seq, 7);
        }
        other => panic!("{other:?}"),
    }

    let mut sub = Vec::new();
    for v in [17u16, 41, 1, 3] {
        sub.extend_from_slice(&v.to_le_bytes());
    }
    sub.extend_from_slice(&5i64.to_le_bytes());
    sub.extend_from_slice(&6i64.to_le_bytes());
    sub.push(0);
    let channel = b"trades.BTC-USD-PERP";
    sub.push(channel.len() as u8);
    sub.extend_from_slice(channel);
    match decode(&sub).unwrap() {
        ParadexSbeMessage::Subscribed(s) => {
            assert_eq!((s.ts, s.seq, s.status), (5, 6, 0));
            assert_eq!(s.channel, "trades.BTC-USD-PERP");
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn decimal_display_is_exact() {
    let cases = [
        (420_005_000_000_000i64, -8i8, "4200050.00000000"),
        (-1_230_000, -8, "-0.01230000"),
        (0, -8, "0.00000000"),
        (i64::MAX, -8, "92233720368.54775807"),
        (30_320_000, -12, "0.000030320000"),
        (-1, -12, "-0.000000000001"),
    ];
    for (mantissa, exponent, want) in cases {
        assert_eq!(Decimal { mantissa, exponent }.to_string(), want);
    }
}

#[test]
fn account_address_display() {
    assert_eq!(AccountAddress([0; 32]).to_string(), "0x0");
    let mut a = [0u8; 32];
    a[31] = 0x0a;
    a[30] = 0x01;
    assert_eq!(AccountAddress(a).to_string(), "0x10a");
}
