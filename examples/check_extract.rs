//! Check `extract_currency_symbol` against a table of upstream's answers.
//!
//! ```text
//! cargo run --example check_extract -- cases.tsv
//! ```
//!
//! Each line is `price<TAB>hint<TAB>expected`, with `\N` standing for `None`.
//! The file is produced by running the same inputs through upstream, so this
//! is a direct differential check rather than a restatement of the same
//! assumptions in two places.
//!
//! Exits non-zero if anything disagrees.

use std::process::ExitCode;

use price_parser::symbols::extract_currency_symbol;

const NONE: &str = "\\N";

fn decode(field: &str) -> Option<&str> {
    if field == NONE {
        None
    } else {
        Some(field)
    }
}

fn main() -> ExitCode {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("usage: check_extract <cases.tsv>");
        return ExitCode::FAILURE;
    };
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("cannot read {path}: {e}");
            return ExitCode::FAILURE;
        }
    };

    let (mut total, mut mismatches) = (0usize, 0usize);
    for line in text.lines().filter(|l| !l.is_empty()) {
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() != 3 {
            eprintln!("malformed line: {line:?}");
            return ExitCode::FAILURE;
        }
        let (price, hint, expected) = (decode(fields[0]), decode(fields[1]), decode(fields[2]));

        total += 1;
        let got = extract_currency_symbol(price, hint);
        if got != expected {
            mismatches += 1;
            if mismatches <= 20 {
                println!(
                    "MISMATCH price={price:?} hint={hint:?} expected={expected:?} got={got:?}"
                );
            }
        }
    }

    if mismatches == 0 {
        println!("IDENTICAL - all {total} cases agree with upstream");
        ExitCode::SUCCESS
    } else {
        println!("{mismatches} of {total} cases disagree with upstream");
        ExitCode::FAILURE
    }
}
