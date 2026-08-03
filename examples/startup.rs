//! Parse one price and exit. Used to measure process startup cost.
//!
//! ```text
//! cargo run --release --example startup
//! ```
//!
//! Deliberately minimal: the point is to time everything a caller pays before
//! the first useful answer -- process launch, lazy table construction, regex
//! compilation -- not the parsing itself. The Python side of the comparison
//! spawns an interpreter and imports the module to reach the same point.

use std::hint::black_box;

use price_parser::Price;

fn main() {
    let price = Price::fromstring(Some("$12.99"), None, None, None);
    // Consume the result so nothing above can be optimised away.
    black_box(&price);
    assert!(price.amount.is_some());

    // `--hold <ms>` keeps the process alive after the work is done so a parent
    // can sample its memory. Reading our own RSS would mean an unsafe FFI call
    // for a benchmark, and the zero-unsafe guarantee is worth more than that.
    let mut args = std::env::args().skip(1);
    if args.next().as_deref() == Some("--hold") {
        if let Some(ms) = args.next().and_then(|v| v.parse().ok()) {
            std::thread::sleep(std::time::Duration::from_millis(ms));
        }
    }
}
