//! Guards on the generated currency tables.
//!
//! `src/currencies.rs` is machine-generated from upstream data, so these check
//! that generation actually produced what upstream holds rather than silently
//! emitting a truncated or mangled table.
//!
//! The expected counts are pinned against upstream at revision
//! `64e213a46a40473ba4f8aa3b249917fdc64d8a16`.

use std::collections::HashSet;

use price_parser::currencies::{CURRENCY_CODES, CURRENCY_NATIONAL_SYMBOLS, CURRENCY_SYMBOLS};

#[test]
fn table_sizes_match_upstream() {
    assert_eq!(CURRENCY_CODES.len(), 208, "currency code count drifted");
    assert_eq!(CURRENCY_SYMBOLS.len(), 172, "currency symbol count drifted");
    assert_eq!(
        CURRENCY_NATIONAL_SYMBOLS.len(),
        165,
        "national symbol count drifted"
    );
}

#[test]
fn no_entry_is_empty() {
    for (name, table) in [
        ("CURRENCY_CODES", CURRENCY_CODES),
        ("CURRENCY_SYMBOLS", CURRENCY_SYMBOLS),
        ("CURRENCY_NATIONAL_SYMBOLS", CURRENCY_NATIONAL_SYMBOLS),
    ] {
        assert!(
            table.iter().all(|s| !s.is_empty()),
            "{name} contains an empty entry"
        );
    }
}

#[test]
fn no_table_has_duplicates() {
    for (name, table) in [
        ("CURRENCY_CODES", CURRENCY_CODES),
        ("CURRENCY_SYMBOLS", CURRENCY_SYMBOLS),
        ("CURRENCY_NATIONAL_SYMBOLS", CURRENCY_NATIONAL_SYMBOLS),
    ] {
        let unique: HashSet<&&str> = table.iter().collect();
        assert_eq!(unique.len(), table.len(), "{name} contains duplicates");
    }
}

#[test]
fn codes_preserve_upstream_declaration_order() {
    // Upstream builds CURRENCY_CODES from dict keys, so order is meaningful
    // and stable. Sorting it here would be a real behavioural change.
    assert_eq!(CURRENCY_CODES[0], "AED");
    assert_eq!(CURRENCY_CODES[1], "AFN");
    assert!(CURRENCY_CODES.contains(&"USD"));
    assert!(CURRENCY_CODES.contains(&"NZD"));
}

#[test]
fn set_derived_tables_are_sorted() {
    // These come from Python sets, which have no stable order. The generator
    // sorts them so the emitted file is reproducible.
    let mut expected = CURRENCY_SYMBOLS.to_vec();
    expected.sort_unstable();
    assert_eq!(CURRENCY_SYMBOLS, expected.as_slice());

    let mut expected = CURRENCY_NATIONAL_SYMBOLS.to_vec();
    expected.sort_unstable();
    assert_eq!(CURRENCY_NATIONAL_SYMBOLS, expected.as_slice());
}

#[test]
fn right_to_left_marks_survived_generation() {
    // Several national symbols end in U+200F RIGHT-TO-LEFT MARK. It is
    // invisible, so a generator that dropped it would look correct on
    // inspection while quietly failing to match real input.
    let with_rtl: Vec<&&str> = CURRENCY_NATIONAL_SYMBOLS
        .iter()
        .filter(|s| s.contains('\u{200f}'))
        .collect();
    assert_eq!(
        with_rtl.len(),
        16,
        "expected 16 national symbols carrying U+200F, found {}",
        with_rtl.len()
    );
    assert!(CURRENCY_NATIONAL_SYMBOLS.contains(&"د.إ.\u{200f}"));
}

#[test]
fn known_symbols_are_present() {
    for sym in ["$", "€", "£", "¥", "₹"] {
        assert!(
            CURRENCY_SYMBOLS.contains(&sym) || CURRENCY_NATIONAL_SYMBOLS.contains(&sym),
            "expected {sym} somewhere in the symbol tables"
        );
    }
}

#[test]
fn ruble_sign_is_absent_from_the_data_tables() {
    // U+20BD RUBLE SIGN appears nowhere in upstream's currency data: the RUB
    // entry gives its native symbol as "руб.". The sign reaches the matcher
    // only through the hand-written SAFE_CURRENCY_SYMBOLS list in parser.py.
    //
    // Pinned deliberately. If a future regeneration starts emitting it, the
    // upstream data changed shape and the hand-written list needs rechecking
    // for newly duplicated entries.
    assert!(!CURRENCY_SYMBOLS.contains(&"₽"));
    assert!(!CURRENCY_NATIONAL_SYMBOLS.contains(&"₽"));
    assert!(CURRENCY_NATIONAL_SYMBOLS.contains(&"руб."));
}
