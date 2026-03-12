//! Standalone benchmark binary for use with `perf stat`.
//!
//! Usage:
//!   perf stat -e L1-dcache-loads,L1-dcache-load-misses,L1-icache-load-misses,\
//!     dTLB-load-misses,instructions,cycles,branch-misses \
//!     ./target/release/examples/cache_bench [scenario]
//!
//! Scenarios: depth_5 depth_20 depth_50 depth_100 depth_200
//!            sym_1 sym_5 sym_10 sym_20 sym_50
//!            all (default)

use roshar_bt::l2::backtest::Backtest as L2Backtest;
use roshar_bt::l2::L2ConfigBuilder;
use roshar_bt::source::VecEventFeed;
use roshar_bt::types::{
    Event, EVENT_CLEAR_SIDE_ASK, EVENT_CLEAR_SIDE_BID, EVENT_TRADE_BUY, EVENT_TRADE_SELL,
    EVENT_UPDATE_LEVEL_ASK, EVENT_UPDATE_LEVEL_BID,
};

use std::hint::black_box;

fn generate_l2_events(num_snapshots: usize, levels_per_side: usize) -> Vec<Event> {
    let mut events = Vec::with_capacity(num_snapshots * (2 + 2 * levels_per_side + 1));
    let base_ts: i64 = 1_000_000;

    for i in 0..num_snapshots {
        let ts = base_ts + (i as i64) * 100;
        let base_px = 2700.0 + (i % 50) as f64 * 0.1;

        events.push(Event::new_with_symbol(EVENT_CLEAR_SIDE_BID, ts, "0.0", "0.0", "ETH"));
        events.push(Event::new_with_symbol(EVENT_CLEAR_SIDE_ASK, ts, "0.0", "0.0", "ETH"));

        for l in 0..levels_per_side {
            let px = base_px - (l as f64) * 0.1;
            let qty = 1.0 + (l as f64) * 0.5;
            events.push(Event::new_with_symbol(
                EVENT_UPDATE_LEVEL_BID, ts,
                &format!("{:.1}", px), &format!("{:.4}", qty), "ETH",
            ));
        }
        for l in 0..levels_per_side {
            let px = base_px + 0.1 + (l as f64) * 0.1;
            let qty = 1.0 + (l as f64) * 0.5;
            events.push(Event::new_with_symbol(
                EVENT_UPDATE_LEVEL_ASK, ts,
                &format!("{:.1}", px), &format!("{:.4}", qty), "ETH",
            ));
        }

        let trade_type = if i % 2 == 0 { EVENT_TRADE_BUY } else { EVENT_TRADE_SELL };
        events.push(Event::new_with_symbol(
            trade_type, ts, &format!("{:.1}", base_px), "0.5000", "ETH",
        ));
    }
    events
}

fn generate_l2_multi_symbol_events(
    num_snapshots: usize,
    levels_per_side: usize,
    num_symbols: usize,
) -> Vec<Event> {
    let symbols: Vec<String> = (0..num_symbols).map(|i| format!("SYM{}", i)).collect();
    let mut events = Vec::with_capacity(num_snapshots * (2 + 2 * levels_per_side + 1) * num_symbols);
    let base_ts: i64 = 1_000_000;

    for i in 0..num_snapshots {
        let ts = base_ts + (i as i64) * 100;
        for (si, sym) in symbols.iter().enumerate() {
            let base_px = 2700.0 + (i % 50) as f64 * 0.1 + si as f64 * 1000.0;
            events.push(Event::new_with_symbol(EVENT_CLEAR_SIDE_BID, ts, "0.0", "0.0", sym));
            events.push(Event::new_with_symbol(EVENT_CLEAR_SIDE_ASK, ts, "0.0", "0.0", sym));

            for l in 0..levels_per_side {
                let px = base_px - (l as f64) * 0.1;
                let qty = 1.0 + (l as f64) * 0.5;
                events.push(Event::new_with_symbol(
                    EVENT_UPDATE_LEVEL_BID, ts,
                    &format!("{:.1}", px), &format!("{:.4}", qty), sym,
                ));
            }
            for l in 0..levels_per_side {
                let px = base_px + 0.1 + (l as f64) * 0.1;
                let qty = 1.0 + (l as f64) * 0.5;
                events.push(Event::new_with_symbol(
                    EVENT_UPDATE_LEVEL_ASK, ts,
                    &format!("{:.1}", px), &format!("{:.4}", qty), sym,
                ));
            }

            let trade_type = if i % 2 == 0 { EVENT_TRADE_BUY } else { EVENT_TRADE_SELL };
            events.push(Event::new_with_symbol(
                trade_type, ts, &format!("{:.1}", base_px), "0.5000", sym,
            ));
        }
    }
    events
}

fn run_l2(events: Vec<Event>) -> usize {
    let feed = VecEventFeed::new(events);
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
    ticks
}

fn run_scenario(label: &str, events: Vec<Event>) {
    let n = events.len();
    // Warmup
    let _ = run_l2(events.clone());
    // Measured run
    let ticks = black_box(run_l2(events));
    eprintln!("  {} | events: {} | ticks: {}", label, n, ticks);
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let scenario = args.get(1).map(|s| s.as_str()).unwrap_or("all");

    eprintln!("roshar-bt L2 cache benchmark");
    eprintln!("============================");

    match scenario {
        "all" => {
            // Run a representative workload: 10k snapshots, 20 levels, 1 symbol
            let events = generate_l2_events(10_000, 20);
            run_scenario("10k snaps / 20 levels / 1 sym", events);
        }
        s if s.starts_with("depth_") => {
            let levels: usize = s[6..].parse().expect("usage: depth_N");
            let events = generate_l2_events(5_000, levels);
            run_scenario(&format!("5k snaps / {} levels / 1 sym", levels), events);
        }
        s if s.starts_with("sym_") => {
            let n_sym: usize = s[4..].parse().expect("usage: sym_N");
            let events = generate_l2_multi_symbol_events(5_000, 10, n_sym);
            run_scenario(&format!("5k snaps / 10 levels / {} syms", n_sym), events);
        }
        _ => {
            eprintln!("Unknown scenario: {}", scenario);
            eprintln!("Options: all, depth_5, depth_20, depth_50, depth_100, depth_200, sym_1, sym_5, sym_10, sym_20, sym_50");
            std::process::exit(1);
        }
    }
}
