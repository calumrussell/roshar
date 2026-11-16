//! Offline parse tests over live captures (`tests/fixtures`, 2026-11-15, production V2 hosts).

use roshar_edgex::signer::L2Domain;
use roshar_edgex::types::*;
use serde::de::DeserializeOwned;
use serde_json::Value;

fn fixture(name: &str) -> String {
    let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn data<T: DeserializeOwned>(name: &str) -> T {
    let reply: EdgexResponse<T> =
        serde_json::from_str(&fixture(name)).unwrap_or_else(|e| panic!("{name}: {e}"));
    reply
        .into_result()
        .unwrap_or_else(|e| panic!("{name}: {e}"))
        .unwrap_or_else(|| panic!("{name}: no data"))
}

#[test]
fn server_time() {
    let t: EdgexServerTime = data("meta_get_server_time.json");
    assert!(t.time_millis.parse::<u64>().unwrap() > 1_700_000_000_000);
}

#[test]
fn meta_data() {
    let m: EdgexMetaData = data("meta_get_meta_data.json");
    assert_eq!(m.global.native_chain_id, "3343");
    assert_eq!(m.global.chain_id, "0xd0f");
    let btc = m
        .contract_list
        .iter()
        .find(|c| c.contract_name == "BTCUSDC")
        .unwrap();
    assert_eq!(btc.contract_id, "30000001");
    assert_eq!(btc.quote_coin_id, "1000");
    assert!(!btc.risk_tier_list.is_empty());
    let usdc = m.coin_list.iter().find(|c| c.coin_id == "1000").unwrap();
    assert_eq!(usdc.resolution.as_deref(), Some("0xf4240"));
    assert!(m.coin_list.iter().any(|c| c.resolution.is_none()));

    let domain = L2Domain::from_global(&m.global).unwrap();
    assert_eq!(domain.chain_id, 3343);
}

#[test]
fn depth() {
    let d: Vec<EdgexDepth> = data("quote_get_depth.json");
    assert_eq!(d.len(), 1);
    assert_eq!(d[0].level, 15);
    assert_eq!(d[0].depth_type, "SNAPSHOT");
    assert_eq!(d[0].asks.len(), 15);
    assert_eq!(d[0].bids.len(), 15);
}

#[test]
fn ticker() {
    let t: Vec<EdgexTicker> = data("quote_get_ticker.json");
    assert_eq!(t[0].contract_id, "30000001");
    assert!(!t[0].oracle_price.is_empty());
}

#[test]
fn klines() {
    let k: EdgexPage<EdgexKline> = data("quote_get_kline.json");
    assert_eq!(k.data_list.len(), 3);
    assert_eq!(k.data_list[0].kline_type, "HOUR_1");
    assert!(!k.next_page_offset_data.is_empty());

    let m: Vec<EdgexContractKlines> = data("quote_get_multi_contract_kline.json");
    assert_eq!(m.len(), 2);
    assert_eq!(m[0].kline_list.len(), 2);
}

#[test]
fn market_status() {
    let s: EdgexMarketStatus = data("quote_get_market_status.json");
    assert_eq!(s.contract_id, 30000001);
    assert!(s.status);
}

#[test]
fn funding() {
    let latest: Vec<EdgexFundingRate> = data("funding_get_latest_funding_rate.json");
    assert_eq!(latest[0].contract_id, "30000001");
    assert!(!latest[0].is_settlement);

    let page: EdgexPage<EdgexFundingRate> = data("funding_get_funding_rate_page.json");
    assert_eq!(page.data_list.len(), 3);
    assert!(page.data_list[0].forecast_funding_rate.is_empty());
}

#[test]
fn error_envelopes() {
    for (name, code) in [
        ("error_invalid_depth_level.json", "INVALID_DEPTH_LEVEL"),
        ("error_invalid_api_key.json", "INVALID_API_KEY"),
    ] {
        let reply: EdgexResponse<Vec<EdgexDepth>> = serde_json::from_str(&fixture(name)).unwrap();
        let err = reply.into_result().unwrap_err();
        assert_eq!(err.code, code);
        assert!(err.msg.is_some());
    }
}

#[test]
fn strict_parsing_rejects_missing_fields() {
    let mut v: Value = serde_json::from_str(&fixture("quote_get_ticker.json")).unwrap();
    v["data"][0].as_object_mut().unwrap().remove("oraclePrice");
    assert!(serde_json::from_value::<EdgexResponse<Vec<EdgexTicker>>>(v).is_err());
}

fn ws(key: &str) -> EdgexPublicWsMessage {
    let all: Value = serde_json::from_str(&fixture("ws_public.json")).unwrap();
    serde_json::from_value(all[key].clone()).unwrap_or_else(|e| panic!("{key}: {e}"))
}

fn quote<T: DeserializeOwned>(key: &str, expect: EdgexDataType) -> Vec<T> {
    match ws(key) {
        EdgexPublicWsMessage::QuoteEvent { content, .. } => {
            assert_eq!(content.data_type, expect, "{key}");
            let items = content.data::<T>().unwrap_or_else(|e| panic!("{key}: {e}"));
            assert!(!items.is_empty(), "{key}");
            items
        }
        other => panic!("{key}: {other:?}"),
    }
}

#[test]
fn ws_control_frames() {
    assert!(matches!(
        ws("connected"),
        EdgexPublicWsMessage::Connected { .. }
    ));
    match ws("subscribed") {
        EdgexPublicWsMessage::Subscribed { channel, .. } => assert_eq!(channel, "ticker.30000001"),
        other => panic!("{other:?}"),
    }
    assert!(matches!(
        ws("unsubscribed"),
        EdgexPublicWsMessage::Unsubscribed { .. }
    ));
    match ws("ping") {
        EdgexPublicWsMessage::Ping { time } => {
            assert_eq!(
                EdgexWsRequest::pong(&time).to_json(),
                format!(r#"{{"type":"pong","time":"{time}"}}"#)
            )
        }
        other => panic!("{other:?}"),
    }
    match ws("error") {
        EdgexPublicWsMessage::Error { content, .. } => {
            assert_eq!(content.code, "GATEWAY_INVALID_CHANNEL")
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn ws_quote_events() {
    use EdgexDataType::{Changed, Snapshot};
    let t: Vec<EdgexWsTicker> = quote("ticker_snapshot", Snapshot);
    assert!(t[0].best_ask_price.is_some());
    quote::<EdgexWsTicker>("ticker_changed", Changed);
    quote::<EdgexWsTicker>("ticker_all_snapshot", Snapshot);
    quote::<EdgexWsTicker>("ticker_all_changed", Changed);
    let mixed: Vec<EdgexWsTicker> = quote("ticker_all_mixed", Snapshot);
    assert!(mixed.iter().any(|t| t.best_ask_price.is_none()));
    assert!(mixed.iter().any(|t| t.liquidity.is_some()));

    let d: Vec<EdgexDepth> = quote("depth_snapshot", Snapshot);
    assert_eq!(d[0].depth_type, "SNAPSHOT");
    let d: Vec<EdgexDepth> = quote("depth_changed", Changed);
    assert_eq!(d[0].depth_type, "CHANGED");

    quote::<EdgexWsTrade>("trades_snapshot", Snapshot);
    quote::<EdgexWsTrade>("trades_changed", Changed);
    quote::<EdgexKline>("kline_snapshot", Snapshot);
    quote::<EdgexKline>("kline_changed", Changed);
    let f: Vec<EdgexWsFundingRate> = quote("fundingRate_snapshot", Snapshot);
    assert_eq!(f[0].contract_id, "30000001");
    quote::<EdgexWsFundingRate>("fundingRate_changed", Changed);
    quote::<EdgexWsFundingRate>("fundingRate_all_snapshot", Snapshot);
    quote::<EdgexWsFundingRate>("fundingRate_all_changed", Changed);
}

#[test]
fn ws_metadata() {
    let msg: EdgexPublicWsMessage = serde_json::from_str(&fixture("ws_metadata.json")).unwrap();
    let EdgexPublicWsMessage::QuoteEvent { channel, content } = msg else {
        panic!("not a quote-event")
    };
    assert_eq!(channel, "metadata");
    assert_eq!(content.data_type, EdgexDataType::Snapshot);
    let m: Vec<EdgexWsMetaData> = content.data().unwrap();
    assert_eq!(m[0].contract_list[0].contract_id, "30000001");
    assert_eq!(m[0].global.collateral_coin.coin_name, "USDC");
    assert!(!m[0].multi_chain.chain_list[0].token_list.is_empty());

    let mut v: Value = serde_json::from_str(&fixture("ws_metadata.json")).unwrap();
    v["content"]["data"][0]["contractList"][0]
        .as_object_mut()
        .unwrap()
        .remove("tickSize");
    let EdgexPublicWsMessage::QuoteEvent { content, .. } = serde_json::from_value(v).unwrap()
    else {
        panic!("not a quote-event")
    };
    assert!(content.data::<EdgexWsMetaData>().is_err());
}

#[test]
fn ws_requests_and_channels() {
    assert_eq!(
        EdgexWsRequest::subscribe(EdgexChannel::depth("30000001", EdgexDepthLevel::L200)).to_json(),
        r#"{"type":"subscribe","channel":"depth.30000001.200"}"#
    );
    assert_eq!(
        EdgexWsRequest::unsubscribe(EdgexChannel::trades("30000001")).to_json(),
        r#"{"type":"unsubscribe","channel":"trades.30000001"}"#
    );
    assert_eq!(
        EdgexChannel::kline(
            "30000001",
            EdgexPriceType::LastPrice,
            EdgexKlineType::Minute1
        ),
        "kline.LAST_PRICE.30000001.MINUTE_1"
    );
    assert_eq!(EdgexChannel::ticker_all(), "ticker.all.1s");
    assert_eq!(EdgexChannel::funding_rate_all(), "fundingRate.all");
}

/// The docs' live (trimmed) private-websocket samples.
#[test]
fn private_ws_doc_samples() {
    let connected = r#"{"sid":"371619ee-b099-751a-74b1-b46e98606322","type":"connected","time":"1773832863182"}"#;
    assert!(matches!(
        serde_json::from_str::<EdgexPrivateWsMessage>(connected).unwrap(),
        EdgexPrivateWsMessage::Connected { .. }
    ));
    let update = r#"{"type":"trade-event","content":{"event":"ORDER_UPDATE","version":916,"time":1773833120269,"accountId":0,"data":{
        "order":[{"id":"729107150223180559","contractId":"10000002","side":"SELL","type":"STOP_MARKET","status":"FILLED","triggerPrice":"2310.00","cumMatchSize":"0.02","cumMatchValue":"46.1952","cumMatchFee":"0.017554","updatedTime":"1773833120256"}],
        "orderFillTransaction":[{"id":"729108951626416911","orderId":"729107150223180559","fillSize":"0.02","fillValue":"46.1952","fillFee":"0.017554","fillPrice":"2309.76","direction":"TAKER","matchTime":"1773833120252"}],
        "position":[{"contractId":"10000002","openSize":"0.01","openValue":"23.200067","fundingFee":"-0.000096","updatedTime":"1773833120269"}],
        "collateral":[{"coinId":"1000","amount":"129.690336","cumPositionSellAmount":"46.1952","cumFillFeeAmount":"-0.046603","updatedTime":"1773833120269"}]}}}"#;
    match serde_json::from_str::<EdgexPrivateWsMessage>(update).unwrap() {
        EdgexPrivateWsMessage::TradeEvent { content } => {
            assert_eq!(content.event, "ORDER_UPDATE");
            assert_eq!(
                content.data.order[0].cum_match_size.as_deref(),
                Some("0.02")
            );
            assert_eq!(content.data.order_fill_transaction[0].direction, "TAKER");
            assert_eq!(content.data.position[0].open_size, "0.01");
            assert!(content.data.account.is_empty());
        }
        other => panic!("{other:?}"),
    }
}
