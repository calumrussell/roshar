use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use roshar_bt::exchanges::hyperliquid::HyperliquidCandleParser;
use roshar_bt::l1::backtest::Backtest as L1Backtest;
use roshar_bt::l1::L1ConfigBuilder;
use roshar_bt::l2::backtest::Backtest as L2Backtest;
use roshar_bt::l2::L2ConfigBuilder;
use roshar_bt::source::{BufSource, EventFeed, MultiplexedFeed, ParsedCandleFeed, VecEventFeed};
use roshar_bt::types::{
    Event, EVENT_CLEAR_SIDE_ASK, EVENT_CLEAR_SIDE_BID, EVENT_TRADE_BUY, EVENT_TRADE_SELL,
    EVENT_UPDATE_LEVEL_ASK, EVENT_UPDATE_LEVEL_BID,
};

/// Count events processed by an L1 backtest over the real data file.
fn count_l1_events(file_path: &str) -> u64 {
    let src = BufSource::new_file(file_path);
    let feed = ParsedCandleFeed::new(src, HyperliquidCandleParser::new());
    let config = L1ConfigBuilder::new()
        .set_tick_size(0.1)
        .set_start_ts(1739491140000)
        .set_lines_read_per_tick(100)
        .set_return_window(1)
        .build()
        .unwrap();

    let mut bt = L1Backtest::new(&config, feed);
    let mut count = 0u64;
    while bt.step().is_ok() {
        count += 1;
    }
    count
}

/// Generate synthetic L2 orderbook events.
///
/// Creates `num_snapshots` orderbook snapshots, each with `levels_per_side` bid
/// and ask levels plus one trade. Timestamps are spaced 100ms apart.
fn generate_l2_events(num_snapshots: usize, levels_per_side: usize) -> (Vec<Event>, u64) {
    let mut events = Vec::with_capacity(num_snapshots * (2 + 2 * levels_per_side + 1));
    let base_ts: i64 = 1_000_000;

    for i in 0..num_snapshots {
        let ts = base_ts + (i as i64) * 100;
        let base_px = 2700.0 + (i % 50) as f64 * 0.1;

        // Clear both sides (snapshot reset)
        events.push(Event::new_with_symbol(
            EVENT_CLEAR_SIDE_BID,
            ts,
            "0.0",
            "0.0",
            "ETH",
        ));
        events.push(Event::new_with_symbol(
            EVENT_CLEAR_SIDE_ASK,
            ts,
            "0.0",
            "0.0",
            "ETH",
        ));

        // Bid levels
        for l in 0..levels_per_side {
            let px = base_px - (l as f64) * 0.1;
            let qty = 1.0 + (l as f64) * 0.5;
            events.push(Event::new_with_symbol(
                EVENT_UPDATE_LEVEL_BID,
                ts,
                &format!("{:.1}", px),
                &format!("{:.4}", qty),
                "ETH",
            ));
        }

        // Ask levels
        for l in 0..levels_per_side {
            let px = base_px + 0.1 + (l as f64) * 0.1;
            let qty = 1.0 + (l as f64) * 0.5;
            events.push(Event::new_with_symbol(
                EVENT_UPDATE_LEVEL_ASK,
                ts,
                &format!("{:.1}", px),
                &format!("{:.4}", qty),
                "ETH",
            ));
        }

        // One trade per snapshot
        let trade_type = if i % 2 == 0 {
            EVENT_TRADE_BUY
        } else {
            EVENT_TRADE_SELL
        };
        events.push(Event::new_with_symbol(
            trade_type,
            ts,
            &format!("{:.1}", base_px),
            "0.5000",
            "ETH",
        ));
    }

    let total = events.len() as u64;
    (events, total)
}

fn l1_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("l1");
    group.sample_size(10);

    let file_path = "20250214_ETH.gz";
    let event_count = count_l1_events(file_path);
    group.throughput(Throughput::Elements(event_count));

    group.bench_function("single_feed", |b| {
        b.iter(|| {
            let src = BufSource::new_file(black_box(file_path));
            let feed = ParsedCandleFeed::new(src, HyperliquidCandleParser::new());
            let config = L1ConfigBuilder::new()
                .set_tick_size(0.1)
                .set_start_ts(1739491140000)
                .set_lines_read_per_tick(100)
                .set_return_window(1)
                .build()
                .unwrap();

            let mut bt = L1Backtest::new(&config, feed);
            let mut events_processed = 0;

            while bt.step().is_ok() {
                events_processed += 1;
            }

            black_box(events_processed)
        })
    });

    group.finish();

    // Multiplexed feed gets its own group with 2x event count
    let mut mux_group = c.benchmark_group("l1_multiplexed");
    mux_group.sample_size(10);
    mux_group.throughput(Throughput::Elements(event_count * 2));

    mux_group.bench_function("two_feeds", |b| {
        b.iter(|| {
            let feed_a: Box<dyn EventFeed> = Box::new(ParsedCandleFeed::new(
                BufSource::new_file(black_box(file_path)),
                HyperliquidCandleParser::new(),
            ));
            let feed_b: Box<dyn EventFeed> = Box::new(ParsedCandleFeed::new(
                BufSource::new_file(black_box(file_path)),
                HyperliquidCandleParser::new(),
            ));

            let mux = MultiplexedFeed::new(vec![feed_a, feed_b]);
            let config = L1ConfigBuilder::new()
                .set_tick_size(0.1)
                .set_start_ts(1739491140000)
                .set_lines_read_per_tick(100)
                .set_return_window(1)
                .build()
                .unwrap();

            let mut bt = L1Backtest::new(&config, mux);
            let mut events_processed = 0;

            while bt.step().is_ok() {
                events_processed += 1;
            }

            black_box(events_processed)
        })
    });

    mux_group.finish();
}

fn l2_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("l2");
    group.sample_size(10);

    // 10k snapshots with 10 levels per side = 230k events
    let num_snapshots = 10_000;
    let levels_per_side = 10;
    let (events, event_count) = generate_l2_events(num_snapshots, levels_per_side);
    group.throughput(Throughput::Elements(event_count));

    group.bench_function("synthetic_orderbook", |b| {
        b.iter(|| {
            let feed = VecEventFeed::new(events.clone());
            let config = L2ConfigBuilder::new()
                .set_tick_size(0.1)
                .set_lot_size(0.001)
                .set_start_ts(0)
                .set_lines_read_per_tick(1024)
                .set_return_window(86400)
                .build()
                .unwrap();

            let mut bt = L2Backtest::new_with_level_chg_fill(&config, feed);
            let mut ticks = 0;

            // Elapse 100ms at a time (matches event spacing)
            while bt.elapse(100).is_ok() {
                ticks += 1;
            }

            black_box(ticks)
        })
    });

    group.finish();
}

/// Generate L2 events spread across multiple symbols.
fn generate_l2_multi_symbol_events(
    num_snapshots: usize,
    levels_per_side: usize,
    num_symbols: usize,
) -> (Vec<Event>, u64) {
    let symbols: Vec<String> = (0..num_symbols).map(|i| format!("SYM{}", i)).collect();
    let events_per_snapshot = 2 + 2 * levels_per_side + 1;
    let mut events = Vec::with_capacity(num_snapshots * events_per_snapshot * num_symbols);
    let base_ts: i64 = 1_000_000;

    for i in 0..num_snapshots {
        let ts = base_ts + (i as i64) * 100;

        for (si, sym) in symbols.iter().enumerate() {
            let base_px = 2700.0 + (i % 50) as f64 * 0.1 + si as f64 * 1000.0;

            events.push(Event::new_with_symbol(
                EVENT_CLEAR_SIDE_BID,
                ts,
                "0.0",
                "0.0",
                sym,
            ));
            events.push(Event::new_with_symbol(
                EVENT_CLEAR_SIDE_ASK,
                ts,
                "0.0",
                "0.0",
                sym,
            ));

            for l in 0..levels_per_side {
                let px = base_px - (l as f64) * 0.1;
                let qty = 1.0 + (l as f64) * 0.5;
                events.push(Event::new_with_symbol(
                    EVENT_UPDATE_LEVEL_BID,
                    ts,
                    &format!("{:.1}", px),
                    &format!("{:.4}", qty),
                    sym,
                ));
            }

            for l in 0..levels_per_side {
                let px = base_px + 0.1 + (l as f64) * 0.1;
                let qty = 1.0 + (l as f64) * 0.5;
                events.push(Event::new_with_symbol(
                    EVENT_UPDATE_LEVEL_ASK,
                    ts,
                    &format!("{:.1}", px),
                    &format!("{:.4}", qty),
                    sym,
                ));
            }

            let trade_type = if i % 2 == 0 {
                EVENT_TRADE_BUY
            } else {
                EVENT_TRADE_SELL
            };
            events.push(Event::new_with_symbol(
                trade_type,
                ts,
                &format!("{:.1}", base_px),
                "0.5000",
                sym,
            ));
        }
    }

    let total = events.len() as u64;
    (events, total)
}

/// Benchmarks that reveal cache pressure by scaling the working set.
///
/// System: L1D=64KB, L2=4MB, cache line=128B
fn cache_benchmarks(c: &mut Criterion) {
    // Dimension 1: Orderbook depth scaling (fixed 5k snapshots, 1 symbol)
    // More levels = larger orderbook HashMap = more cache pressure per lookup
    {
        let mut group = c.benchmark_group("cache_orderbook_depth");
        group.sample_size(10);

        for levels in [5, 20, 50, 100, 200] {
            let (events, event_count) = generate_l2_events(5_000, levels);
            group.throughput(Throughput::Elements(event_count));

            group.bench_function(format!("{}_levels_per_side", levels), |b| {
                b.iter(|| {
                    let feed = VecEventFeed::new(events.clone());
                    let config = L2ConfigBuilder::new()
                        .set_tick_size(0.1)
                        .set_lot_size(0.001)
                        .set_start_ts(0)
                        .set_lines_read_per_tick(1024)
                        .set_return_window(86400)
                        .build()
                        .unwrap();

                    let mut bt = L2Backtest::new_with_level_chg_fill(&config, feed);
                    let mut ticks = 0;
                    while bt.elapse(100).is_ok() {
                        ticks += 1;
                    }
                    black_box(ticks)
                })
            });
        }

        group.finish();
    }

    // Dimension 2: Multi-symbol scaling (fixed 5k snapshots, 10 levels)
    // More symbols = more Exchange instances = working set multiplied
    {
        let mut group = c.benchmark_group("cache_num_symbols");
        group.sample_size(10);

        for num_symbols in [1, 5, 10, 20, 50] {
            let (events, event_count) =
                generate_l2_multi_symbol_events(5_000, 10, num_symbols);
            group.throughput(Throughput::Elements(event_count));

            group.bench_function(format!("{}_symbols", num_symbols), |b| {
                b.iter(|| {
                    let feed = VecEventFeed::new(events.clone());
                    let config = L2ConfigBuilder::new()
                        .set_tick_size(0.1)
                        .set_lot_size(0.001)
                        .set_start_ts(0)
                        .set_lines_read_per_tick(1024)
                        .set_return_window(86400)
                        .build()
                        .unwrap();

                    let mut bt = L2Backtest::new_with_level_chg_fill(&config, feed);
                    let mut ticks = 0;
                    while bt.elapse(100).is_ok() {
                        ticks += 1;
                    }
                    black_box(ticks)
                })
            });
        }

        group.finish();
    }
}

criterion_group!(benches, l1_benchmarks, l2_benchmarks, cache_benchmarks);
criterion_main!(benches);
