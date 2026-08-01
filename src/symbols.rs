//! Currency symbol matching.
//!
//! Two tiers, mirroring upstream:
//!
//! * [`SAFE_CURRENCY_SYMBOLS`] -- unambiguous indicators, accepted wherever
//!   they appear in the text.
//! * [`other_currency_symbols`] -- three-letter codes and looser abbreviations,
//!   only consulted once the safe tier has found nothing.
//!
//! This module holds the logic; the literal tables it consumes are generated
//! into [`crate::currencies`].

use std::collections::HashSet;
use std::sync::LazyLock;

use regex::Regex;

use crate::currencies::{
    CURRENCY_CODES, CURRENCY_NATIONAL_SYMBOLS, CURRENCY_SYMBOLS, SAFE_CURRENCY_SYMBOLS,
};

/// Build a regex matching any of `symbols`.
///
/// Every symbol is escaped, so regex metacharacters such as the `.` in `"Nu."`
/// or `"руб."` match literally.
///
/// `regex` resolves alternations leftmost-first, matching Python's `re`. Two
/// rules follow, and both are pinned by tests:
///
/// 1. The earliest start position always wins, whatever order the branches are
///    listed in.
/// 2. Order decides only between branches that match at the *same* position,
///    where the earlier-listed branch is preferred even if a later one is
///    longer.
///
/// Callers are therefore responsible for listing longer candidates before the
/// shorter ones they begin with, or the shorter branch truncates the match.
///
/// # Panics
///
/// Panics if the resulting pattern fails to compile. The inputs are fixed
/// tables escaped at build time, so this cannot happen at run time.
pub fn or_regex(symbols: &[&str]) -> Regex {
    let pattern = symbols
        .iter()
        .map(|s| regex::escape(s))
        .collect::<Vec<_>>()
        .join("|");
    Regex::new(&pattern).expect("currency symbol alternation must compile")
}

/// Placeholder entries upstream drops explicitly.
const PLACEHOLDERS: [&str; 2] = ["-", "XXX"];

/// Returns true for a bare `A`--`Z`, which upstream excludes.
///
/// Mirrors subtracting `set(string.ascii_uppercase)`: a lone uppercase letter
/// is far too weak a signal to treat as a currency on its own.
fn is_bare_ascii_uppercase(s: &str) -> bool {
    let mut chars = s.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) => c.is_ascii_uppercase(),
        _ => false,
    }
}

/// The looser second tier of currency symbols, longest first.
///
/// Built as upstream does: the union of every generated table plus the two
/// bare Cyrillic rouble abbreviations, minus the safe tier, minus placeholders,
/// minus single uppercase ASCII letters.
///
/// # Ordering
///
/// Sorted by **character count** descending, not byte length. Upstream sorts
/// with Python's `len`, which counts code points; Rust's `str::len` counts
/// UTF-8 bytes. Using bytes would rank `€` (1 char, 3 bytes) alongside `US$`
/// (3 chars, 3 bytes) and quietly reorder the alternation.
///
/// Equal-length entries are then ordered lexicographically. Upstream leaves
/// these ties to Python set iteration order, which varies per process; pinning
/// them keeps this build reproducible. It cannot change behaviour, because two
/// distinct strings of equal length can never both match at the same position.
pub fn other_currency_symbols() -> &'static [&'static str] {
    static SYMBOLS: LazyLock<Vec<&'static str>> = LazyLock::new(|| {
        let safe: HashSet<&str> = SAFE_CURRENCY_SYMBOLS.iter().copied().collect();

        let mut candidates: HashSet<&'static str> = HashSet::new();
        candidates.extend(CURRENCY_CODES.iter().copied());
        candidates.extend(CURRENCY_SYMBOLS.iter().copied());
        candidates.extend(CURRENCY_NATIONAL_SYMBOLS.iter().copied());
        // Even where these appear in prose, the currency is almost certainly
        // roubles.
        candidates.insert("р");
        candidates.insert("Р");

        let mut out: Vec<&'static str> = candidates
            .into_iter()
            .filter(|s| !safe.contains(s))
            .filter(|s| !PLACEHOLDERS.contains(s))
            .filter(|s| !is_bare_ascii_uppercase(s))
            .collect();

        out.sort_by(|a, b| {
            b.chars()
                .count()
                .cmp(&a.chars().count())
                .then_with(|| a.cmp(b))
        });
        out
    });
    &SYMBOLS
}

/// Matcher for the safe tier, in upstream's hand-ordered sequence.
pub fn safe_currency_regex() -> &'static Regex {
    static RE: LazyLock<Regex> = LazyLock::new(|| or_regex(SAFE_CURRENCY_SYMBOLS));
    &RE
}

/// Matcher for the looser tier, longest candidates first.
pub fn other_currency_regex() -> &'static Regex {
    static RE: LazyLock<Regex> = LazyLock::new(|| or_regex(other_currency_symbols()));
    &RE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leftmost_position_beats_listed_order() {
        // The earliest start position always wins, whatever the ordering: at
        // index 0 of "US$100" only `US$` can match, so `$` never gets a look in.
        //
        // Cross-checked against CPython:
        //   re.search(r"\$|US\$", "US$100").group(0) == "US$"
        //   re.search(r"US\$|\$", "US$100").group(0) == "US$"
        assert_eq!(
            or_regex(&["US$", "$"]).find("US$100").unwrap().as_str(),
            "US$"
        );
        assert_eq!(
            or_regex(&["$", "US$"]).find("US$100").unwrap().as_str(),
            "US$"
        );
    }

    #[test]
    fn listed_order_decides_at_equal_positions() {
        // Ordering matters only between branches that start at the same index.
        // This is the case the tables are ordered for, and the reason bare `$`
        // sits near the end of upstream's hand-written safe list (index 32)
        // while `$U` sits at 19.
        //
        // Cross-checked against CPython:
        //   re.search(r"\$|\$U", "$U").group(0) == "$"
        //   re.search(r"\$U|\$", "$U").group(0) == "$U"
        assert_eq!(or_regex(&["$", "$U"]).find("$U").unwrap().as_str(), "$");
        assert_eq!(or_regex(&["$U", "$"]).find("$U").unwrap().as_str(), "$U");
    }

    #[test]
    fn symbols_are_escaped_not_interpreted() {
        let re = or_regex(&["Nu."]);
        assert!(re.is_match("Nu."));
        assert!(!re.is_match("Nux"), "the dot must be literal");
    }

    #[test]
    fn other_symbols_sorted_by_character_count_not_bytes() {
        let symbols = other_currency_symbols();
        let counts: Vec<usize> = symbols.iter().map(|s| s.chars().count()).collect();
        assert!(
            counts.windows(2).all(|w| w[0] >= w[1]),
            "must be descending by character count"
        );

        // Guard the actual trap: a multi-byte single character must sort as
        // length 1, well after three-character entries.
        let pos = |needle: &str| symbols.iter().position(|s| *s == needle);
        if let (Some(euro), Some(code)) = (pos("€"), pos("CHF")) {
            assert!(euro > code, "€ is 1 char and must follow 3-char entries");
        }
    }

    #[test]
    fn safe_tier_is_excluded_from_other_tier() {
        let symbols = other_currency_symbols();
        for safe in SAFE_CURRENCY_SYMBOLS {
            assert!(
                !symbols.contains(safe),
                "{safe} appears in both tiers; upstream subtracts the safe set"
            );
        }
    }

    #[test]
    fn placeholders_and_bare_letters_are_dropped() {
        let symbols = other_currency_symbols();
        assert!(!symbols.contains(&"-"));
        assert!(!symbols.contains(&"XXX"));
        for letter in 'A'..='Z' {
            let s = letter.to_string();
            assert!(
                !symbols.iter().any(|c| *c == s),
                "bare {s} must be excluded"
            );
        }
    }

    #[test]
    fn rouble_abbreviations_are_included() {
        let symbols = other_currency_symbols();
        assert!(symbols.contains(&"р"));
        assert!(symbols.contains(&"Р"));
    }

    #[test]
    fn multi_character_dollar_variants_beat_bare_dollar() {
        // The practical payoff of preserving upstream's hand-ordering.
        let re = safe_currency_regex();
        for (text, expected) in [
            ("CA$20", "CA$"),
            ("AU$20", "AU$"),
            ("US$20", "US$"),
            ("$20", "$"),
        ] {
            assert_eq!(re.find(text).unwrap().as_str(), expected, "for {text}");
        }
    }

    #[test]
    fn derived_tier_matches_upstream_size() {
        // Cross-checked against upstream at revision 64e213a: its
        // OTHER_CURRENCY_SYMBOLS holds 300 entries, and a full set diff against
        // this implementation came back identical. Pinned so a change to the
        // set arithmetic, the exclusions, or the generated tables is caught.
        assert_eq!(other_currency_symbols().len(), 300);
    }

    #[test]
    fn no_duplicates_in_derived_tier() {
        let symbols = other_currency_symbols();
        let unique: HashSet<&&str> = symbols.iter().collect();
        assert_eq!(unique.len(), symbols.len());
    }
}
