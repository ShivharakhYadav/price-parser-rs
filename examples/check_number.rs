//! Check the numeric helpers against tables of upstream's answers.
//!
//! ```text
//! cargo run --example check_number -- decimal-separator cases.tsv
//! ```
//!
//! Each line is `input<TAB>expected`, with `\N` for `None` and `\\`, `\t`,
//! `\n` escaped. Tables are produced by running the same inputs through
//! upstream, so this compares two implementations rather than restating one
//! set of assumptions twice.
//!
//! Exits non-zero on any disagreement.

use std::process::ExitCode;

use price_parser::number::get_decimal_separator;

const NONE: &str = "\\N";

/// Reverse the escaping applied by the generator.
fn unescape(field: &str) -> String {
    let mut out = String::with_capacity(field.len());
    let mut chars = field.chars();
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

fn run(which: &str, path: &str) -> Result<(usize, usize), String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("cannot read {path}: {e}"))?;

    let (mut total, mut mismatches) = (0usize, 0usize);
    for line in text.lines().filter(|l| !l.is_empty()) {
        let (raw_input, raw_expected) = line
            .split_once('\t')
            .ok_or_else(|| format!("malformed line: {line:?}"))?;
        let input = unescape(raw_input);

        let (got, expected) = match which {
            "decimal-separator" => {
                let got = get_decimal_separator(&input).map(|c| c.to_string());
                let expected = (raw_expected != NONE).then(|| raw_expected.to_string());
                (got, expected)
            }
            other => return Err(format!("unknown check: {other}")),
        };

        total += 1;
        if got != expected {
            mismatches += 1;
            if mismatches <= 20 {
                println!("MISMATCH input={input:?} expected={expected:?} got={got:?}");
            }
        }
    }
    Ok((total, mismatches))
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [which, path] = args.as_slice() else {
        eprintln!("usage: check_number <check> <cases.tsv>");
        return ExitCode::FAILURE;
    };

    match run(which, path) {
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
        Ok((total, 0)) => {
            println!("IDENTICAL - all {total} cases agree with upstream");
            ExitCode::SUCCESS
        }
        Ok((total, bad)) => {
            println!("{bad} of {total} cases disagree with upstream");
            ExitCode::FAILURE
        }
    }
}
