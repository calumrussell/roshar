mod market;
pub use market::*;
mod ws;
pub use ws::*;

use serde::{Deserialize, Deserializer, Serialize};

/// For fields the exchange always sends but may send as `null`: the key is required
/// (no `#[serde(default)]`), the value may be `null`.
pub(crate) fn nullable<'de, D, T>(d: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(d)
}

/// KuCoin REST envelope.
///
/// Success: `{"code":"200000","data":...}` with no `msg`. Error: `{"code":"<non-200000>",
/// "msg":"..."}` with no `data`. `data` is also `null` on success when a query matches
/// nothing (e.g. an empty funding-history window).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KuCoinResponse<T> {
    pub code: String,
    pub msg: Option<String>,
    pub data: Option<T>,
}

/// One entry of GET /api/v1/contracts/active (Get All Symbols).
///
/// The exchange documents every field as present; fields modelled as `Option` are
/// sent as `null` for some contracts (delivery contracts, or not populated by the
/// exchange at all).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KuCoinContract {
    pub symbol: String,
    #[serde(rename = "rootSymbol")]
    pub root_symbol: String,
    #[serde(rename = "baseCurrency")]
    pub base_currency: String,
    #[serde(rename = "quoteCurrency")]
    pub quote_currency: String,
    #[serde(rename = "settleCurrency")]
    pub settle_currency: String,
    /// Current funding fee rate. `null` for delivery (`FFICSX`) contracts.
    #[serde(rename = "fundingFeeRate", deserialize_with = "nullable")]
    pub funding_fee_rate: Option<f64>,
    /// Predicted next funding fee rate. Documented, but sent as `null`.
    #[serde(rename = "predictedFundingFeeRate", deserialize_with = "nullable")]
    pub predicted_funding_fee_rate: Option<f64>,
    /// Configured funding interval in milliseconds. `null` for delivery contracts.
    #[serde(rename = "fundingRateGranularity", deserialize_with = "nullable")]
    pub funding_rate_granularity: Option<i64>,
    #[serde(rename = "markPrice")]
    pub mark_price: f64,
    #[serde(rename = "indexPrice")]
    pub index_price: f64,
    #[serde(rename = "lastTradePrice")]
    pub last_trade_price: f64,
    #[serde(rename = "maxLeverage")]
    pub max_leverage: i32,
    #[serde(rename = "tickSize")]
    pub tick_size: f64,
    /// Minimum lot size, in lots (documented as an integer).
    #[serde(rename = "lotSize")]
    pub lot_size: f64,
    /// Contract status: `Init`, `Open`, `BeingSettled`, `Settled`, `Paused`, `Closed`,
    /// `CancelOnly`.
    pub status: String,
    /// Contract type: `FFWCSX` (perpetual) or `FFICSX` (delivery).
    #[serde(rename = "type")]
    pub contract_type: String,
    /// Underlying amount per lot.
    pub multiplier: f64,
    #[serde(rename = "makerFeeRate")]
    pub maker_fee_rate: f64,
    #[serde(rename = "takerFeeRate")]
    pub taker_fee_rate: f64,
    /// Funding rate index symbol, e.g. `.XBTUSDTMFPI8H`. `null` for delivery contracts.
    #[serde(rename = "fundingRateSymbol", deserialize_with = "nullable")]
    pub funding_rate_symbol: Option<String>,
    /// Open interest, in lots.
    #[serde(rename = "openInterest")]
    pub open_interest: String,
    #[serde(rename = "turnoverOf24h")]
    pub turnover_of_24h: f64,
    #[serde(rename = "volumeOf24h")]
    pub volume_of_24h: f64,
}

/// One entry of GET /api/v1/contract/funding-rates (Get Public Funding History).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KuCoinFundingRate {
    pub symbol: String,
    #[serde(rename = "fundingRate")]
    pub funding_rate: f64,
    /// Settlement timestamp in milliseconds
    pub timepoint: i64,
}

/// One entry of `instanceServers` returned by POST /api/v1/bullet-public
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KuCoinWsInstanceServer {
    pub endpoint: String,
    pub encrypt: bool,
    pub protocol: String,
    /// Recommended ping interval, milliseconds.
    #[serde(rename = "pingInterval")]
    pub ping_interval: u64,
    /// Heartbeat timeout, milliseconds.
    #[serde(rename = "pingTimeout")]
    pub ping_timeout: u64,
}

/// `data` of POST /api/v1/bullet-public (Get Public Token - Classic Futures)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KuCoinWsTokenData {
    pub token: String,
    #[serde(rename = "instanceServers")]
    pub instance_servers: Vec<KuCoinWsInstanceServer>,
}

impl KuCoinWsTokenData {
    /// The first instance server, which the exchange tells clients to use.
    pub fn server(
        &self,
    ) -> Result<&KuCoinWsInstanceServer, Box<dyn std::error::Error + Send + Sync>> {
        self.instance_servers
            .first()
            .ok_or_else(|| "KuCoin bullet-public returned no instance servers".into())
    }

    /// `<instanceServers[0].endpoint>?token=<token>`.
    pub fn url(&self) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        Ok(format!("{}?token={}", self.server()?.endpoint, self.token))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_envelope_without_data_parses() {
        let r: KuCoinResponse<Vec<KuCoinFundingRate>> =
            serde_json::from_str(r#"{"msg":"This contract does not exist.","code":"404000"}"#)
                .unwrap();
        assert_eq!(r.code, "404000");
        assert!(r.data.is_none());
    }

    #[test]
    fn empty_funding_window_parses() {
        let r: KuCoinResponse<Vec<KuCoinFundingRate>> =
            serde_json::from_str(r#"{"code":"200000","data":null}"#).unwrap();
        assert!(r.msg.is_none());
        assert!(r.data.is_none());
    }

    #[test]
    fn delivery_contract_with_null_funding_fields_parses() {
        let json = r#"{"symbol":"XBTMZ26","rootSymbol":"XBT","type":"FFICSX",
            "baseCurrency":"XBT","quoteCurrency":"USD","settleCurrency":"XBT",
            "fundingFeeRate":null,"predictedFundingFeeRate":null,
            "fundingRateGranularity":null,"fundingRateSymbol":null,
            "markPrice":65457.0,"indexPrice":65119.59,"lastTradePrice":64584.3,
            "maxLeverage":20,"tickSize":0.1,"lotSize":1,"status":"Open",
            "multiplier":-1.0,"makerFeeRate":0.0002,"takerFeeRate":0.0006,
            "openInterest":"54176","turnoverOf24h":6.93107709,"volumeOf24h":435662.0}"#;
        let c: KuCoinContract = serde_json::from_str(json).unwrap();
        assert!(c.funding_fee_rate.is_none());
        assert!(c.funding_rate_symbol.is_none());
        assert_eq!(c.contract_type, "FFICSX");
    }

    #[test]
    fn contract_absent_nullable_key_is_error() {
        let json = r#"{"symbol":"XBTMZ26","rootSymbol":"XBT","type":"FFICSX",
            "baseCurrency":"XBT","quoteCurrency":"USD","settleCurrency":"XBT",
            "fundingFeeRate":null,"predictedFundingFeeRate":null,
            "fundingRateGranularity":null,
            "markPrice":65457.0,"indexPrice":65119.59,"lastTradePrice":64584.3,
            "maxLeverage":20,"tickSize":0.1,"lotSize":1,"status":"Open",
            "multiplier":-1.0,"makerFeeRate":0.0002,"takerFeeRate":0.0006,
            "openInterest":"54176","turnoverOf24h":6.93107709,"volumeOf24h":435662.0}"#;
        assert!(serde_json::from_str::<KuCoinContract>(json).is_err());
    }

    #[test]
    fn contract_missing_documented_field_is_error() {
        let json = r#"{"symbol":"XBTUSDTM"}"#;
        assert!(serde_json::from_str::<KuCoinContract>(json).is_err());
    }
}
