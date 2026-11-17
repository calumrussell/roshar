use serde::{Deserialize, Deserializer, Serialize};
use std::fmt;

/// A field the exchange always sends but whose value may be `null`.
/// Unlike a bare `Option<T>`, a missing key is an error.
fn nullable<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}

// ── Stats (GET /metadata/v2/stats, GET /metadata/stats) ──────────────────────
//
// All decimals are strings on the wire ("to preserve decimal precision") and are kept as strings.
// Prices and volumes are in USDC. Funding rates are decimals, not percentages.

/// Response of both stats endpoints; only the listing shape differs between v1 and v2.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VariationalStatsResponse<L> {
    /// 24h volume across all markets (USDC).
    pub total_volume_24h: String,
    /// Lifetime cumulative volume (USDC).
    pub cumulative_volume: String,
    /// Value locked in settlement pools and the OLP vault (USDC).
    pub tvl: String,
    /// Total open interest (USDC), double-sided: trader and OLP positions.
    pub open_interest: String,
    /// Listed markets, perpetuals and swaps alike. May exceed `listings.len()`.
    pub num_markets: u64,
    pub loss_refund: VariationalLossRefund,
    pub listings: Vec<L>,
}

/// `GET /metadata/v2/stats`.
pub type VariationalStats = VariationalStatsResponse<VariationalListing>;

/// `GET /metadata/stats` (legacy v1).
pub type VariationalStatsV1 = VariationalStatsResponse<VariationalListingV1>;

/// Deprecated by the exchange: both fields are always `"0"`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VariationalLossRefund {
    pub pool_size: String,
    pub refunded_24h: String,
}

/// One v2 listing.
///
/// `name`, `base_spread_bps` and `quotes` are documented as omitted when unavailable (e.g. while a
/// swap market is closed). `contract_type` selects the shape of `funding`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VariationalListing {
    /// Suffixed with the contract type, e.g. `BTC-PERP`, `XAU-SWAP`.
    pub ticker: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub mark_price: String,
    /// 24h volume (USDC).
    pub volume_24h: String,
    pub open_interest: VariationalListingOpenInterest,
    #[serde(flatten)]
    pub contract: VariationalContract,
    /// Bid/ask spread of the smallest quote (`quotes.base`, else `quotes.size_1k`), in bps of mark.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_spread_bps: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quotes: Option<VariationalQuotes>,
}

/// `contract_type` and the matching `funding` object. `funding` is always present and is `null`
/// when no funding data is available for the market.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "contract_type", rename_all = "lowercase")]
pub enum VariationalContract {
    Perpetual {
        #[serde(deserialize_with = "nullable")]
        funding: Option<VariationalPerpetualFunding>,
    },
    Swap {
        #[serde(deserialize_with = "nullable")]
        funding: Option<VariationalSwapFunding>,
    },
}

/// Single-sided (trader positions only) open interest in USDC.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VariationalListingOpenInterest {
    pub long_open_interest: String,
    pub short_open_interest: String,
}

/// Funding of a perpetual. A positive rate means longs pay shorts.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VariationalPerpetualFunding {
    /// Current funding rate.
    pub last: VariationalPerpetualRate,
    /// Predicted rate for the next interval (currently the same as `last`).
    #[serde(deserialize_with = "nullable")]
    pub next: Option<VariationalPerpetualRate>,
    pub interval_seconds: u64,
    /// Next funding payment, Unix seconds.
    pub next_timestamp: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VariationalPerpetualRate {
    pub annualized_rate: String,
    /// Rate for one funding interval.
    pub interval_rate: String,
}

/// Funding of a swap, paid once per trading day with separate long and short rates. At least
/// one of `last` and `next` is set; otherwise the listing's `funding` is `null`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VariationalSwapFunding {
    /// Most recently applied payment; `null` if none in the past week.
    #[serde(deserialize_with = "nullable")]
    pub last: Option<VariationalSwapRate>,
    /// Upcoming payment; `null` between a payment and the publication of the next one's rates.
    #[serde(deserialize_with = "nullable")]
    pub next: Option<VariationalSwapRate>,
    /// Always 86400.
    pub interval_seconds: u64,
    /// When `next` applies, Unix seconds; `null` when `next` is `null`.
    #[serde(deserialize_with = "nullable")]
    pub next_timestamp: Option<u64>,
}

/// Interval rates are the annualized rate divided by 360. Payments covering weekends or market
/// holidays include the closed days.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VariationalSwapRate {
    /// Positive means longs pay.
    pub annualized_long_rate: String,
    /// Positive means shorts receive.
    pub annualized_short_rate: String,
    pub interval_long_rate: String,
    pub interval_short_rate: String,
}

/// Indicative quotes at several sizes. Each size is omitted when unavailable; `size_1m` is only
/// quoted for major and high-volume markets. Quotes may be cached for up to 600 s.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VariationalQuotes {
    /// ISO 8601 time the quotes were fetched.
    pub updated_at: String,
    /// At the market's minimum order size.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base: Option<VariationalQuote>,
    /// At $1,000 notional.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_1k: Option<VariationalQuote>,
    /// At $100,000 notional.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_100k: Option<VariationalQuote>,
    /// At $1,000,000 notional.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_1m: Option<VariationalQuote>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VariationalQuote {
    pub bid: String,
    pub ask: String,
}

/// One legacy v1 listing. Per the exchange's v1→v2 migration notes, only `ticker` (no suffix;
/// swaps under their symbol, e.g. `XAUS`), `funding_rate` and `funding_interval_s` differ from v2,
/// and there is no `contract_type`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VariationalListingV1 {
    pub ticker: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub mark_price: String,
    pub volume_24h: String,
    pub open_interest: VariationalListingOpenInterest,
    /// Annualized perpetual rate (v2 `funding.last.annualized_rate`). A `"0"` placeholder for
    /// swaps and for markets without funding data.
    pub funding_rate: String,
    /// v2 `funding.interval_seconds`; `0` for swaps.
    pub funding_interval_s: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_spread_bps: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quotes: Option<VariationalQuotes>,
}

// ── Errors ───────────────────────────────────────────────────────────────────

/// Error body. Documented for HTTP 429 (shared limit of v1 and v2), e.g.
/// `{"error":"rate_limited","message":"Service is busy; try again in 1 seconds","retry_after_seconds":1}`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VariationalErrorResponse {
    pub error: String,
    pub message: String,
    /// Only documented on 429 responses.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry_after_seconds: Option<u64>,
}

/// Non-2xx reply. `error` is set when the body parses as the exchange's error envelope; `body`
/// is the raw body either way (empty on 404).
#[derive(Debug, Clone)]
pub struct VariationalApiError {
    pub status: u16,
    pub error: Option<VariationalErrorResponse>,
    pub body: String,
}

impl fmt::Display for VariationalApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.error {
            Some(e) => {
                write!(
                    f,
                    "Variational HTTP {}: {}: {}",
                    self.status, e.error, e.message
                )?;
                if let Some(secs) = e.retry_after_seconds {
                    write!(f, " (retry after {secs}s)")?;
                }
                Ok(())
            }
            None => write!(f, "Variational HTTP {}: {}", self.status, self.body),
        }
    }
}

impl std::error::Error for VariationalApiError {}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    // Live captures of https://omni-client-api.prod.ap-northeast-1.variational.io on 2026-11-15
    // (a Sunday, so swap markets were closed), trimmed to four listings. `stats_v2_docs_sample`
    // is the sample from the API docs, which shows an open swap market.
    const STATS_V2: &str = include_str!("../../tests/fixtures/stats_v2.json");
    const STATS_V2_DOCS: &str = include_str!("../../tests/fixtures/stats_v2_docs_sample.json");
    const STATS_V1: &str = include_str!("../../tests/fixtures/stats_v1.json");
    const RATE_LIMITED: &str = include_str!("../../tests/fixtures/error_rate_limited.json");

    fn listing<'a>(stats: &'a VariationalStats, ticker: &str) -> &'a VariationalListing {
        stats
            .listings
            .iter()
            .find(|l| l.ticker == ticker)
            .unwrap_or_else(|| panic!("{ticker} missing"))
    }

    fn perp_funding(l: &VariationalListing) -> &VariationalPerpetualFunding {
        match &l.contract {
            VariationalContract::Perpetual { funding: Some(f) } => f,
            other => panic!("{} not a funded perpetual: {other:?}", l.ticker),
        }
    }

    fn swap_funding(l: &VariationalListing) -> &VariationalSwapFunding {
        match &l.contract {
            VariationalContract::Swap { funding: Some(f) } => f,
            other => panic!("{} not a funded swap: {other:?}", l.ticker),
        }
    }

    fn without(json: &str, pointer: &str, key: &str) -> String {
        let mut v: Value = serde_json::from_str(json).unwrap();
        v.pointer_mut(pointer)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .remove(key)
            .unwrap_or_else(|| panic!("{pointer}/{key} not in fixture"));
        v.to_string()
    }

    fn index_of(json: &str, ticker: &str) -> usize {
        let v: Value = serde_json::from_str(json).unwrap();
        v["listings"]
            .as_array()
            .unwrap()
            .iter()
            .position(|l| l["ticker"] == ticker)
            .unwrap()
    }

    #[test]
    fn test_stats_v2_live() {
        let stats: VariationalStats = serde_json::from_str(STATS_V2).unwrap();
        assert_eq!(stats.num_markets, 589);
        assert_eq!(stats.listings.len(), 4);
        assert_eq!(stats.loss_refund.pool_size, "0");

        let btc = listing(&stats, "BTC-PERP");
        assert_eq!(btc.name.as_deref(), Some("Bitcoin"));
        let f = perp_funding(btc);
        assert!(f.next.is_some());
        assert!(f.interval_seconds > 0 && f.next_timestamp > 0);
        let q = btc.quotes.as_ref().unwrap();
        assert!(q.base.is_some() && q.size_1k.is_some() && q.size_100k.is_some());
        assert!(q.size_1m.is_some());
        assert!(btc.base_spread_bps.is_some());

        let tnsr = listing(&stats, "TNSR-PERP");
        assert!(tnsr.quotes.as_ref().unwrap().size_1m.is_none());

        // Closed swap: no quotes or spread, next payment not yet published.
        for t in ["XAU-SWAP", "US500-SWAP"] {
            let swap = listing(&stats, t);
            assert!(swap.quotes.is_none() && swap.base_spread_bps.is_none());
            let f = swap_funding(swap);
            assert!(f.last.is_some());
            assert!(f.next.is_none() && f.next_timestamp.is_none());
            assert_eq!(f.interval_seconds, 86400);
        }
    }

    #[test]
    fn test_stats_v2_docs_sample() {
        let stats: VariationalStats = serde_json::from_str(STATS_V2_DOCS).unwrap();
        let xau = listing(&stats, "XAU-SWAP");
        let f = swap_funding(xau);
        assert_eq!(f.next.as_ref().unwrap().annualized_long_rate, "0.05843");
        assert_eq!(f.next_timestamp, Some(1792184700));
        assert_eq!(
            xau.quotes.as_ref().unwrap().size_1m.as_ref().unwrap().ask,
            "4168.74"
        );

        let btc = perp_funding(listing(&stats, "BTC-PERP"));
        assert_eq!(btc.last.interval_rate, "0.00008246");
        assert_eq!(btc.interval_seconds, 28800);
        assert_eq!(btc.next_timestamp, 1792137600);
    }

    #[test]
    fn test_stats_v1_live() {
        let stats: VariationalStatsV1 = serde_json::from_str(STATS_V1).unwrap();
        assert_eq!(stats.listings.len(), 4);
        let btc = stats.listings.iter().find(|l| l.ticker == "BTC").unwrap();
        assert!(btc.funding_interval_s > 0);
        assert!(btc.quotes.is_some());
        let xaus = stats.listings.iter().find(|l| l.ticker == "XAUS").unwrap();
        assert_eq!(xaus.funding_rate, "0");
        assert_eq!(xaus.funding_interval_s, 0);
        assert!(xaus.quotes.is_none() && xaus.base_spread_bps.is_none());
    }

    #[test]
    fn test_funding_key_required() {
        let i = index_of(STATS_V2, "BTC-PERP");
        let json = without(STATS_V2, &format!("/listings/{i}"), "funding");
        assert!(serde_json::from_str::<VariationalStats>(&json).is_err());
    }

    #[test]
    fn test_swap_nullable_keys_required() {
        let i = index_of(STATS_V2, "XAU-SWAP");
        for key in ["next", "next_timestamp", "last"] {
            let json = without(STATS_V2, &format!("/listings/{i}/funding"), key);
            assert!(
                serde_json::from_str::<VariationalStats>(&json).is_err(),
                "{key} missing should fail"
            );
        }
    }

    #[test]
    fn test_required_fields() {
        let i = index_of(STATS_V2, "BTC-PERP");
        for key in ["mark_price", "contract_type", "open_interest", "ticker"] {
            let json = without(STATS_V2, &format!("/listings/{i}"), key);
            assert!(
                serde_json::from_str::<VariationalStats>(&json).is_err(),
                "{key} missing should fail"
            );
        }
        let json = without(STATS_V2, "", "num_markets");
        assert!(serde_json::from_str::<VariationalStats>(&json).is_err());
        let json = without(STATS_V2, &format!("/listings/{i}/quotes"), "updated_at");
        assert!(serde_json::from_str::<VariationalStats>(&json).is_err());
    }

    #[test]
    fn test_contract_type_must_match_funding() {
        let mut v: Value = serde_json::from_str(STATS_V2).unwrap();
        let i = index_of(STATS_V2, "XAU-SWAP");
        v["listings"][i]["contract_type"] = "perpetual".into();
        assert!(serde_json::from_str::<VariationalStats>(&v.to_string()).is_err());

        v["listings"][i]["contract_type"] = "future".into();
        assert!(serde_json::from_str::<VariationalStats>(&v.to_string()).is_err());
    }

    #[test]
    fn test_null_funding() {
        let mut v: Value = serde_json::from_str(STATS_V2).unwrap();
        let i = index_of(STATS_V2, "BTC-PERP");
        v["listings"][i]["funding"] = Value::Null;
        let stats: VariationalStats = serde_json::from_str(&v.to_string()).unwrap();
        assert!(matches!(
            listing(&stats, "BTC-PERP").contract,
            VariationalContract::Perpetual { funding: None }
        ));
    }

    #[test]
    fn test_rate_limited_envelope() {
        let e: VariationalErrorResponse = serde_json::from_str(RATE_LIMITED).unwrap();
        assert_eq!(e.error, "rate_limited");
        assert_eq!(e.retry_after_seconds, Some(1));
        let err = VariationalApiError {
            status: 429,
            error: Some(e),
            body: RATE_LIMITED.to_string(),
        };
        assert_eq!(
            err.to_string(),
            "Variational HTTP 429: rate_limited: Service is busy; try again in 1 seconds (retry after 1s)"
        );
    }
}
