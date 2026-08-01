//! Print the derived symbol tables as JSON, for cross-checking against
//! upstream.
//!
//! ```text
//! cargo run --example dump_symbols
//! ```
//!
//! Useful when verifying that the Rust set arithmetic reproduces upstream's,
//! since the two can be diffed directly.

use price_parser::symbols::other_currency_symbols;

fn main() {
    let mut sorted = other_currency_symbols().to_vec();
    sorted.sort_unstable();

    println!("{{");
    println!("  \"count\": {},", sorted.len());
    print!("  \"symbols\": [");
    for (i, s) in sorted.iter().enumerate() {
        if i > 0 {
            print!(", ");
        }
        // Minimal JSON string escaping; the tables contain no control
        // characters other than U+200F, which is valid raw in JSON.
        print!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""));
    }
    println!("]");
    println!("}}");
}
