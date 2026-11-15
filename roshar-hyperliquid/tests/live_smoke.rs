//! Live end-to-end check against mainnet; needs network. Uses the throwaway vector key, which has no
//! account, so the order is rejected by the exchange and nothing trades.
//! `cargo test -p roshar-hyperliquid --test live_smoke -- --ignored --nocapture`

use roshar_hyperliquid::{
    to_tick, Action, Info, NonceGen, OrderReq, OrderStatus, Signer, Tif, WsPoster,
};
use serde_json::json;
use std::time::Duration;

const WS_URL: &str = "wss://api.hyperliquid.xyz/ws";

#[tokio::test]
#[ignore]
async fn live_smoke() {
    let info = Info::new().unwrap();
    let (asset, sz_dec) = info.asset_meta("PUMP").await.unwrap();
    let snap = info.l2_snapshot("PUMP").await.unwrap();
    assert!(!snap.bids.is_empty() && !snap.asks.is_empty());
    assert!(snap.bids[0].px < snap.asks[0].px);
    println!(
        "PUMP asset {asset} szDecimals {sz_dec}; l2 time {} bid {} ask {}",
        snap.time_ms, snap.bids[0].px, snap.asks[0].px
    );

    let poster = WsPoster::connect(WS_URL).await.unwrap();

    let p = poster
        .post_info(json!({"type": "l2Book", "coin": "PUMP"}))
        .await
        .unwrap();
    let r = tokio::time::timeout(Duration::from_secs(12), p.rx)
        .await
        .unwrap()
        .unwrap();
    println!(
        "post_info l2Book: {:?} in {} us",
        r.status,
        (r.recv_ns - r.sent_ns) / 1000
    );
    assert_eq!(r.status, Ok(OrderStatus::Success));

    let v: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/vectors.json"
    )))
    .unwrap();
    let signer = Signer::from_hex(v["key"].as_str().unwrap(), None, true).unwrap();
    let px = to_tick(snap.bids[0].px * 0.95, sz_dec, false);
    let sz = ((12.0 / px) * 10f64.powi(sz_dec as i32)).ceil() / 10f64.powi(sz_dec as i32);
    let action = Action::Order {
        orders: vec![OrderReq {
            asset,
            is_buy: true,
            px,
            sz,
            reduce_only: false,
            tif: Tif::Ioc,
            cloid: Some([7u8; 16]),
        }],
        priority: None,
    };
    let payload = signer.sign(&action, NonceGen::new().next()).unwrap();
    let p = poster.post(payload).await.unwrap();
    let r = tokio::time::timeout(Duration::from_secs(12), p.rx)
        .await
        .unwrap()
        .unwrap();
    println!(
        "post IOC: {:?} in {} us",
        r.status,
        (r.recv_ns - r.sent_ns) / 1000
    );
    match r.status {
        Ok(OrderStatus::Rejected(t)) => {
            assert!(t.contains("does not exist"), "unexpected rejection: {t}");
        }
        other => panic!("expected Ok(Rejected(..does not exist..)), got {other:?}"),
    }
}
