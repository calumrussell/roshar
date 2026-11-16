//! Offline parsing of live captures (fapi.binance.com and fstream.binance.com, 2026-11-16).
//! Captures are trimmed to a few elements; values are unedited.

use roshar_binance::types::*;
use serde::de::DeserializeOwned;
use serde_json::Value;

macro_rules! rest {
    ($name:literal) => {
        include_str!(concat!("fixtures/rest/", $name, ".json"))
    };
}

macro_rules! ws {
    ($name:literal) => {
        include_str!(concat!("fixtures/ws/", $name, ".json"))
    };
}

fn parse<T: DeserializeOwned>(text: &str) -> T {
    serde_json::from_str(text).unwrap_or_else(|e| panic!("{e}: {text}"))
}

/// Removes `field` from the object (or from the first element of an array) and expects `T` to fail.
fn assert_requires<T: DeserializeOwned>(text: &str, field: &str) {
    let mut value: Value = serde_json::from_str(text).unwrap();
    let obj = match &mut value {
        Value::Array(items) => items[0].as_object_mut().unwrap(),
        other => other.as_object_mut().unwrap(),
    };
    assert!(obj.remove(field).is_some(), "fixture has no {field}");
    assert!(
        serde_json::from_value::<T>(value).is_err(),
        "parsed without required {field}"
    );
}

fn event(text: &str) -> BinanceWsEvent {
    BinanceWsEvent::parse(text).unwrap_or_else(|e| panic!("{e}: {text}"))
}

fn assert_event_requires(text: &str, field: &str) {
    let mut value: Value = serde_json::from_str(text).unwrap();
    let obj = match &mut value {
        Value::Array(items) => items[0].as_object_mut().unwrap(),
        other => other.as_object_mut().unwrap(),
    };
    assert!(obj.remove(field).is_some(), "fixture has no {field}");
    assert!(
        BinanceWsEvent::parse(&value.to_string()).is_err(),
        "event parsed without required {field}"
    );
}

#[test]
fn rest_market_basics() {
    let t: ServerTime = parse(rest!("time"));
    assert!(t.server_time > 0);
    assert_requires::<ServerTime>(rest!("time"), "serverTime");

    let info: ExchangeInfo = parse(rest!("exchange_info"));
    assert_eq!(info.futures_type, "U_MARGINED");
    let btc = &info.symbols[0];
    assert_eq!(btc.symbol, "BTCUSDT");
    assert!(btc
        .filters
        .iter()
        .any(|f| matches!(f, SymbolFilter::PriceFilter { tick_size, .. } if tick_size == "0.10")));
    assert!(info
        .symbols
        .iter()
        .any(|s| s.contract_type == "TRADIFI_PERPETUAL"));
    assert_requires::<ExchangeInfo>(rest!("exchange_info"), "symbols");
    let mut symbol: Value = serde_json::from_str(rest!("exchange_info")).unwrap();
    let symbol = symbol["symbols"][0].take();
    assert_requires::<SymbolInfo>(&symbol.to_string(), "pricePrecision");
    let filter: SymbolFilter =
        parse(r#"{"filterType":"POSITION_RISK_CONTROL","positionControlSide":"NONE"}"#);
    assert!(matches!(
        filter,
        SymbolFilter::PositionRiskControl { position_control_side } if position_control_side == "NONE"
    ));
    assert!(
        serde_json::from_str::<SymbolFilter>(r#"{"filterType":"LOT_SIZE","minQty":"1"}"#).is_err()
    );
    assert!(
        serde_json::from_str::<SymbolFilter>(r#"{"filterType":"POSITION_RISK_CONTROL"}"#).is_err()
    );

    for text in [rest!("depth"), rest!("rpi_depth")] {
        let book: BinanceOrderBookSnapshot = parse(text);
        assert!(book.last_update_id > 0 && book.transaction_time > 0);
        assert_requires::<BinanceOrderBookSnapshot>(text, "E");
    }
}

#[test]
fn rest_trades() {
    let trades: Vec<BinanceTrade> = parse(rest!("trades"));
    assert!(!trades[0].is_rpi_trade);
    assert_requires::<Vec<BinanceTrade>>(rest!("trades"), "isRPITrade");

    let agg: Vec<BinanceAggTrade> = parse(rest!("agg_trades"));
    assert_eq!(agg[0].normal_quantity, "0.007");
    assert_requires::<Vec<BinanceAggTrade>>(rest!("agg_trades"), "nq");

    let err: BinanceApiError = parse(rest!("error_historical_trades_no_key"));
    assert_eq!(err.code, -2014);
}

#[test]
fn rest_klines() {
    for text in [
        rest!("klines"),
        rest!("continuous_klines"),
        rest!("index_price_klines"),
        rest!("mark_price_klines"),
        rest!("premium_index_klines"),
    ] {
        let klines: Vec<BinanceKline> = parse(text);
        assert_eq!(klines.len(), 2);
    }
    let premium: Vec<BinanceKline> = parse(rest!("premium_index_klines"));
    assert!(premium[0].low < 0.0);
    // Dropping the trailing "ignore" element must fail.
    assert!(serde_json::from_str::<BinanceKline>(
        r#"[1794823200000,"1","1","1","1","0",1794826799999,"0",720,"0","0"]"#
    )
    .is_err());
}

#[test]
fn rest_funding_and_prices() {
    for text in [rest!("premium_index_symbol")] {
        let p: BinancePremiumIndex = parse(text);
        assert_eq!(p.interest_rate, "0.00010000");
        assert_requires::<BinancePremiumIndex>(text, "estimatedSettlePrice");
    }
    let all: Vec<BinancePremiumIndex> = parse(rest!("premium_index_all"));
    assert_eq!(all.len(), 2);

    let rates: Vec<BinanceHistoricalFundingRate> = parse(rest!("funding_rate"));
    assert_eq!(rates[0].rate_type, "Regular");
    assert_requires::<Vec<BinanceHistoricalFundingRate>>(rest!("funding_rate"), "rateType");

    let info: Vec<FundingInfo> = parse(rest!("funding_info"));
    assert!(info[0].funding_interval_hours > 0);
    assert_requires::<Vec<FundingInfo>>(rest!("funding_info"), "fundingIntervalHours");
    assert!(info.iter().any(|i| i.update_time.is_none()));
    assert!(info.iter().any(|i| i.update_time.is_some()));
    assert_requires::<Vec<FundingInfo>>(rest!("funding_info"), "updateTime");

    let t: TickerData = parse(rest!("ticker_24hr_symbol"));
    assert_eq!(t.symbol, "BTCUSDT");
    assert_requires::<TickerData>(rest!("ticker_24hr_symbol"), "count");

    for text in [
        rest!("ticker_price_v1_symbol"),
        rest!("ticker_price_v2_symbol"),
    ] {
        let p: PriceTicker = parse(text);
        assert_eq!(p.symbol, "BTCUSDT");
        assert_requires::<PriceTicker>(text, "time");
    }
    let prices: Vec<PriceTicker> = parse(rest!("ticker_price_v2_all"));
    assert_eq!(prices.len(), 2);

    let b: BookTicker = parse(rest!("book_ticker_symbol"));
    assert!(b.last_update_id > 0);
    assert_requires::<BookTicker>(rest!("book_ticker_symbol"), "lastUpdateId");
    let all: Vec<BookTicker> = parse(rest!("book_ticker_all"));
    assert_eq!(all.len(), 2);
}

#[test]
fn rest_open_interest_and_statistics() {
    let oi: OpenInterestData = parse(rest!("open_interest"));
    assert_eq!(oi.symbol, "BTCUSDT");
    assert_requires::<OpenInterestData>(rest!("open_interest"), "openInterest");

    let d: Vec<DeliveryPrice> = parse(rest!("delivery_price"));
    assert!(d[0].delivery_price > 0.0);
    assert_requires::<Vec<DeliveryPrice>>(rest!("delivery_price"), "deliveryPrice");

    let h: Vec<OpenInterestHist> = parse(rest!("open_interest_hist"));
    assert!(!h[0].cmc_circulating_supply.is_empty());
    assert_requires::<Vec<OpenInterestHist>>(rest!("open_interest_hist"), "CMCCirculatingSupply");

    for text in [
        rest!("top_long_short_account_ratio"),
        rest!("top_long_short_position_ratio"),
        rest!("global_long_short_account_ratio"),
    ] {
        let r: Vec<LongShortRatio> = parse(text);
        assert_eq!(r.len(), 2);
        assert_requires::<Vec<LongShortRatio>>(text, "longShortRatio");
    }

    let t: Vec<TakerBuySellVolume> = parse(rest!("taker_long_short_ratio"));
    assert_eq!(t.len(), 2);
    assert_requires::<Vec<TakerBuySellVolume>>(rest!("taker_long_short_ratio"), "buyVol");

    let b: Vec<Basis> = parse(rest!("basis"));
    assert_eq!(b[0].annualized_basis_rate, "");
    assert_requires::<Vec<Basis>>(rest!("basis"), "basis");
}

#[test]
fn rest_indexes_and_insurance() {
    let i: Vec<CompositeIndexInfo> = parse(rest!("index_info"));
    assert_eq!(i[0].base_asset_list.len(), 3);
    assert_requires::<Vec<CompositeIndexInfo>>(rest!("index_info"), "baseAssetList");

    let a: AssetIndex = parse(rest!("asset_index_symbol"));
    assert_eq!(a.symbol, "BTCUSD");
    assert_requires::<AssetIndex>(rest!("asset_index_symbol"), "autoExchangeAskRate");
    let all: Vec<AssetIndex> = parse(rest!("asset_index_all"));
    assert_eq!(all.len(), 2);

    let c: IndexConstituents = parse(rest!("constituents"));
    assert!(!c.constituents.is_empty());
    assert_requires::<IndexConstituents>(rest!("constituents"), "constituents");

    let ins: InsuranceBalance = parse(rest!("insurance_balance_symbol"));
    assert!(ins.symbols.contains(&"BTCUSDT".to_string()));
    assert_requires::<InsuranceBalance>(rest!("insurance_balance_symbol"), "assets");
    let all: Vec<InsuranceBalance> = parse(rest!("insurance_balance_all"));
    assert_eq!(all.len(), 2);

    let adl: AdlRisk = parse(rest!("adl_risk_symbol"));
    assert_eq!(adl.adl_risk, "LOW");
    assert_requires::<AdlRisk>(rest!("adl_risk_symbol"), "adlRisk");
    let all: Vec<AdlRisk> = parse(rest!("adl_risk_all"));
    assert_eq!(all.len(), 2);

    let s: TradingSchedule = parse(rest!("trading_schedule"));
    assert_eq!(s.market_schedules.len(), 2);
    assert_requires::<TradingSchedule>(rest!("trading_schedule"), "marketSchedules");
}

#[test]
fn ws_control_frames() {
    match event(ws!("ack_subscribe")) {
        BinanceWsEvent::Response(r) => {
            assert!(r.result.is_null());
            assert_eq!(r.id, Some(1));
        }
        other => panic!("{other:?}"),
    }
    match event(ws!("ack_list_subscriptions")) {
        BinanceWsEvent::Response(r) => assert_eq!(r.result[0], "!markPrice@arr"),
        other => panic!("{other:?}"),
    }
    match event(ws!("error_invalid_request")) {
        BinanceWsEvent::Error(e) => assert_eq!(e.error.code, 2),
        other => panic!("{other:?}"),
    }
    match event(ws!("combined_agg_trade")) {
        BinanceWsEvent::AggTrade(t) => assert_eq!(t.symbol, "BTCUSDT"),
        other => panic!("{other:?}"),
    }
    assert!(BinanceWsEvent::parse(r#"{"e":"somethingNew","E":1}"#).is_err());
}

#[test]
fn ws_public_streams() {
    for text in [
        ws!("depth"),
        ws!("depth5"),
        ws!("depth10_100ms"),
        ws!("depth20_500ms"),
        ws!("rpiDepth_500ms"),
    ] {
        match event(text) {
            BinanceWsEvent::Depth(d) => assert_eq!(d.pair, "BTCUSDT"),
            other => panic!("{other:?}"),
        }
        assert_event_requires(text, "pu");
    }
    for text in [ws!("bookTicker"), ws!("all_bookTicker")] {
        match event(text) {
            BinanceWsEvent::BookTicker(b) => {
                assert!(b.transaction_time.is_some() && b.symbol_type == Some(1))
            }
            other => panic!("{other:?}"),
        }
        assert_event_requires(text, "b");
    }
    match event(ws!("trade")) {
        BinanceWsEvent::Trade(t) => assert_eq!(t.order_type.as_deref(), Some("MARKET")),
        other => panic!("{other:?}"),
    }
    assert_event_requires(ws!("trade"), "t");
}

#[test]
fn ws_market_streams() {
    match event(ws!("aggTrade")) {
        BinanceWsEvent::AggTrade(t) => assert_eq!(t.normal_quantity, "0.001"),
        other => panic!("{other:?}"),
    }
    assert_event_requires(ws!("aggTrade"), "nq");

    for text in [ws!("markPrice"), ws!("markPrice_1s")] {
        assert!(matches!(event(text), BinanceWsEvent::MarkPrice(_)));
        assert_event_requires(text, "r");
    }
    for text in [ws!("all_markPrice_arr"), ws!("all_markPrice_arr_1s")] {
        match event(text) {
            BinanceWsEvent::MarkPrices(v) => assert_eq!(v.len(), 2),
            other => panic!("{other:?}"),
        }
        assert_event_requires(text, "P");
    }

    match event(ws!("kline_1m")) {
        BinanceWsEvent::Kline(k) => assert_eq!(k.kline.interval, "1m"),
        other => panic!("{other:?}"),
    }
    assert_event_requires(ws!("kline_1m"), "k");
    for text in [
        ws!("perpetual_continuousKline_1m"),
        ws!("current_quarter_continuousKline_1m"),
    ] {
        match event(text) {
            BinanceWsEvent::ContinuousKline(k) => assert_eq!(k.pair, "BTCUSDT"),
            other => panic!("{other:?}"),
        }
        assert_event_requires(text, "ct");
    }

    assert!(matches!(
        event(ws!("miniTicker")),
        BinanceWsEvent::MiniTicker(_)
    ));
    assert_event_requires(ws!("miniTicker"), "q");
    match event(ws!("all_miniTicker_arr")) {
        BinanceWsEvent::MiniTickers(v) => assert_eq!(v.len(), 2),
        other => panic!("{other:?}"),
    }
    assert!(matches!(event(ws!("ticker")), BinanceWsEvent::Ticker(_)));
    assert_event_requires(ws!("ticker"), "n");
    match event(ws!("all_ticker_arr")) {
        BinanceWsEvent::Tickers(v) => assert_eq!(v.len(), 2),
        other => panic!("{other:?}"),
    }

    match event(ws!("all_forceOrder_arr")) {
        BinanceWsEvent::ForceOrder(f) => assert_eq!(f.order.status, "FILLED"),
        other => panic!("{other:?}"),
    }
    assert_event_requires(ws!("all_forceOrder_arr"), "o");

    match event(ws!("allusdt_compositeIndex")) {
        BinanceWsEvent::CompositeIndex(c) => assert_eq!(c.composition.len(), 3),
        other => panic!("{other:?}"),
    }
    assert_event_requires(ws!("allusdt_compositeIndex"), "c");

    assert!(matches!(
        event(ws!("assetIndex")),
        BinanceWsEvent::AssetIndex(_)
    ));
    assert_event_requires(ws!("assetIndex"), "G");
    match event(ws!("all_assetIndex_arr")) {
        BinanceWsEvent::AssetIndexes(v) => assert_eq!(v.len(), 2),
        other => panic!("{other:?}"),
    }

    match event(ws!("tradingSession")) {
        BinanceWsEvent::TradingSession(s) => assert_eq!(s.session_type, "REGULAR"),
        other => panic!("{other:?}"),
    }
    assert_event_requires(ws!("tradingSession"), "S");
}

/// No `!contractInfo` push arrived during a 200s capture; this is the documented example.
#[test]
fn ws_contract_info_doc_example() {
    let text = r#"{"e":"contractInfo","E":1669356423908,"s":"IOTAUSDT","ps":"IOTAUSDT","ct":"PERPETUAL","dt":4133404800000,"ot":1569398400000,"cs":"TRADING","bks":[{"bs":1,"bnf":0,"bnc":5000,"mmr":0.01,"cf":0,"mi":21,"ma":50},{"bs":2,"bnf":5000,"bnc":25000,"mmr":0.025,"cf":75.0,"mi":11,"ma":20}]}"#;
    match event(text) {
        BinanceWsEvent::ContractInfo(c) => assert_eq!(c.brackets.unwrap().len(), 2),
        other => panic!("{other:?}"),
    }
    assert_event_requires(text, "cs");
}
