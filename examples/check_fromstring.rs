//! Check `Price::fromstring` field-by-field against upstream's answers.
//!
//! ```text
//! cargo run --example check_fromstring -- cases.tsv
//! ```
//!
//! Each line is
//! `price<TAB>hint<TAB>decsep<TAB>amount<TAB>currency<TAB>amount_text`, with
//! `\N` for `None`. The table is produced by running the same inputs through
//! upstream, so this compares two implementations rather than restating one set
//! of assumptions twice.
//!
//! Comparing each field separately matters: a port can get the amount right
//! while quietly losing the currency, and a whole-object check would report
//! that as one failure rather than showing which field drifted.
//!
//! Exits non-zero on any disagreement.

use std::process::ExitCode;

use price_parser::Price;

const NONE: &str = "\\N";

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

fn decode(field: &str) -> Option<String> {
    (field != NONE).then(|| unescape(field))
}

#[derive(Default)]
struct Failures {
    amount: usize,
    currency: usize,
    amount_text: usize,
}

fn main() -> ExitCode {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("usage: check_fromstring <cases.tsv>");
        return ExitCode::FAILURE;
    };
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("cannot read {path}: {e}");
            return ExitCode::FAILURE;
        }
    };

    let mut total = 0usize;
    let mut failed = 0usize;
    let mut by_field = Failures::default();

    for line in text.lines().filter(|l| !l.is_empty()) {
        let fields: Vec<&str> = line.split('\t').collect();
        let [price, hint, decsep, amount, currency, amount_text] = fields.as_slice() else {
            eprintln!("malformed line: {line:?}");
            return ExitCode::FAILURE;
        };

        let price = decode(price);
        let hint = decode(hint);
        let separator = decode(decsep).and_then(|s| s.chars().next());

        let got = Price::fromstring(price.as_deref(), hint.as_deref(), separator, None);

        let want_amount = decode(amount);
        let want_currency = decode(currency);
        let want_text = decode(amount_text);

        let got_amount = got.amount.map(|d| d.to_string());
        let amount_ok = got_amount == want_amount;
        let currency_ok = got.currency == want_currency;
        let text_ok = got.amount_text == want_text;

        total += 1;
        if !(amount_ok && currency_ok && text_ok) {
            failed += 1;
            by_field.amount += usize::from(!amount_ok);
            by_field.currency += usize::from(!currency_ok);
            by_field.amount_text += usize::from(!text_ok);
            if failed <= 25 {
                println!("MISMATCH price={price:?} hint={hint:?} sep={separator:?}");
                if !amount_ok {
                    println!("    amount      expected {want_amount:?} got {got_amount:?}");
                }
                if !currency_ok {
                    println!(
                        "    currency    expected {want_currency:?} got {:?}",
                        got.currency
                    );
                }
                if !text_ok {
                    println!(
                        "    amount_text expected {want_text:?} got {:?}",
                        got.amount_text
                    );
                }
            }
        }
    }

    println!();
    if failed == 0 {
        println!("IDENTICAL - all {total} cases agree with upstream on every field");
        ExitCode::SUCCESS
    } else {
        println!("{failed} of {total} cases disagree");
        println!(
            "  by field: amount {}, currency {}, amount_text {}",
            by_field.amount, by_field.currency, by_field.amount_text
        );
        ExitCode::FAILURE
    }
}
