//! Print `find_dollar_code` results for a generated matrix of inputs.
//!
//! ```text
//! cargo run --example dump_dollar_codes
//! ```
//!
//! Emits one `input<TAB>result` line per case so the output can be diffed
//! against the same matrix run through upstream's `_DOLLAR_REGEX`. Keeping the
//! matrix in both languages makes the split-out lookahead auditable rather than
//! merely asserted.

use price_parser::symbols::{dollar_codes, find_dollar_code};

/// Suffixes chosen to exercise each branch of the original lookahead:
/// attached dollar, separators, digits, end of input, and the letter and
/// underscore cases that must be rejected.
const SUFFIXES: [&str; 12] = [
    "", "$", "$123", " $123", "100", "-5", " ", "\n", "X", "_", "a", ".50",
];

const PREFIXES: [&str; 3] = ["", "price: ", "x"];

fn main() {
    for prefix in PREFIXES {
        for code in dollar_codes() {
            for suffix in SUFFIXES {
                let input = format!("{prefix}{code}{suffix}");
                let got = find_dollar_code(&input).unwrap_or("<none>");
                // Escape newlines so each case stays on one line.
                println!("{}\t{}", input.replace('\n', "\\n"), got);
            }
        }
    }
}
