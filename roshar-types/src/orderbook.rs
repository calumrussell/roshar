use chrono::{DateTime, Utc};
#[cfg(feature = "clickhouse")]
use clickhouse::Row;
use ordered_float::OrderedFloat;
use serde::Serialize;
use std::cmp::Reverse;
use std::collections::BTreeMap;
use thiserror::Error;

use crate::{DepthSnapshotData, DepthUpdate, Venue};

#[derive(Debug, Error)]
pub enum LocalOrderBookError {
    #[error("Bid Above Ask of {0}/{1} for {2}/{3}")]
    BidAboveAsk(String, String, String, String),
    #[error("Attempted to update {0}/{1} before initial snapshot")]
    BookUpdateBeforeSnapshot(String, String),
    #[error("Unparseable input for order book update for {0}/{1}")]
    UnparseableInputs(String, String),
    #[error("Out of order updates for {0}/{1} expecting {2} but have {3}")]
    OutOfOrderUpdate(String, String, i64, i64),
    #[error("Wrong symbol: expected {0} but received message for {1}")]
    WrongSymbol(String, String),
    #[error("Message is not a partial update for {0}/{1}")]
    NotPartialUpdate(String, String),
}

#[derive(Debug)]
pub struct LocalOrderBook {
    // BTreeMap for O(log n) operations while maintaining sort order
    // Reverse for bids (highest price first), normal for asks (lowest price first)
    // CompactString avoids heap allocations for small strings (prices/sizes typically < 24 bytes)
    pub(crate) bids: BTreeMap<Reverse<OrderedFloat<f64>>, compact_str::CompactString>,
    pub(crate) asks: BTreeMap<OrderedFloat<f64>, compact_str::CompactString>,
    pub(crate) last_update: i64,
    pub(crate) last_update_ts: DateTime<Utc>,
    pub(crate) exchange: compact_str::CompactString,
    pub(crate) coin: compact_str::CompactString,
}

// Manual Clone implementation for LocalOrderBook
impl Clone for LocalOrderBook {
    fn clone(&self) -> Self {
        Self {
            bids: self.bids.clone(),
            asks: self.asks.clone(),
            last_update: self.last_update,
            last_update_ts: self.last_update_ts,
            exchange: self.exchange.clone(),
            coin: self.coin.clone(),
        }
    }
}

// Manual Serialize implementation for LocalOrderBook
impl serde::Serialize for LocalOrderBook {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("LocalOrderBook", 8)?;

        // Build Vecs on-demand from BTreeMap
        let bid_prices: Vec<String> = self
            .bids
            .keys()
            .map(|price| price.0.0.to_string())
            .collect();
        let bid_sizes: Vec<String> = self.bids.values().map(|size| size.to_string()).collect();
        let ask_prices: Vec<String> = self.asks.keys().map(|price| price.0.to_string()).collect();
        let ask_sizes: Vec<String> = self.asks.values().map(|size| size.to_string()).collect();

        state.serialize_field("bid_prices", &bid_prices)?;
        state.serialize_field("bid_sizes", &bid_sizes)?;
        state.serialize_field("ask_prices", &ask_prices)?;
        state.serialize_field("ask_sizes", &ask_sizes)?;
        state.serialize_field("last_update", &self.last_update)?;
        state.serialize_field("last_update_ts", &self.last_update_ts)?;
        state.serialize_field("exchange", self.exchange.as_str())?;
        state.serialize_field("coin", self.coin.as_str())?;
        state.end()
    }
}

// Manual Row implementation for ClickHouse compatibility
#[cfg(feature = "clickhouse")]
impl clickhouse::Row for LocalOrderBook {
    const COLUMN_NAMES: &'static [&'static str] = &[
        "bid_prices",
        "bid_sizes",
        "ask_prices",
        "ask_sizes",
        "last_update",
        "last_update_ts",
        "exchange",
        "coin",
    ];
}

// Helper struct for ClickHouse Row serialization
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "clickhouse", derive(clickhouse::Row))]
pub struct LocalOrderBookData {
    pub bid_prices: Vec<String>,
    pub bid_sizes: Vec<String>,
    pub ask_prices: Vec<String>,
    pub ask_sizes: Vec<String>,
    pub last_update: i64,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub last_update_ts: DateTime<Utc>,
    pub exchange: String,
    pub coin: String,
}

impl LocalOrderBook {
    pub fn new(ts: i64, exchange: String, coin: String) -> Self {
        Self {
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
            last_update: ts,
            last_update_ts: DateTime::from_timestamp_millis(ts).unwrap_or_default(),
            exchange: exchange.into(),
            coin: coin.into(),
        }
    }

    /// Create a LocalOrderBook from a DepthSnapshotData
    pub fn from_depth_snapshot(snapshot: &DepthSnapshotData, max_depth: usize) -> Self {
        let mut book = Self::new(
            snapshot.time as i64,
            snapshot.venue.as_str().to_string(),
            snapshot.ticker.clone(),
        );

        // Build updates from snapshot data
        let mut updates = Vec::new();

        // Add bids
        for (px, qty) in snapshot.bid_prices.iter().zip(snapshot.bid_sizes.iter()) {
            if let (Ok(price), Ok(quantity)) = (px.parse::<f64>(), qty.parse::<f64>()) {
                if quantity > 0.0 {
                    updates.push(DepthUpdate {
                        time: snapshot.time as i64,
                        exchange: snapshot.venue.as_str().to_string(),
                        side: false,
                        coin: snapshot.ticker.clone(),
                        px: price,
                        sz: quantity,
                    });
                }
            }
        }

        // Add asks
        for (px, qty) in snapshot.ask_prices.iter().zip(snapshot.ask_sizes.iter()) {
            if let (Ok(price), Ok(quantity)) = (px.parse::<f64>(), qty.parse::<f64>()) {
                if quantity > 0.0 {
                    updates.push(DepthUpdate {
                        time: snapshot.time as i64,
                        exchange: snapshot.venue.as_str().to_string(),
                        side: true,
                        coin: snapshot.ticker.clone(),
                        px: price,
                        sz: quantity,
                    });
                }
            }
        }

        book.apply_updates(&updates, max_depth);
        book
    }

    pub fn get_bbo(&self) -> (String, String) {
        let bid = self
            .bids
            .iter()
            .next()
            .map_or_else(String::new, |(price, _)| price.0.0.to_string());
        let ask = self
            .asks
            .iter()
            .next()
            .map_or_else(String::new, |(price, _)| price.0.to_string());
        (bid, ask)
    }

    pub fn diff(&self, other: &LocalOrderBook) -> Vec<DepthUpdate> {
        let mut diffs = Vec::new();

        // Check bids
        for (price, sz) in other.bids.iter() {
            let px = price.0.0; // Unwrap Reverse<OrderedFloat<f64>>
            match self.bids.get(price) {
                Some(current_sz) if current_sz != sz => {
                    diffs.push(DepthUpdate {
                        time: other.last_update,
                        exchange: other.exchange.to_string(),
                        side: false, // false for bid
                        coin: other.coin.to_string(),
                        px,
                        sz: sz.parse::<f64>().unwrap_or(0.0),
                    });
                }
                None => {
                    diffs.push(DepthUpdate {
                        time: other.last_update,
                        exchange: other.exchange.to_string(),
                        side: false, // false for bid
                        coin: other.coin.to_string(),
                        px,
                        sz: sz.parse::<f64>().unwrap_or(0.0),
                    });
                }
                _ => {}
            }
        }

        // Check for removed bids
        for (price, _) in self.bids.iter() {
            if !other.bids.contains_key(price) {
                diffs.push(DepthUpdate {
                    time: other.last_update,
                    exchange: other.exchange.to_string(),
                    side: false, // false for bid
                    coin: other.coin.to_string(),
                    px: price.0.0,
                    sz: 0.0,
                });
            }
        }

        // Check asks
        for (price, sz) in other.asks.iter() {
            let px = price.0; // Unwrap OrderedFloat<f64>
            match self.asks.get(price) {
                Some(current_sz) if current_sz != sz => {
                    diffs.push(DepthUpdate {
                        time: other.last_update,
                        exchange: other.exchange.to_string(),
                        side: true, // true for ask
                        coin: other.coin.to_string(),
                        px,
                        sz: sz.parse::<f64>().unwrap_or(0.0),
                    });
                }
                None => {
                    diffs.push(DepthUpdate {
                        time: other.last_update,
                        exchange: other.exchange.to_string(),
                        side: true, // true for ask
                        coin: other.coin.to_string(),
                        px,
                        sz: sz.parse::<f64>().unwrap_or(0.0),
                    });
                }
                _ => {}
            }
        }

        // Check for removed asks
        for (price, _) in self.asks.iter() {
            if !other.asks.contains_key(price) {
                diffs.push(DepthUpdate {
                    time: other.last_update,
                    exchange: other.exchange.to_string(),
                    side: true, // true for ask
                    coin: other.coin.to_string(),
                    px: price.0,
                    sz: 0.0,
                });
            }
        }

        diffs
    }

    pub fn apply_updates(&mut self, updates: &Vec<DepthUpdate>, max_depth: usize) {
        if updates.is_empty() {
            return;
        }

        // Get exchange and coin from the first update if available
        if let Some(first_update) = updates.first() {
            if self.exchange.is_empty() {
                self.exchange = first_update.exchange.as_str().into();
            }
            if self.coin.is_empty() {
                self.coin = first_update.coin.as_str().into();
            }
        }

        // Use BTreeMap for efficient updates, then sync to Vec
        for update in updates {
            let price = OrderedFloat(update.px);
            let size = compact_str::CompactString::new(update.sz.to_string());

            if update.side {
                // ask/sell side (true)
                if update.sz == 0.0 {
                    // Remove level
                    self.asks.remove(&price);
                } else {
                    // Insert or update level
                    self.asks.insert(price, size);
                }
            } else {
                // bid/buy side (false)
                let reverse_price = Reverse(price);
                if update.sz == 0.0 {
                    // Remove level
                    self.bids.remove(&reverse_price);
                } else {
                    // Insert or update level
                    self.bids.insert(reverse_price, size);
                }
            }

            self.last_update = update.time;
            self.last_update_ts = DateTime::from_timestamp_millis(update.time).unwrap();
        }

        // Trim to max_depth if needed
        if self.bids.len() > max_depth {
            let keys_to_remove: Vec<_> = self.bids.keys().skip(max_depth).cloned().collect();
            for key in keys_to_remove {
                self.bids.remove(&key);
            }
        }

        if self.asks.len() > max_depth {
            let keys_to_remove: Vec<_> = self.asks.keys().skip(max_depth).cloned().collect();
            for key in keys_to_remove {
                self.asks.remove(&key);
            }
        }
    }

    pub fn calc_top_n_exp_weight(&self, n: usize, exp: f64) -> f64 {
        // Calculate exponentially weighted order imbalance in single pass
        let mut b_sz = 0.0;
        let mut a_sz = 0.0;

        // Process bids directly from BTreeMap
        for (i, (_, size)) in self.bids.iter().take(n.min(20)).enumerate() {
            let weight = (-exp * i as f64).exp();
            b_sz += weight * size.parse::<f64>().unwrap_or(0.0);
        }

        // Process asks directly from BTreeMap
        for (i, (_, size)) in self.asks.iter().take(n.min(20)).enumerate() {
            let weight = (-exp * i as f64).exp();
            a_sz += weight * size.parse::<f64>().unwrap_or(0.0);
        }

        if b_sz + a_sz > 0.0 {
            (b_sz - a_sz) / (b_sz + a_sz)
        } else {
            0.0
        }
    }

    // Helper method to get snapshot for serialization/cloning
    pub fn to_data(&self) -> LocalOrderBookData {
        // Build Vecs on-demand from BTreeMap
        let bid_prices: Vec<String> = self
            .bids
            .keys()
            .map(|price| price.0.0.to_string())
            .collect();
        let bid_sizes: Vec<String> = self.bids.values().map(|size| size.to_string()).collect();
        let ask_prices: Vec<String> = self.asks.keys().map(|price| price.0.to_string()).collect();
        let ask_sizes: Vec<String> = self.asks.values().map(|size| size.to_string()).collect();

        LocalOrderBookData {
            bid_prices,
            bid_sizes,
            ask_prices,
            ask_sizes,
            last_update: self.last_update,
            last_update_ts: self.last_update_ts,
            exchange: self.exchange.to_string(),
            coin: self.coin.to_string(),
        }
    }

    // Convert LocalOrderBook to DepthSnapshotData for new daily table
    pub fn to_depth_snapshot(&self, venue: Venue, ticker: String) -> DepthSnapshotData {
        // Build Vecs on-demand from BTreeMap
        let bid_prices: Vec<String> = self
            .bids
            .keys()
            .map(|price| price.0.0.to_string())
            .collect();
        let bid_sizes: Vec<String> = self.bids.values().map(|size| size.to_string()).collect();
        let ask_prices: Vec<String> = self.asks.keys().map(|price| price.0.to_string()).collect();
        let ask_sizes: Vec<String> = self.asks.values().map(|size| size.to_string()).collect();

        DepthSnapshotData {
            bid_prices,
            bid_sizes,
            ask_prices,
            ask_sizes,
            time: self.last_update as u64,
            time_ts: DateTime::from_timestamp_millis(self.last_update).unwrap_or_default(),
            ticker,
            venue,
        }
    }

    // Test helper methods for accessing private fields
    #[cfg(test)]
    pub fn test_bid_prices(&self) -> Vec<String> {
        self.bids
            .keys()
            .map(|price| price.0.0.to_string())
            .collect()
    }

    #[cfg(test)]
    pub fn test_ask_prices(&self) -> Vec<String> {
        self.asks.keys().map(|price| price.0.to_string()).collect()
    }

    #[cfg(test)]
    pub fn test_bid_sizes(&self) -> Vec<String> {
        self.bids.values().map(|size| size.to_string()).collect()
    }

    #[cfg(test)]
    pub fn test_ask_sizes(&self) -> Vec<String> {
        self.asks.values().map(|size| size.to_string()).collect()
    }

    #[cfg(test)]
    pub fn test_coin(&self) -> String {
        self.coin.to_string()
    }

    #[cfg(test)]
    pub fn test_exchange(&self) -> String {
        self.exchange.to_string()
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::DepthUpdate;

    #[test]
    fn test_local_order_book_apply_updates_bid_sorting() {
        let mut order_book =
            LocalOrderBook::new(1000000, "test_exchange".to_string(), "BTC".to_string());

        // Create bid updates in random order
        let updates = vec![
            DepthUpdate {
                px: 100.0,
                sz: 1.0,
                time: 1000000,
                exchange: "test_exchange".to_string(),
                side: false, // bid
                coin: "BTC".to_string(),
            },
            DepthUpdate {
                px: 102.0,
                sz: 2.0,
                time: 1000000,
                exchange: "test_exchange".to_string(),
                side: false, // bid
                coin: "BTC".to_string(),
            },
            DepthUpdate {
                px: 99.0,
                sz: 3.0,
                time: 1000000,
                exchange: "test_exchange".to_string(),
                side: false, // bid
                coin: "BTC".to_string(),
            },
            DepthUpdate {
                px: 101.0,
                sz: 4.0,
                time: 1000000,
                exchange: "test_exchange".to_string(),
                side: false, // bid
                coin: "BTC".to_string(),
            },
        ];

        order_book.apply_updates(&updates, 50);

        let bid_prices = order_book.test_bid_prices();
        let bid_sizes = order_book.test_bid_sizes();

        // Check that bid prices are sorted in descending order (highest first)
        assert_eq!(bid_prices[0], "102");
        assert_eq!(bid_prices[1], "101");
        assert_eq!(bid_prices[2], "100");
        assert_eq!(bid_prices[3], "99");

        // Check that bid sizes match their corresponding prices
        assert_eq!(bid_sizes[0], "2"); // size for 102
        assert_eq!(bid_sizes[1], "4"); // size for 101
        assert_eq!(bid_sizes[2], "1"); // size for 100
        assert_eq!(bid_sizes[3], "3"); // size for 99

        // Check that get_bbo returns the best bid
        let (best_bid, _) = order_book.get_bbo();
        assert_eq!(best_bid, "102");
    }

    #[test]
    fn test_local_order_book_apply_updates_ask_sorting() {
        let mut order_book =
            LocalOrderBook::new(1000000, "test_exchange".to_string(), "BTC".to_string());

        // Create ask updates in random order
        let updates = vec![
            DepthUpdate {
                px: 105.0,
                sz: 1.0,
                time: 1000000,
                exchange: "test_exchange".to_string(),
                side: true, // ask
                coin: "BTC".to_string(),
            },
            DepthUpdate {
                px: 103.0,
                sz: 2.0,
                time: 1000000,
                exchange: "test_exchange".to_string(),
                side: true, // ask
                coin: "BTC".to_string(),
            },
            DepthUpdate {
                px: 106.0,
                sz: 3.0,
                time: 1000000,
                exchange: "test_exchange".to_string(),
                side: true, // ask
                coin: "BTC".to_string(),
            },
            DepthUpdate {
                px: 104.0,
                sz: 4.0,
                time: 1000000,
                exchange: "test_exchange".to_string(),
                side: true, // ask
                coin: "BTC".to_string(),
            },
        ];

        order_book.apply_updates(&updates, 50);

        let ask_prices = order_book.test_ask_prices();
        let ask_sizes = order_book.test_ask_sizes();

        // Check that ask prices are sorted in ascending order (lowest first)
        assert_eq!(ask_prices[0], "103");
        assert_eq!(ask_prices[1], "104");
        assert_eq!(ask_prices[2], "105");
        assert_eq!(ask_prices[3], "106");

        // Check that ask sizes match their corresponding prices
        assert_eq!(ask_sizes[0], "2"); // size for 103
        assert_eq!(ask_sizes[1], "4"); // size for 104
        assert_eq!(ask_sizes[2], "1"); // size for 105
        assert_eq!(ask_sizes[3], "3"); // size for 106

        // Check that get_bbo returns the best ask
        let (_, best_ask) = order_book.get_bbo();
        assert_eq!(best_ask, "103");
    }

    #[test]
    fn test_local_order_book_apply_updates_mixed_bids_asks() {
        let mut order_book =
            LocalOrderBook::new(1000000, "test_exchange".to_string(), "BTC".to_string());

        // Create mixed bid and ask updates
        let updates = vec![
            DepthUpdate {
                px: 101.0,
                sz: 1.0,
                time: 1000000,
                exchange: "test_exchange".to_string(),
                side: false, // bid
                coin: "BTC".to_string(),
            },
            DepthUpdate {
                px: 103.0,
                sz: 2.0,
                time: 1000000,
                exchange: "test_exchange".to_string(),
                side: true, // ask
                coin: "BTC".to_string(),
            },
            DepthUpdate {
                px: 102.0,
                sz: 3.0,
                time: 1000000,
                exchange: "test_exchange".to_string(),
                side: false, // bid
                coin: "BTC".to_string(),
            },
            DepthUpdate {
                px: 104.0,
                sz: 4.0,
                time: 1000000,
                exchange: "test_exchange".to_string(),
                side: true, // ask
                coin: "BTC".to_string(),
            },
        ];

        order_book.apply_updates(&updates, 50);

        let bid_prices = order_book.test_bid_prices();
        let bid_sizes = order_book.test_bid_sizes();
        let ask_prices = order_book.test_ask_prices();
        let ask_sizes = order_book.test_ask_sizes();

        // Check bid sorting (descending)
        assert_eq!(bid_prices[0], "102");
        assert_eq!(bid_prices[1], "101");
        assert_eq!(bid_sizes[0], "3"); // size for 102
        assert_eq!(bid_sizes[1], "1"); // size for 101

        // Check ask sorting (ascending)
        assert_eq!(ask_prices[0], "103");
        assert_eq!(ask_prices[1], "104");
        assert_eq!(ask_sizes[0], "2"); // size for 103
        assert_eq!(ask_sizes[1], "4"); // size for 104

        // Check BBO
        let (best_bid, best_ask) = order_book.get_bbo();
        assert_eq!(best_bid, "102");
        assert_eq!(best_ask, "103");
    }

    #[test]
    fn test_local_order_book_apply_updates_with_removals() {
        let mut order_book =
            LocalOrderBook::new(1000000, "test_exchange".to_string(), "BTC".to_string());

        // Add some initial updates
        let initial_updates = vec![
            DepthUpdate {
                px: 100.0,
                sz: 1.0,
                time: 1000000,
                exchange: "test_exchange".to_string(),
                side: false, // bid
                coin: "BTC".to_string(),
            },
            DepthUpdate {
                px: 102.0,
                sz: 2.0,
                time: 1000000,
                exchange: "test_exchange".to_string(),
                side: false, // bid
                coin: "BTC".to_string(),
            },
            DepthUpdate {
                px: 103.0,
                sz: 3.0,
                time: 1000000,
                exchange: "test_exchange".to_string(),
                side: true, // ask
                coin: "BTC".to_string(),
            },
        ];

        order_book.apply_updates(&initial_updates, 50);

        // Now remove the best bid
        let removal_updates = vec![DepthUpdate {
            px: 102.0,
            sz: 0.0, // size 0 means removal
            time: 1000001,
            exchange: "test_exchange".to_string(),
            side: false, // bid
            coin: "BTC".to_string(),
        }];

        order_book.apply_updates(&removal_updates, 50);

        let bid_prices = order_book.test_bid_prices();
        let bid_sizes = order_book.test_bid_sizes();
        let ask_prices = order_book.test_ask_prices();
        let ask_sizes = order_book.test_ask_sizes();

        // Check that the best bid is now 100
        assert_eq!(bid_prices[0], "100");
        assert_eq!(bid_sizes[0], "1");

        // Check that we only have one bid left
        assert_eq!(bid_prices.len(), 1);
        assert_eq!(bid_sizes.len(), 1);

        // Ask should still be there
        assert_eq!(ask_prices[0], "103");
        assert_eq!(ask_sizes[0], "3");
    }

    #[test]
    fn test_local_order_book_max_depth_bids() {
        let max_depth = 2;
        let mut order_book =
            LocalOrderBook::new(1000000, "test_exchange".to_string(), "BTC".to_string());

        // Add 5 bid updates
        let updates = vec![
            DepthUpdate {
                px: 100.0,
                sz: 1.0,
                time: 1000000,
                exchange: "test_exchange".to_string(),
                side: false, // bid
                coin: "BTC".to_string(),
            },
            DepthUpdate {
                px: 102.0,
                sz: 2.0,
                time: 1000000,
                exchange: "test_exchange".to_string(),
                side: false, // bid
                coin: "BTC".to_string(),
            },
            DepthUpdate {
                px: 99.0,
                sz: 3.0,
                time: 1000000,
                exchange: "test_exchange".to_string(),
                side: false, // bid
                coin: "BTC".to_string(),
            },
            DepthUpdate {
                px: 101.0,
                sz: 4.0,
                time: 1000000,
                exchange: "test_exchange".to_string(),
                side: false, // bid
                coin: "BTC".to_string(),
            },
            DepthUpdate {
                px: 98.0,
                sz: 5.0,
                time: 1000000,
                exchange: "test_exchange".to_string(),
                side: false, // bid
                coin: "BTC".to_string(),
            },
        ];

        order_book.apply_updates(&updates, max_depth);

        let bid_prices = order_book.test_bid_prices();
        let bid_sizes = order_book.test_bid_sizes();

        // Should only have 2 levels (max_depth)
        assert_eq!(bid_prices.len(), 2);
        assert_eq!(bid_sizes.len(), 2);

        // Should have the top 2 bids (highest prices: 102 and 101)
        assert_eq!(bid_prices[0], "102");
        assert_eq!(bid_prices[1], "101");
        assert_eq!(bid_sizes[0], "2"); // size for 102
        assert_eq!(bid_sizes[1], "4"); // size for 101

        // Best bid should still work correctly
        let (best_bid, _) = order_book.get_bbo();
        assert_eq!(best_bid, "102");
    }

    #[test]
    fn test_local_order_book_max_depth_asks() {
        let max_depth = 3;
        let mut order_book =
            LocalOrderBook::new(1000000, "test_exchange".to_string(), "BTC".to_string());

        // Add 5 ask updates
        let updates = vec![
            DepthUpdate {
                px: 105.0,
                sz: 1.0,
                time: 1000000,
                exchange: "test_exchange".to_string(),
                side: true, // ask
                coin: "BTC".to_string(),
            },
            DepthUpdate {
                px: 103.0,
                sz: 2.0,
                time: 1000000,
                exchange: "test_exchange".to_string(),
                side: true, // ask
                coin: "BTC".to_string(),
            },
            DepthUpdate {
                px: 106.0,
                sz: 3.0,
                time: 1000000,
                exchange: "test_exchange".to_string(),
                side: true, // ask
                coin: "BTC".to_string(),
            },
            DepthUpdate {
                px: 104.0,
                sz: 4.0,
                time: 1000000,
                exchange: "test_exchange".to_string(),
                side: true, // ask
                coin: "BTC".to_string(),
            },
            DepthUpdate {
                px: 107.0,
                sz: 5.0,
                time: 1000000,
                exchange: "test_exchange".to_string(),
                side: true, // ask
                coin: "BTC".to_string(),
            },
        ];

        order_book.apply_updates(&updates, max_depth);

        let ask_prices = order_book.test_ask_prices();
        let ask_sizes = order_book.test_ask_sizes();

        // Should only have 3 levels (max_depth)
        assert_eq!(ask_prices.len(), 3);
        assert_eq!(ask_sizes.len(), 3);

        // Should have the top 3 asks (lowest prices: 103, 104, 105)
        assert_eq!(ask_prices[0], "103");
        assert_eq!(ask_prices[1], "104");
        assert_eq!(ask_prices[2], "105");
        assert_eq!(ask_sizes[0], "2"); // size for 103
        assert_eq!(ask_sizes[1], "4"); // size for 104
        assert_eq!(ask_sizes[2], "1"); // size for 105

        // Best ask should still work correctly
        let (_, best_ask) = order_book.get_bbo();
        assert_eq!(best_ask, "103");
    }

    #[test]
    fn test_local_order_book_max_depth_mixed() {
        let max_depth = 2;

        let mut order_book =
            LocalOrderBook::new(1000000, "test_exchange".to_string(), "BTC".to_string());

        // Add mixed bid and ask updates (more than max_depth)
        let updates = vec![
            DepthUpdate {
                px: 101.0,
                sz: 1.0,
                time: 1000000,
                exchange: "test_exchange".to_string(),
                side: false, // bid
                coin: "BTC".to_string(),
            },
            DepthUpdate {
                px: 103.0,
                sz: 2.0,
                time: 1000000,
                exchange: "test_exchange".to_string(),
                side: true, // ask
                coin: "BTC".to_string(),
            },
            DepthUpdate {
                px: 102.0,
                sz: 3.0,
                time: 1000000,
                exchange: "test_exchange".to_string(),
                side: false, // bid
                coin: "BTC".to_string(),
            },
            DepthUpdate {
                px: 104.0,
                sz: 4.0,
                time: 1000000,
                exchange: "test_exchange".to_string(),
                side: true, // ask
                coin: "BTC".to_string(),
            },
            DepthUpdate {
                px: 100.0,
                sz: 5.0,
                time: 1000000,
                exchange: "test_exchange".to_string(),
                side: false, // bid
                coin: "BTC".to_string(),
            },
            DepthUpdate {
                px: 105.0,
                sz: 6.0,
                time: 1000000,
                exchange: "test_exchange".to_string(),
                side: true, // ask
                coin: "BTC".to_string(),
            },
        ];

        order_book.apply_updates(&updates, max_depth);

        let bid_prices = order_book.test_bid_prices();
        let bid_sizes = order_book.test_bid_sizes();
        let ask_prices = order_book.test_ask_prices();
        let ask_sizes = order_book.test_ask_sizes();

        // Should have max 2 levels for both bids and asks
        assert_eq!(bid_prices.len(), 2);
        assert_eq!(bid_sizes.len(), 2);
        assert_eq!(ask_prices.len(), 2);
        assert_eq!(ask_sizes.len(), 2);

        // Check top 2 bids (highest prices: 102, 101)
        assert_eq!(bid_prices[0], "102");
        assert_eq!(bid_prices[1], "101");

        // Check top 2 asks (lowest prices: 103, 104)
        assert_eq!(ask_prices[0], "103");
        assert_eq!(ask_prices[1], "104");

        // BBO should still work correctly
        let (best_bid, best_ask) = order_book.get_bbo();
        assert_eq!(best_bid, "102");
        assert_eq!(best_ask, "103");
    }

    #[test]
    fn test_local_order_book_unlimited_depth() {
        let mut order_book =
            LocalOrderBook::new(1000000, "test_exchange".to_string(), "BTC".to_string());

        // Add many updates (should store all since max_depth is 50)
        let updates = vec![
            DepthUpdate {
                px: 100.0,
                sz: 1.0,
                time: 1000000,
                exchange: "test_exchange".to_string(),
                side: false, // bid
                coin: "BTC".to_string(),
            },
            DepthUpdate {
                px: 102.0,
                sz: 2.0,
                time: 1000000,
                exchange: "test_exchange".to_string(),
                side: false, // bid
                coin: "BTC".to_string(),
            },
            DepthUpdate {
                px: 99.0,
                sz: 3.0,
                time: 1000000,
                exchange: "test_exchange".to_string(),
                side: false, // bid
                coin: "BTC".to_string(),
            },
            DepthUpdate {
                px: 101.0,
                sz: 4.0,
                time: 1000000,
                exchange: "test_exchange".to_string(),
                side: false, // bid
                coin: "BTC".to_string(),
            },
            DepthUpdate {
                px: 98.0,
                sz: 5.0,
                time: 1000000,
                exchange: "test_exchange".to_string(),
                side: false, // bid
                coin: "BTC".to_string(),
            },
        ];

        order_book.apply_updates(&updates, 50);

        let bid_prices = order_book.test_bid_prices();
        let bid_sizes = order_book.test_bid_sizes();

        // Should store all 5 levels
        assert_eq!(bid_prices.len(), 5);
        assert_eq!(bid_sizes.len(), 5);

        // Check correct sorting
        assert_eq!(bid_prices[0], "102");
        assert_eq!(bid_prices[1], "101");
        assert_eq!(bid_prices[2], "100");
        assert_eq!(bid_prices[3], "99");
        assert_eq!(bid_prices[4], "98");
    }
}
