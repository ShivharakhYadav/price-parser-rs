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

/// Currency codes ending in `D`, such as `NZD`, `SGD` and `USD`.
///
/// The trailing `D` stands for "dollar", so a code like `SGD$123` names the
/// currency more precisely than the bare `$` beside it and should win.
pub fn dollar_codes() -> &'static [&'static str] {
    static CODES: LazyLock<Vec<&'static str>> = LazyLock::new(|| {
        CURRENCY_CODES
            .iter()
            .copied()
            .filter(|code| code.ends_with('D'))
            .collect()
    });
    &CODES
}

/// Matches a dollar code at a word boundary, without the trailing condition.
fn dollar_code_regex() -> &'static Regex {
    static RE: LazyLock<Regex> = LazyLock::new(|| {
        let alternation = dollar_codes()
            .iter()
            .map(|code| regex::escape(code))
            .collect::<Vec<_>>()
            .join("|");
        Regex::new(&format!(r"\b(?:{alternation})")).expect("dollar code alternation must compile")
    });
    &RE
}

/// The condition upstream expresses as a lookahead, applied to what follows a
/// candidate code: an optional `$`, then a non-letter or the end of the text.
fn dollar_follower_regex() -> &'static Regex {
    static RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^\$?(?:[\W\d]|$)").expect("follower pattern must compile"));
    &RE
}

/// Find a dollar-style currency code, e.g. the `NZD` in `"NZD $123"`.
///
/// Upstream writes this as one regex ending in a lookahead:
///
/// ```text
/// \b(?:NZD|SGD|...)(?=\$?(?:[\W\d]|$))
/// ```
///
/// Rust's `regex` crate has no lookaround, so the assertion is split out: find
/// a code at a word boundary, then test the text after it against an anchored
/// pattern. Because the check is a separate step rather than part of the
/// match, a rejected candidate must not end the search -- scanning continues,
/// which is what the lookahead form does implicitly.
///
/// The `\b` matters more than it looks. In `"USDUSD "` the first `USD` is
/// rejected (a letter follows) and the second cannot match at all, since there
/// is no word boundary mid-run. Upstream returns nothing here, and so does
/// this.
pub fn find_dollar_code(text: &str) -> Option<&str> {
    dollar_code_regex()
        .find_iter(text)
        .find(|m| dollar_follower_regex().is_match(&text[m.end()..]))
        .map(|m| m.as_str())
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
    fn dollar_code_table_matches_upstream() {
        // Upstream's DOLLAR_CODES holds 43 entries at revision 64e213a.
        assert_eq!(dollar_codes().len(), 43);
        for code in ["USD", "NZD", "SGD", "CAD", "AUD", "AED"] {
            assert!(dollar_codes().contains(&code), "{code} missing");
        }
        assert!(
            dollar_codes().iter().all(|c| c.ends_with('D')),
            "every dollar code must end in D"
        );
    }

    #[test]
    fn dollar_code_matches_upstream_behaviour() {
        // Every expectation below was produced by running upstream's
        // _DOLLAR_REGEX.search() on the same input.
        let cases: &[(&str, Option<&str>)] = &[
            ("NZD $123", Some("NZD")),
            ("SGD$123", Some("SGD")),
            ("NZD", Some("NZD")),
            ("NZD100", Some("NZD")),
            ("NZD-5", Some("NZD")),
            ("NZD$", Some("NZD")),
            ("AED", Some("AED")),
            ("CAD\n", Some("CAD")),
            ("price: 100 CAD", Some("CAD")),
            ("USD USD", Some("USD")),
            // A letter, or an underscore, disqualifies the candidate.
            ("NZDX", None),
            ("NZD_", None),
            ("NZDa", None),
            // No word boundary before the code.
            ("xNZD $1", None),
            // Lowercase codes are not in the table.
            ("usd 5", None),
            ("$100", None),
        ];
        for (input, expected) in cases {
            assert_eq!(find_dollar_code(input), *expected, "for input {input:?}");
        }
    }

    #[test]
    fn rejected_candidate_does_not_stop_the_search() {
        // The lookahead form retries at later positions automatically; the
        // split form has to do it explicitly. Upstream yields AUD at span
        // (5, 8) here.
        assert_eq!(find_dollar_code("NZDX AUD"), Some("AUD"));
    }

    #[test]
    fn word_boundary_blocks_a_code_inside_a_letter_run() {
        // "USDUSD ": the first USD is followed by a letter so it is rejected,
        // and the second sits mid-run with no boundary before it. Upstream
        // returns None, and splitting the lookahead out must not change that.
        assert_eq!(find_dollar_code("USDUSD "), None);
    }

    #[test]
    fn no_duplicates_in_derived_tier() {
        let symbols = other_currency_symbols();
        let unique: HashSet<&&str> = symbols.iter().collect();
        assert_eq!(unique.len(), symbols.len());
    }
}
