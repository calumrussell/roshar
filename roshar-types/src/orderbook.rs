use compact_str::CompactString;
use ordered_float::OrderedFloat;
use std::cmp::Reverse;
use std::collections::BTreeMap;
use thiserror::Error;

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

/// Type aliases for order book storage
pub type BidMap = BTreeMap<Reverse<OrderedFloat<f64>>, CompactString>;
pub type AskMap = BTreeMap<OrderedFloat<f64>, CompactString>;

/// Owns the order book data and handles mutation.
/// Exchange-specific order books should own this and mutate it directly.
#[derive(Debug, Clone)]
pub struct OrderBookState {
    bids: BidMap,
    asks: AskMap,
    max_depth: usize,
}

impl OrderBookState {
    pub fn new(max_depth: usize) -> Self {
        Self {
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
            max_depth,
        }
    }

    /// Set or update a bid level. If size is zero or empty, removes the level.
    /// Returns error if size cannot be parsed as f64.
    #[inline]
    pub fn set_bid(&mut self, price: f64, size: &str) -> Result<(), LocalOrderBookError> {
        let key = Reverse(OrderedFloat(price));
        if size.is_empty() {
            self.bids.remove(&key);
            return Ok(());
        }
        let size_f64 = size.parse::<f64>().map_err(|_| {
            LocalOrderBookError::UnparseableInputs(
                format!("bid size: {}", size),
                format!("price: {}", price),
            )
        })?;
        if size_f64 == 0.0 {
            self.bids.remove(&key);
        } else {
            self.bids.insert(key, size.into());
            self.trim_bids();
        }
        Ok(())
    }

    /// Set or update an ask level. If size is zero or empty, removes the level.
    /// Returns error if size cannot be parsed as f64.
    #[inline]
    pub fn set_ask(&mut self, price: f64, size: &str) -> Result<(), LocalOrderBookError> {
        let key = OrderedFloat(price);
        if size.is_empty() {
            self.asks.remove(&key);
            return Ok(());
        }
        let size_f64 = size.parse::<f64>().map_err(|_| {
            LocalOrderBookError::UnparseableInputs(
                format!("ask size: {}", size),
                format!("price: {}", price),
            )
        })?;
        if size_f64 == 0.0 {
            self.asks.remove(&key);
        } else {
            self.asks.insert(key, size.into());
            self.trim_asks();
        }
        Ok(())
    }

    /// Remove a bid level by price
    #[inline]
    pub fn remove_bid(&mut self, price: f64) {
        self.bids.remove(&Reverse(OrderedFloat(price)));
    }

    /// Remove an ask level by price
    #[inline]
    pub fn remove_ask(&mut self, price: f64) {
        self.asks.remove(&OrderedFloat(price));
    }

    /// Clear all levels
    pub fn clear(&mut self) {
        self.bids.clear();
        self.asks.clear();
    }

    /// Get a read-only view for calculations
    pub fn as_view(&self) -> LocalOrderBook<'_> {
        LocalOrderBook::new(&self.bids, &self.asks)
    }

    /// Get BBO directly without creating a view
    pub fn get_bbo(&self) -> (Option<f64>, Option<f64>) {
        let bid = self.bids.keys().next().map(|k| k.0 .0);
        let ask = self.asks.keys().next().map(|k| k.0);
        (bid, ask)
    }

    /// Get BBO as strings (for compatibility)
    pub fn get_bbo_strings(&self) -> (String, String) {
        let bid = self
            .bids
            .keys()
            .next()
            .map_or(String::new(), |k| k.0 .0.to_string());
        let ask = self
            .asks
            .keys()
            .next()
            .map_or(String::new(), |k| k.0.to_string());
        (bid, ask)
    }

    fn trim_bids(&mut self) {
        if self.bids.len() > self.max_depth {
            let keys_to_remove: Vec<_> = self.bids.keys().skip(self.max_depth).cloned().collect();
            for key in keys_to_remove {
                self.bids.remove(&key);
            }
        }
    }

    fn trim_asks(&mut self) {
        if self.asks.len() > self.max_depth {
            let keys_to_remove: Vec<_> = self.asks.keys().skip(self.max_depth).cloned().collect();
            for key in keys_to_remove {
                self.asks.remove(&key);
            }
        }
    }

    /// Direct access to bids for advanced use cases
    pub fn bids(&self) -> &BidMap {
        &self.bids
    }

    /// Direct access to asks for advanced use cases
    pub fn asks(&self) -> &AskMap {
        &self.asks
    }

    /// Check if the book is empty
    pub fn is_empty(&self) -> bool {
        self.bids.is_empty() && self.asks.is_empty()
    }
}

/// Read-only view over order book data for calculations.
/// Borrows the underlying BTreeMaps - no copying.
#[derive(Debug)]
pub struct LocalOrderBook<'a> {
    bids: &'a BidMap,
    asks: &'a AskMap,
}

impl<'a> LocalOrderBook<'a> {
    pub fn new(bids: &'a BidMap, asks: &'a AskMap) -> Self {
        Self { bids, asks }
    }

    pub fn get_bbo(&self) -> (String, String) {
        let bid = self
            .bids
            .keys()
            .next()
            .map_or(String::new(), |k| k.0 .0.to_string());
        let ask = self
            .asks
            .keys()
            .next()
            .map_or(String::new(), |k| k.0.to_string());
        (bid, ask)
    }

    pub fn get_bbo_f64(&self) -> (Option<f64>, Option<f64>) {
        let bid = self.bids.keys().next().map(|k| k.0 .0);
        let ask = self.asks.keys().next().map(|k| k.0);
        (bid, ask)
    }

    pub fn calc_top_n_exp_weight(&self, n: usize, exp: f64) -> f64 {
        let mut b_sz = 0.0;
        let mut a_sz = 0.0;

        for (i, (_, size)) in self.bids.iter().take(n.min(20)).enumerate() {
            let weight = (-exp * i as f64).exp();
            b_sz += weight * size.parse::<f64>().unwrap_or(0.0);
        }

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

    pub fn bid_prices(&self) -> Vec<String> {
        self.bids.keys().map(|k| k.0 .0.to_string()).collect()
    }

    pub fn ask_prices(&self) -> Vec<String> {
        self.asks.keys().map(|k| k.0.to_string()).collect()
    }

    pub fn bid_sizes(&self) -> Vec<String> {
       self.bids.values().map(|v| v.to_string()).collect()
    }

    pub fn ask_sizes(&self) -> Vec<String> {
        self.asks.values().map(|v| v.to_string()).collect()
    }

    /// Convert the view to a DepthSnapshotData for database storage.
    /// This copies the data since DepthSnapshotData needs owned Vecs.
    pub fn to_depth_snapshot_data(
        &self,
        venue: crate::Venue,
        ticker: String,
    ) -> crate::DepthSnapshotData {
        let now = chrono::Utc::now();
        crate::DepthSnapshotData {
            bid_prices: self.bid_prices(),
            bid_sizes: self.bid_sizes(),
            ask_prices: self.ask_prices(),
            ask_sizes: self.ask_sizes(),
            time: now.timestamp_millis() as u64,
            time_ts: now,
            ticker,
            venue,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_order_book_state_set_bid() {
        let mut state = OrderBookState::new(50);
        state.set_bid(100.0, "1.5").unwrap();
        state.set_bid(101.0, "2.0").unwrap();
        state.set_bid(99.0, "3.0").unwrap();

        let view = state.as_view();
        let prices = view.bid_prices();

        // Bids should be sorted descending (highest first)
        assert_eq!(prices[0], "101");
        assert_eq!(prices[1], "100");
        assert_eq!(prices[2], "99");
    }

    #[test]
    fn test_order_book_state_set_ask() {
        let mut state = OrderBookState::new(50);
        state.set_ask(103.0, "1.5").unwrap();
        state.set_ask(101.0, "2.0").unwrap();
        state.set_ask(102.0, "3.0").unwrap();

        let view = state.as_view();
        let prices = view.ask_prices();

        // Asks should be sorted ascending (lowest first)
        assert_eq!(prices[0], "101");
        assert_eq!(prices[1], "102");
        assert_eq!(prices[2], "103");
    }

    #[test]
    fn test_order_book_state_remove_on_zero() {
        let mut state = OrderBookState::new(50);
        state.set_bid(100.0, "1.5").unwrap();
        state.set_bid(100.0, "0").unwrap();

        assert!(state.bids().is_empty());
    }

    #[test]
    fn test_order_book_state_max_depth() {
        let mut state = OrderBookState::new(2);
        state.set_bid(100.0, "1.0").unwrap();
        state.set_bid(101.0, "2.0").unwrap();
        state.set_bid(102.0, "3.0").unwrap();
        state.set_bid(99.0, "4.0").unwrap();

        // Should only keep top 2 bids (102, 101)
        assert_eq!(state.bids().len(), 2);
        let view = state.as_view();
        let prices = view.bid_prices();
        assert_eq!(prices[0], "102");
        assert_eq!(prices[1], "101");
    }

    #[test]
    fn test_order_book_state_get_bbo() {
        let mut state = OrderBookState::new(50);
        state.set_bid(100.0, "1.0").unwrap();
        state.set_bid(101.0, "2.0").unwrap();
        state.set_ask(102.0, "1.5").unwrap();
        state.set_ask(103.0, "2.5").unwrap();

        let (bid, ask) = state.get_bbo();
        assert_eq!(bid, Some(101.0));
        assert_eq!(ask, Some(102.0));
    }

    #[test]
    fn test_local_order_book_view_get_bbo() {
        let mut state = OrderBookState::new(50);
        state.set_bid(100.0, "1.0").unwrap();
        state.set_ask(101.0, "1.5").unwrap();

        let view = state.as_view();
        let (bid, ask) = view.get_bbo();
        assert_eq!(bid, "100");
        assert_eq!(ask, "101");
    }

    #[test]
    fn test_order_book_state_clear() {
        let mut state = OrderBookState::new(50);
        state.set_bid(100.0, "1.0").unwrap();
        state.set_ask(101.0, "1.5").unwrap();

        state.clear();

        assert!(state.is_empty());
    }
}
