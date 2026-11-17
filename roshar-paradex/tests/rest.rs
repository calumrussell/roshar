//! REST responses captured from `https://api.prod.paradex.trade/v1` on 2026-11-16.

use roshar_paradex::types::{
    ParadexBbo, ParadexExchangeStats, ParadexImpactPrice, ParadexInsuranceFund,
    ParadexInteractiveOrderBook, ParadexMarginConfig, ParadexMarketEvent, ParadexSettlementPrice,
    ParadexSystemConfig, ParadexSystemState, ParadexSystemStatus, ParadexSystemTime,
};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
struct Results<T> {
    results: Vec<T>,
}

fn parse<T: DeserializeOwned>(text: &str) -> T {
    serde_json::from_str(text).unwrap_or_else(|e| panic!("{e}"))
}

/// Parses `text` with `pointer`'s field removed; must fail.
fn assert_required<T: DeserializeOwned>(text: &str, pointer: &str, field: &str) {
    let mut v: Value = serde_json::from_str(text).unwrap();
    v.pointer_mut(pointer)
        .and_then(Value::as_object_mut)
        .unwrap()
        .remove(field)
        .unwrap_or_else(|| panic!("{field} not in fixture"));
    assert!(
        serde_json::from_value::<T>(v).is_err(),
        "missing {field} accepted"
    );
}

#[test]
fn bbo() {
    let text = include_str!("fixtures/rest_bbo.json");
    let b: ParadexBbo = parse(text);
    assert_eq!(b.market, "BTC-USD-PERP");
    assert_eq!(b.seq_no, 7725218008);
    assert_eq!((b.bid.as_str(), b.ask.as_str()), ("64387.9", "64456.3"));
    assert_eq!(b.last_updated_at, 1794826506217);

    let i: ParadexBbo = parse(include_str!("fixtures/rest_bbo_interactive.json"));
    assert_eq!(i.ask_size, "0.024");

    let empty: ParadexBbo = parse(include_str!("fixtures/rest_bbo_empty.json"));
    assert_eq!((empty.seq_no, empty.bid.as_str()), (0, "0"));

    for f in ["market", "seq_no", "ask", "bid_size", "last_updated_at"] {
        assert_required::<ParadexBbo>(text, "", f);
    }
}

#[test]
fn orderbook_interactive() {
    let text = include_str!("fixtures/rest_orderbook_interactive.json");
    let b: ParadexInteractiveOrderBook = parse(text);
    assert_eq!(b.asks[0], ["64431.8".to_string(), "0.024".to_string()]);
    assert_eq!(
        b.best_bid_api,
        Some(["64387.9".to_string(), "0.00054".to_string()])
    );
    assert!(b.best_ask_interactive.is_none());

    let empty: ParadexInteractiveOrderBook = parse(include_str!(
        "fixtures/rest_orderbook_interactive_empty.json"
    ));
    assert!(empty.bids.is_empty() && empty.best_ask_api.is_none());

    for f in ["asks", "bids", "seq_no", "market"] {
        assert_required::<ParadexInteractiveOrderBook>(text, "", f);
    }
}

#[test]
fn impact_price() {
    let text = include_str!("fixtures/rest_impact_price.json");
    let p: ParadexImpactPrice = parse(text);
    assert_eq!(
        (p.side.as_str(), p.size.as_str(), p.impact_price.as_str()),
        ("SELL", "1", "64421.1024")
    );
    assert_required::<ParadexImpactPrice>(text, "", "impact_price");
}

#[test]
fn markets_history() {
    let text = include_str!("fixtures/rest_markets_history.json");
    let r: Results<ParadexMarketEvent> = parse(text);
    assert_eq!(r.results.len(), 4);

    let created = &r.results[0];
    assert_eq!(created.event_type, "CREATED");
    let f = &created.changed_fields;
    assert_eq!(f.asset_kind.as_deref(), Some("OPTION"));
    assert_eq!(f.option_type.as_deref(), Some("PUT"));
    assert_eq!(f.max_open_orders, Some(150));
    assert_eq!(f.price_bands_width, Some(0.4));
    assert!(f.option_cross_margin_params.is_some() && f.fee_config.is_some());

    let settled = &r.results[1];
    assert_eq!(
        (settled.symbol.as_str(), settled.event_type.as_str()),
        ("SNDK-USD-PERP", "SETTLED")
    );
    assert_eq!(
        settled.changed_fields.settlement_price,
        Some(845.2929065185375)
    );
    assert!(settled.changed_fields.asset_kind.is_none());

    let updated = &r.results[3];
    assert_eq!(updated.changed_fields.expiry_at, Some(1794817800000));

    for f in ["symbol", "event_type", "updated_at", "changed_fields"] {
        assert_required::<Results<ParadexMarketEvent>>(text, "/results/1", f);
    }
}

#[test]
fn settlement_price() {
    let text = include_str!("fixtures/rest_settlement_price.json");
    let s: ParadexSettlementPrice = parse(text);
    assert_eq!(s.settlement_price, "4893.983842437905");
    assert_eq!(s.expiry_at, 1794816000000);
    assert_required::<ParadexSettlementPrice>(text, "", "underlying_twap_price");
}

#[test]
fn insurance_fund() {
    let text = include_str!("fixtures/rest_insurance.json");
    let i: ParadexInsuranceFund = parse(text);
    assert_eq!(i.settlement_asset, "USDC");
    assert_eq!(i.account_value, "410948.9826134");
    assert_required::<ParadexInsuranceFund>(text, "", "account_value");
}

#[test]
fn system() {
    let state: ParadexSystemState = parse(include_str!("fixtures/rest_system_state.json"));
    assert_eq!(state.status, ParadexSystemStatus::Ok);
    for (s, want) in [
        ("maintenance", ParadexSystemStatus::Maintenance),
        ("cancel_only", ParadexSystemStatus::CancelOnly),
        ("post_only", ParadexSystemStatus::PostOnly),
    ] {
        let st: ParadexSystemState = parse(&format!(r#"{{"status":"{s}"}}"#));
        assert_eq!(st.status, want);
    }
    assert!(serde_json::from_str::<ParadexSystemState>(r#"{"status":"halted"}"#).is_err());
    assert!(serde_json::from_str::<ParadexSystemState>("{}").is_err());

    let time: ParadexSystemTime = parse(include_str!("fixtures/rest_system_time.json"));
    assert_eq!(time.server_time, "1794826522642");
    assert!(serde_json::from_str::<ParadexSystemTime>("{}").is_err());

    let stats: ParadexExchangeStats = parse(include_str!("fixtures/rest_system_stats.json"));
    assert_eq!(stats.lifetime_volume, "260644366251.21021320470989");
    assert!(serde_json::from_str::<ParadexExchangeStats>("{}").is_err());
}

#[test]
fn system_config() {
    let text = include_str!("fixtures/rest_system_config.json");
    let c: ParadexSystemConfig = parse(text);
    assert_eq!(c.starknet_chain_id, "PRIVATE_SN_PARACLEAR_MAINNET");
    assert_eq!(c.paraclear_decimals, 8);
    assert_eq!(c.bridged_tokens[0].symbol, "USDC");
    assert_eq!(c.bridged_tokens[0].decimals, 6);
    assert_required::<ParadexSystemConfig>(text, "", "starknet_chain_id");
    assert_required::<ParadexSystemConfig>(text, "/bridged_tokens/0", "l2_token_address");
}

#[test]
fn margin_config() {
    let text = include_str!("fixtures/rest_margin_config.json");
    let r: Results<ParadexMarginConfig> = parse(text);
    let btc = &r.results[0];
    assert_eq!(btc.base_asset, "BTC");
    assert_eq!(btc.mmf_factor, Some(0.5));
    assert!(!btc.scenarios.is_empty());
    assert_eq!(btc.vol_shock_params.as_ref().unwrap().dte_floor_days, 1.0);

    let aave = &r.results[1];
    assert_eq!(aave.base_asset, "AAVE");
    assert!(aave.scenarios.is_empty() && aave.vol_shock_params.is_none());
    assert!(aave.mmf_factor.is_none());

    assert_required::<Results<ParadexMarginConfig>>(text, "/results/0", "rfq_max_slippage");
    // Nullable but always sent.
    assert_required::<Results<ParadexMarginConfig>>(text, "/results/1", "vol_shock_params");
    assert_required::<Results<ParadexMarginConfig>>(text, "/results/0/scenarios/0", "weight");
}
