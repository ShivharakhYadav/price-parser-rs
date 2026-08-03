//! Time native Rust parsing over a corpus, for comparison against Python.
//!
//! ```text
//! cargo run --release --example bench -- corpus.txt [rounds] [repeats]
//! ```
//!
//! One input per line, with `\\`, `\t` and `\n` escaped. Prints JSON so
//! `tools/bench.py` can fold it into a single report.
//!
//! Two things are measured, because they answer different questions.
//!
//! **Throughput** is the best of several long rounds. Best rather than mean,
//! since the fastest observed run is least polluted by scheduling noise, and
//! the Python side is measured the same way so the bias points one direction.
//!
//! **Latency percentiles** are timed per parse in a separate pass. A mean hides
//! the tail, and the tail is what a caller actually notices. Timer overhead is
//! tens of nanoseconds against a parse of several microseconds, so it is
//! visible in the numbers but does not dominate them -- stated rather than
//! hidden, since a p99 quoted without that caveat would be overclaiming.
//!
//! Results are only meaningful from a release build.

use std::hint::black_box;
use std::process::ExitCode;
use std::time::Instant;

use price_parser::Price;

fn unescape(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('\\') => out.push('\\'),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

/// Percentile from an already-sorted slice, by nearest rank.
fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return f64::NAN;
    }
    let rank = (p / 100.0 * sorted.len() as f64).ceil() as usize;
    sorted[rank.saturating_sub(1).min(sorted.len() - 1)]
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let Some(path) = args.next() else {
        eprintln!("usage: bench <corpus.txt> [rounds] [repeats]");
        return ExitCode::FAILURE;
    };
    let rounds: usize = args.next().and_then(|r| r.parse().ok()).unwrap_or(5);
    // Passes over the corpus per round. One pass takes only a few milliseconds,
    // which is far too short to time reliably -- short rounds produced a table
    // where the FFI path looked faster than the native call it wraps.
    let repeats: usize = args.next().and_then(|r| r.parse().ok()).unwrap_or(1);

    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("cannot read {path}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let corpus: Vec<String> = text.lines().map(unescape).collect();
    if corpus.is_empty() {
        eprintln!("corpus is empty");
        return ExitCode::FAILURE;
    }

    // Warm up: first touch builds the lazily-initialised symbol tables and
    // compiles every regex, which would otherwise land entirely in round one.
    for input in &corpus {
        black_box(Price::fromstring(Some(input), None, None, None));
    }

    // --- throughput -------------------------------------------------------

    let mut best = f64::INFINITY;
    for _ in 0..rounds {
        let start = Instant::now();
        for _ in 0..repeats {
            for input in &corpus {
                black_box(Price::fromstring(Some(black_box(input)), None, None, None));
            }
        }
        best = best.min(start.elapsed().as_secs_f64());
    }
    let per_item = best / (corpus.len() * repeats) as f64;

    // --- latency distribution --------------------------------------------

    // Twenty passes gives enough samples that p99 is not decided by a handful
    // of outliers.
    const LATENCY_PASSES: usize = 20;
    let mut samples: Vec<f64> = Vec::with_capacity(corpus.len() * LATENCY_PASSES);
    for _ in 0..LATENCY_PASSES {
        for input in &corpus {
            let start = Instant::now();
            black_box(Price::fromstring(Some(black_box(input)), None, None, None));
            samples.push(start.elapsed().as_secs_f64());
        }
    }
    samples.sort_by(|a, b| a.partial_cmp(b).expect("no NaN timings"));

    println!(
        "{{\"items\": {}, \"rounds\": {}, \"repeats\": {}, \
         \"seconds_per_item\": {:.12}, \"items_per_second\": {:.1}, \
         \"latency_samples\": {}, \
         \"p50\": {:.12}, \"p90\": {:.12}, \"p99\": {:.12}, \"p999\": {:.12}, \"max\": {:.12}}}",
        corpus.len(),
        rounds,
        repeats,
        per_item,
        1.0 / per_item,
        samples.len(),
        percentile(&samples, 50.0),
        percentile(&samples, 90.0),
        percentile(&samples, 99.0),
        percentile(&samples, 99.9),
        samples[samples.len() - 1],
    );
    ExitCode::SUCCESS
}
