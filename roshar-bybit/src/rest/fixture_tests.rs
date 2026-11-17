//! Offline parse tests over live REST captures in `tests/fixtures/rest` (2026-11-16, UK IP).

use super::market::parse_response;
use super::*;
use crate::types::ByBitHistoricalFundingRate;
use serde::de::DeserializeOwned;

macro_rules! fixture {
    ($name:literal) => {
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/rest/",
            $name,
            ".json"
        ))
    };
}

fn parse<T: DeserializeOwned>(body: &str) -> T {
    parse_response::<T>(body).unwrap_or_else(|e| panic!("{e}"))
}

/// Removes `key` from the first object found at `pointer` and checks parsing then fails.
fn assert_required<T: DeserializeOwned>(body: &str, pointer: &str, key: &str) {
    let mut v: serde_json::Value = serde_json::from_str(body).unwrap();
    v.pointer_mut(pointer)
        .and_then(|o| o.as_object_mut())
        .unwrap_or_else(|| panic!("no object at {pointer}"))
        .remove(key)
        .unwrap_or_else(|| panic!("no key {key} at {pointer}"));
    assert!(
        parse_response::<T>(&v.to_string()).is_err(),
        "missing {pointer}/{key} must be an error"
    );
}

#[test]
fn server_time() {
    let t: ByBitServerTime = parse(fixture!("time"));
    assert_eq!(t.time_second, "1794826997");
    assert_required::<ByBitServerTime>(fixture!("time"), "/result", "timeNano");
}

#[test]
fn system_status() {
    let s: ByBitSystemStatusResult = parse(fixture!("system_status"));
    assert!(s.list.is_empty());
    // No maintenance record was live; the documented example shows the row shape.
    let s: ByBitSystemStatusResult = parse(fixture!("docs_system_status"));
    assert_eq!(s.list[0].service_types, vec![2, 3, 4, 5]);
    assert_required::<ByBitSystemStatusResult>(
        fixture!("docs_system_status"),
        "/result/list/0",
        "maintainType",
    );
}

#[test]
fn error_envelope_surfaces_code_and_message() {
    let err = parse_response::<ByBitOrderbook>(fixture!("error_invalid_symbol")).unwrap_err();
    assert_eq!(
        err.to_string(),
        "ByBit API error 10001: params error: symbol invalid"
    );
}

#[test]
fn klines() {
    for (body, n) in [
        (fixture!("kline_spot"), "spot"),
        (fixture!("kline_linear"), "linear"),
        (fixture!("kline_inverse"), "inverse"),
    ] {
        let k: ByBitKlineResult = parse(body);
        assert_eq!(k.category, n);
        assert_eq!(k.list.len(), 3);
        assert!(k.list[0].start_time > k.list[1].start_time);
    }
    let k: ByBitKlineResult = parse(fixture!("kline_linear"));
    assert_eq!(k.list[0].turnover, "11670853.4821");

    for body in [
        fixture!("mark_price_kline_linear"),
        fixture!("mark_price_kline_option"),
        fixture!("index_price_kline_inverse"),
        fixture!("premium_index_price_kline"),
    ] {
        let k: ByBitPriceKlineResult = parse(body);
        assert_eq!(k.list.len(), 3);
    }
    // A 7-element row is not a price kline and a 5-element row is not a trade kline.
    assert!(parse_response::<ByBitPriceKlineResult>(fixture!("kline_linear")).is_err());
    assert!(parse_response::<ByBitKlineResult>(fixture!("mark_price_kline_linear")).is_err());
    assert_required::<ByBitKlineResult>(fixture!("kline_spot"), "/result", "symbol");
}

#[test]
fn contract_instruments() {
    for body in [
        fixture!("instruments_linear"),
        fixture!("instruments_inverse"),
        fixture!("instruments_linear_closed"),
        fixture!("instruments_linear_futures_stock"),
    ] {
        let r: ByBitInstrumentsResult<ByBitInstrumentInfo> = parse(body);
        assert!(!r.list.is_empty());
    }
    let r: ByBitInstrumentsResult<ByBitInstrumentInfo> =
        parse(fixture!("instruments_linear_closed"));
    assert_eq!(r.list[0].status, "Closed");
    assert!(!r.next_page_cursor.is_empty());
    assert!(r.list[0].pre_listing_info.is_none());
    assert_required::<ByBitInstrumentsResult<ByBitInstrumentInfo>>(
        fixture!("instruments_linear"),
        "/result/list/0",
        "preListingInfo",
    );
    assert_required::<ByBitInstrumentsResult<ByBitInstrumentInfo>>(
        fixture!("instruments_linear"),
        "/result/list/0/lotSizeFilter",
        "postOnlyMaxOrderQty",
    );
}

#[test]
fn spot_instruments() {
    let r: ByBitSpotInstrumentsResult = parse(fixture!("instruments_spot"));
    assert_eq!(
        r.list[0].lot_size_filter.post_only_max_limit_order_size,
        "1150"
    );
    assert_required::<ByBitSpotInstrumentsResult>(
        fixture!("instruments_spot"),
        "/result/list/0",
        "xstockMultiplier",
    );
    // Spot rows are not contract rows.
    assert!(
        parse_response::<ByBitInstrumentsResult<ByBitInstrumentInfo>>(fixture!("instruments_spot"))
            .is_err()
    );
}

#[test]
fn option_instruments() {
    let r: ByBitInstrumentsResult<ByBitOptionInstrumentInfo> =
        parse(fixture!("instruments_option"));
    assert_eq!(r.list[0].options_type, "Put");
    assert_eq!(r.next_page_cursor, "0%2C3");
    assert_required::<ByBitInstrumentsResult<ByBitOptionInstrumentInfo>>(
        fixture!("instruments_option"),
        "/result/list/0",
        "optionsType",
    );
}

#[test]
fn instrument_summary_option_base_coins_corporate_actions() {
    for body in [
        fixture!("instrument_summary_linear"),
        fixture!("instrument_summary_option"),
    ] {
        let r: ByBitInstrumentSummaryResult = parse(body);
        assert_eq!(r.list.len(), 3);
    }
    assert_required::<ByBitInstrumentSummaryResult>(
        fixture!("instrument_summary_option"),
        "/result/list/0",
        "priceIndex",
    );

    let r: ByBitOptionBaseCoinsResult = parse(fixture!("option_base_coins"));
    assert_eq!(r.list[0].base_coin, "BTC");
    assert_required::<ByBitOptionBaseCoinsResult>(
        fixture!("option_base_coins"),
        "/result/list/0",
        "hasSymbol",
    );

    let r: ByBitCorporateActionResult = parse(fixture!("corporate_action"));
    assert_eq!(r.list[0].details.split_factor.as_deref(), Some("8"));
    assert!(r.list[0].details.dividend_amount_per_share.is_none());
    assert_required::<ByBitCorporateActionResult>(
        fixture!("corporate_action"),
        "/result/list/0",
        "effectiveTime",
    );
}

#[test]
fn orderbooks() {
    for body in [
        fixture!("orderbook_spot"),
        fixture!("orderbook_linear"),
        fixture!("orderbook_option"),
        fixture!("full_orderbook_linear"),
        fixture!("spread_orderbook"),
    ] {
        let ob: ByBitOrderbook = parse(body);
        assert!(!ob.b.is_empty() && !ob.a.is_empty());
    }
    let rpi: ByBitRpiOrderbook = parse(fixture!("rpi_orderbook_linear"));
    assert_eq!(rpi.a[0][2], "0.000");
    // RPI levels carry three values; a plain book does not.
    assert!(parse_response::<ByBitRpiOrderbook>(fixture!("orderbook_linear")).is_err());
    assert_required::<ByBitOrderbook>(fixture!("orderbook_linear"), "/result", "cts");
}

#[test]
fn tickers() {
    for body in [
        fixture!("tickers_linear"),
        fixture!("tickers_inverse"),
        fixture!("tickers_linear_futures_prelisting"),
    ] {
        let r: ByBitTickersResult<ByBitTickerData> = parse(body);
        assert!(!r.list.is_empty());
    }
    assert_required::<ByBitTickersResult<ByBitTickerData>>(
        fixture!("tickers_linear"),
        "/result/list/0",
        "fundingRate",
    );

    let r: ByBitTickersResult<ByBitSpotTicker> = parse(fixture!("tickers_spot"));
    assert!(r.list[0].usd_index_price.is_some());
    let r: ByBitTickersResult<ByBitSpotTicker> = parse(fixture!("tickers_spot_no_usd_index"));
    assert!(r.list[0].usd_index_price.is_none());
    assert_required::<ByBitTickersResult<ByBitSpotTicker>>(
        fixture!("tickers_spot"),
        "/result/list/0",
        "bid1Price",
    );

    let r: ByBitTickersResult<ByBitOptionTicker> = parse(fixture!("tickers_option"));
    assert_eq!(r.category, "option");
    assert_required::<ByBitTickersResult<ByBitOptionTicker>>(
        fixture!("tickers_option"),
        "/result/list/0",
        "markIv",
    );

    let r: ByBitSpreadTickersResult = parse(fixture!("spread_tickers"));
    assert_eq!(r.list[0].bid_price, "-50");
    assert_required::<ByBitSpreadTickersResult>(
        fixture!("spread_tickers"),
        "/result/list/0",
        "volume24h",
    );
}

#[test]
fn recent_trades() {
    let r: ByBitRecentTradesResult = parse(fixture!("recent_trade_linear"));
    assert_eq!(r.list[0].is_rpi_trade, Some(false));
    assert!(r.list[0].m_p.is_none());
    let _: ByBitRecentTradesResult = parse(fixture!("recent_trade_spot"));
    let r: ByBitRecentTradesResult = parse(fixture!("recent_trade_option"));
    assert!(r.list[0].is_rpi_trade.is_none());
    assert_eq!(r.list[0].m_iv.as_deref(), Some("0.3809"));
    assert_required::<ByBitRecentTradesResult>(
        fixture!("recent_trade_linear"),
        "/result/list/0",
        "execId",
    );
}

#[test]
fn spread_recent_trades_and_instruments() {
    // Live call returned no trades; the documented example shows the row shape.
    let body = fixture!("docs_spread_recent_trade");
    let r: ByBitSpreadRecentTradesResult = parse(body);
    assert_eq!(r.list[0].side, "Sell");
    assert_required::<ByBitSpreadRecentTradesResult>(body, "/result/list/0", "seq");
    let empty: ByBitSpreadRecentTradesResult = parse(fixture!("spread_recent_trade_empty"));
    assert!(empty.list.is_empty());

    let r: ByBitSpreadInstrumentsResult = parse(fixture!("spread_instrument"));
    assert_eq!(r.list[0].legs.len(), 2);
    assert_required::<ByBitSpreadInstrumentsResult>(
        fixture!("spread_instrument"),
        "/result/list/0",
        "legs",
    );
}

#[test]
fn funding_open_interest_volatility_ratio() {
    let r: ByBitFundingHistoryResult = parse(fixture!("funding_history_inverse"));
    assert_eq!(r.category, "inverse");
    assert_required::<ByBitFundingHistoryResult>(
        fixture!("funding_history_inverse"),
        "/result/list/0",
        "fundingRate",
    );
    let _: Vec<ByBitHistoricalFundingRate> = r.list;

    let r: ByBitOpenInterestResult = parse(fixture!("open_interest_linear"));
    assert_eq!(r.list.len(), 3);
    assert_required::<ByBitOpenInterestResult>(
        fixture!("open_interest_linear"),
        "/result/list/0",
        "singleOpenInterest",
    );

    let r: Vec<ByBitHistoricalVolatility> = parse(fixture!("historical_volatility"));
    assert_eq!(r[0].period, 7);
    assert_required::<Vec<ByBitHistoricalVolatility>>(
        fixture!("historical_volatility"),
        "/result/0",
        "value",
    );

    let r: ByBitLongShortRatioResult = parse(fixture!("account_ratio"));
    assert_eq!(r.list[0].buy_ratio, "0.72");
    assert_required::<ByBitLongShortRatioResult>(
        fixture!("account_ratio"),
        "/result/list/0",
        "sellRatio",
    );
}

#[test]
fn insurance_adl_risk_delivery_index_price_limit() {
    let r: ByBitInsuranceResult = parse(fixture!("insurance"));
    assert_eq!(r.list[0].coin, "BTC");
    assert_required::<ByBitInsuranceResult>(fixture!("insurance"), "/result/list/0", "value");

    let r: ByBitAdlAlertResult = parse(fixture!("adl_alert"));
    assert_eq!(r.list[0].max_balance, "");
    assert_required::<ByBitAdlAlertResult>(fixture!("adl_alert"), "/result/list/0", "adlStopRatio");

    let r: ByBitRiskLimitResult = parse(fixture!("risk_limit_linear"));
    assert_eq!(r.list[0].is_lowest_risk, 1);
    assert_required::<ByBitRiskLimitResult>(
        fixture!("risk_limit_linear"),
        "/result/list/0",
        "maintenanceMargin",
    );

    for body in [
        fixture!("delivery_price_option"),
        fixture!("delivery_price_linear"),
    ] {
        let r: ByBitDeliveryPriceResult = parse(body);
        assert_eq!(r.list.len(), 3);
    }
    assert_required::<ByBitDeliveryPriceResult>(
        fixture!("delivery_price_linear"),
        "/result/list/0",
        "deliveryPrice",
    );

    let r: ByBitNewDeliveryPriceResult = parse(fixture!("new_delivery_price"));
    assert_eq!(r.category, "option");
    assert_required::<ByBitNewDeliveryPriceResult>(
        fixture!("new_delivery_price"),
        "/result/list/0",
        "deliveryTime",
    );

    let r: ByBitIndexPriceComponents = parse(fixture!("index_price_components"));
    assert_eq!(r.components[0].exchange, "Bybit");
    assert_required::<ByBitIndexPriceComponents>(
        fixture!("index_price_components"),
        "/result/components/0",
        "weight",
    );

    let r: ByBitOrderPriceLimit = parse(fixture!("price_limit"));
    assert_eq!(r.buy_lmt, "65217.40");
    assert_required::<ByBitOrderPriceLimit>(fixture!("price_limit"), "/result", "sellLmt");
}
