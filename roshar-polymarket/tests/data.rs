//! Data API v2 fixtures: trimmed (fewer rows, values unedited) live responses captured on
//! 2026-11-16 from `https://data-api.polymarket.com/v2/...`.

use roshar_polymarket::types::{
    DataEnvelope, DataError, DataLiveVolume, DataOpenInterest, DataPage, DataPricePoint,
    DataResolution, DataSide, DataStatus, DataToken, DataTokenHolders, DataTrade,
};
use serde_json::Value;

fn without(fixture: &str, pointer: &str, key: &str) -> String {
    let mut v: Value = serde_json::from_str(fixture).unwrap();
    v.pointer_mut(pointer)
        .unwrap()
        .as_object_mut()
        .unwrap()
        .remove(key)
        .unwrap();
    v.to_string()
}

const TRADES: &str = include_str!("fixtures/data_trades.json");
const HOLDERS: &str = include_str!("fixtures/data_holders.json");
const HOLDERS_PNL: &str = include_str!("fixtures/data_holders_pnl.json");
const TOKENS: &str = include_str!("fixtures/data_tokens.json");
const STATUS: &str = include_str!("fixtures/data_status.json");
const RESOLUTIONS: &str = include_str!("fixtures/data_resolutions.json");

#[test]
fn parses_trades() {
    let page: DataPage<DataTrade> = serde_json::from_str(TRADES).unwrap();
    assert_eq!(page.data.len(), 3);
    assert!(page.pagination.has_more);
    assert!(page.pagination.next_cursor.is_some());
    // Unenriched rows send empty strings, not nulls.
    assert_eq!(page.data[0].outcome, "");
    for t in &page.data {
        assert!(t.price > 0.0 && t.size > 0.0);
        assert!(matches!(t.side, DataSide::Buy | DataSide::Sell));
    }
}

#[test]
fn trades_are_strict() {
    let missing = without(TRADES, "/data/0", "price");
    assert!(serde_json::from_str::<DataPage<DataTrade>>(&missing).is_err());
    let missing = without(TRADES, "/pagination", "next_cursor");
    assert!(serde_json::from_str::<DataPage<DataTrade>>(&missing).is_err());
}

#[test]
fn parses_holders_with_and_without_pnl() {
    let page: DataPage<DataTokenHolders> = serde_json::from_str(HOLDERS).unwrap();
    assert_eq!(page.data.len(), 2);
    assert!(page.data[0].holders[0].total_pnl.is_none());

    let page: DataPage<DataTokenHolders> = serde_json::from_str(HOLDERS_PNL).unwrap();
    let h = &page.data[0].holders[0];
    assert!(h.avg_price.is_some() && h.total_pnl.is_some());

    let missing = without(HOLDERS, "/data/0/holders/0", "amount");
    assert!(serde_json::from_str::<DataPage<DataTokenHolders>>(&missing).is_err());
}

#[test]
fn parses_open_interest_and_live_volume() {
    let oi: DataEnvelope<Vec<DataOpenInterest>> =
        serde_json::from_str(include_str!("fixtures/data_oi.json")).unwrap();
    assert_eq!(oi.data.len(), 3);
    let global: DataEnvelope<Vec<DataOpenInterest>> =
        serde_json::from_str(include_str!("fixtures/data_oi_global.json")).unwrap();
    assert_eq!(global.data[0].condition_id, "GLOBAL");

    let lv = include_str!("fixtures/data_live_volume.json");
    let parsed: DataEnvelope<DataLiveVolume> = serde_json::from_str(lv).unwrap();
    assert_eq!(parsed.data.conditions.len(), 3);
    let missing = without(lv, "/data", "taker_volume_total");
    assert!(serde_json::from_str::<DataEnvelope<DataLiveVolume>>(&missing).is_err());
}

#[test]
fn parses_prices_history() {
    let page: DataPage<DataPricePoint> =
        serde_json::from_str(include_str!("fixtures/data_prices_history.json")).unwrap();
    assert_eq!(page.data.len(), 3);
    assert!(!page.pagination.has_more);
    assert!(page.pagination.next_cursor.is_none());
}

#[test]
fn parses_resolutions() {
    let r: DataEnvelope<Vec<DataResolution>> = serde_json::from_str(RESOLUTIONS).unwrap();
    assert!(r.data[0].expected_settlement_time.is_some());
    assert!(r.data[1].expected_settlement_time.is_none());
    assert!(r.data.iter().all(|x| x.condition_id.is_some()));

    let q: DataEnvelope<Vec<DataResolution>> =
        serde_json::from_str(include_str!("fixtures/data_resolutions_question.json")).unwrap();
    assert!(q.data[0].condition_id.is_none());

    let missing = without(RESOLUTIONS, "/data/0", "status");
    assert!(serde_json::from_str::<DataEnvelope<Vec<DataResolution>>>(&missing).is_err());
}

#[test]
fn parses_tokens_with_nulls() {
    let t: DataEnvelope<Vec<DataToken>> = serde_json::from_str(TOKENS).unwrap();
    assert_eq!(t.data.len(), 3);
    assert!(t.data[0].clob_index.is_none());
    assert!(t.data[1].neg_risk_market_id.is_some());
    assert!(t.data[2].neg_risk_market_id.is_none());

    // Nullable keys are still required.
    let missing = without(TOKENS, "/data/0", "final_price");
    assert!(serde_json::from_str::<DataEnvelope<Vec<DataToken>>>(&missing).is_err());
}

#[test]
fn parses_status() {
    let s: DataEnvelope<DataStatus> = serde_json::from_str(STATUS).unwrap();
    assert!(!s.data.serving.mechanisms.is_empty());
    assert!(s.data.serving.mechanisms[0].blocks_behind.is_none());
    assert!(s.data.ingestion.most_lagged.is_some());

    let missing = without(STATUS, "/data/ingestion", "most_lagged");
    assert!(serde_json::from_str::<DataEnvelope<DataStatus>>(&missing).is_err());
}

#[test]
fn parses_error_body() {
    let e: DataError = serde_json::from_str(include_str!("fixtures/data_error.json")).unwrap();
    assert_eq!(e.code, "invalid_request");
    assert_eq!(e.parameter.as_deref(), Some("condition"));
    assert!(!e.retryable);
}
