//! Typed frames of the public v3 websocket (https://docs.derive.xyz/subscriptions,
//! https://docs.derive.xyz/subscriptions.asyncapi.json).

use crate::rest::{DeriveTicker, DeriveTrade};
use anyhow::{anyhow, Result};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// `orderbook.{instrument}.{group}.{depth}` snapshot. Levels are
/// `[price, amount]`.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeriveWsOrderbook {
    pub instrument_name: String,
    pub publish_id: u64,
    /// ms.
    pub timestamp: i64,
    pub bids: Vec<[String; 2]>,
    pub asks: Vec<[String; 2]>,
}

/// `ticker_slim.{instrument}.{interval}` update.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeriveWsTickerSlim {
    /// ms.
    pub timestamp: i64,
    pub instrument_ticker: DeriveTicker,
}

/// One currency of a `spot_feed.{currency}` update.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeriveWsSpotFeedEntry {
    pub price: String,
    pub confidence: String,
    pub price_prev_daily: String,
    pub confidence_prev_daily: String,
    /// ms.
    pub timestamp_prev_daily: i64,
}

/// `spot_feed.{currency}` update, keyed by currency.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeriveWsSpotFeed {
    /// ms.
    pub timestamp: i64,
    pub feeds: HashMap<String, DeriveWsSpotFeedEntry>,
}

/// `AuctionDetails` of an `auctions.watch` item.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeriveWsAuctionDetails {
    pub risk_universe_id: u32,
    pub margin_type: String,
    /// Only set when the account is not on standard margin.
    #[serde(default)]
    pub currency: Option<String>,
    pub estimated_bid_price: String,
    pub estimated_discount_pnl: String,
    pub estimated_mtm: String,
    pub estimated_percent_bid: String,
    pub min_price_limit: String,
    pub subaccount_balances: HashMap<String, String>,
}

/// `auctions.watch` item.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeriveWsAuction {
    pub subaccount_id: u64,
    /// `"ongoing"` or `"ended"`.
    pub state: String,
    /// ms.
    pub timestamp: i64,
    /// Null once the auction has ended.
    #[serde(default)]
    pub details: Option<DeriveWsAuctionDetails>,
}

/// `margin.watch` item. The asyncapi spec leaves `collaterals` and
/// `positions` items untyped, so they are kept as raw JSON.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeriveWsMargin {
    pub subaccount_id: u64,
    pub currency: String,
    pub margin_type: String,
    pub initial_margin: String,
    pub maintenance_margin: String,
    pub subaccount_value: String,
    /// ms.
    pub valuation_timestamp: i64,
    pub collaterals: Vec<serde_json::Value>,
    pub positions: Vec<serde_json::Value>,
}

/// Reply to `subscribe` (`current_subscriptions`) or `unsubscribe`
/// (`remaining_subscriptions`). `status` maps each requested channel to
/// `"ok"` or an error string.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeriveWsSubscriptionResult {
    pub status: HashMap<String, String>,
    #[serde(default)]
    pub current_subscriptions: Option<Vec<String>>,
    #[serde(default)]
    pub remaining_subscriptions: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeriveWsError {
    pub code: i64,
    pub message: String,
    #[serde(default)]
    pub data: Option<String>,
}

/// A public websocket frame. Each notification carries its full channel
/// name.
#[derive(Debug, Clone)]
pub enum DeriveWsMessage {
    SubscriptionResult {
        id: Option<serde_json::Value>,
        result: DeriveWsSubscriptionResult,
    },
    /// Exchange error reply (e.g. code 13000 "Invalid channels").
    Error {
        id: Option<serde_json::Value>,
        error: DeriveWsError,
    },
    Orderbook {
        channel: String,
        data: DeriveWsOrderbook,
    },
    TickerSlim {
        channel: String,
        data: DeriveWsTickerSlim,
    },
    /// `trades.{instrument}` and `trades.{instrument_type}.{currency}`.
    Trades {
        channel: String,
        data: Vec<DeriveTrade>,
    },
    SpotFeed {
        channel: String,
        data: DeriveWsSpotFeed,
    },
    AuctionsWatch {
        channel: String,
        data: Vec<DeriveWsAuction>,
    },
    MarginWatch {
        channel: String,
        data: Vec<DeriveWsMargin>,
    },
}

#[derive(Deserialize)]
struct RawFrame {
    #[serde(default)]
    id: Option<serde_json::Value>,
    #[serde(default)]
    method: Option<String>,
    #[serde(default)]
    params: Option<RawParams>,
    #[serde(default)]
    result: Option<DeriveWsSubscriptionResult>,
    #[serde(default)]
    error: Option<DeriveWsError>,
}

#[derive(Deserialize)]
struct RawParams {
    channel: String,
    data: serde_json::Value,
}

/// The asyncapi spec documents `auctions.watch` / `margin.watch` payloads as
/// single objects while the trade channels (documented the same way) send
/// arrays live; accept both.
#[derive(Deserialize)]
#[serde(untagged)]
enum OneOrMany<T> {
    Many(Vec<T>),
    One(T),
}

fn one_or_many<T: DeserializeOwned>(v: serde_json::Value) -> Result<Vec<T>> {
    Ok(match OneOrMany::<T>::deserialize(v)? {
        OneOrMany::Many(v) => v,
        OneOrMany::One(x) => vec![x],
    })
}

fn de<T: DeserializeOwned>(v: serde_json::Value) -> Result<T> {
    Ok(T::deserialize(v)?)
}

impl DeriveWsMessage {
    pub fn parse(text: &str) -> Result<Self> {
        let frame: RawFrame = serde_json::from_str(text)?;
        if let Some(error) = frame.error {
            return Ok(Self::Error {
                id: frame.id,
                error,
            });
        }
        if let Some(result) = frame.result {
            return Ok(Self::SubscriptionResult {
                id: frame.id,
                result,
            });
        }
        match (frame.method.as_deref(), frame.params) {
            (Some("subscription"), Some(RawParams { channel, data })) => {
                let kind = channel.split('.').next().unwrap_or("");
                Ok(match kind {
                    "orderbook" => Self::Orderbook {
                        data: de(data)?,
                        channel,
                    },
                    "ticker_slim" => Self::TickerSlim {
                        data: de(data)?,
                        channel,
                    },
                    "trades" => Self::Trades {
                        data: de(data)?,
                        channel,
                    },
                    "spot_feed" => Self::SpotFeed {
                        data: de(data)?,
                        channel,
                    },
                    "auctions" if channel == "auctions.watch" => Self::AuctionsWatch {
                        data: one_or_many(data)?,
                        channel,
                    },
                    "margin" if channel == "margin.watch" => Self::MarginWatch {
                        data: one_or_many(data)?,
                        channel,
                    },
                    _ => return Err(anyhow!("Derive ws: unknown channel {channel}")),
                })
            }
            _ => Err(anyhow!("Derive ws: unrecognised frame: {text}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Live frames from wss://api.derive.xyz/v3/ws on 2026-11-16.

    fn fx(name: &str) -> String {
        std::fs::read_to_string(format!(
            "{}/tests/fixtures/ws_{name}.json",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap()
    }

    #[test]
    fn orderbook() {
        match DeriveWsMessage::parse(&fx("orderbook")).unwrap() {
            DeriveWsMessage::Orderbook { channel, data } => {
                assert_eq!(channel, "orderbook.ETH-PERP.1.10");
                assert_eq!(data.bids.len(), 10);
                assert_eq!(data.asks[0][0], "1808.77");
            }
            m => panic!("{m:?}"),
        }
    }

    #[test]
    fn ticker_slim() {
        match DeriveWsMessage::parse(&fx("ticker_slim")).unwrap() {
            DeriveWsMessage::TickerSlim { data, .. } => {
                assert_eq!(data.instrument_ticker.best_ask_price, "1808.77");
            }
            m => panic!("{m:?}"),
        }
    }

    #[test]
    fn trades() {
        for name in ["trades_instrument", "trades_option_currency"] {
            match DeriveWsMessage::parse(&fx(name)).unwrap() {
                DeriveWsMessage::Trades { data, .. } => {
                    assert_eq!(data.len(), 1);
                    assert!(data[0].tx_hash.is_none());
                }
                m => panic!("{m:?}"),
            }
        }
        match DeriveWsMessage::parse(&fx("trades_option_currency")).unwrap() {
            DeriveWsMessage::Trades { data, .. } => assert!(data[0].rfq_id.is_some()),
            m => panic!("{m:?}"),
        }
    }

    #[test]
    fn spot_feed() {
        match DeriveWsMessage::parse(&fx("spot_feed")).unwrap() {
            DeriveWsMessage::SpotFeed { data, .. } => {
                assert_eq!(data.feeds["ETH"].confidence, "1");
            }
            m => panic!("{m:?}"),
        }
    }

    #[test]
    fn acks_and_errors() {
        match DeriveWsMessage::parse(&fx("subscribe_ok")).unwrap() {
            DeriveWsMessage::SubscriptionResult { result, .. } => {
                assert_eq!(result.status["trades.ETH-PERP"], "ok");
                assert!(result.current_subscriptions.is_some());
            }
            m => panic!("{m:?}"),
        }
        match DeriveWsMessage::parse(&fx("unsubscribe_ok")).unwrap() {
            DeriveWsMessage::SubscriptionResult { result, .. } => {
                assert!(result.remaining_subscriptions.is_some());
            }
            m => panic!("{m:?}"),
        }
        match DeriveWsMessage::parse(&fx("subscribe_error")).unwrap() {
            DeriveWsMessage::Error { error, .. } => assert_eq!(error.code, 13000),
            m => panic!("{m:?}"),
        }
    }

    #[test]
    fn auctions_and_margin_watch_spec_shapes() {
        // No frames were published during capture; payloads follow the asyncapi spec.
        let a = r#"{"method":"subscription","params":{"channel":"auctions.watch","data":[{"subaccount_id":5,"state":"ended","timestamp":1,"details":null}]}}"#;
        assert!(matches!(
            DeriveWsMessage::parse(a).unwrap(),
            DeriveWsMessage::AuctionsWatch { .. }
        ));
        let bad = r#"{"method":"subscription","params":{"channel":"auctions.watch","data":{"subaccount_id":5,"timestamp":1}}}"#;
        assert!(DeriveWsMessage::parse(bad).is_err());
        let m = r#"{"method":"subscription","params":{"channel":"margin.watch","data":{"subaccount_id":5,"currency":"ETH","margin_type":"SM","initial_margin":"1","maintenance_margin":"1","subaccount_value":"2","valuation_timestamp":1,"collaterals":[],"positions":[]}}}"#;
        assert!(matches!(
            DeriveWsMessage::parse(m).unwrap(),
            DeriveWsMessage::MarginWatch { .. }
        ));
    }

    #[test]
    fn strictness() {
        let mut v: serde_json::Value = serde_json::from_str(&fx("orderbook")).unwrap();
        v["params"]["data"]
            .as_object_mut()
            .unwrap()
            .remove("publish_id");
        assert!(DeriveWsMessage::parse(&v.to_string()).is_err());
        let mut v: serde_json::Value = serde_json::from_str(&fx("spot_feed")).unwrap();
        v["params"]["data"]["feeds"]["ETH"]
            .as_object_mut()
            .unwrap()
            .remove("price_prev_daily");
        assert!(DeriveWsMessage::parse(&v.to_string()).is_err());
        let mut v: serde_json::Value = serde_json::from_str(&fx("trades_instrument")).unwrap();
        v["params"]["data"][0]
            .as_object_mut()
            .unwrap()
            .remove("trade_id");
        assert!(DeriveWsMessage::parse(&v.to_string()).is_err());
        assert!(DeriveWsMessage::parse(
            r#"{"method":"subscription","params":{"channel":"nope.x","data":{}}}"#
        )
        .is_err());
    }
}
