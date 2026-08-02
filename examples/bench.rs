//! Time native Rust parsing over a corpus, for comparison against Python.
//!
//! ```text
//! cargo run --release --example bench -- corpus.txt [rounds]
//! ```
//!
//! One input per line, with `\\`, `\t` and `\n` escaped. Prints JSON so
//! `tools/bench.py` can fold it into a single table.
//!
//! Methodology matches the Python side deliberately: same corpus, same warmup,
//! and the **best** of several rounds reported rather than the mean, since the
//! fastest observed run is the one least polluted by scheduling noise. Results
//! are only meaningful from a release build.

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

    let mut best = f64::INFINITY;
    for _ in 0..rounds {
        let start = Instant::now();
        for _ in 0..repeats {
            for input in &corpus {
                black_box(Price::fromstring(Some(black_box(input)), None, None, None));
            }
        }
        let elapsed = start.elapsed().as_secs_f64();
        best = best.min(elapsed);
    }

    let parsed = corpus.len() * repeats;
    let per_item = best / parsed as f64;
    println!(
        "{{\"items\": {}, \"rounds\": {}, \"repeats\": {}, \"best_seconds\": {:.9}, \
         \"seconds_per_item\": {:.12}, \"items_per_second\": {:.1}}}",
        corpus.len(),
        rounds,
        repeats,
        best,
        per_item,
        1.0 / per_item,
    );
    ExitCode::SUCCESS
}
