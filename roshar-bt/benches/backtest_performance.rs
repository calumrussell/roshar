use criterion::{black_box, criterion_group, criterion_main, Criterion};
use roshar_bt::exchanges::hyperliquid::HyperliquidCandleParser;
use roshar_bt::l1::backtest::Backtest;
use roshar_bt::l1::L1ConfigBuilder;
use roshar_bt::source::{BufSource, EventFeed, MultiplexedFeed, ParsedCandleFeed};

fn backtest_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("backtest");
    group.sample_size(10);

    let file_path = "20250214_ETH.gz";

    group.bench_function("backtest_l1_real_data", |b| {
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

            let mut bt = Backtest::new(&config, feed);
            let mut events_processed = 0;

            while bt.step().is_ok() {
                events_processed += 1;
            }

            black_box(events_processed)
        })
    });

    group.bench_function("backtest_l1_multiplexed_feed", |b| {
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

            let mut bt = Backtest::new(&config, mux);
            let mut events_processed = 0;

            while bt.step().is_ok() {
                events_processed += 1;
            }

            black_box(events_processed)
        })
    });

    group.finish();
}

criterion_group!(benches, backtest_benchmark);
criterion_main!(benches);
