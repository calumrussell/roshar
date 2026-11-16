//! Offline parsing of live public market-data captures (2026-11-16) in `tests/fixtures/`.

use std::collections::BTreeMap;

use roshar_hyperliquid::types::*;
use serde::de::DeserializeOwned;
use serde_json::Value;

fn fixture(name: &str) -> String {
    std::fs::read_to_string(format!(
        "{}/tests/fixtures/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn parse<T: DeserializeOwned>(name: &str) -> T {
    serde_json::from_str(&fixture(name)).unwrap_or_else(|e| panic!("{name}: {e}"))
}

/// Removes `key` from the object at JSON pointer `at` and checks `T` then fails to parse.
fn assert_required<T: DeserializeOwned + std::fmt::Debug>(name: &str, at: &str, key: &str) {
    let mut v: Value = serde_json::from_str(&fixture(name)).unwrap();
    v.pointer_mut(at)
        .and_then(Value::as_object_mut)
        .unwrap_or_else(|| panic!("{name}: no object at {at}"))
        .remove(key)
        .unwrap_or_else(|| panic!("{name}: no {key} at {at}"));
    assert!(
        serde_json::from_value::<T>(v).is_err(),
        "{name}: {at}/{key} should be required"
    );
}

#[test]
fn all_mids() {
    let m: BTreeMap<String, String> = parse("rest_all_mids.json");
    assert_eq!(m["BTC"], "64473.5");
    assert!(m.contains_key("@107"));
    let x: BTreeMap<String, String> = parse("rest_all_mids_xyz.json");
    assert!(x.keys().all(|k| k.starts_with("xyz:")));
}

#[test]
fn l2_book_full_and_aggregated() {
    let b: HyperliquidBook = parse("rest_l2_book.json");
    assert_eq!(b.coin, "BTC");
    assert_eq!(b.levels.len(), 2);
    assert_eq!(b.levels[0][0].px, "64487.0");
    let a: HyperliquidBook = parse("rest_l2_book_nsigfigs5_mantissa2.json");
    assert_eq!(a.levels[0][0].px, "64486.0");
    assert_required::<HyperliquidBook>("rest_l2_book.json", "/levels/0/0", "n");
}

#[test]
fn recent_trades() {
    let t: Vec<HyperliquidTrade> = parse("rest_recent_trades.json");
    assert_eq!(t.len(), 2);
    assert_eq!(t[0].users.len(), 2);
    assert_required::<Vec<HyperliquidTrade>>("rest_recent_trades.json", "/0", "tid");
}

#[test]
fn meta_first_dex_and_hip3() {
    let m: Universe = parse("rest_meta.json");
    assert_eq!(m.universe[0].name, "BTC");
    assert!(!m.margin_tables.is_empty());
    assert!(m.universe.iter().any(|a| a.is_delisted == Some(true)));
    let x: Universe = parse("rest_meta_xyz.json");
    assert_eq!(x.universe[0].growth_mode.as_deref(), Some("enabled"));
    assert_eq!(x.universe[0].deployer_fee_scale.as_deref(), Some("1.0"));
    assert!(x.universe[1].growth_mode.is_none());
    assert_required::<Universe>("rest_meta.json", "", "marginTables");
    assert_required::<Universe>("rest_meta.json", "/universe/0", "szDecimals");
}

#[test]
fn all_perp_metas() {
    let m: Vec<Universe> = parse("rest_all_perp_metas.json");
    assert_eq!(m.len(), 2);
    assert!(m[1].universe[0].name.contains(':'));
    assert_required::<Vec<Universe>>("rest_all_perp_metas.json", "/1", "collateralToken");
}

#[test]
fn perp_dexs() {
    let d: Vec<Option<PerpDex>> = parse("rest_perp_dexs.json");
    assert!(d[0].is_none());
    let xyz = d[1].as_ref().unwrap();
    assert_eq!(xyz.name, "xyz");
    assert!(xyz.oracle_updater.is_none());
    assert_eq!(xyz.sub_deployers[0].0, "registerAsset");
    assert!(d
        .iter()
        .flatten()
        .any(|d| !d.asset_to_funding_clamp.is_empty()));
    assert_required::<Vec<Option<PerpDex>>>("rest_perp_dexs.json", "/1", "assetToStreamingOiCap");
    // Nullable but always sent: the key is required even though its value may be null.
    assert_required::<Vec<Option<PerpDex>>>("rest_perp_dexs.json", "/1", "oracleUpdater");
}

#[test]
fn predicted_fundings() {
    let p: PredictedFundings = parse("rest_predicted_fundings.json");
    let btc = p.iter().find(|(c, _)| c == "BTC").unwrap();
    let hl = btc
        .1
        .iter()
        .find(|(v, _)| v == "HlPerp")
        .unwrap()
        .1
        .as_ref()
        .unwrap();
    assert_eq!(hl.funding_interval_hours, Some(1));
    assert!(p.iter().any(|(_, v)| v.iter().any(|(_, f)| f.is_none())));
    assert!(p.iter().any(|(_, v)| v.iter().any(|(_, f)| f
        .as_ref()
        .is_some_and(|f| f.funding_interval_hours.is_none()))));
    assert_required::<PredictedFundings>(
        "rest_predicted_fundings.json",
        "/2/1/0/1",
        "nextFundingTime",
    );
}

#[test]
fn perp_dex_limits_status_oi_cap() {
    let l: PerpDexLimits = parse("rest_perp_dex_limits_xyz.json");
    assert_eq!(l.coin_to_oi_cap.len(), 3);
    assert_required::<PerpDexLimits>("rest_perp_dex_limits_xyz.json", "", "totalOiCap");
    let s: PerpDexStatus = parse("rest_perp_dex_status.json");
    assert!(!s.total_net_deposit.is_empty());
    assert_required::<PerpDexStatus>("rest_perp_dex_status.json", "", "totalNetDeposit");
    let c: Vec<String> = parse("rest_perps_at_oi_cap.json");
    assert!(c.contains(&"LOOM".to_string()));
}

#[test]
fn perp_annotations_and_categories() {
    let a: Option<PerpAnnotation> = parse("rest_perp_annotation_xyz.json");
    let a = a.unwrap();
    assert_eq!(a.category, "indices");
    assert_eq!(a.display_name.as_deref(), Some("XYZ100"));
    assert!(serde_json::from_str::<Option<PerpAnnotation>>("null")
        .unwrap()
        .is_none());
    assert_required::<PerpAnnotation>("rest_perp_annotation_xyz.json", "", "description");
    let c: Vec<(String, String)> = parse("rest_perp_categories.json");
    assert_eq!(c.len(), 4);
    let cc: Vec<(String, PerpConciseAnnotation)> = parse("rest_perp_concise_annotations.json");
    assert!(cc
        .iter()
        .any(|(_, a)| a.display_name.is_none() && a.keywords.is_none()));
    assert!(cc.iter().any(|(_, a)| a.keywords.is_some()));
    assert_required::<Vec<(String, PerpConciseAnnotation)>>(
        "rest_perp_concise_annotations.json",
        "/0/1",
        "category",
    );
}

#[test]
fn spot_meta() {
    let m: SpotMeta = parse("rest_spot_meta.json");
    assert_eq!(m.tokens[0].name, "USDC");
    assert_eq!(
        m.tokens[0]
            .evm_contract
            .as_ref()
            .unwrap()
            .evm_extra_wei_decimals,
        -2
    );
    assert!(m.tokens.iter().any(|t| t.deployer_label.is_some()));
    assert_eq!(m.universe[1].name, "@1");
    assert_required::<SpotMeta>(
        "rest_spot_meta.json",
        "/tokens/0",
        "deployerTradingFeeShare",
    );
    assert!(m.tokens[4].evm_contract.is_none() && m.tokens[4].full_name.is_none());
    assert_required::<SpotMeta>("rest_spot_meta.json", "/tokens/4", "evmContract");
}

#[test]
fn token_details() {
    let h: TokenDetails = parse("rest_token_details_hfun.json");
    assert_eq!(h.name, "HFUN");
    let g = h.genesis.unwrap();
    assert_eq!(g.existing_token_balances[0].0, 1);
    let u: TokenDetails = parse("rest_token_details_usdc.json");
    assert!(u.genesis.is_none() && u.deployer.is_none() && u.deploy_gas.is_none());
    assert_required::<TokenDetails>("rest_token_details_usdc.json", "", "circulatingSupply");
    assert_required::<TokenDetails>("rest_token_details_usdc.json", "", "genesis");
    assert_required::<TokenDetails>("rest_token_details_usdc.json", "", "deployTime");
}

#[test]
fn outcomes() {
    let m: OutcomeMeta = parse("rest_outcome_meta.json");
    assert!(m.outcomes.iter().any(|o| o.venue.is_none()));
    assert!(m.outcomes.iter().any(|o| o.deployer_fee_scale.is_some()));
    assert_eq!(m.questions.len(), 1);
    assert!(!m.deployers[0].sub_deployers.is_empty());
    assert_required::<OutcomeMeta>("rest_outcome_meta.json", "/outcomes/0", "sideSpecs");

    let t: Vec<OutcomeTemplate> = parse("rest_outcome_templates.json");
    assert!(t.iter().any(|t| t.role == OutcomeTemplateRole::Question));
    assert!(t
        .iter()
        .any(|t| matches!(t.role, OutcomeTemplateRole::StandaloneOutcome { .. })));
    assert!(t
        .iter()
        .any(|t| matches!(t.role, OutcomeTemplateRole::QuestionOutcome { .. })));
    assert_required::<Vec<OutcomeTemplate>>("rest_outcome_templates.json", "/0", "keywords");

    let s: Option<SettledOutcome> = parse("rest_settled_outcome.json");
    let s = s.unwrap();
    assert_eq!(s.spec.outcome, 95);
    assert_eq!(s.details, "price:76876.9");
    assert_required::<SettledOutcome>("rest_settled_outcome.json", "", "settleFraction");
}

fn ws_frames() -> Vec<(String, HyperliquidWsEvent)> {
    fixture("ws_frames.jsonl")
        .lines()
        .map(|l| {
            let e = HyperliquidWsEvent::parse(l).unwrap_or_else(|e| panic!("{e}: {l}"));
            (l.to_string(), e)
        })
        .collect()
}

#[test]
fn ws_frames_all_parse() {
    use HyperliquidWsEvent as E;
    let frames = ws_frames();
    let mut acks = 0;
    let (mut book, mut agg, mut perp_ctx, mut spot_ctx, mut fast) = (0, 0, 0, 0, 0);
    for (_, e) in &frames {
        match e {
            E::SubscriptionResponse(m) => {
                acks += 1;
                let s = m.subscription.as_ref().unwrap();
                if s.typ == "l2Book" {
                    assert_eq!(s.fast, Some(false));
                }
            }
            E::Error(msg) => assert!(msg.contains("bogus")),
            E::Pong => {}
            E::L2Book(b) => {
                book += 1;
                if b.levels[0][0].px.ends_with("2.0") {
                    agg += 1;
                }
            }
            E::Candle(c) => assert_eq!(c.i, "1m"),
            E::Trades(t) => assert_eq!(t.len(), 2),
            E::Bbo(b) => assert!(b.bbo[0].is_some()),
            E::ActiveAssetCtx(c) => {
                perp_ctx += 1;
                assert_eq!(c.coin, "BTC");
            }
            E::ActiveSpotAssetCtx(c) => {
                spot_ctx += 1;
                assert_eq!(c.ctx.coin, "@107");
            }
            E::AllMids(m) => assert!(m.dex.is_none() || m.dex.as_deref() == Some("xyz")),
            E::AllDexsAssetCtxs(a) => {
                assert_eq!(a.ctxs[0].0, "");
                assert!(a.ctxs[0].1.iter().any(|c| c.mid_price.is_none()));
            }
            E::FastAssetCtxs(p) => {
                fast += 1;
                let d = decode_fast_asset_ctxs(p).unwrap();
                assert!(!d.is_empty());
                assert!(d.values().any(|c| c.mark_px.is_none()));
            }
            other => panic!("unexpected frame {other:?}"),
        }
    }
    assert_eq!(acks, 24);
    assert_eq!((book, agg, perp_ctx, spot_ctx, fast), (2, 1, 1, 1, 1));
}

#[test]
fn ws_l2_book_ack_echoes_aggregation() {
    let ack = ws_frames()
        .into_iter()
        .find_map(|(_, e)| match e {
            HyperliquidWsEvent::SubscriptionResponse(m)
                if m.subscription
                    .as_ref()
                    .is_some_and(|s| s.n_sig_figs.is_some()) =>
            {
                m.subscription
            }
            _ => None,
        })
        .unwrap();
    assert_eq!((ack.n_sig_figs, ack.mantissa), (Some(5), Some(2)));
}

#[test]
fn ws_unknown_channel_and_missing_field_are_errors() {
    assert!(HyperliquidWsEvent::parse(r#"{"channel":"nope","data":{}}"#).is_err());
    let (line, _) = ws_frames()
        .into_iter()
        .find(|(_, e)| matches!(e, HyperliquidWsEvent::ActiveAssetCtx(_)))
        .unwrap();
    let mut v: Value = serde_json::from_str(&line).unwrap();
    v["data"]["ctx"].as_object_mut().unwrap().remove("markPx");
    assert!(serde_json::from_value::<HyperliquidWsEvent>(v).is_err());
    let (line, _) = ws_frames()
        .into_iter()
        .find(|(_, e)| matches!(e, HyperliquidWsEvent::AllDexsAssetCtxs(_)))
        .unwrap();
    let mut v: Value = serde_json::from_str(&line).unwrap();
    v["data"]["ctxs"][0][1][0]
        .as_object_mut()
        .unwrap()
        .remove("openInterest");
    assert!(serde_json::from_value::<HyperliquidWsEvent>(v).is_err());
    // The second first-dex ctx has `midPx: null`; dropping the key must still fail.
    let (line, _) = ws_frames()
        .into_iter()
        .find(|(_, e)| matches!(e, HyperliquidWsEvent::AllDexsAssetCtxs(_)))
        .unwrap();
    let mut v: Value = serde_json::from_str(&line).unwrap();
    assert!(v["data"]["ctxs"][0][1][1]["midPx"].is_null());
    v["data"]["ctxs"][0][1][1]
        .as_object_mut()
        .unwrap()
        .remove("midPx")
        .unwrap();
    assert!(serde_json::from_value::<HyperliquidWsEvent>(v).is_err());
}

/// Test vector from the subscriptions docs.
#[test]
fn fast_asset_ctxs_doc_vector() {
    let d = decode_fast_asset_ctxs(
        "q1ZyCnFWsqpWyk0syg6oULJSsjQ3NTDQM1Wq1VFyDfFAkTI2MzXQMwJLVVRWWfmFuTiiyBuamOoZKdXWAgA=",
    )
    .unwrap();
    assert_eq!(d["BTC"].mark_px.as_deref(), Some("97500.5"));
    assert_eq!(d["xyz:NVDA"].mid_px, None);
    let n: BTreeMap<String, FastAssetCtx> =
        serde_json::from_str(r#"{"X":{"midPx":null}}"#).unwrap();
    assert_eq!(n["X"].mid_px, Some(None));
    assert!(decode_fast_asset_ctxs("not base64!").is_err());
}

/// `outcomeMetaUpdates` sent no data during capture; shapes from the subscriptions docs.
#[test]
fn outcome_meta_updates_doc_shapes() {
    let e = HyperliquidWsEvent::parse(
        r#"{"channel":"outcomeMetaUpdates","data":[
            {"outcomeSettled":95},
            {"questionSettled":647},
            {"outcomeCreated":{"outcome":1,"name":"Recurring","description":"d","sideSpecs":[{"name":"Yes"},{"name":"No"}],"quoteToken":"USDC"}},
            {"questionUpdated":{"question":2,"name":"n","description":"d","fallbackOutcome":3,"namedOutcomes":[4],"settledNamedOutcomes":[]}}
        ]}"#,
    )
    .unwrap();
    let HyperliquidWsEvent::OutcomeMetaUpdates(u) = e else {
        panic!()
    };
    assert!(matches!(u[0], WsOutcomeMetaUpdate::OutcomeSettled(95)));
    assert!(matches!(u[3], WsOutcomeMetaUpdate::QuestionUpdated(_)));
}

#[test]
fn subscription_builders() {
    assert_eq!(
        HyperliquidWssMessage::l2_book_with("BTC", Some(5), Some(2), None).to_json(),
        r#"{"method":"subscribe","subscription":{"type":"l2Book","coin":"BTC","nSigFigs":5,"mantissa":2}}"#
    );
    assert_eq!(
        HyperliquidWssMessage::l2_book_with_unsub("BTC", Some(3), None, Some(true)).to_json(),
        r#"{"method":"unsubscribe","subscription":{"type":"l2Book","coin":"BTC","nSigFigs":3,"fast":true}}"#
    );
    assert_eq!(
        HyperliquidWssMessage::active_asset_ctx("@107").to_json(),
        r#"{"method":"subscribe","subscription":{"type":"activeAssetCtx","coin":"@107"}}"#
    );
    assert_eq!(
        HyperliquidWssMessage::all_dexs_asset_ctxs_unsub().to_json(),
        r#"{"method":"unsubscribe","subscription":{"type":"allDexsAssetCtxs"}}"#
    );
    assert_eq!(
        HyperliquidWssMessage::fast_asset_ctxs().to_json(),
        r#"{"method":"subscribe","subscription":{"type":"fastAssetCtxs"}}"#
    );
    assert_eq!(
        HyperliquidWssMessage::outcome_meta_updates().to_json(),
        r#"{"method":"subscribe","subscription":{"type":"outcomeMetaUpdates"}}"#
    );
    assert_eq!(
        HyperliquidWssMessage::all_mids_dex_unsub("xyz").to_json(),
        r#"{"method":"unsubscribe","subscription":{"type":"allMids","dex":"xyz"}}"#
    );
}
