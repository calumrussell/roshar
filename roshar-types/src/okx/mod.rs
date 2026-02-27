use serde::{Deserialize, Serialize};

/// OKX instrument information for SWAP (perpetual) contracts.
///
/// Corresponds to the response from `GET /api/v5/public/instruments?instType=SWAP`.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct OkxInstrumentInfo {
    /// Instrument type (e.g. "SWAP")
    #[serde(rename = "instType")]
    pub inst_type: String,
    /// Instrument ID (e.g. "BTC-USDT-SWAP")
    #[serde(rename = "instId")]
    pub inst_id: String,
    /// Underlying (e.g. "BTC-USDT")
    #[serde(default)]
    pub uly: String,
    /// Instrument family (e.g. "BTC-USDT")
    #[serde(rename = "instFamily", default)]
    pub inst_family: String,
    /// Settlement currency (e.g. "USDT")
    #[serde(rename = "settleCcy", default)]
    pub settle_ccy: String,
    /// Contract value (e.g. "0.01")
    #[serde(rename = "ctVal", default)]
    pub ct_val: String,
    /// Contract multiplier
    #[serde(rename = "ctMult", default)]
    pub ct_mult: String,
    /// Contract value currency (e.g. "BTC")
    #[serde(rename = "ctValCcy", default)]
    pub ct_val_ccy: String,
    /// Contract type: "linear" or "inverse"
    #[serde(rename = "ctType", default)]
    pub ct_type: String,
    /// Tick size - minimum price increment (e.g. "0.1")
    #[serde(rename = "tickSz", default)]
    pub tick_sz: String,
    /// Lot size - minimum trading size (e.g. "1")
    #[serde(rename = "lotSz", default)]
    pub lot_sz: String,
    /// Minimum order size
    #[serde(rename = "minSz", default)]
    pub min_sz: String,
    /// Maximum leverage
    #[serde(default)]
    pub lever: String,
    /// Instrument state: "live", "suspend", "preopen", etc.
    #[serde(default)]
    pub state: String,
    /// Listing time (ms timestamp)
    #[serde(rename = "listTime", default)]
    pub list_time: String,
    /// Expiry time (empty for perpetuals)
    #[serde(rename = "expTime", default)]
    pub exp_time: String,
    /// Maximum order size
    #[serde(rename = "maxLmtSz", default)]
    pub max_lmt_sz: String,
    /// Maximum market order size
    #[serde(rename = "maxMktSz", default)]
    pub max_mkt_sz: String,
}
