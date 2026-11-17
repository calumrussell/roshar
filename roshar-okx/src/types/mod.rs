pub mod market;
pub mod ws;

pub use market::*;
pub use ws::*;

use serde::{Deserialize, Deserializer, Serialize};

/// OKX instrument information.
///
/// Corresponds to an entry of `GET /api/v5/public/instruments`. OKX always sends every field;
/// fields not applicable to an instrument type are sent as "".
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct OkxInstrumentInfo {
    /// Instrument type (e.g. "SWAP")
    #[serde(rename = "instType")]
    pub inst_type: String,
    /// Instrument ID (e.g. "BTC-USDT-SWAP")
    #[serde(rename = "instId")]
    pub inst_id: String,
    /// Base currency (e.g. "BTC"); SPOT/MARGIN only
    #[serde(rename = "baseCcy")]
    pub base_ccy: String,
    /// Quote currency (e.g. "USDT"); SPOT/MARGIN only
    #[serde(rename = "quoteCcy")]
    pub quote_ccy: String,
    /// Event-contract series ID (e.g. "BTC-UPDOWN-5MIN"; "" outside EVENTS). Absent from the WS
    /// `instruments` push and from some `preopen` SPOT rows over REST.
    #[serde(rename = "seriesId", default, skip_serializing_if = "Option::is_none")]
    pub series_id: Option<String>,
    /// Underlying (e.g. "BTC-USDT"); MARGIN/FUTURES/SWAP/OPTION only
    pub uly: String,
    /// Instrument family (e.g. "BTC-USDT"); MARGIN/FUTURES/SWAP/OPTION only
    #[serde(rename = "instFamily")]
    pub inst_family: String,
    /// Settlement and margin currency (e.g. "USDT"); FUTURES/SWAP/OPTION only
    #[serde(rename = "settleCcy")]
    pub settle_ccy: String,
    /// Contract value (e.g. "0.01"); FUTURES/SWAP/OPTION only
    #[serde(rename = "ctVal")]
    pub ct_val: String,
    /// Contract multiplier; FUTURES/SWAP/OPTION only
    #[serde(rename = "ctMult")]
    pub ct_mult: String,
    /// Contract value currency (e.g. "BTC"); FUTURES/SWAP/OPTION only
    #[serde(rename = "ctValCcy")]
    pub ct_val_ccy: String,
    /// Contract type: "linear" or "inverse"; FUTURES/SWAP only
    #[serde(rename = "ctType")]
    pub ct_type: String,
    /// Tick size - minimum price increment (e.g. "0.1")
    #[serde(rename = "tickSz")]
    pub tick_sz: String,
    /// Lot size - order size increment (contracts for derivatives)
    #[serde(rename = "lotSz")]
    pub lot_sz: String,
    /// Minimum order size (contracts for derivatives)
    #[serde(rename = "minSz")]
    pub min_sz: String,
    /// Maximum leverage; not applicable to SPOT/OPTION
    pub lever: String,
    /// Instrument state: "live", "suspend", "preopen", "test", "rebase", "post_only", ...
    pub state: String,
    /// Listing time (ms timestamp)
    #[serde(rename = "listTime")]
    pub list_time: String,
    /// Expiry / offline time (ms timestamp; "" when none)
    #[serde(rename = "expTime")]
    pub exp_time: String,
    /// Maximum quantity of a single limit order
    #[serde(rename = "maxLmtSz")]
    pub max_lmt_sz: String,
    /// Maximum quantity of a single market order
    #[serde(rename = "maxMktSz")]
    pub max_mkt_sz: String,
}

/// OKX candlestick data from REST API.
///
/// OKX returns candles as arrays of strings in the format:
/// [ts, o, h, l, c, vol, volCcy, volCcyQuote, confirm]
///
/// All values are strings and need to be parsed.
#[derive(Debug, Clone, Serialize)]
pub struct OkxCandle {
    /// Opening time of the candlestick (Unix timestamp in milliseconds)
    pub ts: i64,
    /// Open price
    pub open: f64,
    /// Highest price
    pub high: f64,
    /// Lowest price
    pub low: f64,
    /// Close price
    pub close: f64,
    /// Trading volume in contracts
    pub vol: f64,
    /// Trading volume in base currency
    pub vol_ccy: f64,
    /// Trading volume in quote currency
    pub vol_ccy_quote: f64,
    /// Candle state: 0 = incomplete, 1 = complete
    pub confirm: u8,
}

impl<'de> Deserialize<'de> for OkxCandle {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::{Error, SeqAccess, Visitor};
        use std::fmt;

        struct OkxCandleVisitor;

        impl<'de> Visitor<'de> for OkxCandleVisitor {
            type Value = OkxCandle;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a sequence of 9 string elements representing an OKX candle")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let ts_str: String = seq
                    .next_element()?
                    .ok_or_else(|| Error::missing_field("ts"))?;
                let ts = ts_str
                    .parse::<i64>()
                    .map_err(|_| Error::custom("failed to parse ts as i64"))?;

                let open_str: String = seq
                    .next_element()?
                    .ok_or_else(|| Error::missing_field("open"))?;
                let open = open_str
                    .parse::<f64>()
                    .map_err(|_| Error::custom("failed to parse open as f64"))?;

                let high_str: String = seq
                    .next_element()?
                    .ok_or_else(|| Error::missing_field("high"))?;
                let high = high_str
                    .parse::<f64>()
                    .map_err(|_| Error::custom("failed to parse high as f64"))?;

                let low_str: String = seq
                    .next_element()?
                    .ok_or_else(|| Error::missing_field("low"))?;
                let low = low_str
                    .parse::<f64>()
                    .map_err(|_| Error::custom("failed to parse low as f64"))?;

                let close_str: String = seq
                    .next_element()?
                    .ok_or_else(|| Error::missing_field("close"))?;
                let close = close_str
                    .parse::<f64>()
                    .map_err(|_| Error::custom("failed to parse close as f64"))?;

                let vol_str: String = seq
                    .next_element()?
                    .ok_or_else(|| Error::missing_field("vol"))?;
                let vol = vol_str
                    .parse::<f64>()
                    .map_err(|_| Error::custom("failed to parse vol as f64"))?;

                let vol_ccy_str: String = seq
                    .next_element()?
                    .ok_or_else(|| Error::missing_field("volCcy"))?;
                let vol_ccy = vol_ccy_str
                    .parse::<f64>()
                    .map_err(|_| Error::custom("failed to parse volCcy as f64"))?;

                let vol_ccy_quote_str: String = seq
                    .next_element()?
                    .ok_or_else(|| Error::missing_field("volCcyQuote"))?;
                let vol_ccy_quote = vol_ccy_quote_str
                    .parse::<f64>()
                    .map_err(|_| Error::custom("failed to parse volCcyQuote as f64"))?;

                let confirm_str: String = seq
                    .next_element()?
                    .ok_or_else(|| Error::missing_field("confirm"))?;
                let confirm = confirm_str
                    .parse::<u8>()
                    .map_err(|_| Error::custom("failed to parse confirm as u8"))?;

                Ok(OkxCandle {
                    ts,
                    open,
                    high,
                    low,
                    close,
                    vol,
                    vol_ccy,
                    vol_ccy_quote,
                    confirm,
                })
            }
        }

        deserializer.deserialize_seq(OkxCandleVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_okx_candle_deserialize() {
        let json_str = r#"["1697026383085","28350.1","28400.0","28300.5","28375.2","12345","1.5","42567.89","1"]"#;
        let candle: OkxCandle = serde_json::from_str(json_str).unwrap();

        assert_eq!(candle.ts, 1697026383085);
        assert!((candle.open - 28350.1).abs() < 1e-10);
        assert!((candle.high - 28400.0).abs() < 1e-10);
        assert!((candle.low - 28300.5).abs() < 1e-10);
        assert!((candle.close - 28375.2).abs() < 1e-10);
        assert!((candle.vol - 12345.0).abs() < 1e-10);
        assert!((candle.vol_ccy - 1.5).abs() < 1e-10);
        assert!((candle.vol_ccy_quote - 42567.89).abs() < 1e-10);
        assert_eq!(candle.confirm, 1);
    }

    #[test]
    fn test_okx_candle_deserialize_array() {
        let json_str = r#"[
            ["1697026383085","28350.1","28400.0","28300.5","28375.2","12345","1.5","42567.89","1"],
            ["1697026443085","28375.2","28410.0","28370.0","28390.5","9876","1.2","34123.45","1"]
        ]"#;
        let candles: Vec<OkxCandle> = serde_json::from_str(json_str).unwrap();

        assert_eq!(candles.len(), 2);
        assert_eq!(candles[0].ts, 1697026383085);
        assert_eq!(candles[1].ts, 1697026443085);
    }
}
