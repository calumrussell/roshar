//! Offline parsing of live captures (tests/fixtures, recorded 2026-11-15 from
//! https://perps-api.jup.ag/v2).

use roshar_jupiter_perps::types::*;
use serde::de::DeserializeOwned;

fn fixture<T: DeserializeOwned>(name: &str) -> T {
    let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{name}: {e}"))
}

#[test]
fn market_stats() {
    let s: MarketStats = fixture("market_stats.json");
    assert_eq!(s.price, "71.93572611");
    assert!(!s.volume.is_empty());
}

#[test]
fn pool_info() {
    let p: PoolInfo = fixture("pool_info.json");
    assert_eq!(p.long_borrow_rate_percent, "0.0015");
    assert_eq!(p.max_request_execution_sec, "45");
}

#[test]
fn jlp_info() {
    let j: JlpInfo = fixture("jlp_info.json");
    assert!(!j.custodies.is_empty());
    let sol = j
        .custodies
        .iter()
        .find(|c| c.mint == SOL_MINT)
        .expect("SOL custody");
    assert_eq!(sol.symbol, "SOL");
    assert_eq!(sol.pubkey, "7xS2gz2bTp3fwCC7knJvUWTEU9Tycczu6VhJYKgi1wdz");
}

#[test]
fn top_traders_and_trader_stats() {
    let t: TopTraders = fixture("top_traders.json");
    assert_eq!(t.market_mint, SOL_MINT);
    assert!(!t.top_traders_by_pnl.is_empty());
    assert!(!t.top_traders_by_volume.is_empty());
    let s: TraderStats = fixture("trader_stats.json");
    assert_eq!(s.start_timestamp, t.start_timestamp);
}

#[test]
fn positions() {
    let p: DataList<Position> = fixture("positions.json");
    assert_eq!(p.count as usize, p.data_list.len());
    let short = p
        .data_list
        .iter()
        .find(|x| x.position_pubkey == "8rceJGmfxcpQLVSEdayQbvaKfN2gEkxZzYSFjyTKUKiH")
        .expect("short position");
    assert_eq!(short.side, Side::Short);
    assert_eq!(short.collateral_token, Token::Usdc);
    assert_eq!(short.tpsl_requests.len(), 2);
    assert_eq!(short.tpsl_requests[0].request_type, TpslRequestType::Tp);
    assert_eq!(
        short.tpsl_requests[0].trigger_price_usd.as_deref(),
        Some("67770000")
    );

    let closed: DataList<Position> = fixture("positions_closed.json");
    assert!(!closed.data_list.is_empty());
}

#[test]
fn limit_orders() {
    let o: DataList<LimitOrder> = fixture("orders_limit.json");
    assert_eq!(o.count, 2);
    assert_eq!(o.data_list[0].market_mint, BTC_MINT);
    assert_eq!(o.data_list[0].trigger_price.as_deref(), Some("58208000000"));
    assert!(!o.data_list[0].executed);
}

#[test]
fn expired_orders_and_close_all() {
    let e: SerializedTxBatch = fixture("orders_expired.json");
    assert!(e.serialized_txs.is_empty());
    let c: SerializedTxBatch = fixture("close_all.json");
    assert_eq!(c.serialized_txs.len(), 2);
    assert!(!c.tx_metadata.blockhash.is_empty());
}

#[test]
fn trades() {
    let t: DataList<Trade> = fixture("trades.json");
    assert_eq!(t.data_list.len(), 5);
    let liq = &t.data_list[0];
    assert_eq!(liq.order_type, TradeOrderType::Liquidation);
    assert_eq!(liq.action, TradeAction::Decrease);
    assert_eq!(liq.position_name, PositionName::SolPerp);
    assert_eq!(liq.transaction_fee_lamports, None);
    let market = &t.data_list[1];
    assert_eq!(market.order_type, TradeOrderType::Market);
    assert_eq!(market.transaction_fee_lamports.as_deref(), Some("20747"));
}

#[test]
fn collateral_limits() {
    let c: CollateralLimits = fixture("collateral_limits.json");
    assert_eq!(c.min_collateral_deposit_usd, "235601");
}

#[test]
fn increase_position() {
    let r: IncreasePositionResponse = fixture("increase_position.json");
    assert!(r.position_pubkey.is_some());
    assert!(r.serialized_tx_base64.is_some());
    assert!(r.tx_metadata.is_some());
    assert_eq!(r.tpsl.len(), 1);
    assert_eq!(r.tpsl[0].request_type, TpslRequestType::Tp);
    assert_eq!(r.quote.side, Side::Long);

    let q: IncreasePositionResponse = fixture("increase_position_quote.json");
    assert!(q.position_pubkey.is_none());
    assert!(q.serialized_tx_base64.is_none());
    assert!(q.tx_metadata.is_none());
    assert!(q.tpsl.is_empty());
}

#[test]
fn decrease_position() {
    let r: DecreasePositionResponse = fixture("decrease_position.json");
    assert_eq!(
        r.position_pubkey,
        "8rceJGmfxcpQLVSEdayQbvaKfN2gEkxZzYSFjyTKUKiH"
    );
    assert_eq!(r.quote.side, Side::Short);
    assert_eq!(r.quote.transfer_token_mint, USDC_MINT);
}

#[test]
fn limit_order_lifecycle() {
    for name in [
        "create_limit_order.json",
        "create_limit_order_2.json",
        "create_limit_order_3.json",
    ] {
        let r: CreateLimitOrderResponse = fixture(name);
        assert!(r.serialized_tx_base64.is_some(), "{name}");
        assert!(r.tx_metadata.is_some(), "{name}");
    }
    let q: CreateLimitOrderResponse = fixture("create_limit_order_quote.json");
    assert!(q.serialized_tx_base64.is_none());

    let u: UpdateLimitOrderResponse = fixture("update_limit_order.json");
    assert_eq!(
        u.position_request_pubkey.as_deref(),
        Some("EHEUAgMxWB2WpHDDfT1ZNGVX96z4XN2GtzPH2ZdwQerq")
    );
    let c: CancelLimitOrderResponse = fixture("cancel_limit_order.json");
    assert_eq!(c.require_keeper_signature, Some(false));
    assert!(c.serialized_tx_base64.is_some());
}

#[test]
fn tpsl_lifecycle() {
    let c: CreateTpslResponse = fixture("create_tpsl.json");
    assert_eq!(c.tpsl_pubkeys.len(), 1);
    assert!(c.require_keeper_signature);
    assert_eq!(c.transaction_type, TransactionType::Instant);
    assert_eq!(
        c.tpsl_requests[0].position_request_pubkey,
        c.tpsl_pubkeys[0]
    );

    let u: UpdateTpslResponse = fixture("update_tpsl.json");
    assert_eq!(u.transaction_type, TransactionType::Instant);

    let d: CancelTpslResponse = fixture("cancel_tpsl.json");
    assert!(!d.require_keeper_signature);
    assert_eq!(d.transaction_type, TransactionType::Legacy);
}

#[test]
fn error_bodies() {
    let e: ErrorBody = fixture("error_insufficient_funds.json");
    assert_eq!(e.code, "insufficient_funds");
    assert!(e.metadata.is_none(), "metadata: null");

    let e: ErrorBody = fixture("error_invalid_sl_price.json");
    assert_eq!(e.code, "invalid_sl_price");
    let metadata = e.metadata.expect("metadata");
    assert!(metadata.get("minTriggerPriceUsd").is_some());

    let e: ErrorBody = fixture("error_invalid_argument.json");
    assert_eq!(e.code, "invalid_argument");

    let e: ErrorBody = fixture("error_internal.json");
    assert_eq!(e.code, "internal_server_error");
    assert!(e.metadata.is_none());
}

#[test]
fn request_bodies_serialise_as_documented() {
    let req = IncreasePositionRequest {
        asset: Asset::Sol,
        input_token: Token::Usdc,
        input_token_amount: "10000000".into(),
        leverage: Some("2".into()),
        max_slippage_bps: "200".into(),
        priority_fee_micro_lamports: None,
        side: Side::Long,
        size_usd_delta: None,
        tpsl: Some(vec![IncreaseTpsl {
            receive_token: Token::Usdc,
            trigger_price: "90000000".into(),
            request_type: TpslRequestType::Tp,
        }]),
        wallet_address: Some("9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM".into()),
    };
    assert_eq!(
        serde_json::to_value(&req).unwrap(),
        serde_json::json!({
            "asset": "SOL", "inputToken": "USDC", "inputTokenAmount": "10000000",
            "leverage": "2", "maxSlippageBps": "200", "side": "long",
            "tpsl": [{"receiveToken": "USDC", "triggerPrice": "90000000", "requestType": "tp"}],
            "walletAddress": "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM"
        })
    );

    let exec = ExecuteRequest {
        action: ExecuteAction::IncreasePositionWithFee,
        serialized_tx_base64: "AQ==".into(),
    };
    assert_eq!(
        serde_json::to_value(&exec).unwrap(),
        serde_json::json!({"action": "increase-position-with-fee", "serializedTxBase64": "AQ=="})
    );

    let tpsl = CreateTpslRequest {
        wallet_address: "w".into(),
        position_pubkey: "p".into(),
        tpsl: vec![TpslSpec {
            receive_token: Token::Usdc,
            entire_position: Some(false),
            size_usd_delta: Some("20000000".into()),
            trigger_price: "60000000".into(),
            request_type: TpslRequestType::Sl,
        }],
        transaction_type: Some(TpslTransactionTypeRequest::Mixed),
    };
    assert_eq!(
        serde_json::to_value(&tpsl).unwrap(),
        serde_json::json!({
            "walletAddress": "w", "positionPubkey": "p", "transactionType": "mixed",
            "tpsl": [{"receiveToken": "USDC", "entirePosition": false, "sizeUsdDelta": "20000000",
                      "triggerPrice": "60000000", "requestType": "sl"}]
        })
    );
}

#[test]
fn asset_mints() {
    assert_eq!(Asset::Sol.mint(), SOL_MINT);
    assert_eq!(Asset::Eth.mint(), ETH_MINT);
    assert_eq!(Asset::Btc.mint(), BTC_MINT);
}
