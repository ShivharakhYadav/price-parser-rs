//! Check `extract_price_text` against a table of upstream's answers.
//!
//! ```text
//! cargo run --example check_text -- cases.tsv
//! ```
//!
//! Each line is `input<TAB>expected`, with `\N` for `None` and `\\`, `\t`, `\n`
//! escaped. The table is produced by running the same inputs through upstream,
//! so this compares two implementations rather than restating one set of
//! assumptions twice.
//!
//! Exits non-zero on any disagreement.

use std::process::ExitCode;

use price_parser::text::extract_price_text;

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

fn main() -> ExitCode {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("usage: check_text <cases.tsv>");
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
        let Some((raw_input, raw_expected)) = line.split_once('\t') else {
            eprintln!("malformed line: {line:?}");
            return ExitCode::FAILURE;
        };
        let input = unescape(raw_input);
        let expected = (raw_expected != NONE).then(|| unescape(raw_expected));

        total += 1;
        let got = extract_price_text(&input);
        if got != expected {
            mismatches += 1;
            if mismatches <= 20 {
                println!("MISMATCH input={input:?} expected={expected:?} got={got:?}");
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
