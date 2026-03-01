//! Converts normalized data types from `roshar-types` into backtest events.
//!
//! Enable with the `roshar-types` feature flag. This allows feeding data from
//! ClickHouse (or any source that produces `TradeData`, `DepthUpdateData`,
//! `DepthSnapshotData`) directly into the backtest engine without going through
//! text parsing.

use roshar_types::{DepthSnapshotData, DepthUpdateData, TradeData};

use crate::source::VecEventFeed;
use crate::types::{
    Event, EVENT_CLEAR_SIDE_ASK, EVENT_CLEAR_SIDE_BID, EVENT_TRADE_BUY, EVENT_TRADE_SELL,
    EVENT_UPDATE_LEVEL_ASK, EVENT_UPDATE_LEVEL_BID,
};

impl From<&TradeData> for Event {
    fn from(trade: &TradeData) -> Self {
        let typ = if trade.side {
            EVENT_TRADE_BUY
        } else {
            EVENT_TRADE_SELL
        };
        Event::new(typ, trade.time as i64, &trade.px, &trade.qty)
    }
}

impl From<&DepthUpdateData> for Event {
    fn from(update: &DepthUpdateData) -> Self {
        let typ = if update.side {
            EVENT_UPDATE_LEVEL_BID
        } else {
            EVENT_UPDATE_LEVEL_ASK
        };
        Event::new(typ, update.time as i64, &update.px, &update.qty)
    }
}

/// Convert a `DepthSnapshotData` into a series of events:
/// CLEAR_SIDE_BID, CLEAR_SIDE_ASK, then UPDATE_LEVEL for each price level.
pub fn snapshot_to_events(snapshot: &DepthSnapshotData) -> Vec<Event> {
    let ts = snapshot.time as i64;
    let mut events = Vec::with_capacity(2 + snapshot.bid_prices.len() + snapshot.ask_prices.len());

    events.push(Event::new(EVENT_CLEAR_SIDE_BID, ts, "0.0", "0.0"));
    events.push(Event::new(EVENT_CLEAR_SIDE_ASK, ts, "0.0", "0.0"));

    for (px, qty) in snapshot.bid_prices.iter().zip(snapshot.bid_sizes.iter()) {
        events.push(Event::new(EVENT_UPDATE_LEVEL_BID, ts, px, qty));
    }

    for (px, qty) in snapshot.ask_prices.iter().zip(snapshot.ask_sizes.iter()) {
        events.push(Event::new(EVENT_UPDATE_LEVEL_ASK, ts, px, qty));
    }

    events
}

impl VecEventFeed {
    /// Create a feed from depth updates and trades (both sorted by time).
    /// Events are merged in timestamp order.
    pub fn from_depth_updates_and_trades(
        depth_updates: Vec<DepthUpdateData>,
        trades: Vec<TradeData>,
    ) -> Self {
        let mut events: Vec<Event> = Vec::with_capacity(depth_updates.len() + trades.len());

        for du in &depth_updates {
            events.push(du.into());
        }
        for trade in &trades {
            events.push(trade.into());
        }

        // Stable sort preserves insertion order: depth updates before trades
        // at the same timestamp.
        events.sort_by_key(|e| e.ts);
        Self::new(events)
    }

    /// Create a feed from depth snapshots and trades.
    /// Each snapshot generates CLEAR_SIDE + UPDATE_LEVEL events.
    /// All events are merged in timestamp order. Depth events are sorted
    /// before trades at the same timestamp.
    pub fn from_snapshots_and_trades(
        snapshots: Vec<DepthSnapshotData>,
        trades: Vec<TradeData>,
    ) -> Self {
        let mut events: Vec<Event> = Vec::new();

        for snap in &snapshots {
            events.extend(snapshot_to_events(snap));
        }
        for trade in &trades {
            events.push(trade.into());
        }

        // Stable sort preserves insertion order: snapshot events before trades
        // at the same timestamp.
        events.sort_by_key(|e| e.ts);
        Self::new(events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::EventFeed;
    use crate::types::{Candle, EVENT_TRADE_BUY, EVENT_TRADE_SELL};
    use chrono::{TimeZone, Utc};
    use roshar_types::Venue;
    use std::collections::VecDeque;

    fn make_trade(px: &str, qty: &str, time: u64, side: bool) -> TradeData {
        TradeData {
            px: px.to_string(),
            qty: qty.to_string(),
            time,
            time_ts: Utc.timestamp_millis_opt(time as i64).unwrap(),
            ticker: "BTCUSDT".to_string(),
            meta: String::new(),
            side,
            venue: Venue::Binance,
        }
    }

    fn make_depth_update(px: &str, qty: &str, time: u64, side: bool) -> DepthUpdateData {
        DepthUpdateData {
            px: px.to_string(),
            qty: qty.to_string(),
            time,
            time_ts: Utc.timestamp_millis_opt(time as i64).unwrap(),
            ticker: "BTCUSDT".to_string(),
            meta: String::new(),
            side,
            venue: Venue::Binance,
        }
    }

    fn make_snapshot(
        time: u64,
        bid_prices: Vec<&str>,
        bid_sizes: Vec<&str>,
        ask_prices: Vec<&str>,
        ask_sizes: Vec<&str>,
    ) -> DepthSnapshotData {
        DepthSnapshotData {
            bid_prices: bid_prices.into_iter().map(String::from).collect(),
            bid_sizes: bid_sizes.into_iter().map(String::from).collect(),
            ask_prices: ask_prices.into_iter().map(String::from).collect(),
            ask_sizes: ask_sizes.into_iter().map(String::from).collect(),
            time,
            time_ts: Utc.timestamp_millis_opt(time as i64).unwrap(),
            ticker: "BTCUSDT".to_string(),
            venue: Venue::Binance,
        }
    }

    #[test]
    fn test_trade_conversion() {
        let trade = make_trade("100.5", "1.0", 1000, true);
        let event: Event = (&trade).into();
        assert_eq!(event.typ, EVENT_TRADE_BUY);
        assert_eq!(event.ts, 1000);
        assert_eq!(event.px, "100.5");
        assert_eq!(event.qty, "1.0");

        let sell_trade = make_trade("99.5", "2.0", 2000, false);
        let sell_event: Event = (&sell_trade).into();
        assert_eq!(sell_event.typ, EVENT_TRADE_SELL);
    }

    #[test]
    fn test_depth_update_conversion() {
        let bid = make_depth_update("100.0", "50.0", 1000, true);
        let event: Event = (&bid).into();
        assert_eq!(event.typ, EVENT_UPDATE_LEVEL_BID);

        let ask = make_depth_update("101.0", "50.0", 1000, false);
        let event: Event = (&ask).into();
        assert_eq!(event.typ, EVENT_UPDATE_LEVEL_ASK);
    }

    #[test]
    fn test_snapshot_to_events() {
        let snap = make_snapshot(
            1000,
            vec!["100.0", "99.0"],
            vec!["10.0", "20.0"],
            vec!["101.0", "102.0"],
            vec!["15.0", "25.0"],
        );

        let events = snapshot_to_events(&snap);
        // 2 clear + 2 bids + 2 asks = 6
        assert_eq!(events.len(), 6);
        assert_eq!(events[0].typ, EVENT_CLEAR_SIDE_BID);
        assert_eq!(events[1].typ, EVENT_CLEAR_SIDE_ASK);
        assert_eq!(events[2].typ, EVENT_UPDATE_LEVEL_BID);
        assert_eq!(events[2].px, "100.0");
        assert_eq!(events[4].typ, EVENT_UPDATE_LEVEL_ASK);
        assert_eq!(events[4].px, "101.0");
    }

    #[test]
    fn test_from_depth_updates_and_trades() {
        let trades = vec![
            make_trade("100.5", "1.0", 1000, true),
            make_trade("100.6", "2.0", 3000, false),
        ];
        let depth_updates = vec![
            make_depth_update("100.0", "50.0", 2000, true),
            make_depth_update("101.0", "30.0", 4000, false),
        ];

        let mut feed = VecEventFeed::from_depth_updates_and_trades(depth_updates, trades);
        let mut events = VecDeque::new();
        let mut candles: VecDeque<Candle> = VecDeque::new();
        feed.fill(&mut events, &mut candles, 10);

        assert_eq!(events.len(), 4);
        // Should be sorted by timestamp
        assert_eq!(events[0].ts, 1000);
        assert_eq!(events[1].ts, 2000);
        assert_eq!(events[2].ts, 3000);
        assert_eq!(events[3].ts, 4000);
    }

    #[test]
    fn test_from_snapshots_and_trades() {
        let trades = vec![make_trade("100.5", "1.0", 2000, true)];
        let snapshots = vec![make_snapshot(
            1000,
            vec!["100.0"],
            vec!["10.0"],
            vec!["101.0"],
            vec!["15.0"],
        )];

        let mut feed = VecEventFeed::from_snapshots_and_trades(snapshots, trades);
        let mut events = VecDeque::new();
        let mut candles: VecDeque<Candle> = VecDeque::new();
        feed.fill(&mut events, &mut candles, 10);

        // 2 clear + 1 bid + 1 ask + 1 trade = 5
        assert_eq!(events.len(), 5);
        // Snapshot events at ts=1000, trade at ts=2000
        assert_eq!(events[0].ts, 1000);
        assert_eq!(events[4].ts, 2000);
    }
}
