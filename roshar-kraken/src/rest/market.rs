use roshar_http::RateLimitedClient;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

use super::parse_response;

const BASE_URL: &str = "https://futures.kraken.com/derivatives/api/v3";

/// For fields that are always sent but may be `null`: unlike a plain `Option`, a missing key is
/// an error.
fn nullable<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}

/// `contractType` filter of `tickers`, `instruments` and `instruments/status`. Without a filter
/// Kraken returns all futures types but no options.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KrakenContractType {
    FuturesInverse,
    FuturesVanilla,
    FlexibleFutures,
    Options,
    All,
}

impl KrakenContractType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::FuturesInverse => "futures_inverse",
            Self::FuturesVanilla => "futures_vanilla",
            Self::FlexibleFutures => "flexible_futures",
            Self::Options => "options",
            Self::All => "all",
        }
    }
}

/// Greeks of an options ticker ("OptionGreeks" in the OpenAPI spec). `iv` is -1.0 when Kraken
/// cannot compute it.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct KrakenOptionGreeks {
    pub iv: f64,
    pub delta: f64,
    pub gamma: Option<f64>,
    pub vega: Option<f64>,
    pub theta: Option<f64>,
    pub rho: Option<f64>,
}

/// One market of `GET /derivatives/api/v3/tickers` or `tickers/{symbol}` ("Market Ticker" in the
/// OpenAPI spec). Fields the spec marks optional, or that are absent live for some market types,
/// are `Option`.
///
/// The spec also lists index tickers (`in_*`/`rr_*`, `symbol`/`last`/`lastTime` only); Kraken
/// returned none on 2026-11-16 (`tickers/rr_xbtusd` is `contractNotFound`), so they are not
/// modelled.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct KrakenTickerData {
    pub symbol: String,
    /// `perpetual`, `day`, `week`, `month`, `quarter` or `semiannual`.
    pub tag: String,
    pub pair: String,
    #[serde(rename = "markPrice")]
    pub mark_price: f64,
    #[serde(rename = "indexPrice")]
    pub index_price: f64,
    pub last: Option<f64>,
    #[serde(rename = "lastTime")]
    pub last_time: Option<String>,
    #[serde(rename = "lastSize")]
    pub last_size: Option<f64>,
    pub bid: Option<f64>,
    #[serde(rename = "bidSize")]
    pub bid_size: Option<f64>,
    pub ask: Option<f64>,
    #[serde(rename = "askSize")]
    pub ask_size: Option<f64>,
    #[serde(rename = "vol24h")]
    pub vol_24h: f64,
    #[serde(rename = "volumeQuote")]
    pub volume_quote: f64,
    #[serde(rename = "openInterest")]
    pub open_interest: f64,
    #[serde(rename = "open24h")]
    pub open_24h: Option<f64>,
    #[serde(rename = "high24h")]
    pub high_24h: Option<f64>,
    #[serde(rename = "low24h")]
    pub low_24h: Option<f64>,
    /// Sent live, not in the spec.
    #[serde(rename = "vwap24h")]
    pub vwap_24h: Option<f64>,
    /// Perpetuals only.
    #[serde(rename = "fundingRate")]
    pub funding_rate: Option<f64>,
    /// Perpetuals only.
    #[serde(rename = "fundingRatePrediction")]
    pub funding_rate_prediction: Option<f64>,
    #[serde(rename = "relativeFundingRate")]
    pub relative_funding_rate: Option<f64>,
    #[serde(rename = "relativeFundingRatePrediction")]
    pub relative_funding_rate_prediction: Option<f64>,
    pub suspended: bool,
    #[serde(rename = "postOnly")]
    pub post_only: bool,
    /// 24h change in percent.
    #[serde(rename = "change24h")]
    pub change_24h: f64,
    /// Tradfi markets only.
    #[serde(rename = "isUnderlyingMarketClosed")]
    pub is_underlying_market_closed: Option<bool>,
    /// Options only.
    pub greeks: Option<KrakenOptionGreeks>,
    /// Options only: mark price less intrinsic value.
    #[serde(rename = "extrinsicValue")]
    pub extrinsic_value: Option<f64>,
    /// Options only: implied volatility of the mark price. Sent live, not in the spec.
    #[serde(rename = "markIv")]
    pub mark_iv: Option<f64>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct KrakenTickerResponse {
    pub result: String,
    pub tickers: Vec<KrakenTickerData>,
    #[serde(rename = "serverTime")]
    pub server_time: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct KrakenSingleTickerResponse {
    ticker: KrakenTickerData,
}

/// One margin tier. Inverse futures send `contracts`, linear ones `numNonContractUnits`.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct KrakenMarginLevel {
    pub contracts: Option<i64>,
    #[serde(rename = "numNonContractUnits")]
    pub num_non_contract_units: Option<f64>,
    #[serde(rename = "initialMargin")]
    pub initial_margin: f64,
    #[serde(rename = "maintenanceMargin")]
    pub maintenance_margin: f64,
}

/// Margin tiers of one platform in `marginSchedules`.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct KrakenMarginSchedule {
    pub retail: Vec<KrakenMarginLevel>,
    pub professional: Vec<KrakenMarginLevel>,
}

/// One entry of `GET /derivatives/api/v3/instruments` ("Instrument" in the OpenAPI spec).
/// Required fields are those sent for every instrument type live (current and expired,
/// 2026-11-16); the rest depend on the type and are `Option`.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct KrakenInstrument {
    pub symbol: String,
    /// `flexible_futures`, `futures_inverse`, `futures_vanilla` or `options`.
    #[serde(rename = "type")]
    pub kind: String,
    pub tradeable: bool,
    #[serde(rename = "tickSize")]
    pub tick_size: f64,
    #[serde(rename = "contractSize")]
    pub contract_size: f64,
    #[serde(rename = "contractValueTradePrecision")]
    pub contract_value_trade_precision: i64,
    #[serde(rename = "openingDate")]
    pub opening_date: String,
    #[serde(rename = "postOnly")]
    pub post_only: bool,
    pub suspended: bool,
    pub mtf: bool,
    pub tradfi: bool,
    #[serde(rename = "isExpired")]
    pub is_expired: bool,
    pub base: String,
    pub quote: String,
    pub pair: String,
    #[serde(rename = "platformsPermitted")]
    pub platforms_permitted: Vec<String>,
    #[serde(rename = "countriesBanned")]
    pub countries_banned: Vec<String>,
    /// Always empty live.
    pub tags: Vec<String>,
    /// Market share level to rebate percentage.
    #[serde(rename = "rebateLevels")]
    pub rebate_levels: HashMap<String, f64>,
    /// Single-collateral markets: underlying index (e.g. `rr_xbtusd`).
    pub underlying: Option<String>,
    /// Options: the underlying future.
    #[serde(rename = "underlyingFuture")]
    pub underlying_future: Option<String>,
    /// Options: `call` or `put`.
    #[serde(rename = "optionType")]
    pub option_type: Option<String>,
    #[serde(rename = "strikePrice")]
    pub strike_price: Option<f64>,
    #[serde(rename = "lastTradingTime")]
    pub last_trading_time: Option<String>,
    #[serde(rename = "impactMidSize")]
    pub impact_mid_size: Option<f64>,
    #[serde(rename = "maxPositionSize")]
    pub max_position_size: Option<f64>,
    #[serde(rename = "maxOpenInterestUsd")]
    pub max_open_interest_usd: Option<f64>,
    #[serde(rename = "maxOpenInterestShare")]
    pub max_open_interest_share: Option<f64>,
    #[serde(rename = "fundingRateCoefficient")]
    pub funding_rate_coefficient: Option<f64>,
    #[serde(rename = "maxRelativeFundingRate")]
    pub max_relative_funding_rate: Option<f64>,
    #[serde(rename = "minRelativeFundingRate")]
    pub min_relative_funding_rate: Option<f64>,
    #[serde(rename = "makerProtectionMillis")]
    pub maker_protection_millis: Option<i64>,
    /// Absent for some markets.
    pub isin: Option<String>,
    pub category: Option<String>,
    pub description: Option<String>,
    /// Deprecated by Kraken.
    #[serde(rename = "feeScheduleUid")]
    pub fee_schedule_uid: Option<String>,
    /// Professional clients. Futures only.
    #[serde(rename = "marginLevels")]
    pub margin_levels: Option<Vec<KrakenMarginLevel>>,
    /// Futures only.
    #[serde(rename = "retailMarginLevels")]
    pub retail_margin_levels: Option<Vec<KrakenMarginLevel>>,
    /// Margin schedules by platform. Futures only.
    #[serde(rename = "marginSchedules")]
    pub margin_schedules: Option<HashMap<String, KrakenMarginSchedule>>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct KrakenInstrumentsResponse {
    instruments: Vec<KrakenInstrument>,
}

/// Price dislocation and volatility state of one market (`instruments/status`).
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct KrakenInstrumentStatus {
    pub tradeable: String,
    #[serde(rename = "experiencingDislocation")]
    pub experiencing_dislocation: bool,
    /// `ABOVE_UPPER_BOUND`, `BELOW_LOWER_BOUND` or `null`. The key is required.
    #[serde(rename = "priceDislocationDirection", deserialize_with = "nullable")]
    pub price_dislocation_direction: Option<String>,
    #[serde(rename = "experiencingExtremeVolatility")]
    pub experiencing_extreme_volatility: bool,
    #[serde(rename = "extremeVolatilityInitialMarginMultiplier")]
    pub extreme_volatility_initial_margin_multiplier: i64,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct KrakenInstrumentStatusListResponse {
    #[serde(rename = "instrumentStatus")]
    instrument_status: Vec<KrakenInstrumentStatus>,
}

/// Full non-cumulative book (`orderbook`). Levels are `[price, size]`; bids descending, asks
/// ascending.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct KrakenOrderBook {
    pub bids: Vec<[f64; 2]>,
    pub asks: Vec<[f64; 2]>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct KrakenOrderBookResponse {
    #[serde(rename = "orderBook")]
    order_book: KrakenOrderBook,
}

/// One trade of `GET /derivatives/api/v3/history` ("HistoryJson" in the spec). The MTF
/// transparency fields are sent only with `includeMTFData=true`.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct KrakenHistoryTrade {
    /// RFC 3339.
    pub time: String,
    pub trade_id: i64,
    pub price: f64,
    /// The spec says string; sent as a number.
    pub size: f64,
    /// Taker side: `buy` or `sell`.
    pub side: String,
    /// `fill`, `liquidation`, `partial liquidation`, `assignment`, `unwind`, `rfq`,
    /// `termination` or `block`.
    #[serde(rename = "type")]
    pub kind: String,
    pub uid: String,
    /// Per-market execution sequence number, as a decimal string.
    pub sequence_id: String,
    pub instrument_identification_type: Option<String>,
    pub isin: Option<String>,
    pub execution_venue: Option<String>,
    pub price_notation: Option<String>,
    pub price_currency: Option<String>,
    pub notional_amount: Option<f64>,
    pub notional_currency: Option<String>,
    pub publication_time: Option<String>,
    pub publication_venue: Option<String>,
    pub transaction_identification_code: Option<String>,
    pub to_be_cleared: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct KrakenHistoryResponse {
    history: Vec<KrakenHistoryTrade>,
}

/// One funding period of `historical-funding-rates`.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct KrakenHistoricalFundingRate {
    /// RFC 3339 start of the period.
    pub timestamp: String,
    #[serde(rename = "fundingRate")]
    pub funding_rate: f64,
    #[serde(rename = "relativeFundingRate")]
    pub relative_funding_rate: f64,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct KrakenHistoricalFundingRatesResponse {
    rates: Vec<KrakenHistoricalFundingRate>,
}

fn push_contract_types(
    query: &mut Vec<(&'static str, String)>,
    types: Option<&[KrakenContractType]>,
) {
    for t in types.unwrap_or_default() {
        query.push(("contractType", t.as_str().to_string()));
    }
}

fn with_query(
    path: &str,
    query: &[(&'static str, String)],
) -> Result<String, Box<dyn std::error::Error>> {
    let mut url = format!("{BASE_URL}{path}");
    if !query.is_empty() {
        url.push('?');
        url.push_str(&serde_urlencoded::to_string(query)?);
    }
    Ok(url)
}

pub struct MarketApi {
    client: Arc<RateLimitedClient>,
}

impl MarketApi {
    pub fn new_with_client(client: Arc<RateLimitedClient>) -> Self {
        Self { client }
    }

    async fn get<R: serde::de::DeserializeOwned>(
        &self,
        url: &str,
    ) -> Result<R, Box<dyn std::error::Error>> {
        let text = self.client.get(url).await?.text().await?;
        parse_response(&text)
    }

    /// `GET /tickers`, keyed by symbol. `None` filters are omitted.
    pub async fn get_tickers(
        &self,
        contract_types: Option<&[KrakenContractType]>,
        symbols: Option<&[&str]>,
    ) -> Result<HashMap<String, KrakenTickerData>, Box<dyn std::error::Error>> {
        let mut query = Vec::new();
        push_contract_types(&mut query, contract_types);
        for s in symbols.unwrap_or_default() {
            query.push(("symbol", s.to_string()));
        }
        let url = with_query("/tickers", &query)?;
        let ticker_response: KrakenTickerResponse = self.get(&url).await?;

        let mut tickers = HashMap::with_capacity(ticker_response.tickers.len());
        for ticker in ticker_response.tickers {
            tickers.insert(ticker.symbol.clone(), ticker);
        }
        Ok(tickers)
    }

    /// `GET /tickers/{symbol}`.
    pub async fn get_ticker(
        &self,
        symbol: &str,
    ) -> Result<KrakenTickerData, Box<dyn std::error::Error>> {
        let r: KrakenSingleTickerResponse =
            self.get(&format!("{BASE_URL}/tickers/{symbol}")).await?;
        Ok(r.ticker)
    }

    /// `GET /instruments`. `expired: Some(true)` returns only expired instruments.
    pub async fn get_instruments(
        &self,
        contract_types: Option<&[KrakenContractType]>,
        expired: Option<bool>,
    ) -> Result<Vec<KrakenInstrument>, Box<dyn std::error::Error>> {
        let mut query = Vec::new();
        push_contract_types(&mut query, contract_types);
        if let Some(expired) = expired {
            query.push(("expired", expired.to_string()));
        }
        let url = with_query("/instruments", &query)?;
        let r: KrakenInstrumentsResponse = self.get(&url).await?;
        Ok(r.instruments)
    }

    /// `GET /instruments/status`.
    pub async fn get_instruments_status(
        &self,
        contract_types: Option<&[KrakenContractType]>,
    ) -> Result<Vec<KrakenInstrumentStatus>, Box<dyn std::error::Error>> {
        let mut query = Vec::new();
        push_contract_types(&mut query, contract_types);
        let url = with_query("/instruments/status", &query)?;
        let r: KrakenInstrumentStatusListResponse = self.get(&url).await?;
        Ok(r.instrument_status)
    }

    /// `GET /instruments/{symbol}/status`.
    pub async fn get_instrument_status(
        &self,
        symbol: &str,
    ) -> Result<KrakenInstrumentStatus, Box<dyn std::error::Error>> {
        self.get(&format!("{BASE_URL}/instruments/{symbol}/status"))
            .await
    }

    /// `GET /orderbook?symbol=`.
    pub async fn get_orderbook(
        &self,
        symbol: &str,
    ) -> Result<KrakenOrderBook, Box<dyn std::error::Error>> {
        let url = with_query("/orderbook", &[("symbol", symbol.to_string())])?;
        let r: KrakenOrderBookResponse = self.get(&url).await?;
        Ok(r.order_book)
    }

    /// `GET /history?symbol=&lastTime=&includeMTFData=`: up to 100 trades before `last_time`
    /// (RFC 3339), newest first.
    pub async fn get_trade_history(
        &self,
        symbol: &str,
        last_time: Option<&str>,
        include_mtf_data: Option<bool>,
    ) -> Result<Vec<KrakenHistoryTrade>, Box<dyn std::error::Error>> {
        let mut query = vec![("symbol", symbol.to_string())];
        if let Some(t) = last_time {
            query.push(("lastTime", t.to_string()));
        }
        if let Some(b) = include_mtf_data {
            query.push(("includeMTFData", b.to_string()));
        }
        let url = with_query("/history", &query)?;
        let r: KrakenHistoryResponse = self.get(&url).await?;
        Ok(r.history)
    }

    /// `GET /historical-funding-rates?symbol=` (perpetuals only), oldest first.
    pub async fn get_historical_funding_rates(
        &self,
        symbol: &str,
    ) -> Result<Vec<KrakenHistoricalFundingRate>, Box<dyn std::error::Error>> {
        let url = with_query(
            "/historical-funding-rates",
            &[("symbol", symbol.to_string())],
        )?;
        let r: KrakenHistoricalFundingRatesResponse = self.get(&url).await?;
        Ok(r.rates)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> String {
        std::fs::read_to_string(format!(
            "{}/tests/fixtures/{name}",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap()
    }

    fn without(text: &str, path: &[&str], key: &str) -> String {
        let mut v: serde_json::Value = serde_json::from_str(text).unwrap();
        let mut node = &mut v;
        for p in path {
            node = match p.parse::<usize>() {
                Ok(i) => &mut node[i],
                Err(_) => &mut node[*p],
            };
        }
        node.as_object_mut()
            .unwrap()
            .remove(key)
            .expect("key present");
        v.to_string()
    }

    #[test]
    fn parses_live_tickers() {
        // Captured from GET /derivatives/api/v3/tickers on 2026-11-15: a perpetual and a
        // dated future (no funding fields).
        let text = r#"{"result":"success","serverTime":"2026-11-15T21:48:34.304Z","tickers":[
{"symbol":"PF_XBTUSD","last":65073,"lastTime":"2026-11-15T21:48:33.810110762Z","tag":"perpetual","pair":"XBT:USD","markPrice":65076.81201067088,"bid":65073,"bidSize":0.3172,"ask":65074,"askSize":0.018,"vol24h":13000.5011,"volumeQuote":835849924.2114,"openInterest":2290.7603,"open24h":64432,"high24h":66087,"low24h":61950,"vwap24h":64293.669742576305,"lastSize":0.0021,"fundingRate":0.7203430753178116,"fundingRatePrediction":0.666571057384,"relativeFundingRate":1.1066058333333e-05,"relativeFundingRatePrediction":1.02376e-05,"suspended":false,"indexPrice":65071.32,"postOnly":false,"change24h":0.99},
{"symbol":"FF_SOLUSD_261225","last":72.11,"lastTime":"2026-11-15T21:32:10.522340008Z","tag":"quarter","pair":"SOL:USD","markPrice":72.09815029296,"bid":71.89,"bidSize":277.7,"ask":72.29,"askSize":277.7,"vol24h":76069.12,"volumeQuote":5477589.0195,"openInterest":5255.37,"open24h":69.9,"high24h":73.51,"low24h":67.58,"vwap24h":72.00805030346085,"lastSize":4.03,"suspended":false,"indexPrice":71.99,"postOnly":false,"change24h":3.16}]}"#;
        let r: KrakenTickerResponse = parse_response(text).unwrap();
        assert_eq!(r.tickers.len(), 2);
        assert_eq!(r.tickers[0].funding_rate, Some(0.7203430753178116));
        assert_eq!(r.tickers[1].funding_rate, None);
    }

    #[test]
    fn parses_all_contract_type_tickers() {
        // tickers?contractType=all, 2026-11-16: perpetual, future, options (with and without a
        // trade), tradfi perpetual, untraded perpetual.
        let r: KrakenTickerResponse = parse_response(&fixture("tickers_all.json")).unwrap();
        assert_eq!(r.tickers.len(), 6);
        let option = &r.tickers[2];
        assert!(option.symbol.starts_with("OF_"));
        let greeks = option.greeks.as_ref().unwrap();
        assert!(greeks.delta < 0.0);
        assert!(option.extrinsic_value.is_some());
        assert!(option.mark_iv.is_some());
        assert!(r.tickers[3].last.is_none());
        assert!(r.tickers[4].is_underlying_market_closed.is_some());
        assert!(r.tickers[0].greeks.is_none());
    }

    #[test]
    fn ticker_missing_greeks_delta_is_error() {
        let text = without(
            &fixture("tickers_all.json"),
            &["tickers", "2", "greeks"],
            "delta",
        );
        assert!(parse_response::<KrakenTickerResponse>(&text).is_err());
        let text = without(&fixture("tickers_all.json"), &["tickers", "0"], "markPrice");
        assert!(parse_response::<KrakenTickerResponse>(&text).is_err());
    }

    #[test]
    fn parses_single_ticker() {
        let r: KrakenSingleTickerResponse =
            parse_response(&fixture("ticker_pf_xbtusd.json")).unwrap();
        assert_eq!(r.ticker.symbol, "PF_XBTUSD");
        assert_eq!(r.ticker.tag, "perpetual");
    }

    #[test]
    fn parses_instruments() {
        let r: KrakenInstrumentsResponse = parse_response(&fixture("instruments.json")).unwrap();
        let pf = &r.instruments[0];
        assert_eq!(pf.symbol, "PF_XBTUSD");
        assert_eq!(pf.kind, "flexible_futures");
        assert!(pf.margin_schedules.as_ref().unwrap().len() > 0);
        let opt = r.instruments.iter().find(|i| i.kind == "options").unwrap();
        assert!(opt.strike_price.is_some());
        assert!(opt.margin_levels.is_none());
        assert!(r.instruments.iter().any(|i| i.isin.is_none()));

        let r: KrakenInstrumentsResponse =
            parse_response(&fixture("instruments_expired.json")).unwrap();
        assert!(r.instruments.iter().all(|i| i.is_expired));
        let inv = &r.instruments[0];
        assert_eq!(inv.kind, "futures_inverse");
        assert!(inv.margin_levels.as_ref().unwrap()[0].contracts.is_some());
    }

    #[test]
    fn instrument_missing_tick_size_is_error() {
        let text = without(
            &fixture("instruments.json"),
            &["instruments", "0"],
            "tickSize",
        );
        assert!(parse_response::<KrakenInstrumentsResponse>(&text).is_err());
    }

    #[test]
    fn parses_instrument_status() {
        let r: KrakenInstrumentStatusListResponse =
            parse_response(&fixture("instruments_status.json")).unwrap();
        assert_eq!(r.instrument_status.len(), 3);
        let one: KrakenInstrumentStatus =
            parse_response(&fixture("instrument_status_pf_xbtusd.json")).unwrap();
        assert_eq!(one.tradeable, "PF_XBTUSD");
        assert_eq!(one.price_dislocation_direction, None);

        let text = without(
            &fixture("instrument_status_pf_xbtusd.json"),
            &[],
            "priceDislocationDirection",
        );
        assert!(parse_response::<KrakenInstrumentStatus>(&text).is_err());
    }

    #[test]
    fn instrument_status_error_is_error() {
        // GET /instruments/NOPE/status, 2026-11-16 (HTTP 404).
        let text = r#"{"result":"error","error":"invalidArgument: tradeable 'NOPE'","serverTime":"2026-11-16T11:10:26.645Z"}"#;
        let err = parse_response::<KrakenInstrumentStatus>(text).unwrap_err();
        assert!(err.to_string().contains("invalidArgument"));
    }

    #[test]
    fn parses_orderbook() {
        let r: KrakenOrderBookResponse = parse_response(&fixture("orderbook.json")).unwrap();
        assert_eq!(r.order_book.bids.len(), 5);
        assert_eq!(r.order_book.asks.len(), 5);
        let text = without(&fixture("orderbook.json"), &["orderBook"], "asks");
        assert!(parse_response::<KrakenOrderBookResponse>(&text).is_err());
    }

    #[test]
    fn parses_trade_history() {
        let r: KrakenHistoryResponse = parse_response(&fixture("history.json")).unwrap();
        assert_eq!(r.history.len(), 3);
        assert!(r.history[0].isin.is_none());

        let r: KrakenHistoryResponse = parse_response(&fixture("history_mtf.json")).unwrap();
        assert_eq!(r.history[0].execution_venue.as_deref(), Some("CRYP"));

        let text = without(&fixture("history.json"), &["history", "0"], "sequence_id");
        assert!(parse_response::<KrakenHistoryResponse>(&text).is_err());
    }

    #[test]
    fn parses_historical_funding_rates() {
        let r: KrakenHistoricalFundingRatesResponse =
            parse_response(&fixture("historical_funding_rates.json")).unwrap();
        assert_eq!(r.rates.len(), 4);
        let text = without(
            &fixture("historical_funding_rates.json"),
            &["rates", "0"],
            "relativeFundingRate",
        );
        assert!(parse_response::<KrakenHistoricalFundingRatesResponse>(&text).is_err());
        // historical-funding-rates?symbol=NOPE, 2026-11-16 (HTTP 400).
        let err = r#"{"status":"BAD_REQUEST","result":"error","errors":[{"code":11,"message":"Argument invalid: symbol"}],"serverTime":"2026-11-16T11:10:26.436Z"}"#;
        assert!(parse_response::<KrakenHistoricalFundingRatesResponse>(err).is_err());
    }

    #[tokio::test]
    async fn test_get_tickers() {
        let api = MarketApi::new_with_client(Arc::new(RateLimitedClient::new(10, 1)));
        let tickers = api.get_tickers(None, None).await.expect("tickers");
        assert!(tickers.contains_key("PF_XBTUSD"));
    }

    #[tokio::test]
    async fn test_get_tickers_options() {
        let api = MarketApi::new_with_client(Arc::new(RateLimitedClient::new(10, 1)));
        let tickers = api
            .get_tickers(Some(&[KrakenContractType::Options]), None)
            .await
            .expect("tickers");
        assert!(tickers.values().all(|t| t.greeks.is_some()));
    }

    #[tokio::test]
    async fn test_get_public_market_endpoints() {
        let api = MarketApi::new_with_client(Arc::new(RateLimitedClient::new(10, 1)));
        api.get_ticker("PF_XBTUSD").await.expect("ticker");
        assert!(!api
            .get_instruments(Some(&[KrakenContractType::All]), None)
            .await
            .expect("instruments")
            .is_empty());
        api.get_instruments(None, Some(true))
            .await
            .expect("expired");
        api.get_instruments_status(None).await.expect("status list");
        api.get_instrument_status("PF_XBTUSD")
            .await
            .expect("status");
        api.get_orderbook("PF_XBTUSD").await.expect("orderbook");
        api.get_trade_history("PF_XBTUSD", None, Some(true))
            .await
            .expect("history");
        api.get_historical_funding_rates("PF_XBTUSD")
            .await
            .expect("funding");
    }
}
